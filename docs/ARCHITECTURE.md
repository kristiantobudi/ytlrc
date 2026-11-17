# Architecture

ytlrc is intentionally small: one Rust binary that is both an HTTP daemon and a
terminal UI, plus a Manifest V3 browser extension.

```
browser tab ── content.js ──runtime message──▶ background.js ──HTTP──▶ daemon
                                                                       │
                                                       ┌───────────────┼───────────────┐
                                                       ▼               ▼               ▼
                                                  playback state   lyrics cache    TUI / print
```

## Components

| Path | Responsibility |
| --- | --- |
| `extension/content.js` | Reads `mediaSession`, title/artist, `currentTime`, duration and ad state every second (and after SPA navigation). |
| `extension/background.js` | Chooses the tab being watched, coalesces updates, posts them with a request timeout. |
| `src/server.rs` | axum server: `POST /state`, `GET /state`, `GET /health`. Freezes position during ads but accepts track changes. |
| `src/model.rs` | Validates updates, extrapolates the current position, computes the track identity used for change detection. |
| `src/lyrics/title.rs` | Cleans YouTube titles, guesses artist/title, builds fallback query candidates. |
| `src/lyrics/lrclib.rs` | Calls LRCLIB through `curl`, scores results by title, artist, duration, and synced availability. |
| `src/lyrics/lrc.rs` | Parses LRC (including hours, fractions, offsets, enhanced word tags). |
| `src/lyrics/cache.rs` | Per-track JSON cache: 30 days for hits, 10 minutes for misses. |
| `src/lyrics/mod.rs` | Orchestrates cache, the candidate loop, and plain-lyrics fallback. |
| `src/ui.rs` | ratatui TUI, themes, retry state machine, self-healing lookup, `--print` mode. |
| `src/theme.rs` | Five 24-bit color themes. |
| `src/config.rs` | Stores the selected theme in the per-user config directory. |

## Track identity and change detection

`PlaybackState::track_key()` is `video_id + normalized title` when a video id is
known, and `artist + title` otherwise. Including the title is what makes live
streams and mixes switch lyrics without changing the video. The TUI stores the
key of the lyrics on screen (`view_key`); if it ever differs from the current
key, a fresh lookup starts automatically.

## Lookup and retry flow

1. New key → parse title, set `view_key`, spawn a lookup.
2. `lyrics::fetch` checks the cache, then tries up to six query candidates.
3. Synced lyrics win; plain lyrics are kept as a fallback; network errors are not
   cached.
4. On failure the UI schedules a retry with exponential backoff
   (8s, 25s, 90s, 240s, 300s) until the track changes or the user searches
   manually.

## Concurrency

The daemon and the TUI share `Arc<Mutex<Option<PlaybackState>>>`. Lookups run in
detached tokio tasks and report back through an unbounded channel tagged with the
track key, so late results for a previous song are discarded instead of
overwriting the current one.

## Security boundaries

- The HTTP API has no authentication; it binds loopback by default and is meant
  for localhost only.
- Lyrics requests are outbound HTTPS to LRCLIB via the system `curl`.
- No telemetry; the extension only talks to the local daemon.
