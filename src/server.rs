use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    extract::{Request, State},
    http::{
        header::{self, HeaderName},
        request::Parts,
        HeaderValue, Method, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::model::{PlaybackState, StateUpdate};

pub type PlaybackRef = Arc<Mutex<Option<PlaybackState>>>;

#[derive(Clone)]
struct Ctx {
    playback: PlaybackRef,
    started: Instant,
    version: &'static str,
    token: Option<Arc<str>>,
}

pub fn router(playback: PlaybackRef, version: &'static str, token: Option<String>) -> Router {
    let ctx = Ctx {
        playback,
        started: Instant::now(),
        version,
        token: token.map(Arc::from),
    };

    Router::new()
        .route("/state", get(get_state).post(post_state))
        .route("/health", get(health))
        .layer(cors())
        .layer(middleware::from_fn_with_state(ctx.clone(), require_token))
        .with_state(ctx)
}

/// The extension talks from a service worker (`chrome-extension://…`) or from a
/// YouTube page; browsers only need CORS headers for those origins. Requests
/// without an `Origin` header (curl, scripts, the extension itself) are not
/// subject to CORS and keep working.
fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-ytlrc-token"),
        ])
        .allow_origin(AllowOrigin::predicate(
            |origin: &HeaderValue, _parts: &Parts| is_allowed_origin(origin),
        ))
}

fn is_allowed_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    matches!(
        origin,
        "https://www.youtube.com" | "https://music.youtube.com" | "https://m.youtube.com"
    ) || origin.starts_with("chrome-extension://")
        || origin.starts_with("moz-extension://")
}

/// When `--token` is set, every endpoint except `/health` requires a bearer
/// token. This makes `--bind 0.0.0.0` (used by the WSL launcher and for LAN
/// setups) safe enough to keep on a trusted network.
async fn require_token(State(ctx): State<Ctx>, request: Request, next: Next) -> Response {
    let Some(expected) = ctx.token.as_deref() else {
        return next.run(request).await;
    };
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| {
            request
                .headers()
                .get("x-ytlrc-token")
                .and_then(|value| value.to_str().ok())
        });

    match provided {
        Some(provided) if constant_time_eq(provided.as_bytes(), expected.as_bytes()) => {
            next.run(request).await
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "ok": false, "error": "missing or invalid token" })),
        )
            .into_response(),
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

async fn post_state(State(ctx): State<Ctx>, Json(update): Json<StateUpdate>) -> Response {
    let mut guard = ctx.playback.lock().unwrap();

    // While an ad is playing, keep the current song and position so the
    // lyrics do not jump to the ad. If the track itself changed (for example
    // the viewer skipped to the next song during an ad), accept the new track
    // right away instead of freezing the old lyrics forever.
    if update.is_ad {
        let Some(next) = PlaybackState::from_update(update) else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "ok": false, "error": "empty state" })),
            )
                .into_response();
        };

        let changed = guard
            .as_ref()
            .map(|prev| prev.track_key() != next.track_key())
            .unwrap_or(true);

        if changed {
            *guard = Some(next);
        } else if let Some(prev) = guard.as_mut() {
            prev.is_ad = true;
            prev.received_at = Instant::now();
        }

        return (StatusCode::OK, Json(json!({ "ok": true, "ad": true }))).into_response();
    }

    let Some(next) = PlaybackState::from_update(update) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "empty state" })),
        )
            .into_response();
    };

    let track_changed = guard
        .as_ref()
        .map(|prev| prev.track_key() != next.track_key())
        .unwrap_or(true);
    *guard = Some(next);

    (
        StatusCode::OK,
        Json(json!({ "ok": true, "trackChanged": track_changed })),
    )
        .into_response()
}

async fn get_state(State(ctx): State<Ctx>) -> Response {
    let snapshot = ctx.playback.lock().unwrap().clone();
    match snapshot {
        Some(state) => (StatusCode::OK, Json(state.view())).into_response(),
        None => (StatusCode::OK, Json(json!({ "ok": true, "state": null }))).into_response(),
    }
}

async fn health(State(ctx): State<Ctx>) -> Response {
    let (has_state, age) = {
        let guard = ctx.playback.lock().unwrap();
        match guard.as_ref() {
            Some(state) => (true, state.received_at.elapsed().as_secs_f64()),
            None => (false, f64::NAN),
        }
    };
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "version": ctx.version,
            "uptimeSecs": ctx.started.elapsed().as_secs(),
            "hasState": has_state,
            "lastUpdateAgoSecs": age,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cors_allows_only_youtube_and_extension_origins() {
        for origin in [
            "https://www.youtube.com",
            "https://music.youtube.com",
            "https://m.youtube.com",
            "chrome-extension://abcdefghijklmnop",
            "moz-extension://abcdefghijklmnop",
        ] {
            assert!(
                is_allowed_origin(&HeaderValue::from_static(origin)),
                "expected {origin} to be allowed"
            );
        }

        for origin in [
            "https://evil.example",
            "http://www.youtube.com",
            "https://youtube.com.evil.example",
            "chrome-extensionx://abc",
        ] {
            assert!(
                !is_allowed_origin(&HeaderValue::from_static(origin)),
                "expected {origin} to be rejected"
            );
        }
    }

    #[test]
    fn token_comparison_requires_an_exact_match() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret "));
        assert!(!constant_time_eq(b"", b"secret"));
    }

    // --- integration tests: exercise the real axum router end to end ---

    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn app(token: Option<&str>) -> Router {
        router(
            Arc::new(Mutex::new(None)),
            "test",
            token.map(|value| value.to_string()),
        )
    }

    async fn post(
        app: Router,
        path: &str,
        body: serde_json::Value,
        token: Option<&str>,
    ) -> Response {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = builder.body(Body::from(body.to_string())).unwrap();
        app.oneshot(request).await.unwrap()
    }

    async fn get(app: Router, path: &str, token: Option<&str>) -> Response {
        let mut builder = Request::builder().method("GET").uri(path);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = builder.body(Body::empty()).unwrap();
        app.oneshot(request).await.unwrap()
    }

    async fn json_body(response: Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn sample_state(title: &str) -> serde_json::Value {
        json!({
            "videoId": "abc123",
            "title": title,
            "artist": "Someone",
            "duration": 200.0,
            "position": 10.0,
            "paused": false,
            "isAd": false,
            "source": "youtube",
        })
    }

    #[tokio::test]
    async fn health_reports_no_state_before_any_update() {
        let response = get(app(None), "/health", None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["hasState"], false);
        assert_eq!(body["version"], "test");
    }

    #[tokio::test]
    async fn post_state_then_get_state_round_trips() {
        let app = app(None);
        let response = post(app.clone(), "/state", sample_state("Song A"), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["trackChanged"], true);

        let response = get(app, "/state", None).await;
        let body = json_body(response).await;
        assert_eq!(body["title"], "Song A");
    }

    #[tokio::test]
    async fn empty_state_is_rejected_with_bad_request() {
        let response = post(
            app(None),
            "/state",
            json!({ "videoId": "", "title": "" }),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn malformed_json_is_rejected() {
        let request = Request::builder()
            .method("POST")
            .uri("/state")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("not json"))
            .unwrap();
        let response = app(None).oneshot(request).await.unwrap();
        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn ad_freezes_track_but_accepts_a_real_track_change_during_the_ad() {
        let app = app(None);
        post(app.clone(), "/state", sample_state("Song A"), None).await;

        // Ad starts: same track key, is_ad=true -> frozen, song position unchanged.
        let mut ad_update = sample_state("Song A");
        ad_update["isAd"] = json!(true);
        let response = post(app.clone(), "/state", ad_update, None).await;
        let body = json_body(response).await;
        assert_eq!(body["ad"], true);

        let response = get(app.clone(), "/state", None).await;
        let body = json_body(response).await;
        assert_eq!(body["title"], "Song A");

        // Viewer skips to a new track while the ad plays: the new track wins.
        let mut new_track_during_ad = sample_state("Song B");
        new_track_during_ad["isAd"] = json!(true);
        post(app.clone(), "/state", new_track_during_ad, None).await;

        let response = get(app, "/state", None).await;
        let body = json_body(response).await;
        assert_eq!(body["title"], "Song B");
    }

    #[tokio::test]
    async fn token_is_required_on_state_but_not_on_health() {
        let app = app(Some("secret"));

        let response = get(app.clone(), "/health", None).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "health never needs a token"
        );

        let response = get(app.clone(), "/state", None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = get(app.clone(), "/state", Some("wrong")).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = get(app, "/state", Some("secret")).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
