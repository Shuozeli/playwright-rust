<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Rust remote-CDP API

The `playwright-rust-remote-cdp` crate is the supported high-level Rust entry
point for attaching to externally owned Chromium.

## Feature selection

| Umbrella feature | Adds |
| --- | --- |
| `remote-cdp` | High-level attach facade |
| `cdp-handshake-headers` | Remote facade plus custom discovery/upgrade headers |

Both preserve `default-features = false`.

## Connect

```rust
pub async fn connect_over_cdp(
    ws_url: impl AsRef<str>,
    options: RemoteCdpConnectOptions,
) -> Result<Browser, RemoteCdpError>
```

The URL may be an HTTP discovery endpoint or browser WebSocket endpoint.

## Connection options

| Field | Default | Purpose |
| --- | --- | --- |
| `connect_timeout` | 30 seconds | Entire attach attempt |
| `command_timeout` | 30 seconds | Ordinary CDP response wait |
| `operation_timeout` | 30 seconds | High-level facade operations |
| `close_timeouts` | Host defaults | Graceful and forced local retirement |
| `lease_release_timeout` | 10 seconds | External lease hook |
| `remote_lease` | None | Generic external release notification |
| `headers` | Empty | Discovery and WebSocket upgrade headers; feature-gated |

Timeouts must be nonzero.

## Browser

| Method | Behavior |
| --- | --- |
| `is_connected()` | Reports whether the attachment remains usable |
| `contexts()` | Enumerates visible borrowed and owned Contexts |
| `new_context()` | Creates an owned incognito Context |
| `new_page()` | Creates an owned incognito Context and Page |
| `close()` | Cleans owned targets, disconnects, and attempts lease release |

## BrowserContext

| Method | Behavior |
| --- | --- |
| `id()` | Returns the internal Context identifier |
| `is_owned()` | Distinguishes attachment-created from borrowed Contexts |
| `pages()` | Enumerates visible Pages in the Context |
| `new_page()` | Creates a Page owned by this attachment |
| `close()` | Closes an owned Context; borrowed Context close is rejected |

## Page

| Method | Behavior |
| --- | --- |
| `goto(url, options)` | Navigates with an explicit wait state and timeout |
| `url()` | Returns the current main-frame URL |
| `content()` | Returns full rendered HTML |
| `evaluate(expression)` | Evaluates JavaScript in the main frame |
| `locator(selector)` | Creates a CSS Locator recipe |
| `screenshot(options)` | Captures the Page |
| `close()` | Closes the Page or its implicit owned Context |

`WaitUntil` supports `Commit`, `DomContentLoaded`, `Load`, and `NetworkIdle`.

## Locator

The high-level facade currently exposes `wait_for`, `click`, `fill`,
`text_content`, and `count`. It is a focused scraper workflow, not the complete
upstream Locator API.

## Errors

`RemoteCdpError` distinguishes invalid options, connection timeout,
disconnection, operation failure, lease failure, and close failure. The current
host still maps some target closure and transport failures too broadly; typed
closure classification remains open work.

## Close contract

Call `Browser::close` explicitly. Dropping the final handle disconnects local
state but is not yet a complete asynchronous cleanup contract for every owned
target or external lease.

Continue with [Ownership and lifecycle](../concepts/ownership-lifecycle.md).
