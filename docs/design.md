<!-- agent-updated: 2026-11-14T02:34:45Z -->

# Design decisions to settle

This is design work in progress, not a stable public API contract.

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

The design favors compile-time composition over unrestricted dynamic Rust
library loading. Browser download and launch must be opt-in. Chromium's CDP
backend is not a universal engine protocol. See the detailed
[v2 architecture](architecture-v2.md) and [resource boundaries](resource-boundaries-v2.md)
for the current proposals and earlier decisions.
