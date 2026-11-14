<!-- agent-updated: 2026-11-14T05:28:56Z -->

# playwright-rust-file-input-model

Dependency-free, allocation-free preparation policy for a future optional Rust
file-input capability. It classifies path/payload inputs, enforces explicit
metadata limits and pinned Playwright 1.58.2 transfer limits, plans native versus
buffered delivery, and validates post-retarget input multiplicity/directory flags.

It performs no filesystem access, encoding, browser operation or Node binding.
Supplied metadata does not certify actual file/Buffer storage or admit the later
operation's allocations. Filesystem, MIME, time and ownership belong to separately
injected and resource-admitted boundaries. Ordinary host/addon builds do not
depend on this crate.

[Design, upstream evidence and remaining work](../../docs/knowledge/playwright-rust-file-input.md)

```sh
CARGO_INCREMENTAL=0 cargo test --release --locked -p playwright-rust-file-input-model
node scripts/check-v2-file-input-dependencies.mjs
```
