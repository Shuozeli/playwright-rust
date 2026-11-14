#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
manifest="$repo_dir/examples/v2-umbrella-profiles/Cargo.toml"
temporary_files=("")

cleanup() {
  for temporary_file in "${temporary_files[@]}"; do
    if [[ -n "$temporary_file" ]]; then
      rm -f "$temporary_file"
    fi
  done
}
trap cleanup EXIT

profile_packages=(
  playwright-rust-v2-umbrella-empty
  playwright-rust-v2-umbrella-client-content
  playwright-rust-v2-umbrella-client-context-lifecycle
  playwright-rust-v2-umbrella-client-page-lifecycle
  playwright-rust-v2-umbrella-client-page-creation
  playwright-rust-v2-umbrella-client-page-events
  playwright-rust-v2-umbrella-client-network-events
  playwright-rust-v2-umbrella-client-handle-lifecycle
  playwright-rust-v2-umbrella-client-handle-evaluation
  playwright-rust-v2-umbrella-client-context-creation
  playwright-rust-v2-umbrella-ordered-handle-delivery
  playwright-rust-v2-umbrella-client-locator
  playwright-rust-v2-umbrella-client-locator-screenshot
  playwright-rust-v2-umbrella-client-dom-content
  playwright-rust-v2-umbrella-client-evaluation
  playwright-rust-v2-umbrella-client-navigation
  playwright-rust-v2-umbrella-chromium-content
  playwright-rust-v2-umbrella-chromium-context-lifecycle
  playwright-rust-v2-umbrella-chromium-page-lifecycle
  playwright-rust-v2-umbrella-chromium-page-creation
  playwright-rust-v2-umbrella-chromium-page-events
  playwright-rust-v2-umbrella-chromium-network-events
  playwright-rust-v2-umbrella-chromium-handle-lifecycle
  playwright-rust-v2-umbrella-chromium-handle-evaluation
  playwright-rust-v2-umbrella-chromium-context-creation
  playwright-rust-v2-umbrella-chromium-locator
  playwright-rust-v2-umbrella-chromium-locator-screenshot
  playwright-rust-v2-umbrella-chromium-dom-content
  playwright-rust-v2-umbrella-chromium-evaluation
  playwright-rust-v2-umbrella-chromium-navigation
)

for package in "${profile_packages[@]}"; do
  CARGO_INCREMENTAL=0 cargo check --release --locked \
    --manifest-path "$manifest" \
    --package "$package"
done

cargo package --package playwright-rust --allow-dirty --list >/dev/null

check_profile() {
  local package=$1
  shift
  local tree_file
  local expected_file
  tree_file=$(mktemp)
  expected_file=$(mktemp)
  temporary_files+=("$tree_file" "$expected_file")

  cargo tree \
    --manifest-path "$manifest" \
    --package "$package" \
    --locked \
    --edges normal \
    --prefix none \
    --format '{p}' \
    | awk '$1 == "playwright-rust" || $1 ~ /^playwright-rust-/ { print $1 }' \
    | sort -u >"$tree_file"
  printf '%s\n' "$@" | sort -u >"$expected_file"

  if ! diff -u "$expected_file" "$tree_file"; then
    echo "umbrella dependency proof failed for $package" >&2
    exit 1
  fi
}

check_profile \
  playwright-rust-v2-umbrella-empty \
  playwright-rust \
  playwright-rust-v2-umbrella-empty

check_profile \
  playwright-rust-v2-umbrella-client-content \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-content \
  playwright-rust-client \
  playwright-rust-client-content \
  playwright-rust-core \
  playwright-rust-screenshot-model \
  playwright-rust-v2-umbrella-client-content

check_profile \
  playwright-rust-v2-umbrella-client-context-lifecycle \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-context-lifecycle \
  playwright-rust-client \
  playwright-rust-client-context-lifecycle \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-context-lifecycle

check_profile \
  playwright-rust-v2-umbrella-client-page-lifecycle \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-page-lifecycle \
  playwright-rust-client \
  playwright-rust-client-page-lifecycle \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-page-lifecycle

check_profile \
  playwright-rust-v2-umbrella-client-page-creation \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-page-creation \
  playwright-rust-cap-page-lifecycle \
  playwright-rust-client \
  playwright-rust-client-page-creation \
  playwright-rust-client-page-lifecycle \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-page-creation

check_profile \
  playwright-rust-v2-umbrella-client-page-events \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-page-events \
  playwright-rust-client \
  playwright-rust-client-events \
  playwright-rust-client-page-events \
  playwright-rust-client-wire \
  playwright-rust-codec-page-events \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-client-page-events

check_profile \
  playwright-rust-v2-umbrella-client-network-events \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-network-events \
  playwright-rust-client \
  playwright-rust-client-events \
  playwright-rust-client-network-events \
  playwright-rust-client-wire \
  playwright-rust-codec-binary \
  playwright-rust-codec-network-events \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-network-model \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-client-network-events

check_profile \
  playwright-rust-v2-umbrella-client-handle-lifecycle \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-handle-lifecycle \
  playwright-rust-client \
  playwright-rust-client-handle-lifecycle \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-handle-lifecycle

check_profile \
  playwright-rust-v2-umbrella-client-handle-evaluation \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-evaluation \
  playwright-rust-cap-handle-evaluation \
  playwright-rust-cap-handle-lifecycle \
  playwright-rust-client \
  playwright-rust-client-handle-evaluation \
  playwright-rust-client-handle-lifecycle \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-handle-evaluation

check_profile \
  playwright-rust-v2-umbrella-client-context-creation \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-context-creation \
  playwright-rust-cap-context-lifecycle \
  playwright-rust-client \
  playwright-rust-client-context-creation \
  playwright-rust-client-context-lifecycle \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-context-creation

check_profile \
  playwright-rust-v2-umbrella-client-locator \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-locator \
  playwright-rust-client \
  playwright-rust-client-locator \
  playwright-rust-core \
  playwright-rust-input-model \
  playwright-rust-selector-model \
  playwright-rust-v2-umbrella-client-locator

check_profile \
  playwright-rust-v2-umbrella-client-locator-screenshot \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-locator-screenshot \
  playwright-rust-client \
  playwright-rust-client-locator-screenshot \
  playwright-rust-core \
  playwright-rust-screenshot-model \
  playwright-rust-selector-model \
  playwright-rust-v2-umbrella-client-locator-screenshot

check_profile \
  playwright-rust-v2-umbrella-client-dom-content \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-dom-content \
  playwright-rust-client \
  playwright-rust-client-dom-content \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-dom-content

check_profile \
  playwright-rust-v2-umbrella-client-evaluation \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-evaluation \
  playwright-rust-client \
  playwright-rust-client-evaluation \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-evaluation

check_profile \
  playwright-rust-v2-umbrella-client-navigation \
  playwright-rust \
  playwright-rust-browser-model \
  playwright-rust-cap-navigation \
  playwright-rust-client \
  playwright-rust-client-navigation \
  playwright-rust-core \
  playwright-rust-v2-umbrella-client-navigation

check_profile \
  playwright-rust-v2-umbrella-chromium-dom-content \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-dom-content \
  playwright-rust-browser-backend \
  playwright-rust-cap-dom-content \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-dom-content \
  playwright-rust-composition-chromium-dom-content \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-server-dom-content \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-dom-content

check_profile \
  playwright-rust-v2-umbrella-chromium-content \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-content \
  playwright-rust-browser-backend \
  playwright-rust-cap-content \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-content \
  playwright-rust-codec-screenshot \
  playwright-rust-composition-chromium-content \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-screenshot-model \
  playwright-rust-server-content \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-content

check_profile \
  playwright-rust-v2-umbrella-chromium-context-lifecycle \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-context-lifecycle \
  playwright-rust-backend-cdp-topology \
  playwright-rust-browser-model \
  playwright-rust-cap-context-lifecycle \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-context-lifecycle \
  playwright-rust-composition-chromium-context-lifecycle \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-runtime \
  playwright-rust-server-context-lifecycle \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-context-lifecycle

check_profile \
  playwright-rust-v2-umbrella-chromium-page-lifecycle \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-page-lifecycle \
  playwright-rust-backend-cdp-topology \
  playwright-rust-browser-model \
  playwright-rust-cap-page-lifecycle \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-page-lifecycle \
  playwright-rust-composition-chromium-page-lifecycle \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-runtime \
  playwright-rust-server-page-lifecycle \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-page-lifecycle

check_profile \
  playwright-rust-v2-umbrella-chromium-page-creation \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-page-creation \
  playwright-rust-backend-cdp-page-lifecycle \
  playwright-rust-backend-cdp-topology \
  playwright-rust-browser-model \
  playwright-rust-cap-page-creation \
  playwright-rust-cap-page-lifecycle \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-page-creation \
  playwright-rust-codec-page-lifecycle \
  playwright-rust-composition-chromium-page-creation \
  playwright-rust-composition-chromium-page-lifecycle \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-runtime \
  playwright-rust-server-page-creation \
  playwright-rust-server-page-lifecycle \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-page-creation

check_profile \
  playwright-rust-v2-umbrella-chromium-page-events \
  playwright-rust \
  playwright-rust-backend-cdp-page-events \
  playwright-rust-backend-cdp-topology \
  playwright-rust-browser-model \
  playwright-rust-cap-page-events \
  playwright-rust-cdp \
  playwright-rust-codec-page-events \
  playwright-rust-composition-chromium-page-events \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-runtime \
  playwright-rust-runtime-control \
  playwright-rust-server-events \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-page-events

check_profile \
  playwright-rust-v2-umbrella-chromium-network-events \
  playwright-rust \
  playwright-rust-backend-cdp-network-events \
  playwright-rust-browser-model \
  playwright-rust-cap-network-events \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-network-events \
  playwright-rust-composition-chromium-network-events \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-network-model \
  playwright-rust-runtime \
  playwright-rust-runtime-control \
  playwright-rust-server-events \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-network-events

check_profile \
  playwright-rust-v2-umbrella-chromium-handle-lifecycle \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-handle-lifecycle \
  playwright-rust-cap-handle-lifecycle \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-handle-lifecycle \
  playwright-rust-composition-chromium-handle-lifecycle \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-runtime \
  playwright-rust-server-handle-lifecycle \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-handle-lifecycle

check_profile \
  playwright-rust-v2-umbrella-chromium-handle-evaluation \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-evaluation \
  playwright-rust-backend-cdp-handle-evaluation \
  playwright-rust-backend-cdp-handle-lifecycle \
  playwright-rust-browser-backend \
  playwright-rust-browser-model \
  playwright-rust-cap-evaluation \
  playwright-rust-cap-handle-evaluation \
  playwright-rust-cap-handle-lifecycle \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-handle-evaluation \
  playwright-rust-codec-handle-lifecycle \
  playwright-rust-composition-chromium-handle-evaluation \
  playwright-rust-composition-chromium-handle-lifecycle \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-runtime \
  playwright-rust-server-handle-evaluation \
  playwright-rust-server-handle-lifecycle \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-handle-evaluation

check_profile \
  playwright-rust-v2-umbrella-chromium-context-creation \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-context-creation \
  playwright-rust-backend-cdp-context-lifecycle \
  playwright-rust-backend-cdp-topology \
  playwright-rust-browser-model \
  playwright-rust-cap-context-creation \
  playwright-rust-cap-context-lifecycle \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-context-creation \
  playwright-rust-codec-context-lifecycle \
  playwright-rust-composition-chromium-context-creation \
  playwright-rust-composition-chromium-context-lifecycle \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-runtime \
  playwright-rust-server-context-creation \
  playwright-rust-server-context-lifecycle \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-context-creation

check_profile \
  playwright-rust-v2-umbrella-chromium-locator \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-input \
  playwright-rust-backend-cdp-locator \
  playwright-rust-backend-cdp-selector-runtime \
  playwright-rust-cap-locator \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-locator \
  playwright-rust-composition-chromium-locator \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-input-model \
  playwright-rust-selector-model \
  playwright-rust-server-locator \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-locator

check_profile \
  playwright-rust-v2-umbrella-chromium-locator-screenshot \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-content \
  playwright-rust-backend-cdp-locator-screenshot \
  playwright-rust-backend-cdp-selector-geometry \
  playwright-rust-backend-cdp-selector-runtime \
  playwright-rust-browser-backend \
  playwright-rust-cap-content \
  playwright-rust-cap-locator-screenshot \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-locator-screenshot \
  playwright-rust-codec-screenshot \
  playwright-rust-codec-selector \
  playwright-rust-composition-chromium-locator-screenshot \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-screenshot-model \
  playwright-rust-selector-model \
  playwright-rust-server-locator-screenshot \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-locator-screenshot

check_profile \
  playwright-rust-v2-umbrella-chromium-evaluation \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-evaluation \
  playwright-rust-browser-backend \
  playwright-rust-cap-evaluation \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-evaluation \
  playwright-rust-composition-chromium-evaluation \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-server-evaluation \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-evaluation

check_profile \
  playwright-rust-v2-umbrella-ordered-handle-delivery \
  playwright-rust \
  playwright-rust-cap-evaluation \
  playwright-rust-cap-handle-evaluation \
  playwright-rust-cap-handle-lifecycle \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-handle-evaluation \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-plugin-ordered-handle-delivery \
  playwright-rust-runtime \
  playwright-rust-runtime-control \
  playwright-rust-server-handle-evaluation \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-ordered-handle-delivery

check_profile \
  playwright-rust-v2-umbrella-chromium-navigation \
  playwright-rust \
  playwright-rust-backend-cdp-common \
  playwright-rust-backend-cdp-navigation \
  playwright-rust-browser-backend \
  playwright-rust-cap-navigation \
  playwright-rust-cdp \
  playwright-rust-codec-binary \
  playwright-rust-codec-common \
  playwright-rust-codec-navigation \
  playwright-rust-composition-chromium-navigation \
  playwright-rust-connection-control \
  playwright-rust-core \
  playwright-rust-server-navigation \
  playwright-rust-server-wire \
  playwright-rust-transport \
  playwright-rust-v2-umbrella-chromium-navigation

empty_features=$(mktemp)
client_content_features=$(mktemp)
client_context_lifecycle_features=$(mktemp)
client_page_lifecycle_features=$(mktemp)
client_page_creation_features=$(mktemp)
client_page_events_features=$(mktemp)
client_network_events_features=$(mktemp)
client_handle_lifecycle_features=$(mktemp)
client_handle_evaluation_features=$(mktemp)
client_context_creation_features=$(mktemp)
ordered_handle_delivery_features=$(mktemp)
client_locator_features=$(mktemp)
client_locator_screenshot_features=$(mktemp)
client_dom_content_features=$(mktemp)
client_evaluation_features=$(mktemp)
client_features=$(mktemp)
chromium_content_features=$(mktemp)
chromium_context_lifecycle_features=$(mktemp)
chromium_page_lifecycle_features=$(mktemp)
chromium_page_creation_features=$(mktemp)
chromium_page_events_features=$(mktemp)
chromium_network_events_features=$(mktemp)
chromium_handle_lifecycle_features=$(mktemp)
chromium_handle_evaluation_features=$(mktemp)
chromium_context_creation_features=$(mktemp)
chromium_locator_features=$(mktemp)
chromium_locator_screenshot_features=$(mktemp)
chromium_dom_content_features=$(mktemp)
chromium_evaluation_features=$(mktemp)
chromium_features=$(mktemp)
temporary_files+=(
  "$empty_features"
  "$client_content_features"
  "$client_context_lifecycle_features"
  "$client_page_lifecycle_features"
  "$client_page_creation_features"
  "$client_page_events_features"
  "$client_network_events_features"
  "$client_handle_lifecycle_features"
  "$client_handle_evaluation_features"
  "$client_context_creation_features"
  "$ordered_handle_delivery_features"
  "$client_locator_features"
  "$client_locator_screenshot_features"
  "$client_dom_content_features"
  "$client_evaluation_features"
  "$client_features"
  "$chromium_content_features"
  "$chromium_context_lifecycle_features"
  "$chromium_page_lifecycle_features"
  "$chromium_page_creation_features"
  "$chromium_page_events_features"
  "$chromium_network_events_features"
  "$chromium_handle_lifecycle_features"
  "$chromium_handle_evaluation_features"
  "$chromium_context_creation_features"
  "$chromium_locator_features"
  "$chromium_locator_screenshot_features"
  "$chromium_dom_content_features"
  "$chromium_evaluation_features"
  "$chromium_features"
)

cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-empty \
  --locked --edges features --prefix none >"$empty_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-content \
  --locked --edges features --prefix none >"$client_content_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-context-lifecycle \
  --locked --edges features --prefix none >"$client_context_lifecycle_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-page-lifecycle \
  --locked --edges features --prefix none >"$client_page_lifecycle_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-page-creation \
  --locked --edges features --prefix none >"$client_page_creation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-page-events \
  --locked --edges features --prefix none >"$client_page_events_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-network-events \
  --locked --edges features --prefix none >"$client_network_events_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-handle-lifecycle \
  --locked --edges features --prefix none >"$client_handle_lifecycle_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-handle-evaluation \
  --locked --edges features --prefix none >"$client_handle_evaluation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-context-creation \
  --locked --edges features --prefix none >"$client_context_creation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-ordered-handle-delivery \
  --locked --edges features --prefix none >"$ordered_handle_delivery_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-locator \
  --locked --edges features --prefix none >"$client_locator_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-locator-screenshot \
  --locked --edges features --prefix none >"$client_locator_screenshot_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-dom-content \
  --locked --edges features --prefix none >"$client_dom_content_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-evaluation \
  --locked --edges features --prefix none >"$client_evaluation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-client-navigation \
  --locked --edges features --prefix none >"$client_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-content \
  --locked --edges features --prefix none >"$chromium_content_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-context-lifecycle \
  --locked --edges features --prefix none >"$chromium_context_lifecycle_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-page-lifecycle \
  --locked --edges features --prefix none >"$chromium_page_lifecycle_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-page-creation \
  --locked --edges features --prefix none >"$chromium_page_creation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-page-events \
  --locked --edges features --prefix none >"$chromium_page_events_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-network-events \
  --locked --edges features --prefix none >"$chromium_network_events_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-handle-lifecycle \
  --locked --edges features --prefix none >"$chromium_handle_lifecycle_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-handle-evaluation \
  --locked --edges features --prefix none >"$chromium_handle_evaluation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-context-creation \
  --locked --edges features --prefix none >"$chromium_context_creation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-locator \
  --locked --edges features --prefix none >"$chromium_locator_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-locator-screenshot \
  --locked --edges features --prefix none >"$chromium_locator_screenshot_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-dom-content \
  --locked --edges features --prefix none >"$chromium_dom_content_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-evaluation \
  --locked --edges features --prefix none >"$chromium_evaluation_features"
cargo tree --manifest-path "$manifest" --package playwright-rust-v2-umbrella-chromium-navigation \
  --locked --edges features --prefix none >"$chromium_features"

if grep -Eq '^playwright-rust feature ' "$empty_features"; then
  echo "empty umbrella profile unexpectedly selected a playwright-rust feature" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-content"' "$client_content_features"; then
  echo "client umbrella profile did not select client-content" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-content|chromium-dom-content|chromium-evaluation|chromium-navigation|dom-content|evaluation|navigation)"' "$client_content_features"; then
  echo "client content profile leaked a CDP, server, DOM, evaluation, or navigation feature" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-context-lifecycle"' "$client_context_lifecycle_features"; then
  echo "client umbrella profile did not select client-context-lifecycle" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|dom-content|evaluation|locator|navigation|runtime)"' "$client_context_lifecycle_features"; then
  echo "client Context-lifecycle profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-page-lifecycle"' "$client_page_lifecycle_features"; then
  echo "client umbrella profile did not select client-page-lifecycle" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|context-|dom-content|evaluation|locator|navigation|runtime)"' "$client_page_lifecycle_features"; then
  echo "client Page-lifecycle profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-page-creation"' "$client_page_creation_features"; then
  echo "client umbrella profile did not select client-page-creation" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-page-lifecycle"' "$client_page_creation_features"; then
  echo "client Page-creation profile did not select its required lifecycle client" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|context-|dom-content|evaluation|locator|navigation|runtime)"' "$client_page_creation_features"; then
  echo "client Page-creation profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-page-events"' "$client_page_events_features"; then
  echo "client umbrella profile did not select client-page-events" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|context-|dom-content|evaluation|locator|navigation|runtime)"' "$client_page_events_features"; then
  echo "client Page-events profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-network-events"' "$client_network_events_features"; then
  echo "client umbrella profile did not select client-network-events" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|context-|dom-content|evaluation|handle-|locator|navigation|page-|runtime)"' "$client_network_events_features"; then
  echo "client Network-events profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-handle-lifecycle"' "$client_handle_lifecycle_features"; then
  echo "client umbrella profile did not select client-handle-lifecycle" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|context-|dom-content|evaluation|locator|navigation|page-|runtime)"' "$client_handle_lifecycle_features"; then
  echo "client Handle-lifecycle profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-handle-evaluation"' "$client_handle_evaluation_features"; then
  echo "client umbrella profile did not select client-handle-evaluation" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-handle-lifecycle"' "$client_handle_evaluation_features"; then
  echo "client Handle-evaluation profile did not select its required lifecycle client" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|context-|dom-content|locator|navigation|page-|runtime)"' "$client_handle_evaluation_features"; then
  echo "client Handle-evaluation profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-context-creation"' "$client_context_creation_features"; then
  echo "client umbrella profile did not select client-context-creation" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-context-lifecycle"' "$client_context_creation_features"; then
  echo "client Context-creation profile did not select its required lifecycle client" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|dom-content|evaluation|locator|navigation|runtime)"' "$client_context_creation_features"; then
  echo "client Context-creation profile leaked CDP, server, runtime, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "ordered-handle-delivery"' "$ordered_handle_delivery_features"; then
  echo "Ordered Handle delivery profile did not select its plugin feature" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "handle-evaluation"' "$ordered_handle_delivery_features"; then
  echo "Ordered Handle delivery profile did not select Handle evaluation" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "runtime"' "$ordered_handle_delivery_features"; then
  echo "Ordered Handle delivery profile did not select runtime ownership" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "transport"' "$ordered_handle_delivery_features"; then
  echo "Ordered Handle delivery profile did not select transport ownership" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|client|client-|content|context-|dom-content|locator|navigation|page-)"' "$ordered_handle_delivery_features"; then
  echo "Ordered Handle delivery profile leaked a browser backend, client, or unrelated capability feature" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-locator"' "$client_locator_features"; then
  echo "client umbrella profile did not select client-locator" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|dom-content|evaluation|locator-screenshot|navigation)"' "$client_locator_features"; then
  echo "client Locator profile leaked CDP, server, or unrelated capability features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-locator-screenshot"' "$client_locator_screenshot_features"; then
  echo "client umbrella profile did not select client-locator-screenshot" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-|content|dom-content|evaluation|navigation)"' "$client_locator_screenshot_features"; then
  echo "client Locator screenshot profile leaked CDP, server, Page screenshot, DOM, evaluation, or navigation features" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-dom-content"' "$client_dom_content_features"; then
  echo "client umbrella profile did not select client-dom-content" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-content|chromium-dom-content|chromium-evaluation|chromium-navigation|content|evaluation|navigation)"' "$client_dom_content_features"; then
  echo "client DOM-content profile leaked a CDP, server, screenshot, evaluation, or navigation feature" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-evaluation"' "$client_evaluation_features"; then
  echo "client umbrella profile did not select client-evaluation" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-evaluation|chromium-navigation|navigation)"' "$client_evaluation_features"; then
  echo "client evaluation umbrella profile leaked a CDP, server, or navigation feature" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "client-navigation"' "$client_features"; then
  echo "client umbrella profile did not select client-navigation" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(cdp|chromium-navigation)"' "$client_features"; then
  echo "client umbrella profile leaked a CDP/server composition feature" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-content"' "$chromium_content_features"; then
  echo "Chromium umbrella profile did not select chromium-content" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-content|client-dom-content|client-evaluation|client-navigation|dom-content|evaluation|navigation|chromium-dom-content|chromium-evaluation|chromium-navigation)"' "$chromium_content_features"; then
  echo "Chromium content profile leaked a client, DOM, evaluation, or navigation feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_content_features"; then
  echo "Chromium content profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-context-lifecycle"' "$chromium_context_lifecycle_features"; then
  echo "Chromium umbrella profile did not select chromium-context-lifecycle" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "runtime"' "$chromium_context_lifecycle_features"; then
  echo "Chromium Context-lifecycle profile did not expose its required runtime boundary" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-dom-content|chromium-evaluation|chromium-locator|chromium-navigation|content|dom-content|evaluation|locator|navigation)"' "$chromium_context_lifecycle_features"; then
  echo "Chromium Context-lifecycle profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_context_lifecycle_features"; then
  echo "Chromium Context-lifecycle profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-page-lifecycle"' "$chromium_page_lifecycle_features"; then
  echo "Chromium umbrella profile did not select chromium-page-lifecycle" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "runtime"' "$chromium_page_lifecycle_features"; then
  echo "Chromium Page-lifecycle profile did not expose its required runtime boundary" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-context|chromium-dom-content|chromium-evaluation|chromium-locator|chromium-navigation|content|context-|dom-content|evaluation|locator|navigation)"' "$chromium_page_lifecycle_features"; then
  echo "Chromium Page-lifecycle profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_page_lifecycle_features"; then
  echo "Chromium Page-lifecycle profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-page-creation"' "$chromium_page_creation_features"; then
  echo "Chromium umbrella profile did not select chromium-page-creation" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-page-lifecycle"' "$chromium_page_creation_features"; then
  echo "Chromium Page-creation profile did not select its required lifecycle composition" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-context|chromium-dom-content|chromium-evaluation|chromium-locator|chromium-navigation|content|context-|dom-content|evaluation|locator|navigation)"' "$chromium_page_creation_features"; then
  echo "Chromium Page-creation profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_page_creation_features"; then
  echo "Chromium Page-creation profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-page-events"' "$chromium_page_events_features"; then
  echo "Chromium umbrella profile did not select chromium-page-events" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "transport"' "$chromium_page_events_features"; then
  echo "Chromium Page-events profile did not expose its transport boundary" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-context|chromium-dom-content|chromium-evaluation|chromium-handle|chromium-locator|chromium-navigation|content|context-|dom-content|evaluation|handle-|locator|navigation)"' "$chromium_page_events_features"; then
  echo "Chromium Page-events profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_page_events_features"; then
  echo "Chromium Page-events profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-network-events"' "$chromium_network_events_features"; then
  echo "Chromium umbrella profile did not select chromium-network-events" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "transport"' "$chromium_network_events_features"; then
  echo "Chromium Network-events profile did not expose its transport boundary" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "cdp"' "$chromium_network_events_features"; then
  echo "Chromium Network-events profile did not expose its CDP batch boundary" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "runtime"' "$chromium_network_events_features"; then
  echo "Chromium Network-events profile did not expose its lifecycle runtime boundary" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-context|chromium-dom-content|chromium-evaluation|chromium-handle|chromium-locator|chromium-navigation|chromium-page|content|context-|dom-content|evaluation|handle-|locator|navigation|page-)"' "$chromium_network_events_features"; then
  echo "Chromium Network-events profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_network_events_features"; then
  echo "Chromium Network-events profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-handle-lifecycle"' "$chromium_handle_lifecycle_features"; then
  echo "Chromium umbrella profile did not select chromium-handle-lifecycle" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "runtime"' "$chromium_handle_lifecycle_features"; then
  echo "Chromium Handle-lifecycle profile did not expose its required runtime boundary" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-context|chromium-dom-content|chromium-evaluation|chromium-locator|chromium-navigation|chromium-page|content|context-|dom-content|evaluation|locator|navigation|page-)"' "$chromium_handle_lifecycle_features"; then
  echo "Chromium Handle-lifecycle profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_handle_lifecycle_features"; then
  echo "Chromium Handle-lifecycle profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-handle-evaluation"' "$chromium_handle_evaluation_features"; then
  echo "Chromium umbrella profile did not select chromium-handle-evaluation" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-handle-lifecycle"' "$chromium_handle_evaluation_features"; then
  echo "Chromium Handle-evaluation profile did not select its required lifecycle composition" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "runtime"' "$chromium_handle_evaluation_features"; then
  echo "Chromium Handle-evaluation profile did not expose its required runtime boundary" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-context|chromium-dom-content|chromium-evaluation|chromium-locator|chromium-navigation|chromium-page|content|context-|dom-content|locator|navigation|page-)"' "$chromium_handle_evaluation_features"; then
  echo "Chromium Handle-evaluation profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_handle_evaluation_features"; then
  echo "Chromium Handle-evaluation profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-context-creation"' "$chromium_context_creation_features"; then
  echo "Chromium umbrella profile did not select chromium-context-creation" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-context-lifecycle"' "$chromium_context_creation_features"; then
  echo "Chromium Context-creation profile did not select its required lifecycle composition" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-dom-content|chromium-evaluation|chromium-locator|chromium-navigation|content|dom-content|evaluation|locator|navigation)"' "$chromium_context_creation_features"; then
  echo "Chromium Context-creation profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_context_creation_features"; then
  echo "Chromium Context-creation profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-locator"' "$chromium_locator_features"; then
  echo "Chromium umbrella profile did not select chromium-locator" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-dom-content|chromium-evaluation|chromium-locator-screenshot|chromium-navigation|content|dom-content|evaluation|locator-screenshot|navigation)"' "$chromium_locator_features"; then
  echo "Chromium Locator profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_locator_features"; then
  echo "Chromium Locator profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-locator-screenshot"' "$chromium_locator_screenshot_features"; then
  echo "Chromium umbrella profile did not select chromium-locator-screenshot" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-selector-geometry"' "$chromium_locator_screenshot_features"; then
  echo "Chromium Locator screenshot profile did not select the first-party selector-geometry plugin" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-|chromium-content|chromium-dom-content|chromium-evaluation|chromium-navigation|content|dom-content|evaluation|navigation)"' "$chromium_locator_screenshot_features"; then
  echo "Chromium Locator screenshot profile leaked a client or unrelated capability feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_locator_screenshot_features"; then
  echo "Chromium Locator screenshot profile selected the aggregate CDP backend" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-cap-locator ' "$chromium_locator_screenshot_features"; then
  echo "Chromium Locator screenshot profile selected public Locator command authority" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-dom-content"' "$chromium_dom_content_features"; then
  echo "Chromium umbrella profile did not select chromium-dom-content" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-dom-content|client-evaluation|client-navigation|evaluation|navigation|chromium-evaluation|chromium-navigation)"' "$chromium_dom_content_features"; then
  echo "Chromium DOM-content profile leaked a client, evaluation, or navigation feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_dom_content_features"; then
  echo "Chromium DOM-content profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-evaluation"' "$chromium_evaluation_features"; then
  echo "Chromium umbrella profile did not select chromium-evaluation" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-evaluation|client-navigation|navigation|chromium-navigation)"' "$chromium_evaluation_features"; then
  echo "Chromium evaluation umbrella profile leaked a client or navigation feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_evaluation_features"; then
  echo "Chromium evaluation umbrella profile selected the aggregate CDP backend" >&2
  exit 1
fi
if ! grep -Fq 'playwright-rust feature "chromium-navigation"' "$chromium_features"; then
  echo "Chromium umbrella profile did not select chromium-navigation" >&2
  exit 1
fi
if grep -Eq 'playwright-rust feature "(client|client-navigation)"' "$chromium_features"; then
  echo "Chromium umbrella profile leaked a client feature" >&2
  exit 1
fi
if grep -Fq 'playwright-rust-backend-cdp ' "$chromium_features"; then
  echo "Chromium navigation profile selected the aggregate CDP backend" >&2
  exit 1
fi

echo "v2 umbrella dependency proofs passed (30 explicit empty, client, Chromium, and ordered Handle-delivery profiles)"
