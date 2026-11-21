<!-- agent-updated: 2026-11-21T05:29:00Z -->

# Design decisions to settle

This is design work in progress, not a stable public API contract.

## Remote CDP ownership and teardown

A CDP attachment borrows a browser process. It never acquires process-control
authority, and no attach shutdown path may send `Browser.close`. Session
ownership is established only by successful creation through that session:
new Contexts are owned as a whole, while new Pages in a pre-existing/default
Context are owned individually. Pre-existing Contexts, Pages, profile state,
and the Chrome process remain external.

Close is an ordered, bounded state transition: seal admissions; attempt exact
owned-target cleanup under the operation deadline; disconnect the WebSocket;
join the driver during the graceful window; force disconnect and join during
the forced window; abort only the remaining local task; then invoke the
external `RemoteLease::release` hook at most once. Concurrent/repeated close is
serialized and idempotent. A peer close immediately makes in-flight calls
return a transport/disconnected error; it cannot wait for browser-side cleanup
that the revoked socket can no longer perform.

The high-level facade is generic. `RemoteCdpConnectOptions` carries connection,
command, operation, close, and lease-release timeouts plus optional handshake
headers and `Arc<dyn RemoteLease>`. It does not know lease IDs, profile IDs,
fencing tokens, renewal RPCs, or any BCP schema. Those belong in an adapter
outside this repository.

The product scope is Playwright-shaped automation for Chromium only. Firefox
and WebKit backends, protocol adapters, browser downloads, fixtures, and
engine-specific conformance are intentional non-goals. Compatibility work is
measured against the Chromium portion of upstream Playwright; unsupported
Chromium APIs remain gaps and must not be hidden by the engine decision.

1. Define the smallest portable browser, context, page, frame, and handle
   contracts, independent of Chromium or a particular async runtime.
2. Keep each capability independently selectable and prove exact dependency
   closures, including Node and Python native feature profiles.
3. Specify ownership for an attached browser versus an explicitly launched
   browser, and define close, crash, and reconnect behavior.
4. Specify a typed client/server protocol with versioning, ordering, event
   backpressure, and explicit terminal-state behavior.
5. Set measured budgets for idle RSS, per-page and per-connection memory,
   remote object tables, event queues, traces, and network-retained data.
6. Define FFI responsibilities and package naming separately from the core
   Rust API. Compatibility claims require full, unmodified official test
   execution or an explicit scoped matrix with receipts.

Compatibility profiles must not invent finite object-count limits where the
corresponding upstream Playwright implementation has none. Dynamic lifecycle
tables, including Chromium's active Request map and selected Context/Page
registries, grow through the allocator. Teardown-sensitive pools are
replenished during normal creation/reconciliation so terminal cleanup does not
allocate. Independent payload, unread-queue, and explicitly opt-in retained-
data budgets remain allowed when they do not reject otherwise valid upstream
behavior.
Binding registries that merely retain live session adapters obey the same rule.
Public raw CDP response compatibility follows it as well. A selected
`cdp-session` profile may disable the trusted-browser WebSocket/result/error
byte ceiling with one explicit allocator-backed sentinel. Every layer that
derives finite resource requirements must map that sentinel to zero accounted
bytes and must not multiply, reserve, or pre-size it. Finite embedders retain
their exact limits. Public-session unread queues, shared-connection event
payloads, and retained reclaiming-fanout entries have the same allocator-backed
ownership rule. Finite compositions may still select explicit bounds.
The Node publication-port registry therefore owns weak session entries through
the allocator; its fixed resource claim covers only the environment header,
not a fabricated maximum number of sessions.

Transported JavaScript RegExp values use one optional backend-neutral policy
leaf, not feature-local approximations. It preserves UTF-16 indices and
observable `RegExp.test()` state, including `g`/`y` `lastIndex`, while HAR,
route, and cookie consumers own only their selection policy. Source length,
flag length, compiled-program size, and matcher count default to allocator
behavior because pinned Playwright defines no corresponding public rejection.
Embedders may add explicit constrained profiles, but compatibility artifacts
must not silently select them. Focused syntax tests do not substitute for a
source-pinned Test262 or upstream behavioral oracle.

The Playwright v1.58.0 profile fixes the ECMAScript data policy too: its
vendored Rust engine uses Unicode 16.0, matching pinned Node 24.12, rather than
whatever Unicode release an indirect dependency adopts later. The checked-in
Test262 corpus is generated by file-isolated Node oracle processes and covers
static literal/constructor patterns plus UTF-16 `test()` state. Regeneration
must fail on a Test262 commit, Node version, or Unicode-input mismatch. Dynamic
Test262 programs and live route dispatch remain separate proof obligations.

The current migration distinguishes a finite execution lane from a public
rejection threshold. Topology/lifecycle requests use an allocator-backed FIFO;
Locator evaluation, Worker Handle calls, and execution-world readiness retain
finite active work but queue excess callers. Allocator-backed topology profiles
must treat `usize::MAX` only as a sentinel: they may not preallocate or multiply
it into a fake retained-byte budget. Frame-world snapshots use the same rule:
compatibility profiles dynamically own each snapshot and native-retirement
receipt, while an explicitly constrained embedder may still select a finite
profile. Remaining count-limited leaves are tracked
explicitly in `tasks.md` and are not covered by the compatibility claim.
Node and Python Connect calls follow the same rule: their argument/future and
eventual outcome owners are allocator-backed, while only native startup enters
a fair finite semaphore. The scheduler slot ends at native completion rather
than foreign result delivery. Dropped language futures cancel their exact queue
position; environment shutdown wakes all waiters and joins their registered
work. Compatibility profiles may tune actual startup parallelism, but cannot
turn that setting into an immediate public-call or undelivered-result cap.
Context metadata applies the same compatibility distinction without an
execution lane: active/pending Context identities, settings, retained bindings
and short-lived conversions are ordinary dynamic ownership, not backpressure.
The compatibility constructor grows its registry fallibly and validates the
16-KiB base URL per value; it does not translate a preallocated table or an
aggregate metadata-byte plan into a public Context-count error. Explicit
finite profiles remain testable for constrained embedders.
Extra Headers uses the same ownership rule end to end. Header values, sparse
Context/Page settings, concurrent proposals, topology routes, per-Page fanout
groups, applied native-session identities, failure latches and close watches
grow from actual live work. The compatibility host awaits the existing CDP
write queue rather than exposing its old nine-operation rejection, 80-slot
semaphore or a private 30-second enqueue timeout. Finite portable profiles are
still composable choices for an embedder, but production compatibility never
selects them implicitly.
Mutable Context policies keep one Rust owner across creation, runtime update,
target initialization and teardown. A runtime update changes retained state
before broadcasting to current targets, and an async policy gate serializes
new Page/OOPIF initialization against that transition. HTTP credentials follow
this rule for replacement and clear: routing compositions refresh the shared
Fetch owner for Page/OOPIF and Service Worker sessions, while a credentials-
only profile keeps a narrower direct Fetch path. Language facades validate and
transport values but do not mirror policy or challenge state.
Page Bindings follows pinned Playwright's dynamic ownership rather than using a
queue limit as public API policy. The browser-side controller and Rust host may
retain any allocator-supported number of registrations, callbacks, early calls,
session installations and serialization references. Allocation is attempted
before mutating registries or issuing native installation work; a closed
delivery receiver rolls back the exact pending call. Page/Context retirement
cancels the matching records, while Browser shutdown owns every remaining
Python callback task through bounded teardown. Binding names and serialized
values are accepted on the same semantic domain as upstream instead of local
256-byte, 64-KiB, depth-64 or 4,096-node ceilings. Explicit finite profiles may
still be designed for constrained embedders, but compatibility compositions
must not select them implicitly.
The Node `context-bindings` leaf extends that same transport. Rust owns the
Context scope and its application to current/future Page and OOPIF sessions;
the language layer keeps only the callback that must run in JavaScript. One
published call stream carries both scopes, with Page callback lookup preceding
Context lookup after Rust has enforced exact cross-scope uniqueness.
Init-script source size is application data, not a scheduler or protocol
backpressure setting. Pinned Playwright reads or serializes the complete source,
stores it in dynamic Page/BrowserContext arrays, and forwards it to Chromium
without a 64 KiB check. The compatibility composition therefore retains the
observed source allocation and uses an allocator-backed large-command owner
with bounded frame fragments but no artificial total-message ceiling. Ordinary
CDP commands do not inherit that policy, inbound message limits are untouched,
and constrained embedders may still construct an explicitly finite
`LargeCommandProfile`. Independently, the public raw CDP-session capability
selects that transport for its arbitrary outbound parameter object; this does
not make every internal command a large command.
For raw Keyboard, the controlled runtime queue precedes the finite backend
execution lane and every retained invocation owns its own Page-route ledger.
Locator Keyboard retains real manager/controller owners without reserving a
fixed number of parameter envelopes. Touchscreen has no equivalent runtime
queue, so its compatibility host must select allocator-backed backend operation
ownership as well as allocator-backed host retention; merely deleting the outer
gate would still expose the inner backend limit.
The shared raw-input first-poll order is an allocator-backed FIFO too. It
serializes only the first poll needed to preserve Playwright input state order;
it does not impose a public count. Cancellation removes an arbitrary queued
ticket, head release wakes the next ticket, and shutdown moves the queue out
before waking callbacks so no waker runs under the order lock.
Browser-generated CDP session and unique execution-context strings are dynamic
identity, not payload policy. Compatibility drag profiles retain their observed
lengths through the allocator; only explicitly finite profiles may select the
legacy namespace/context envelopes. The one active drag capture is instead an
execution invariant protected by cancellation-safe waiting, matching the one
Page `_dragState` and one Chromium interception interval.
Public CDP sessions and their listeners are dynamic ownership too. Compatibility
facades must not cap simultaneous Page/browser-root sessions, registered event
handlers, asynchronous Python handler tasks, or callers waiting behind the
Node session's ordering tail. Session retirement cancels only that session's
unfinished Python callback tasks. Native unread events grow with actual
session ownership. Bounded claim/flush batches remain valid scheduling
fairness policies because they neither reject event publication nor limit the
number of live public sessions.
Raw `CDPSession.send` parameter size is application/protocol data, not a public
admission count. Node/Python FFI admission follows the serialized input actually
owned by the call, and the shared large-command transport bounds fragments
without imposing a total-message ceiling. Feature composition must install one
pool: File Input first, otherwise Init Scripts, otherwise CDP Session. The
matching raw response, protocol-error, and trusted-browser inbound-frame path
uses explicit allocator-backed sentinels for trusted-browser event payloads and
retained event count. Finite connection profiles preserve their own bounds.
Concurrent close uses an allocator-backed FIFO in front of its finite native
lane. Waiting requests own only their target and reply channel: they do not
reserve authority, Page confirmation, topology transition, runtime disposal,
or native command capacity until promotion. Cancelled waiters are discarded
before native admission. Once a native close is written, its finite reservation
remains live until the existing evidence/retirement path settles it or terminal
drain returns the exact failure. The runtime disposal profile therefore treats
the deferred pending-call set as allocator-backed while retaining eight as the
maximum simultaneously executing close count.
Retained file-input invocation and exact-element resolution follow this policy,
including cancellation cleanup. The ordinary in-memory payload lane applies
the same policy through materialization, encoding, upload ownership, cleanup,
and large-command transport; its semantic per-operation payload limits are not
concurrency limits. Path preparation now retains each queued/running job and
completed result through allocator-backed cancellation-safe ownership.
Native-path mutation ownership and marker lanes remain separate boundaries and
cannot inherit that claim implicitly.

The ordinary Chromium network profile follows that rule at both levels: live
BrowserContext and Page adapters are allocator-backed, while each adapter keeps
its independent request, field, payload, and unread-event policies. The
separately selected Service Worker network profile extends the same policy to
related Worker sessions and Context-owned adapters; all three registries grow
through the allocator.

Service Worker request interception follows the same ownership rule as pinned
Chromium Playwright: only BrowserContext registrations apply to a browser-root
Worker session. A Page registration never falls back into a Worker request, and
the public Request carries its Context and exact Worker identity without a
fabricated Page or Frame. Page-session preparation may keep a finite temporary
initialization pool, but the registry of live Service Worker route sessions is
allocator-backed and retires on target detach or Context removal.

Video recording is independently selectable and Rust-owned. Enabling
`record_video_dir` may add the screencast and FFmpeg dependency edge, but a
profile that omits it carries neither process nor frame-buffer ownership. The
recorder begins before the target resumes, acknowledges every native frame,
and completes encoder finalization before owned-browser shutdown returns.
Language facades may normalize Playwright-shaped options and expose artifact
objects, but they must not retain raw frames, CDP sessions, or encoder
processes. Accepting a record-video option without producing a verified file
is never compatibility.

Each artifact is keyed by the portable Page ID and carries its exact portable
Context owner. A persistent-launch default is eligible only for the default
Context; an ordinary `Browser.new_context()` recording option is registered
explicitly and cannot leak between owned Contexts. `path()` is available as
soon as recording starts;
async `save_as()` waits for the Rust encoder-completion result, while the sync
facade rejects `save_as()` until the Page is closed with Playwright's exact
message. Copy and delete share one Rust operation lock. A completed deletion is
idempotent and makes later save fail with Playwright's deleted-file error, but
an actual filesystem deletion failure must not poison the artifact as deleted.
Python supports persistent Contexts, ordinary owned Contexts, and the implicit
Context created by `Browser.new_page()`. Node supports ordinary owned Contexts
and the implicit Context created by `Browser.newPage()`, using the same Rust
artifact lifecycle through `Page.video()`. Context close finalizes only that
Context's recordings. Remote artifact transfer remains a separate open
capability. Node now also launches a non-video persistent
Chromium profile in Rust and publishes its single default Context; persistent
recording remains separate because the video launch port does not yet compose
every persistent Context policy.

HAR recording follows the same scope boundary. A Page recorder accepts only
Page-owned requests and never fabricates a Page for Service Worker traffic; a
BrowserContext recorder accepts both Page and exact Context-owned Worker
requests. Lifecycle order is committed synchronously before an asynchronous
backend body fetch, and export waits for the serialized Context-close boundary.
The policy leaf returns bytes and attachments to its caller rather than owning
filesystem or archive capabilities. Attachment identity matches upstream:
SHA-1 of the bytes plus the MIME-derived extension, deduplicated within one
recording. The capability-owning host writes ordinary HAR resources beside the
document and ZIP resources beside the root `har.har`; the replay policy accepts
an injected content source for either layout and never opens a path itself.
Minimal mode retains upstream sentinel values (`time`, body sizes, and content
size remain `-1`) rather than reporting measurements that upstream deliberately
omits. POST data uses the distinct HAR `PostData` shape, and textual response
detection preserves the upstream font exception.

HAR replay defaults must not add quotas absent from Playwright. Archive size,
entry count, URL/method/header size, POST/response bytes, retained bytes, and
redirect count therefore default to unlimited. A constrained embedder can
still compose explicit finite `ReplayLimits`; the unlimited sentinel must not
be converted into eager reservation or multiplication. HAR paths similarly
have no library-owned maximum length; the filesystem remains the authority.
Replay lookup returns the recorded status unchanged; it does not reject an
entire archive based on a status that upstream defers to route fulfillment.
No upper layer may reintroduce these defaults as an archive-count or aggregate
retained-byte admission gate. Capability reports list only active finite
policies, not removed historical quotas.

HAR open must parse metadata without acquiring every body. External resources,
base64 decoding, and POST-data decoding happen only for candidates used by a
lookup. The backend-neutral policy stores a thread-safe content-source
capability; the runtime-owning host is responsible for executing potentially
blocking acquisition outside its async executor threads.
Lookup acquisition or decoding failures are route-time HAR misses for policy
purposes: the host records the diagnostic and executes the caller-selected
abort/fallback behavior.

The design favors compile-time composition over unrestricted dynamic Rust
library loading. Browser download and launch must be opt-in. Chromium's CDP
backend is not a universal engine protocol. See the detailed
[v2 architecture](architecture-v2.md) and [resource boundaries](resource-boundaries-v2.md)
for the current proposals and earlier decisions.
