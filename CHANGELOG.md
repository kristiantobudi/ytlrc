# Changelog

All notable changes to this project are documented here. This project follows
[Semantic Versioning](https://semver.org/) and loosely follows
[Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- `NO_COLOR` (<https://no-color.org>) disables themed ANSI colors;
  `REDUCE_MOTION` disables the per-word karaoke highlight animation.
- Remappable single-key shortcuts (offset up/down/reset, theme next,
  synced/plain toggle, refetch, manual search) via `keybindings` in
  `config.json`.
- `lyrics::provider::Provider` trait: the LRCLIB client is now the
  default implementation behind a seam, so a second lyrics source can
  be added without touching cache/orchestration logic.
- ARM64 release binaries: `ytlrc-linux-arm64` (static musl) and
  `ytlrc-windows-arm64.exe`, built on native GitHub-hosted ARM64
  runners.
- Package manager manifests: Homebrew formula, Scoop manifest, AUR
  PKGBUILD, winget manifest, and a root `flake.nix` for Nix
  (`packaging/`). Checksums are placeholders until filled in per
  release — see `docs/GITHUB_SETUP.md`.
- Repository automation: `.github/CODEOWNERS`, `release-drafter`
  (auto-drafted release notes from merged PR labels), a PR auto-labeler
  keyed on changed paths, and a `FUNDING.yml` placeholder.

## [0.2.0] — 2026-11-17

### Added

- Five interactive themes: Neon, Dracula, Nord, Gruvbox, Mono (`t`, `1`–`5`,
  `--theme`), persisted in the user config.
- Automatic retrying with multiple query strategies and backoff
  (8s → 25s → 90s → 240s → every 5 minutes).
- Automatic re-lookup when the displayed lyrics no longer match the current
  track (self-healing watchdog).
- Cross-platform support and build helpers for Windows (`build.ps1`), Linux
  (static musl) and macOS.
- CI matrix for Linux, macOS and Windows; release workflow with prebuilt
  binaries; issue/PR templates, contributing, security and code of conduct docs.
- English and Indonesian documentation.
- Optional API token (`--token` / `YTLRC_TOKEN`) for WSL and LAN bindings, plus
  CORS restricted to YouTube and browser-extension origins.
- Automatic retries in `--print` mode, matching the TUI backoff.
- Startup cache maintenance: expired entries, stale temp files, and a 500-file
  cap (oldest removed first).

### Changed

- Playback metadata is now read from `mediaSession` first, falling back to
  `document.title` and DOM selectors, so SPA navigation no longer returns stale
  song titles.
- The active-tab rule prefers the focused tab and forwards paused updates, so a
  background tab cannot overwrite the song you are watching.
- Cache keys include the track title (per-track, not per-video), fixing reused
  lyrics for live streams and mixes.
- Default bind address is `127.0.0.1`; the WSL launcher opts into `0.0.0.0`.
- All UI text is English.

### Fixed

- Lyrics staying on the previous song after changing videos or titles.
- Stuck ad state when the track changed during an ad.
- Extension updates getting stuck forever when a daemon request hung (request
  timeout added).
- YouTube Shorts/embeds without `?v=` now resolve a video id from the media URL.
- Formatting of the test import and long curl error messages.

## [0.1.0] — 2026-11-17

### Added

- Initial release: browser extension, local Rust daemon, LRCLIB client with
  synced/plain lyrics, ratatui TUI, `--print` mode, per-video lyrics cache,
  manual search, offset adjustment, and ad handling.
