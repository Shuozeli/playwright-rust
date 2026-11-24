<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Roadmap

## Milestone 1: safe primary remote-CDP client

- Attribute popup and indirect targets.
- Eliminate the operation-drain publication race.
- Make close cancellation-safe and deadline-bounded.
- Make lease release retry-stable.
- Define Drop behavior.

Exit criterion: a shared logged-in profile retains every pre-existing target
across normal close, cancellation, timeout, and peer revocation.

## Milestone 2: small Rust composition

- Extract the generic session layer from the Node host.
- Reduce the remote facade's normal dependency closure.
- Keep handshake headers and lease hooks independently selectable.

Exit criterion: the high-level Rust consumer does not depend on the Node
compatibility composition root.

## Milestone 3: consumer-driven API coverage

- Add the crawler/scraper APIs required by real Rust consumers.
- Align Node and Python only through the same Rust-owned capabilities.
- Expand typed errors and the deterministic failure matrix.

Exit criterion: each advertised operation has contract, live-browser, and
scoped upstream evidence appropriate to the claim.

## Later

- Published crates and language packages.
- Release automation and multi-platform artifacts.
- Performance and memory budgets with reproducible benchmarks.

Firefox and WebKit are not roadmap items.
