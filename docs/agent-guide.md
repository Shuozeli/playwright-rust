<!-- agent-updated: 2026-11-21T21:02:00Z -->

# Agent guide

This is the operational map for maintainers and coding agents. Read
[Architecture](architecture.md), [Design](design.md), and the relevant open
[Task](tasks.md) before implementation.

## Non-negotiable invariants

1. Chromium/CDP is the only product backend. Do not add Firefox/WebKit edges.
2. Workspace crates keep `default = []`; features represent explicit choices.
3. Remote attach never owns Chrome: no `Browser.close`, process termination,
   or cleanup of pre-existing targets.
4. BCP stays external. This repo accepts URL, headers, timeouts, and a generic
   optional `RemoteLease` hook.
5. Rust owns browser state and semantics; Node/Python are thin translations.
6. Do not invent public finite limits where upstream relies on allocation.
7. Focused tests prove only their declared slice, never full conformance.

## Repository map

```text
crates/playwright-rust/                 no-default-features umbrella
crates/playwright-rust-core/            portable contracts
crates/playwright-rust-cap-*/           one capability per crate
crates/playwright-rust-provider-cdp/    CDP provider
crates/playwright-rust-browser-provider/attach/process ownership boundary
crates/playwright-rust-remote-cdp/      high-level Rust attach facade
crates/playwright-rust-node-host/       broad compatibility composition root
crates/*-node-addon, *-python-addon/    native boundaries
packages/playwright-rust/               Node facade
packages/playwright-rust-python/        Python facade
docs/                                   canonical design/tasks/evidence
```

Intended direction is facade -> composition -> capability contracts ->
provider -> transport. The Rust facade currently imports `HostSession` and
values from `playwright-rust-node-host`; that is tracked debt, not the target.

## Remote-CDP lifecycle

```text
current happy path
connect -> validate -> attach with headers -> snapshot topology
        -> expose borrowed Contexts -> record explicit creations
close   -> seal facade -> close remembered Contexts -> HostSession cleanup
        -> disconnect/join local driver -> invoke external lease release

target state machine
OPEN -> CLOSING -> TRANSPORT_CLOSED -> LEASE_RELEASED -> CLOSED
          + one absolute deadline
          + cancellation guard before first await
          + late-publication reconciliation/fence
          + stable result for repeated close
```

The P0/P1 “Remote-CDP shortcut audit” is authoritative. Do not hide a race by
lengthening a timeout.

## Ownership model

| Object | Ownership proof | Allowed close behavior |
| --- | --- | --- |
| Chrome process | Never | Disconnect only. |
| Pre-existing Context/Page | Present at attach or no creation lineage | Borrow; never close on session shutdown. |
| Explicit new Context | Successful session creation | Close it and descendants. |
| Explicit Page in borrowed Context | Successful session creation | Close exact Page. |
| Popup/indirect target | Opener/creation lineage required | Gap until lineage exists. |
| CDP socket/driver | Created by attachment | Retire under bounded deadlines. |
| External lease | Generic supplied hook | Target is cancellation/retry-safe shared release state; current code is incomplete. |

Topology visibility is not ownership.

## Adding a capability

1. Identify one behavior's portable contract, Chromium adapter, runtime needs,
   host composition, and language surfaces.
2. Keep contracts small and browser-specific commands in CDP adapters.
3. Add opt-in umbrella/native features and test positive and negative closures.
4. Keep bindings thin; reject unsupported options explicitly.
5. Add deterministic Arrange-Act-Assert contract, mock-transport, and real
   Chromium tests as behavior requires.
6. Update the support matrix and exact task without broadening claims.

## Test ladder

```bash
CARGO_INCREMENTAL=0 cargo test --release -p playwright-rust-remote-cdp
CARGO_INCREMENTAL=0 cargo test --release -p playwright-rust-remote-cdp --features cdp-handshake-headers
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Live tests use only the dedicated automation profile:

```bash
chrome-cdp start
chrome-cdp status
export PLAYWRIGHT_RUST_TEST_CDP=http://127.0.0.1:9222
export PLAYWRIGHT_RUST_TEST_CDP_WS="$(curl -fsS "$PLAYWRIGHT_RUST_TEST_CDP/json/version" | jq -r .webSocketDebuggerUrl)"
CARGO_INCREMENTAL=0 cargo test --release -p playwright-rust-remote-cdp \
  --test live_facade -- --ignored --test-threads=1
```

Ownership regressions inventory targets before/after, preserve a pre-existing
marker and Chrome process, and prove the owned target disappears. Disconnect
tests bound pending-call and close waits. Node uses `pnpm`; Python one-offs use
`uv`. Official conformance keeps upstream pin, source hashes, and unchanged or
adapted classification in its receipt.

## Review checklist

- Did process, Context, Page, handle, transport, or lease ownership change?
- Can cancellation consume authority without publishing terminal state?
- Can timeouts multiply by target or phase?
- Can peer disconnect strand a caller or driver task?
- Do maps/queues retire exact entries at terminal state?
- Did a binding duplicate Rust state or accept an unsupported option?
- Does `default = []` still have the expected dependency closure?
- Are tests deterministic AAA tests with postcondition assertions?
- Are docs, `docs/MANIFEST.md`, tasks, and evidence boundaries accurate?

## Documentation and priorities

Repository Markdown is canonical; the website is a rendered artifact. Updated
Markdown starts with `agent-updated`; add/remove entries in `docs/MANIFEST.md`.
Historical receipts stay labeled historical.

Priority order: popup lineage; operation-drain race; cancellation-safe single-
deadline close; retry-safe lease release and Drop contract; extraction from the
Node host; typed errors/full failure matrix; then consumer-driven Chromium API
coverage with scoped official-test evidence.
