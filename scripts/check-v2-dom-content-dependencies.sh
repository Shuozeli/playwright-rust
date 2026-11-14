#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
manifest="$repo_dir/examples/v2-dom-content-only/Cargo.toml"
tree_file=$(mktemp)
trap 'rm -f "$tree_file"' EXIT

CARGO_INCREMENTAL=0 cargo check --release --locked --manifest-path "$manifest"

cargo tree \
  --manifest-path "$manifest" \
  --locked \
  --edges normal \
  --prefix none \
  --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u >"$tree_file"

required_packages=(
  playwright-rust-v2-dom-content-only-example
  playwright-rust-composition-chromium-dom-content
  playwright-rust-backend-cdp-common
  playwright-rust-backend-cdp-dom-content
  playwright-rust-browser-backend
  playwright-rust-cap-dom-content
  playwright-rust-cdp
  playwright-rust-codec-binary
  playwright-rust-codec-common
  playwright-rust-codec-dom-content
  playwright-rust-connection-control
  playwright-rust-core
  playwright-rust-server-dom-content
  playwright-rust-server-wire
  playwright-rust-transport
)

for package in "${required_packages[@]}"; do
  if ! grep -Fxq "$package" "$tree_file"; then
    echo "DOM-content-only dependency proof is missing required package: $package" >&2
    exit 1
  fi
done

while IFS= read -r package; do
  allowed=false
  for required in "${required_packages[@]}"; do
    if [[ "$package" == "$required" ]]; then
      allowed=true
      break
    fi
  done
  if [[ "$allowed" != true ]]; then
    echo "DOM-content-only dependency proof found forbidden Playwright Rust package: $package" >&2
    exit 1
  fi
done <"$tree_file"

echo "DOM-content-only dependency proof passed (${#required_packages[@]} Playwright Rust packages)"
