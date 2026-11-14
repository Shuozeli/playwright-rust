#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
manifest="$repo_dir/examples/v2-locator-composition-only/Cargo.toml"
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
  playwright-rust-v2-locator-composition-only-example
  playwright-rust-backend-cdp-common
  playwright-rust-backend-cdp-input
  playwright-rust-backend-cdp-locator
  playwright-rust-backend-cdp-selector-runtime
  playwright-rust-cap-locator
  playwright-rust-cdp
  playwright-rust-codec-binary
  playwright-rust-codec-common
  playwright-rust-codec-locator
  playwright-rust-composition-chromium-locator
  playwright-rust-connection-control
  playwright-rust-core
  playwright-rust-input-model
  playwright-rust-selector-model
  playwright-rust-server-locator
  playwright-rust-server-wire
  playwright-rust-transport
)

for package in "${required_packages[@]}"; do
  if ! grep -Fxq "$package" "$tree_file"; then
    echo "Locator-composition dependency proof is missing required package: $package" >&2
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
    echo "Locator-composition dependency proof found forbidden Playwright Rust package: $package" >&2
    exit 1
  fi
done <"$tree_file"

echo "Locator-composition dependency proof passed (${#required_packages[@]} Playwright Rust packages)"
