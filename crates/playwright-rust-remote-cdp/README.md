<!-- agent-updated: 2026-11-23T03:34:47Z -->

# playwright-rust-remote-cdp

High-level, attach-only Chromium CDP client. It borrows an externally managed
Chrome process, tracks explicitly created resources, disconnects without
sending `Browser.close`, and can notify a generic external lease authority.

The crate has `default = []`. Enable `cdp-handshake-headers` to forward custom
headers through HTTP discovery and the WebSocket upgrade.

```rust,no_run
use playwright_rust_remote_cdp::{
    CdpHandshakeHeaders, RemoteCdpConnectOptions, connect_over_cdp,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let headers = CdpHandshakeHeaders::new(vec![
        ("X-Example-Lease-Id".into(), "lease-123".into()),
    ])?;
    let browser = connect_over_cdp(
        "ws://agent.example/cdp",
        RemoteCdpConnectOptions::default().with_headers(headers),
    ).await?;
    let context = browser.contexts()?.into_iter()
        .find(|context| !context.is_owned())
        .ok_or_else(|| std::io::Error::other("no borrowed default Context"))?;
    let page = context.new_page().await?;
    page.goto("https://example.com", Default::default()).await?;
    println!("{}", page.content().await?);
    page.close().await?;
    browser.close().await?;
    Ok(())
}
```

The borrowed default Context reuses profile cookies/login. `Browser::new_page`
creates an owned incognito Context. Explicit close is required; lease release
should be idempotent. Popup lineage, cancellation safety, one global close
deadline, retry-safe lease release, and Node-host decoupling remain open.

Continue with the [remote-CDP guide](../../docs/getting-started/remote-cdp.md),
[Rust API reference](../../docs/reference/rust-api.md), and [ownership and
lifecycle contract](../../docs/concepts/ownership-lifecycle.md). Maintainers
should also read the [known gaps](../../docs/status/known-gaps.md).
