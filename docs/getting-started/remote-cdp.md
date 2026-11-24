<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Connect to remote Chromium

Use the high-level Rust facade when another system starts and owns Chromium.
The facade attaches over CDP, creates automation objects, and disconnects
without terminating the browser process.

## Prerequisites

- Chromium exposes an HTTP discovery URL or browser WebSocket URL.
- The endpoint is reachable by the caller.
- If the proxy authenticates the WebSocket upgrade, you have the required
  request headers.
- Your application runs Tokio.

The repository does not download or bundle Chromium.

## Add the dependency

The crate is pre-release and not yet published to crates.io. Pin a reviewed
commit:

```toml
[dependencies]
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", rev = "<commit>", default-features = false, features = ["cdp-handshake-headers"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Use `remote-cdp` instead of `cdp-handshake-headers` when the endpoint needs no
custom headers.

## Attach and reuse profile state

An attached persistent profile exposes its existing/default Context as a
borrowed Context. Create a Page inside it to reuse cookies and login state:

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

    let result = async {
        let context = browser.contexts()?.into_iter()
            .find(|context| !context.is_owned())
            .ok_or_else(|| std::io::Error::other("no borrowed default Context"))?;
        let page = context.new_page().await?;
        page.goto("https://example.com", GotoOptions::default()).await?;
        println!("{}", page.content().await?);
        page.close().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    }.await;

    let close_result = browser.close().await;
    result?;
    close_result?;
    Ok(())
}
```

Always call `browser.close()` explicitly, including on the error path.

## Forward handshake headers

```rust,no_run
use playwright_rust::remote_cdp::{
    CdpHandshakeHeaders, RemoteCdpConnectOptions, connect_over_cdp,
};

# async fn attach(url: &str) -> Result<(), Box<dyn std::error::Error>> {
let headers = CdpHandshakeHeaders::new(vec![
    ("X-Example-Lease-Id".into(), "lease-123".into()),
    ("X-Example-Fencing-Token".into(), "7".into()),
])?;
let browser = connect_over_cdp(
    url,
    RemoteCdpConnectOptions::default().with_headers(headers),
).await?;
browser.close().await?;
# Ok(())
# }
```

Headers are sent during HTTP discovery and the WebSocket upgrade.

## Choose borrowed or isolated state

| Call | Result | Profile state |
| --- | --- | --- |
| `browser.contexts()` then `context.new_page()` | Page in a borrowed Context | Reuses the attached profile |
| `browser.new_context()` | Owned incognito Context | Isolated from the attached profile |
| `browser.new_page()` | Owned incognito Context plus Page | Isolated convenience path |

Closing a borrowed Context is rejected. Closing an owned Context closes its
descendants.

## Integrate an external lease

Implement `RemoteLease` in the adapter that owns acquisition and renewal, then
pass it with `with_remote_lease`. The hook is a notification/release boundary;
it does not grant permission to terminate Chrome.

Lease implementations must currently be idempotent. Retry-stable release state
and cancellation behavior remain open work.

## Next steps

- [Rust API reference](../reference/rust-api.md)
- [Ownership and lifecycle](../concepts/ownership-lifecycle.md)
- [Support matrix](../reference/support-matrix.md)
- [Known gaps](../status/known-gaps.md)
