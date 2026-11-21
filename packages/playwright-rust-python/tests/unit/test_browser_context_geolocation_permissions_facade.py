# pyright: reportPrivateUsage=false

from __future__ import annotations

import asyncio
from typing import Any, cast

import pytest
from playwright_rust.async_api import Browser, BrowserContext
from playwright_rust.sync_api import BrowserContext as SyncBrowserContext


class NativeGeolocationPermissions:
    def __init__(self) -> None:
        self.geolocation_calls: list[tuple[str, tuple[float, float, float] | None]] = []
        self.grant_calls: list[tuple[str, list[str], str | None]] = []
        self.clear_calls: list[str] = []

    async def set_context_geolocation(
        self,
        context_id: str,
        geolocation: tuple[float, float, float] | None,
    ) -> None:
        self.geolocation_calls.append((context_id, geolocation))

    async def grant_context_permissions(
        self,
        context_id: str,
        permissions: list[str],
        origin: str | None,
    ) -> None:
        self.grant_calls.append((context_id, permissions, origin))

    async def clear_context_permissions(self, context_id: str) -> None:
        self.clear_calls.append(context_id)


def context(native: NativeGeolocationPermissions) -> BrowserContext:
    browser = object.__new__(Browser)
    browser._native = cast(Any, native)
    return BrowserContext(browser, "7", None)


@pytest.mark.asyncio
async def test_context_geolocation_forwards_value_and_clear_to_native() -> None:
    # Arrange
    native = NativeGeolocationPermissions()
    value = context(native)

    # Act
    await value.set_geolocation({"longitude": 10, "latitude": 20, "accuracy": 3})
    await value.set_geolocation(None)

    # Assert
    assert native.geolocation_calls == [("7", (10.0, 20.0, 3.0)), ("7", None)]


@pytest.mark.asyncio
async def test_context_geolocation_rejects_invalid_shape_before_native() -> None:
    # Arrange
    native = NativeGeolocationPermissions()
    value = context(native)

    # Act
    with pytest.raises(TypeError, match="set_geolocation geolocation must be a mapping"):
        await value.set_geolocation(cast(Any, "London"))

    # Assert
    assert native.geolocation_calls == []


@pytest.mark.asyncio
async def test_context_permissions_deduplicate_and_forward_origin() -> None:
    # Arrange
    native = NativeGeolocationPermissions()
    value = context(native)

    # Act
    await value.grant_permissions(
        ["geolocation", "camera", "geolocation"],
        origin="https://example.test/path",
    )

    # Assert
    assert native.grant_calls == [("7", ["geolocation", "camera"], "https://example.test/path")]


@pytest.mark.asyncio
async def test_context_permissions_reject_invalid_collection_before_native() -> None:
    # Arrange
    native = NativeGeolocationPermissions()
    value = context(native)

    # Act
    with pytest.raises(TypeError, match="permissions must be a sequence of strings"):
        await value.grant_permissions(cast(Any, "camera"))

    # Assert
    assert native.grant_calls == []


@pytest.mark.asyncio
async def test_context_permissions_reject_invalid_origin_before_native() -> None:
    # Arrange
    native = NativeGeolocationPermissions()
    value = context(native)

    # Act
    with pytest.raises(TypeError, match="grant_permissions origin must be a string"):
        await value.grant_permissions(["camera"], origin=cast(Any, 7))

    # Assert
    assert native.grant_calls == []


@pytest.mark.asyncio
async def test_context_clear_permissions_forwards_to_native() -> None:
    # Arrange
    native = NativeGeolocationPermissions()
    value = context(native)

    # Act
    await value.clear_permissions()

    # Assert
    assert native.clear_calls == ["7"]


def test_sync_context_geolocation_and_permissions_forward_to_async_owner() -> None:
    # Arrange
    native = NativeGeolocationPermissions()
    async_value = context(native)

    class Bridge:
        def run(self, awaitable: Any) -> None:
            asyncio.run(awaitable)

    sync_value = object.__new__(SyncBrowserContext)
    sync_value._bridge = cast(Any, Bridge())
    sync_value._value = async_value

    # Act
    sync_value.set_geolocation({"longitude": 10, "latitude": 20})
    sync_value.grant_permissions(["notifications"])
    sync_value.clear_permissions()

    # Assert
    assert native.geolocation_calls == [("7", (10.0, 20.0, 0.0))]
    assert native.grant_calls == [("7", ["notifications"], None)]
    assert native.clear_calls == ["7"]
