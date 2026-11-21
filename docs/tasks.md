<!-- agent-updated: 2026-11-21T05:29:00Z -->

# Open tasks

## Chromium compatibility scope and ordered API backlog

Firefox and WebKit support is intentionally excluded. Their launchers,
protocols, browser fixtures, engine-specific semantics, and conformance cases
are non-goals and must not be counted as missing features. The compatibility
denominator is the public Chromium API in pinned upstream Playwright 1.58.0.
Static method-name overlap is only inventory evidence; a row closes only after
its Rust owner, optional binding profile, focused tests, and source-bound
upstream behavior are verified.

Current reproducible static baseline against clean pinned Playwright 1.58.0:
Node object members 382/479 (79.7%), declared event names 12/37 (32.4%), and
sampled option fields 32/62 (51.6%); Python async and sync are each 399/493
(80.9%). The regenerated Core inventory contains 48,477 rows, of which 3,199
public-API candidate invocations across 225 files are in the `library`
Chromium slice. This is not a behavioral pass rate; the complete 3,199-row
candidate slice has not been freshly executed. Reproduce with
`scripts/compare_playwright_public_api.py` and see
[`knowledge/feature-comparison.md`](knowledge/feature-comparison.md).

Use the measured gaps in this order while retaining the detailed dependency
order below:

1. Finish unchanged-upstream verification for Context-owned Service Worker
   network events. Rust, Python, the Node Worker/Request/Response facade, and
   focused real-Chrome verification are now present.
2. Close BrowserContext event, route/HAR/WebSocket-route, background-page,
   Clock, Tracing, storage, cookie, credential, and option gaps.
3. Close missing auxiliary objects and Page events: Worker, Download,
   FileChooser, WebSocket/WebSocketRoute, Video, Coverage, and WebError.
4. Close Frame and ElementHandle gaps, then Request/Response metadata and
   lifecycle gaps.
5. Execute the unchanged 3,199-row Chromium public-library slice in bounded
   shards and publish complete candidate/Oracle receipts.

- [x] Remove the legacy exported `firefox`/`webkit` BrowserType placeholders
  from the Node CommonJS, ESM, and TypeScript public surfaces. Node launch and
  CDP dispatch now have only a Chromium provider, and contract/type tests prove
  the other names are absent. Historical device-descriptor
  `defaultBrowserType` metadata remains data only.
- [x] Finish the non-Chromium cleanup behind the Node public boundary. Browser
  registry, install/backend manifests, BiDi startup, persistent-context
  preflight, connect-header admission, and active remote-connect adapter tests
  are Chromium-only. Official Playwright Server bootstrap fixtures retain the
  required three-engine initializer graph only as non-selectable wire data;
  historical pinned receipts remain inert evidence.
- [x] Remove the legacy exported `firefox`/`webkit` BrowserType placeholders
  and engine-specific launch paths from the Python sync/async public package.
  Direct BrowserType construction is Chromium-only; 663 unit tests, Ruff, and
  strict Pyright pass after deleting the obsolete non-Chromium process-startup
  facade cases.
- [x] Remove the now-unreachable Firefox/WebKit registry, process-startup, and
  engine-selection ports from the Python native binding and shared Rust
  registry/install/backend leaves. Remote-connect admission accepts only
  Chromium. Historical manifests and the official Server initializer may
  retain engine names as evidence/wire data but cannot create providers.
- [x] Complete the Chromium Node `Page.pdf()` option surface. The default-off
  `page-pdf` profile keeps protocol dispatch, streamed output, cleanup,
  recursive output-directory creation, file persistence, unit/format parsing,
  and result allocation in Rust. All pinned 1.58.0 public options are typed and
  mapped: headers/footers, format or dimensions, landscape, margins, ranges,
  CSS-page preference, backgrounds, scale, tagging, outline, and path. The
  source-bound real Chrome/CDP E2E covers both upstream Chromium PDF behaviors
  (save and tagged outline) plus the complete option payload.
- [x] Make allocator-backed topology compatible with attach preflight: runtime
  ownership, Page routes, evaluation/DOM/Locator context routes, and event
  cursor fanout no longer multiply `usize::MAX` sentinels into fictitious
  up-front backend-state reservations. Focused release tests and the real
  `Page.pdf()` attach exercise cover this regression.
- [ ] Complete Chromium `BrowserContext` parity. Firefox/WebKit-only properties,
  events, fixtures, and protocol behavior are excluded from this denominator.
  - [x] Add async/sync runtime `set_geolocation()`, `grant_permissions()`, and
    `clear_permissions()` through Rust-owned CDP state. Grants merge per
    normalized origin using the pinned Chromium permission mapping; there is no
    fabricated public permission-count limit. A real Chrome 157/CDP E2E reads
    the exact overridden coordinates and observes `prompt` after clearing.
  - [x] Widen `new_context(permissions=...)` and persistent-Context creation
    from the legacy geolocation-only boolean to the complete Chromium mapping.
    Rust validates and deduplicates before native creation. A real Chrome 157
    Context created with geolocation and camera reports both as `granted`;
    persistent defaults reuse the same Rust value and mapping path.
  - [x] Align the Chromium locale value domain with pinned Playwright 1.58.0.
    Node, Python, and Rust no longer impose the retired 256-byte, control-
    character, nonempty, or 16-live-Context policy. Locale strings are retained
    in allocator-backed ownership, FFI admission accounts for their observed
    UTF-8 length, and an empty string is a no-op at Chromium initialization.
    The independent Node `context-locale` profile, Python ordinary and
    persistent Context paths, a 4,096-byte regression, seven Rust ownership/
    payload tests, 956 Node tests, and 713 Python tests pass.
    - [x] Compose the Node locale leaf with device emulation, extra headers,
      proxy, offline, HTTPS-error, and CSP policies through one typed native
      Context-creation port. The facade validates every selected capability,
      then performs one Rust-owned atomic Context creation; the extra-header
      variant uses the same port after exact header conversion. Full and
      no-header adapter regressions pass, a release check covers all seven
      native features together, and the rebuilt `context-locale` addon passes
      the native export smoke. A live-Chromium all-options proof was not run in
      this change and remains separate conformance evidence.
  - [x] Align the Chromium timezone value domain with pinned Playwright 1.58.0.
    Node, Python, and Rust no longer impose the retired 256-byte, control-
    character, nonempty, or 16-live-Context policy. An empty identifier is a
    no-op; every nonempty identifier is forwarded exactly and Chromium remains
    the authority that rejects an invalid timezone. Allocator-backed ownership
    and FFI admission account for the observed UTF-8 bytes. The ordinary Node
    `context-timezone` leaf is independently selectable and composes through
    the generic Rust policy port. Four focused Rust tests, the rebuilt native
    export smoke, 956 Node tests, and 713 Python tests pass; no live-Chromium
    timezone proof was run in this change.
  - [x] Expose Chromium geolocation in the Node facade without duplicating its
    owner in JavaScript. `Browser.newContext({ geolocation })` and
    `BrowserContext.setGeolocation()` use the existing allocator-backed Rust
    policy; omitted accuracy defaults to zero, `null` clears the override, and
    Rust validates finite longitude, latitude, and accuracy ranges. The
    independent `context-geolocation` addon, strict release Clippy, native
    export smoke, strict TypeScript, and 956 Node tests pass. No new live-
    Chromium geolocation proof was run in this change.
  - [x] Expose Chromium permission overrides in the Node facade through the
    existing Rust owner. `Browser.newContext({ permissions })`,
    `grantPermissions(permissions, { origin })`, and `clearPermissions()` use
    the complete Chromium mapping, merge grants per normalized origin, and
    retain no JavaScript permission mirror. Empty creation permissions are a
    no-op; non-string entries and invalid origins fail before browser work.
    The independent `context-permissions` profile, both release feature checks,
    strict TypeScript, and 956 Node tests pass. A generic policy port now also
    rejects any unselected leaf instead of silently discarding its input.
  - [x] Expose creation-time and deprecated runtime Chromium HTTP credentials
    in the Node facade through the existing Rust authentication owner. Ordinary
    `Browser.newContext({ httpCredentials })` supports username, password,
    optional origin, and the pinned `unauthorized`/`always` send policy. The
    Playwright-compatible `BrowserContext.setHTTPCredentials()` replaces or
    clears the retained value, then refreshes current Page/OOPIF and Service
    Worker sessions through the same composable Fetch owner; it adds no JS
    credential mirror. The runtime update gate also serializes new Page/OOPIF
    initialization against replacement and clear. The independently selectable
    `context-http-credentials` profile and the
    generic composition port retain no JavaScript credential or challenge
    state. Strict types, both release Clippy profiles, thirteen focused Rust
    owner tests, all 967 Node tests, and actual native loading pass.
    No live-Chromium Node authentication proof was run in this change.
  - [x] Expose `BrowserContext.exposeBinding()` and `exposeFunction()` through
    the existing Rust binding owner. The `context-bindings` addon profile
    composes `page-bindings` and Handles instead of adding another driver,
    event cursor, or protocol registry. Rust installs Context registrations in
    current and future Page/OOPIF sessions, enforces exact Playwright
    Context/Page duplicate diagnostics, and retires them with the Context.
    Node retains only callback identity and resolves a call first against its
    Page registry, then its Context registry. Thirty-nine focused Rust tests,
    strict release Clippy, the real native ABI/load smoke, strict TypeScript,
    dependency-boundary verification, and all 973 Node tests pass. A real
    Chrome 157/CDP run covers an existing Page, a future Page, source identity,
    and both conflict directions. The rebuilt full official profile also fixes
    a dependency-feature-unification regression by routing locale creation
    through the stable policy port. The unchanged pinned Playwright 1.58.0
    `browsercontext-expose-function.spec.ts` then passes all 6/6 cases through
    the candidate adapter: binding source, returned handle values, duplicate
    diagnostics, init-script calls, handle mode, and CSP.
  - [x] Close the fixed Chromium BrowserContext cookie slice in Node. Rust owns
    `cookies()`, `addCookies()`, `clearCookies()`, storage-state restore/export,
    partition-key projection, and filtered matching. The ordinary network
    profile no longer rejects a ninth Context or Page adapter. Non-video
    `launchPersistentContext()` now performs a real Rust-owned profile launch,
    publishes exactly one default Context, and composes
    `ignoreHTTPSErrors`. Two final source-bound runs of the pinned Playwright
    1.58.0 four-file manifest each record 55 passed, one upstream skip, and
    zero unexpected across 56 tests. This closes only those four files, not the
    full 3,199-row Chromium library slice.
  - [ ] Finish the remaining persistent-Context option surface. Fail closed for
    options that are not composed yet; carry the same Context policy contract
    through video persistent launch before claiming recording parity.
    - [x] Carry `locale` through ordinary and video persistent Chromium launch.
      A nonempty value selects one typed Rust defaults port, an empty value is
      a no-op, and a missing leaf fails closed before launch. The independent
      `persistent-context-locale` profile, a 4,096-byte adapter regression,
      minimal and video-combined release checks, 956 Node tests, strict types,
      and the rebuilt native export smoke pass. No live-Chromium persistent
      locale proof was run in this change.
    - [x] Carry `timezoneId` through ordinary and video persistent Chromium
      launch. It shares the typed Rust defaults object and value-domain rules:
      an empty string is a no-op, a nonempty string selects the independent
      `persistent-context-timezone` leaf, and a missing selected leaf fails
      closed before launch. The standalone leaf passes strict release Clippy;
      the rebuilt combined `persistent-context-policies` addon composes a
      4,096-byte value with locale, offline, CSP, JavaScript and HTTPS policy
      and passes its native export smoke. No live-Chromium timezone proof was
      run in this change.
    - [x] Carry `geolocation` through ordinary and video persistent Chromium
      launch using the same typed defaults object. The independent
      `persistent-context-geolocation` leaf composes the ordinary runtime leaf;
      missing selected capability fails before launch. Standalone and combined
      `persistent-context-policies` release artifacts both load and pass the
      native ABI smoke; the combined profile retains no JavaScript coordinate
      mirror. Live-Chromium persistent geolocation remains separate evidence.
    - [x] Carry nonempty `permissions` through ordinary and video persistent
      launch with the same Rust `ContextPermissions` value used by ordinary
      creation and runtime grants. The independently selectable
      `persistent-context-permissions` profile composes the ordinary owner;
      empty permissions remain a no-op and a missing selected leaf fails before
      launch. The standalone ordinary/persistent artifacts and rebuilt combined
      policy artifact pass native ABI load; 12 Rust owner tests cover the full
      mapping, per-origin merge, reset, persistent defaults, allocator growth,
      and release. Live-Chromium Node permission proof remains separate.
    - [x] Carry `httpCredentials` through ordinary and video persistent launch
      with the same Rust `ContextHttpCredentials` value used by ordinary
      Context creation. The independent
      `persistent-context-http-credentials` profile composes its ordinary
      owner; username, password and optional origin bytes are charged before
      launch, and a missing selected leaf fails closed. Ordinary, persistent,
      and rebuilt combined-policy artifacts all pass native ABI loading. No
      live-Chromium Node authentication proof was run in this change.
    - [x] Carry `offline`, `bypassCSP`, and `javaScriptEnabled: false` through
      the same typed persistent defaults object. Each policy remains an
      independently selectable native leaf; its no-op value does not require
      the leaf, and a selected value fails closed before launch when absent.
      The combined `persistent-context-policies` profile atomically composes
      all five defaults plus `ignoreHTTPSErrors`, and its rebuilt native export
      smoke passes. Minimal strict Clippy, the complete video combination,
      strict types, and 956 Node tests pass. Live-Chromium behavior remains a
      separate conformance proof.
  - [ ] Add Chromium background-page and service-worker collection/event
    surfaces. Do not add Firefox/WebKit implementations to satisfy the public
    inventory.
    - [x] Add an independently selectable `service-worker-events` leaf. Rust
      retains browser-root `service_worker` targets only when selected, owns
      them by BrowserContext, publishes create/close notices, and exposes
      async/sync `service_workers`, `serviceworker`, and scalar `evaluate()`.
    - [x] Add Context-owned Handle and console ownership for service workers.
      `evaluate_handle()` and console argument Handles retain the exact Worker
      CDP owner without borrowing a Page. Service Worker console messages have
      `page=None`, preserve Worker/Context message identity, and fan out only
      to Pages inside the worker scope. Rust records them only while a Worker,
      Context, or related Page console subscription is active; the legacy
      128-notice/16-MiB public queue limits are removed.
    - [x] Preserve upstream's Service Worker exception boundary. Pinned
      Playwright 1.58.0 does not subscribe `Runtime.exceptionThrown` in
      `CRServiceWorker`, does not expose a Worker error event, and does not
      fabricate a Page for a Service Worker exception. The selected
      `worker-error-events` leaf remains dedicated-Worker-to-Page `pageerror`
      attribution only; a Service Worker with no Page is intentionally ignored.
    - [ ] Complete Context-owned service-worker network ownership. Pinned
      Chromium creates a separate network manager per Service Worker; it does
      not borrow an arbitrary Page lane. Implement the following in order:
      - [x] Add a BrowserContext-owned native network lane and publish Service
        Worker request/response/finished/failed events only to the Context.
        Expose the exact `Request.service_worker` owner, keep `Request.frame`
        unavailable, and preserve `Response.from_service_worker` for the
        separate Page request satisfied by a Service Worker.
        Implementation is in progress: the independent Context adapter,
        pre-resume registration hook, exact Worker/Context payloads,
        Context-only Python event fanout, redirect identity cache, and
        Context-owned request/response history are present. The minimal
        `worker-network-events` profile now selects that history leaf directly
        instead of relying on the full wheel's unrelated feature list. Focused
        release tests, the rebuilt ABI138 wheel, the dedicated-Chrome live test,
        and Node adapter tests pass. The Node binding now exports exact Worker
        owners, Context-only request/response/finished/failed fanout,
        `Request.serviceWorker()`, the official `Request.frame()` exception,
        `Response.fromServiceWorker()`, snapshots, close events, and scalar
        Worker evaluation. Its release native profile loads with all required
        ports. The repository-owned Node live exercise passes against Chrome
        157 and the dedicated CDP profile with exact event/snapshot/request/
        response identity, Context-only fanout, scalar evaluation, and clean
        native teardown after finalization. The unchanged upstream cases remain
        tracked separately below.
      - [x] Apply BrowserContext offline state to Service Worker sessions.
        Service Worker creation now enables its independent Network domain even
        while the Context starts online, then applies the same legacy
        `Network.emulateNetworkConditions` payload as pinned Playwright
        1.58.0 on later toggles. The repository-owned Node exercise passes
        against Chrome 157: Page and Service Worker fetches reject offline,
        the Worker observes `navigator.onLine === false`, and fetch recovers
        after returning online.
      - [x] Apply BrowserContext extra HTTP headers to Service Worker sessions.
        Creation installs the Context-only effective headers after
        `Network.enable` and before resume; runtime Context updates fan out to
        Pages first and then each current Service Worker, matching pinned
        Playwright 1.58.0 without inheriting Page-level headers. Obsolete-session
        reconciliation now treats a retained Service Worker as live instead of
        clearing its headers after the next CDP event. The repository-owned Node
        exercise passes against Chrome 157 for both creation-time headers and
        replacement by a later Context update.
      - [x] Add the missing public Node BrowserContext `route()`, `unroute()`,
        and `unrouteAll()` binding surfaces over the existing Rust Context
        registration owner. String and RegExp registrations retain Context
        ownership; Page handlers take precedence for Page requests; exact
        handler removal and all three `unrouteAll` behaviors are implemented.
        `wait` drains active callbacks before Rust updates CDP interception, as
        pinned Playwright 1.58.0 does. Adapter tests pass 53/53, the minimal
        `page-unroute` and `route-regexp` artifacts native-load independently,
        and a repository-owned Chrome 157 exercise verifies Context RegExp,
        Page-over-Context precedence, exact unroute, wait ordering, and clean
        native teardown. This is focused evidence, not unchanged-upstream
        conformance.
      - [x] Apply BrowserContext routes to Service Worker sessions. Pinned
        Playwright 1.58.0 enables `Fetch` on the Worker's independent network
        manager and dispatches only `browserContext.requestInterceptors`; Page
        routes must not intercept requests initiated by the Service Worker.
        Preserve the Context/Worker request owner instead of fabricating a Page
        ID for the route callback. The Rust route owner now installs Context-only
        patterns before Service Worker resume and updates already-live Worker
        sessions after Context route mutations. Active Worker session ownership
        is allocator-backed instead of sharing the 64-entry Page-initialization
        table. Optional Page IDs and exact Worker IDs cross the Node FFI; adapter
        tests prove Page handlers cannot receive Worker requests. The independent
        `service-worker-routes` profile and native exports load cleanly. A
        repository-owned Chrome 157 exercise covers both an already-active
        Worker updated at runtime and a Worker configured before debugger
        resume. Each receives exactly one Context fulfillment (`201`/`202`)
        with zero Page-route hits. This is focused evidence, not
        unchanged-upstream conformance.
      - [x] Apply BrowserContext HTTP credentials to Service Worker sessions.
        Service Worker attachment now records the exact Context/session before
        debugger resume and composes one wildcard
        `Fetch.enable(handleAuthRequests=true)` with Context routes instead of
        installing competing Fetch owners. Auth challenges use the exact
        Worker session, preserve credential-origin matching, and cancel the
        repeated challenge for a request after one credential attempt. The
        live Worker registry is allocator-backed rather than capped by the
        Page-initialization table. Console and Page-binding observer wrappers
        forward all Service Worker capabilities so optional features remain
        composable. Focused Rust release tests pass 12/12, composition tests
        pass 5/5, strict combined-feature Clippy passes, and a rebuilt Python
        addon passes a dedicated Chrome 157 HTTPS exercise: the protected
        Worker fetch challenges once then succeeds, while the always-rejected
        fetch challenges twice and returns 401. This is focused evidence, not
        unchanged-upstream conformance.
      - [ ] Complete Context HAR capture for Service Worker traffic.
        - [x] Add the independently selectable `har-recording` leaf and host
          adapter. Context recording now includes Page and Service Worker
          fetch lifecycles once, pulls ordinary bodies only after the exact
          terminal CDP event on the exact session, and retires correlations
          when the Worker closes. Explicit Python
          `route_from_har(update=True, update_content="embed")` writes on
          Context close. A real Chrome 157 trusted-HTTPS capture contains both
          `page-response` and `worker-response`, and the same Rust library
          replays both while the origin data endpoints return 503.
        - [x] Implement upstream-default attachment persistence and ZIP replay.
          Content uses SHA-1 plus MIME-extension names with per-recording
          deduplication. Plain HAR resources live beside the HAR; ZIP output
          uses root `har.har` plus resources. Host tests pass both layouts, and
          rebuilt ABI139 passes two dedicated Chrome 157 capture/replay runs
          using only `route_from_har(path, update=True)`: Page and Service
          Worker bodies replay from both `.har` and `.zip` after the origin
          data endpoints switch to 503.
        - [x] Align the implemented minimal HAR document shape with pinned
          Playwright 1.58.0: `time`, request/response body sizes, and response
          content size retain upstream `-1` sentinels; POST bodies use HAR
          `PostData` with form params; font responses stay base64 despite a
          textual server MIME; creator version and all 956 preferred MIME
          extensions come from the pinned upstream data. Policy tests pass
          10/10.
        - [x] Remove replay-only default quotas absent from upstream. Archive,
          entry, field, header, POST/response-body, retained-byte, and redirect
          limits now default to unlimited while remaining available as an
          explicit embedder policy. Release replay tests pass 11/11, including
          a response beyond the former 1 MiB cap and a chain beyond the former
          20-redirect cap. The host also removes its private 4 KiB HAR-path
          ceiling and lets the filesystem determine valid path lengths.
          Recorded statuses are returned unchanged instead of rejecting the
          whole archive before route fulfillment.
        - [x] Remove the host's remaining eight-HAR and aggregate 8 MiB
          registration gates, its static 8 MiB pseudo-reservation, and the
          three stale Python capability-report keys. A host regression grows
          the allocator-backed registration vector past eight HAR handlers.
        - [x] Match upstream lazy replay-body acquisition. Archive open retains
          content descriptors without reading attachments or decoding bodies;
          lookup loads only matching POST data and the selected response. The
          host reopens plain/ZIP resources through a thread-safe capability and
          runs lookup in a blocking worker. Lookup errors now follow the
          configured not-found policy. Replay policy tests pass 12/12 and
          focused host HAR tests pass 8/8 under strict Clippy.
        - [ ] Capture Service Worker main/import script requests if Chromium
          exposes them through an appropriate target event; the focused run
          did not publish the top-level `sw.js` install request through the
          Context network lane.
        - [x] Implement full-mode metadata for explicit HAR updates. The
          backend-neutral policy now emits Playwright-shaped pages, cookies,
          raw headers, protocol/server/TLS details, redirect targets, timing,
          sizes, compression, and transfer size. The host uses an injected
          wall clock for opaque `page@...` entries, observes new Pages, and
          captures title plus DOMContentLoaded/load timing on the exact Page
          CDP session. Release policy tests pass 15/15, focused host tests pass
          5/5, strict Clippy passes, and a rebuilt Rust-backed Python wheel
          captures and replays Page plus Service Worker bodies in full mode on
          Chrome 157. A focused official 1.58.0 Oracle run has the same page,
          entry, request, response, timing, server, and TLS field sets. This is
          focused evidence, not unchanged-upstream conformance.
        - [x] Add Context-creation `record_har` in both language bindings. The
          Python async/sync facade accepts the official five creation options;
          the Node facade accepts the official `recordHar` object. Both use
          upstream precedence/defaults (`content` over `omitContent`, full
          mode, `.zip` attachment and plain-HAR embedding), install recording
          before publishing the Context, and close an unpublished Context if
          initialization fails. Node has lean `har-recording` and composed
          `official-conformance-page-har-recording` profiles. Focused Node
          tests pass 8/8 plus native-load and strict TypeScript; Python units
          pass 688 with 9 skipped. Rust-backed Chrome 157 HTTPS E2Es pass for
          both Node `browser.newContext({ recordHar })` and Python
          `browser.new_context(record_har_path=...)`, including Page/Worker
          capture and replay. The network poll batch is boxed so an empty poll
          no longer carries the largest 216-byte enum layout; its composition
          release suite passes 431 with 16 ignored, and the complete Node HAR
          profile passes strict Clippy.
        - [x] Run the fixed unchanged Playwright Python v1.58.0 creation-time
          HAR slice in both lanes. Its source-pinned manifest selects 10 async
          and 9 sync Chromium cases; candidate and Oracle each pass 19/19.
          ABI140 adds `har-recording-regexp` as a separate native capability,
          full mode records no-POST request `bodySize=0`, and lean/composed
          Node native artifacts both load with the exact expected exports.
        - [ ] Promote those local XML/provenance sidecars into the immutable
          conformance ledger. The current reports remain under
          `.work/har-full-live.psmZdi`; they do not close the complete 6,526-row
          Python corpus or the Core Node/TypeScript HAR suites.
        - [x] Replace the Rust `regex` matcher used by HAR, routes, and cookie
          clear filters with the optional Rust-native
          `playwright-rust-ecmascript-regexp` leaf. It supports lookarounds,
          backreferences, UTF-16 indexing, Unicode sets, all current JavaScript
          flags, and `g`/`y` `lastIndex`; arbitrary 32-route, 4 KiB route-source,
          string/glob route-source, and 16 KiB cookie-filter limits are removed.
          Route matchers retain independent server/client state, reproducing
          the pinned source-derived `/g` sequence. Focused Rust tests pass
          31/31, the Node route adapter passes 53/53, the full Node suite passes
          845/845, the rebuilt Python package passes 688 with 9 skipped, exact
          Node/Python addon compositions release-check, and the fixed official
          Python HAR candidate remains 19/19. The rebuilt Node HAR-profile
          addon is SHA-256 `2cff1e6714a21d5a23deac1a8e9061703af107779fbbeffb512945c6c35313d7`;
          the rebuilt Python abi3 module is SHA-256
          `4d653dcca9e944935a98de8153c01f039c671097c20a35f94ff51b1ce171fbdd`.
        - [x] Add a source-pinned Test262/Node differential corpus for the
          transportable `RegExp.test()` surface. Test262 commit
          `3c6180f581e5f468d6767af2f9b1f9c3ca444e77` is the last main commit
          before Playwright v1.58.0's tag; an isolated Node 24.12 oracle extracts
          5,135 static RegExp literal/constructor cases and records 97,198
          match/`lastIndex` steps. The Rust leaf passes the compressed corpus;
          generator output is byte-for-byte reproducible at SHA-256
          `90b348d0c0bc63cca448976a3bfd95299c2a56b158b68b5181c82a40ac85c3f6`.
          This work found and fixed raw non-Unicode astral literals, astral
          named-group identifiers, unpaired-surrogate input, non-Unicode
          ignore-case word atoms, duplicate named-group backreferences, and
          Unicode data drift. The vendored `regress` source is pinned to
          `95c04041` with tables regenerated from official Unicode 16.0 inputs,
          matching the Node baseline. Route 49/49, cookie 10/10, and HAR 8/8
          host regressions remain green.
        - [ ] Complete the remaining ECMAScript RegExp proof. The committed
          corpus does not execute every dynamically constructed Test262 program.
          A real-browser upstream route oracle also remains blocked because
          Playwright 1.58 cannot attach to dedicated Chrome 157:
          `Browser.setDownloadBehavior` reports unsupported browser-context
          management. Until both gaps close, do not claim blanket all-version
          RegExp parity.
    - [x] Verify the service-worker list/event/evaluate path against real
      Chrome 157 over the dedicated CDP profile. The Rust host passes both
      consumed- and unconsumed-notice cases, scalar evaluation observes the
      fixture marker, Python preserves event/snapshot identity, console event
      identity/page/argument semantics and Context-owned `evaluate_handle()`
      pass through ABI137, and close returns with a zero resource ledger. The
      Handle/console path is now also a repository-owned ignored live test
      using the exact minimal service-worker feature composition.
    - [ ] Run the pinned upstream Chromium service-worker cases unchanged
      against the built package and publish candidate/Oracle receipts. The
      focused real-browser E2E above is not a full upstream conformance claim.
      - [x] Re-run all seven cases already covered by the source-pinned Python
        Service Worker manifests against the current ABI140 module. Candidate
        and official Playwright 1.58.0 Oracle each pass 2/2 sync network,
        4/4 async/sync policy, and 1/1 Page-route precedence cases under the
        trusted Tailscale HTTPS fixture. Every case ran in its own process and
        all eight result provenance sidecars verify independently.
      - [ ] Inventory and execute the remaining Chromium Service Worker cases
        from the full Python corpus and the pinned Core Node/TypeScript suite.
        The canonical Python inventory contains 14 matching Chromium IDs.
        All 14 now pass in both current candidate and Oracle lanes: the
        original seven, one navigation and two async-network cases, plus the
        four async/sync extension cases. The Page response
        satisfied by a Service Worker exposed a candidate-only body-read gap;
        ABI140 now carries the exact native request ID through FFI and derives
        the response-body route from its Page/Frame-owned Request. True
        Service-Worker-owned responses without Page/Frame ownership remain
        unsupported. Candidate and Oracle each pass the new three-case slice
        3/3 with verified provenance. The extension pair required bundled
        `chromium` channel selection, upstream component-extension isolation,
        persistent-owned teardown, and the deprecated always-empty
        `background_pages` surface; both lanes pass 2/2 with verified
        provenance. The final video pair now also passes 2/2 in candidate and
        Oracle owned-launch lanes. Rust owns CDP screencast capture, frame
        acknowledgement, pinned `ffmpeg-1011`, WebM naming, and close-time
        finalization; the repository live E2E verifies a nonempty VP8 640x360
        file. The Core inventory's 293 raw text
        matches mix Android, BiDi, Firefox/WebKit, and repeated projects, so a
        project-aware Chromium filter is required before declaring its
        denominator. The fourteen green Python rows are a scoped checkpoint, not
        completion.
    - [x] Match the pinned extension background-page surface. Playwright 1.58.0
      deprecated the event and its Chromium client returns an empty
      `background_pages` list, so the compatibility implementation returns
      `[]` in async and sync facades rather than fabricating target ownership.
    - [x] Add composable Chromium video capture and encoding. The default-off
      Rust leaf owns screencast capture, frame acknowledgement, ffmpeg/process
      ownership, WebM naming, and finalization before browser shutdown. Python
      wires `record_video_dir` and `record_video_size`; a repository live E2E
      proves a real VP8 640x360 file, and the final candidate/Oracle Service
      Worker rows pass 2/2 with verified candidate provenance.
    - [ ] Complete public Video artifact lifecycle parity across all binding
      modes and binding surfaces.
      - [x] Expose persistent-context Python `Page.video` plus async/sync
        `Video.path()`, `save_as()`, and `delete()`. Rust publishes the path
        immediately, waits for encoder completion before copy/delete, keeps
        artifacts alive after Page/Context close, creates destination parents,
        preserves a failed deletion for retry, and makes successful deletion
        idempotent. Sync `save_as()` rejects an open Page with Playwright's
        exact error; async `save_as()` may wait across Context close. The final
        candidate module and official Oracle each pass the same three unchanged
        persistent-video cases 3/3 with independently verified provenance; the
        repository async/sync E2E also validates copied VP8 640x360 files and
        deletion.
      - [x] Add non-persistent owned `Browser.new_context()` and implicit
        `Browser.new_page()` recording. Rust registers exact per-Context
        options, prevents persistent defaults from leaking into other
        Contexts, and finalizes only the closing Context's recorders. All 11
        unchanged async/sync Python video cases pass in the installed ABI141
        candidate. The independently verified Playwright 1.58.0 Oracle run is
        9/11 on bundled Chromium 145: its two failures are both official
        `Video.save_as()` calls rejected as `Page closed` after Page close.
        Repository E2E covers async/sync persistent and implicit modes with
        valid VP8 640x360 output.
      - [x] Add the local Node Video facade for ordinary owned Contexts and
        `Browser.newPage()` implicit Contexts. `Page.video()` exposes Rust-owned
        path/copy/delete artifacts; native E2E proves 640x360 and default
        800x450 VP8 files, save-before-close waiting, idempotent delete, and
        zero retained leases.
      - [x] Execute the complete unchanged pinned Core `video.spec.ts` file
        through a source-bound manifest. The final candidate run is 24 passed,
        five upstream-declared fixmes skipped, and zero failed. Four skips are
        selected by the full-Chromium classification; the internal-page row is
        unconditionally fixme. The manifest pins `channel: chromium` because
        the registry executable is full Google Chrome for Testing rather than
        headless shell. The composed
        profile's browser-death row performs a real Rust-owned process kill;
        the terminal frame-ack teardown race is covered by an exact error
        classifier instead of suppressing arbitrary CDP failures.
      - [ ] Add remote artifact transfer semantics and Node persistent-context
        recording after the broader Node persistent-context surface exists.
    - [x] Complete the pinned Core Context Tracing file as independent leaves.
      - [x] Add the screenshot-only trace leaf. Rust captures a real JPEG at
        stop and writes a Playwright-v8 ZIP with `trace.trace` and the matching
        SHA-1 resource; the JS facade supports `start({ screenshots })`, ZIP
        export through `stop({ path })`, and Rust-owned discard through
        pathless `stop()`. Pathless stop before start is an upstream-compatible
        no-op, while exporting without start retains the lifecycle error.
      - [x] Pin and execute the complete unchanged Core `tracing.spec.ts` file.
        Its 35-case RED baseline was 3 passed / three upstream fixmes skipped /
        29 failed. Two consecutive rebuilt candidates are 32 / 3 / 0 with
        exact result, source, manifest, harness, and artifact provenance.
      - [x] Add the independent chunk/group leaf. Rust owns `startChunk`,
        optional chunk names/titles, `stopChunk`, nested `group`/`groupEnd`,
        parent IDs, automatic group close, export, and discard. Remove the
        private 1 MiB total, 8 KiB record/path, and 1 KiB title ceilings from
        this public path; retain byte-proportional FFI admission, fallible
        allocation, and checked accounting.
      - [x] Add Rust-owned `tracing.start({ title })`; the initial chunk writes
        it into `context-options.title` without a private title byte ceiling.
      - [x] Add the first independent action/API leaf. Rust owns call IDs and
        titles for Page route/navigation/set-content/click/double-click/
        evaluation/evaluation-handle/generic event wait/reload/close, Route
        continue, Dialog accept, and BrowserContext storage state, including
        primitive results, URL path/query titles, and pending-action lifecycle. The
        unchanged official callback-title and internal-page cases are green;
        this is not complete action tracing.
      - [x] Add an independently selectable Node
        `BrowserContext.storageState({ indexedDB })` read leaf backed by the
        existing Rust host. Unsupported path persistence remains fail-closed.
      - [x] Add independently selectable Rust-owned trace console events. The
        existing CDP observer records Page ownership, arguments, and location;
        the unchanged official stop-time console-flush case is green. Completed
        actions flush named live files while console-only events stay buffered,
        matching the unchanged official live-console row.
      - [x] Add independently selectable global `APIRequestContext` tracing.
        Rust spools typed `resource-snapshot` JSONL and SHA-1 content resources
        per request context, then packages `trace.network`; the official basic,
        separate-context, and POST-data cases are green.
      - [x] Add independently selectable BrowserContext trace network capture.
        Rust reuses the hidden HAR recorder, snapshots network state per chunk,
        omits JavaScript response bodies, and packages `trace.network` plus
        resources; the unchanged network-failure row is green.
      - [x] Implement named live `.trace`/`.network` files under launch
        `tracesDir`, overwrite behavior, valid `trace.stacks`, pending actions,
        browser-close teardown, and concurrent export during navigation. The
        corresponding unchanged official rows are green.
      - [x] Collect source files per chunk. Node captures only the external
        callsite while `sources` is enabled; Rust deduplicates paths, reads
        files, computes official path SHA-1 names, writes ZIP resources, and
        starts each chunk with an empty source set. Both unchanged source rows
        are green.
      - [x] Bracket mouse move/double-click, keyboard insert-text,
        set-input-files, wait-for-timeout, and Page/Frame hover actions; keep
        internal dialog auto-dismiss out of the public trace.
      - [x] Route BrowserContext-owned API requests through the exact active
        Context trace action and Rust HAR registration without changing the
        independently selectable global request recorder.
      - [x] Write Playwright-compatible `trace.stacks` call-ID/file tables for
        ordinary actions from the first external Node callsite; internal frames
        are filtered before crossing FFI.
      - [x] Add DOM/action frame snapshots for resource, multi-chunk,
        iframe-head, and screenshot-result rows; add expect-title execution and
        tracing; preserve upstream action snapshot and stack semantics. The
        complete pinned file is 32 passed / three upstream fixmes skipped /
        zero failed in two consecutive runs. Continuous screencast-frame rows
        remain upstream full-Chromium fixmes.
  - [ ] Add the independently selectable Clock leaf.
  - [ ] Add Chromium WebSocket routing (`route_web_socket`).
  - [ ] Finish remaining context options, storage/session state, and lifecycle
    edge cases, each with source-bound Chromium conformance receipts.
- [ ] Complete `Frame`, `Locator`, and `ElementHandle` parity, especially
  frame/handle adoption, actionability, selector semantics, detached-object
  behavior, and unsupported option coverage.
- [ ] Complete network parity: Request/Response fields and timing, routing and
  fulfillment options, downloads, uploads, WebSocket/event streams, HAR, and
  service-worker interactions.
- [ ] Complete Page event and auxiliary-object parity: popups, dialogs,
  downloads, workers, console/page errors, bindings, video, and teardown.
- [ ] Complete launch/connect and process ownership for Chromium without
  bundling a mandatory Node driver or browser binary; browser acquisition stays
  an explicit optional package.
- [ ] Finish tracing, clock, coverage, accessibility, and remaining CDP-session
  surfaces as independent leaves rather than default dependencies. Tracing now
  has screenshot, chunk/group, action, storage-state, console, and global
  request-tracing leaves plus BrowserContext network capture and named live
  files. Rust now retains every start option and records pending action
  lifecycle, per-chunk sources, Context-owned API requests, and ordinary action
  stacks; its complete pinned Core file is 32 passed / three upstream fixmes
  skipped / zero failed. This closes that source-bound tracing file, not the
  full upstream corpus.
- [ ] Decide whether Playwright Test, assertions, reporters, fixtures, and code
  generation are separate packages or explicit non-goals; they are not part of
  the current library compatibility claim.
- [ ] Run the complete pinned upstream Chromium Node/TypeScript and Python
  suites against built packages, publishing pass/fail/skip/not-run receipts.
  Local adapter tests and API inventories do not establish full compatibility.
- [ ] Track the external Pwright CLI cleanup discrepancy observed during HAR
  conformance: `tab-close <raw CDP target id>` reported success for an
  unmanaged target but left that target open. Selecting the raw target through
  task-local `.pwright/state.json` and navigating it to `about:blank` worked.
  This is test-tooling debt, not a Playwright Rust browser API claim.

## Upstream compatibility limit audit

The 2026-11-14 static audit compares the production Rust host and Node/Python
bindings with pinned upstream Playwright 1.58.0. A resource budget may bound
internal retained bytes or apply backpressure, but a Playwright-compatible
profile must not reject otherwise valid public behavior at a fabricated object
count or result-size threshold that upstream does not have.

- [x] Remove the 16-session Node/Python environment admission limit. Omit the
  finite Python capability entry and prove at least 129 simultaneous pending
  admissions without `ResourceExhausted`. ABI132 release installation and all
  302 Python contract tests pass; selected Node/Python remote-session Clippy is
  warning-free. The Node publication-port registry is allocator-backed too: a
  129-owner regression crosses its hidden 16-session cap and reclaims a dead
  weak entry; the complete Node addon library passes 83/83. The former
  16-connect public rejection is now closed by the execution scheduler below.
- [x] Replace the 16-Context and 16/64-Page fixed tables across selected
  capabilities with allocator-backed dynamic ownership. Binding registries,
  core topology, settings, event/history, storage-state, credentials, and file-
  input timeout indexes plus geolocation/permission policies are dynamic. The
  drag runtime's controller inventory, queued operations, utility worlds,
  listener ownership, event subscribers, and protocol identities are dynamic
  too. Its one active capture and retained data record are Page-local execution
  state behind cancellation-safe scheduling, not public admission limits.
  - [x] Remove the production host's final 16-Context metadata registry and
    its hidden 128-conversion/256-settings-and-binding ceilings. Compatibility
    sessions now select fallibly growing ownership for pending and committed
    Context rows, settings snapshots, retained bindings, temporary conversions,
    and their measured bytes. The finite constructor remains available to
    constrained embedders. One regression simultaneously retains 129 pending
    creations, 129 conversions and 258 settings/binding owners, commits every
    Context, then proves last-reference reclamation after the connection book
    is dropped. The complete all-feature host library passes 1,566 tests with
    27 ignored.
  - [x] Remove Extra Headers' second 16-Context/16-Page settings table and the
    derived 80-route, 160-applied-session, 16-failed-Page, 720-watch and value-
    owner ceilings. Production HeaderPool/settings, topology routes, fanout
    groups, native applied-session identities, initialization failures and
    retirement watches now grow with exact live ownership; constrained
    embedders retain the finite network-model constructors. Regressions cross
    the former limits with 129 retained values, Contexts, Pages, simultaneous
    writers, routes, fanout calls, jobs and watches. The complete network model
    passes 75/75, the all-feature host passes 1,564 with 27 ignored, the Node
    addon passes 83/83, both FFI addons pass release checks, and strict
    all-target host/model Clippy is warning-free.
- [x] Redesign frame-lifecycle ownership as one composed allocator-backed
  boundary. The production outer registry, reduced/native Frame and request
  stores, loader/request/session IDs, and Page owners now grow from actual
  observations. Page ObjectIds, rather than retired slot numbers, select
  lifecycle resource scopes. Escaped handles preserve exact `Arc` lifetime and
  sealing behavior; finite constructors remain available for constrained
  embedders.
  - [x] Make `FrameLifecycleStore` and `CdpFrameLifecycle` grow Frame/request
    rows and exact native ID ownership dynamically while retaining the old
    finite behavior behind explicit finite profiles.
  - [x] Replace the registry's fixed owner table and slot-derived scopes with
    dynamic Page identity ownership, reusing retired holes only after their
    exact owners are reclaimable.
  - [x] Add regressions retaining 129 Pages, 130 Frames and 257 requests, then
    switch the Node-host production profile. Evidence: backend 80/80, plugin
    4/4, composition 479 passed with 16 ignored, and Node host 1,556 passed
    with 27 ignored.
  - [x] Remove the separate aggregate navigation-registration discrepancy.
    Production Page indexes, registration owners and fixed wait arenas now grow
    through the allocator; neither the ordinary profile's 4/128 counts nor the
    burst profile's 21-permit pool rejects a valid operation. A regression
    retains 129 registrations with no finite ledger capacity. Explicit finite
    profiles and their shared-pool tests remain available for constrained
    embedders.
  - [x] Redesign the per-invocation navigation journal itself. Production keeps
    exact owned loader/error/URL/response payloads and dynamically grows event
    and document-request rows; 16 and eight are initial capacities only. Fixed
    per-field ID/error/URL/response validation remains. A regression retains
    130 events and 130 requests, selects the last candidate, and returns its
    response without a ledger lease. Explicit finite profiles still reject row
    overflow; backend navigation tests pass 82/82.
- [x] Add the allocator-backed `FrameTreeFactory` foundation without changing
  constrained embedders. Frame nodes, trees, revisions, conversions, reserved
  Pages, detach revisions, ordinary bindings, and immutable-read bindings now
  share one allocator-mode ownership signal instead of falling back to the old
  ledger. A regression simultaneously retains 130 Frame nodes and 129 staged
  revisions, publishes the tree, and binds Frame 130 from an immutable read.
  All 34 frame-tree tests and strict Node-host Clippy pass.
- [x] Convert production Frame metadata publication ownership to allocator-
  backed storage. URL/name entries, pending changes, close permits, tree-detach
  events, collection/lifecycle events and queue growth no longer consume a
  fabricated aggregate count or metadata-byte ceiling. The finite constructor
  remains available for constrained tests/embedders, and 16 KiB per-URL and
  per-name validation remains unchanged. A regression holds 130 entries, 129
  pending URL revisions and 129 close permits simultaneously. The focused
  metadata run passes 163 tests with one real-Chrome fixture ignored; the full
  Node-host run passes 1,556 unit tests with 27 ignored plus all non-browser
  integration tests, and strict all-target Clippy passes.
- [x] Switch production `FrameTrees` from its 128-node finite factory and
  pre-sized projection index to the allocator-backed factory. Snapshot, node,
  Page, attachment, detach and desired-projection vectors reserve fallibly from
  their observed size. Deferred Context close uses exact receipt membership
  rather than a fabricated 129-descendant envelope. A production-assembly
  regression publishes 130 nodes and verifies zero synthetic ledger counts;
  the complete Node-host run passes 1,556 tests with 27 ignored plus all
  non-browser integration tests.
- [x] Finish drag inner-count classification without conflating persistent
  ownership with execution backpressure.
  - [x] Make the host's Page/session controller identity and storage
    allocator-backed. Each controller now owns an independent local finite
    ledger rather than consuming a connection-wide 32-controller plan. The
    composition selects allocator-backed CDP subscriber bookkeeping, while
    event item/byte backpressure remains separate. Regressions retain 129
    controller budgets and select dynamic fanout after later fixed extensions.
  - [x] Remove the per-controller 32-world/listener count. Pinned upstream
    evaluates drag setup in all current Frames and enumerates all Frames again
    for cleanup, with no count rejection. Production utility-world snapshots,
    listener ownership, `DragMove` world vectors, and the injected listener map
    now grow through the allocator; explicit finite constructors remain for
    constrained embedders. Backend drag passes 77/77, including 130-listener
    and 130-world regressions; the complete Node host remains 1,558 passed / 27
    ignored plus all non-browser integration results.
  - [x] Replace the one retained source move/capture/data record's immediate
    `busy`/capacity failure with cancellation-safe Page-local scheduling.
    Upstream has one Page `_dragState`, and CDP interception is one native
    interval, so these are exclusive state-machine semantics rather than
    allocator-backed parallel ownership; queued callers must wait instead of
    receiving a fabricated resource error. The dispatcher now queues before
    snapshot/listener/capture creation, retains the exclusive gate through dirty
    retirement, removes cancelled waiters without native work, and rechecks
    `_dragState` after waking so a later move becomes `dragOver` when required.
    All-feature drag backend tests pass 104/104, including waiting,
    cancellation, and state-transition regressions.
  - [x] Remove the fabricated DragPage command-owner and Mouse-drag dispatcher
    operation limits from the production host. Both components now expose
    allocator-backed constructors; dispatcher slot, quarantine-mask, and
    cleanup-round storage grows fallibly, while explicit 1..=1024 finite
    constructors remain available for constrained embedders. Separate 1,025-
    owner regressions cross both former ceilings, including quarantine and
    retirement. The all-feature drag backend passes 106/106, the complete Node
    host passes 1,558 / 27 ignored plus all non-browser integration tests, and
    strict combined all-target Clippy passes.
  - [x] Remove the production drag path's fabricated 48-byte CDP session, 64-
    byte listener namespace, and 128-byte unique execution-context ceilings.
    Pinned Chromium protocol types expose these browser identities as strings
    without those maxima. Allocator-backed listener and data profiles now
    retain the observed strings, while explicit finite constructors preserve
    their declared 64/128-byte envelopes for constrained embedders. Regressions
    preserve 4 KiB-plus session/context identities and a namespace containing
    characters the finite identifier profile rejects; the all-feature drag
    backend passes 108/108 with two live-Chrome tests explicitly ignored, the
    focused all-feature host passes 6/6 with four live tests ignored, and both
    crates pass strict all-target Clippy.
- [x] Remove the 64 retained Page/Worker Handle count limits while preserving
  exact disposal and cleanup reservations.
- [x] Remove the eight-public-CDP-session limit; retain bounded unread-event
  delivery independently from session count. Node regressions now retain and
  detach 129 simultaneous Page sessions and 129 auxiliary browser-root
  sessions, so the language adapter cannot reintroduce the native limit.
- [x] Remove the Node/Python 64-listener and Node 32-pending-delivery limits
  from public CDP sessions, plus the Python 64-listener limits on remote Page,
  Context, and Browser lifecycle events. Node disables only EventEmitter's
  warning threshold and keeps its ordered promise tail; Python owns each
  asynchronous handler task until completion or exact session retirement.
  Regressions deliver 129 listeners in order, acknowledge and retire 129
  Python asynchronous handlers, and complete 129 simultaneously submitted
  Node CDP sends. The complete Node suite passes 901/901; Python passes 699
  with 9 skipped, Ruff, and strict Pyright with zero diagnostics.
- [x] Remove the fabricated 1 MiB `CDPSession.send` parameter ceiling across
  Node, Python, both native boundaries, and the Rust host. `cdp-session` now
  selects the allocator-backed large-command capability itself when neither
  Init Scripts nor File Input already supplies the shared pool; actual foreign
  UTF-8 input determines FFI admission and 64 KiB fragments bound individual
  WebSocket writes without limiting the complete outbound message. Node and
  Python regressions forward a 2 MiB object, the native Node parser accepts it,
  and the standalone Rust feature proves an unlimited total profile. Complete
  facades pass 902/902 Node and 700 Python with 9 skipped. Host all-feature and
  Node `cdp-session` strict Clippy pass; Python release check passes. Python
  strict Clippy is currently blocked by unrelated pre-existing HAR fallback
  functions with nine arguments. The former raw result/inbound boundary is
  addressed independently below and receives no parity credit in this item.
- [x] Remove the public raw-CDP path's fabricated 4 MiB response/WebSocket and
  4 KiB protocol-error ceilings. Selecting `cdp-session` now gives trusted
  browser inbound frames/results an explicit allocator-backed sentinel; it is
  never used as an allocation, reservation, or finite ledger value. The
  WebSocket parser disables only its frame/message size check, the connection
  skips its reader/result lease, and topology startup storage excludes the
  sentinel from every finite retention multiplier. Restricted embedders keep
  the existing finite configuration. Regressions accept a 5 MiB result and an
  8 KiB protocol error through the real reader, accept a 5 MiB native Node
  result conversion, and prove parallel/recursive topology composition cannot
  multiply the sentinel. The CDP suite passes 145 library tests, 11 generated-
  type tests and 3 boundary tests with 1 explicit diagnostic ignored; Node
  native CDP tests pass 7/7 and the selected host profile test passes. Public
  facades pass 903/903 Node and 709 Python with 14 skipped; TypeScript, Ruff,
  strict Pyright, strict production-library Clippy for CDP/composition/host,
  Node-addon `cdp-session` all-target Clippy, and the Python-addon release check
  pass. The CDP all-target Clippy gate remains blocked by an unrelated existing
  handshake-test `result_large_err` warning. Public CDP event delivery's
  separate session queue and connection-fanout boundaries receive no result-
  path parity credit here.
- [x] Remove the public CDP session owner's fabricated 64-unread-event and
  1 MiB aggregate queue ceilings. The queue and an outstanding claim retain
  exact event objects through ordinary allocator ownership without a second
  JSON serialization. A 64-event claim remains only an FFI fairness quantum;
  repeated claims and the existing eight-claim flush scheduler can drain an
  arbitrarily larger queue. Regressions retain 130 events across queued and
  claimed ownership, admit a 2 MiB event followed by another event, preserve a
  Browser target fence after 129 earlier events, and drain a 129-event queue in
  batches without invalidation. Native capability reporting no longer exposes
  the retired count or aggregate-byte ceilings. The 33 focused host tests and
  strict host Clippy pass; the complete abi3 Python profile builds, its focused
  contract/facade selection passes 358/358, the full Python suite passes 709
  with 14 skipped, and the Node facade passes 903/903 plus strict TypeScript.
  Ruff and strict Pyright also pass. A direct Python-addon Rust test binary is
  not runnable on this Mac because Xcode's removed Python 3.9 library is absent;
  the actual abi3 extension build and imported capability contract are green.
- [x] Remove the public-CDP shared-connection per-event byte discrepancy.
  `cdp-session` selects the allocator-backed byte sentinel for events as well
  as trusted-browser frames/results/errors. The reader skips finite
  `EventBytes` admission under that sentinel while finite profiles keep exact
  byte leases. A real-reader regression delivers a 5 MiB event across the old
  256 KiB ceiling, and topology projection maps the sentinel to zero rather
  than multiplying `usize::MAX`. Python CDP conversion now uses an explicit
  post-order stack and accepts events beyond the retired 64-depth and 65,536-
  node limits; Node has no independent event payload/node threshold. CDP
  all-feature tests pass 163/163, strict CDP and host Clippy pass, the selected
  host factory profile passes, and the Python addon's focused release check
  passes.
- [x] Remove the remaining public-CDP shared-connection retained fanout/backlog
  discrepancy, which can still terminate otherwise valid upstream event
  delivery. Preserve the 64-event FFI claim only as a scheduling quantum, not
  as total capacity. Implementation is staged: an explicit allocator-backed
  event-count sentinel is accepted only by production reclaiming fanout, does
  not preallocate or project `usize::MAX`, and a unit regression queues 8,193
  events across the former 8,192-item boundary without lag. Allocation growth
  is fallible and closes cleanly only on actual allocator failure. Pinned
  Playwright 1.58.0 `CRConnection`/`CRSession` directly microtask-dispatches
  each transport event and disables EventEmitter listener caps; it declares no
  retained-event backlog rejection. Production code passes an all-feature
  release `cargo check`; Node CDP facade tests pass 28/28 and the focused
  Python large-event facade regression passes. The complete CDP library passes
  166/166, the focused allocator-backed subset passes 11/11, and the final host
  production-profile test passes. The rebuilt abi3 extension passes 362/362
  focused contract/CDP tests and the complete Python suite passes 713 with 14
  skipped. The complete Node facade passes 907/907 plus strict TypeScript;
  Ruff and strict Pyright pass. Finite profiles retain their explicit bounded
  fanout and lag behavior.
- [x] Remove local raw-CDP method and params validation that upstream delegates
  to Chromium. Node, Python, and the Rust host no longer impose the fabricated
  256-byte method-name ceiling or require local `Domain.command` syntax;
  empty strings reach Chromium as they do through pinned Playwright 1.58.0.
  Serialized JSON params are no longer restricted to objects/mappings because
  the upstream CDP channel declares optional `any`. JSON serialization and FFI
  ownership checks remain intact. Node also uses the ordinary official
  `JSON.stringify` projection, so `NaN` and either infinity become JSON `null`
  instead of receiving a fabricated preflight error. Node CDP facade tests pass
  31/31, four focused Python method/params/event regressions pass, both
  cdp-session addons pass release `cargo check`, and the Node parser has a
  non-object JSON unit regression. Non-object event params are also preserved
  because the pinned upstream channel declares event `params` as optional
  `any`; malformed native event tuple shapes and non-string methods remain
  fail-closed. After rebuilding the actual abi3 extension, complete validation
  passes 907/907 Node, 713 Python with 14 skipped, strict TypeScript, Ruff, and
  strict Pyright.
- [x] Remove the 64-Worker and 64-Download owner limits; keep queue byte
  backpressure separate from live object ownership.
- [x] Remove the 256-route, 128-init-script, and 64-selector-engine count
  limits from Playwright-compatible profiles. Init Scripts also no longer
  impose the non-Playwright 64 KiB per-source cap: Node/Python FFI allowances
  follow the observed UTF-8 input, persistent registrations grow fallibly, and
  the independently selectable leaf installs through an allocator-backed,
  fragmented CDP large-command profile whose total message size has no
  fabricated ceiling. That Init Script change does not alter other ordinary
  commands or inbound WebSocket limits. Regressions store a 2 MiB source and serialize a 128 KiB+1
  native command; focused Init Scripts pass 8/8, CDP large commands pass 13/13,
  the complete host passes 1,568 / 27 ignored, Node adapters pass 898/898,
  Python units pass 699 with 9 skipped, and both addons pass strict Clippy. The
  standalone `init-scripts` all-target strict Clippy gate also passes; it
  caught and fixed two test/constant imports that had relied on unrelated
  all-feature selections.
- [x] Remove the 2 MiB screenshot/PDF and 4 MiB response-body result limits;
  use allocator-backed buffers or streaming/spill ownership instead of
  rejecting valid browser output.
- [ ] Replace immediate `ResourceExhausted` concurrency failures for ordinary
  operations with cancellation-safe scheduling/backpressure matching upstream
  behavior. Navigation, screenshot/PDF, topology/lifecycle requests, Locator
  evaluation, execution-world readiness, Worker Handle calls, and ordinary
  main-Frame/Handle call admission and file-input native-path/marker helpers no
  longer reject at their legacy lane counts. Concurrent close now queues before
  its finite native lane too; other selected leaves still require an individual
  audit.
  - [x] Replace the Node and Python FFI connect lanes' immediate 16-operation
    rejection with cancellation-safe waiting before network/process startup.
    Connect tickets and completed results are allocator-backed; a fair native
    semaphore limits only the 16 operations actually executing startup. Exact
    cancellation removes one FIFO waiter, environment shutdown closes and
    wakes the lane, and completion releases its execution permit before later
    JS/Python conversion. A 129-caller regression crosses the former public
    limit; 26 focused host tests and the complete 83-test Node addon library
    pass, and both Node/Python addons pass release `cargo check`. Python test
    linking remains unavailable on this Mac because the configured Xcode
    Python 3.9 library is absent; the all-feature Python profile also has a
    pre-existing async `page_video_path` compile error outside this change.
  - [x] Remove the ordinary Mouse host's 16-retained-invocation rejection.
    Retained calls and their retirement workers grow through the allocator; each
    invocation owns an independent one-record Page route ledger, decoupled from
    the shared backend factory's ledger. The controlled runtime already queues
    beyond eight active commands before backend admission, so the finite backend
    profile is execution backpressure rather than a public rejection. Its 520
    pending-CDP allowance covers exactly eight active maximum 65-command clicks;
    queued calls create no native commands. A 129-invocation regression consumes
    no connection-level synthetic lease. Mouse composition passes 39/39, the
    complete Node host passes 1,558 / 27 ignored plus integrations, and strict
    combined Clippy passes.
  - [x] Remove the analogous retained-call rejections from raw Keyboard, Locator
    Keyboard, and Touchscreen. Raw Keyboard now gives every retained invocation
    an independent one-record Page route ledger while the controlled runtime
    queues beyond its eight active executions. Locator Keyboard retains its
    drag manager and lazily selected Page controller without a sixteen-call
    aggregate byte plan. Touchscreen needed a deeper fix because it bypasses
    that runtime: its production host now selects an explicit allocator-backed
    backend profile, while the finite backend constructor remains available to
    constrained embedders. Separate 129-owner regressions cover raw Keyboard,
    host Touchscreen, and backend Touchscreen without native input. Keyboard
    composition passes 19/19, Touchscreen backend/composition pass 6/6 and 1/1,
    the complete Node host passes 1,558 / 27 ignored plus integrations, and
    strict combined Clippy passes.
  - [x] Replace the `concurrent-close` host's eight-intent public rejection with
    cancellation-safe waiting. This is not a one-constant change: the host ready
    ring, native close scheduler, close authority/Page confirmation, topology
    transition reservations, and runtime disposal reservations currently share
    the finite profile. An allocator-backed FIFO now retains excess requests
    before any of those reservations or native writes; cancelled waiters are
    removed before promotion. Eight remains the internal native execution lane,
    and already-written close commands remain owned through evidence retirement
    and shutdown drain. Runtime deferred disposal now follows the actual
    allocator-backed pending-call table rather than a fabricated sixteen-call
    profile. Regressions retain 129 close requests, discard a cancelled waiter
    before native admission, and retire 129 pending runtime calls. Focused close
    tests pass 6/6, the runtime suite passes 217/217 plus its doctest, and the
    complete Node host passes 1,560 / 27 ignored plus all non-browser
    integrations under strict combined Clippy.
  - [x] Remove the shared raw-input first-poll order's 32-ticket public
    rejection. Mouse, Keyboard, Touchscreen, and Locator drag now enter an
    allocator-backed FIFO; cancellation removes the exact ticket, first-poll
    completion wakes the next submission, and terminal close moves out the
    queue before waking callbacks. A 129-ticket regression crosses the former
    bound while the focused input-start suite passes 7/7. The complete Node
    host remains 1,560 / 27 ignored plus all non-browser integrations under
    strict all-target Clippy.
  - [x] Remove Extra Headers' nine-operation immediate rejection, 80-command
    native semaphore and private 30-second enqueue timeout. Setters and their
    rollback/compensation jobs retain exact allocator-backed ownership while
    the CDP write channel supplies natural asynchronous backpressure. The
    finite HeaderSettings writer profile remains available only when selected
    explicitly by a constrained embedder.
  - [x] Remove Page Bindings' fabricated 64-registration/64-pending-call,
    16-early-call, 128-session and 16-rejection-task ceilings. Production
    registration, early-call, session/script, pending-call, delivery and
    rejection ownership now grows fallibly from live work and retires by exact
    Page/Context/Browser lifetime. The injected controller no longer rejects
    callback 65 or imposes local name, identity, argument, payload, graph-depth,
    node-count or string-size limits absent from pinned Playwright 1.58.0. The
    Python facade likewise retains every registration transaction and callback
    dispatcher instead of rejecting number 65. Regressions cross the old limits
    with 129 registrations/calls/sessions and 130 Python dispatchers. Page
    Bindings pass 39/39 focused tests; the complete host passes 1,567 / 27
    ignored, Node adapters pass 897/897, Python units pass 698 with 9 skipped,
    the Python addon passes 86/86, both FFI addons compile, and strict all-target
    host/Python Clippy passes.
  - [x] Restore Python `Video.path()` after the Rust host operation became
    asynchronous. The PyO3 method now returns an awaitable, the async facade
    awaits artifact finalization, and the sync facade continues through its
    existing loop bridge. The all-feature Python addon compiles and its focused
    artifact-lifecycle contract passes.
- [x] Remove the ordinary main-Handle eight-call admission/publication
  rejection. Scalar and batch call admission, temporary read ownership,
  publication inboxes, console publication queues, and ElementHandle nested
  action admission now grow through the allocator. Focused release tests cover
  129 simultaneous call/read admissions while preserving exact call,
  work-token, result, cancellation, and cleanup lifetimes. Retained file-input
  helper profiles are covered by the completed native-path/marker item below.
- [x] Remove the derived finite Frame-world snapshot and native-retirement
  receipt count from the compatibility composition. The shared accessor and
  weak receipt registry now grow through the allocator; explicitly constrained
  profiles keep their finite admission and byte accounting. A 129-snapshot
  regression retains 129 exact receipts simultaneously, retires their shared
  native session once, and proves no fabricated resource-ledger count is
  consumed.
- [x] Remove the eight-entry retained file-input invocation registry, retained
  exact-element resolver budget, and cleanup-continuation rejection. Their
  compatibility profiles are allocator-backed while finite profiles remain
  available to constrained embedders. Regressions retain 129 pending FFI
  revocation hooks and 129 native element owners; cancellation still keeps the
  route/source alive through exact group-release acknowledgement.
- [x] Remove the ordinary in-memory file-payload aggregate count gates. Host
  call/source owners, materialized batches, payload encoders, upload owners,
  cleanup continuations, and the selected large-command pool are allocator-
  backed. Finite constructors remain available to constrained embedders, while
  regressions retain 129 host owners, batches, upload owners, and large-command
  owners. The 64-file, metadata, 50 MiB source, and encoded-command bounds remain
  per-operation semantic/payload limits. Native-path owners, retained markers,
  and their cleanup chains remain separate open audits.
- [x] Remove the path-preparation worker's one-job aggregate limit. The
  allocator-backed profile owns request copies, normalized paths, opened
  readers, queued/running `spawn_blocking` work, and completed-but-unconsumed
  results until cancellation or consumption. A 129-job regression crosses the
  old limit and drains every owner; the finite profile remains available for
  constrained embedders.
- [x] Remove the retained native-path and marker-chain aggregate count limits.
  Native-path preflight, marker state/calls, composition owners, serialized
  mutation owners, cleanup continuations, and the exact-world marker cache are
  allocator-backed. The cache grows world, waiter, and retiring-generation
  slots dynamically while retaining generation checks, exact-route pruning,
  and fail-closed cleanup. Regressions cross the former 64-world, 8-waiter,
  8-retirement, and single-mutation thresholds; per-operation file/field/byte
  validation remains finite. Backend file-input tests pass 64/64, composition
  passes 61 with one real-Chrome test ignored, the 23 marker-cache lifecycle
  tests pass, the full Node host library passes 1,552 with 27 ignored, and
  strict all-target Clippy passes for all three affected crates.
- [x] Make allocator-backed runtime ownership composable with the optional
  `concurrent-close` leaf without restoring the former 256-object public
  ceiling. Pending close intents remain bounded, but latest-subtree IDs and
  close-route scratch are allocator-backed and are admitted before mutation.
  A 300-object runtime regression proves retirement past the legacy ceiling;
  the complete disposal-reservation suite passes 83/83, the Node host library
  passes 1,485 executed tests with 27 ignored after rerunning three sandbox-
  denied loopback tests outside the sandbox, and the previously failing real
  Chrome 157 all-feature Service Worker attachment now passes.
- [x] Remove the eight-item topology/lifecycle driver-request rejection; use an
  allocator-backed queue and preserve closed-driver cancellation semantics.
- [x] Stop allocator-backed world readiness from reserving `usize::MAX` Frame
  slots; queue waiters, allocate the Frame route vector on demand, and defer
  retained world-snapshot admission until both worlds exist.
- [x] Allow an allocator-backed runtime pending-call table to exceed a selected
  deferred-disposal reservation's concurrency requirement. The former exact-
  equality check blocked every real CDP attach after the runtime became
  allocator-backed; focused runtime tests and the ABI134 Chrome E2E cover it.
- [x] Remove the eight-operation extra-header navigation-referrer gate. Keep
  the 16 KiB field bound, retain snapshots through ordinary allocator-backed
  ownership, and cover 129 simultaneously retained values.
- [x] Remove the ordinary Chromium network profile's aggregate eight-Context
  and eight-Page adapter limits. Context/Page registries now grow through the
  allocator while request, field, payload, and event-queue policies stay
  independent.
- [x] Remove the separately selected Service Worker network profile's aggregate
  Context, Page, and related-session limits. Its registries and exact
  Context-owned adapters are allocator-backed; a 129-Worker regression crosses
  the former eight-adapter ceiling while request, field, payload, and unread-
  event safeguards remain independent.
- [x] Separate Python `frame-detach-events` from `frame-events`; prevent its
  lifecycle discriminant from colliding with file chooser kind 10 and compile
  the exact ABI133 feature selection rather than relying on `--all-features`.
- [ ] Audit the remaining payload and field limits individually against
  upstream and browser-protocol constraints; do not mechanically remove
  protocol-validity or internal queue-byte safeguards.
- [x] Correct the copied documents that still describe a current
  64-active-request limit while preserving clearly labelled historical
  evidence.

- [x] Match upstream Chromium's unbounded active Request count; add 129-request
  lifecycle/adapter regressions and verify that an unfiltered WSJ load no
  longer fails on request 65.
- [x] Treat a network Request whose observed frame has already left topology as
  frame-unavailable instead of terminating the Browser.
- [x] Fix the independent HostSession teardown hang observed after remote CDP
  screenshot runs. `HostSession::close` now has a graceful cleanup window,
  terminal transport disconnect, a forced local-driver window, and a final
  abort boundary; cancellation and `Drop` also disconnect the borrowed
  transport. The dedicated-Chrome `live_remote_cdp_close` regression completed
  navigation against a deterministic rendered page, content and evaluation reads, a nonempty
  screenshot, bounded close, zero-resource cleanup, and a fresh reattach that
  proved the pre-existing marked tab and shared Chrome process remained alive.
- [x] Enforce attach-safe remote ownership. `HostSession` now records created
  Contexts and Pages, closes a created Page individually when its Context was
  pre-existing, closes only session-created Contexts as a unit, and never sends
  `Browser.close`. A dedicated-Chrome regression proves the default Context's
  marked Page and the Chrome process survive. Peer WebSocket revocation returns
  a disconnected error to a pending evaluation and bounded shutdown completes.
- [x] Add the default-off `playwright-rust-remote-cdp` high-level Rust facade
  and umbrella `remote-cdp` / `cdp-handshake-headers` features. The facade
  exposes the scraper workflow, bounded timeout options, custom handshake
  headers, attach-safe close, and a generic exactly-once `RemoteLease` release
  hook without any BCP dependency. Mock-upgrade, live scraper, lease-hook, and
  revoked-socket tests pass.
- [x] Align Node and Python CDP attachment with the same HostSession path.
  Focused native tests prove BCP-shaped headers reach the WebSocket upgrade;
  live binding tests create a Page in the shared default Context and prove
  `browser.close()` retires only that Page while every pre-existing target and
  the dedicated Chrome process survive.

- [x] Verify standalone locked release checks, the Rust workspace tests excluding
  Cargo's Python-addon test binary, Node build/tests, Python wheel/tests, and
  relevant dependency proofs. The addon test binary cannot link on this Mac
  because its configured Xcode Python 3.9 framework is absent; the same addon
  builds through maturin and passes the Python package tests.
- [x] Resolve Rust and vendored WebSocket formatting failures without removing
  upstream provenance.
- [ ] Replace transitional `playwright-rust-fake`/`playwright-rust-js` test support with a
  v2-only CDP fake and prove the production and test dependency graphs.
- [ ] Review the copied scripts and create CI scoped to this repository;
  remove old-repo checks and path assumptions before enabling broad gates.
- [ ] Rebuild native packages from source. Do not import generated `.node`,
  `.so`, `.dylib`, wheel files, or local `.work/` evidence as source.
- [ ] Re-run source-bound official Playwright Core JS/TS and Python test suites
  from the new checkout; regenerate immutable receipts and report pass/fail,
  skip, error, and not-run separately. No full-parity claim is authorized yet.
- [ ] Measure the Rust host and browser process footprints separately under
  representative workloads; enforce retained-object and queue budgets.
- [ ] Audit copied detailed docs for source-era URLs and unresolved `.work/`
  links, preserving provenance without implying fresh verification.
- [x] Rename the repository, Rust crates, Node package, Python distribution,
  import namespace, environment variables, protocol namespace, documentation,
  and generated artifacts consistently as Playwright Rust.
- [ ] Decide publication policy for the Rust crates, Node package, and Python
  distribution.
- [ ] Design any future CLI as a separate opt-in surface.
