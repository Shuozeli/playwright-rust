//! JS value conversion only. All browser behavior belongs to the Rust host.

// Generated N-API paths use this crate-wide facade, including registration
// modules. Renaming the dependency removes the competing extern-prelude name.
extern crate self as napi;
pub use napi_runtime::ctor;
pub use napi_runtime::*;
pub use native_napi::bindgen_prelude;

#[cfg(feature = "api-request")]
mod api_request;
mod boundary;
#[cfg(feature = "browser-registry")]
mod browser_registry;
mod callback_exception;
#[cfg(feature = "cdp-session")]
mod cdp_sessions;
#[cfg(feature = "context-cookies")]
mod context_cookies;
mod context_settings;
#[cfg(feature = "context-storage-state")]
mod context_storage_state;
#[cfg(feature = "context-tracing-screenshots")]
mod context_tracing;
mod deferred_conversion;
mod environment;
mod evaluation_value;
#[cfg(feature = "extra-headers")]
mod extra_headers;
#[cfg(feature = "frame-navigation")]
mod frame_navigation;
mod frame_tree;
#[cfg(feature = "handles")]
mod handles;
#[cfg(feature = "har-recording")]
mod har_recording;
#[cfg(feature = "history-navigation")]
mod history_navigation;
mod keyboard;
mod locator;
mod locator_conversion;
mod mouse;
mod native_classes;
mod native_napi;
mod node_reference;
#[cfg(feature = "page-dialogs")]
mod page_dialogs;
#[cfg(feature = "page-pdf")]
mod page_pdf;
#[cfg(feature = "page-routes")]
mod page_routes;
mod page_snapshot;
#[cfg(feature = "persistent-context-preflight")]
mod persistent_context_preflight;
mod publication_port;
mod reload;
#[cfg(feature = "playwright-server-connect-session")]
mod remote_browser;
#[cfg(feature = "playwright-server-connect-page-events")]
mod remote_page_notice;
#[cfg(feature = "response-body")]
mod response_body;
mod screenshot;
#[cfg(feature = "timeout-settings")]
mod timeout_settings;
#[cfg(feature = "touchscreen-input")]
mod touchscreen;
mod url_value;
#[cfg(any(
    feature = "cdp-default-user-agent",
    feature = "playwright-server-connect-headers",
    feature = "playwright-server-connect-session"
))]
mod user_agent_runtime;
#[cfg(feature = "video-recording")]
mod video_recording;
#[cfg(feature = "handles")]
mod wait_admission;
#[cfg(feature = "wait-for-timeout")]
mod wait_for_timeout;

#[cfg(feature = "retained-file-input")]
pub use handles::file_input::{
    inspect_file_input_arguments, inspect_file_input_channel, inspect_file_input_materialization,
    inspect_file_input_preflight_context, inspect_file_input_preflight_conversion,
    inspect_file_input_preflight_owner, inspect_file_input_projection,
    inspect_file_input_server_channel,
};
pub use native_classes::{native_class_matches, native_class_names};

use std::collections::HashMap;
use std::sync::Arc;

use bindgen_prelude::Buffer;
#[cfg(any(
    feature = "request-history",
    feature = "response-finished",
    feature = "worker-events"
))]
use bindgen_prelude::External;
use bindgen_prelude::{PromiseRaw, ToNapiValue};
use napi_derive::napi;
use playwright_rust_node_host::ffi_budget::{FfiBudget, FfiLane};
use playwright_rust_node_host::{HostError, HostSession};

/// JS/Rust addon contract identity. Increment this for any breaking export or
/// value-shape change that the package loader cannot safely feature-detect.
#[napi]
pub fn native_abi_version() -> u32 {
    1
}

/// Build identity for the opt-in CDP HTTP discovery and WebSocket upgrade headers.
#[napi]
pub fn cdp_handshake_headers_enabled() -> bool {
    cfg!(feature = "cdp-handshake-headers")
}

/// Build identity for Playwright-compatible default CDP transport User-Agent.
#[napi]
pub fn cdp_default_user_agent_enabled() -> bool {
    cfg!(feature = "cdp-default-user-agent")
}

/// Profile-local ABI proof for the bounded runtime-identity tuple argument.
#[cfg(feature = "cdp-default-user-agent")]
#[napi]
pub fn cdp_default_user_agent_abi_shape() -> u32 {
    2
}

/// Build identity for Playwright-compatible CDP HTTP discovery errors.
#[napi]
pub fn cdp_discovery_errors_enabled() -> bool {
    cfg!(feature = "cdp-discovery-errors")
}

/// Build identity for peer WebSocket close reasons in CDP attach errors.
#[napi]
pub fn cdp_ws_close_reason_enabled() -> bool {
    cfg!(feature = "cdp-ws-close-reason")
}

/// Build identity only; fresh Chromium process ownership remains opt-in.
#[napi]
pub fn owned_launch_enabled() -> bool {
    cfg!(feature = "owned-launch")
}

/// Build identity for the selected trace-directory path boundary only.
/// This does not claim Context recording, ZIP export, or snapshot support.
#[napi]
pub fn playwright_trace_dir_enabled() -> bool {
    cfg!(feature = "playwright-trace")
}

/// Build identity for the opt-in, failure-only BiDi-configured startup probe.
/// It never publishes a Browser or establishes a BiDi protocol session.
#[napi(js_name = "bidiStartupFailureEnabled")]
pub fn bidi_startup_failure_enabled() -> bool {
    cfg!(feature = "bidi-startup-failure")
}

/// Build identity only; browser executable metadata does not install or launch.
#[napi]
pub fn browser_registry_enabled() -> bool {
    cfg!(feature = "browser-registry")
}

/// Build identity for the narrow Playwright Server WebSocket error path.
/// A successful remote connection never produces a Browser session.
#[napi]
pub fn playwright_server_connect_ws_error_enabled() -> bool {
    cfg!(feature = "playwright-server-connect-ws-error")
}

/// Build identity for the distinct Root.initialize remote-Close error path.
/// A received response never produces a remote Browser session.
#[napi]
pub fn playwright_server_connect_close_error_enabled() -> bool {
    cfg!(feature = "playwright-server-connect-close-error")
}

/// Build identity for the BrowserType.connect WebSocket request-header slice.
/// The upgraded connection never produces a remote Browser session.
#[napi]
pub fn playwright_server_connect_headers_enabled() -> bool {
    cfg!(feature = "playwright-server-connect-headers")
}

/// Profile-local ABI proof for the bounded runtime-identity tuple argument.
/// Older header addons exported the same port name with a different arity.
#[cfg(feature = "playwright-server-connect-headers")]
#[napi]
pub fn playwright_server_connect_headers_abi_shape() -> u32 {
    2
}

/// A separately selected successful remote Browser transport and ownership path.
#[napi]
pub fn playwright_server_connect_session_enabled() -> bool {
    cfg!(feature = "playwright-server-connect-session")
}

/// A separate, default-off remote Page notice/subscription composition.
#[napi]
pub fn playwright_server_connect_page_events_enabled() -> bool {
    cfg!(feature = "playwright-server-connect-page-events")
}

/// Profile-local shape proof for the separate connect port and typed notices.
#[cfg(feature = "playwright-server-connect-page-events")]
#[napi]
pub fn playwright_server_connect_page_events_abi_shape() -> u32 {
    1
}

/// Profile-local shape proof for the connect port and Browser/Context/Page leases.
#[cfg(feature = "playwright-server-connect-session")]
#[napi]
pub fn playwright_server_connect_session_abi_shape() -> u32 {
    2
}

/// Build identity only; persistent-Context validation never starts a browser.
#[napi]
pub fn persistent_context_preflight_enabled() -> bool {
    cfg!(feature = "persistent-context-preflight")
}

/// Build identity for the default-Context locale applied during an owned
/// persistent Chromium launch.
#[napi]
pub fn persistent_context_locale_enabled() -> bool {
    cfg!(feature = "persistent-context-locale")
}

/// Build identity for the default-Context timezone applied during an owned
/// persistent Chromium launch.
#[napi]
pub fn persistent_context_timezone_enabled() -> bool {
    cfg!(feature = "persistent-context-timezone")
}

/// Build identity for the default-Context geolocation applied during an owned
/// persistent Chromium launch.
#[napi]
pub fn persistent_context_geolocation_enabled() -> bool {
    cfg!(feature = "persistent-context-geolocation")
}

/// Build identity for permissions installed on the default Context of an
/// owned persistent Chromium launch.
#[napi]
pub fn persistent_context_geolocation_permission_enabled() -> bool {
    cfg!(feature = "persistent-context-geolocation-permission")
}

/// Build identity for HTTP credentials installed on the default Context of an
/// owned persistent Chromium launch.
#[napi]
pub fn persistent_context_http_credentials_enabled() -> bool {
    cfg!(feature = "persistent-context-http-credentials")
}

/// Build identity for default-Context offline emulation during an owned
/// persistent Chromium launch.
#[napi]
pub fn persistent_context_offline_enabled() -> bool {
    cfg!(feature = "persistent-context-offline")
}

/// Build identity for the default-Context CSP bypass policy during an owned
/// persistent Chromium launch.
#[napi]
pub fn persistent_context_bypass_csp_enabled() -> bool {
    cfg!(feature = "persistent-context-bypass-csp")
}

/// Build identity for disabling JavaScript in the default Context of an owned
/// persistent Chromium launch.
#[napi]
pub fn persistent_context_javascript_disabled_enabled() -> bool {
    cfg!(feature = "persistent-context-javascript-disabled")
}

/// Build identity only; no session, task, transport or allocation is created.
#[napi]
pub fn drag_runtime_enabled() -> bool {
    cfg!(feature = "drag-runtime")
}

/// Build identity only; Frame navigation remains an explicitly selected leaf.
#[napi]
pub fn frame_navigation_enabled() -> bool {
    cfg!(feature = "frame-navigation")
}

/// Build identity only; the 20-wait opt-in must never accept an ordinary
/// Frame-navigation binary with a four-registration Page profile.
#[napi(js_name = "frameNavigationBurst20Enabled")]
pub fn frame_navigation_burst_20_enabled() -> bool {
    cfg!(feature = "frame-navigation-burst-20")
}

/// Build identity only; browser-history traversal remains independently selected.
#[napi]
pub fn history_navigation_enabled() -> bool {
    cfg!(feature = "history-navigation")
}

/// Build identity only; Page document replacement remains an explicitly
/// selected leaf.
#[napi]
pub fn set_content_enabled() -> bool {
    cfg!(feature = "set-content")
}

/// Build identity only; event observation remains an explicitly selected leaf.
#[napi]
pub fn page_events_enabled() -> bool {
    cfg!(feature = "page-events")
}

/// Build identity only; child Frame attach/navigation publication is opt-in.
#[napi(js_name = "frameEventsEnabled")]
pub fn frame_events_enabled() -> bool {
    cfg!(feature = "frame-events")
}

#[napi(js_name = "frameDetachEventsEnabled")]
pub fn frame_detach_events_enabled() -> bool {
    cfg!(feature = "frame-detach-events")
}

/// Build identity only; Page dialog publication/actions remain opt-in.
#[napi]
pub fn page_dialogs_enabled() -> bool {
    cfg!(feature = "page-dialogs")
}

/// Build identity only; Page request interception remains opt-in.
#[napi]
pub fn page_routes_enabled() -> bool {
    cfg!(feature = "page-routes")
}

/// Build identity for Rust-owned Page and BrowserContext HAR replay.
#[napi]
pub fn har_replay_enabled() -> bool {
    cfg!(feature = "har-replay")
}

/// Build identity for Context-owned HAR recording and its exact FFI port.
#[napi]
pub fn har_recording_enabled() -> bool {
    cfg!(feature = "har-recording")
}

/// Build identity for the independently selectable HAR RegExp matcher.
#[napi(js_name = "harRecordingRegExpEnabled")]
pub fn har_recording_reg_exp_enabled() -> bool {
    cfg!(feature = "har-recording-regexp")
}

/// Build identity for the bounded, opt-in RegExp Page.route matcher.
#[napi(js_name = "routeRegExpEnabled")]
pub fn route_reg_exp_enabled() -> bool {
    cfg!(feature = "route-regexp")
}

/// Build identity for the opt-in, registration-ID-owned Page.unroute port.
#[napi]
pub fn page_unroute_enabled() -> bool {
    cfg!(feature = "page-unroute")
}

/// Build identity for the opt-in utility-world direct-fill commit receipt.
/// The ordinary v1 fill path does not install a CDP binding.
#[napi]
pub fn fill_navigation_commit_enabled() -> bool {
    cfg!(feature = "fill-navigation-commit")
}

/// Build identity for the opt-in utility-world select-option commit receipt.
#[napi]
pub fn select_option_navigation_commit_enabled() -> bool {
    cfg!(feature = "select-option-navigation-commit")
}

/// Build identity for the opt-in per-call select-option timeout progress.
#[napi]
pub fn select_option_timeout_progress_enabled() -> bool {
    cfg!(feature = "select-option-timeout-progress")
}

/// Build identity for the opt-in React selector engine.
#[napi]
pub fn react_selectors_enabled() -> bool {
    cfg!(feature = "react-selectors")
}

/// Build identity for the opt-in non-piercing attribute selector engine.
#[napi]
pub fn light_selectors_enabled() -> bool {
    cfg!(feature = "light-selectors")
}

/// Build identity for the narrow Route header/URL override conversion port.
#[napi]
pub fn route_header_url_overrides_enabled() -> bool {
    cfg!(feature = "route-header-url-overrides")
}

/// Build identity for opt-in cross-origin Route.fulfill CORS response headers.
#[napi]
pub fn route_fulfill_cors_enabled() -> bool {
    cfg!(feature = "route-fulfill-cors")
}

/// Build identity only; console observation remains an explicitly selected
/// leaf.
#[napi]
pub fn console_events_enabled() -> bool {
    cfg!(feature = "console-events")
}

/// Build identity for Rust-owned BrowserContext trace console events.
#[napi]
pub fn context_tracing_console_enabled() -> bool {
    cfg!(feature = "context-tracing-console")
}

/// Build identity for Rust-owned BrowserContext trace network snapshots.
#[napi]
pub fn context_tracing_network_enabled() -> bool {
    cfg!(feature = "context-tracing-network")
}

/// Build identity for Rust-owned APIRequestContext network tracing.
#[napi]
pub fn api_request_tracing_enabled() -> bool {
    cfg!(feature = "api-request-tracing")
}

/// Build identity only; Page binding transport remains an explicitly selected
/// leaf.
#[napi]
pub fn page_bindings_enabled() -> bool {
    cfg!(feature = "page-bindings")
}

/// Build identity only; BrowserContext bindings extend the Page binding
/// transport while keeping registration state in the Rust host.
#[napi]
pub fn context_bindings_enabled() -> bool {
    cfg!(feature = "context-bindings")
}

/// Build identity only; Page init-script persistence is independently selected.
#[napi]
pub fn init_scripts_enabled() -> bool {
    cfg!(feature = "init-scripts")
}

/// Build identity only; function waiting remains an independently selected leaf.
#[napi]
pub fn wait_for_function_enabled() -> bool {
    cfg!(feature = "wait-for-function")
}

/// Build identity only; URL waiting remains an independently selected leaf.
#[napi]
pub fn wait_for_url_enabled() -> bool {
    cfg!(feature = "wait-for-url")
}

/// Build identity only; timeout waiting remains an independently selected leaf.
#[napi]
pub fn wait_for_timeout_enabled() -> bool {
    cfg!(feature = "wait-for-timeout")
}

/// Build identity only; timeout inheritance remains an independently selected leaf.
#[napi]
pub fn timeout_settings_enabled() -> bool {
    cfg!(feature = "timeout-settings")
}

/// Build identity only; Page media emulation remains an independently selected leaf.
#[napi]
pub fn emulate_media_enabled() -> bool {
    cfg!(feature = "emulate-media")
}

/// Build identity only; Context-scoped device emulation remains independently selected.
#[napi]
pub fn device_emulation_enabled() -> bool {
    cfg!(feature = "device-emulation")
}

#[napi]
pub fn touchscreen_input_enabled() -> bool {
    cfg!(feature = "touchscreen-input")
}

/// Build identity only; explicit Page GC remains an independently selected leaf.
#[napi]
pub fn request_gc_enabled() -> bool {
    cfg!(feature = "request-gc")
}

/// Build identity only; Chromium PDF export remains an independently selected leaf.
#[napi]
pub fn page_pdf_enabled() -> bool {
    cfg!(feature = "page-pdf")
}

/// Build identity only; navigation response bodies remain independently selected.
#[napi]
pub fn response_body_enabled() -> bool {
    cfg!(feature = "response-body")
}

/// Build identity only; navigation Response request snapshots remain
/// independently selected from routing and public network event streams.
#[napi]
pub fn response_request_enabled() -> bool {
    cfg!(feature = "response-request")
}

/// Build identity only; bounded Page request notices remain opt-in.
#[napi]
pub fn request_events_enabled() -> bool {
    cfg!(feature = "request-events")
}

/// Build identity only; Request.frame ownership checks remain opt-in.
#[napi]
pub fn request_frame_enabled() -> bool {
    cfg!(feature = "request-frame")
}

/// Build identity only; bounded Page response notices remain opt-in.
#[napi]
pub fn response_events_enabled() -> bool {
    cfg!(feature = "response-events")
}

/// Build identity for Context-owned Chromium Service Worker publication.
#[napi]
pub fn service_worker_events_enabled() -> bool {
    cfg!(feature = "service-worker-events")
}

/// Build identity for Service Worker requests published on BrowserContext.
#[napi]
pub fn worker_network_events_enabled() -> bool {
    cfg!(feature = "worker-network-events")
}

/// Build identity only; terminal response observation remains independently
/// selectable from response publication and body retrieval.
#[napi]
pub fn response_finished_enabled() -> bool {
    cfg!(feature = "response-finished")
}

/// Build identity only; Rust-owned Request post-data decoding remains opt-in.
#[napi]
pub fn request_post_data_json_enabled() -> bool {
    cfg!(feature = "request-post-data-json")
}

/// Build identity only; browser-independent API request remains opt-in.
#[napi]
pub fn api_request_enabled() -> bool {
    cfg!(feature = "api-request")
}

/// Build identity only; client-certificate parsing and TLS selection remain opt-in.
#[napi]
pub fn api_request_client_certificates_enabled() -> bool {
    cfg!(feature = "api-request-client-certificates")
}

/// Build identity only; HTTP CONNECT proxying remains independently selectable.
#[napi]
pub fn api_request_http_proxy_enabled() -> bool {
    cfg!(feature = "api-request-http-proxy")
}

/// Build identity only; SOCKS5 proxying remains independently selectable.
#[napi]
pub fn api_request_socks5_proxy_enabled() -> bool {
    cfg!(feature = "api-request-socks5-proxy")
}

/// Build identity only; proxy bypass selection remains independently selectable.
#[napi]
pub fn api_request_proxy_bypass_enabled() -> bool {
    cfg!(feature = "api-request-proxy-bypass")
}

/// Build identity only; BrowserContext.request remains independently selectable.
#[napi]
pub fn context_api_request_enabled() -> bool {
    cfg!(feature = "context-api-request")
}

/// Build identity for independently selected, Rust-owned Context cookies.
#[napi]
pub fn context_cookies_enabled() -> bool {
    cfg!(feature = "context-cookies")
}

/// Build identity for bounded BrowserContext storage-state collection.
#[napi]
pub fn context_storage_state_enabled() -> bool {
    cfg!(feature = "context-storage-state")
}

/// Build identity only; large evaluation input copies remain opt-in.
#[napi(js_name = "largeEvaluationInputsEnabled")]
pub fn large_evaluation_inputs_enabled() -> bool {
    cfg!(feature = "large-evaluation-inputs")
}

/// Build identity only; bounded large Route request bodies remain opt-in.
#[napi(js_name = "largeRoutePostDataEnabled")]
pub fn large_route_post_data_enabled() -> bool {
    cfg!(feature = "large-route-post-data")
}

/// Build identity only; BrowserContext proxy ownership remains independently selectable.
#[napi]
pub fn context_proxy_enabled() -> bool {
    cfg!(feature = "context-proxy")
}

/// Build identity only; Context offline emulation remains independently selectable.
#[napi]
pub fn context_offline_enabled() -> bool {
    cfg!(feature = "context-offline")
}

/// Build identity only; Context locale emulation remains independently selectable.
#[napi]
pub fn context_locale_enabled() -> bool {
    cfg!(feature = "context-locale")
}

/// Build identity only; Context timezone emulation remains independently selectable.
#[napi]
pub fn context_timezone_enabled() -> bool {
    cfg!(feature = "context-timezone")
}

/// Build identity only; Context geolocation remains independently selectable.
#[napi]
pub fn context_geolocation_enabled() -> bool {
    cfg!(feature = "context-geolocation")
}

/// Build identity for Chromium BrowserContext permission overrides.
#[napi]
pub fn context_geolocation_permission_enabled() -> bool {
    cfg!(feature = "context-geolocation-permission")
}

/// Build identity for Chromium BrowserContext HTTP authentication.
#[napi]
pub fn context_http_credentials_enabled() -> bool {
    cfg!(feature = "context-http-credentials")
}

/// Build identity only; Context certificate-error policy remains independently selectable.
#[napi]
pub fn ignore_https_errors_enabled() -> bool {
    cfg!(feature = "ignore-https-errors")
}

/// Build identity only; Context CSP bypass remains independently selectable.
#[napi]
pub fn bypass_csp_enabled() -> bool {
    cfg!(feature = "bypass-csp")
}

/// Build identity only; Locator geometry remains independently selected.
#[napi]
pub fn locator_bounding_box_enabled() -> bool {
    cfg!(feature = "locator-bounding-box")
}

/// Build identity only; the browser highlight overlay remains independently selected.
#[napi]
pub fn locator_highlight_enabled() -> bool {
    cfg!(feature = "locator-highlight")
}

/// Build identity only; Locator capture remains independently selected.
#[napi]
pub fn locator_screenshot_enabled() -> bool {
    cfg!(feature = "locator-screenshot")
}

/// Build identity only; static Rust-owned Locator DOM actions remain independently selected.
#[napi]
pub fn locator_dom_actions_enabled() -> bool {
    cfg!(feature = "locator-dom-actions")
}

/// Build identity only; Page-scoped viewport overrides remain independently selected.
#[napi]
pub fn page_viewport_enabled() -> bool {
    cfg!(feature = "page-viewport")
}

/// Build identity only; custom selector registration remains independently selected.
#[napi]
pub fn selector_registration_enabled() -> bool {
    cfg!(feature = "selector-registration")
}

/// Build identity only; bounded chunked string results are opt-in.
#[napi]
pub fn large_evaluation_results_enabled() -> bool {
    cfg!(feature = "large-evaluation-results")
}

#[napi]
pub fn extra_headers_enabled() -> bool {
    cfg!(feature = "extra-headers")
}

#[napi]
pub fn concurrent_close_enabled() -> bool {
    cfg!(feature = "concurrent-close")
}

#[napi]
pub fn file_input_enabled() -> bool {
    cfg!(feature = "file-input")
}

#[napi]
pub fn handles_enabled() -> bool {
    cfg!(feature = "handles")
}

/// Build identity only; ElementHandle DOM reads remain an explicitly selected leaf.
#[napi]
pub fn element_handle_dom_enabled() -> bool {
    cfg!(feature = "element-handle-dom")
}

/// Build identity for the exact-root ElementHandle selector-wait port.
#[napi]
pub fn element_handle_wait_for_selector_enabled() -> bool {
    cfg!(feature = "element-handle-wait-for-selector")
}

/// Build identity for the retained value-string selectOption subset.
#[napi]
pub fn element_handle_select_option_value_enabled() -> bool {
    cfg!(feature = "element-handle-select-option-value")
}

/// Build identity only; retained ElementHandle scrolling is an independent leaf.
#[napi]
pub fn element_handle_scroll_into_view_enabled() -> bool {
    cfg!(feature = "element-handle-scroll-into-view")
}

/// Build identity for the optional inherited-timeout Node-API argument shape.
#[napi]
pub fn element_handle_scroll_timeout_inheritance_enabled() -> bool {
    cfg!(all(
        feature = "element-handle-scroll-into-view",
        feature = "timeout-settings"
    ))
}

#[napi]
pub fn retained_file_input_enabled() -> bool {
    cfg!(feature = "retained-file-input")
}

/// Build identity only; actual target marking remains an explicitly selected leaf.
#[napi]
pub fn retained_target_marking_enabled() -> bool {
    cfg!(feature = "retained-target-marking")
}

/// Build identity only; retained DOM prevalidation is independently selected.
#[napi]
pub fn retained_target_prevalidation_enabled() -> bool {
    cfg!(feature = "retained-target-prevalidation")
}

/// Build identity only; the awaited input-action boundary is independently selected.
#[napi]
pub fn retained_input_instrumentation_enabled() -> bool {
    cfg!(feature = "retained-input-instrumentation")
}

/// Build identity only; late call-argument serialization is independently selected.
#[napi]
pub fn retained_call_argument_serialization_enabled() -> bool {
    cfg!(feature = "retained-call-argument-serialization")
}

/// Build identity only; Rust graph import is independently selected.
#[napi]
pub fn retained_serialized_payload_import_enabled() -> bool {
    cfg!(feature = "retained-serialized-payload-import")
}

/// Build identity only; retained graph mutation is independently selected.
#[napi]
pub fn retained_serialized_payload_mutation_enabled() -> bool {
    cfg!(feature = "retained-serialized-payload-mutation")
}

fn binding_error(error: HostError) -> napi::Error {
    napi::Error::new(
        napi::Status::GenericFailure,
        format!("{}: {}", error.code(), error),
    )
}

#[napi(object, object_from_js = false)]
pub struct ContextDescriptor {
    pub id: String,
    pub settings: context_settings::NativeContextSettings,
}

#[napi(object, object_from_js = false)]
pub struct PageDescriptor {
    pub id: String,
    pub context_id: String,
    pub main_frame_id: String,
    pub frame_tree: frame_tree::NativeFrameTree,
    pub main_frame_binding: Option<frame_tree::NativeFrameNode>,
}

#[napi(object, object_from_js = false)]
pub struct BrowserPageDescriptor {
    pub context: ContextDescriptor,
    pub page: PageDescriptor,
}

impl TryFrom<playwright_rust_node_host::BrowserPageDescriptor> for BrowserPageDescriptor {
    type Error = HostError;

    fn try_from(
        value: playwright_rust_node_host::BrowserPageDescriptor,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            context: ContextDescriptor::try_from(value.context)?,
            page: PageDescriptor::from(value.page),
        })
    }
}

impl From<playwright_rust_node_host::PageDescriptor> for PageDescriptor {
    fn from(value: playwright_rust_node_host::PageDescriptor) -> Self {
        Self {
            id: value.id,
            context_id: value.context_id,
            main_frame_id: value.main_frame_id,
            frame_tree: frame_tree::NativeFrameTree {
                binding: value.frame_tree_binding,
            },
            main_frame_binding: value
                .main_frame_binding
                .map(|binding| frame_tree::NativeFrameNode { binding: *binding }),
        }
    }
}

#[napi(object)]
pub struct FrameMetadataMetrics {
    pub entries: u32,
    pub url_bytes: u32,
    pub name_bytes: u32,
    pub active_frames: u32,
    pub pending_publications: u32,
}

#[napi(object)]
pub struct FrameTreeMetrics {
    pub states: u32,
    pub revisions: u32,
    pub bytes: u32,
}

#[napi(object)]
pub struct SessionMetrics {
    pub connected: bool,
    pub live_objects: u32,
    pub pending_calls: u32,
    pub topology_drivers: u32,
    pub active_leases: u32,
    pub live_scopes: u32,
    pub retained_event_bytes: u32,
    pub active_operations: u32,
    pub cleanup_in_flight: u32,
}

#[cfg(not(feature = "response-finished"))]
#[napi(object)]
pub struct NavigationResponse {
    pub request_id: Option<String>,
    pub request: Option<NativeNavigationRequest>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub from_service_worker: bool,
    pub url: String,
    pub status: i64,
    pub status_text: String,
    pub ok: bool,
    pub headers: HashMap<String, String>,
}

#[cfg(feature = "response-finished")]
#[napi(object, object_from_js = false)]
pub struct NavigationResponse {
    pub request_id: Option<String>,
    pub request: Option<NativeNavigationRequest>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub from_service_worker: bool,
    pub url: String,
    pub status: i64,
    pub status_text: String,
    pub ok: bool,
    pub headers: HashMap<String, String>,
    pub finished_owner:
        Option<External<Arc<playwright_rust_node_host::network_events::ResponseFinishedOwner>>>,
}

#[napi(object)]
pub struct NativeNavigationRequestHeader {
    pub name: String,
    pub value: String,
}

#[cfg(feature = "worker-events")]
#[napi(object, object_from_js = false)]
pub struct NativeWorkerDescriptor {
    pub id: String,
    pub url: String,
    pub owner: External<Arc<playwright_rust_node_host::worker_events::WorkerOwner>>,
}

#[cfg(feature = "service-worker-events")]
#[napi(object, object_from_js = false)]
pub struct NativeServiceWorkerNotice {
    pub kind: String,
    pub context_id: String,
    pub worker: NativeWorkerDescriptor,
}

#[cfg(feature = "worker-events")]
fn native_worker_descriptor(
    owner: Arc<playwright_rust_node_host::worker_events::WorkerOwner>,
) -> NativeWorkerDescriptor {
    NativeWorkerDescriptor {
        id: owner.id().to_owned(),
        url: owner.url().to_owned(),
        owner: External::new(owner),
    }
}

#[cfg(feature = "service-worker-events")]
struct HostServiceWorkerNotice(playwright_rust_node_host::worker_events::ServiceWorkerNotice);

#[cfg(feature = "service-worker-events")]
impl ToNapiValue for HostServiceWorkerNotice {
    unsafe fn to_napi_value(
        env: napi_runtime::sys::napi_env,
        value: Self,
    ) -> napi::Result<napi_runtime::sys::napi_value> {
        let notice = value.0;
        let result = NativeServiceWorkerNotice {
            kind: match notice.kind {
                playwright_rust_node_host::worker_events::WorkerNoticeKind::Created => {
                    "created".to_owned()
                }
                playwright_rust_node_host::worker_events::WorkerNoticeKind::Closed => {
                    "closed".to_owned()
                }
            },
            context_id: notice.context.get().to_string(),
            worker: native_worker_descriptor(notice.owner),
        };
        unsafe { NativeServiceWorkerNotice::to_napi_value(env, result) }
    }
}

#[cfg(not(feature = "request-post-data-json"))]
#[napi(object)]
pub struct NativeNavigationRequest {
    pub id: String,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub headers: Vec<NativeNavigationRequestHeader>,
    pub post_data: Option<Buffer>,
    pub is_navigation_request: bool,
    pub frame_id: Option<String>,
    pub frame_available: bool,
    pub redirected_from: Option<String>,
}

// napi-derive expands object fields before rustc applies field-level cfg. Keep
// the parser-selected shape as a complete alternative so request-events alone
// never resolves or links serde_json.
#[cfg(feature = "request-post-data-json")]
#[napi(object)]
pub struct NativeNavigationRequest {
    pub id: String,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub headers: Vec<NativeNavigationRequestHeader>,
    pub post_data: Option<Buffer>,
    pub is_navigation_request: bool,
    pub frame_id: Option<String>,
    pub frame_available: bool,
    pub redirected_from: Option<String>,
    pub post_data_json_available: bool,
    pub post_data_json: Option<serde_json::Value>,
    pub post_data_json_error: Option<String>,
}

#[cfg(any(
    feature = "response-request",
    feature = "request-events",
    feature = "request-history"
))]
impl From<&playwright_rust_node_host::network_events::NetworkRequestRecord>
    for NativeNavigationRequest
{
    fn from(value: &playwright_rust_node_host::network_events::NetworkRequestRecord) -> Self {
        #[cfg(feature = "request-post-data-json")]
        let (post_data_json, post_data_json_error) =
            match playwright_rust_node_host::network_events::request_post_data_json(value) {
                Ok(value) => (value, None),
                Err(error) => (None, Some(error)),
            };
        Self {
            id: value.id.clone(),
            context_id: value.context_id.clone(),
            service_worker_id: value.service_worker_id.clone(),
            url: value.url.clone(),
            method: value.method.clone(),
            resource_type: value.resource_type.to_ascii_lowercase(),
            headers: value
                .headers
                .iter()
                .map(|(name, value)| NativeNavigationRequestHeader {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
            post_data: value.post_data.clone().map(Into::into),
            is_navigation_request: value.is_navigation_request,
            frame_id: value
                .frame_available
                .then(|| value.frame_id.clone())
                .flatten(),
            frame_available: value.frame_available,
            redirected_from: value.redirected_from.clone(),
            #[cfg(feature = "request-post-data-json")]
            post_data_json_available: true,
            #[cfg(feature = "request-post-data-json")]
            post_data_json,
            #[cfg(feature = "request-post-data-json")]
            post_data_json_error,
        }
    }
}

#[cfg(all(feature = "request-events", not(feature = "request-history")))]
#[napi(object, object_from_js = false)]
pub struct NativeRequestNotice {
    pub sequence: u64,
    pub page_id: Option<String>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub kind: String,
    pub request: NativeNavigationRequest,
    pub failure_text: Option<String>,
}

#[cfg(all(feature = "request-events", feature = "request-history"))]
#[napi(object, object_from_js = false)]
pub struct NativeRequestNotice {
    pub sequence: u64,
    pub page_id: Option<String>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub kind: String,
    pub request: NativeNavigationRequest,
    pub failure_text: Option<String>,
    pub history_owner:
        Option<External<Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>>>,
}

#[cfg(feature = "request-events")]
pub(crate) struct HostRequestNotice(pub playwright_rust_node_host::network_events::RequestNotice);

#[cfg(feature = "request-events")]
impl HostRequestNotice {
    fn foreign_copy_bytes(&self) -> u64 {
        self.0.foreign_copy_bytes()
    }
}

#[cfg(feature = "request-events")]
impl ToNapiValue for HostRequestNotice {
    unsafe fn to_napi_value(
        env: napi_runtime::sys::napi_env,
        value: Self,
    ) -> napi::Result<napi_runtime::sys::napi_value> {
        let result = native_request_notice(value.0);
        // SAFETY: napi invokes this inside the live settlement callback after
        // the request-notice foreign-copy allowance has been reserved.
        unsafe { NativeRequestNotice::to_napi_value(env, result) }
    }
}

#[cfg(feature = "request-events")]
fn native_request_notice(
    notice: playwright_rust_node_host::network_events::RequestNotice,
) -> NativeRequestNotice {
    let service_worker_id = notice.request.service_worker_id.clone();
    NativeRequestNotice {
        sequence: notice.sequence,
        page_id: service_worker_id
            .is_none()
            .then(|| notice.page.get().to_string()),
        context_id: notice.request.context_id.clone(),
        service_worker_id,
        kind: notice.kind.as_str().to_owned(),
        request: NativeNavigationRequest::from(&notice.request),
        failure_text: notice.failure_text,
        #[cfg(feature = "request-history")]
        history_owner: notice.history_owner.map(External::new),
    }
}

#[cfg(all(
    any(feature = "response-events", feature = "request-history"),
    not(feature = "response-finished"),
    not(feature = "request-history")
))]
#[napi(object, object_from_js = false)]
pub struct NativeResponseNotice {
    pub sequence: u64,
    pub page_id: Option<String>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub native_request_id: Option<String>,
    pub response: NavigationResponse,
}

#[cfg(all(
    any(feature = "response-events", feature = "request-history"),
    not(feature = "response-finished"),
    feature = "request-history"
))]
#[napi(object, object_from_js = false)]
pub struct NativeResponseNotice {
    pub sequence: u64,
    pub page_id: Option<String>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub native_request_id: Option<String>,
    pub response: NavigationResponse,
    pub history_owner:
        Option<External<Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>>>,
}

#[cfg(all(
    any(feature = "response-events", feature = "request-history"),
    feature = "response-finished",
    not(feature = "request-history")
))]
#[napi(object, object_from_js = false)]
pub struct NativeResponseNotice {
    pub sequence: u64,
    pub page_id: Option<String>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub native_request_id: Option<String>,
    pub response: NavigationResponse,
    pub finished_owner:
        Option<External<Arc<playwright_rust_node_host::network_events::ResponseFinishedOwner>>>,
}

#[cfg(all(
    any(feature = "response-events", feature = "request-history"),
    feature = "response-finished",
    feature = "request-history"
))]
#[napi(object, object_from_js = false)]
pub struct NativeResponseNotice {
    pub sequence: u64,
    pub page_id: Option<String>,
    pub context_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub native_request_id: Option<String>,
    pub response: NavigationResponse,
    pub finished_owner:
        Option<External<Arc<playwright_rust_node_host::network_events::ResponseFinishedOwner>>>,
    pub history_owner:
        Option<External<Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>>>,
}

#[cfg(feature = "response-events")]
pub(crate) struct HostResponseNotice(pub playwright_rust_node_host::network_events::ResponseNotice);

#[cfg(feature = "response-events")]
impl HostResponseNotice {
    fn foreign_copy_bytes(&self) -> u64 {
        self.0.foreign_copy_bytes()
    }
}

#[cfg(feature = "response-events")]
impl ToNapiValue for HostResponseNotice {
    unsafe fn to_napi_value(
        env: napi_runtime::sys::napi_env,
        value: Self,
    ) -> napi::Result<napi_runtime::sys::napi_value> {
        let notice = value.0;
        let result = native_response_notice(
            notice.sequence,
            notice.page.get().to_string(),
            notice.native_request_id,
            notice.response,
            #[cfg(feature = "request-history")]
            notice.history_owner,
        );
        // SAFETY: napi invokes this inside the live settlement callback after
        // the response-notice foreign-copy allowance has been reserved.
        unsafe { NativeResponseNotice::to_napi_value(env, result) }
    }
}

#[cfg(any(feature = "response-events", feature = "request-history"))]
fn native_response_notice(
    sequence: u64,
    owner_id: String,
    native_request_id: Option<String>,
    response: playwright_rust_node_host::network_events::NetworkResponseRecord,
    #[cfg(feature = "request-history")] history_owner: Option<
        Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>,
    >,
) -> NativeResponseNotice {
    let context_id = response.context_id.clone();
    let service_worker_id = response.service_worker_id.clone();
    let page_id = service_worker_id.is_none().then_some(owner_id);
    #[cfg(feature = "request-history")]
    let request = history_owner
        .as_deref()
        .map(|owner| NativeNavigationRequest::from(owner.request()));
    #[cfg(not(feature = "request-history"))]
    let request = None;
    #[cfg(feature = "response-finished")]
    let finished_owner = response
        .finished_owner
        .as_ref()
        .map(|owner| External::new(Arc::clone(owner)));
    NativeResponseNotice {
        sequence,
        page_id,
        context_id: context_id.clone(),
        service_worker_id: service_worker_id.clone(),
        native_request_id,
        response: NavigationResponse {
            request_id: Some(response.request_id),
            request,
            context_id,
            service_worker_id,
            from_service_worker: response.from_service_worker,
            url: response.url,
            status: response.status,
            status_text: response.status_text,
            ok: response.status == 0 || (200..=299).contains(&response.status),
            headers: response.headers.into_iter().collect(),
            #[cfg(feature = "response-finished")]
            finished_owner: response
                .finished_owner
                .as_ref()
                .map(|owner| External::new(Arc::clone(owner))),
        },
        #[cfg(feature = "response-finished")]
        finished_owner,
        #[cfg(feature = "request-history")]
        history_owner: history_owner.map(External::new),
    }
}

#[cfg(all(feature = "request-events", feature = "response-events"))]
#[napi(object, object_from_js = false)]
pub struct NativeNetworkNotice {
    pub kind: String,
    pub request_notice: Option<NativeRequestNotice>,
    pub response_notice: Option<NativeResponseNotice>,
}

#[cfg(all(feature = "request-events", feature = "response-events"))]
pub(crate) struct HostNetworkNotice(pub playwright_rust_node_host::network_events::NetworkNotice);

#[cfg(all(feature = "request-events", feature = "response-events"))]
impl HostNetworkNotice {
    fn foreign_copy_bytes(&self) -> u64 {
        match &self.0 {
            playwright_rust_node_host::network_events::NetworkNotice::Request(notice) => {
                notice.foreign_copy_bytes()
            }
            playwright_rust_node_host::network_events::NetworkNotice::Response(notice) => {
                notice.foreign_copy_bytes()
            }
        }
    }
}

#[cfg(all(feature = "request-events", feature = "response-events"))]
impl ToNapiValue for HostNetworkNotice {
    unsafe fn to_napi_value(
        env: napi_runtime::sys::napi_env,
        value: Self,
    ) -> napi::Result<napi_runtime::sys::napi_value> {
        let result = match value.0 {
            playwright_rust_node_host::network_events::NetworkNotice::Request(notice) => {
                NativeNetworkNotice {
                    kind: "request".to_owned(),
                    request_notice: Some(native_request_notice(notice)),
                    response_notice: None,
                }
            }
            playwright_rust_node_host::network_events::NetworkNotice::Response(notice) => {
                NativeNetworkNotice {
                    kind: "response".to_owned(),
                    request_notice: None,
                    response_notice: Some(native_response_notice(
                        notice.sequence,
                        notice.page.get().to_string(),
                        notice.native_request_id,
                        notice.response,
                        #[cfg(feature = "request-history")]
                        notice.history_owner,
                    )),
                }
            }
        };
        // SAFETY: the bounded notice is converted inside the admitted N-API
        // settlement callback, while its foreign-copy reservation is held.
        unsafe { NativeNetworkNotice::to_napi_value(env, result) }
    }
}

#[cfg(feature = "request-history")]
struct HostRequestResponse {
    owner: Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>,
    response: Option<playwright_rust_node_host::network_events::NetworkResponseRecord>,
}

#[cfg(feature = "request-history")]
#[napi(object, object_from_js = false)]
pub struct NativeRequestPredecessor {
    pub request: NativeNavigationRequest,
    pub history_owner:
        External<Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>>,
}

#[cfg(feature = "request-history")]
impl HostRequestResponse {
    fn foreign_copy_bytes(&self) -> u64 {
        self.response.as_ref().map_or(0, |response| {
            self.owner
                .foreign_copy_bytes()
                .saturating_add(response.foreign_copy_bytes())
                .saturating_add(4 * 1024)
        })
    }
}

#[cfg(feature = "request-history")]
impl ToNapiValue for HostRequestResponse {
    unsafe fn to_napi_value(
        env: napi_runtime::sys::napi_env,
        value: Self,
    ) -> napi::Result<napi_runtime::sys::napi_value> {
        let response = value.response.map(|response| {
            native_response_notice(
                0,
                value.owner.request().page_id.clone(),
                value.owner.request().native_request_id.clone(),
                response,
                Some(value.owner),
            )
        });
        // SAFETY: the bounded owner/response copy is converted while the
        // admitted FFI ticket and additional result reservation remain held.
        unsafe { Option::<NativeResponseNotice>::to_napi_value(env, response) }
    }
}

impl From<playwright_rust_node_host::ResponseMetadata> for NavigationResponse {
    fn from(value: playwright_rust_node_host::ResponseMetadata) -> Self {
        #[cfg(feature = "response-request")]
        let request = value
            .request
            .as_deref()
            .map(|request| NativeNavigationRequest::from(request.request()));
        #[cfg(not(feature = "response-request"))]
        let request = None;
        Self {
            request_id: value.request_id,
            request,
            context_id: None,
            service_worker_id: None,
            from_service_worker: false,
            url: value.url,
            status: value.status,
            status_text: value.status_text,
            ok: value.ok,
            headers: value.headers.into_iter().collect(),
            #[cfg(feature = "response-finished")]
            finished_owner: None,
        }
    }
}

#[napi(object, object_from_js = false)]
pub struct NavigationResult {
    pub url: String,
    pub response: Option<NavigationResponse>,
}

impl From<playwright_rust_node_host::NavigationResult> for NavigationResult {
    fn from(value: playwright_rust_node_host::NavigationResult) -> Self {
        Self {
            url: value.url,
            response: value.response.map(NavigationResponse::from),
        }
    }
}

pub(crate) struct HostNavigationOutput(pub playwright_rust_node_host::NavigationResult);

impl HostNavigationOutput {
    pub fn response_request_foreign_copy_bytes(&self) -> u64 {
        #[cfg(feature = "response-request")]
        {
            self.0.response_request_foreign_copy_bytes()
        }
        #[cfg(not(feature = "response-request"))]
        {
            0
        }
    }
}

impl ToNapiValue for HostNavigationOutput {
    unsafe fn to_napi_value(
        env: napi_runtime::sys::napi_env,
        value: Self,
    ) -> napi::Result<napi_runtime::sys::napi_value> {
        let result = NavigationResult::from(value.0);
        // SAFETY: napi invokes this inside the live settlement callback after
        // the response-request foreign-copy allowance has been reserved.
        unsafe { NavigationResult::to_napi_value(env, result) }
    }
}

#[cfg(feature = "page-bindings")]
#[napi(object)]
pub struct PageBindingCallDescriptor {
    pub call_id: String,
    pub context_id: String,
    pub page_id: String,
    pub frame_id: String,
    pub name: String,
    pub arguments_json: String,
    pub needs_handle: bool,
}

#[napi]
pub struct NativeSessionSignal {
    token: tokio_util::sync::CancellationToken,
    ffi: FfiBudget,
    publications: publication_port::PublicationPort,
}

#[napi]
impl NativeSessionSignal {
    #[napi]
    pub fn wait<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        // Never let an async task borrow a JS-owned native object: Worker
        // teardown can finalize that object before a pending Promise settles.
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let token = self.token.clone();
        let port = self.publications.clone();
        let publications = port.publications();
        boundary::spawn_with_env(
            env,
            execution,
            async move {
                tokio::select! {
                    biased;
                    () = token.cancelled() => Ok((false, publications.fence()?)),
                    fence = publications.next() => Ok((true, fence?)),
                }
            },
            move |env, (connected, fence)| {
                port.publish(env, fence)?;
                Ok(connected)
            },
        )
    }
}

#[napi]
pub struct NativeSession {
    #[cfg(feature = "owned-launch")]
    launch_profile: Option<environment::LaunchProfile>,
    #[cfg(feature = "retained-file-input")]
    file_input_timeout_budget: playwright_rust_node_host::handles::TimeoutPolicyBudget,
    #[cfg(feature = "retained-file-input")]
    file_input_preparations: handles::file_input::FileInputPreparations,
    #[cfg(feature = "retained-file-input")]
    file_input_scope: handles::file_input::PreparationScope,
    publications: publication_port::PublicationPort,
    host: Arc<HostSession>,
    ffi: FfiBudget,
    #[cfg(feature = "handles")]
    wait_admissions: wait_admission::WaitAdmissions,
    conversions: deferred_conversion::DeferredConversions,
    locator_conversions: locator_conversion::LocatorConversions,
    queries: playwright_rust_node_host::locator_query::QueryBudget,
    #[cfg(feature = "selector-registration")]
    selectors: playwright_rust_node_host::selector_registry::SelectorRegistry,
}

#[cfg(feature = "official-test-browser-kill")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn kill_owned_browser_for_tests<'env>(
        &self,
        env: &'env Env,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Close, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.kill_owned_browser_for_tests().await
        })
    }
}

#[cfg(feature = "context-geolocation-permission")]
fn context_permission_input_allowance(
    permissions: &[String],
    origin_bytes: usize,
) -> napi::Result<u64> {
    permissions.iter().try_fold(
        boundary::SMALL_ALLOWANCE
            .checked_add(u64::try_from(origin_bytes).map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "permission origin byte length overflow".into(),
                ))
            })?)
            .ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "permission FFI allowance overflow".into(),
                ))
            })?,
        |total, permission| {
            total
                .checked_add(u64::try_from(permission.len()).map_err(|_| {
                    binding_error(HostError::ResourceExhausted(
                        "permission name byte length overflow".into(),
                    ))
                })?)
                .ok_or_else(|| {
                    binding_error(HostError::ResourceExhausted(
                        "permission FFI allowance overflow".into(),
                    ))
                })
        },
    )
}

#[cfg(feature = "context-geolocation-permission")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn grant_context_permissions<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'_>,
        permissions: Vec<String>,
        origin: Option<JsString<'_>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let allowance = context_permission_input_allowance(
            &permissions,
            origin
                .as_ref()
                .map(JsString::utf8_len)
                .transpose()?
                .unwrap_or(0),
        )?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let origin = origin
            .map(|value| boundary::bounded_string(value, 16 * 1024, "permission origin"))
            .transpose()?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.grant_context_permissions(&context_id, &permissions, origin.as_deref())
                .await
        })
    }

    #[napi]
    pub fn clear_context_permissions<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'_>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.clear_context_permissions(&context_id).await
        })
    }
}

#[napi(object)]
pub struct NativeContextProxy {
    pub server: String,
    pub bypass: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[cfg(feature = "context-proxy")]
impl NativeContextProxy {
    fn into_host(self) -> std::result::Result<playwright_rust_node_host::ContextProxy, HostError> {
        playwright_rust_node_host::ContextProxy::new(
            self.server,
            self.bypass,
            self.username,
            self.password,
        )
    }
}

#[cfg(feature = "context-api-request")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn context_request(
        &self,
        context_id: JsString<'_>,
    ) -> napi::Result<api_request::NativeApiRequestContext> {
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let options = self
            .host
            .context_api_request_options(&context_id)
            .map_err(binding_error)?;
        api_request::NativeApiRequestContext::for_browser_context(
            Arc::clone(&self.host),
            context_id,
            options,
        )
    }
}

#[cfg(feature = "service-worker-events")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn next_service_worker_notice<'env>(
        &self,
        env: &'env Env,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.next_service_worker_notice()
                .await
                .map(HostServiceWorkerNotice)
        })
    }

    #[napi]
    pub fn context_service_workers(
        &self,
        context_id: JsString<'_>,
    ) -> napi::Result<Vec<NativeWorkerDescriptor>> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        self.host
            .context_service_workers(&context_id)
            .map(|workers| workers.into_iter().map(native_worker_descriptor).collect())
            .map_err(binding_error)
    }
}

#[cfg(feature = "worker-events")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn worker_evaluate<'env>(
        &self,
        env: &'env Env,
        owner: &External<Arc<playwright_rust_node_host::worker_events::WorkerOwner>>,
        source: JsString<'env>,
        is_function: bool,
        argument: Unknown<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(
                FfiLane::Command,
                playwright_rust_node_host::evaluation::FFI_ALLOWANCE,
            )
            .map_err(binding_error)?;
        let source = evaluation_value::source(env, source, is_function)?;
        let input = self.conversions.prepare(env, argument)?;
        self.conversions.schedule(
            env,
            deferred_conversion::EvaluationCall {
                publications: self.publications.clone(),
                execution,
                host: Arc::clone(&self.host),
                target: deferred_conversion::EvaluationTarget::WorkerValue(Arc::clone(
                    owner.as_ref(),
                )),
                source,
                is_function,
                input,
            },
        )
    }
}

#[cfg(feature = "response-finished")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn response_finished<'env>(
        &self,
        env: &'env Env,
        owner: &External<Arc<playwright_rust_node_host::network_events::ResponseFinishedOwner>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let owner = Arc::clone(owner.as_ref());
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            owner.finished().await
        })
    }
}

#[cfg(feature = "response-events")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn next_response_notice<'env>(
        &self,
        env: &'env Env,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published_sized(
            env,
            execution,
            self.publications.clone(),
            async move { host.next_response_notice().await.map(HostResponseNotice) },
            HostResponseNotice::foreign_copy_bytes,
        )
    }
}

#[cfg(feature = "request-history")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn request_redirected_from(
        &self,
        owner: &External<Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>>,
    ) -> Option<NativeRequestPredecessor> {
        owner
            .as_ref()
            .redirected_from()
            .map(|predecessor| NativeRequestPredecessor {
                request: NativeNavigationRequest::from(predecessor.request()),
                history_owner: External::new(predecessor),
            })
    }

    #[napi]
    pub fn request_failure(
        &self,
        owner: &External<Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>>,
    ) -> napi::Result<Option<String>> {
        owner.as_ref().failure_text().map_err(binding_error)
    }

    #[napi]
    pub fn request_response<'env>(
        &self,
        env: &'env Env,
        owner: &External<Arc<playwright_rust_node_host::network_events::RequestHistoryOwner>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let owner = Arc::clone(owner.as_ref());
        boundary::spawn_published_sized(
            env,
            execution,
            self.publications.clone(),
            async move {
                let response = owner.response().await?;
                Ok(HostRequestResponse { owner, response })
            },
            HostRequestResponse::foreign_copy_bytes,
        )
    }
}

#[cfg(all(feature = "request-events", feature = "response-events"))]
#[napi]
impl NativeSession {
    #[napi]
    pub fn next_network_notice<'env>(
        &self,
        env: &'env Env,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published_sized(
            env,
            execution,
            self.publications.clone(),
            async move { host.next_network_notice().await.map(HostNetworkNotice) },
            HostNetworkNotice::foreign_copy_bytes,
        )
    }

    #[napi]
    pub fn network_notice_fence(&self) -> napi::Result<Vec<u64>> {
        let (request_sequence, response_sequence) =
            self.host.network_notice_fence().map_err(binding_error)?;
        Ok(vec![request_sequence, response_sequence])
    }
}

#[cfg(feature = "ignore-https-errors")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_ignore_https_errors<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        enabled: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_ignore_https_errors(
                    base_url.as_deref(),
                    #[cfg(feature = "device-emulation")]
                    None,
                    enabled,
                )
                .await?,
            )
        })
    }
}

#[cfg(feature = "bypass-csp")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_bypass_csp<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        enabled: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let policies =
            playwright_rust_node_host::ContextPolicyOptions::default().with_bypass_csp(enabled);
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_policies(
                    base_url.as_deref(),
                    // This independent policy supplies no device override even
                    // when device emulation is selected alongside it.
                    #[cfg(feature = "device-emulation")]
                    None,
                    policies,
                )
                .await?,
            )
        })
    }
}

#[cfg(feature = "context-offline")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_offline<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        offline: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let policies =
            playwright_rust_node_host::ContextPolicyOptions::default().with_offline(offline);
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_policies(
                    base_url.as_deref(),
                    #[cfg(feature = "device-emulation")]
                    None,
                    policies,
                )
                .await?,
            )
        })
    }

    #[napi]
    pub fn set_context_offline<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'_>,
        offline: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.set_context_offline(&context_id, offline).await
        })
    }
}

#[cfg(feature = "context-locale")]
fn context_locale_input_allowance(lengths: &[usize]) -> napi::Result<u64> {
    lengths
        .iter()
        .try_fold(boundary::SMALL_ALLOWANCE, |total, length| {
            let length = u64::try_from(*length).map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "Context locale FFI input byte length overflow".into(),
                ))
            })?;
            total.checked_add(length).ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "Context locale FFI input byte allowance overflow".into(),
                ))
            })
        })
}

#[cfg(feature = "context-geolocation")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn set_context_geolocation<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'_>,
        geolocation: Option<NativeContextGeolocation>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let geolocation = geolocation
            .map(NativeContextGeolocation::into_host)
            .transpose()
            .map_err(binding_error)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.set_context_geolocation(&context_id, geolocation).await
        })
    }
}

#[cfg(feature = "context-http-credentials")]
fn context_http_credentials_input_allowance(
    context_id_bytes: usize,
    credentials: Option<&NativeContextHttpCredentials>,
) -> napi::Result<u64> {
    let credential_bytes = credentials
        .map_or(Some(0), |credentials| {
            credentials
                .username
                .len()
                .checked_add(credentials.password.len())
                .and_then(|total| {
                    total.checked_add(credentials.origin.as_ref().map_or(0, String::len))
                })
        })
        .ok_or_else(|| {
            binding_error(HostError::ResourceExhausted(
                "Context HTTP credentials input byte length overflow".into(),
            ))
        })?;
    let input_bytes = context_id_bytes
        .checked_add(credential_bytes)
        .ok_or_else(|| {
            binding_error(HostError::ResourceExhausted(
                "Context HTTP credentials input byte length overflow".into(),
            ))
        })?;
    boundary::SMALL_ALLOWANCE
        .checked_add(u64::try_from(input_bytes).map_err(|_| {
            binding_error(HostError::ResourceExhausted(
                "Context HTTP credentials FFI input byte length overflow".into(),
            ))
        })?)
        .ok_or_else(|| {
            binding_error(HostError::ResourceExhausted(
                "Context HTTP credentials FFI input byte allowance overflow".into(),
            ))
        })
}

#[cfg(feature = "context-http-credentials")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn set_context_http_credentials<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'_>,
        credentials: Option<NativeContextHttpCredentials>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let allowance =
            context_http_credentials_input_allowance(context_id.utf8_len()?, credentials.as_ref())?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let credentials = credentials
            .map(NativeContextHttpCredentials::into_host)
            .transpose()
            .map_err(binding_error)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.set_context_http_credentials(&context_id, credentials)
                .await
        })
    }
}

#[cfg(feature = "context-locale")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_locale<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        locale: JsString<'_>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let allowance = context_locale_input_allowance(&[
            base_url
                .as_ref()
                .map(JsString::utf8_len)
                .transpose()?
                .unwrap_or(0),
            locale.utf8_len()?,
        ])?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let locale =
            playwright_rust_node_host::ContextLocale::new(locale.into_utf8()?.into_owned()?)
                .map_err(binding_error)?;
        let policies =
            playwright_rust_node_host::ContextPolicyOptions::default().with_locale(Some(locale));
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_policies(
                    base_url.as_deref(),
                    #[cfg(feature = "device-emulation")]
                    None,
                    policies,
                )
                .await?,
            )
        })
    }
}

#[napi(object)]
pub struct NativeDeviceContextOptions {
    pub user_agent: Option<String>,
    pub viewport_width: Option<u32>,
    pub viewport_height: Option<u32>,
    pub screen_width: Option<u32>,
    pub screen_height: Option<u32>,
    pub device_scale_factor: Option<f64>,
    pub is_mobile: Option<bool>,
    pub has_touch: Option<bool>,
}

#[cfg(feature = "device-emulation")]
impl NativeDeviceContextOptions {
    fn into_host(
        self,
    ) -> std::result::Result<playwright_rust_node_host::DeviceContextOptions, HostError> {
        let viewport = match (self.viewport_width, self.viewport_height) {
            (Some(width), Some(height)) => Some((width, height)),
            (None, None) => None,
            _ => {
                return Err(HostError::InvalidArgument(
                    "device viewport requires both width and height".into(),
                ));
            }
        };
        let screen = match (self.screen_width, self.screen_height) {
            (Some(width), Some(height)) => Some((width, height)),
            (None, None) => None,
            _ => {
                return Err(HostError::InvalidArgument(
                    "device screen requires both width and height".into(),
                ));
            }
        };
        playwright_rust_node_host::DeviceContextOptions::new(
            self.user_agent,
            viewport,
            screen,
            self.device_scale_factor,
            self.is_mobile,
            self.has_touch,
        )
    }
}

#[cfg(any(
    feature = "extra-headers",
    feature = "context-locale",
    feature = "context-timezone",
    feature = "context-geolocation",
    feature = "context-geolocation-permission",
    feature = "context-http-credentials",
    feature = "context-offline",
    feature = "context-proxy",
    feature = "ignore-https-errors",
    feature = "bypass-csp"
))]
#[napi(object)]
pub struct NativeContextGeolocation {
    pub longitude: f64,
    pub latitude: f64,
    pub accuracy: Option<f64>,
}

#[cfg(any(
    feature = "extra-headers",
    feature = "context-locale",
    feature = "context-timezone",
    feature = "context-geolocation",
    feature = "context-geolocation-permission",
    feature = "context-http-credentials",
    feature = "context-offline",
    feature = "context-proxy",
    feature = "ignore-https-errors",
    feature = "bypass-csp"
))]
#[napi(object)]
pub struct NativeContextHttpCredentials {
    pub username: String,
    pub password: String,
    pub origin: Option<String>,
    pub send_always: Option<bool>,
}

#[cfg(feature = "context-http-credentials")]
impl NativeContextHttpCredentials {
    fn into_host(
        self,
    ) -> std::result::Result<playwright_rust_node_host::ContextHttpCredentials, HostError> {
        playwright_rust_node_host::ContextHttpCredentials::new_with_send(
            self.username,
            self.password,
            self.origin,
            self.send_always.unwrap_or(false),
        )
    }
}

#[cfg(feature = "context-geolocation")]
impl NativeContextGeolocation {
    fn into_host(
        self,
    ) -> std::result::Result<playwright_rust_node_host::ContextGeolocation, HostError> {
        playwright_rust_node_host::ContextGeolocation::new(
            self.longitude,
            self.latitude,
            self.accuracy.unwrap_or(0.0),
        )
    }
}

#[cfg(any(
    feature = "extra-headers",
    feature = "context-locale",
    feature = "context-timezone",
    feature = "context-geolocation",
    feature = "context-geolocation-permission",
    feature = "context-http-credentials",
    feature = "context-offline",
    feature = "context-proxy",
    feature = "ignore-https-errors",
    feature = "bypass-csp"
))]
#[napi(object)]
pub struct NativeContextCreationOptions {
    /// Keeps the N-API object nonempty in an extra-headers-only build. Callers
    /// leave this unset; it is not a browser policy.
    pub reserved: Option<bool>,
    pub device: Option<NativeDeviceContextOptions>,
    pub locale: Option<String>,
    pub timezone_id: Option<String>,
    pub geolocation: Option<NativeContextGeolocation>,
    pub permissions: Option<Vec<String>>,
    pub http_credentials: Option<NativeContextHttpCredentials>,
    pub offline: Option<bool>,
    pub proxy: Option<NativeContextProxy>,
    pub ignore_https_errors: Option<bool>,
    pub bypass_csp: Option<bool>,
}

#[cfg(any(
    feature = "extra-headers",
    feature = "context-locale",
    feature = "context-timezone",
    feature = "context-geolocation",
    feature = "context-geolocation-permission",
    feature = "context-http-credentials",
    feature = "context-offline",
    feature = "context-proxy",
    feature = "ignore-https-errors",
    feature = "bypass-csp"
))]
pub(crate) struct HostContextCreationOptions {
    #[cfg(feature = "device-emulation")]
    device: Option<playwright_rust_node_host::DeviceContextOptions>,
    policies: playwright_rust_node_host::ContextPolicyOptions,
}

#[cfg(any(
    feature = "extra-headers",
    feature = "context-locale",
    feature = "context-timezone",
    feature = "context-geolocation",
    feature = "context-geolocation-permission",
    feature = "context-http-credentials",
    feature = "context-offline",
    feature = "context-proxy",
    feature = "ignore-https-errors",
    feature = "bypass-csp"
))]
impl NativeContextCreationOptions {
    pub(crate) fn ffi_allowance(&self, base: u64) -> napi::Result<u64> {
        #[cfg(feature = "context-locale")]
        let locale_bytes = self.locale.as_ref().map(String::len).unwrap_or(0);
        #[cfg(not(feature = "context-locale"))]
        let locale_bytes = 0;
        let locale_bytes = u64::try_from(locale_bytes).map_err(|_| {
            binding_error(HostError::ResourceExhausted(
                "Context locale FFI input byte length overflow".into(),
            ))
        })?;
        let allowance = base.checked_add(locale_bytes).ok_or_else(|| {
            binding_error(HostError::ResourceExhausted(
                "Context policy FFI input byte allowance overflow".into(),
            ))
        })?;
        #[cfg(feature = "context-timezone")]
        let timezone_bytes = self.timezone_id.as_ref().map(String::len).unwrap_or(0);
        #[cfg(not(feature = "context-timezone"))]
        let timezone_bytes = 0;
        let timezone_bytes = u64::try_from(timezone_bytes).map_err(|_| {
            binding_error(HostError::ResourceExhausted(
                "Context timezone FFI input byte length overflow".into(),
            ))
        })?;
        let allowance = allowance.checked_add(timezone_bytes).ok_or_else(|| {
            binding_error(HostError::ResourceExhausted(
                "Context policy FFI input byte allowance overflow".into(),
            ))
        })?;
        #[cfg(feature = "context-geolocation-permission")]
        let permission_bytes = self
            .permissions
            .as_ref()
            .into_iter()
            .flatten()
            .try_fold(0usize, |total, permission| {
                total.checked_add(permission.len())
            })
            .ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "Context permission input byte length overflow".into(),
                ))
            })?;
        #[cfg(not(feature = "context-geolocation-permission"))]
        let permission_bytes = 0;
        let allowance = allowance
            .checked_add(u64::try_from(permission_bytes).map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "Context permission FFI input byte length overflow".into(),
                ))
            })?)
            .ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "Context policy FFI input byte allowance overflow".into(),
                ))
            })?;
        #[cfg(feature = "context-http-credentials")]
        let credential_bytes = if let Some(credentials) = self.http_credentials.as_ref() {
            credentials
                .username
                .len()
                .checked_add(credentials.password.len())
                .and_then(|total| {
                    total.checked_add(credentials.origin.as_ref().map_or(0, String::len))
                })
                .ok_or_else(|| {
                    binding_error(HostError::ResourceExhausted(
                        "Context HTTP credentials input byte length overflow".into(),
                    ))
                })?
        } else {
            0
        };
        #[cfg(not(feature = "context-http-credentials"))]
        let credential_bytes = 0;
        allowance
            .checked_add(u64::try_from(credential_bytes).map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "Context HTTP credentials FFI input byte length overflow".into(),
                ))
            })?)
            .ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "Context policy FFI input byte allowance overflow".into(),
                ))
            })
    }

    pub(crate) fn into_host(self) -> std::result::Result<HostContextCreationOptions, HostError> {
        let policies = playwright_rust_node_host::ContextPolicyOptions::default();
        #[cfg(feature = "context-locale")]
        let policies = policies.with_locale(
            self.locale
                .map(playwright_rust_node_host::ContextLocale::new)
                .transpose()?,
        );
        #[cfg(feature = "context-timezone")]
        let policies = policies.with_timezone(
            self.timezone_id
                .map(playwright_rust_node_host::ContextTimezone::new)
                .transpose()?,
        );
        #[cfg(feature = "context-geolocation")]
        let policies = policies.with_geolocation(
            self.geolocation
                .map(NativeContextGeolocation::into_host)
                .transpose()?,
        );
        #[cfg(feature = "context-geolocation-permission")]
        let policies =
            policies.with_permissions(playwright_rust_node_host::ContextPermissions::new(
                self.permissions.unwrap_or_default(),
            )?);
        #[cfg(feature = "context-http-credentials")]
        let policies = policies.with_http_credentials(
            self.http_credentials
                .map(NativeContextHttpCredentials::into_host)
                .transpose()?,
        );
        #[cfg(feature = "context-offline")]
        let policies = policies.with_offline(self.offline.unwrap_or(false));
        #[cfg(feature = "context-proxy")]
        let policies =
            policies.with_proxy(self.proxy.map(NativeContextProxy::into_host).transpose()?);
        #[cfg(feature = "ignore-https-errors")]
        let policies = policies.with_ignore_https_errors(self.ignore_https_errors.unwrap_or(false));
        #[cfg(feature = "bypass-csp")]
        let policies = policies.with_bypass_csp(self.bypass_csp.unwrap_or(false));
        Ok(HostContextCreationOptions {
            #[cfg(feature = "device-emulation")]
            device: self
                .device
                .map(NativeDeviceContextOptions::into_host)
                .transpose()?,
            policies,
        })
    }
}

#[cfg(any(
    feature = "extra-headers",
    feature = "context-locale",
    feature = "context-timezone",
    feature = "context-geolocation",
    feature = "context-geolocation-permission",
    feature = "context-http-credentials",
    feature = "context-offline",
    feature = "context-proxy",
    feature = "ignore-https-errors",
    feature = "bypass-csp"
))]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_policies<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        options: NativeContextCreationOptions,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let allowance = options.ffi_allowance(boundary::SMALL_ALLOWANCE)?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let options = options.into_host().map_err(binding_error)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_policies(
                    base_url.as_deref(),
                    #[cfg(feature = "device-emulation")]
                    options.device,
                    options.policies,
                )
                .await?,
            )
        })
    }
}

#[cfg(feature = "device-emulation")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_device_options<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        options: NativeDeviceContextOptions,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let device = options.into_host().map_err(binding_error)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_device_options(base_url.as_deref(), Some(device))
                    .await?,
            )
        })
    }

    #[napi]
    pub fn new_context_with_has_touch<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        enabled: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let device = playwright_rust_node_host::DeviceContextOptions::new(
            None,
            None,
            None,
            None,
            None,
            Some(enabled),
        )
        .map_err(binding_error)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_device_options(base_url.as_deref(), Some(device))
                    .await?,
            )
        })
    }
}

#[cfg(all(feature = "device-emulation", feature = "ignore-https-errors"))]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_device_options_and_ignore_https_errors<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        options: NativeDeviceContextOptions,
        enabled: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let device = options.into_host().map_err(binding_error)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_ignore_https_errors(
                    base_url.as_deref(),
                    Some(device),
                    enabled,
                )
                .await?,
            )
        })
    }
}

#[cfg(feature = "context-proxy")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn new_context_with_proxy<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        proxy: NativeContextProxy,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let proxy = playwright_rust_node_host::ContextProxy::new(
            proxy.server,
            proxy.bypass,
            proxy.username,
            proxy.password,
        )
        .map_err(binding_error)?;
        let policies =
            playwright_rust_node_host::ContextPolicyOptions::default().with_proxy(Some(proxy));
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(
                host.new_context_with_policies(
                    base_url.as_deref(),
                    #[cfg(feature = "device-emulation")]
                    None,
                    policies,
                )
                .await?,
            )
        })
    }
}

#[napi(object)]
pub struct FfiMetrics {
    pub pending: u32,
    pub queued: u32,
    pub queued_commands: u32,
    pub running: u32,
    pub reserved_bytes: u32,
    pub connects: u32,
    pub commands: u32,
    pub closes: u32,
    pub watches: u32,
}

impl Drop for NativeSession {
    fn drop(&mut self) {
        #[cfg(feature = "retained-file-input")]
        self.file_input_scope.close();
        self.host.request_shutdown();
    }
}

#[cfg(feature = "init-scripts")]
fn init_script_input_allowance(lengths: &[usize]) -> napi::Result<u64> {
    lengths
        .iter()
        .try_fold(boundary::SMALL_ALLOWANCE, |total, length| {
            let length = u64::try_from(*length).map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "init-script FFI input byte length overflow".into(),
                ))
            })?;
            total.checked_add(length).ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "init-script FFI input byte allowance overflow".into(),
                ))
            })
        })
}

#[cfg(feature = "init-scripts")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn context_add_init_script<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        source: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let allowance = init_script_input_allowance(&[context_id.utf8_len()?, source.utf8_len()?])?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let source = source.into_utf8()?.into_owned()?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_add_init_script(&context_id, source).await
        })
    }

    #[napi]
    pub fn page_add_init_script<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        source: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let allowance = init_script_input_allowance(&[page_id.utf8_len()?, source.utf8_len()?])?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let source = source.into_utf8()?.into_owned()?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_add_init_script(&page_id, source).await
        })
    }
}

#[cfg(feature = "request-gc")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn page_request_gc<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_request_gc(&page_id).await
        })
    }
}

#[cfg(feature = "page-viewport")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn page_set_viewport_size<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        width: u32,
        height: u32,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_set_viewport_size(&page_id, width, height).await
        })
    }
}

#[cfg(feature = "url-glob")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn page_url_glob(&self, page_id: JsString<'_>, glob: JsString<'_>) -> napi::Result<String> {
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let glob = boundary::bounded_string(glob, 16 * 1024, "URL glob")?;
        self.host
            .page_url_glob(&page_id, &glob)
            .map(|plan| plan.pattern().to_owned())
            .map_err(binding_error)
    }
}

#[cfg(feature = "wait-for-url")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn frame_wait_for_url<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        frame_id: JsString<'env>,
        matcher: napi::bindgen_prelude::Function<'env, String, bool>,
        wait_until: JsString<'env>,
        timeout_ms: u32,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::NAVIGATION_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let frame_id = boundary::bounded_string(frame_id, 20, "frame ID")?;
        let wait_until = boundary::bounded_string(wait_until, 32, "waitUntil")?;
        let matcher = matcher
            .build_threadsafe_function::<String>()
            .callee_handled::<false>()
            .max_queue_size::<8>()
            .build()?;
        let matcher = Arc::new(matcher);
        let host = Arc::clone(&self.host);
        let port = self.publications.clone();
        let publications = port.publications();
        boundary::spawn_with_env(
            env,
            execution,
            async move {
                let result = host
                    .frame_wait_for_url(
                        &page_id,
                        &frame_id,
                        &wait_until,
                        timeout_ms,
                        move |candidate| {
                            let matcher = Arc::clone(&matcher);
                            async move { matcher.call_async_catch(candidate).await }
                        },
                    )
                    .await;
                let result = match result {
                    Ok(url) => Ok(url),
                    Err(playwright_rust_node_host::UrlWaitError::Host(error)) => {
                        Err(binding_error(error))
                    }
                    Err(playwright_rust_node_host::UrlWaitError::Matcher(error)) => Err(error),
                };
                Ok((result, publications.fence()?))
            },
            move |env, (result, fence)| {
                port.publish(env, fence)?;
                result
            },
        )
    }
}

#[cfg(feature = "frame-expect-title")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn frame_wait_for_title<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        frame_id: JsString<'env>,
        matcher: napi::bindgen_prelude::Function<'env, String, bool>,
        timeout_ms: u32,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::READ_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let frame_id = boundary::bounded_string(frame_id, 20, "frame ID")?;
        let matcher = matcher
            .build_threadsafe_function::<String>()
            .callee_handled::<false>()
            .max_queue_size::<8>()
            .build()?;
        let matcher = Arc::new(matcher);
        let host = Arc::clone(&self.host);
        let port = self.publications.clone();
        let publications = port.publications();
        boundary::spawn_with_env(
            env,
            execution,
            async move {
                let result = host
                    .frame_wait_for_title(&page_id, &frame_id, timeout_ms, move |candidate| {
                        let matcher = Arc::clone(&matcher);
                        async move { matcher.call_async_catch(candidate).await }
                    })
                    .await;
                let result = match result {
                    Ok(title) => Ok(title),
                    Err(playwright_rust_node_host::UrlWaitError::Host(error)) => {
                        Err(binding_error(error))
                    }
                    Err(playwright_rust_node_host::UrlWaitError::Matcher(error)) => Err(error),
                };
                Ok((result, publications.fence()?))
            },
            move |env, (result, fence)| {
                port.publish(env, fence)?;
                result
            },
        )
    }
}

#[cfg(feature = "request-events")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn next_request_notice<'env>(
        &self,
        env: &'env Env,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published_sized(
            env,
            execution,
            self.publications.clone(),
            async move { host.next_request_notice().await.map(HostRequestNotice) },
            HostRequestNotice::foreign_copy_bytes,
        )
    }
}

#[cfg(feature = "page-bindings")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn next_page_binding_call<'env>(
        &self,
        env: &'env Env,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Watch, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_with_env(
            env,
            execution,
            async move { Ok(host.next_page_binding_call().await) },
            |_env, call| {
                Ok(call.map(|call| PageBindingCallDescriptor {
                    call_id: call.id.to_string(),
                    context_id: call.context.get().to_string(),
                    page_id: call.page.get().to_string(),
                    frame_id: call.frame.get().to_string(),
                    name: call.name,
                    arguments_json: call.arguments_json,
                    needs_handle: call.needs_handle,
                }))
            },
        )
    }

    #[napi]
    pub fn acknowledge_page_binding_call<'env>(
        &self,
        env: &'env Env,
        call_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let call_id = boundary::bounded_string(call_id, 20, "binding call ID")?;
        let call_id = HostSession::parse_page_binding_call_id(&call_id).map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.acknowledge_page_binding_call(call_id).await
        })
    }

    #[napi]
    pub fn page_expose_binding<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        name: JsString<'env>,
        needs_handle: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let name = boundary::bounded_string(name, 256, "binding name")?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_expose_binding(&page_id, name, needs_handle).await
        })
    }

    #[cfg(feature = "context-bindings")]
    #[napi]
    pub fn context_expose_binding<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        name: JsString<'env>,
        needs_handle: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "browser context ID")?;
        let name = boundary::bounded_string(name, 256, "binding name")?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_expose_binding(&context_id, name, needs_handle)
                .await
        })
    }

    #[napi]
    pub fn deliver_page_binding_result<'env>(
        &self,
        env: &'env Env,
        call_id: JsString<'env>,
        payload: Unknown<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(
                FfiLane::Command,
                playwright_rust_node_host::evaluation::FFI_ALLOWANCE,
            )
            .map_err(binding_error)?;
        let call_id = boundary::bounded_string(call_id, 20, "binding call ID")?;
        let call_id = HostSession::parse_page_binding_call_id(&call_id).map_err(binding_error)?;
        let input = self.conversions.prepare(env, payload)?;
        self.conversions.schedule(
            env,
            deferred_conversion::EvaluationCall {
                publications: self.publications.clone(),
                execution,
                host: Arc::clone(&self.host),
                target: deferred_conversion::EvaluationTarget::PageBindingResult(call_id),
                source: String::new(),
                is_function: true,
                input,
            },
        )
    }
}

#[cfg(feature = "set-content")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn page_set_content<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        html: JsString<'env>,
        wait_until: JsString<'env>,
        timeout_ms: u32,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        use playwright_rust_node_host::evaluation::{FFI_ALLOWANCE, STRING_BYTES};

        let execution = self
            .ffi
            .admit(FfiLane::Command, FFI_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let html = boundary::bounded_string(html, STRING_BYTES, "html")?;
        let wait_until = boundary::bounded_string(wait_until, 32, "waitUntil")?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_set_content(&page_id, html, &wait_until, timeout_ms)
                .await
        })
    }
}

#[cfg(feature = "emulate-media")]
#[napi]
impl NativeSession {
    #[napi]
    #[allow(clippy::too_many_arguments)]
    pub fn page_emulate_media<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        media: Option<JsString<'env>>,
        color_scheme: Option<JsString<'env>>,
        reduced_motion: Option<JsString<'env>>,
        forced_colors: Option<JsString<'env>>,
        contrast: Option<JsString<'env>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let media = media
            .map(|value| boundary::bounded_string(value, 32, "media"))
            .transpose()?;
        let color_scheme = color_scheme
            .map(|value| boundary::bounded_string(value, 32, "color scheme"))
            .transpose()?;
        let reduced_motion = reduced_motion
            .map(|value| boundary::bounded_string(value, 32, "reduced motion"))
            .transpose()?;
        let forced_colors = forced_colors
            .map(|value| boundary::bounded_string(value, 32, "forced colors"))
            .transpose()?;
        let contrast = contrast
            .map(|value| boundary::bounded_string(value, 32, "contrast"))
            .transpose()?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_emulate_media(
                &page_id,
                media.as_deref(),
                color_scheme.as_deref(),
                reduced_motion.as_deref(),
                forced_colors.as_deref(),
                contrast.as_deref(),
            )
            .await
        })
    }
}

#[cfg(feature = "console-events")]
#[napi]
impl NativeSession {
    #[napi]
    pub fn set_page_console_listener(
        &self,
        page_id: JsString<'_>,
        enabled: bool,
    ) -> napi::Result<()> {
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        self.host
            .set_page_console_listener(&page_id, enabled)
            .map_err(binding_error)
    }

    #[napi]
    pub fn set_context_console_listener(
        &self,
        context_id: JsString<'_>,
        enabled: bool,
    ) -> napi::Result<()> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        self.host
            .set_context_console_listener(&context_id, enabled)
            .map_err(binding_error)
    }
}

#[napi]
impl NativeSession {
    #[napi]
    pub fn set_lifecycle_callback(
        &self,
        env: Env,
        callback: napi::bindgen_prelude::Function<'_>,
    ) -> napi::Result<()> {
        self.publications.register(&env, callback)
    }

    #[napi]
    pub fn version(&self) -> String {
        self.host.version().to_owned()
    }

    #[napi]
    pub fn is_connected(&self) -> napi::Result<bool> {
        self.host
            .frame_publications()
            .connection_is_published_open()
            .map_err(binding_error)
    }

    #[napi]
    pub fn publish_disconnect(&self) -> napi::Result<bool> {
        self.host
            .frame_publications()
            .publish_disconnect()
            .map_err(binding_error)
    }

    #[napi]
    pub fn session_signal(&self) -> NativeSessionSignal {
        NativeSessionSignal {
            token: self.host.disconnect_signal(),
            ffi: self.ffi.clone(),
            publications: self.publications.clone(),
        }
    }

    #[napi]
    pub fn contexts<'env>(&self, env: &'env Env) -> napi::Result<Unknown<'env>> {
        let guard = self
            .host
            .reserve_context_conversion(page_snapshot::CONVERSION_BYTES)
            .map_err(binding_error)?;
        let contexts: Vec<_> = self
            .host
            .published_contexts()
            .map_err(binding_error)?
            .into_iter()
            .map(ContextDescriptor::try_from)
            .collect::<std::result::Result<_, _>>()
            .map_err(binding_error)?;
        let result = contexts.into_unknown(env);
        guard.finish();
        result
    }

    #[napi]
    pub fn pages<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'_>,
    ) -> napi::Result<Unknown<'env>> {
        let guard = self
            .host
            .reserve_metadata_conversion(page_snapshot::CONVERSION_BYTES)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let pages: Vec<_> = self
            .host
            .published_pages(&context_id)
            .map_err(binding_error)?
            .into_iter()
            .map(PageDescriptor::from)
            .collect();
        let result = pages.into_unknown(env);
        guard.finish();
        result
    }

    #[napi]
    pub fn new_context<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            ContextDescriptor::try_from(host.new_context_with_base_url(base_url.as_deref()).await?)
        })
    }

    #[napi]
    pub fn close_context<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Close, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.close_context(&context_id).await
        })
    }

    #[napi]
    pub fn new_page<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.new_page(&context_id).await.map(PageDescriptor::from)
        })
    }

    #[napi]
    pub fn new_browser_page<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'_>>,
        accept_downloads: bool,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let base_url = context_settings::read_base_url(env, base_url)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            BrowserPageDescriptor::try_from(
                host.new_browser_page_with_downloads(base_url.as_deref(), accept_downloads)
                    .await?,
            )
        })
    }

    #[napi]
    pub fn close_page<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Close, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.close_page(&page_id).await
        })
    }

    #[napi]
    pub fn metrics(&self) -> napi::Result<SessionMetrics> {
        let value = self.host.metrics().map_err(binding_error)?;
        Ok(SessionMetrics {
            connected: value.connected,
            live_objects: value.live_objects,
            pending_calls: value.pending_calls,
            topology_drivers: value.topology_drivers,
            active_leases: value.active_leases,
            live_scopes: value.live_scopes,
            retained_event_bytes: value.retained_event_bytes,
            active_operations: value.active_operations,
            cleanup_in_flight: value.cleanup_in_flight,
        })
    }

    #[napi]
    pub fn frame_metadata_metrics(&self) -> napi::Result<FrameMetadataMetrics> {
        let value = self.host.frame_metadata_metrics().map_err(binding_error)?;
        Ok(FrameMetadataMetrics {
            entries: value.entries,
            url_bytes: value.url_bytes,
            name_bytes: value.name_bytes,
            active_frames: value.active_frames,
            pending_publications: value.pending_publications,
        })
    }

    #[napi]
    pub fn frame_tree_metrics(&self) -> napi::Result<FrameTreeMetrics> {
        let value = self.host.frame_tree_metrics().map_err(binding_error)?;
        let count = |value: u64| {
            u32::try_from(value)
                .map_err(|error| binding_error(HostError::Backend(error.to_string())))
        };
        Ok(FrameTreeMetrics {
            states: count(value.states)?,
            revisions: count(value.revisions)?,
            bytes: count(value.bytes)?,
        })
    }

    #[napi]
    pub fn ffi_metrics(&self) -> napi::Result<FfiMetrics> {
        let value = self.ffi.metrics().map_err(binding_error)?;
        Ok(FfiMetrics {
            pending: value.pending,
            queued: value.queued,
            queued_commands: value.queued_commands,
            running: value.running,
            reserved_bytes: u32::try_from(value.reserved_bytes)
                .map_err(|error| binding_error(HostError::Backend(error.to_string())))?,
            connects: value.connects,
            commands: value.commands,
            closes: value.closes,
            watches: value.watches,
        })
    }

    #[napi]
    pub fn conversion_metrics(&self) -> deferred_conversion::ConversionMetrics {
        self.conversions.metrics()
    }

    #[napi]
    pub fn locator_conversion_metrics(&self) -> locator_conversion::LocatorConversionMetrics {
        self.locator_conversions.metrics()
    }

    #[napi]
    pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Close, boundary::SMALL_ALLOWANCE)
            .map_err(binding_error)?;
        let host = Arc::clone(&self.host);
        #[cfg(feature = "owned-launch")]
        let launch_profile = self.launch_profile.clone();
        #[cfg(feature = "retained-file-input")]
        self.file_input_scope.close();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.close().await?;
            #[cfg(feature = "owned-launch")]
            if let Some(profile) = launch_profile {
                profile
                    .lock()
                    .map_err(|_| {
                        HostError::Backend("Node launch profile ownership is unavailable".into())
                    })?
                    .take();
            }
            Ok(())
        })
    }

    #[napi]
    pub fn frame_title<'env>(
        &self,
        env: &'env Env,
        frame_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::READ_ALLOWANCE)
            .map_err(binding_error)?;
        let frame_id = boundary::bounded_string(frame_id, 20, "frame ID")?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.frame_title(&frame_id).await
        })
    }

    #[napi]
    pub fn frame_content<'env>(
        &self,
        env: &'env Env,
        frame_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::READ_ALLOWANCE)
            .map_err(binding_error)?;
        let frame_id = boundary::bounded_string(frame_id, 20, "frame ID")?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.frame_content(&frame_id).await
        })
    }

    #[napi]
    pub fn frame_evaluate<'env>(
        &self,
        env: &'env Env,
        frame_id: JsString<'env>,
        source: JsString<'env>,
        is_function: bool,
        argument: Unknown<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let ffi_allowance = playwright_rust_node_host::evaluation::FFI_ALLOWANCE;
        let execution = self
            .ffi
            .admit(FfiLane::Command, ffi_allowance)
            .map_err(binding_error)?;
        let frame_id = boundary::bounded_string(frame_id, 20, "frame ID")?;
        let source = evaluation_value::source(env, source, is_function)?;
        // Even expression arguments are validated like the Playwright client,
        // although expression mode does not pass them to the browser program.
        let input = self.conversions.prepare(env, argument)?;
        self.conversions.schedule(
            env,
            deferred_conversion::EvaluationCall {
                publications: self.publications.clone(),
                execution,
                host: self.host.clone(),
                target: deferred_conversion::EvaluationTarget::FrameValue(frame_id),
                source,
                is_function,
                input,
            },
        )
    }

    #[napi]
    pub fn page_goto<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        url: JsString<'env>,
        wait_until: JsString<'env>,
        timeout_ms: u32,
        referrer: Option<JsString<'env>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::NAVIGATION_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let url = boundary::bounded_string(url, 16 * 1024, "navigation URL")?;
        let wait_until = boundary::bounded_string(wait_until, 32, "waitUntil")?;
        let referrer = referrer
            .map(|value| boundary::bounded_string(value, 16 * 1024, "referer"))
            .transpose()?;
        let host = Arc::clone(&self.host);
        let publications = self.publications.clone();
        #[cfg(feature = "extra-headers")]
        let pending = host.start_page_goto(page_id, url, wait_until, referrer, timeout_ms);
        #[cfg(not(feature = "extra-headers"))]
        let pending = async move {
            host.page_goto(&page_id, url, &wait_until, referrer, timeout_ms)
                .await
        };
        boundary::spawn_published_sized(
            env,
            execution,
            publications,
            async move { pending.await.map(HostNavigationOutput) },
            HostNavigationOutput::response_request_foreign_copy_bytes,
        )
    }
}
