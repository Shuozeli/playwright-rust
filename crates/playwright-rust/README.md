<!-- agent-updated: 2026-11-14T05:28:56Z -->

# playwright-rust

The no-default-features umbrella for Playwright Rust v2.

Playwright Rust capabilities remain independently usable without this crate. The
umbrella provides a convenient, explicit installation surface and adds no
implicit browser download, browser launch, Node.js process, runtime, transport,
server, or capability.

```toml
# Typed navigation contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["navigation"] }

# Typed value-evaluation contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["evaluation"] }

# Typed Page screenshot contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["content"] }

# Typed BrowserContext lifecycle contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["context-lifecycle"] }

# Typed BrowserContext creation plus its required lifecycle contract
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["context-creation"] }

# Typed Page lifecycle contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["page-lifecycle"] }

# Typed BrowserContext.new_page plus its required Page lifecycle contract
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["page-creation"] }

# Typed JSHandle/ElementHandle explicit-disposal contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["handle-lifecycle"] }

# Typed retained-Handle creation and operations plus required disposal contract
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["handle-evaluation"] }

# Typed Locator contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["locator"] }

# Typed Locator screenshot contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["locator-screenshot"] }

# Typed read-only Frame DOM serialization contract only
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["dom-content"] }

# Playwright-shaped Page navigation client, without CDP or a server
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-navigation"] }

# Playwright-shaped Frame evaluation client, without CDP or a server
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-evaluation"] }

# Playwright-shaped Page screenshot client, without CDP or a server
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-content"] }

# Playwright-shaped BrowserContext.close client, without CDP or a server
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-context-lifecycle"] }

# Playwright-shaped Browser.new_context client plus Context.close
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-context-creation"] }

# Playwright-shaped Page close and before-unload client
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-page-lifecycle"] }

# Playwright-shaped BrowserContext.new_page client plus Page lifecycle
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-page-creation"] }

# Playwright-shaped JSHandle/ElementHandle dispose client
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-handle-lifecycle"] }

# Playwright-shaped retained-Handle client plus required disposal client
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-handle-evaluation"] }

# Playwright-shaped Locator client, without CDP or a server
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-locator"] }

# Playwright-shaped Locator screenshot client, without CDP or a server
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-locator-screenshot"] }

# Playwright-shaped Frame/Page content client, without CDP or a server
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["client-dom-content"] }

# Attach-only Chromium navigation server composition, without a client,
# browser provider, launcher, or installer
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-navigation"] }

# Attach-only Chromium Frame evaluation server composition
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-evaluation"] }

# Attach-only Chromium Page screenshot server composition
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-content"] }

# Attach-only Chromium BrowserContext.close composition with an injected
# descendant-retirement plugin and caller-owned runtime/topology
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-context-lifecycle"] }

# Attach-only Chromium Browser.newContext + BrowserContext.close composition;
# caller owns CDP, topology, runtime, ObjectId source, and continuation queues
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-context-creation"] }

# Attach-only Chromium Page lifecycle composition with exact target confirmation
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-page-lifecycle"] }

# Attach-only Chromium BrowserContext.newPage + Page lifecycle composition
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-page-creation"] }

# Caller-polled Chromium HTTP lifecycle composition with an ordered remote
# queue plus injected batch-driver and dynamic subscription-selection ports
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-network-events"] }

# Attach-only Chromium Handle lifecycle with lifecycle-owned bounded routes,
# injected native disposers, and caller-owned continuation capacity
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-handle-lifecycle"] }

# Attach-only Chromium Handle evaluation 1.2 plus required lifecycle leaf
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-handle-evaluation"] }

# Attach-only Chromium Locator server composition with no browser owner
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-locator"] }

# Attach-only Chromium Locator screenshot server composition; selector
# resolution is an injected plugin and does not install public Locator commands
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-locator-screenshot"] }

# Attach-only Chromium Frame DOM-content server composition
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", features = ["chromium-dom-content"] }
```

Executor and byte-stream adapters are separate choices. Select `runtime-tokio`
or `transport-tokio` individually, or `tokio` for both.

## Feature map

| Feature | Adds |
| --- | --- |
| `core` | Dependency-free service, protocol, identity, and resource contracts |
| `navigation` | Portable `playwright.rust.navigation` command contract |
| `evaluation` | Portable, value-only `playwright.rust.evaluation` command contract |
| `content` | Portable `playwright.rust.content` Page screenshot contract |
| `context-lifecycle` | Portable `playwright.rust.context_lifecycle` BrowserContext close contract |
| `context-creation` | Portable `playwright.rust.context_creation` Browser creation contract plus its required Context lifecycle contract |
| `page-lifecycle` | Portable `playwright.rust.page_lifecycle` ordinary close and before-unload contracts |
| `page-creation` | Portable `playwright.rust.page_creation` Page creation contract plus its required Page lifecycle contract |
| `handle-lifecycle` | Portable `playwright.rust.handle_lifecycle` JSHandle/ElementHandle disposal contract |
| `handle-evaluation` | Portable `playwright.rust.handle_evaluation` 1.2 contract plus required Handle lifecycle |
| `ordered-handle-delivery` | Backend-neutral remote Handle `Create -> Result` and ordered `Dispose` fences with bounded Result-abandonment cleanup |
| `locator` | Portable `playwright.rust.locator` 1.x and 2.x command contracts |
| `locator-screenshot` | Portable `playwright.rust.locator_screenshot` command contract without public Locator commands |
| `dom-content` | Portable, read-only `playwright.rust.dom_content` Frame command contract |
| `page-events` | Portable Page load, Context Page, and popup event contracts |
| `network-events` | Portable bounded HTTP request/response lifecycle event contracts |
| `client` | Typed Browser/Context/Page/Frame/Handle objects |
| `client-navigation` | `client` + `navigation` + Page navigation extension traits |
| `client-evaluation` | `client` + `evaluation` + Frame value-evaluation extension trait |
| `client-content` | `client` + `content` + Page screenshot extension trait |
| `client-context-lifecycle` | `client` + `context-lifecycle` + BrowserContext close extension trait |
| `client-context-creation` | `client` + `context-creation` + required lifecycle client + Browser creation extension trait |
| `client-page-lifecycle` | `client` + `page-lifecycle` + Page close and before-unload extension traits |
| `client-page-creation` | `client` + `page-creation` + required lifecycle client + BrowserContext Page creation extension trait |
| `client-handle-lifecycle` | `client` + `handle-lifecycle` + JSHandle/ElementHandle disposal extension trait |
| `client-handle-evaluation` | `client` + `handle-evaluation` + required lifecycle client + Handle operation extension traits |
| `client-locator` | `client` + `locator` + Playwright-shaped Locator factories and actions |
| `client-locator-screenshot` | `client` + `locator-screenshot` + Locator screenshot extension trait |
| `client-dom-content` | `client` + `dom-content` + Frame/Page content extension traits |
| `client-page-events` | `client` + `page-events` + Page/Context event subscription helpers |
| `client-network-events` | `client` + `network-events` + Page/Context network subscription helpers |
| `cdp` | Raw Chromium CDP client |
| `chromium-content` | `cdp` + `content` + minimal generated Chromium Page screenshot server composition |
| `chromium-context-lifecycle` | `cdp` + `runtime` + Context lifecycle transaction, generated server, and injected descendant-retirement plugin |
| `chromium-context-creation` | `cdp` + `runtime` + creation/lifecycle pair, portable identity publication, exact native compensation, and injected publication/retirement plugins |
| `chromium-page-lifecycle` | `cdp` + `runtime` + exact-target close transaction, before-unload session routing, and injected retirement plugin |
| `chromium-page-creation` | `cdp` + `runtime` + Page creation/lifecycle pair, Page/Frame publication, exact target compensation, and injected publication/retirement plugins |
| `chromium-page-events` | `cdp` + `transport` + three independently bounded Page-event lanes and caller-polled load normalization |
| `chromium-network-events` | `cdp` + `runtime` + `transport` + one ordered network queue, creation fence, and retained receive/publish/ack backend over injected driver and subscription ports |
| `chromium-handle-lifecycle` | `cdp` + `runtime` + bounded shared Handle routes, injected object-local native disposal, generated server, and caller-owned continuation |
| `chromium-handle-evaluation` | `cdp` + `runtime` + Handle evaluation 1.2 over required lifecycle routes, atomic single/batch publication, native compensation, and caller-owned continuation |
| `chromium-locator` | `cdp` + `locator` + minimal generated Chromium Locator 1.22/2.5 server composition |
| `chromium-selector-geometry` | `cdp` + `locator-screenshot` + independently usable first-party selector-to-geometry plugin |
| `chromium-locator-screenshot` | `chromium-selector-geometry` + generic injected-resolver capture/wire composition |
| `chromium-evaluation` | `cdp` + `evaluation` + minimal generated Chromium evaluation server composition |
| `chromium-dom-content` | `cdp` + `dom-content` + minimal generated Chromium DOM-content server composition |
| `chromium-navigation` | `cdp` + `navigation` + minimal generated Chromium navigation server composition |
| `runtime` | Executor-neutral bounded command runtime |
| `runtime-tokio` | Tokio deadline and cleanup adapter |
| `transport` | Executor-neutral bounded frame protocol |
| `transport-tokio` | Tokio byte-stream transport adapter |
| `tokio` | `runtime-tokio` + `transport-tokio` convenience selection |

The umbrella intentionally has no `full` feature until the complete supported
feature topology is defined. Production applications should select only the
pieces they use.

Every optional dependency also carries a `0.1.0` version requirement so the
same manifest is ready for crates.io after the v2 leaf crates are published in
dependency order.
