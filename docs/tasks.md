<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Active tasks

This file contains only current, ordered work. Historical completed items are
preserved in [archive/task-ledger-2026-11.md](archive/task-ledger-2026-11.md).

## P0: protect shared remote browsers

- [ ] Attribute popups and indirect page-like targets to the attachment that
  created them.
- [ ] Fence or reconcile target creation that crosses the operation-drain
  deadline.

## P1: make teardown cancellation-safe

- [ ] Install the disconnect guard before the first cancellable close await.
- [ ] Enforce one absolute graceful deadline and one forced-local deadline.
- [ ] Make lease release cancellation-safe, retry-stable, and observable by
  repeated `close()` calls.
- [ ] Define and test Drop behavior for Rust, Node, and Python callers.

## P1: restore the intended dependency boundary

- [ ] Extract the generic remote session layer from `playwright-rust-node-host`.
- [ ] Keep the Rust facade dependent only on the selected CDP/provider and
  workflow capabilities.

## P2: improve compatibility clarity

- [ ] Distinguish transport disconnect from Context, Page, and Frame closure.
- [ ] Expand the remote-CDP failure matrix with deterministic tests.
- [ ] Add Chromium APIs only from consumer demand and pair each addition with
  scoped official-test evidence.

See [Known gaps](status/known-gaps.md) for impact and acceptance criteria, and
[Roadmap](status/roadmap.md) for milestone ordering.
