#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-composition-chromium-locator-evaluation
actual=$(cargo tree --locked -p playwright-rust-composition-chromium-locator-evaluation --edges normal --prefix none --format '{p}' | awk '/^playwright-rust-/ {print $1}' | sort -u)
expected=$(printf '%s\n' \
  playwright-rust-backend-cdp-common playwright-rust-backend-cdp-evaluation \
  playwright-rust-backend-cdp-handle-evaluation playwright-rust-backend-cdp-locator-resolution \
  playwright-rust-backend-cdp-selector-runtime playwright-rust-browser-backend playwright-rust-browser-model \
  playwright-rust-cap-evaluation playwright-rust-cap-handle-evaluation playwright-rust-cdp \
  playwright-rust-composition-chromium-locator-evaluation playwright-rust-composition-chromium-locator-resolution \
  playwright-rust-core playwright-rust-runtime playwright-rust-selector-model | sort -u)
if [[ "$actual" != "$expected" ]]; then
  printf 'Unexpected Locator element-evaluation dependency closure:\n%s\nExpected:\n%s\n' "$actual" "$expected" >&2
  exit 1
fi
printf 'Locator element-evaluation dependency proof passed (15 Playwright Rust packages)\n'
