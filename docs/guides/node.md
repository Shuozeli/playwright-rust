<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Node.js guide

`@shuozeli/playwright-rust` is a Playwright-shaped Node.js facade backed by the
Rust host. It requires Node.js 20 or newer and is currently workspace-private.

## Status

- Chromium is the only exported browser type.
- `connectOverCDP` uses the same attach path as the Rust facade.
- Native capability profiles are opt-in; unsupported options fail explicitly.
- The package is not yet published to npm.

## Remote-CDP example

```js
import { chromium } from '@shuozeli/playwright-rust';

const browser = await chromium.connectOverCDP('ws://agent.example/cdp', {
  headers: { 'X-Example-Lease-Id': 'lease-123' },
});

try {
  const context = browser.contexts()[0];
  const page = await context.newPage();
  await page.goto('https://example.com');
  console.log(await page.content());
  await page.close();
} finally {
  await browser.close();
}
```

Nonempty headers require the `cdp-handshake-headers` native profile.

## Ownership

`browser.close()` disconnects an attached browser. It must not terminate the
external Chrome process or close pre-existing tabs. Pages and Contexts created
by the attachment remain the attachment's responsibility.

## Workspace development

```sh
cd packages/playwright-rust
pnpm install --frozen-lockfile
pnpm run build
pnpm test
```

Package scripts select explicit native profiles. A JavaScript import alone
cannot add a missing Rust capability.

## Compatibility boundary

The facade covers a substantial but incomplete Chromium API. Package contract
tests are not the same as unchanged upstream Playwright tests. Consult the
[support matrix](../reference/support-matrix.md) and source-bound
[evidence](../evidence/README.md) before relying on an advanced feature.
