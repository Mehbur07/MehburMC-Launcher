#!/bin/sh
# Runs inside the builder container; the working tree arrives as a tar on stdin.
#   build.sh test     Rust checks and tests only
#   build.sh package  signed .deb into /out
set -eu

find /work/src -mindepth 1 -maxdepth 1 ! -name node_modules -exec rm -rf {} +
tar -xf - -C /work/src
cd /work/src

case "$1" in
  test)
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    ;;
  package)
    npm ci --no-audit --no-fund
    rm -rf "$CARGO_TARGET_DIR/release/bundle/deb"
    npx tauri build
    cp "$CARGO_TARGET_DIR"/release/bundle/deb/*.deb "$CARGO_TARGET_DIR"/release/bundle/deb/*.deb.sig /out/
    ls -la /out
    ;;
  *)
    echo "usage: build.sh test|package" >&2
    exit 2
    ;;
esac
