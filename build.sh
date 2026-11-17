#!/usr/bin/env bash
# Cross-platform build helper.
#
#   Linux : static musl binary (rust-lld; Zig is used when there is no cc)
#   macOS : native release build (clang from Xcode Command Line Tools)
#   other : native release build
#
# Usage: ./build.sh [extra cargo args]
set -euo pipefail

cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found. Install Rust first: https://rustup.rs" >&2
  exit 1
fi

OS="$(uname -s)"
ARCH="$(uname -m)"
BUILD_ARGS=(--release)

if [ "$OS" = "Linux" ]; then
  case "$ARCH" in
    aarch64|arm64) TARGET="aarch64-unknown-linux-musl" ;;
    *)             TARGET="x86_64-unknown-linux-musl" ;;
  esac

  if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
    echo "Adding target $TARGET ..."
    rustup target add "$TARGET"
  fi

  # rust-lld is used as the static linker (no gcc required).
  if ! command -v ld.lld >/dev/null 2>&1; then
    HOST="$(rustc -vV | sed -n 's/^host: //p')"
    LLD="$(rustc --print sysroot)/lib/rustlib/$HOST/bin/rust-lld"
    if [ ! -x "$LLD" ]; then
      echo "rust-lld not found at $LLD" >&2
      exit 1
    fi
    mkdir -p "$HOME/.local/bin"
    ln -sf "$LLD" "$HOME/.local/bin/ld.lld"
  fi

  # Rust build scripts still need a host linker (glibc). If `cc` is missing,
  # fall back to the portable Zig toolchain as `cc-zig`.
  if ! command -v cc >/dev/null 2>&1; then
    if ! command -v zig >/dev/null 2>&1; then
      echo "Neither 'cc' nor 'zig' was found." >&2
      echo "Install build-essential (sudo) or Zig: https://ziglang.org/download/" >&2
      exit 1
    fi
    if ! command -v cc-zig >/dev/null 2>&1; then
      printf '#!/bin/sh\nexec "%s" cc "$@"\n' "$(command -v zig)" > "$HOME/.local/bin/cc-zig"
      chmod +x "$HOME/.local/bin/cc-zig"
    fi
    HOST="$(rustc -vV | sed -n 's/^host: //p' | tr '[:lower:]-' '[:upper:]_')"
    export "CARGO_TARGET_${HOST}_LINKER=cc-zig"
  fi

  BUILD_ARGS+=(--target "$TARGET")
  BIN="target/$TARGET/release/ytlrc"
else
  echo "Native build for $OS/$ARCH ..."
  BIN="target/release/ytlrc"
fi

echo "Running: cargo build ${BUILD_ARGS[*]} $*"
cargo build "${BUILD_ARGS[@]}" "$@"

if [ -x "$BIN" ]; then
  cp -f "$BIN" ./ytlrc
  echo
  echo "Done:"
  echo "  $BIN"
  echo "  ./ytlrc  (copy)"
  ls -lh "$BIN" | awk '{print "  size: " $5}'
fi
