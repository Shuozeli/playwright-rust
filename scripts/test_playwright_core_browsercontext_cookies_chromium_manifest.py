"""Source and selection gates for pinned Chromium BrowserContext cookie files."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import cast

import pytest
from run_playwright_official_conformance import (
    Suite,
    _cross_origin_forward_enabled,
    _pinned_full_file_source,
)

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = Path("/Users/cyuan/projects/thirdparty/playwright-v1.58.0")
MANIFEST = (
    ROOT
    / "tests/official/playwright-core-browsercontext-cookies-chromium-v1-manifest.json"
)
COMMIT = "961381ec73a9fa2661d92504eab1a6bd6eaffc75"
EXPECTED = {
    "library-browsercontext-cookies-chromium-v1": (
        "tests/library/browsercontext-cookies.spec.ts",
        16,
        "a2ee0efe1e482c50b1f8e927066b3f4ec6dbd6f33af25eae63fa534e6ce6bdf9",
        "b22d26b82fa61dd5da6243fc90574c9f78af80519acbe5c7a2f2964e6190f768",
    ),
    "library-browsercontext-clearcookies-chromium-v1": (
        "tests/library/browsercontext-clearcookies.spec.ts",
        7,
        "34c061221df5950e8bfd16675b94d49fe1190d7ff79f9e3f0bb3c2da97eb36f1",
        "472dac4cbb5d36c7f19d731fb1f94cfc390d6269c7ef0d21147fe39dd5c6276b",
    ),
    "library-browsercontext-add-cookies-chromium-v1": (
        "tests/library/browsercontext-add-cookies.spec.ts",
        23,
        "83a86c29c2a05be4ec7aac868b7aa89c666f7cde5c6ffab3d818ab460266e761",
        "b9658f4b46202f5e2b945fc8236f1a17d349aa6920a12c1179834015cc3ef16c",
    ),
    "library-browsercontext-cookies-third-party-chromium-v1": (
        "tests/library/browsercontext-cookies-third-party.spec.ts",
        11,
        "5e81ef0e3b4dfae9ae432cb6bc536db72f9d856a91f941effd5bcde4101b3a5e",
        "6ac1a0ebe0a016e8be98794acecb34e14507e6a91040728ccc2253a89b411d3e",
    ),
}


def _sha256(source: bytes) -> str:
    return hashlib.sha256(source).hexdigest()


def _report_specs(report: dict[str, object]) -> list[dict[str, object]]:
    specs: list[dict[str, object]] = []

    def visit(node: dict[str, object]) -> None:
        specs.extend(cast(list[dict[str, object]], node.get("specs", [])))
        for child in cast(list[dict[str, object]], node.get("suites", [])):
            visit(child)

    for suite in cast(list[dict[str, object]], report["suites"]):
        visit(suite)
    return specs


@pytest.fixture(scope="module")
def suites() -> list[Suite]:
    if not UPSTREAM.is_dir():
        pytest.skip("pinned Playwright v1.58.0 checkout is unavailable")
    manifest = json.loads(MANIFEST.read_text())
    return cast(list[Suite], manifest["suites"])


def test_manifest_pins_the_exact_revision_and_four_complete_files(
    suites: list[Suite],
) -> None:
    # Arrange
    revision = subprocess.run(
        ["git", "-C", str(UPSTREAM), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()

    # Act
    selections = {
        suite["id"]: _pinned_full_file_source(suite, UPSTREAM) for suite in suites
    }

    # Assert
    assert revision == COMMIT
    assert set(selections) == set(EXPECTED)
    for suite in suites:
        relative, count, source_digest, ids_digest = EXPECTED[suite["id"]]
        assert _sha256((UPSTREAM / relative).read_bytes()) == source_digest
        assert selections[suite["id"]] == (
            relative.removeprefix("tests/"),
            count,
            "chromium-library",
            ids_digest,
        )
        assert _cross_origin_forward_enabled(suite) is False


@pytest.mark.parametrize("suite_id", tuple(EXPECTED))
def test_official_list_matches_the_exact_pinned_cookie_selection(
    suites: list[Suite], suite_id: str
) -> None:
    # Arrange
    suite = next(value for value in suites if value["id"] == suite_id)
    relative, count, _source_digest, ids_digest = EXPECTED[suite_id]

    # Act
    completed = subprocess.run(
        [
            "node",
            str(UPSTREAM / suite["runner"]),
            "test",
            "--list",
            "--reporter=json",
            f"--config={suite['config']}",
            f"--project={suite['project']}",
            *suite["args"],
        ],
        cwd=UPSTREAM,
        check=True,
        capture_output=True,
        text=True,
    )
    report = cast(dict[str, object], json.loads(completed.stdout))
    specs = _report_specs(report)
    ids = [cast(str, spec["id"]) for spec in specs]

    # Assert
    assert report["errors"] == []
    assert len(ids) == len(set(ids)) == count
    assert _sha256("".join(f"{value}\n" for value in sorted(ids)).encode()) == ids_digest
    assert all(spec["file"] == relative.removeprefix("tests/") for spec in specs)
    assert all(
        len(cast(list[dict[str, object]], spec["tests"])) == 1
        and cast(list[dict[str, object]], spec["tests"])[0]["projectId"]
        == "chromium-library"
        for spec in specs
    )
