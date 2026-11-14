#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

actual=$(cargo tree --locked -p playwright-rust-plugin-chromium-frame-lifecycle \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
expected=$(printf '%s\n' \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-navigation \
  playwright-rust-backend-cdp-topology \
  playwright-rust-browser-backend \
  playwright-rust-browser-model \
  playwright-rust-cap-navigation \
  playwright-rust-cdp \
  playwright-rust-core \
  playwright-rust-plugin-chromium-frame-lifecycle \
  playwright-rust-plugin-chromium-frame-observer \
  | sort -u)

if [[ "$actual" != "$expected" ]]; then
  printf 'Unexpected Chromium frame-lifecycle dependency closure:\n%s\nExpected:\n%s\n' \
    "$actual" "$expected" >&2
  exit 1
fi

aggregate_files=$(rg -l 'playwright_rust_composition_chromium' \
  crates/playwright-rust-node-host/src --glob '*.rs' | wc -l | tr -d ' ')
if (( aggregate_files > 39 )); then
  echo "Node host aggregate coupling grew to $aggregate_files source files (maximum 39)" >&2
  exit 1
fi

host_lifecycle=crates/playwright-rust-node-host/src/frame_lifecycle.rs
if rg -q 'playwright_rust_composition_chromium' "$host_lifecycle"; then
  echo "Extracted Frame lifecycle regressed to the aggregate in $host_lifecycle" >&2
  exit 1
fi
rg -q 'playwright_rust_plugin_chromium_frame_lifecycle' "$host_lifecycle"
rg -q 'pub use playwright_rust_plugin_chromium_frame_lifecycle::\*' \
  crates/playwright-rust-composition-chromium/src/lib.rs

CARGO_INCREMENTAL=0 cargo check --release --locked \
  -p playwright-rust-plugin-chromium-frame-lifecycle

echo "Chromium frame-lifecycle dependency proof passed (10 Playwright Rust packages; $aggregate_files aggregate-coupled host source files)"
