# Publishing and SEO checklist

This is a one-time (plus occasional) checklist to make the repository easy to
find on GitHub and search engines. It uses the GitHub CLI (`gh`), but everything
can also be done in the repository settings UI.

## 1. Repository description

GitHub search weighs the description heavily. Keep it under 350 characters and
put the main keywords first:

> Open-source YouTube lyrics in your terminal. Tiny Rust daemon + Chrome
> extension with synced LRC lyrics for YouTube & YouTube Music on Windows,
> macOS, and Linux. Auto-detects song changes, 5 themes, LRCLIB, no API key.

Set it with:

```bash
gh repo edit kristiantobudi/ytlrc \
  --description "Open-source YouTube lyrics in your terminal. Tiny Rust daemon + Chrome extension with synced LRC lyrics for YouTube & YouTube Music on Windows, macOS, and Linux. Auto-detects song changes, 5 themes, LRCLIB, no API key." \
  --homepage "https://github.com/kristiantobudi/ytlrc"
```

## 2. Topics

Topics drive GitHub topic pages and search. Use all 20 relevant ones:

```bash
gh repo edit kristiantobudi/ytlrc \
  --add-topic youtube \
  --add-topic youtube-music \
  --add-topic lyrics \
  --add-topic synced-lyrics \
  --add-topic lrc \
  --add-topic lrclib \
  --add-topic terminal \
  --add-topic tui \
  --add-topic ratatui \
  --add-topic rust \
  --add-topic cli \
  --add-topic music \
  --add-topic now-playing \
  --add-topic karaoke \
  --add-topic windows \
  --add-topic macos \
  --add-topic linux \
  --add-topic chrome-extension \
  --add-topic browser-extension \
  --add-topic open-source
```

Suggested priorities if you want fewer: `youtube`, `lyrics`, `terminal`, `tui`,
`rust`, `cli`, `synced-lyrics`, `chrome-extension`, `lrclib`, `now-playing`.

## 3. Social preview

GitHub shows a preview card when the repository is shared. In
**Settings → General → Social preview**, upload `docs/social-preview.png`
(1280×640, ~68 KB). It is also displayed at the top of both READMEs.

The included card contains:

- Big wordmark: `ytlrc`
- Tagline: “Synced YouTube lyrics in your terminal”
- Platform pills: `Windows · macOS · Linux · Open source`
- A terminal mock with the active lyric line highlighted

Regenerate or tweak it with PIL/any image editor and keep the 1280×640 size.

## 4. Repository settings

- Enable **Discussions** (Settings → General → Features) for usage questions and
  show-and-tell; search engines index these too.
- Enable **Private vulnerability reporting** (Settings → Code security).
- Disable the wiki unless you plan to maintain it; README docs rank better.
- Add the repository to a **public** profile and pin it (Profile → Customize your
  pins).
- Use a concise, descriptive release title, e.g. `v0.2.0 — reliable song switching
  + cross-platform builds`.
- Enable **GitHub Pages** (Settings → Pages → Source: Deploy from a branch,
  Branch: `main`, folder: `/docs`) to publish `docs/index.html` as a short demo
  page at `https://kristiantobudi.github.io/ytlrc/`.

## 5. Releases

The `release` workflow builds and attaches:

- `ytlrc-linux-x86_64` (static musl)
- `ytlrc-linux-arm64` (static musl)
- `ytlrc-macos-arm64`
- `ytlrc-macos-x86_64`
- `ytlrc-windows-x86_64.exe`
- `ytlrc-windows-x86_64.msi` (WiX installer, built from `packaging/windows/`)
- `ytlrc-windows-arm64.exe`
- `ytlrc-extension.zip`

Cut a release:

```bash
git tag -a v0.2.0 -m "ytlrc 0.2.0"
git push origin v0.2.0
```

Then edit the generated release notes to include a short “What’s new” and the
installation one-liner for each OS.

## 6. README SEO checklist

Already included:

- Keyword-rich title and first paragraph (“synced YouTube lyrics in terminal”).
- Badges (CI, license, version, Rust, platforms) for trust signals.
- Quick start within the first screen.
- Table of contents with question-style FAQ headings (long-tail searches such as
  “youtube lyrics in terminal windows”).
- Comparison table, platform matrix, config paths, troubleshooting, contributing.
- English (`README.md`) and Indonesian (`README.id.md`) versions with reciprocal
  links.

When adding screenshots/GIFs, always set descriptive alt text, for example:

```markdown
![ytlrc terminal UI showing synced YouTube lyrics in the Dracula theme](docs/screenshot.png)
```

## 7. Distribution (optional but effective)

- Submit to `awesome-rust`, `awesome-tuis`, `awesome-cli-apps`, and
  `awesome-youtube` style lists (follow each list’s contribution rules).
- Post a short demo in r/rust, r/commandline, Hacker News “Show HN”, and
  r/youtube (check each community’s self-promotion rules).
- Add the repository to your GitHub bio and link it from related discussions.

### Package manager manifests

Ready-to-use manifests live in `packaging/` and `flake.nix`, but none are
published to an official tap/bucket/repo yet. After cutting a release:

1. Compute the SHA256 of each release asset:

   ```bash
   gh release download v0.2.0 --repo kristiantobudi/ytlrc --pattern 'ytlrc-*'
   sha256sum ytlrc-macos-arm64 ytlrc-macos-x86_64 ytlrc-windows-x86_64.exe
   ```

2. Replace every `REPLACE_WITH_SHA256_OF_*` placeholder in:
   - `packaging/homebrew/ytlrc.rb` (both macOS binaries)
   - `packaging/scoop/ytlrc.json`
   - `packaging/winget/kristiantobudi.ytlrc.installer.yaml`
   - `packaging/aur/PKGBUILD` (hash of the `v0.2.0` source tarball, not a
     binary — `sha256sum` the file from
     `https://github.com/kristiantobudi/ytlrc/archive/refs/tags/v0.2.0.tar.gz`)
3. **Homebrew**: push `packaging/homebrew/ytlrc.rb` to a new
   `kristiantobudi/homebrew-ytlrc` repo (Homebrew tap naming convention),
   then `brew tap kristiantobudi/ytlrc && brew install ytlrc`.
4. **Scoop**: push `packaging/scoop/ytlrc.json` to a new
   `kristiantobudi/scoop-ytlrc` bucket repo, then
   `scoop bucket add ytlrc https://github.com/kristiantobudi/scoop-ytlrc`.
5. **AUR**: create the `ytlrc` AUR package (needs an Arch Linux account),
   push `packaging/aur/PKGBUILD` plus a generated `.SRCINFO`
   (`makepkg --printsrcinfo > .SRCINFO`).
6. **winget**: open a PR against
   [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) adding
   `manifests/k/kristiantobudi/ytlrc/0.2.0/` with the three
   `packaging/winget/*.yaml` files (`winget validate` first).
7. **Nix**: nothing to publish — `flake.nix` at the repo root already lets
   anyone run `nix run github:kristiantobudi/ytlrc`.

### Windows MSI / code signing

`ytlrc-windows-x86_64.msi` is built and attached automatically from
`packaging/windows/ytlrc.wxs` (WiX v4) on every tagged release. It is **not
code-signed**, so Windows SmartScreen will warn on first run — same as the
plain `.exe`. To remove that warning, buy a code-signing certificate (EV
certs avoid the SmartScreen reputation delay entirely) and sign both
`ytlrc-windows-x86_64.exe` and the `.msi` in the release job with
`signtool sign /fd SHA256 /a <file>` before the "Attach to release" step.

## 8. Verify discoverability

After publishing, search GitHub for:

- `youtube lyrics terminal`
- `synced lyrics cli`
- `youtube lyrics rust`
- `topic:ytlrc`

and check that the repository appears in the first results. Google indexing can
take a few days; a release and a few stars accelerate it.
