import asyncio

import pytest
from playwright_rust.async_api import async_playwright


@pytest.mark.asyncio
async def test_custom_headers_reach_the_websocket_upgrade() -> None:
    # Arrange
    loop = asyncio.get_running_loop()
    received: asyncio.Future[bytes] = loop.create_future()

    async def capture_upgrade(
        reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        try:
            request = await reader.readuntil(b"\r\n\r\n")
            if not received.done():
                received.set_result(request)
            writer.write(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n")
            await writer.drain()
        finally:
            writer.close()
            await writer.wait_closed()

    server = await asyncio.start_server(capture_upgrade, "127.0.0.1", 0)
    address = server.sockets[0].getsockname()
    playwright = await async_playwright().start()

    try:
        # Act
        with pytest.raises(Exception):
            await playwright.chromium.connect_over_cdp(
                f"ws://127.0.0.1:{address[1]}/cdp",
                timeout=1_000,
                headers={
                    "X-BCP-Lease-Id": "lease-python-7",
                    "X-BCP-Fencing-Token": "42",
                },
            )
        request = (await asyncio.wait_for(received, timeout=1)).lower()

        # Assert
        assert b"x-bcp-lease-id: lease-python-7\r\n" in request
        assert b"x-bcp-fencing-token: 42\r\n" in request
    finally:
        await playwright.stop()
        server.close()
        await server.wait_closed()
