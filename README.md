<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Playwright Rust

Composable Chromium automation in Rust, with Playwright-shaped Node.js and
Python facades.

> **Pre-release.** Playwright Rust is an independent Shuozeli project, not the
> Microsoft Playwright implementation. Chromium over CDP is the only supported
> browser target. Complete Playwright API compatibility is not claimed.

## Why this project exists

Official Playwright is an excellent general browser-automation system, but its
Node.js driver and bundled browser workflow are too large for some remote,
shared-browser deployments. Playwright Rust is designed around three choices:

- **Composable:** every Rust crate has `default = []`; consumers select only
  the capabilities they need.
- **Attach-safe:** a remote-CDP client can borrow a shared, logged-in Chrome
  profile without owning or terminating the Chrome process.
- **Rust-owned behavior:** lifecycle, browser state, and operation semantics
  live in Rust; Node and Python remain compatibility facades.

## Quick start: attach to Chromium

The packages are not yet published to crates.io, npm, or PyPI. Pin a reviewed
Git commit:

```toml
[dependencies]
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", rev = "<commit>", default-features = false, features = ["cdp-handshake-headers"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust,no_run
use playwright_rust::remote_cdp::{
    GotoOptions, RemoteCdpConnectOptions, connect_over_cdp,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let browser = connect_over_cdp(
        "http://127.0.0.1:9222",
        RemoteCdpConnectOptions::default(),
    ).await?;

    let context = browser.contexts()?.into_iter()
        .find(|context| !context.is_owned())
        .ok_or_else(|| std::io::Error::other("no default Context"))?;
    let page = context.new_page().await?;
    page.goto("https://example.com", GotoOptions::default()).await?;
    println!("{}", page.content().await?);

    page.close().await?;
    browser.close().await?;
    Ok(())
}
```

Use the borrowed default Context when you need the attached profile's cookies
and login state. `browser.new_page()` instead creates an owned incognito
Context.

## Supported remote-CDP workflow

| Area | Current support |
| --- | --- |
| Connection | HTTP discovery or WebSocket endpoint; optional handshake headers |
| Contexts | Enumerate borrowed Contexts; create owned Contexts |
| Pages | Enumerate and create Pages; attach-safe explicit close |
| Navigation | `goto` with commit, DOM-content-loaded, load, or network-idle waiting |
| Content | Final URL, rendered HTML, JavaScript evaluation, screenshots |
| Locators | Wait, click, fill, text content, and count |
| Shutdown | Close explicitly owned targets, disconnect, then notify an optional lease hook |

Popup ownership, cancellation-safe teardown, retry-stable lease release, Drop
semantics, and complete upstream compatibility remain open. See the
[support matrix](docs/reference/support-matrix.md) and
[known gaps](docs/status/known-gaps.md).

## Choose your interface

| Caller | Start here |
| --- | --- |
| Rust remote-CDP consumer | [Getting started](docs/getting-started/remote-cdp.md) |
| Rust API consumer | [Rust API reference](docs/reference/rust-api.md) |
| Node.js consumer | [Node.js guide](docs/guides/node.md) |
| Python consumer | [Python guide](docs/guides/python.md) |
| Maintainer | [Development guide](docs/contributing/development.md) |
| Coding agent | [Agent guide](docs/contributing/agent-guide.md) |

## Architecture in one picture

```text
Rust / Node / Python caller
          |
          v
high-level facade or language binding
          |
          v
Rust host + selected capability compositions
          |
          v
Chromium CDP provider -> borrowed WebSocket -> externally owned Chrome
```

The attach path has no browser-process lease and must never send
`Browser.close`. External systems such as browser-control-plane own profile
allocation, lease renewal, fencing, and process lifetime.

Read [Architecture](docs/concepts/architecture.md) and
[Ownership and lifecycle](docs/concepts/ownership-lifecycle.md) for the full
boundary.

## Repository layout

```text
crates/       composable Rust contracts, providers, hosts, and FFI crates
packages/     Playwright-shaped Node.js and Python packages
docs/         current guides, concepts, references, decisions, and evidence
tests/        pinned compatibility fixtures and runners
vendor/       modified third-party transport crates with preserved licenses
```

## Verify a checkout

```sh
cargo fmt --all --check
CARGO_INCREMENTAL=0 cargo test --release --locked \
  -p playwright-rust-remote-cdp --features cdp-handshake-headers
CARGO_INCREMENTAL=0 cargo clippy --release --locked \
  -p playwright-rust-remote-cdp --features cdp-handshake-headers \
  --all-targets -- -D warnings
```

Use `pnpm` for Node work and `uv` for Python work. Browser tests must use a
dedicated automation profile, never a personal Chrome profile.

## Documentation

The [documentation index](docs/README.md) separates current user documentation,
maintainer material, decisions, evidence, and archived working notes. Large
historical ledgers are retained for provenance but are not current API
contracts.

## License

The repository is licensed under [MIT](LICENSE). Adapted Playwright material
and forked transport code retain their own notices and licenses.
