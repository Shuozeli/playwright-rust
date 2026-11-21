from __future__ import annotations

import argparse
import asyncio
import json
import subprocess
from pathlib import Path
from typing import TypedDict, cast

from playwright_rust.async_api import async_playwright
from playwright_rust.sync_api import Error, sync_playwright


class VideoStream(TypedDict):
    codec_name: str
    width: int
    height: int


def probe_video(video: Path) -> VideoStream:
    probe = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height",
            "-of",
            "json",
            str(video),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    return cast(VideoStream, json.loads(probe.stdout)["streams"][0])


def assert_video(video: Path, width: int, height: int) -> VideoStream:
    stream = probe_video(video)
    assert video.stat().st_size > 0
    assert stream == {"codec_name": "vp8", "width": width, "height": height}
    return stream


async def async_main(output: Path, width: int, height: int, mode: str) -> None:
    # Arrange
    profile = output / "profile"
    videos = output / "videos"
    profile.mkdir(parents=True)
    videos.mkdir()

    # Act
    async with async_playwright() as playwright:
        browser = None
        if mode == "persistent":
            context = await playwright.chromium.launch_persistent_context(
                profile,
                channel="chromium",
                headless=True,
                record_video_dir=videos,
                record_video_size={"width": width, "height": height},
            )
            page = context.pages[0]
        else:
            browser = await playwright.chromium.launch(channel="chromium", headless=True)
            page = await browser.new_page(
                record_video_dir=videos,
                record_video_size={"width": width, "height": height},
            )
            context = page.context
        video = page.video
        assert video is not None
        source = await video.path()
        assert source.parent == videos.resolve()
        saved = output / "copies" / "async-saved.webm"
        pending_save = asyncio.create_task(video.save_as(saved))
        await asyncio.sleep(0)
        await page.set_content(
            "<style>body{margin:0;background:#123;color:white;font:48px sans-serif}"
            "</style><main>playwright-rust video proof</main>"
        )
        await page.wait_for_timeout(500)
        assert not pending_save.done()
        await context.close()
        await pending_save
        assert source.exists()
        assert saved.read_bytes() == source.read_bytes()
        await video.delete()
        await video.delete()
        assert not source.exists()
        try:
            await video.save_as(output / "copies" / "after-delete.webm")
        except Error as error:
            assert str(error) == "File already deleted. Save before deleting."
        else:
            raise AssertionError("Video.save_as must reject a deleted artifact")
        if browser is not None:
            await browser.close()

    stream = assert_video(saved, width, height)

    # Assert
    print(
        json.dumps(
            {
                "api": "async",
                "mode": mode,
                "bytes": saved.stat().st_size,
                "file": str(saved),
                "stream": stream,
            },
            sort_keys=True,
        ),
        flush=True,
    )


def sync_main(output: Path, width: int, height: int, mode: str) -> None:
    # Arrange
    profile = output / "profile"
    videos = output / "videos"
    profile.mkdir(parents=True)
    videos.mkdir()
    saved = output / "copies" / "sync-saved.webm"

    # Act
    with sync_playwright() as playwright:
        browser = None
        if mode == "persistent":
            context = playwright.chromium.launch_persistent_context(
                profile,
                channel="chromium",
                headless=True,
                record_video_dir=videos,
                record_video_size={"width": width, "height": height},
            )
            page = context.pages[0]
        else:
            browser = playwright.chromium.launch(channel="chromium", headless=True)
            page = browser.new_page(
                record_video_dir=videos,
                record_video_size={"width": width, "height": height},
            )
            context = page.context
        video = page.video
        assert video is not None
        source = video.path()
        assert source.parent == videos.resolve()
        try:
            video.save_as(saved)
        except Error as error:
            assert str(error) == ("Page is not yet closed. Close the page prior to calling save_as")
        else:
            raise AssertionError("sync Video.save_as must reject an open page")
        assert not saved.exists()
        page.set_content(
            "<style>body{margin:0;background:#321;color:white;font:48px sans-serif}"
            "</style><main>playwright-rust sync video proof</main>"
        )
        page.wait_for_timeout(500)
        context.close()
        video.save_as(saved)
        assert saved.read_bytes() == source.read_bytes()
        video.delete()
        video.delete()
        assert not source.exists()
        try:
            video.save_as(output / "copies" / "after-delete.webm")
        except Error as error:
            assert str(error) == "File already deleted. Save before deleting."
        else:
            raise AssertionError("Video.save_as must reject a deleted artifact")
        if browser is not None:
            browser.close()

    stream = assert_video(saved, width, height)

    # Assert
    print(
        json.dumps(
            {
                "api": "sync",
                "mode": mode,
                "bytes": saved.stat().st_size,
                "file": str(saved),
                "stream": stream,
            },
            sort_keys=True,
        ),
        flush=True,
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--api", choices=("async", "sync"), default="async")
    parser.add_argument("--mode", choices=("persistent", "implicit"), default="persistent")
    parser.add_argument("--width", type=int, default=640)
    parser.add_argument("--height", type=int, default=360)
    arguments = parser.parse_args()
    if arguments.api == "async":
        asyncio.run(
            async_main(
                arguments.output,
                arguments.width,
                arguments.height,
                arguments.mode,
            )
        )
    else:
        sync_main(arguments.output, arguments.width, arguments.height, arguments.mode)
