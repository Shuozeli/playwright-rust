from __future__ import annotations

import argparse
import asyncio
import json
import time
from typing import cast

from playwright_rust.async_api import Page, async_playwright


async def worker_fetch(page: Page, script_url: str, path: str) -> dict[str, object]:
    result = await page.evaluate(
        """
        async ({scriptURL, path}) => {
          const registration = await navigator.serviceWorker.register(scriptURL);
          await navigator.serviceWorker.ready;
          const worker = registration.active || registration.waiting || registration.installing;
          if (!worker)
            throw new Error('Service Worker did not become active');
          return await new Promise((resolve, reject) => {
            const channel = new MessageChannel();
            const timer = setTimeout(
              () => reject(new Error('Service Worker fetch timed out')), 10000);
            channel.port1.onmessage = event => {
              clearTimeout(timer);
              if (event.data.error)
                reject(new Error(event.data.error));
              else
                resolve(event.data);
            };
            worker.postMessage({ path }, [channel.port2]);
          });
        }
        """,
        {"scriptURL": script_url, "path": path},
    )
    if not isinstance(result, dict):
        raise AssertionError(f"unexpected Service Worker result: {result!r}")
    return cast(dict[str, object], result)


def counter(stats: dict[str, object], name: str) -> float:
    value = stats.get(name)
    if not isinstance(value, (int, float)):
        raise AssertionError(f"unexpected {name}: {value!r}")
    return float(value)


async def main(endpoint: str, origin: str) -> None:
    async with async_playwright() as playwright:
        browser = await playwright.chromium.connect_over_cdp(endpoint)
        context = await browser.new_context(
            http_credentials={
                "username": "user",
                "password": "pass",
                "origin": origin,
            }
        )
        try:
            page = await context.new_page()
            await page.goto(f"{origin}/index.html")

            script_url = f"/sw.js?run={time.time_ns()}"
            authenticated = await worker_fetch(page, script_url, "/protected")
            rejected = await worker_fetch(page, script_url, "/reject")
            service_workers = [worker.url for worker in context.service_workers]
            page_authenticated = await page.evaluate(
                """async () => {
                  const response = await fetch('/page-protected', {cache: 'no-store'});
                  return {status: response.status, body: await response.text()};
                }"""
            )
            stats_value = await page.evaluate(
                "async () => await (await fetch('/stats', {cache: 'no-store'})).json()"
            )
            if not isinstance(stats_value, dict):
                raise AssertionError(f"unexpected stats: {stats_value!r}")
            stats = cast(dict[str, object], stats_value)
            observed = {
                "page_authenticated": page_authenticated,
                "authenticated": authenticated,
                "rejected": rejected,
                "service_workers": service_workers,
                "stats": stats,
            }
            print(json.dumps(observed), flush=True)

            assert page_authenticated == {"status": 200, "body": "authenticated-page"}
            assert authenticated == {
                "status": 200,
                "body": "authenticated-service-worker",
            }
            assert rejected == {"status": 401, "body": "always rejected"}
            assert counter(stats, "protectedChallenges") >= 1
            assert counter(stats, "protectedSuccesses") >= 1
            assert counter(stats, "rejectedChallenges") >= 2
        finally:
            await context.close()
            await browser.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--endpoint", required=True)
    parser.add_argument("--origin", required=True)
    arguments = parser.parse_args()
    asyncio.run(main(arguments.endpoint, arguments.origin))
