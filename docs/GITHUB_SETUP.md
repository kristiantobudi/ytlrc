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

## 5. Releases

The `release` workflow builds and attaches:

- `ytlrc-linux-x86_64` (static musl)
- `ytlrc-macos-arm64`
- `ytlrc-macos-x86_64`
- `ytlrc-windows-x86_64.exe`
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
- Package for Homebrew, Scoop, AUR, and Nix to appear in package search.
- Add the repository to your GitHub bio and link it from related discussions.

## 8. Verify discoverability

After publishing, search GitHub for:

- `youtube lyrics terminal`
- `synced lyrics cli`
- `youtube lyrics rust`
- `topic:ytlrc`

and check that the repository appears in the first results. Google indexing can
take a few days; a release and a few stars accelerate it.
