<!-- agent-updated: 2026-11-14T05:28:56Z -->

# Playwright Rust file-input content ownership

Optional Rust leaf depending only on core and the allocation-free file-input
model. Copies actual byte slices or reads finite opened readers into a
non-cloneable, ledger-admitted batch. No Node, CDP, executor or browser action.

See the [ownership contract, tests and remaining tasks](../../docs/knowledge/playwright-rust-file-input.md#node-142-ledger-admitted-content-ownership).

Run `cargo test --release --locked -p playwright-rust-file-input-source`.
Synchronous reads belong in an explicitly admitted embedding executor, not
on a Node/event-loop thread. This is not yet public setInputFiles support.
