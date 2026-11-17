#!/usr/bin/env bash
# Build helper untuk lingkungan tanpa gcc/build-essential.
# - Target: static musl (binary bisa dipindah ke mana saja)
# - Linker: rust-lld bawaan toolchain
# - Build script host: Zig (`zig cc`) bila `cc` tidak tersedia
set -euo pipefail

cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

TARGET="x86_64-unknown-linux-musl"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo tidak ditemukan. Install rustup dulu:" >&2
  echo '  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal' >&2
  exit 1
fi

if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  echo "Menambah target $TARGET ..."
  rustup target add "$TARGET"
fi

# rust-lld dipakai sebagai linker statis (tanpa gcc).
if ! command -v ld.lld >/dev/null 2>&1; then
  SYSROOT="$(rustc --print sysroot)"
  LLD="$SYSROOT/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld"
  if [ ! -x "$LLD" ]; then
    echo "rust-lld tidak ditemukan di $LLD" >&2
    exit 1
  fi
  mkdir -p "$HOME/.local/bin"
  ln -sf "$LLD" "$HOME/.local/bin/ld.lld"
fi

# Build script Rust tetap butuh linker host (glibc). Kalau `cc` tidak ada,
# pakai Zig portable sebagai `cc-zig`.
if ! command -v cc >/dev/null 2>&1; then
  if ! command -v zig >/dev/null 2>&1; then
    echo "Tidak ada 'cc' maupun 'zig'." >&2
    echo "Install build-essential (sudo) atau Zig portable: https://ziglang.org/download/" >&2
    exit 1
  fi
  if ! command -v cc-zig >/dev/null 2>&1; then
    printf '#!/bin/sh\nexec "%s" cc "$@"\n' "$(command -v zig)" > "$HOME/.local/bin/cc-zig"
    chmod +x "$HOME/.local/bin/cc-zig"
  fi
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc-zig
fi

echo "Build release ($TARGET) ..."
cargo build --release "$@"

BIN="target/$TARGET/release/ytlrc"
if [ -x "$BIN" ]; then
  cp -f "$BIN" ./ytlrc
  echo
  echo "Selesai:"
  echo "  $BIN"
  echo "  ./ytlrc  (salinan)"
  ls -lh "$BIN" | awk '{print "  ukuran: " $5}'
fi
