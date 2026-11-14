# pyright: reportPrivateUsage=false
"""Candidate sync namespace used by the unmodified official test corpus."""

import os
import sys
from typing import cast

from playwright_rust import Error as Error
from playwright_rust import TimeoutError as TimeoutError
from playwright_rust import UnsupportedError as UnsupportedError
from playwright_rust.sync_api import (
    APIRequest as APIRequest,
)
from playwright_rust.sync_api import (
    APIRequestContext as APIRequestContext,
)
from playwright_rust.sync_api import (
    APIResponse as APIResponse,
)
from playwright_rust.sync_api import (
    Browser as Browser,
)
from playwright_rust.sync_api import (
    BrowserContext as BrowserContext,
)
from playwright_rust.sync_api import (
    BrowserType as BrowserType,
)
from playwright_rust.sync_api import (
    ClientCertificate as ClientCertificate,
)
from playwright_rust.sync_api import (
    ConsoleMessage as ConsoleMessage,
)
from playwright_rust.sync_api import (
    Download as Download,
)
from playwright_rust.sync_api import (
    ElementHandle as ElementHandle,
)
from playwright_rust.sync_api import (
    FileChooser as FileChooser,
)
from playwright_rust.sync_api import (
    Frame as Frame,
)
from playwright_rust.sync_api import (
    JSHandle as JSHandle,
)
from playwright_rust.sync_api import (
    Locator as Locator,
)
from playwright_rust.sync_api import (
    Page as Page,
)
from playwright_rust.sync_api import (
    Playwright as Playwright,
)
from playwright_rust.sync_api import (
    Request as Request,
)
from playwright_rust.sync_api import (
    Response as Response,
)
from playwright_rust.sync_api import (
    Selectors as Selectors,
)
from playwright_rust.sync_api import (
    SourceLocation as SourceLocation,
)
from playwright_rust.sync_api import (
    StorageState as StorageState,
)
from playwright_rust.sync_api import (
    StorageStateCookie as StorageStateCookie,
)
from playwright_rust.sync_api import Worker as Worker
from playwright_rust.sync_api import expect as expect
from playwright_rust.sync_api import (
    sync_playwright as sync_playwright,
)

from ._public_types import install_public_type_placeholders

install_public_type_placeholders(globals())


class _LaunchedBrowser(Browser):
    _official_fixture_context_ids: set[int]

    @property
    def _impl_obj(self) -> "_LaunchedBrowser":
        return self

    @property
    def contexts(self) -> list[BrowserContext]:
        return [
            context
            for context in super().contexts
            if id(context._value) not in self._official_fixture_context_ids
        ]


def _candidate_launch(self: BrowserType, **options: object) -> Browser:
    if os.environ.get("PLAYWRIGHT_RUST_OFFICIAL_LAUNCH_MODE", "cdp-adapter") != "cdp-adapter":
        raise UnsupportedError("official-suite CDP launch adapter is disabled in owned mode")
    unsupported = sorted(set(options) - {"headless", "handle_sigint", "channel"})
    if unsupported:
        raise UnsupportedError(
            "official-suite launch adapter cannot apply options: " + ", ".join(unsupported)
        )
    if "handle_sigint" in options and options["handle_sigint"] is not False:
        raise UnsupportedError("official-suite CDP attach adapter requires handle_sigint=False")
    expected_channel = os.environ.get("PLAYWRIGHT_RUST_OFFICIAL_BROWSER_CHANNEL")
    if "channel" in options and options["channel"] != expected_channel:
        raise UnsupportedError(
            "official-suite CDP attach adapter requires the configured browser channel"
        )
    if self.name != "chromium":
        raise UnsupportedError(
            f"official-suite launch adapter cannot attach the {self.name} engine"
        )
    endpoint = os.environ["PLAYWRIGHT_RUST_CDP_ENDPOINT"]
    print(
        "[playwright-rust-candidate] BrowserType.launch fixture adapter -> "
        f"connect_over_cdp({endpoint})",
        file=sys.stderr,
    )
    browser = self.connect_over_cdp(endpoint)
    hidden = {id(context._value) for context in browser.contexts}
    browser.__class__ = _LaunchedBrowser
    launched = cast(_LaunchedBrowser, browser)
    launched._official_fixture_context_ids = hidden
    return launched


_launch_mode = os.environ.get("PLAYWRIGHT_RUST_OFFICIAL_LAUNCH_MODE", "cdp-adapter")
if _launch_mode == "cdp-adapter":
    setattr(BrowserType, "launch", _candidate_launch)
elif _launch_mode == "owned":
    if not callable(getattr(BrowserType, "launch", None)):
        raise RuntimeError("owned launch requires playwright_rust.sync_api.BrowserType.launch")
else:
    raise RuntimeError(f"unsupported official launch mode: {_launch_mode!r}")
