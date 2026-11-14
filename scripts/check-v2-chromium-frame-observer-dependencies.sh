#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

actual=$(cargo tree --locked -p playwright-rust-plugin-chromium-frame-observer \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
expected=$(printf '%s\n' \
  playwright-rust-backend-cdp-topology \
  playwright-rust-browser-model \
  playwright-rust-cdp \
  playwright-rust-core \
  playwright-rust-plugin-chromium-frame-observer \
  | sort -u)

if [[ "$actual" != "$expected" ]]; then
  printf 'Unexpected Chromium frame-observer dependency closure:\n%s\nExpected:\n%s\n' \
    "$actual" "$expected" >&2
  exit 1
fi

aggregate_files=$(rg -l 'playwright_rust_composition_chromium' \
  crates/playwright-rust-node-host/src --glob '*.rs' | wc -l | tr -d ' ')
if (( aggregate_files > 39 )); then
  echo "Node host aggregate coupling grew to $aggregate_files source files (maximum 39)" >&2
  exit 1
fi

for host_observer in \
  crates/playwright-rust-node-host/src/extra_headers/session_retirement.rs \
  crates/playwright-rust-node-host/src/frame_metadata.rs \
  crates/playwright-rust-node-host/src/frame_trees.rs
do
  if rg -q 'playwright_rust_composition_chromium' "$host_observer"; then
    echo "Extracted frame observer regressed to the aggregate in $host_observer" >&2
    exit 1
  fi
  rg -q 'playwright_rust_plugin_chromium_frame_observer' "$host_observer"
done
rg -q 'playwright_rust_plugin_chromium_frame_observer::ChromiumFrameEventObserver' \
  crates/playwright-rust-node-host/src/lib.rs
rg -q 'ChromiumFrameEventObserver, FrameObservationResult' \
  crates/playwright-rust-composition-chromium/src/frame_event_observer.rs
rg -q 'ChromiumFrameUrlObserver' \
  crates/playwright-rust-composition-chromium/src/topology_events.rs

CARGO_INCREMENTAL=0 cargo check --release --locked \
  -p playwright-rust-plugin-chromium-frame-observer

echo "Chromium frame-observer dependency proof passed (5 Playwright Rust packages; $aggregate_files aggregate-coupled host source files)"
