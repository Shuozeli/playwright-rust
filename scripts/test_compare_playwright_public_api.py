"""Tests for the static Playwright public API comparison."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts import compare_playwright_public_api as comparison


class TypeScriptInventoryTests(unittest.TestCase):
    def test_generic_default_and_multiline_parameters_are_not_members(self) -> None:
        # Arrange
        source = """
        export interface Parent { inherited(): void; }
        export interface Example<Unused = {}> extends Parent {
          send<T>(
            method: T,
            params?: object
          ): Promise<void>;
          readonly value: string;
        }
        """

        # Act
        blocks = comparison.interface_blocks(source)
        members = comparison.all_ts_members("Example", blocks, {})

        # Assert
        self.assertEqual(members, {"inherited", "send", "value"})

    def test_event_names_are_distinct_from_generic_event_emitter_methods(self) -> None:
        # Arrange
        body = """
        on(event: 'close', listener: () => void): this;
        on(event: 'request' | 'response', listener: () => void): this;
        """

        # Act
        events = comparison.ts_events(body)

        # Assert
        self.assertEqual(events, {"close", "request", "response"})


class PythonInventoryTests(unittest.TestCase):
    def test_public_constructor_attributes_are_members(self) -> None:
        # Arrange
        source = """
class Playwright:
    def __init__(self):
        self.chromium = object()
        self._internal = object()

    async def stop(self):
        pass
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "api.py"
            path.write_text(source, encoding="utf-8")

            # Act
            classes = comparison.python_classes(path)

        # Assert
        self.assertEqual(classes["Playwright"][0], {"chromium", "stop"})


if __name__ == "__main__":
    unittest.main()
