<!-- agent-updated: 2026-11-14T05:29:40Z -->

# Playwright Rust

Playwright Rust is an experimental, composable browser-automation stack written
in Rust, with Playwright-shaped Node.js and Python APIs. It is an independent
Shuozeli project, not the official Microsoft Playwright implementation, and it
does not yet claim full Playwright compatibility.

The design separates capability contracts, codecs, client/server adapters,
browser backends, runtime integrations, and language bindings. Applications
select only the capabilities they need; browser installation, process launch,
remote connections, tracing, request interception, and other expensive
features remain explicit opt-ins. Chromium uses CDP. Firefox and WebKit require
engine-specific backends rather than pretending CDP is universal.

## Repository layout

- `crates/playwright-rust` is the default-free Rust umbrella crate.
- `crates/playwright-rust-*` contains small contracts, codecs, adapters,
  backends, compositions, runtimes, and FFI crates.
- `packages/playwright-rust` is the Node package
  `@shuozeli/playwright-rust`.
- `packages/playwright-rust-python` is the Python distribution
  `shuozeli-playwright-rust`; its import namespace is `playwright_rust`.
- `tests/official` and package-local official-test directories contain pinned
  conformance fixtures and runners.
- `vendor/playwright-rust-*` contains the two modified WebSocket crates used by
  the optional WebSocket transport. Their upstream licenses and notices remain
  intact.

The repository currently exposes library and binding surfaces, but no general
user-facing CLI. `playwright-rust-schema-gen` is a development-time protocol
generator, not a browser automation command.

## Naming

The repository uses one consistent project namespace:

| Surface | Name |
| --- | --- |
| Product and repository | Playwright Rust / `playwright-rust` |
| Rust packages | `playwright-rust-*` |
| Rust identifiers | `playwright_rust_*` |
| Node package | `@shuozeli/playwright-rust` |
| Python distribution | `shuozeli-playwright-rust` |
| Python import | `playwright_rust` |
| Environment variables | `PLAYWRIGHT_RUST_*` |
| Protocol namespace | `playwright.rust.*` |

## First checks

From the repository root:

```sh
cargo metadata --locked --format-version 1 --no-deps
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust --no-default-features
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-node-host --no-default-features
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust-python-addon --no-default-features
```

Use `pnpm` for Node package work and `uv` for Python package work. Native Node
and Python distributions require their package-specific release build steps and
explicit feature profiles.

## Status and evidence

This is active design and implementation work. Historical design notes and
conformance ledgers are retained for engineering context, but their recorded
results are not automatically fresh proof for a renamed build. Treat a
capability as verified only after its current source, dependency boundary,
language binding, and relevant browser lane pass together.

Start with [architecture](docs/architecture.md), [design](docs/design.md),
[tasks](docs/tasks.md), and [codelabs](docs/codelabs.md). The detailed design is
in [architecture-v2](docs/architecture-v2.md).

## License

The repository license is [MIT](LICENSE). Adapted Playwright material and
forked WebSocket code carry additional notices and licenses in their source
directories; preserve them in downstream distributions.
