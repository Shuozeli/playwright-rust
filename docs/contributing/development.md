<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Development guide

## Toolchain

- Rust stable with Cargo.
- Node.js 20+ and `pnpm` for Node packages.
- `uv` for Python environments and one-off tools.
- A dedicated Chromium CDP profile for live browser tests.

Prefer release-mode Rust commands and disable incremental compilation for large
one-off builds.

## Repository map

| Path | Purpose |
| --- | --- |
| `crates/playwright-rust` | Default-empty umbrella crate |
| `crates/playwright-rust-cap-*` | Portable capability contracts |
| `crates/playwright-rust-provider-cdp` | CDP provider |
| `crates/playwright-rust-browser-provider` | Launch/attach ownership boundary |
| `crates/playwright-rust-remote-cdp` | High-level Rust attach facade |
| `crates/playwright-rust-node-host` | Broad compatibility host; currently too central |
| `packages/playwright-rust` | Node.js facade |
| `packages/playwright-rust-python` | Python facade |
| `docs/evidence` | Source-bound compatibility records |

## Add a capability

1. Define one portable behavior and its typed values.
2. Implement it in the Chromium provider/backend layer.
3. Add opt-in composition and umbrella features.
4. Keep Node and Python as translations of the Rust-owned behavior.
5. Add deterministic Arrange-Act-Assert tests.
6. Verify positive and negative dependency closures.
7. Update the support matrix and relevant guide.

Do not add an API-shaped stub that silently lacks an implementation.

## Documentation changes

- Keep current user documents short and navigable.
- Put durable decisions in ADRs.
- Put active work in `docs/tasks.md` and `docs/status`.
- Put source-bound results in `docs/evidence`.
- Move superseded working notes to `docs/archive`; do not delete provenance.
- Update `docs/MANIFEST.md` when destinations or triggers change.

## Review focus

Lifecycle changes require explicit reasoning about admission, ownership,
cancellation, deadlines, peer disconnect, retained state, and repeated close.
Compatibility changes require scoped upstream evidence rather than method-name
counts alone.

Continue with [Testing](testing.md) and the [Agent guide](agent-guide.md).
