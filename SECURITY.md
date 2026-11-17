# Security policy

## Supported versions

Security fixes are applied to the latest release. Please upgrade before
reporting an issue.

| Version | Supported |
| --- | --- |
| 0.2.x | ✅ |
| < 0.2 | ❌ |

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Use GitHub's private
vulnerability reporting:

<https://github.com/kristiantobudi/ytlrc/security/advisories/new>

Include the affected version, operating system, reproduction steps, and impact.
You will receive an acknowledgement within a few days.

## Security model and scope

- The daemon exposes an HTTP API used by the browser extension. It binds
  `127.0.0.1` by default. Binding `0.0.0.0` (used by the WSL launcher) makes it
  reachable from your LAN. Set `--token <secret>` (or `YTLRC_TOKEN`) in that case
  and the same value in `extension/background.js`; `/state` then requires
  `Authorization: Bearer <secret>`. Only do this on trusted networks.
- CORS is restricted to `https://www.youtube.com`, `https://music.youtube.com`,
  `https://m.youtube.com`, and browser-extension origins (`chrome-extension://`,
  `moz-extension://`). Requests without an `Origin` header (curl, scripts) are
  unaffected by CORS.
- The extension requests access to `youtube.com`, `music.youtube.com`, and
  `http://127.0.0.1:8765/*`. It reads playback metadata and sends it to the local
  daemon. No analytics or third-party endpoints are contacted by the extension.
- Lyrics requests go to `https://lrclib.net` through the system `curl` binary.
- No credentials are stored. The only local files are the theme config and the
  lyrics cache under the standard per-user directories; cache entries expire
  after 30 days and the directory is capped at 500 files.

Reports about third-party services (LRCLIB, YouTube) are out of scope.
