<!-- agent-updated: 2026-11-21T03:41:33Z -->

# Feature Comparison: Playwright vs PinchTab vs playwright-rust

> Current scope correction (2026-11-15): Playwright Rust targets Chromium over
> CDP only. Firefox and WebKit are intentional non-goals. This document contains
> an older coarse comparison below; its checkmarks, binary-size claims, CLI/gRPC
> descriptions, and “all gaps complete” language are not current conformance
> evidence. The authoritative ordered gap list is
> [`../tasks.md`](../tasks.md), and no full Playwright parity is claimed.

> Public package boundary: the Node CJS, ESM, and TypeScript surfaces export
> only `chromium`. Firefox/WebKit names in historical receipts or device
> metadata are not executable providers and are excluded from the denominator.
> Python exposes the same single provider. Shared registry/install/backend,
> startup, preflight, and connect-admission code is also Chromium-only; the
> official Server initializer's three-engine object graph remains wire data.

## Measured Chromium-only v1.58.0 baseline

The repository now has a reproducible static comparison:

```bash
uv run --no-project python scripts/compare_playwright_public_api.py \
  /Users/cyuan/projects/thirdparty/playwright-v1.58.0 \
  /Users/cyuan/projects/thirdparty/playwright-python \
  --output .work/official-conformance/playwright-public-api-comparison-v1.58.0.json
```

Both upstream checkouts must be clean and at the exact pinned commits. The
comparison counts unique public member names per object; overloads do not add
credit. Generic Node `EventEmitter` methods are removed from the method count,
and declared event names are measured separately. Firefox and WebKit provider
properties are removed from the Python denominator.

| Surface | Upstream names | Local overlap | Static overlap |
| --- | ---: | ---: | ---: |
| Node object members | 479 | 382 | 79.7% |
| Node declared event names | 37 | 12 | 32.4% |
| Node sampled option fields | 62 | 32 | 51.6% |
| Python async object members | 493 | 399 | 80.9% |
| Python sync object members | 493 | 399 | 80.9% |

The sampled Node option denominator covers `BrowserContextOptions`,
`ConnectOverCDPOptions`, `PageScreenshotOptions`, and
`LocatorScreenshotOptions`. It is deliberately reported separately because a
method-name match can conceal an incompatible or incomplete option surface.

The pinned Core inventory was also regenerated from source. It contains 558
tracked specs and 48,477 expanded invocations, with SHA-256
`2f7fe371e87267225d195ce537db18621728902ebb5e343ab98c2bc8144b8c30`. The
in-scope `library` Chromium public-API slice is 3,199 expanded invocations from
225 source files: 1,209 `chromium-library` and 1,990 `chromium-page` rows.
There is no fresh complete candidate execution for those 3,199 rows, so this
snapshot does **not** report a behavioral compatibility percentage.

The latest focused closures expose Node
`BrowserContext.setHTTPCredentials()`, `exposeBinding()`, and
`exposeFunction()` through existing Rust owners. Credential update and clear
mutate retained Context state before refreshing current Page/OOPIF and Service
Worker Fetch sessions. Context bindings compose the Page binding transport,
install in current and future Page/OOPIF sessions, and keep duplicate/conflict
rules in Rust; Node retains only the user callback. The two binding methods
raise BrowserContext overlap to 27/31 and total Node overlap to 382/479. A real
Chrome 157 run covers existing/future Pages, source identity, and both
Context/Page conflict directions. Static overlap is not behavioral
conformance. Separately, the unchanged pinned Playwright 1.58.0
`browsercontext-expose-function.spec.ts` now passes all 6/6 cases through the
rebuilt full official candidate profile. No new live-Chromium Node
authentication run was performed for the credential setter.

The largest current static gaps are:

1. Remaining Node event declarations and event-producing facades, especially
   Page lifecycle, Worker console, Download, FileChooser, and WebSocket ownership.
2. Node `BrowserContext` methods and options: WebSocket/HAR routing,
   background pages, Clock, and the remaining option edges. Separately
   selectable tracing
   leaves now write one real screenshot resource, implement Rust-owned chunks
   plus nested `group`/`groupEnd` events, and bracket Page route/navigation/
   set-content/click/double-click/evaluation/reload/close, Route continue,
   Dialog accept, and BrowserContext
   storage-state calls with Rust-owned action IDs/titles. The storage-state
   leaf supports `indexedDB` but not path persistence. A separate Rust console
   leaf writes Page console events into the exported trace at stop. The
   separate global-request leaf spools Playwright `trace.network` events and
   resources per request context in Rust. A separate BrowserContext network
   composition now records failures and resources, while named live trace files
   honor launch `tracesDir`, overwrite, console flush ordering, pending actions,
   and concurrent export. External callsites cross the Node boundary only when
   requested; Rust owns per-chunk source deduplication, reads, hashes and ZIP
   resources. Mouse, keyboard, file-input, timeout, and hover actions are now
   bracketed; Context-owned API requests reuse the active Rust trace HAR
   recorder; and `trace.stacks` maps ordinary action IDs to the first external
   callsite. DOM/action frame snapshots, Page screenshot result tracing, and
   expect-title execution/tracing are now implemented as composable leaves.
   The complete pinned Core tracing file is 32 passed / three upstream fixmes
   skipped / zero failed in two consecutive runs. This is a green source-bound
   Chromium slice, not full-corpus parity. Creation-time `recordHar` now supports a separately
   selectable Rust RegExp matcher and passes the fixed 19-case upstream Python
   HAR manifest in both candidate and Oracle. HAR, route, and cookie filters
   now share an optional Rust-native ECMAScript matcher with lookaround,
   backreferences, UTF-16 indexing, Unicode sets, and all current flags;
   route matching keeps the upstream server/client state split. A source-pinned
   Test262/Node 24.12 corpus passes 5,135 static patterns and 97,198
   `test()`/`lastIndex` steps with Unicode 16.0 data. Dynamically constructed
   Test262 programs and a live upstream route oracle remain open.
3. Missing auxiliary objects in Node: Download, FileChooser,
   WebSocket/WebSocketRoute, Clock, Coverage, WebError, and BrowserServer;
   dedicated-Worker publication and Worker console remain open. Local Video
   artifacts are implemented for ordinary owned Contexts and the complete
   pinned Core video file is 24 passed / five upstream-declared fixmes skipped /
   zero failed, while remote Video transfer and Node persistent-context recording
   remain open.
4. Frame and ElementHandle parity. Node statically overlaps 51/60 Frame and
   25/44 ElementHandle members; Python overlaps 27/60 and 31/44.
5. Request/Response API and lifecycle fidelity. Full HAR recording now consumes
   raw extra-info headers, timing/sizes, protocol, server address, TLS details,
   Page lifecycle metadata, and Context-owned Service Worker bodies. Remaining
   Python and Node creation-time HAR now use the official option defaults;
   the unchanged 10 async plus 9 sync Python HAR cases pass 19/19 on the
   installed ABI140 candidate and 19/19 on the Oracle. Remaining gaps include
   exposing all metadata consistently through public Request/Response methods,
   top-level Service Worker script/import capture, the remaining dynamic/live
   ECMAScript RegExp proof, and broad upstream HAR/network execution beyond this
   fixed slice.

These are scheduling signals, not proof that every overlapping name behaves
like Playwright. Behavioral closure still requires the Rust owner, binding
profile, focused regression, and unchanged source-bound upstream test.

The fixed BrowserContext cookie slice supplies that narrower behavioral
evidence. Four complete, unchanged Chromium library files pinned to Playwright
v1.58.0 contain 56 tests: 55 pass in the Rust candidate, one is skipped by
upstream, and none is unexpected. The slice includes a real Rust-owned
persistent Chromium profile and third-party cookie cases. It does not convert
the static overlap table into a whole-library pass rate.

> playwright-rust is a **Playwright-compatible** Rust CDP bridge. It covers the core browser control surface and exposes it as both a Rust library (with a Playwright-style Page/Locator API), a gRPC service, and a CLI. PinchTab is a full browser control plane with orchestration, profiles, security, and a dashboard. Playwright is the original, full-featured Microsoft framework handling everything from browser installation to high-level test assertions.

## At a Glance

| Dimension | Playwright | PinchTab | playwright-rust |
|-----------|------------|----------|---------|
| Language | TS/JS, Python, Java, .NET | Go | Rust |
| Binary size | Large (Node/Py + Browsers) | ~12 MB | ~5.9 MB |
| API transport | Node IPC / WebSocket | HTTP/REST | gRPC + Rust lib + CLI |
| Architecture | Node server + multiple client bindings | All-in-one control plane | Thin bridge + Playwright API |
| CDP library | Custom | chromedp (Go) | Raw WebSocket (custom) |
| Process model | Framework manages browser execution | Server manages Chrome lifecycle | Attach to existing Chrome |
| API style | Playwright | Custom REST endpoints | Playwright-compatible Page/Locator |

---

## Browser Actions

| Action | Playwright | PinchTab | playwright-rust | Notes |
|--------|:----------:|:--------:|:-------:|-------|
| Click (by selector) | ✅ | ✅ | ✅ | `locator("button").click()` |
| Click (by ref) | ❌ | ✅ | ✅ | gRPC `ExecuteAction` |
| Double-click | ✅ | ✅ | ✅ | `mouse.dblclick()` |
| Type text | ✅ | ✅ | ✅ | `locator.type_text()` |
| Fill value | ✅ | ✅ | ✅ | `locator.fill()` |
| Press key | ✅ | ✅ | ✅ | `keyboard.press()` / `locator.press()` |
| Focus | ✅ | ✅ | ✅ | `locator.focus()` |
| Blur | ✅ | ❌ | ✅ | `locator.blur()` |
| Hover | ✅ | ✅ | ✅ | `locator.hover()` |
| Check/Uncheck | ✅ | ❌ | ✅ | `locator.check()` / `locator.uncheck()` |
| Select (`<select>`) | ✅ | ✅ | ✅ | `locator.select_option()` |
| Scroll (element) | ✅ | ✅ | ✅ | `locator.scroll_into_view()` |
| Scroll (page) | ✅ | ✅ | ✅ | `mouse.wheel()` |
| Drag | ✅ | ✅ | ✅ | `drag_by_node_id()` |
| Dispatch event | ✅ | ❌ | ✅ | `locator.dispatch_event()` |
| Human-like click | ❌ | ✅ | ❌ | Bezier curves, random delays |
| Human-like type | ❌ | ✅ | ❌ | Variable keystroke timing |

---

## Navigation

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| Navigate to URL | ✅ | ✅ | ✅ | `page.goto()` |
| Reload | ✅ | ❌ | ✅ | `page.reload()` |
| Go back/forward | ✅ | ❌ | ✅ | `page.go_back()` / `page.go_forward()` |
| Wait: DOM ready | ✅ | ✅ | ✅ | |
| Wait: network idle | ✅ | ✅ | ✅ | |
| Wait: CSS selector | ✅ | ✅ | ✅ | `locator.wait_for()` |
| Wait: timeout | ✅ | ❌ | ✅ | `page.wait_for_timeout()` |
| Block images | ✅ | ✅ | ✅ | |
| Block media | ✅ | ✅ | ✅ | |
| Configurable timeout | ✅ | ✅ | ✅ | |

---

## Content Extraction

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| Accessibility snapshot | ✅ | ✅ | ✅ | |
| Screenshot (PNG/JPEG) | ✅ | ✅ | ✅ | `page.screenshot()` |
| Full-page screenshot | ✅ | ✅ | ✅ | |
| PDF export | ✅ | ✅ | ✅ | Chromium `page.pdf()` options match pinned 1.58.0; source-bound CDP E2E covers save and outline |
| Text extraction | ✅ | ✅ | ✅ | `page.body_text()` |
| Page title | ✅ | ❌ | ✅ | `page.title()` |
| Page content (HTML) | ✅ | ❌ | ✅ | `page.content()` |
| Page URL | ✅ | ❌ | ✅ | `page.url()` |
| JS evaluate | ✅ | ✅ | ✅ | `page.evaluate()` |
| Cookies get/set | ✅ | ✅ | ✅ | |
| Element text content | ✅ | ❌ | ✅ | `locator.text_content()` |
| Element inner text | ✅ | ❌ | ✅ | `locator.inner_text()` |
| Element inner HTML | ✅ | ❌ | ✅ | `locator.inner_html()` |
| Element attributes | ✅ | ❌ | ✅ | `locator.get_attribute()` |
| Input value | ✅ | ❌ | ✅ | `locator.input_value()` |
| Bounding box | ✅ | ❌ | ✅ | `locator.bounding_box()` |
| Element visibility | ✅ | ❌ | ✅ | `locator.is_visible()` / `is_hidden()` |
| Element state | ✅ | ❌ | ✅ | `is_enabled()`, `is_disabled()`, `is_checked()` |
| Element count | ✅ | ❌ | ✅ | `locator.count()` |
| File download | ✅ | ✅ | ✅ | `page.expect_download()` / CLI `download` |
| File upload | ✅ | ✅ | ✅ | `locator.set_input_files()` / `DOM.setFileInputFiles` |
| Network response capture | ✅ | ❌ | ✅ | `page.wait_for_response()` / `page.on_response()` |
| Network request capture | ✅ | ❌ | ✅ | `page.wait_for_request()` / `page.on_request()` |
| Response body access | ✅ | ❌ | ✅ | `page.response_body(request_id)` |
| Evaluate with args | ✅ | ❌ | ✅ | `page.evaluate_with_arg()` (no JS injection) |
| Async JS evaluate | ✅ | ❌ | ✅ | `page.evaluate_async()` (awaitPromise) |
| Per-element evaluate | ✅ | ❌ | ✅ | `locator.evaluate()` |

---

## Locator & Selector Engine

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| CSS selector | ✅ | ✅ | ✅ | `page.locator()` |
| By test ID | ✅ | ❌ | ✅ | `page.get_by_test_id()` |
| By placeholder | ✅ | ❌ | ✅ | `page.get_by_placeholder()` |
| By alt text | ✅ | ❌ | ✅ | `page.get_by_alt_text()` |
| By title | ✅ | ❌ | ✅ | `page.get_by_title()` |
| Scoped sub-locator | ✅ | ❌ | ✅ | `locator.locator()` |
| First/Last/Nth | ✅ | ❌ | ✅ | `locator.first()` / `locator.last()` / `locator.nth(n)` |
| By role | ✅ | ❌ | ✅ | `page.get_by_role()` with implicit role mapping |
| By text | ✅ | ❌ | ✅ | `page.get_by_text()` via JS text matching |
| By label | ✅ | ❌ | ✅ | `page.get_by_label()` via label/aria lookup |
| Filter (hasText) | ✅ | ❌ | ✅ | `locator.filter_by_text()` |
| And/Or composition | ✅ | ❌ | ✅ | `locator.and()` / `locator.or()` |

---

## Input Devices

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| Keyboard press | ✅ | ✅ | ✅ | `keyboard.press()` |
| Keyboard down/up | ✅ | ❌ | ✅ | `keyboard.down()` / `keyboard.up()` |
| Keyboard type | ✅ | ✅ | ✅ | `keyboard.type_text()` |
| Keyboard insert | ✅ | ❌ | ✅ | `keyboard.insert_text()` |
| Mouse click | ✅ | ✅ | ✅ | `mouse.click()` |
| Mouse dblclick | ✅ | ❌ | ✅ | `mouse.dblclick()` |
| Mouse move | ✅ | ❌ | ✅ | `mouse.move_to()` |
| Mouse down/up | ✅ | ❌ | ✅ | `mouse.down()` / `mouse.up()` |
| Mouse wheel | ✅ | ❌ | ✅ | `mouse.wheel()` |
| Touchscreen | ✅ | ❌ | ✅ | `page.touchscreen().tap()` |

---

## Tab Management

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| Create tab (page) | ✅ | ✅ | ✅ | |
| Close tab | ✅ | ✅ | ✅ | |
| List tabs | ✅ | ✅ | ✅ | |
| Bring to front | ✅ | ❌ | ✅ | `page.bring_to_front()` |
| Tab locking (mutex) | ❌ | ✅ | ❌ | Owner-based TTL locks |
| Tab executor (parallel) | ✅ | ✅ | ✅ | Per-tab Mutex + cross-tab Semaphore |
| Tab screencast (live) | ❌ | ✅ | ❌ | WebSocket-based live video |

---

## CLI

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| CLI tool | ✅ | ✅ | ✅ | `playwright-cli` / `playwright-rust-cli` |
| Navigate command | ✅ | ✅ | ✅ | |
| Snapshot command | ✅ | ✅ | ✅ | |
| Screenshot command | ✅ | ✅ | ✅ | |
| Evaluate command | ✅ | ✅ | ✅ | |
| Click command | ✅ | ✅ | ✅ | |
| Type command | ❌ | ✅ | ✅ | |
| Fill command | ❌ | ✅ | ✅ | |
| Download command | ❌ | ❌ | ✅ | |

---

## Instance & Profile Management

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| Browser orchestrator | partial | ✅ | ❌ | Playwright test worker scaling |
| Profile persistence | ✅ | ✅ | ❌ | UserDataDir context |
| Profile CRUD | ❌ | ✅ | ❌ | |
| Browser lifecycle mgmt | ✅ | ✅ | ❌ | |
| Allocation policies | ❌ | ✅ | ❌ | |

---

## Security & Stealth

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| Stealth mode | plugins | ✅ | ❌ | Playwright requires playwright-stealth |
| IDPI (prompt defense) | ❌ | ✅ | ❌ | |
| API token auth | ❌ | ✅ | ❌ | |
| Ad blocking | ❌ | ✅ | ❌ | |

---

## Infrastructure

| Feature | Playwright | PinchTab | playwright-rust | Notes |
|---------|:----------:|:--------:|:-------:|-------|
| Web dashboard | ✅ | ✅ | ❌ | Playwright UI Mode / HTML Report |
| OpenAPI spec | ❌ | ✅ | ❌ | |
| Docker support | ✅ | ✅ | ✅ | Docker integration tests + CI |
| Unit-testable (mocking)| ✅ | ❌ | ✅ | `FakeCdpClient` + `MockCdpClient` |
| Script runner | ❌ | ❌ | ✅ | Declarative YAML scripts (`playwright-rust run`) |

---

## playwright-rust vs Playwright Differences

| Feature | Playwright | playwright-rust |
|---------|:----------:|:-------:|
| Language Binding | TS/JS, Python, Java, C# | Rust |
| IPC / Transport Interface | Node server over JS interop | Native gRPC Protocol Buffers |
| CLI Capabilities | Native code generation / snapshots | Remote browser interaction over CLI |
| Component Focus | End-to-end testing monolithic engine | Highly modular and embedded automation |
| Memory Footprint | Heavy (spawns node processes) | Extremely lightweight (~5.9MB binary) |
| Chromium Distribution | Downloads and manages its own binaries | Connects to any pre-existing browser process |

---

## Playwright Parity Gaps

The following narrow historical list was implemented at the time, but it was
never a complete Playwright Chromium gap inventory and must not be read as a
current parity claim:

| # | Feature | Status | Golden Tests |
|---|---------|:------:|:------------:|
| 1 | File upload (`DOM.setFileInputFiles`) | ✅ | 1/1 |
| 2 | `get_by_text()` | ✅ | 2/2 |
| 3 | `get_by_label()` | ✅ | 2/2 |
| 4 | `get_by_role()` (with implicit role mapping) | ✅ | 3/3 |
| 5 | `filter({ hasText })` | ✅ | 1/1 |
| 6 | `and()` / `or()` composition | ✅ | 2/2 |
| 7 | Touchscreen (`Input.dispatchTouchEvent`) | ✅ | 1/1 |

### Intentionally Out of Scope

These are features from PinchTab or Playwright that playwright-rust will **not** implement, as they fall outside playwright-rust's role as a thin, embeddable browser bridge:

- **Firefox and WebKit providers** — Chromium/CDP is the explicit product scope
- **Human-like click/type** — PinchTab stealth concern (Bezier curves, variable delays)
- **Tab locking / mutex** — orchestration-layer concern
- **Tab screencast** — operational dashboard feature
- **Multi-instance orchestrator / Profile CRUD / Allocation policies** — control plane scope
- **Stealth mode / IDPI / API auth / Ad blocking** — security/anti-detection layer
- **Web dashboard / OpenAPI spec** — operational tooling
- **Browser lifecycle management** — playwright-rust attaches to existing Chrome by design

---

## Summary

**Playwright** is the industry standard for end-to-end browser automation, heavily coupled with testing. It has everything from code generation to a massive distribution architecture. It operates fully client-server where the Node process governs browser lifecycles.

**PinchTab** is a batteries-included browser automation platform with an operational focus — Chrome lifecycle tracking, profiles, security, stealth, multi-instance orchestration, and a custom built dashboard using REST APIs.

**playwright-rust** bridges the gap. It is a lightweight, **Playwright-compatible browser bridge** written in pure Rust. It mirrors Playwright's elegant API surface (`Page` → `Locator`) within a highly constrained (~5.9MB), dependency-free binary execution model. It communicates natively via cross-language gRPC or its CLI. It purposefully does not distribute browsers, handle Chromium lifecycles, or deal with orchestration—instead relying on attaching seamlessly to an already-running browser instance.
