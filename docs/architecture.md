<!-- agent-updated: 2026-11-14T22:51:06Z -->

# Architecture

This repository is the independent v2 Rust workspace. Its default-free
`playwright-rust` umbrella selects capability crates explicitly. Typical dependency
direction is:

```text
Node/Python facade -> native FFI -> Rust host/composition
                                  -> capability contracts and codecs
                                  -> Chromium CDP backend (when selected)
                                  -> runtime/transport adapters (when selected)
```

`playwright-rust-core` defines dependency-light composition contracts. Capability
crates model one behavior at a time; client and server crates provide typed
delivery paths. Chromium adapters are separate from portable contracts.
The Node and Python layers should translate public values and callbacks, while
Rust owns browser state and core operation semantics. The current Node host is
still a broad compatibility composition root; its footprint and feature edges
need further audit.

The Chromium network lifecycle stores active Request and Response state in
allocator-backed dynamic maps, matching upstream Playwright rather than
enforcing a finite request-count admission limit. Bounded event queues,
payload fields, and opt-in retained-data tiers are separate composition leaves;
they must not be multiplied into a fabricated maximum active-request count.

The raw `playwright-rust-cdp` crate provides the shared CDP connection and
message foundation used by Chromium adapters. This repository is
self-contained and does not use cross-repository Cargo path dependencies.

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
