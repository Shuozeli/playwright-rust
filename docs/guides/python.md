<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Python guide

`shuozeli-playwright-rust` provides async and sync Playwright-shaped APIs under
the `playwright_rust` import namespace. The distribution is pre-alpha and is
not yet published to PyPI.

## Async remote-CDP example

```python
from playwright_rust.async_api import async_playwright


async def snapshot() -> None:
    async with async_playwright() as playwright:
        browser = await playwright.chromium.connect_over_cdp(
            "ws://agent.example/cdp",
            headers={"X-Example-Lease-Id": "lease-123"},
        )
        try:
            context = browser.contexts[0]
            page = await context.new_page()
            await page.goto("https://example.com")
            print(await page.content())
            await page.close()
        finally:
            await browser.close()
```

## Sync remote-CDP example

```python
from playwright_rust.sync_api import sync_playwright


def snapshot() -> None:
    with sync_playwright() as playwright:
        browser = playwright.chromium.connect_over_cdp(
            "ws://agent.example/cdp"
        )
        try:
            page = browser.contexts[0].new_page()
            page.goto("https://example.com")
            print(page.content())
            page.close()
        finally:
            browser.close()
```

## Native profiles

Handshake headers require a wheel built with `cdp-handshake-headers`. Other
features are selected independently. Unsupported options raise a typed
`UnsupportedError` instead of silently falling back.

## Workspace development

Use the pinned build configuration in `packages/playwright-rust-python`:

```sh
cd packages/playwright-rust-python
uv run --with maturin==1.15.0 maturin develop --release
uv run pytest tests/unit
```

## Compatibility boundary

The package is not a drop-in replacement for the complete official Python
Playwright distribution. Read the [support matrix](../reference/support-matrix.md)
and [Python evidence ledger](../evidence/python-official-conformance.md) for
scoped, source-bound results.
