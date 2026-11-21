import asyncio
import json
import os
import urllib.request
from typing import Any

import pytest
from playwright_rust.async_api import async_playwright

ENDPOINT = os.environ.get("PLAYWRIGHT_RUST_CDP_ENDPOINT", "")


async def _targets() -> list[dict[str, Any]]:
    def read() -> list[dict[str, Any]]:
        with urllib.request.urlopen(f"{ENDPOINT.rstrip('/')}/json/list", timeout=2) as response:
            return json.load(response)  # type: ignore[no-any-return]

    return await asyncio.to_thread(read)


@pytest.mark.skipif(not ENDPOINT, reason="PLAYWRIGHT_RUST_CDP_ENDPOINT is required")
@pytest.mark.asyncio
async def test_browser_close_retires_only_the_page_created_by_this_attachment() -> None:
    # Arrange
    before = await _targets()
    before_ids = {target["id"] for target in before}
    marker = f"pw-rust-python-owned-{id(before)}"
    playwright = await async_playwright().start()
    browser = await playwright.chromium.connect_over_cdp(
        ENDPOINT,
        headers={
            "X-BCP-Lease-Id": "python-attach-safe-close",
            "X-BCP-Fencing-Token": "1",
        },
    )
    context = browser.contexts[0]
    page = await context.new_page()
    await page.goto(
        f"data:text/html,<title>{marker}</title><main>{marker}</main>",
        wait_until="commit",
    )
    assert marker in await page.content()
    assert await page.evaluate("document.title") == marker
    created_id = next(
        (target["id"] for target in await _targets() if target["title"] == marker), None
    )
    assert created_id is not None

    try:
        # Act
        await asyncio.wait_for(browser.close(), timeout=5)

        # Assert
        after_ids = {target["id"] for target in await _targets()}
        assert created_id not in after_ids
        assert before_ids <= after_ids
        assert not browser.is_connected()
    finally:
        await playwright.stop()
