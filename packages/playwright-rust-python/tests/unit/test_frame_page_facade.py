"""A Frame exposes its owning Page in both Python facades."""

from __future__ import annotations

from typing import Any, cast

from playwright_rust.async_api import Frame as AsyncFrame
from playwright_rust.async_api import Page as AsyncPage
from playwright_rust.sync_api import Frame as SyncFrame
from playwright_rust.sync_api import Page as SyncPage


def test_async_frame_page_returns_the_owning_page() -> None:
    # Arrange
    page = cast(AsyncPage, object())
    frame = AsyncFrame(page, "child-frame", "main-frame", "child", "about:blank")

    # Act
    owner = frame.page

    # Assert
    assert owner is page


def test_sync_frame_page_returns_the_wrapped_owning_page() -> None:
    # Arrange
    async_page = cast(AsyncPage, object())
    async_frame = AsyncFrame(async_page, "child-frame", "main-frame", "child", "about:blank")
    page = cast(SyncPage, object())
    frame = SyncFrame(cast(Any, object()), async_frame, page)

    # Act
    owner = frame.page

    # Assert
    assert owner is page
