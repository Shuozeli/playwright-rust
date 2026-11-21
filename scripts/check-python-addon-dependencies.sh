#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"

actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
expected=$(sort -u scripts/playwright-rust-python-base-dependencies.txt)

if [[ "$actual" != "$expected" ]]; then
  printf 'Unexpected Python base-wheel Playwright Rust dependency closure:\n%s\nExpected:\n%s\n' \
    "$actual" "$expected" >&2
  exit 1
fi

# The security-details leaf is absent from the default wheel. Selecting it
# joins the existing request-history and frame-navigation owners, adds only
# their frame-lifecycle package, and forwards the opt-in through exactly six
# Rust crates. Keep response publication and terminal readiness independent.
response_security_base_tree=$(cargo tree --locked -p playwright-rust-python-addon \
  --no-default-features --edges normal --prefix none --format '{p} [{f}]')
response_security_base_packages=$(printf '%s\n' "$response_security_base_tree" \
  | awk '/^playwright-rust-/ { print $1 }' | sort -u)
response_security_actual_tree=$(cargo tree --locked -p playwright-rust-python-addon \
  --no-default-features --features response-security-details \
  --edges normal --prefix none --format '{p} [{f}]')
response_security_actual_packages=$(printf '%s\n' "$response_security_actual_tree" \
  | awk '/^playwright-rust-/ { print $1 }' | sort -u)
response_security_added=$(comm -13 \
  <(printf '%s\n' "$response_security_base_packages") \
  <(printf '%s\n' "$response_security_actual_packages"))
response_security_removed=$(comm -23 \
  <(printf '%s\n' "$response_security_base_packages") \
  <(printf '%s\n' "$response_security_actual_packages"))

if [[ "$response_security_base_packages" != "$actual" \
  || "$response_security_base_tree" == *response-security-details* \
  || "$response_security_added" != "playwright-rust-plugin-chromium-frame-lifecycle" \
  || -n "$response_security_removed" ]]; then
  printf 'Unexpected Python response-security-details default-off/opt-in package closure:\nAdded:\n%s\nRemoved:\n%s\n' \
    "$response_security_added" "$response_security_removed" >&2
  exit 1
fi

response_security_featured=$(printf '%s\n' "$response_security_actual_tree" \
  | awk '$1 ~ /^playwright-rust-/ && /\[[^]]*response-security-details/ { print $1 }' \
  | sort -u)
response_security_expected_featured=$(printf '%s\n' \
  playwright-rust-backend-cdp \
  playwright-rust-backend-cdp-network-events \
  playwright-rust-composition-chromium \
  playwright-rust-composition-chromium-network-events \
  playwright-rust-node-host \
  playwright-rust-python-addon | sort -u)
response_security_host_features=$(printf '%s\n' "$response_security_actual_tree" \
  | awk '$1 == "playwright-rust-node-host" && !found { print; found = 1 }')
response_security_forbidden=$(printf '%s\n' "$response_security_actual_tree" \
  | awk '$1 == "playwright-rust-python-addon" || $1 == "playwright-rust-node-host" { \
      if ($0 ~ /response-events|response-finished|navigation-response-finished/) print \
    }')

if [[ "$response_security_featured" != "$response_security_expected_featured" \
  || "$response_security_host_features" != *response-request* \
  || "$response_security_host_features" != *frame-navigation* \
  || "$response_security_host_features" != *request-history* \
  || -n "$response_security_forbidden" ]]; then
  printf 'Unexpected Python response-security-details feature closure:\nSelected:\n%s\nExpected:\n%s\nForbidden:\n%s\n' \
    "$response_security_featured" "$response_security_expected_featured" "$response_security_forbidden" >&2
  exit 1
fi
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features response-security-details
echo 'Python response-security-details dependency proof passed (default off; one frame-lifecycle package added; six exact feature owners; no response-events or completion feature)'
if [[ "${1:-}" == "--response-security-details-only" ]]; then
  exit 0
fi

playwright_server_initialize_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features playwright-server-initialize \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
playwright_server_initialize_added=$(comm -13 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$playwright_server_initialize_actual"))
playwright_server_initialize_removed=$(comm -23 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$playwright_server_initialize_actual"))

if [[ "$playwright_server_initialize_added" != $'playwright-rust-playwright-server-initialize\nplaywright-rust-tokio-tungstenite\nplaywright-rust-transport-ws\nplaywright-rust-tungstenite' || -n "$playwright_server_initialize_removed" ]]; then
  printf 'Unexpected Python Playwright Server initialize dependency delta:\nAdded:\n%s\nRemoved:\n%s\n' \
    "$playwright_server_initialize_added" "$playwright_server_initialize_removed" >&2
  exit 1
fi

service_worker_policy_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features service-worker-policy \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$service_worker_policy_actual" != "$actual" ]]; then
  printf 'Python service-worker-policy profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$service_worker_policy_actual" "$actual" >&2
  exit 1
fi

keyboard_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features keyboard \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$keyboard_actual" != "$actual" ]]; then
  printf 'Python keyboard profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$keyboard_actual" "$actual" >&2
  exit 1
fi

cdp_session_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features cdp-session \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$cdp_session_actual" != "$actual" ]]; then
  printf 'Python cdp-session profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$cdp_session_actual" "$actual" >&2
  exit 1
fi

cdp_handshake_headers_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features cdp-handshake-headers \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$cdp_handshake_headers_actual" != "$actual" ]]; then
  printf 'Python cdp-handshake-headers profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$cdp_handshake_headers_actual" "$actual" >&2
  exit 1
fi

cdp_ws_close_reason_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features cdp-ws-close-reason \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$cdp_ws_close_reason_actual" != "$actual" ]]; then
  printf 'Python cdp-ws-close-reason profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$cdp_ws_close_reason_actual" "$actual" >&2
  exit 1
fi

cdp_ws_invalid_frame_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features cdp-ws-invalid-frame \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$cdp_ws_invalid_frame_actual" != "$actual" ]]; then
  printf 'Python cdp-ws-invalid-frame profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$cdp_ws_invalid_frame_actual" "$actual" >&2
  exit 1
fi

handles_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features handles,wait-for-function \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$handles_actual" != "$actual" ]]; then
  printf 'Python handles/wait-for-function profile changed the Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$handles_actual" "$actual" >&2
  exit 1
fi

page_bindings_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-bindings \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
context_bindings_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-bindings \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_bindings_actual" != "$page_bindings_actual" ]]; then
  printf 'Python context-bindings profile changed the Page-bindings Playwright Rust package closure:\n%s\nPage bindings:\n%s\n' \
    "$context_bindings_actual" "$page_bindings_actual" >&2
  exit 1
fi

element_handle_dom_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features element-handle-dom \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$element_handle_dom_actual" != "$handles_actual" ]]; then
  printf 'Python element-handle-dom profile changed the handles Playwright Rust package closure:\n%s\nHandles:\n%s\n' \
    "$element_handle_dom_actual" "$handles_actual" >&2
  exit 1
fi

element_handle_dom_reads_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features element-handle-dom-reads \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$element_handle_dom_reads_actual" != "$element_handle_dom_actual" ]]; then
  printf 'Python element-handle-dom-reads profile changed the DOM Playwright Rust package closure:\n%s\nDOM:\n%s\n' \
    "$element_handle_dom_reads_actual" "$element_handle_dom_actual" >&2
  exit 1
fi

element_handle_scroll_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features element-handle-scroll-into-view \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$element_handle_scroll_actual" != "$handles_actual" ]]; then
  printf 'Python element-handle-scroll-into-view profile changed the Handles Playwright Rust package closure:\n%s\nHandles:\n%s\n' \
    "$element_handle_scroll_actual" "$handles_actual" >&2
  exit 1
fi

element_handle_select_text_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features element-handle-select-text \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$element_handle_select_text_actual" != "$handles_actual" ]]; then
  printf 'Python element-handle-select-text profile changed the Handles Playwright Rust package closure:\n%s\nHandles:\n%s\n' \
    "$element_handle_select_text_actual" "$handles_actual" >&2
  exit 1
fi

locator_highlight_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features locator-highlight \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$locator_highlight_actual" != "$actual" ]]; then
  printf 'Python locator-highlight profile changed the Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$locator_highlight_actual" "$actual" >&2
  exit 1
fi

locator_dom_actions_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features locator-dom-actions \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$locator_dom_actions_actual" != "$actual" ]]; then
  printf 'Python locator-dom-actions profile changed the Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$locator_dom_actions_actual" "$actual" >&2
  exit 1
fi

locator_click_modifiers_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features locator-click-modifiers \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$locator_click_modifiers_actual" != "$actual" ]]; then
  printf 'Python locator-click-modifiers profile changed the Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$locator_click_modifiers_actual" "$actual" >&2
  exit 1
fi

page_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
page_events_added=$(comm -13 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$page_events_actual"))
page_events_removed=$(comm -23 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$page_events_actual"))

if [[ -n "$page_events_removed" ]]; then
  printf 'Python page-events profile removed base dependencies:\n%s\n' \
    "$page_events_removed" >&2
  exit 1
fi

if [[ "$page_events_added" != "playwright-rust-plugin-chromium-frame-lifecycle" ]]; then
  printf 'Unexpected Python page-events dependency delta:\n%s\nExpected:\n%s\n' \
    "$page_events_added" 'playwright-rust-plugin-chromium-frame-lifecycle' >&2
  exit 1
fi

if [[ "$page_bindings_actual" != "$page_events_actual" ]]; then
  printf 'Python page-bindings profile changed the anchored handles/page-events Playwright Rust package closure:\n%s\nExpected:\n%s\n' \
    "$page_bindings_actual" "$page_events_actual" >&2
  exit 1
fi

emulate_media_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features emulate-media \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
context_forced_colors_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-forced-colors \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$emulate_media_actual" != "$actual" || "$context_forced_colors_actual" != "$emulate_media_actual" ]]; then
  printf 'Python emulate-media/context-forced-colors changed the anchored base Playwright Rust package closure:\n%s\n%s\nExpected:\n%s\n' \
    "$emulate_media_actual" "$context_forced_colors_actual" "$actual" >&2
  exit 1
fi

frame_navigation_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features frame-navigation \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$frame_navigation_actual" != "$page_events_actual" ]]; then
  printf 'Python frame-navigation profile changed the frame-lifecycle Playwright Rust package closure:\n%s\nFrame lifecycle:\n%s\n' \
    "$frame_navigation_actual" "$page_events_actual" >&2
  exit 1
fi

navigation_url_match_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features navigation-url-match \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$navigation_url_match_actual" != "$frame_navigation_actual" ]]; then
  printf 'Python navigation-url-match changed the frame-navigation Playwright Rust package closure:\n%s\nFrame navigation:\n%s\n' \
    "$navigation_url_match_actual" "$frame_navigation_actual" >&2
  exit 1
fi

response_body_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features response-body \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$response_body_actual" != "$frame_navigation_actual" ]]; then
  printf 'Python response-body profile changed the frame-navigation Playwright Rust package closure:\n%s\nFrame navigation:\n%s\n' \
    "$response_body_actual" "$frame_navigation_actual" >&2
  exit 1
fi

page_errors_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-errors \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$page_errors_actual" != "$page_events_actual" ]]; then
  printf 'Python page-errors profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$page_errors_actual" "$page_events_actual" >&2
  exit 1
fi

console_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features console-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$console_events_actual" != "$page_events_actual" ]]; then
  printf 'Python console-events profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$console_events_actual" "$page_events_actual" >&2
  exit 1
fi

console_argument_handles_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features console-argument-handles \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$console_argument_handles_actual" != "$page_events_actual" ]]; then
  printf 'Python console-argument-handles profile changed the combined Handle/page-events Playwright Rust package closure:\n%s\nPage events plus Handles:\n%s\n' \
    "$console_argument_handles_actual" "$page_events_actual" >&2
  exit 1
fi

console_history_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features console-history \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$console_history_actual" != "$page_events_actual" ]]; then
  printf 'Python console-history profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$console_history_actual" "$page_events_actual" >&2
  exit 1
fi

page_dialogs_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-dialogs \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$page_dialogs_actual" != "$page_events_actual" ]]; then
  printf 'Python page-dialogs profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$page_dialogs_actual" "$page_events_actual" >&2
  exit 1
fi

page_routes_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-routes \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$page_routes_actual" != "$page_events_actual" ]]; then
  printf 'Python page-routes profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$page_routes_actual" "$page_events_actual" >&2
  exit 1
fi

route_redirects_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features route-redirects \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
route_redirects_union=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-routes,request-history \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$route_redirects_actual" != "$route_redirects_union" ]]; then
  printf 'Python route-redirects profile is not the exact Page-routes/request-history union:\n%s\nExpected:\n%s\n' \
    "$route_redirects_actual" "$route_redirects_union" >&2
  exit 1
fi

route_fulfill_path_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features route-fulfill-path \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$route_fulfill_path_actual" != "$page_routes_actual" ]]; then
  printf 'Python route-fulfill-path profile changed the page-routes Playwright Rust package closure:\n%s\nPage routes:\n%s\n' \
    "$route_fulfill_path_actual" "$page_routes_actual" >&2
  exit 1
fi

har_replay_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features har-replay \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
har_replay_added=$(comm -13 \
  <(printf '%s\n' "$page_routes_actual") \
  <(printf '%s\n' "$har_replay_actual"))
har_replay_removed=$(comm -23 \
  <(printf '%s\n' "$page_routes_actual") \
  <(printf '%s\n' "$har_replay_actual"))

if [[ -n "$har_replay_removed" ]]; then
  printf 'Python har-replay profile removed page-routes dependencies:\n%s\n' \
    "$har_replay_removed" >&2
  exit 1
fi

if [[ "$har_replay_added" != "playwright-rust-har-replay" ]]; then
  printf 'Unexpected Python har-replay dependency delta:\n%s\nExpected:\n%s\n' \
    "$har_replay_added" 'playwright-rust-har-replay' >&2
  exit 1
fi

download_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features download-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$download_events_actual" != "$page_events_actual" ]]; then
  printf 'Python download-events profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$download_events_actual" "$page_events_actual" >&2
  exit 1
fi

worker_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features worker-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$worker_events_actual" != "$page_events_actual" ]]; then
  printf 'Python worker-events profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$worker_events_actual" "$page_events_actual" >&2
  exit 1
fi

worker_handle_evaluation_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features worker-handle-evaluation \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$worker_handle_evaluation_actual" != "$page_events_actual" ]]; then
  printf 'Python worker-handle-evaluation profile changed the combined Worker/Handle Playwright Rust package closure:\n%s\nPage events plus Handles:\n%s\n' \
    "$worker_handle_evaluation_actual" "$page_events_actual" >&2
  exit 1
fi

worker_console_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features worker-console-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$worker_console_events_actual" != "$page_events_actual" ]]; then
  printf 'Python worker-console-events profile changed the combined Worker/console Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$worker_console_events_actual" "$page_events_actual" >&2
  exit 1
fi

worker_console_argument_handles_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features worker-console-argument-handles \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$worker_console_argument_handles_actual" != "$worker_handle_evaluation_actual" ]]; then
  printf 'Python worker-console-argument-handles profile changed the Worker Handle Playwright Rust package closure:\n%s\nWorker Handles:\n%s\n' \
    "$worker_console_argument_handles_actual" "$worker_handle_evaluation_actual" >&2
  exit 1
fi

worker_error_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features worker-error-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$worker_error_events_actual" != "$page_events_actual" ]]; then
  printf 'Python worker-error-events profile changed the combined Worker/page-errors Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$worker_error_events_actual" "$page_events_actual" >&2
  exit 1
fi

network_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features network-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

context_network_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-network-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_network_events_actual" != "$page_events_actual" ]]; then
  printf 'Python context-network-events profile changed the combined network/page-events Playwright Rust package closure:\n%s\nNetwork plus page events:\n%s\n' \
    "$context_network_events_actual" "$page_events_actual" >&2
  exit 1
fi

worker_network_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features worker-network-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$worker_network_events_actual" != "$context_network_events_actual" ]]; then
  printf 'Python worker-network-events profile changed the combined Worker/network Playwright Rust package closure:\n%s\nNetwork plus page events:\n%s\n' \
    "$worker_network_events_actual" "$context_network_events_actual" >&2
  exit 1
fi

request_frame_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features request-frame \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$request_frame_actual" != "$network_events_actual" ]]; then
  printf 'Python request-frame profile changed the network-events Playwright Rust package closure:\n%s\nNetwork events:\n%s\n' \
    "$request_frame_actual" "$network_events_actual" >&2
  exit 1
fi

request_timing_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features request-timing \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$request_timing_actual" != "$network_events_actual" ]]; then
  printf 'Python request-timing profile changed the network-events Playwright Rust package closure:\n%s\nNetwork events:\n%s\n' \
    "$request_timing_actual" "$network_events_actual" >&2
  exit 1
fi

request_post_data_json_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features request-post-data-json \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$request_post_data_json_actual" != "$network_events_actual" ]]; then
  printf 'Python request-post-data-json profile changed the network-events Playwright Rust package closure:\n%s\nNetwork events:\n%s\n' \
    "$request_post_data_json_actual" "$network_events_actual" >&2
  exit 1
fi

request_history_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features request-history \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$request_history_actual" != "$network_events_actual" ]]; then
  printf 'Python request-history profile changed the network-events Playwright Rust package closure:\n%s\nNetwork events:\n%s\n' \
    "$request_history_actual" "$network_events_actual" >&2
  exit 1
fi

request_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features request-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$request_events_actual" != "$network_events_actual" ]]; then
  printf 'Python request-events profile changed the network-events Playwright Rust package closure:\n%s\nNetwork events:\n%s\n' \
    "$request_events_actual" "$network_events_actual" >&2
  exit 1
fi

response_events_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features response-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$response_events_actual" != "$network_events_actual" ]]; then
  printf 'Python response-events profile changed the network-events Playwright Rust package closure:\n%s\nNetwork events:\n%s\n' \
    "$response_events_actual" "$network_events_actual" >&2
  exit 1
fi

response_finished_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features response-finished \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$response_finished_actual" != "$response_events_actual" ]]; then
  printf 'Python response-finished profile changed the response-events Playwright Rust package closure:\n%s\nResponse events:\n%s\n' \
    "$response_finished_actual" "$response_events_actual" >&2
  exit 1
fi

navigation_response_finished_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features navigation-response-finished \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
navigation_response_finished_expected=$(printf '%s\n%s\n' \
  "$frame_navigation_actual" "$response_finished_actual" | sort -u)

if [[ "$navigation_response_finished_actual" != "$navigation_response_finished_expected" ]]; then
  printf 'Python navigation-response-finished changed the exact Frame/navigation plus response-finished Playwright Rust package closure:\n%s\nExpected:\n%s\n' \
    "$navigation_response_finished_actual" "$navigation_response_finished_expected" >&2
  exit 1
fi

extra_headers_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features extra-headers \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$extra_headers_actual" != "$actual" ]]; then
  printf 'Python extra-headers profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$extra_headers_actual" "$actual" >&2
  exit 1
fi

persistent_context_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
persistent_context_added=$(comm -13 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$persistent_context_actual"))
persistent_context_removed=$(comm -23 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$persistent_context_actual"))
persistent_context_expected=$(cat <<'EOF'
playwright-rust-cap-browser-lifecycle
playwright-rust-codec-browser-lifecycle
playwright-rust-composition-chromium-launch
playwright-rust-provider-chromium
playwright-rust-server-browser-lifecycle
EOF
)

if [[ -n "$persistent_context_removed" ]]; then
  printf 'Python persistent-context profile removed base dependencies:\n%s\n' \
    "$persistent_context_removed" >&2
  exit 1
fi

if [[ "$persistent_context_added" != "$persistent_context_expected" ]]; then
  printf 'Unexpected Python persistent-context dependency delta:\n%s\nExpected:\n%s\n' \
    "$persistent_context_added" "$persistent_context_expected" >&2
  exit 1
fi

persistent_context_locale_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-locale \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_locale_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-locale profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_locale_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_timezone_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-timezone \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_timezone_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-timezone profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_timezone_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_offline_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-offline \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_offline_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-offline profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_offline_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_geolocation_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-geolocation \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_geolocation_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-geolocation profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_geolocation_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_geolocation_permission_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-geolocation-permission \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_geolocation_permission_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-geolocation-permission profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_geolocation_permission_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_http_credentials_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-http-credentials \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
persistent_context_http_credentials_expected=$(printf '%s\n%s\n' \
  "$persistent_context_actual" "$page_events_actual" | sort -u)

if [[ "$persistent_context_http_credentials_actual" != "$persistent_context_http_credentials_expected" ]]; then
  printf 'Python persistent-context-http-credentials profile changed the combined persistent-context/page-events Playwright Rust package closure:\n%s\nExpected:\n%s\n' \
    "$persistent_context_http_credentials_actual" "$persistent_context_http_credentials_expected" >&2
  exit 1
fi

persistent_context_launch_arguments_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-launch-arguments \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_launch_arguments_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-launch-arguments profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_launch_arguments_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_ignore_https_errors_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-ignore-https-errors \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_ignore_https_errors_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-ignore-https-errors profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_ignore_https_errors_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_device_emulation_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-device-emulation \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_device_emulation_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-device-emulation profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_device_emulation_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_emulate_media_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-emulate-media \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_emulate_media_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-emulate-media profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_emulate_media_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_extra_headers_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-extra-headers \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_extra_headers_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-extra-headers profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_extra_headers_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_bypass_csp_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-bypass-csp \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_bypass_csp_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-bypass-csp profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_bypass_csp_actual" "$persistent_context_actual" >&2
  exit 1
fi

persistent_context_javascript_disabled_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features persistent-context-javascript-disabled \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$persistent_context_javascript_disabled_actual" != "$persistent_context_actual" ]]; then
  printf 'Python persistent-context-javascript-disabled profile changed the persistent-context Playwright Rust package closure:\n%s\nPersistent context:\n%s\n' \
    "$persistent_context_javascript_disabled_actual" "$persistent_context_actual" >&2
  exit 1
fi

context_cookies_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-cookies \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_cookies_actual" != "$actual" ]]; then
  printf 'Python context-cookies profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$context_cookies_actual" "$actual" >&2
  exit 1
fi

page_pause_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-pause \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$page_pause_actual" != "$actual" ]]; then
  printf 'Python page-pause profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$page_pause_actual" "$actual" >&2
  exit 1
fi

wait_for_timeout_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features wait-for-timeout \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$wait_for_timeout_actual" != "$actual" ]]; then
  printf 'Python wait-for-timeout profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$wait_for_timeout_actual" "$actual" >&2
  exit 1
fi

request_gc_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features request-gc \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$request_gc_actual" != "$actual" ]]; then
  printf 'Python request-gc profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$request_gc_actual" "$actual" >&2
  exit 1
fi

device_emulation_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features device-emulation \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$device_emulation_actual" != "$actual" ]]; then
  printf 'Python device-emulation profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$device_emulation_actual" "$actual" >&2
  exit 1
fi

touchscreen_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features touchscreen-input \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
element_handle_tap_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features element-handle-tap \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
touchscreen_added=$(comm -13 <(printf '%s\n' "$actual") <(printf '%s\n' "$touchscreen_actual"))
touchscreen_removed=$(comm -23 <(printf '%s\n' "$actual") <(printf '%s\n' "$touchscreen_actual"))
expected_touchscreen_added=$(printf '%s\n' \
  playwright-rust-backend-cdp-touchscreen \
  playwright-rust-cap-touchscreen \
  playwright-rust-composition-chromium-touchscreen)
if [[ "$touchscreen_added" != "$expected_touchscreen_added" || -n "$touchscreen_removed" || "$element_handle_tap_actual" != "$touchscreen_actual" ]]; then
  printf 'Unexpected Python touchscreen/retained-tap dependency closure:\nAdded:\n%s\nRemoved:\n%s\nExpected added:\n%s\n' \
    "$touchscreen_added" "$touchscreen_removed" "$expected_touchscreen_added" >&2
  exit 1
fi

context_locale_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-locale \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_locale_actual" != "$actual" ]]; then
  printf 'Python context-locale profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$context_locale_actual" "$actual" >&2
  exit 1
fi

context_timezone_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-timezone \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_timezone_actual" != "$actual" ]]; then
  printf 'Python context-timezone profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$context_timezone_actual" "$actual" >&2
  exit 1
fi

context_offline_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-offline \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_offline_actual" != "$actual" ]]; then
  printf 'Python context-offline profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$context_offline_actual" "$actual" >&2
  exit 1
fi

context_geolocation_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-geolocation \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_geolocation_actual" != "$actual" ]]; then
  printf 'Python context-geolocation profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$context_geolocation_actual" "$actual" >&2
  exit 1
fi

context_geolocation_permission_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-geolocation-permission \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_geolocation_permission_actual" != "$actual" ]]; then
  printf 'Python context-geolocation-permission profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$context_geolocation_permission_actual" "$actual" >&2
  exit 1
fi

context_http_credentials_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-http-credentials \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_http_credentials_actual" != "$page_events_actual" ]]; then
  printf 'Python context-http-credentials profile changed the page-events Playwright Rust package closure:\n%s\nPage events:\n%s\n' \
    "$context_http_credentials_actual" "$page_events_actual" >&2
  exit 1
fi

ignore_https_errors_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features ignore-https-errors \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$ignore_https_errors_actual" != "$actual" ]]; then
  printf 'Python ignore-https-errors profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$ignore_https_errors_actual" "$actual" >&2
  exit 1
fi

bypass_csp_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features bypass-csp \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$bypass_csp_actual" != "$actual" ]]; then
  printf 'Python bypass-csp profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$bypass_csp_actual" "$actual" >&2
  exit 1
fi

javascript_disabled_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features javascript-disabled \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$javascript_disabled_actual" != "$actual" ]]; then
  printf 'Python javascript-disabled profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$javascript_disabled_actual" "$actual" >&2
  exit 1
fi

page_pdf_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-pdf \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$page_pdf_actual" != "$actual" ]]; then
  printf 'Python page-pdf profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$page_pdf_actual" "$actual" >&2
  exit 1
fi

page_viewport_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-viewport \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$page_viewport_actual" != "$actual" ]]; then
  printf 'Python page-viewport profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$page_viewport_actual" "$actual" >&2
  exit 1
fi

locator_bounding_box_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features locator-bounding-box \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$locator_bounding_box_actual" != "$actual" ]]; then
  printf 'Python locator-bounding-box profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$locator_bounding_box_actual" "$actual" >&2
  exit 1
fi

locator_screenshot_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features locator-screenshot \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$locator_screenshot_actual" != "$actual" ]]; then
  printf 'Python locator-screenshot profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$locator_screenshot_actual" "$actual" >&2
  exit 1
fi

selector_registration_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features selector-registration \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$selector_registration_actual" != "$actual" ]]; then
  printf 'Python selector-registration profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$selector_registration_actual" "$actual" >&2
  exit 1
fi

layout_selectors_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features layout-selectors \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$layout_selectors_actual" != "$actual" ]]; then
  printf 'Python layout-selectors profile changed the base Playwright Rust package closure:\n%s\nBase:\n%s\n' \
    "$layout_selectors_actual" "$actual" >&2
  exit 1
fi

file_input_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features file-input \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
file_input_added=$(comm -13 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$file_input_actual"))
file_input_removed=$(comm -23 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$file_input_actual"))
file_input_expected=$(cat <<'EOF'
playwright-rust-backend-cdp-file-input
playwright-rust-composition-chromium-file-input
playwright-rust-file-input-fs
playwright-rust-file-input-model
playwright-rust-file-input-source
playwright-rust-file-input-tokio
EOF
)

if [[ -n "$file_input_removed" ]]; then
  printf 'Python file-input profile removed base dependencies:\n%s\n' \
    "$file_input_removed" >&2
  exit 1
fi

if [[ "$file_input_added" != "$file_input_expected" ]]; then
  printf 'Unexpected Python file-input dependency delta:\n%s\nExpected:\n%s\n' \
    "$file_input_added" "$file_input_expected" >&2
  exit 1
fi

file_input_native_paths_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features file-input-native-paths \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$file_input_native_paths_actual" != "$file_input_actual" ]]; then
  printf 'Python file-input-native-paths profile changed the file-input Playwright Rust package closure:\n%s\nExpected:\n%s\n' \
    "$file_input_native_paths_actual" "$file_input_actual" >&2
  exit 1
fi

file_chooser_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features file-chooser-events \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
file_chooser_expected=$(printf '%s\n%s\n' "$page_events_actual" "$file_input_actual" | sort -u)

if [[ "$file_chooser_actual" != "$file_chooser_expected" ]]; then
  printf 'Python file-chooser-events profile changed the combined page-events/file-input Playwright Rust package closure:\n%s\nExpected:\n%s\n' \
    "$file_chooser_actual" "$file_chooser_expected" >&2
  exit 1
fi

file_chooser_native_paths_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features file-chooser-native-paths \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$file_chooser_native_paths_actual" != "$file_chooser_actual" ]]; then
  printf 'Python file-chooser-native-paths profile changed the file-chooser-events Playwright Rust package closure:\n%s\nExpected:\n%s\n' \
    "$file_chooser_native_paths_actual" "$file_chooser_actual" >&2
  exit 1
fi

drag_and_drop_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features drag-and-drop \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
drag_and_drop_added=$(comm -13 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$drag_and_drop_actual"))
drag_and_drop_removed=$(comm -23 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$drag_and_drop_actual"))

if [[ -n "$drag_and_drop_removed" ]]; then
  printf 'Python drag-and-drop profile removed base dependencies:\n%s\n' \
    "$drag_and_drop_removed" >&2
  exit 1
fi

if [[ "$drag_and_drop_added" != "playwright-rust-backend-cdp-drag" ]]; then
  printf 'Unexpected Python drag-and-drop dependency delta:\n%s\nExpected:\n%s\n' \
    "$drag_and_drop_added" 'playwright-rust-backend-cdp-drag' >&2
  exit 1
fi

for forbidden in \
  playwright-rust-backend-cdp-file-input \
  playwright-rust-composition-chromium-file-input \
  playwright-rust-file-input-fs \
  playwright-rust-file-input-model \
  playwright-rust-plugin-chromium-frame-lifecycle
do
  if grep -qx "$forbidden" <<<"$actual"; then
    echo "Python base wheel unexpectedly selects $forbidden" >&2
    exit 1
  fi
done

api_request_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features api-request \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
api_request_added=$(comm -13 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$api_request_actual"))
api_request_removed=$(comm -23 \
  <(printf '%s\n' "$actual") \
  <(printf '%s\n' "$api_request_actual"))
api_request_expected=$(cat <<'EOF'
playwright-rust-api-request
playwright-rust-api-request-auth-basic
playwright-rust-api-request-connector-rustls
playwright-rust-api-request-connector-tokio
playwright-rust-api-request-cookie
playwright-rust-api-request-decompress
playwright-rust-api-request-http1
playwright-rust-api-request-transport
EOF
)

if [[ -n "$api_request_removed" ]]; then
  printf 'Python api-request profile removed base dependencies:\n%s\n' \
    "$api_request_removed" >&2
  exit 1
fi

if [[ "$api_request_added" != "$api_request_expected" ]]; then
  printf 'Unexpected Python api-request dependency delta:\n%s\nExpected:\n%s\n' \
    "$api_request_added" "$api_request_expected" >&2
  exit 1
fi

context_api_request_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-api-request \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
context_api_request_union=$(cargo tree --locked -p playwright-rust-python-addon \
  --features api-request,context-cookies,device-emulation,context-http-credentials,ignore-https-errors \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$context_api_request_actual" != "$context_api_request_union" ]]; then
  printf 'Python context-api-request profile is not the exact request/context-options union:\n%s\nExpected:\n%s\n' \
    "$context_api_request_actual" "$context_api_request_union" >&2
  exit 1
fi

route_fulfill_api_response_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features route-fulfill-api-response \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
route_fulfill_api_response_union=$(cargo tree --locked -p playwright-rust-python-addon \
  --features page-routes,api-request \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$route_fulfill_api_response_actual" != "$route_fulfill_api_response_union" ]]; then
  printf 'Python route-fulfill-api-response profile is not the exact Page-routes/api-request union:\n%s\nExpected:\n%s\n' \
    "$route_fulfill_api_response_actual" "$route_fulfill_api_response_union" >&2
  exit 1
fi

route_fetch_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features route-fetch \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)
route_fetch_union=$(cargo tree --locked -p playwright-rust-python-addon \
  --features context-api-request,page-routes,route-fulfill-api-response \
  --edges normal --prefix none --format '{p}' \
  | awk '/^playwright-rust-/ { print $1 }' \
  | sort -u)

if [[ "$route_fetch_actual" != "$route_fetch_union" ]]; then
  printf 'Python route-fetch profile is not the exact context-request/Page-routes/fulfill-response union:\n%s\nExpected:\n%s\n' \
    "$route_fetch_actual" "$route_fetch_union" >&2
  exit 1
fi

api_request_all=$(cargo tree --locked -p playwright-rust-python-addon \
  --features api-request \
  --edges normal --prefix none --format '{p}' \
  | awk '{ print $1 }' \
  | sort -u)
client_certificate_actual=$(cargo tree --locked -p playwright-rust-python-addon \
  --features api-request-client-certificates \
  --edges normal --prefix none --format '{p}' \
  | awk '{ print $1 }' \
  | sort -u)
client_certificate_added=$(comm -13 \
  <(printf '%s\n' "$api_request_all") \
  <(printf '%s\n' "$client_certificate_actual"))
client_certificate_removed=$(comm -23 \
  <(printf '%s\n' "$api_request_all") \
  <(printf '%s\n' "$client_certificate_actual"))
client_certificate_expected=$(cat <<'EOF'
foreign-types
foreign-types-shared
openssl
openssl-macros
openssl-sys
playwright-rust-api-request-client-certificates
EOF
)

if [[ -n "$client_certificate_removed" ]]; then
  printf 'Python client-certificate profile removed api-request dependencies:\n%s\n' \
    "$client_certificate_removed" >&2
  exit 1
fi

if [[ "$client_certificate_added" != "$client_certificate_expected" ]]; then
  printf 'Unexpected Python client-certificate dependency delta:\n%s\nExpected:\n%s\n' \
    "$client_certificate_added" "$client_certificate_expected" >&2
  exit 1
fi

CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features keyboard
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features handles,wait-for-function
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features element-handle-dom
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features element-handle-dom-reads
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features page-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features frame-navigation
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features navigation-url-match
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features response-body
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features console-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features console-argument-handles
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features page-dialogs
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features page-errors
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features page-routes
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features har-replay
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features request-history
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-network-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features request-frame
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features request-timing
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features request-post-data-json
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features request-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features response-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features response-finished
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features navigation-response-finished
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-locale
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-timezone
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-offline
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-geolocation
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-geolocation-permission
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-http-credentials
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-launch-arguments
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-ignore-https-errors
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-device-emulation
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-emulate-media
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-extra-headers
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-bypass-csp
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features persistent-context-javascript-disabled
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-cookies
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features cdp-session
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features cdp-handshake-headers
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features cdp-ws-close-reason
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features cdp-ws-invalid-frame
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features page-bindings
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features context-bindings
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features context-forced-colors
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features context-bindings,context-forced-colors
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features page-pause
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features wait-for-timeout
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features request-gc
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features device-emulation
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features touchscreen-input
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --no-default-features --features element-handle-tap
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-locale
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-timezone
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-offline
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-geolocation
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-geolocation-permission
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-http-credentials
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features ignore-https-errors
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features bypass-csp
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features javascript-disabled
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features page-pdf
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features page-viewport
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features locator-bounding-box
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features locator-screenshot
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features selector-registration
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features layout-selectors
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features file-input
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features file-input-native-paths
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features file-chooser-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features file-chooser-native-paths
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features download-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features worker-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features worker-handle-evaluation
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features worker-console-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features worker-console-argument-handles
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features worker-error-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features worker-network-events
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features locator-highlight
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features locator-dom-actions
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features route-fulfill-path
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features route-redirects
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features drag-and-drop
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features api-request
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features context-api-request
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features route-fulfill-api-response
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features route-fetch
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features api-request-client-certificates
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features service-worker-policy
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon \
  --features playwright-server-initialize
echo "Python dependency proof passed ($(wc -l < scripts/playwright-rust-python-base-dependencies.txt | tr -d ' ') base Playwright Rust packages; playwright-server-initialize adds exactly four WebSocket/server leaves; response-security-details adds exactly the frame-lifecycle plugin with six feature owners; keyboard, service-worker-policy, handles/wait-for-function, element-handle-dom/element-handle-dom-reads, wait-for-timeout, locator-bounding-box, locator-highlight, locator-screenshot, locator-dom-actions, page-pdf, page-viewport, page-pause, request-gc, device-emulation, context-locale, context-timezone, context-offline, context-geolocation, context-geolocation-permission, context-cookies, extra-headers, ignore-https-errors, bypass-csp, javascript-disabled, selector-registration and layout-selectors add no package; touchscreen-input and element-handle-tap add exactly three touchscreen leaves; page-events, frame-navigation, and context-http-credentials add the frame-lifecycle plugin; response-body matches the frame-navigation closure; console-events/console-argument-handles/console-history/page-dialogs/page-errors/page-routes/download-events/worker-events/worker-handle-evaluation/worker-console-events/worker-console-argument-handles/worker-error-events add nothing beyond page-events; route-redirects is exactly the page-routes/request-history union; route-fulfill-path changes no Playwright Rust package beyond page-routes; har-replay adds exactly the backend-neutral HAR policy crate beyond page-routes; context-network-events and worker-network-events match the combined network/page-events closure; request-frame, request-timing, request-post-data-json, request-history, request-events and response-events add nothing beyond network-events; response-finished adds nothing beyond response-events; navigation-response-finished is exactly the frame-navigation/response-finished/request-history union; persistent-context adds the owned Chromium launch/lifecycle leaves and persistent-context-locale/persistent-context-timezone/persistent-context-offline/persistent-context-geolocation/persistent-context-geolocation-permission/persistent-context-launch-arguments/persistent-context-ignore-https-errors/persistent-context-device-emulation/persistent-context-emulate-media/persistent-context-extra-headers/persistent-context-bypass-csp/persistent-context-javascript-disabled match that closure; persistent-context-http-credentials is exactly the persistent-context/page-events union; file-input adds six bounded upload packages and file-input-native-paths adds no package; file-chooser-events is exactly the page-events/file-input union and file-chooser-native-paths adds no package; drag-and-drop adds one CDP backend leaf; api-request adds 8; context-api-request, route-fulfill-api-response and route-fetch are exact feature unions; client certificates add one Playwright Rust leaf and five OpenSSL packages)"
