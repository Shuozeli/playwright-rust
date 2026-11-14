<!-- agent-updated: 2026-11-14T02:34:45Z -->

# Architecture

This repository is the independent v2 Rust workspace. Its default-free
`pwright` umbrella selects capability crates explicitly. Typical dependency
direction is:

```text
Node/Python facade -> native FFI -> Rust host/composition
                                  -> capability contracts and codecs
                                  -> Chromium CDP backend (when selected)
                                  -> runtime/transport adapters (when selected)
```

`pwright-core` defines dependency-light composition contracts. Capability
crates model one behavior at a time; client and server crates provide typed
delivery paths. Chromium adapters are separate from portable contracts.
The Node and Python layers should translate public values and callbacks, while
Rust owns browser state and core operation semantics. The current Node host is
still a broad compatibility composition root; its footprint and feature edges
need further audit.

The raw `pwright-cdp` crate is shared source copied from the original repo
because v2 Chromium adapters depend on it. The original copy stays in
`Shuozeli/pwright` to keep its v1 CLI working; this repo is self-contained and
does not use a cross-repository Cargo path dependency.

Two transitional test-only crates, `pwright-fake` and `pwright-js`, remain
because 42 v2 crate test suites currently depend on the fake CDP client.
They are not dependencies of the v2 production graph. They should become a
v2-specific test-support crate without the old JavaScript-snippet dependency.

No old v1 CLI, bridge, server executable, or script runner is present. This
workspace does include optional Playwright Server *protocol client* crates;
they are distinct from the old `pwright-server` executable.

For capability-by-capability background, see [architecture-v2](architecture-v2.md)
and the [dependency topology](knowledge/pwright-dependency-topology.md).
