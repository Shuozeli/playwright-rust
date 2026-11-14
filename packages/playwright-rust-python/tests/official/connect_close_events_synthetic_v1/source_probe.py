"""Run the isolated source-only Python candidate against an installed ABI 125 wheel.

This is a developer probe, not an attestable candidate wheel or ledger input.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import ModuleType

import playwright_rust
import pytest

PACKAGE_SOURCE = Path(__file__).resolve().parents[3] / "python/playwright_rust"


def _replace_module(name: str) -> ModuleType:
    path = PACKAGE_SOURCE / f"{name}.py"
    spec = importlib.util.spec_from_file_location(f"playwright_rust.{name}", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load Playwright Rust source module: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    setattr(playwright_rust, name, module)
    spec.loader.exec_module(module)
    return module


if __name__ == "__main__":
    if playwright_rust.native_capabilities()["abi"] != 125:
        raise RuntimeError("source probe requires an installed ABI 125 native wheel")
    _replace_module("async_api")
    _replace_module("sync_api")
    raise SystemExit(pytest.main(sys.argv[1:]))
