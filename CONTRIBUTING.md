# Contributing to ytlrc

Thanks for taking the time to improve ytlrc! This document explains how to set up
a development environment, run checks, and submit changes.

## Development setup

1. Install Rust (stable): <https://rustup.rs>
2. Make sure `curl` is available (used for LRCLIB requests).
3. Clone the repository and build:

   ```bash
   git clone https://github.com/kristiantobudi/ytlrc.git
   cd ytlrc
   cargo build
   ```

Linux users can build a static binary with `./build.sh`. Windows users can run
`.\build.ps1` for a native release build.

## Running locally

```bash
cargo run                     # TUI
cargo run -- --print          # headless
cargo run -- --theme nord     # pick a theme
```

The daemon listens on `127.0.0.1:8765` by default. To test without a browser:

```bash
curl -s -X POST http://127.0.0.1:8765/state \
  -H 'Content-Type: application/json' \
  -d '{"videoId":"dQw4w9WgXcQ","title":"Rick Astley - Never Gonna Give You Up (Official Video)","channel":"Rick Astley","duration":213,"position":83,"paused":false,"isAd":false,"source":"youtube"}'
```

Add `-H 'Authorization: Bearer <secret>'` when the daemon was started with
`--token <secret>`.

## Checks before opening a pull request

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Please keep the existing structure and naming style. New user-facing UI text is
English only; documentation may be added in English (`README.md`) and Indonesian
(`README.id.md`).

## Testing the browser extension

1. Open `chrome://extensions` and enable **Developer mode**.
2. Click **Load unpacked** and select the `extension/` folder.
3. After changing `content.js` or `background.js`, click the reload icon on the
   extension card and then reload the YouTube tab (content scripts are injected
   per page load).
4. Watch the daemon log printed in the terminal where `ytlrc` runs.

## Commit messages

Use short, imperative subjects, optionally following Conventional Commits:

```
feat: add theme cycling with the t key
fix: prefer mediaSession metadata after SPA navigation
docs: document macOS build steps
```

Add a body when the change is not obvious. Reference issues with `Fixes #123`.

## Pull request checklist

- [ ] `cargo fmt --check`, `cargo clippy`, and `cargo test` pass.
- [ ] The change was tried with a real YouTube / YouTube Music tab when relevant.
- [ ] User-facing text is in English.
- [ ] README / CHANGELOG updated for behaviour changes.
- [ ] No secrets, tokens, or personal paths committed.

## Ideas that fit the project

- Additional lyrics providers (with a small, dependency-free HTTP layer).
- More themes or a configurable theme file.
- Better title parsing for non-English titles.
- Publishing the `packaging/` manifests (Homebrew tap, Scoop bucket, AUR,
  winget) once release checksums are filled in — see
  [docs/GITHUB_SETUP.md](docs/GITHUB_SETUP.md#7-distribution-optional-but-effective).
- Accessibility and terminal compatibility improvements.

## Reporting bugs

Use the bug report form and include:

- OS and terminal (Windows Terminal, iTerm2, GNOME Terminal, …).
- `ytlrc --version` and the browser name/version.
- Whether the daemon was started natively or through WSL.
- The YouTube URL and the title shown in the header.
- Any error text from the terminal or the extension service worker console.
