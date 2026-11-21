<!-- agent-updated: 2026-11-21T05:29:00Z -->

# Playwright Rust

Playwright Rust is an experimental, composable browser-automation stack written
in Rust, with Playwright-shaped Node.js and Python APIs. It is an independent
Shuozeli project, not the official Microsoft Playwright implementation, and it
does not yet claim full Playwright compatibility.

The design separates capability contracts, codecs, client/server adapters,
browser backends, runtime integrations, and language bindings. Applications
select only the capabilities they need; browser installation, process launch,
remote connections, tracing, request interception, and other expensive
features remain explicit opt-ins. The supported browser scope is Chromium over
CDP. Firefox and WebKit are intentional non-goals, not compatibility gaps; the
Node public exports now contain only `chromium`, and the project will not ship
placeholder engine objects or pretend CDP is universal. Python async and sync
facades now follow the same Chromium-only rule. Shared registry, installation,
backend-manifest, persistent-context preflight, process-startup, and remote-
connect admission paths are also Chromium-only. The official Playwright Server
initializer's three BrowserType fields remain wire metadata required to parse
the pinned protocol; they do not create public providers.

The opt-in `playwright-rust-remote-cdp` crate is the high-level Rust entry
point for externally owned Chromium instances. The umbrella selects it with
`remote-cdp`; custom WebSocket upgrade headers additionally select
`cdp-handshake-headers`. `connect_over_cdp()` exposes Context/Page creation,
navigation, rendered HTML, final URL, evaluation, Locator operations,
screenshot, and bounded close without browser-process authority. Attachment
shutdown closes only Contexts and Pages created by that session, disconnects
the borrowed transport, and invokes an optional generic `RemoteLease` release
hook exactly once. The crate contains no BCP protocol or dependency; a BCP
adapter owns lease acquisition, renewal, fencing, and release.

Node `BrowserContext.exposeBinding()` and `exposeFunction()` are available
through the independently selectable `context-bindings` profile. The addon
extends the existing Page binding transport; Rust owns Context/Page conflict
rules, current and future Page/OOPIF installation, call identity, and teardown.
JavaScript retains only the user callback needed to execute language code. A
real Chrome 157 run covers an existing Page, a future Page, source identity,
and both directions of Context/Page name conflict.

The current compatibility profile is migrating ordinary browser objects and
browser-produced results to allocator-backed ownership. Session, core topology,
public browser-CDP-session, request, route, script, selector, screenshot, PDF,
response-body, and Page/Context/Service-Worker network paths have been converted.
The compatibility network profiles now allocate Context, Page, and related
Worker adapters on demand instead of rejecting the ninth one. Ordinary retained
Handle call admission and publication inboxes are allocator-backed instead of
rejecting the ninth call. Frame-world snapshots and their native-retirement
receipt registry now grow through the allocator and retain exact teardown
ownership instead of rejecting a valid Handle/action at a derived aggregate
count. Some selected leaves still expose legacy count
limits—notably the 16-Page lifecycle registry, 128-Frame host tree, finite
file-input native-path/marker helpers, and the 32-controller drag
runtime—so this repository does not yet
claim a fully limit-free
Playwright-compatible profile. Internal concurrency should become cancellable
backpressure; payload, unread-queue, and retained-byte safeguards remain.
The ordinary in-memory file-payload lane now allocates host call/source owners,
materialized batches, encoders, upload owners, cleanup continuations, and
large-command backing on demand. Its 64-file, metadata, 50 MiB source, and
encoded-command limits remain semantic per-operation safeguards, not aggregate
concurrency counts.
Path preparation also owns queued/running jobs, normalized paths, readers, and
completed results through allocator-backed cancellation-safe worker slots.

The optional `service-worker-routes` Node profile now applies BrowserContext
routes to Chromium's independent Service Worker sessions without fabricating a
Page owner. Its live Worker-session registry grows through the allocator and is
retired by target or Context lifecycle; the original 64-entry temporary Page
initialization pool is not a public Worker-count limit.

The independent `context-http-credentials` Node profile now supports both
creation-time credentials and Playwright's deprecated runtime
`BrowserContext.setHTTPCredentials()` replacement/clear API. Rust retains the
credentials and authentication-attempt state, serializes Page/OOPIF
initialization against runtime changes, and composes current Page/OOPIF and
Service Worker interception through one Fetch owner when routing is selected.
JavaScript keeps no credential mirror.

The default-off `video-recording` composition owns Chromium screencast capture,
frame acknowledgement, pinned FFmpeg encoding, WebM finalization, and artifact
copy/delete in Rust. Python ABI141 exposes persistent and ordinary owned
Context recording, including `Browser.new_page()`'s implicit Context, without
retaining frames or encoder state in Python. The complete unchanged async/sync
Playwright Python video selection passes 11/11 in the installed candidate; the
independently verified Playwright 1.58.0 Oracle run passes 9/11 on its bundled
Chromium, with both Oracle failures in post-close `Video.save_as()`. Remote
artifact transfer remains open. The Node facade now supports ordinary owned
Contexts and `Browser.newPage()` implicit Contexts through the same Rust
recorder, with `Page.video()` plus `Video.path()`, `saveAs()`, and `delete()`.
Native E2E verifies requested 640x360 and default 800x450 VP8 output, pre-close
`saveAs()` waiting, idempotent delete, and zero retained native leases. The
complete unchanged pinned Core `video.spec.ts` candidate run passes 24, skips
five upstream-declared fixmes, and fails zero. Four skips are selected by the
full-Chromium classification; the internal-page row is unconditionally fixme
upstream. Its source-bound manifest identifies the registry's full Google
Chrome for Testing executable with `channel: chromium`; none of the skips are
local filters. The composed official profile also proves a real Rust-owned
browser kill and independently selectable Context-tracing leaves. The
screenshot leaf writes a real trace ZIP; the chunk leaf adds Rust-owned
`startChunk`, `stopChunk`, `group`, and `groupEnd`, including chunk names,
nested parent IDs, and auto-close. The independent action leaf brackets the
currently supported Page route, navigation, evaluation, evaluation-handle,
generic event wait, set-content, click, double-click, hover, mouse movement,
keyboard insertion, input-file selection, timeout wait, reload, and close
calls, plus Route continue, public Dialog accept/dismiss, BrowserContext
storage-state, and BrowserContext API-request calls with Rust-owned call IDs,
primitive results, and Playwright-compatible titles. The independent
storage-state leaf reuses the Rust host and supports the `indexedDB` option;
path persistence is still unsupported. The independent console leaf records
Rust-owned Page console events, including arguments and location, into the
exported v8 trace; completed actions flush named live traces while console-only
events remain buffered until the next completed action or stop. The independent
BrowserContext network composition reuses Rust HAR recording to write
`trace.network` and SHA-1 resources, omitting JavaScript response bodies as
upstream does.
The independent `api-request-tracing` leaf spools each global
`APIRequestContext` network trace and content resource in Rust, then packages a
Playwright-compatible `trace.network` ZIP without retaining the trace in JS or
mixing simultaneous request contexts. BrowserContext-owned API requests also
feed the exact active Context trace action and Rust HAR registration. The
complete unchanged pinned Core `tracing.spec.ts` file now passes all 32
runnable cases, skips three upstream fixmes, and fails zero, up from the
3/3/29 RED baseline. BrowserContext network failure capture,
named live files under `tracesDir`, overwrite behavior, pending-action
lifecycle, concurrent export during navigation, internal-page filtering,
stop-time console flush, all three global-request tracing rows, two-trace, and
interrupted-action, source collection, per-chunk resource-isolation, Context
API-request, DOM/action snapshots, Page screenshot result tracing,
expect-title, and external-stack rows are green. Rust writes the numeric-call-ID
`trace.stacks` document; Node supplies only the first external callsite frame.
The result is evidence for this pinned Chromium library file, not a claim that
every Playwright tracing or full-suite case is complete. Continuous frames
remain outside the runnable full-Chromium rows. Start and chunk titles are
Rust-owned. Remote Video transfer and Node persistent-context recording remain
open. Non-video `chromium.launchPersistentContext()` itself is now a real
Rust-owned launch: Rust opens the caller profile, publishes exactly one default
Context, and composes `headless`, `timeout`, `baseURL`, and
`ignoreHTTPSErrors`. Unsupported persistent options remain an explicit
compatibility backlog rather than proof of full option parity.

The independently selectable `playwright-rust-har-recording` policy leaf and
host adapter now provide HAR update recording with upstream-default attachment
semantics. Page or BrowserContext recording is Rust-owned, Context recording
includes exact Service Worker fetches, and response bodies are pulled only
from the producing CDP session after terminal publication. Attached bodies use
Playwright-compatible SHA-1 plus MIME-extension names and deduplicate by
content. Plain `.har` files persist resources beside the document; `.zip`
files use the official root `har.har` plus resource layout, and both layouts
replay through the same Rust library. Minimal-mode sizes/time and POST-data
serialization now follow the pinned upstream shape, including form params,
font/base64 handling, and the pinned `mime@3.0.0` preferred-extension table.
Replay no longer imposes private 8 MiB archive/retained, 4,096-entry, 1 MiB
response, 256 KiB POST, 256-header, or 20-redirect defaults that upstream does
not have, and HAR paths no longer have a private 4 KiB length cap.
The Page-routes host likewise has no eight-archive or aggregate 8 MiB retained
admission cap, and the Python capability report no longer advertises those
removed limits. `ReplayLimits` remains available as an explicit opt-in policy.
Recorded response statuses are likewise returned by lookup without an eager
whole-archive validity rule, matching upstream's route-time behavior.
Replay now retains raw HAR content descriptors and loads/decodes only the
matched request POST data or final response body. Plain attachments and ZIP
entries are reopened by the host content-source plugin in a blocking worker,
so opening a HAR neither duplicates every body in memory nor blocks Tokio.
Lookup read/decode errors are logged and flow through the configured
`not_found` abort/fallback policy, as in upstream, rather than escaping while
the intercepted request is unsettled.
Full update mode now also records Playwright-shaped Page/title/lifecycle data,
cookies, raw headers, request/response timing and sizes, redirect targets,
actual HTTP protocol, server address, and TLS security details. Context-creation
HAR is available through Python `record_har_*` options and Node `recordHar`,
using the pinned official defaults and installing before Context publication.
Top-level Service Worker script capture and broad upstream conformance remain
open. Creation-time URL filtering now has a separate
`har-recording-regexp` capability in both bindings. It carries source and
supported flags into a compiled Rust matcher instead of weakening the filter
to a glob; the base `har-recording` profile still builds and native-loads
without that method. Rust's matcher is not yet a complete ECMAScript RegExp
engine, so lookaround/backreference parity and JavaScript-only flags remain an
explicit gap.

The source-pinned Playwright Python v1.58.0 creation-time HAR manifest contains
19 unchanged Chromium cases (10 async and 9 sync). Oracle and the installed
ABI140 candidate both pass 19/19, including glob and RegExp filters. Full mode
now records request `bodySize` as zero when there is no POST body while minimal
mode retains the official `-1` sentinel. The current local abi3 wheel SHA-256 is
`edaac5d49c0f7c4512b8613a2aaac1038e50322efbaa9dceec4ba84cb9a5f085`;
it is an unrepaired macOS wheel whose Homebrew OpenSSL libraries still need
repair before distribution. This focused result is not the complete 6,526-case
upstream Python corpus.

## Repository layout

- `crates/playwright-rust` is the default-free Rust umbrella crate.
- `crates/playwright-rust-*` contains small contracts, codecs, adapters,
  backends, compositions, runtimes, and FFI crates.
- `packages/playwright-rust` is the Node package
  `@shuozeli/playwright-rust`.
- `packages/playwright-rust-python` is the Python distribution
  `shuozeli-playwright-rust`; its import namespace is `playwright_rust`.
- `tests/official` and package-local official-test directories contain pinned
  conformance fixtures and runners.
- `vendor/playwright-rust-*` contains the two modified WebSocket crates used by
  the optional WebSocket transport. Their upstream licenses and notices remain
  intact.

The repository currently exposes library and binding surfaces, but no general
user-facing CLI. `playwright-rust-schema-gen` is a development-time protocol
generator, not a browser automation command.

## Naming

The repository uses one consistent project namespace:

| Surface | Name |
| --- | --- |
| Product and repository | Playwright Rust / `playwright-rust` |
| Rust packages | `playwright-rust-*` |
| Rust identifiers | `playwright_rust_*` |
| Node package | `@shuozeli/playwright-rust` |
| Python distribution | `shuozeli-playwright-rust` |
| Python import | `playwright_rust` |
| Environment variables | `PLAYWRIGHT_RUST_*` |
| Protocol namespace | `playwright.rust.*` |

## First checks

From the repository root:

```sh
cargo metadata --locked --format-version 1 --no-deps
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust --no-default-features
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-node-host --no-default-features
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon --no-default-features
```

Use `pnpm` for Node package work and `uv` for Python package work. Native Node
and Python distributions require their package-specific release build steps and
explicit feature profiles.

## Status and evidence

This is active design and implementation work. Historical design notes and
conformance ledgers are retained for engineering context, but their recorded
results are not automatically fresh proof for a renamed build. Treat a
capability as verified only after its current source, dependency boundary,
language binding, and relevant browser lane pass together.

Start with [architecture](docs/architecture.md), [design](docs/design.md),
[tasks](docs/tasks.md), and [codelabs](docs/codelabs.md). The detailed design is
in [architecture-v2](docs/architecture-v2.md).

## License

The repository license is [MIT](LICENSE). Adapted Playwright material and
forked WebSocket code carry additional notices and licenses in their source
directories; preserve them in downstream distributions.
