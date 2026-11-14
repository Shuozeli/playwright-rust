<!-- agent-updated: 2026-11-14T02:35:39Z -->

# playwright-rust

Experimental, composable browser automation in Rust, with Playwright-shaped
Node.js and Python bindings. This is an independent work-in-progress repository
extracted from the v2 development branch of
[Shuozeli/pwright](https://github.com/Shuozeli/pwright). It is **not** the
official Microsoft Playwright project and does **not** claim Playwright parity.
The source snapshot was `Shuozeli/pwright` commit
`2a516dc6413d7df8ae84e27a7d20c17358863bd0`.

The current Rust design keeps capabilities, codecs, client/server adapters,
Chromium CDP backend pieces, runtime adapters, and FFI boundaries in separate
crates. The `pwright` umbrella crate has no default features. Browser download,
launch, remote connection, tracing, request interception, and other expensive
behaviors require explicit components or feature selection. Chromium uses CDP;
this is not a claim that Firefox or WebKit can use CDP.

## What is here

- `crates/pwright` and `crates/pwright-*`: Rust v2 contracts, adapters, backend,
  optional capabilities, and development tooling.
- `crates/pwright-node-host` and `crates/pwright-node-addon`, with
  `packages/pwright`: a Node-API boundary and Playwright-shaped JavaScript API.
- `crates/pwright-python-addon`, with `packages/pwright-python`: a PyO3 boundary
  and async/sync Python API.
- `tests/official`, package tests, and `scripts`: conformance fixtures, runners,
  and dependency-boundary checks, not a green full-corpus certification.
- `vendor/pwright-*`: modified WebSocket libraries needed by an optional
  transport; their upstream notices and licenses are retained.

The existing `pwright` CLI, bridge, server, script runner, and CDP generator
remain in the original repository. This repository currently has no v2
user-facing CLI. `pwright-schema-gen` is a development generator, not a browser
automation command.

The Node package name `@shuozeli/pwright` and Python distribution name
`shuozeli-pwright` are retained for compatibility with the WIP code. Both
names are provisional; this repository split is not an official Playwright
release or a published parity promise.

## First checks

From this repository root, with a Rust toolchain installed:

```sh
cargo metadata --locked --format-version 1 --no-deps
CARGO_INCREMENTAL=0 cargo check --release --locked -p pwright --no-default-features
CARGO_INCREMENTAL=0 cargo check --release --locked -p pwright-node-host --no-default-features
CARGO_INCREMENTAL=0 cargo check --release --locked -p pwright-python-addon --no-default-features
```

Native Node and Python distributions need their respective build tools and
explicit feature profiles. See the package READMEs for their WIP instructions.
Use `pnpm` for Node dependencies and `uv` for Python dependencies.

## Evidence and status

This extraction carries source, tests, manifests, and historical design notes,
but deliberately excludes generated native binaries, wheel files, local
`.work/` conformance receipts, and local browser state. Some historical docs
link to those absent local receipts. Their numbers describe earlier WIP
checkpoints, not verification newly performed in this repository. The complete
official Playwright JS/TS and Python test corpora have **not** passed here.
Re-run each relevant test and provenance gate before treating any capability
as verified in the new checkout.

Start with [architecture](docs/architecture.md), [design](docs/design.md),
[tasks](docs/tasks.md), and [codelabs](docs/codelabs.md). The detailed original
v2 design is retained in [architecture-v2](docs/architecture-v2.md).

## License

The repository license is [MIT](LICENSE). Adapted Playwright material and
forked WebSocket code carry additional notices and licenses in their source
directories; preserve them in any downstream distribution.
