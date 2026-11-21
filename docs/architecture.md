<!-- agent-updated: 2026-11-21T05:29:00Z -->

# Architecture

This repository is the independent v2 Rust workspace. Its default-free
`playwright-rust` umbrella selects capability crates explicitly. Typical dependency
direction is:

Only the Chromium/CDP backend is a product target. Firefox and WebKit do not
belong in the dependency graph, package profiles, compatibility denominator,
or test matrix. Portable contracts remain engine-neutral where that improves
composition, but no second engine implementation is planned.

```text
Node/Python facade -> native FFI -> Rust host/composition
                                  -> capability contracts and codecs
                                  -> Chromium CDP backend (when selected)
                                  -> runtime/transport adapters (when selected)
```

The attach-only acquisition path is deliberately incapable of terminating a
browser process:

```text
external lease adapter (optional, outside this repository)
  -> playwright-rust-remote-cdp high-level facade
  -> HostSession composition root
  -> playwright-rust-provider-cdp
  -> playwright-rust-browser-provider::AttachedBrowser
  -> borrowed CDP WebSocket
```

`playwright-rust-remote-cdp` is selected by the umbrella's `remote-cdp`
feature. `cdp-handshake-headers` adds `CdpHandshakeHeaders` to that path.
`AttachedBrowser` carries connection ownership but no `BrowserProcessLease`,
so attach code cannot express `Browser.close` or process termination. The
facade's generic `RemoteLease` is a notification/release hook for an external
authority, not permission to kill Chrome. BCP-specific acquisition, renewal,
fencing, and protos therefore remain in an external adapter crate.

`HostSession` is currently the broad compatibility composition root reused by
the Rust facade and both language bindings. It records each Context and Page
created through the session. On owner-requested shutdown it seals new command
admission, closes pages created inside a pre-existing/default Context, closes
session-created Contexts, then disconnects the transport. A graceful deadline,
forced disconnect deadline, and final driver abort make local teardown bounded.
Peer WebSocket loss skips impossible browser-side cleanup, fails pending calls
as disconnected, and uses the same bounded local retirement path.

`playwright-rust-core` defines dependency-light composition contracts. Capability
crates model one behavior at a time; client and server crates provide typed
delivery paths. Chromium adapters are separate from portable contracts.
The Node and Python layers should translate public values and callbacks, while
Rust owns browser state and core operation semantics. The current Node host is
still a broad compatibility composition root; its footprint and feature edges
need further audit.

Node and Python environment wrappers share the host's `session_admission`
primitive. It increments an allocator-backed pending-session counter with
checked arithmetic but defines no finite policy count; published local and
remote sessions stay in each binding's dynamic ownership collection until
their existing lifecycle cleanup retires them. The Node environment's weak
publication-port registry follows the same allocator-backed ownership rule, so
publishing the seventeenth session does not restore the retired session cap.
FFI operation concurrency is a separate scheduler boundary and must not be
presented as a live-session limit.
The shared Node/Python Connect lane implements that separation directly.
Admission creates an allocator-backed owned ticket and registers shutdown work
without consuming a startup slot. A fair semaphore promotes at most sixteen
native network/process startups; cancellation drops the exact FIFO waiter, and
native completion releases the slot before the completed ticket is converted
by JavaScript or Python. Shutdown seals ticket admission, closes the semaphore,
cancels both running and waiting work, joins registered tasks, and only then
reclaims undelivered outcomes. The value sixteen is therefore an internal
execution/backpressure setting, never a public pending-call or result limit.
Context metadata follows the ownership side of that rule. Compatibility
sessions use a fallibly growing registry for pending and committed Contexts;
settings snapshots, retained native bindings and temporary conversions own
their allocations until cancellation, commit, native retirement or final
reference drop. The 16-Context table and its secondary 128/256 owner ceilings
are no longer selected by Node or Python. A finite constructor and aggregate
byte ledger remain available only for explicit constrained-embedder profiles;
the per-value base-URL validation is unchanged.
Locale is another allocator-backed Context policy. The language bindings admit
the observed UTF-8 bytes, Rust retains the exact optional string, and session
initialization issues Chromium's user-agent accept-language and ICU locale
commands only for a nonempty value. No binding grammar or size validation is
inserted ahead of Chromium. Node exposes locale as an independent native leaf
and also composes it with device emulation, extra headers, proxy, offline,
HTTPS-error, and CSP policies through one typed native Context-creation port.
The facade checks every selected leaf before invoking that port, and Rust owns
the single atomic creation rather than applying policies afterward in
JavaScript. Header conversion remains an explicit typed variant of the same
operation.
Timezone uses the same allocator-backed value boundary. The bindings admit the
observed UTF-8 length and Rust retains the exact optional string. Empty input
selects no policy and emits no command; nonempty input is forwarded unchanged
to `Emulation.setTimezoneOverride`, so Chromium, rather than a binding grammar
or private byte limit, decides whether the identifier is valid. Node exposes
this as an independent `context-timezone` leaf and uses the same generic typed
policy port for standalone and composed ordinary Context creation.
Owned persistent Chromium applies its Context defaults before the initial Page
topology is published. Node exposes locale, timezone, offline emulation, CSP
bypass, and JavaScript disablement as independent launch leaves. Ordinary and
video launch both pass the selected values through one typed Rust defaults
object; empty locale/timezone, `offline: false`, `bypassCSP: false`, and
`javaScriptEnabled: true` do not select capabilities. The combined profile
composes those defaults with `ignoreHTTPSErrors` in the same atomic launch. FFI
connect admission includes the actual locale and timezone UTF-8 byte lengths
rather than imposing a binding-size policy.
Extra Headers follows the same split across both the portable model and the
Chromium host. Compatibility sessions select allocator-backed HeaderPool value
owners, Context/Page settings and overlapping writers. Route snapshots,
fanout/job nodes, applied native-session identities, initialization failures
and exact-session retirement watches are owned by their live operations, not
by the former 16-Page/80-route projection. CDP write-channel pressure is awaited
normally; there is no host operation-count rejection, native semaphore, or
private enqueue timeout. Explicit finite HeaderProfile and
HeaderSettingsProfile constructors remain available for constrained embedders.
Page Bindings is allocator-backed across the same end-to-end boundary. The host
owns registrations, pending and early calls, browser sessions, installed script
identities and outbound delivery records in dynamic collections; retirement
removes the exact Page/Context/Browser generation. Its injected controller uses
the same dynamic callback/reference maps as pinned Playwright 1.58.0 and does
not add callback-count, binding-name, payload-size or serialization-graph
ceilings. Python keeps each shielded registration and callback-dispatch task
until exact completion or Browser teardown rather than imposing a second
language-level count. Semantic JavaScript-safe sequence validation and target
lifecycle checks remain; they are not capacity policy.
Init Scripts uses the same dynamic-source rule as pinned Playwright. Page and
BrowserContext registrations own their exact source strings and replay them to
matching future sessions; no local source-count or UTF-8 byte cap is selected.
Node and Python FFI admission derives its allowance from the observed foreign
input before retaining the Rust copy. Native installation uses the CDP
large-command lane even when `init-scripts` is selected without `file-input`.
The Init Script leaf does not widen other ordinary command or inbound WebSocket
policies, but serializes allocator-backed total messages into bounded 64 KiB
outbound fragments. Partial multi-session installation still unregisters the source and
removes every already-installed native identifier during cleanup.
ABI133 and the subsequent compatibility-limit audit remove finite counts from
binding-owned Context/Page registries, the core topology, public browser CDP
sessions, retained Handle objects, routes, scripts, selectors, and
browser-produced screenshot/PDF/body results. The migration is not complete:
remaining FFI lanes and payload/field policies still require individual
classification. The retained file-input
invocation registry, exact-element resolver, and cancellation continuation are
allocator-backed. The ordinary in-memory payload stack is allocator-backed as
well: host source/call owners, materialized batches, payload encoders, upload
owners, cleanup continuations, and the selected large-command pool retain
actual allocations without a separate aggregate count. Per-call file count,
metadata, source-byte, and encoded-command validation remains intact.
The path-preparation worker follows the same ownership rule across queued work,
blocking I/O, completed-but-unconsumed results, and cancellation; constrained
embedders may still select the original finite worker profile.
The shared Frame-world snapshot accessor and its
native-retirement receipt registry are allocator-backed in the compatibility
composition while explicitly constrained embedders retain a finite profile.
Ordinary main-Frame/Handle call admission and both scalar and batch Handle
publication inboxes grow through the allocator. The
topology/lifecycle command
queue, Locator evaluation, Worker Handle calls, and execution-world readiness
now wait under bounded execution lanes instead of returning a fabricated
capacity error.
Raw Keyboard follows the same split: invocation and Page-route ownership grows
through the allocator before the controlled runtime's eight-operation execution
lane. Locator Keyboard retains its drag manager/controller directly without an
aggregate call plan. Touchscreen bypasses the controlled dispatcher, so its
compatibility host selects allocator-backed operation ownership at both host
and backend layers; the backend's finite profile remains available only as an
explicit constrained composition.
Mouse, Keyboard, Touchscreen, and Locator drag share a first-poll ordering FIFO.
That FIFO now grows through the allocator instead of rejecting the thirty-third
unstarted input call. Dropping any ticket removes its exact identity, the next
head is woken in submission order, and terminal close drains all registered
wakers without allocating.
The production drag controller also treats CDP session IDs, listener
namespaces, and unique execution-context IDs as allocator-backed protocol
identity. It does not impose the finite drag profile's 48/64/128-byte field
ceilings. The single Page capture/data slot remains behind the Page-local
asynchronous gate because Chromium exposes one interception interval and
Playwright maintains one `_dragState`; queued public operations do not consume
or reject against that internal slot.
Public CDP-session ownership follows the same rule through the language
facades. Node Page and browser-root session maps, Node/Python listener tables,
Python remote lifecycle listeners, and the Node ordered delivery chain grow
with their live owners rather than rejecting at 8, 64, or 32 entries. Python
retains each asynchronous handler task in the exact session and cancels only
those still pending when that session retires. The native session unread queue
also grows with the events actually retained. The Node adapter's 64-event
claim and eight-claim per-flush controls remain fairness quanta, not public
queue or listener limits. Shared connection-level event payloads and retained
reclaiming-fanout entries are allocator-backed too.
Raw public CDP-session parameters also use the allocator-backed large-command
path. The language boundary reserves observed UTF-8 input, Rust accepts the
channel's arbitrary JSON value without reserializing it against a private 1 MiB
policy, and outbound transport fragments the complete message at 64 KiB. Node
keeps the official `JSON.stringify` projection for non-finite numbers instead
of rejecting them locally. The `cdp-session` feature
installs that pool only when Init Scripts or File Input has not already supplied
the same capability. Raw command results, protocol errors, and trusted-browser
inbound WebSocket frames and events use the same explicit allocator-backed
sentinel when `cdp-session` is selected. Finite profiles preserve their
configured event-byte and fanout bounds.
The optional concurrent-close host applies the same distinction at teardown:
an allocator-backed, cancellation-aware FIFO owns accepted close requests
before the eight-operation native execution lane. A request reserves close
authority, topology transition, runtime disposal, and native scheduler state
only when promoted. Already-written native closes keep that exact finite
ownership through evidence retirement and terminal drain. Runtime deferred
disposal tracks the actual allocator-backed pending-call set rather than
rejecting a close because more than sixteen calls exist.

The Chromium network lifecycle stores active Request and Response state in
allocator-backed dynamic maps, matching upstream Playwright rather than
enforcing a finite request-count admission limit. Bounded event queues,
payload fields, and opt-in retained-data tiers are separate composition leaves;
they must not be multiplied into a fabricated maximum active-request count.
Its ordinary production composition also creates BrowserContext and Page
network adapters on demand. The allocator-backed aggregate profile passes each
adapter the same independent request/field/event policies, so removing the
aggregate count does not remove payload validation or bounded unread queues.
When Service Worker events are selected, the same profile also grows exact
related-session and Context-owned adapter maps through the allocator. A
129-Worker regression crosses the former eight-adapter threshold without
pre-reserving `usize::MAX` resources.

Video capture is a default-off host composition. The
language facade normalizes the public directory and size options, while Rust
owns `Page.startScreencast`, frame acknowledgement, frame-to-time conversion,
the pinned Playwright FFmpeg process, WebM output naming, and finalization. The
recorder is installed before Chromium resumes a Page and is drained before an
owned browser is shut down. Per-Context recorder ownership lets an ordinary
owned Context finish only its own recorders when it closes; a persistent
launch default applies only to its default Context and cannot leak to another
owned Context. Browser-registry metadata locates `ffmpeg-1011` without
installing it or coupling browser acquisition to the recorder. Public Python
Video artifacts for persistent and ordinary owned Contexts are thin references
to a Rust artifact entry. The entry publishes its path immediately, exposes a
watchable encoder-completion result, serializes copy/delete operations, creates
copy parents on demand, and keeps the artifact alive after Page/Context close.
Deletion is idempotent only after a successful remove or an observed missing
file; an I/O failure remains retryable. Python therefore owns no raw frames,
encoder process, completion task, or artifact byte copy. Node ordinary owned
Contexts and `Browser.newPage()` implicit Contexts use the same artifact entry
through a thin `Page.video()` facade. The installation holds only a weak
connection reference while active recorder tasks own their required
connection, so Browser close reaches zero resource-ledger leases without
waiting for JavaScript GC. Remote artifact transfer semantics remain open.

The Node owned-launch adapter has separate ephemeral and persistent Rust ports.
The persistent port opens the requested profile directory, launches Chromium,
and returns one default Context through the same host/session owner. A compact
checked policy word currently transports `headless` and
`ignoreHTTPSErrors`; `timeout` and `baseURL` remain typed arguments. Unknown
policy bits fail closed. The video persistent port remains separate until it
can carry the same complete policy contract.

`playwright-rust-har-recording` is a backend-neutral leaf over normalized HTTP
lifecycle snapshots. It owns HAR policy and serialization, but no browser,
CDP session, subscription, runtime task, filesystem, or ZIP writer. The optional
Node-host adapter selects Page versus BrowserContext scope and asks the exact
CDP session for a response body only after terminal publication. One fetched
body is then supplied to every matching recorder. Language bindings retain
only public options and the destination path. A separate host archive adapter
owns the filesystem/ZIP capability: it writes SHA-1-named deduplicated
attachments beside a plain HAR or as `har.har` plus resources at ZIP root.
Replay injects a content-source capability into the policy leaf, so
plain `_file` paths and ZIP entries work without moving filesystem ownership
into the backend-neutral replay crate. Plain paths are contained beneath the
HAR directory before acquisition. The policy also owns the pinned upstream
minimal/full serialization distinction and a generated `mime@3.0.0`
MIME-to-preferred-extension table; those are deterministic data policies, not
filesystem capabilities. The default replay policy matches upstream by
imposing no private size, entry, header, body, or redirect quota. Embedders may
select finite `ReplayLimits`; its `usize::MAX` default is an unlimited sentinel
and is never used as an allocation request. The host rejects an empty HAR path
but delegates path-length validity to the filesystem, as upstream does.
The Page-routes composition retains HAR registrations in its allocator-backed
registration vector without a separate archive-count or aggregate-byte
admission gate; dynamic archive bytes are not presented as statically reserved
backend capacity.
The replay archive retains raw embedded text/base64 or `_file` descriptors,
not eagerly decoded bodies. Lookup decodes only selected POST data and the
final matched response. The host's plain-directory and ZIP content-source
implementations reopen the selected resource on demand, and Page-routes runs
lookup in `spawn_blocking` so filesystem and decompression work do not occupy a
Tokio execution thread.

Full HAR recording composes a separate Page observer around the existing
Chromium frame observer. An injected wall clock assigns opaque Playwright-shaped
Page identities when recording begins or a new Page is initialized. Main-frame
DOMContentLoaded/load events enqueue at most one capture per document phase;
the serialized host driver evaluates title and `performance.timing` on that
exact Page session, then updates every matching recorder. Raw request/response
extra-info headers, terminal timing and sizes, HTTP protocol/server address,
and TLS details stay in backend sidecars until the backend-neutral recorder
serializes them. No Page-evaluation task, clock, CDP session, or filesystem
capability is owned by the HAR policy crate.

Creation-time HAR remains a language-level composition over the same Rust
recorder port rather than a second recording engine. Python normalizes the five
official `record_har_*` arguments and Node normalizes the `recordHar` object,
then each calls `context_record_har` after native Context creation but before
publishing that Context to user code. `.zip` defaults to attached content,
plain HAR defaults to embedded content, explicit content overrides the legacy
omit flag, and mode defaults to full. If recorder installation fails, the
language owner closes the unpublished Context before returning the original
error. The Node `har-recording` profile remains a lean Page-routes composition;
`official-conformance-page-har-recording` composes it with the ordinary Page
surface for browser comparison without moving those dependencies into the leaf.
URL RegExp support is another independent composition edge. The optional
`playwright-rust-ecmascript-regexp` leaf wraps `regress` without owning a
browser, runtime, FFI, HAR, route, or cookie policy. `har-recording-regexp`,
`route-regexp`, and filtered `context-cookies` compose that same leaf; their
base artifacts prove the dependency is absent when not selected. Source and
flags cross the language boundary once and compile once per public RegExp.
Matching uses UTF-16 indices, supports lookaround, backreferences,
`d/g/i/m/s/u/v/y`, Unicode-sets syntax, and synchronized JavaScript
`lastIndex` state. Routes retain separate server-dispatch and client-handler
instances, matching the pinned dispatcher's two observable state machines for
`g`/`y`. No source, flag, glob, or matcher-count quota is invented where pinned
Playwright has none. The leaf pins upstream `regress` commit `95c04041` and a
Unicode 16.0 table generated from official UCD/Emoji inputs so the matcher does
not silently follow a newer Unicode version than the Node 24.12 baseline. A
committed, compressed corpus replays 5,135 static patterns and 97,198
`test()`/`lastIndex` steps extracted from the last Test262 main commit before
Playwright v1.58.0. Node oracle generation uses one process per source file;
one shared V8 isolate produced a RegExp-cache false result and is forbidden.
Dynamic Test262 programs and a real-browser oracle for the two-stage route
behavior remain verification work, so this is not a blanket claim of
all-version ECMAScript parity.

Page and Service Worker network sources have different owners. A Page/OOPIF or
dedicated-Worker request uses its Page lane. A browser-root Service Worker uses
an independent BrowserContext lane keyed by the exact Worker session; it never
borrows an arbitrary Page. The topology driver calls a synchronous pre-resume
observer hook while holding its ordered topology cursor, before `Network.enable`
and `Runtime.runIfWaitingForDebugger`, so the Context lane exists before the
first Worker request can run. Service Worker requests publish only to Context
subscribers, retain their Context-owned response history, expose no Frame, and
carry the exact Worker identity across the Rust and FFI boundaries.

The optional Page-route observer also participates in that pre-resume hook. It
derives `Fetch.enable` patterns from BrowserContext registrations only, stores
live Service Worker sessions in an allocator-backed registry, and fans later
Context route/unroute changes out to those exact sessions. Route calls cross
the FFI with an optional Page identity plus an exact Service Worker identity;
the Node adapter therefore dispatches them only through the Context handler.
Page route precedence remains unchanged for Page-owned requests.

The raw `playwright-rust-cdp` crate provides the shared CDP connection and
message foundation used by Chromium adapters. This repository is
self-contained and does not use cross-repository Cargo path dependencies.

The optional public `cdp-session` composition treats trusted-browser response
frames, result objects, and protocol-error text as allocator-backed because
pinned Playwright exposes no byte ceiling for them. `UNBOUNDED_CDP_BYTES` is an
explicit configuration sentinel, not a request to allocate or reserve
`usize::MAX`: the WebSocket parser receives no finite frame/message limit,
reader/result leases are omitted, and topology startup profiles contribute
zero finite response bytes. Ordinary finite profiles retain their limits and
exact accounting. Public-CDP event payloads use the same sentinel: a 5 MiB
reader regression crosses the former 256 KiB ceiling, and Python converts
events iteratively without the former 64-depth/65,536-node rejection. Native
per-session unread queues and public shared-connection fanout grow fallibly
through the allocator. Finite profiles preserve explicit lag-on-overwrite.

Two transitional test-only crates, `playwright-rust-fake` and `playwright-rust-js`, remain
because 42 v2 crate test suites currently depend on the fake CDP client.
They are not dependencies of the v2 production graph. They should become a
v2-specific test-support crate without the old JavaScript-snippet dependency.

No general browser-automation CLI, bridge, or server executable is currently
part of the public surface. The workspace does include optional Playwright
Server *protocol client* crates; these are composable library components, not a
bundled Node driver.

For capability-by-capability background, see [architecture-v2](architecture-v2.md)
and the [dependency topology](knowledge/playwright-rust-dependency-topology.md).
