#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

check_closure() {
  local package=$1
  local feature=$2
  shift 2
  local selection=(--no-default-features)
  if [[ "$feature" != none ]]; then
    selection+=(--features "$feature")
  fi
  CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo check --release --locked -p "$package" "${selection[@]}"
  local actual expected full baseline baseline_features
  full=$(cargo tree --locked -p "$package" "${selection[@]}" --edges normal --prefix none --format '{p}' | sort -u)
  actual=$(printf '%s\n' "$full" | awk '/^playwright-rust-/ {print $1}' | sort -u)
  expected=$(printf '%s\n' "$@" | sort -u)
  if [[ "$actual" != "$expected" ]]; then
    printf 'Unexpected normal dependency closure for %s:\n%s\nExpected:\n%s\n' "$package" "$actual" "$expected" >&2
    return 1
  fi
  baseline=$(cargo tree --locked -p "$package" --no-default-features --edges normal --prefix none --format '{p}' | sort -u)
  if [[ "$full" != "$baseline" ]]; then
    printf 'Feature %s added normal dependencies to %s\n' "$feature" "$package" >&2
    return 1
  fi
  baseline_features=$(cargo tree --locked -p "$package" --no-default-features --edges normal --prefix none --format '{p} [{f}]')
  if [[ "$baseline_features" == *retained-reference* ]]; then
    printf 'Unselected %s implicitly enables retained-reference\n' "$package" >&2
    return 1
  fi
  printf '%s feature=%s dependency proof passed (%s Playwright Rust packages, identical complete closure)\n' "$package" "$feature" "$#"
}

shared=(playwright-rust-backend-cdp-locator-resolution playwright-rust-backend-cdp-selector-runtime playwright-rust-cdp playwright-rust-core playwright-rust-selector-model)
for feature in none retained-reference; do
  check_closure playwright-rust-backend-cdp-locator-resolution "$feature" "${shared[@]}"
  check_closure playwright-rust-composition-chromium-locator-resolution "$feature" "${shared[@]}" playwright-rust-composition-chromium-locator-resolution playwright-rust-runtime
done
