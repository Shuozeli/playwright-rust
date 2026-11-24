<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Architecture

Playwright Rust is a set of opt-in Rust capabilities with Chromium CDP as the
only browser backend. The architecture separates portable contracts from the
browser provider, runtime, transport, host composition, and language bindings.

## Design goals

- Consumers install only the capabilities they select.
- Rust owns lifecycle and browser state.
- Remote attachment cannot terminate an externally owned browser.
- Node and Python share the same Rust behavior instead of reimplementing it.
- Compatibility claims remain tied to explicit evidence.

## Non-goals

- Firefox or WebKit implementations.
- A browser downloader or bundled Chromium.
- BCP-specific lease, fencing, or profile schemas.
- One monolithic crate with every behavior enabled.

## Layer model

```text
application
  |
  +-- Rust high-level facade
  +-- Node facade -> native addon
  +-- Python facade -> native addon
                    |
                    v
             host / composition
                    |
          +---------+----------+
          | capability contracts|
          +---------+----------+
                    |
          Chromium CDP provider
                    |
             runtime + transport
                    |
          borrowed or owned Chrome
```

| Layer | Responsibility |
| --- | --- |
| Capability crates | Small, portable behavior contracts and values |
| Provider/backend crates | Translate capabilities into Chromium CDP operations |
| Runtime/transport crates | Scheduling, I/O, WebSocket delivery, and cancellation |
| Host/composition crates | Select capabilities and own browser/session state |
| FFI addons | Convert typed values across the native boundary |
| Node/Python facades | Present Playwright-shaped language APIs |

Dependency direction flows downward. A capability contract must not depend on
Node, Python, or a concrete application.

## Remote attachment path

```text
external adapter
  -> playwright-rust-remote-cdp
  -> generic HostSession (currently housed in playwright-rust-node-host)
  -> playwright-rust-provider-cdp
  -> playwright-rust-browser-provider::AttachedBrowser
  -> borrowed CDP WebSocket
```

`AttachedBrowser` carries connection ownership but no browser-process lease.
The type boundary therefore cannot express permission to terminate Chrome.

The current dependency on `playwright-rust-node-host` is architectural debt.
The target is a generic session layer consumed by the Rust facade and the Node
host independently.

## Composable features

Every workspace crate has `default = []`. The umbrella crate re-exports a
component only when its feature is selected. Higher-level profiles compose
multiple leaves but must preserve the same explicit dependency closure.

Feature selection answers two separate questions:

1. Is the API present at compile time?
2. Did the selected host install the Rust implementation at runtime?

Language facades must fail explicitly when the second answer is no.

## Rust and language bindings

Rust owns:

- Browser, Context, Page, Frame, and Handle state.
- Operation ordering and cancellation.
- CDP sessions and browser topology.
- Resource ownership and terminal cleanup.

Node and Python own:

- Language-native object wrappers.
- Argument and callback conversion.
- Public naming and error projection.

They must not maintain a second authoritative browser-state graph.

## External control planes

A control plane may acquire a profile, renew a lease, enforce fencing, and
proxy CDP. Its adapter hands Playwright Rust only:

- an HTTP or WebSocket URL;
- optional handshake headers;
- timeout configuration; and
- an optional generic release hook.

This keeps Playwright Rust reusable outside any one fleet manager.

## Related documents

- [Ownership and lifecycle](ownership-lifecycle.md)
- [ADR 0002: attach-safe remote CDP](../adr/0002-attach-safe-remote-cdp.md)
- [Rust API reference](../reference/rust-api.md)
- [Known gaps](../status/known-gaps.md)
