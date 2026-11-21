#!/usr/bin/env python3
"""Compare pinned Playwright public member names with the local bindings.

This is a static inventory. A matching name is not behavioral conformance.
Firefox, WebKit, BiDi, and non-library products are intentionally excluded.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
import subprocess
from pathlib import Path
from typing import cast

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_CORE_COMMIT = "961381ec73a9fa2661d92504eab1a6bd6eaffc75"
EXPECTED_PYTHON_COMMIT = "47a5d35ef4f815a2021349f86ae391f7c20c02d6"

NODE_CLASSES = (
    "BrowserType",
    "Browser",
    "BrowserContext",
    "Page",
    "Frame",
    "Locator",
    "FrameLocator",
    "ElementHandle",
    "JSHandle",
    "CDPSession",
    "Request",
    "Response",
    "Route",
    "Worker",
    "ConsoleMessage",
    "Dialog",
    "Download",
    "FileChooser",
    "WebSocket",
    "WebSocketRoute",
    "Clock",
    "Coverage",
    "Tracing",
    "Video",
    "APIRequest",
    "APIRequestContext",
    "APIResponse",
    "Selectors",
    "Keyboard",
    "Mouse",
    "Touchscreen",
    "WebError",
    "BrowserServer",
)

PYTHON_CLASSES = (
    "Playwright",
    *NODE_CLASSES,
)

OPTION_MAP = {
    "BrowserContextOptions": "BrowserNewContextOptions",
    "ConnectOverCDPOptions": "ConnectOverCDPOptions",
    "PageScreenshotOptions": "ScreenshotOptions",
    "LocatorScreenshotOptions": "LocatorScreenshotOptions",
}

EVENT_EMITTER_METHODS = {
    "addListener",
    "off",
    "on",
    "once",
    "prependListener",
    "removeAllListeners",
    "removeListener",
}

MEMBER_RE = re.compile(
    r"^(?:readonly\s+)?([A-Za-z_$][A-Za-z0-9_$]*)\??\s*"
    r"(?:<[^;{}]*?>\s*)?(?:\(|:)"
)

TypeScriptBlocks = dict[str, tuple[str, list[str]]]
PythonClasses = dict[str, tuple[set[str], list[str]]]


def _git(path: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(path), *args],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def verify_upstream(path: Path, expected_commit: str, name: str) -> None:
    if _git(path, "rev-parse", "HEAD") != expected_commit:
        raise SystemExit(f"{name} checkout does not match the pinned commit")
    if _git(path, "status", "--porcelain", "--untracked-files=no"):
        raise SystemExit(f"{name} checkout has tracked source changes")


def interface_blocks(source: str) -> TypeScriptBlocks:
    """Extract TypeScript interface bodies without confusing generic `{}`."""
    result: TypeScriptBlocks = {}
    for match in re.finditer(r"(?:export\s+)?interface\s+(\w+)", source):
        name = match.group(1)
        angle_depth = 0
        open_brace = match.end()
        while open_brace < len(source):
            char = source[open_brace]
            if char == "<":
                angle_depth += 1
            elif char == ">" and angle_depth:
                angle_depth -= 1
            elif char == "{" and angle_depth == 0:
                break
            open_brace += 1
        if open_brace == len(source):
            raise ValueError(f"interface {name} has no body")

        depth = 1
        end = open_brace + 1
        while end < len(source) and depth:
            if source[end] == "{":
                depth += 1
            elif source[end] == "}":
                depth -= 1
            end += 1
        if depth:
            raise ValueError(f"unterminated interface {name}")

        header = source[match.end() : open_brace]
        parents: list[str] = []
        if extends_match := re.search(r"\bextends\s+(.+)", header):
            parents = [
                re.sub(r"<.*", "", item.strip())
                for item in extends_match.group(1).split(",")
            ]
        result[name] = (source[open_brace + 1 : end - 1], parents)
    return result


def direct_ts_members(body: str) -> set[str]:
    members: set[str] = set()
    brace_depth = 0
    paren_depth = 0
    bracket_depth = 0
    in_block_comment = False
    for raw_line in body.splitlines():
        line = raw_line.strip()
        if in_block_comment:
            if "*/" not in line:
                continue
            in_block_comment = False
            line = line.split("*/", 1)[1].strip()
        while line.startswith("/*"):
            if "*/" not in line:
                in_block_comment = True
                line = ""
                break
            line = line.split("*/", 1)[1].strip()
        if not line or line.startswith(("//", "*")):
            continue
        if brace_depth == paren_depth == bracket_depth == 0:
            if match := MEMBER_RE.match(line):
                members.add(match.group(1))
        brace_depth += line.count("{") - line.count("}")
        paren_depth += line.count("(") - line.count(")")
        bracket_depth += line.count("[") - line.count("]")
    return members


def all_ts_members(
    name: str,
    blocks: TypeScriptBlocks,
    cache: dict[str, set[str]],
) -> set[str]:
    if name in cache:
        return set(cache[name])
    if name not in blocks:
        return set()
    body, parents = blocks[name]
    result = direct_ts_members(body)
    for parent in parents:
        result.update(all_ts_members(parent, blocks, cache))
    cache[name] = set(result)
    return result


def ts_events(body: str) -> set[str]:
    events: set[str] = set()
    for match in re.finditer(r"\bon\s*\(\s*event\s*:\s*([^,\n]+)", body):
        events.update(re.findall(r"['\"]([^'\"]+)['\"]", match.group(1)))
    return events


def _public_self_attributes(
    function: ast.FunctionDef | ast.AsyncFunctionDef,
) -> set[str]:
    attributes: set[str] = set()
    for child in ast.walk(function):
        targets: list[ast.expr] = []
        if isinstance(child, ast.Assign):
            targets.extend(child.targets)
        elif isinstance(child, ast.AnnAssign):
            targets.append(child.target)
        for target in targets:
            if (
                isinstance(target, ast.Attribute)
                and isinstance(target.value, ast.Name)
                and target.value.id == "self"
                and not target.attr.startswith("_")
            ):
                attributes.add(target.attr)
    return attributes


def python_classes(path: Path) -> PythonClasses:
    tree = ast.parse(path.read_bytes(), filename=str(path))
    result: PythonClasses = {}
    for node in tree.body:
        if not isinstance(node, ast.ClassDef):
            continue
        members: set[str] = set()
        for item in node.body:
            if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                if not item.name.startswith("_"):
                    members.add(item.name)
                if item.name == "__init__":
                    members.update(_public_self_attributes(item))
            elif isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name):
                if not item.target.id.startswith("_"):
                    members.add(item.target.id)
        parents = [base.id for base in node.bases if isinstance(base, ast.Name)]
        result[node.name] = (members, parents)
    return result


def merge_python_classes(*sources: PythonClasses) -> PythonClasses:
    result: PythonClasses = {}
    for source in sources:
        for name, (members, parents) in source.items():
            if name not in result:
                result[name] = (set(members), list(parents))
                continue
            result[name][0].update(members)
            for parent in parents:
                if parent not in result[name][1]:
                    result[name][1].append(parent)
    return result


def all_python_members(
    name: str,
    classes: PythonClasses,
    cache: dict[str, set[str]],
) -> set[str]:
    if name in cache:
        return set(cache[name])
    if name not in classes:
        return set()
    members, parents = classes[name]
    result = set(members)
    for parent in parents:
        result.update(all_python_members(parent, classes, cache))
    cache[name] = set(result)
    return result


def comparison_row(name: str, upstream: set[str], local: set[str]) -> dict[str, object]:
    overlap = upstream & local
    return {
        "name": name,
        "upstream": len(upstream),
        "local": len(local),
        "overlap": len(overlap),
        "coveragePercent": round(100 * len(overlap) / len(upstream), 1)
        if upstream
        else 100.0,
        "missing": sorted(upstream - local),
        "extensions": sorted(local - upstream),
    }


def summarize(rows: list[dict[str, object]]) -> dict[str, object]:
    upstream = sum(cast(int, item["upstream"]) for item in rows)
    overlap = sum(cast(int, item["overlap"]) for item in rows)
    return {
        "upstreamMembers": upstream,
        "overlapMembers": overlap,
        "coveragePercent": round(100 * overlap / upstream, 1),
        "objects": rows,
    }


def _python_comparison(
    upstream_path: Path,
    local_paths: tuple[Path, ...],
) -> dict[str, object]:
    upstream = python_classes(upstream_path)
    local = merge_python_classes(*(python_classes(path) for path in local_paths))
    upstream["Playwright"][0].difference_update({"firefox", "webkit"})
    upstream_cache: dict[str, set[str]] = {}
    local_cache: dict[str, set[str]] = {}
    return summarize(
        [
            comparison_row(
                name,
                all_python_members(name, upstream, upstream_cache),
                all_python_members(name, local, local_cache),
            )
            for name in PYTHON_CLASSES
        ]
    )


def compare(upstream_core: Path, upstream_python: Path) -> dict[str, object]:
    upstream_blocks = interface_blocks(
        upstream_core.joinpath("packages/playwright-core/types/types.d.ts").read_text()
    )
    local_blocks = interface_blocks(
        ROOT.joinpath("packages/playwright-rust/src/index.d.ts").read_text()
    )
    upstream_cache: dict[str, set[str]] = {}
    local_cache: dict[str, set[str]] = {}
    node_rows = []
    for name in NODE_CLASSES:
        upstream = all_ts_members(name, upstream_blocks, upstream_cache)
        local = all_ts_members(name, local_blocks, local_cache)
        upstream.difference_update(EVENT_EMITTER_METHODS)
        local.difference_update(EVENT_EMITTER_METHODS)
        node_rows.append(comparison_row(name, upstream, local))

    event_rows = [
        comparison_row(
            name,
            ts_events(upstream_blocks.get(name, ("", []))[0]),
            ts_events(local_blocks.get(name, ("", []))[0]),
        )
        for name in ("Browser", "BrowserContext", "Page", "Worker", "WebSocket")
    ]
    option_rows = [
        comparison_row(
            upstream_name,
            all_ts_members(upstream_name, upstream_blocks, upstream_cache),
            all_ts_members(local_name, local_blocks, local_cache),
        )
        for upstream_name, local_name in OPTION_MAP.items()
    ]
    local_python = ROOT / "packages/playwright-rust-python/python/playwright_rust"
    return {
        "scope": {
            "upstream": "Playwright v1.58.0",
            "browser": "Chromium/CDP only",
            "excluded": [
                "Firefox",
                "WebKit",
                "WebDriver BiDi",
                "Android",
                "Electron",
                "Playwright Test",
                "component test runner",
                "MCP",
            ],
            "warning": "Static name overlap is not behavioral conformance.",
        },
        "node": summarize(node_rows),
        "nodeEvents": summarize(event_rows),
        "nodeOptions": summarize(option_rows),
        "pythonAsync": _python_comparison(
            upstream_python / "playwright/async_api/_generated.py",
            (local_python / "async_api.py", local_python / "_async_api_request.py"),
        ),
        "pythonSync": _python_comparison(
            upstream_python / "playwright/sync_api/_generated.py",
            (local_python / "sync_api.py", local_python / "_sync_api_request.py"),
        ),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("upstream_core", type=Path)
    parser.add_argument("upstream_python", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    upstream_core = args.upstream_core.resolve()
    upstream_python = args.upstream_python.resolve()
    verify_upstream(upstream_core, EXPECTED_CORE_COMMIT, "Playwright Core")
    verify_upstream(upstream_python, EXPECTED_PYTHON_COMMIT, "Playwright Python")
    payload = json.dumps(compare(upstream_core, upstream_python), indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload, encoding="utf-8")
        print(args.output)
    else:
        print(payload, end="")


if __name__ == "__main__":
    main()
