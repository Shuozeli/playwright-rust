from __future__ import annotations

import argparse
import asyncio
import json
import zipfile
from pathlib import Path
from typing import Any, Literal, cast

from playwright_rust.async_api import Page, async_playwright


async def worker_fetch(page: Page, script_url: str, data_url: str) -> dict[str, object]:
    value = await page.evaluate(
        """
        async ({scriptURL, dataURL}) => {
          const registration = await navigator.serviceWorker.register(scriptURL);
          await navigator.serviceWorker.ready;
          const worker = registration.active || registration.waiting || registration.installing;
          if (!worker)
            throw new Error('Service Worker did not become active');
          return await new Promise((resolve, reject) => {
            const channel = new MessageChannel();
            const timer = setTimeout(() => reject(new Error('worker fetch timed out')), 10000);
            channel.port1.onmessage = event => {
              clearTimeout(timer);
              resolve(event.data);
            };
            worker.postMessage({url: dataURL}, [channel.port2]);
          });
        }
        """,
        {"scriptURL": script_url, "dataURL": data_url},
    )
    if not isinstance(value, dict):
        raise AssertionError(f"unexpected worker result: {value!r}")
    return cast(dict[str, object], value)


async def page_fetch(page: Page, data_url: str) -> dict[str, object]:
    value = await page.evaluate(
        """async url => {
          const response = await fetch(url, {cache: 'no-store'});
          return {status: response.status, body: await response.text()};
        }""",
        data_url,
    )
    if not isinstance(value, dict):
        raise AssertionError(f"unexpected page result: {value!r}")
    return cast(dict[str, object], value)


async def main(
    endpoint: str,
    origin: str,
    har_path: Path,
    mode: Literal["full", "minimal"],
    creation: bool,
) -> None:
    async with async_playwright() as playwright:
        browser = await playwright.chromium.connect_over_cdp(endpoint)

        if creation:
            capture = await browser.new_context(
                record_har_path=har_path,
                record_har_mode=mode,
            )
        else:
            capture = await browser.new_context()
            await capture.route_from_har(har_path, update=True, update_mode=mode)
        capture_page = await capture.new_page()
        await capture_page.goto(f"{origin}/index.html")
        captured_page = await page_fetch(capture_page, f"{origin}/data?source=page")
        captured_worker = await worker_fetch(
            capture_page,
            f"{origin}/sw.js?run=capture",
            f"{origin}/data?source=worker",
        )
        await capture_page.evaluate("url => fetch(url)", f"{origin}/shutdown")
        await capture.close()

        archive_names: set[str]
        if har_path.suffix == ".zip":
            with zipfile.ZipFile(har_path) as archive:
                document = json.loads(archive.read("har.har"))
                archive_names = set[str](archive.namelist())
        else:
            document = json.loads(har_path.read_text(encoding="utf-8"))
            archive_names = set()
        entries = cast(list[dict[str, Any]], document["log"]["entries"])
        if mode == "full":
            pages = cast(list[dict[str, Any]], document["log"]["pages"])
            assert len(pages) == 1
            assert pages[0]["title"] == "HAR recording E2E"
            page_timings = cast(dict[str, float], pages[0]["pageTimings"])
            assert page_timings["onContentLoad"] >= 0
            assert page_timings["onLoad"] >= page_timings["onContentLoad"]
        urls = [cast(str, entry["request"]["url"]) for entry in entries]
        attached_files = [
            cast(dict[str, Any], entry["response"]["content"]).get("_file") for entry in entries
        ]
        embedded_text = [
            cast(dict[str, Any], entry["response"]["content"]).get("text") for entry in entries
        ]
        assert any("source=page" in url for url in urls)
        assert any("source=worker" in url for url in urls)
        data_indexes = [index for index, url in enumerate(urls) if "source=" in url]
        if creation and har_path.suffix != ".zip":
            assert [embedded_text[index] for index in data_indexes] == [
                "page-response",
                "worker-response",
            ]
            assert all(attached_files[index] is None for index in data_indexes)
        else:
            data_files = [cast(str, attached_files[index]) for index in data_indexes]
            assert len(data_files) == 2
            assert all(len(name.split(".", 1)[0]) == 40 for name in data_files)
            if har_path.suffix == ".zip":
                assert {"har.har", *data_files}.issubset(archive_names)
            else:
                assert all((har_path.parent / name).is_file() for name in data_files)

        await asyncio.sleep(0.25)

        replay = await browser.new_context()
        await replay.route_from_har(har_path)
        replay_page = await replay.new_page()
        await replay_page.goto(f"{origin}/index.html")
        replayed_page = await page_fetch(replay_page, f"{origin}/data?source=page")
        replayed_worker = await worker_fetch(
            replay_page,
            f"{origin}/sw.js?run=capture",
            f"{origin}/data?source=worker",
        )
        await replay.close()
        await browser.close()

    observed = {
        "creation": creation,
        "mode": mode,
        "entries": len(entries),
        "captured_page": captured_page,
        "captured_worker": captured_worker,
        "replayed_page": replayed_page,
        "replayed_worker": replayed_worker,
    }
    print(json.dumps(observed, sort_keys=True), flush=True)
    assert captured_page == replayed_page == {"status": 200, "body": "page-response"}
    assert captured_worker == replayed_worker == {"status": 200, "body": "worker-response"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--endpoint", required=True)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--har", required=True, type=Path)
    parser.add_argument("--mode", choices=("full", "minimal"), default="minimal")
    parser.add_argument("--creation", action="store_true")
    arguments = parser.parse_args()
    asyncio.run(
        main(
            arguments.endpoint,
            arguments.origin,
            arguments.har,
            arguments.mode,
            arguments.creation,
        )
    )
