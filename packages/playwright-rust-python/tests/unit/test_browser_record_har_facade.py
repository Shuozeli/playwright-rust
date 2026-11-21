# pyright: reportPrivateUsage=false

from __future__ import annotations

import asyncio
import re
from collections.abc import Coroutine
from pathlib import Path
from typing import Any, cast

import pytest
from playwright_rust import InvalidArgumentError, UnsupportedError
from playwright_rust.async_api import Browser
from playwright_rust.sync_api import Browser as SyncBrowser


class NativeHar:
    def __init__(self) -> None:
        self.created = False
        self.record_calls: list[tuple[object, ...]] = []
        self.closed: list[str] = []
        self.fail_recording = False

    async def new_context(self, *_args: object, **_kwargs: object) -> tuple[str, None]:
        self.created = True
        return "context-1", None

    async def context_route_from_har(self, *args: object) -> str:
        self.record_calls.append(args)
        if self.fail_recording:
            raise InvalidArgumentError("recording failed")
        return "registration-1"

    async def context_record_har_regexp(self, *args: object) -> str:
        self.record_calls.append(args)
        if self.fail_recording:
            raise InvalidArgumentError("recording failed")
        return "registration-1"

    async def close_context(self, context_id: str) -> None:
        self.closed.append(context_id)


def browser(native: NativeHar) -> Browser:
    value = object.__new__(Browser)
    value._native = cast(Any, native)
    value._contexts = {}
    value._pending_context_ids = set()
    value._pending_context_cleanup_tasks = {}
    value._unidentified_context_creations = 0
    value._persistent_context_viewport = None
    value._closed = False
    return value


@pytest.mark.asyncio
@pytest.mark.parametrize(
    ("name", "expected_content"),
    [("capture.har", "embed"), ("capture.zip", "attach")],
)
async def test_record_har_creation_uses_upstream_full_and_path_content_defaults(
    name: str,
    expected_content: str,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Arrange
    monkeypatch.setattr(
        "playwright_rust.async_api._native.selected_features", lambda: ["har-recording"]
    )
    native = NativeHar()
    value = browser(native)
    path = tmp_path / name

    # Act
    context = await value.new_context(record_har_path=path)

    # Assert
    assert context._object_id == "context-1"
    assert native.record_calls == [
        ("context-1", str(path), None, None, True, expected_content, "full")
    ]
    assert native.closed == []


@pytest.mark.asyncio
async def test_explicit_record_har_options_override_legacy_omit_flag(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Arrange
    monkeypatch.setattr(
        "playwright_rust.async_api._native.selected_features", lambda: ["har-recording"]
    )
    native = NativeHar()
    value = browser(native)
    path = tmp_path / "capture.har"

    # Act
    await value.new_context(
        record_har_path=path,
        record_har_omit_content=True,
        record_har_url_filter="**/api/**",
        record_har_mode="minimal",
        record_har_content="embed",
    )

    # Assert
    assert native.record_calls == [
        ("context-1", str(path), "**/api/**", None, True, "embed", "minimal")
    ]


@pytest.mark.asyncio
async def test_record_har_compiled_regex_uses_separate_native_capability(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Arrange
    monkeypatch.setattr(
        "playwright_rust.async_api._native.selected_features",
        lambda: ["har-recording", "har-recording-regexp"],
    )
    native = NativeHar()
    value = browser(native)
    path = tmp_path / "capture.har"

    # Act
    await value.new_context(
        record_har_path=path,
        record_har_url_filter=re.compile("HAR.X?HTML", re.IGNORECASE),
    )

    # Assert
    assert native.record_calls == [("context-1", str(path), "HAR.X?HTML", "i", ("embed", "full"))]


@pytest.mark.asyncio
async def test_record_har_validation_happens_before_context_creation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Arrange
    native = NativeHar()
    value = browser(native)

    # Act and Assert
    def no_features() -> list[str]:
        return []

    monkeypatch.setattr("playwright_rust.async_api._native.selected_features", no_features)
    with pytest.raises(UnsupportedError, match="har-recording profile"):
        await value.new_context(record_har_path=tmp_path / "capture.har")
    monkeypatch.setattr(
        "playwright_rust.async_api._native.selected_features", lambda: ["har-recording"]
    )
    with pytest.raises(UnsupportedError, match="har-recording-regexp profile"):
        await value.new_context(
            record_har_path=tmp_path / "capture.har",
            record_har_url_filter=re.compile("api"),
        )
    assert native.created is False


@pytest.mark.asyncio
async def test_failed_record_har_initialization_closes_unpublished_context(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Arrange
    monkeypatch.setattr(
        "playwright_rust.async_api._native.selected_features", lambda: ["har-recording"]
    )
    native = NativeHar()
    native.fail_recording = True
    value = browser(native)

    # Act and Assert
    with pytest.raises(InvalidArgumentError, match="recording failed"):
        await value.new_context(record_har_path=tmp_path / "capture.har")
    assert native.closed == ["context-1"]
    assert value._contexts == {}
    assert value._pending_context_ids == set()


def test_sync_record_har_creation_forwards_to_async_owner(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Arrange
    monkeypatch.setattr(
        "playwright_rust.async_api._native.selected_features", lambda: ["har-recording"]
    )
    native = NativeHar()
    async_value = browser(native)

    class Bridge:
        def run(self, awaitable: Coroutine[Any, Any, object]) -> object:
            return asyncio.run(awaitable)

    sync_value = object.__new__(SyncBrowser)
    sync_value._bridge = cast(Any, Bridge())
    sync_value._value = async_value
    sync_value._contexts = {}
    path = tmp_path / "capture.har"

    # Act
    context = sync_value.new_context(record_har_path=path, record_har_content="omit")

    # Assert
    assert context._value._object_id == "context-1"
    assert native.record_calls == [("context-1", str(path), None, None, True, "omit", "full")]
