use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use tower_http::cors::CorsLayer;

use crate::model::{PlaybackState, StateUpdate};

pub type PlaybackRef = Arc<Mutex<Option<PlaybackState>>>;

#[derive(Clone)]
struct Ctx {
    playback: PlaybackRef,
    started: Instant,
    version: &'static str,
}

pub fn router(playback: PlaybackRef, version: &'static str) -> Router {
    Router::new()
        .route("/state", get(get_state).post(post_state))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(Ctx {
            playback,
            started: Instant::now(),
            version,
        })
}

async fn post_state(State(ctx): State<Ctx>, Json(update): Json<StateUpdate>) -> Response {
    let mut guard = ctx.playback.lock().unwrap();

    // Saat iklan, jangan ganti lagu / posisi: cukup tandai sedang iklan.
    if update.is_ad {
        if let Some(prev) = guard.as_mut() {
            prev.is_ad = true;
            prev.received_at = Instant::now();
        }
        return (StatusCode::OK, Json(json!({ "ok": true, "ad": true }))).into_response();
    }

    let Some(next) = PlaybackState::from_update(update) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": "state kosong" })),
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
        None => (
            StatusCode::OK,
            Json(json!({ "ok": true, "state": null })),
        )
            .into_response(),
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
