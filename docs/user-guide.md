<!-- agent-updated: 2026-11-21T21:04:31Z -->

# User guide

Playwright Rust is a Chromium-only, Playwright-shaped automation workspace. Its
Rust crates are composable and default to no capabilities. Node and Python are
compatibility facades over the same Rust host. The project is pre-release: use
a pinned Git revision or workspace checkout and follow the support matrix
instead of assuming complete upstream Playwright parity.

## Choose an entry point

| Caller | Entry point | Use it when |
| --- | --- | --- |
| Rust scraper or BCP adapter | `playwright-rust::remote_cdp` | Attach to an externally managed Chromium endpoint with the supported high-level workflow. |
| Rust composition author | Individual `playwright-rust-*` crates | Build a smaller or differently composed host. |
| Node.js | `@shuozeli/playwright-rust` | Existing JavaScript expects Playwright-shaped objects; the package is workspace-private today. |
| Python | `shuozeli-playwright-rust`, imported as `playwright_rust` | Existing Python expects async or sync Playwright-shaped objects; the package is pre-alpha. |

Firefox, WebKit, browser downloads, and a bundled Chromium are not product
targets. The remote-CDP client borrows a process that another system owns.

## Rust remote-CDP quick start

Pin a reviewed revision. `default-features = false` is intentional;
`cdp-handshake-headers` includes the high-level remote-CDP facade.

```toml
[dependencies]
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", rev = "<commit>", default-features = false, features = ["cdp-handshake-headers"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The default Context of a persistent profile is *borrowed*. Use it for that
profile's cookies and login state. `browser.new_page()` instead creates a new
owned incognito Context and a Page inside it.

```rust,no_run
use std::time::Duration;
use playwright_rust::remote_cdp::{
    CdpHandshakeHeaders, GotoOptions, RemoteCdpConnectOptions,
    ScreenshotOptions, WaitUntil, connect_over_cdp,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let headers = CdpHandshakeHeaders::new(vec![
        ("X-Example-Lease-Id".into(), "lease-123".into()),
        ("X-Example-Fencing-Token".into(), "7".into()),
    ])?;
    let options = RemoteCdpConnectOptions {
        connect_timeout: Duration::from_secs(10),
        operation_timeout: Duration::from_secs(30),
        ..RemoteCdpConnectOptions::default().with_headers(headers)
    };
    let browser = connect_over_cdp("ws://agent.example/cdp", options).await?;
    let workflow = async {
        let context = browser.contexts()?.into_iter()
            .find(|context| !context.is_owned())
            .ok_or_else(|| std::io::Error::other("no borrowed default Context"))?;
        let page = context.new_page().await?;
        page.goto("https://example.com", GotoOptions {
            wait_until: WaitUntil::DomContentLoaded,
            timeout: Duration::from_secs(30),
        }).await?;
        println!("final URL: {}", page.url().await?);
        println!("rendered bytes: {}", page.content().await?.len());
        println!("title: {:?}", page.evaluate("document.title").await?);
        println!("links: {}", page.locator("a")?.count().await?);
        let image = page.screenshot(ScreenshotOptions::default()).await?;
        println!("screenshot bytes: {}", image.as_bytes().len());
        page.close().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    }.await;

    // Close even after workflow failure: this retires session-owned targets,
    // disconnects, and invokes the optional external lease hook.
    let close_result = browser.close().await;
    workflow?;
    close_result?;
    Ok(())
}
```

The facade accepts HTTP discovery or a WebSocket endpoint. Custom headers are
forwarded to discovery and the upgrade. BCP acquisition, renewal, fencing, and
protos stay in an external adapter that supplies URL, headers, and an optional
generic `RemoteLease` release hook.

## Ownership and close semantics

| Resource | Owner | `browser.close()` behavior |
| --- | --- | --- |
| Remote Chrome process | External system | Never sends `Browser.close`; never terminates Chrome. |
| Context/Page visible before attach | External system | Leaves it alive; borrowed Context close is rejected. |
| Context from `browser.new_context()` | This attachment | Closes the Context and descendants. |
| Page created in a borrowed Context | This attachment | Closes that Page, then disconnects. |
| CDP transport | This attachment | Disconnects with bounded local cleanup. |
| Optional `RemoteLease` | External adapter | Calls `release()` at most once during explicit close. |

Explicit Context/Page creation is tracked today. Popup lineage, cancellation-
safe close, one global close deadline, retry-safe lease release, and safe Drop
semantics remain open. Always call `browser.close()` explicitly and make lease
release idempotent. See [Tasks](tasks.md#remote-cdp-shortcut-audit-2026-11-21).

## Rust API surface

| Type | Supported operations |
| --- | --- |
| `Browser` | `is_connected`, `contexts`, `new_context`, `new_page`, `close` |
| `BrowserContext` | `id`, `is_owned`, `pages`, `new_page`, owned `close` |
| `Page` | `goto`, `url`, `content`, `evaluate`, `locator`, `screenshot`, `close` |
| `Locator` | `wait_for`, `click`, `fill`, `text_content`, `count` |
| `RemoteCdpConnectOptions` | connection/command/operation/close/lease timeouts, headers, lease hook |

`WaitUntil` supports `Commit`, `DomContentLoaded`, `Load`, and `NetworkIdle`.
Rust facade timeouts must be nonzero. A peer socket close fails tested in-flight
work as disconnected; callers must still run the local close path.

## Node.js

The workspace package needs Node 20+. Select a native profile containing every
capability the application uses.

```js
import { chromium } from '@shuozeli/playwright-rust';
const browser = await chromium.connectOverCDP('ws://agent.example/cdp', {
  headers: { 'X-Example-Lease-Id': 'lease-123' },
});
try {
  const page = await browser.contexts()[0].newPage();
  await page.goto('https://example.com');
  console.log(await page.content());
  await page.close();
} finally {
  await browser.close();
}
```

Nonempty headers require `cdp-handshake-headers` in the selected native
profile. The package is marked private; consume it from this workspace until a
release is published.

## Python

Use `playwright_rust`, not the official `playwright` module.

```python
from playwright_rust.async_api import async_playwright

async def snapshot() -> None:
    async with async_playwright() as playwright:
        browser = await playwright.chromium.connect_over_cdp(
            "ws://agent.example/cdp",
            headers={"X-Example-Lease-Id": "lease-123"},
        )
        try:
            page = await browser.contexts[0].new_page()
            await page.goto("https://example.com")
            print(await page.content())
            await page.close()
        finally:
            await browser.close()
```

Headers require the corresponding native wheel/profile. The distribution is
pre-alpha, not a claim of full official Python Playwright compatibility.

## Support matrix

| Area | Status | Evidence boundary |
| --- | --- | --- |
| Chromium remote CDP attach | Supported, pre-release | Focused unit/mock/live regressions. |
| Custom handshake headers | Opt-in supported | Mock upgrade verifies forwarding. |
| Explicit owned Context/Page cleanup | Supported | Live Chrome preserves pre-existing targets/process. |
| Borrowed default Context/profile state | Supported in Rust | Live two-attachment cookie regression. |
| Server-side disconnect | Supported for tested pending calls | Proxy-revocation regression; wider matrix open. |
| Popup/indirect ownership | Incomplete | Tracked P0. |
| Drop without explicit close | Not a safe contract | Tracked P1. |
| Firefox/WebKit | Intentionally unsupported | Outside product scope. |
| Full upstream API/test suite | Not achieved | Pinned inventories and scoped receipts only. |

Continue with [Codelabs](codelabs.md), [Architecture](architecture.md), or the
[Agent guide](agent-guide.md).
