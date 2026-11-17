# ytlrc — lirik YouTube di terminal

Daemon ringan (Rust) + browser extension yang mendeteksi lagu yang sedang diputar
di YouTube / YouTube Music, mengambil lirik dari [LRCLIB](https://lrclib.net),
lalu menampilkannya secara sinkron di Command Prompt / Windows Terminal / terminal Linux.

```
┌─ Now Playing ─────────────────────────────────────┐
│ ▶ Rick Astley — Never Gonna Give You Up    SYNCED │
│ youtube • dQw4w9WgXcQ  •  tema Dracula            │
├───────────────────────────────────────────────────┤
│        We're no strangers to love                 │
│        You know the rules and so do I             │
│   ▶    A full commitment's what I'm thinking of   │
│        You wouldn't get this from any other guy   │
├───────────────────────────────────────────────────┤
│ 01:23 / 03:33   offset +0.00s                     │
│ q keluar · t tema · +/- offset · p synced/plain   │
└───────────────────────────────────────────────────┘
```

## Cara kerja

```
Chrome/Edge (youtube.com, music.youtube.com)
  content.js  ── baca judul, channel, currentTime, duration, status iklan
      │ chrome.runtime.sendMessage (≈1 detik / saat berubah)
      ▼
  background.js ── pilih tab yang audible ── POST /state
                                               │
                                               ▼
                                    Rust daemon (127.0.0.1:8765)
                                      • parser judul → artis/judul
                                      • LRCLIB (/api/get → /api/search)
                                      • cache di disk
                                      • parser LRC + offset
                                               │
                                               ▼
                                       TUI ratatui di terminal
```

Deteksi memakai metadata halaman, **bukan** audio fingerprinting. Karena itu
posisi lagu (`currentTime`) bisa dipakai langsung untuk sinkronisasi lirik.

## Fitur

- Deteksi otomatis lagu + sinkron lirik (LRC) dengan highlight baris aktif.
- **5 tema interaktif**: Neon, Dracula, Nord, Gruvbox, Mono (ganti tanpa restart).
- **Pencarian otomatis berlapis**: kalau lirik tidak ketemu, ytlrc mencoba query
  lain sendiri (artis utama, nama channel, judul saja, judul mentah) dengan
  jeda 8s → 25s → 90s → 240s → tiap 5 menit, tanpa perlu menekan tombol.
- Lagu berganti (termasuk live stream yang ganti judul di video yang sama)
  otomatis memicu pencarian baru.
- Cache lirik per video (30 hari) dan cache "tidak ditemukan" (10 menit).
- Tahan iklan: lirik dijeda saat iklan, tidak ganti lagu.
- Multi-tab: hanya tab YouTube yang berbunyi yang dikirim.
- Offset manual (`+` / `-`), toggle synced/plain, refetch, pencarian manual.
- Mode `--print` untuk debugging/script (tanpa TUI).
- Satu binary statis, tanpa runtime. RAM tipikal ~10–15 MB.

## Prasyarat

- **WSL2 + Ubuntu** (rekomendasi) atau Linux apa pun.
- `curl` di PATH (dipakai untuk request ke LRCLIB).
- Chrome / Edge / Brave (Chromium, Manifest V3).
- Untuk build:
  - `rustup` (Rust stable), dan
  - linker. Di mesin ini **tidak ada gcc/libc-dev**, jadi project di-set build
    statis **musl** memakai `rust-lld` + **Zig** sebagai linker build-script.
  - Kalau di mesinmu ada `build-essential`, kamu boleh menghapus
    `.cargo/config.toml` lalu `cargo build --release` biasa.

## Build

Cara termudah (otomatis menyiapkan target musl, `ld.lld`, dan Zig bila perlu):

```bash
cd /mnt/d/Project/ytlrc
chmod +x build.sh
./build.sh
```

Hasil: `target/x86_64-unknown-linux-musl/release/ytlrc` (statis, bisa dipindah-pindah),
plus salinan `./ytlrc`.

<details>
<summary>Langkah manual (kalau ingin tahu detailnya)</summary>

```bash
# 1) Rust (kalau belum ada)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"
rustup target add x86_64-unknown-linux-musl

# 2) linker statis (tanpa sudo): rust-lld + zig cc
SYSROOT=$(rustc --print sysroot)
ln -sf "$SYSROOT/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld" "$HOME/.local/bin/ld.lld"
#    Zig portable (dipakai build script Rust saat tidak ada gcc)
#    https://ziglang.org/download/  → ekstrak ke ~/.local/opt, lalu:
ln -sf "$HOME/.local/opt/zig/zig" "$HOME/.local/bin/zig"
printf '#!/bin/sh\nexec "$HOME/.local/opt/zig/zig" cc "$@"\n' > "$HOME/.local/bin/cc-zig"
chmod +x "$HOME/.local/bin/cc-zig"

# 3) build
cd /mnt/d/Project/ytlrc
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc-zig cargo build --release
```
</details>

> Di lingkungan ini, `zig` + `cc-zig` hanya dibutuhkan untuk meng-link
> **build script** (target host). Binary akhirnya tetap 100% statis musl.
> Kalau mesinmu punya `build-essential`, kamu boleh menghapus
> `.cargo/config.toml` lalu `cargo build --release` biasa.

## Menjalankan

Dari terminal WSL:

```bash
cd /mnt/d/Project/ytlrc
./target/x86_64-unknown-linux-musl/release/ytlrc
```

Dari **Command Prompt** (Windows):

```bat
D:\Project\ytlrc\ytlrc.cmd
```

Atau langsung:

```bat
wsl -e bash -lc "exec /mnt/d/Project/ytlrc/target/x86_64-unknown-linux-musl/release/ytlrc"
```

Gunakan **Windows Terminal** agar Unicode, warna, dan gauge tampil rapi.
Kalau memakai `cmd.exe` lama, jalankan `chcp 65001` lebih dulu.

Opsi:

```
--bind <addr>   alamat bind (default 0.0.0.0, supaya bisa diakses dari Windows)
--port <port>   port daemon (default 8765)
--theme <t>     tema awal: Neon, Dracula, Nord, Gruvbox, Mono (atau 1-5)
--print         tanpa TUI; cetak baris lirik aktif ke stdout
-h, --help
-V, --version
```

## Install extension (sekali saja)

1. Buka `chrome://extensions` (atau `edge://extensions`).
2. Aktifkan **Developer mode**.
3. Klik **Load unpacked**, pilih folder `D:\Project\ytlrc\extension`.
4. Pin extension **ytlrc bridge** (tidak perlu diklik; berjalan otomatis).
5. Pastikan daemon `ytlrc` sudah berjalan, lalu putar lagu di YouTube.

Kalau port diganti, ubah juga `DAEMON_URL` di `extension/background.js`
dan `host_permissions` di `extension/manifest.json`, lalu reload extension.

## Tes cepat tanpa browser

Jalankan daemon, lalu kirim state palsu:

```bash
curl -s http://127.0.0.1:8765/health
curl -s -X POST http://127.0.0.1:8765/state \
  -H 'Content-Type: application/json' \
  -d '{"videoId":"dQw4w9WgXcQ","title":"Rick Astley - Never Gonna Give You Up (Official Video)","channel":"Rick Astley","duration":213,"position":83,"paused":false,"isAd":false,"source":"youtube"}'
curl -s http://127.0.0.1:8765/state
```

Mode `--print` cocok untuk mengecek alur tanpa TUI:

```bash
./target/x86_64-unknown-linux-musl/release/ytlrc --print
```

## Theme

| # | Nama | Nuansa |
| --- | --- | --- |
| 1 | **Neon** | cyan terang di atas hitam |
| 2 | **Dracula** | magenta/ungu khas Dracula |
| 3 | **Nord** | biru dingin |
| 4 | **Gruvbox** | kuning/oranye hangat |
| 5 | **Mono** | hitam-putih, fokus keterbacaan |

- `t` = tema berikutnya, `1`–`5` = pilih langsung.
- Tema memakai warna 24-bit (truecolor); Windows Terminal mendukung penuh.
- Tema terakhir disimpan di `~/.config/ytlrc/config.json` dan dipakai lagi saat
  start berikutnya.
- `--theme dracula` (atau `--theme 2`) untuk memilih saat start; pilihan itu
  juga disimpan.

## Keybinding TUI

| Tombol | Fungsi |
| --- | --- |
| `q` / `Esc` | keluar |
| `+` / `-` | geser offset lirik 0.25 detik |
| `0` | reset offset |
| `p` | toggle synced ↔ plain |
| `t` | tema berikutnya |
| `1`–`5` | pilih tema langsung |
| `r` | cari ulang sekarang (abaikan cache) |
| `m` | cari manual `Artis - Judul` |
| `↑` `↓` `PgUp` `PgDn` | scroll mode plain |
| `Ctrl+C` | keluar |

## Troubleshooting

**Extension tidak connect / status MENUNGGU**
- Pastikan daemon hidup: `curl http://127.0.0.1:8765/health`.
- Daemon default bind `0.0.0.0` supaya Windows bisa menjangkau WSL lewat
  localhost forwarding. Kalau tetap gagal, cek IP WSL (`hostname -I`) dan
  ganti `DAEMON_URL` ke `http://<ip-wsl>:8765/state`, tambahkan juga ke
  `host_permissions`.
- Firewall Windows bisa memblokir port non-loopback; izinkan bila diminta.

**Lirik tidak ditemukan**
- Banyak video live/remix/cover tidak ada di LRCLIB.
- Tidak perlu melakukan apa-apa: ytlrc mencoba ulang otomatis dengan query
  berbeda. Status di kanan atas menunjukkan hitungan mundur `COBA LAGI 8s`.
- Tekan `r` untuk mempercepat percobaan berikutnya, atau `m` lalu tulis
  `Artis - Judul` (bisa juga judul saja), Enter.
- Cek judul yang ditebak di header; kalau salah, koreksi lewat `m`.

**Lirik geser / telat**
- Gunakan `+` / `-`. Nilai offset terlihat di footer.
- Video dengan intro panjang membuat durasi beda dari LRCLIB; pencarian
  fallback tetap dijalankan.

**Karakter aneh / gauge rusak**
- Pakai Windows Terminal, atau `chcp 65001` di cmd.exe lama.

**Port 8765 dipakai**
- Jalankan `ytlrc --port 9000`, lalu sesuaikan extension (lihat di atas).

**Build gagal: `linker cc not found`**
- Ikuti langkah Zig di atas, dan set
  `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc-zig`.
- Atau install `build-essential` bila punya sudo, hapus `.cargo/config.toml`,
  lalu build biasa.

## Struktur

```
ytlrc/
├─ .cargo/config.toml      # target musl + ld.lld
├─ Cargo.toml
├─ src/
│  ├─ main.rs              # argumen, server + TUI/print
│  ├─ model.rs             # state playback + ekstrapolasi posisi
│  ├─ theme.rs             # 5 tema warna
│  ├─ config.rs            # simpan tema di ~/.config/ytlrc/config.json
│  ├─ server.rs            # axum: POST /state, GET /state, GET /health
│  ├─ ui.rs                # TUI ratatui, auto-retry, mode --print
│  └─ lyrics/
│     ├─ mod.rs            # orkestrasi cache + LRCLIB
│     ├─ lrclib.rs         # HTTP via curl + pemilihan hasil terbaik
│     ├─ lrc.rs            # parser LRC
│     ├─ title.rs          # parser judul YouTube
│     └─ cache.rs          # cache JSON per video
├─ extension/
│  ├─ manifest.json
│  ├─ content.js
│  └─ background.js
└─ ytlrc.cmd               # launcher dari CMD
```

Tes unit:

```bash
cargo test
```

## Batasan

- Khusus YouTube / YouTube Music (bukan Spotify, radio, atau file lokal).
- Bergantung pada LRCLIB; tidak semua lagu punya lirik tersinkron.
- Parser judul berbasis heuristic; gunakan `m` bila tebakan meleset.
- Butuh browser Chromium (Manifest V3).

## Kredit

- Lirik: [LRCLIB](https://lrclib.net) — gratis, tanpa API key.
- UI: [ratatui](https://ratatui.rs), server: [axum](https://github.com/tokio-rs/axum).
