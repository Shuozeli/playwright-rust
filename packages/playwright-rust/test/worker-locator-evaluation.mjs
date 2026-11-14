import assert from 'node:assert/strict';
import { parentPort, workerData } from 'node:worker_threads';
import { chromium } from '@shuozeli/playwright-rust';

const { mode } = workerData;
assert.ok(['raw', 'binary', 'running', 'queued', 'borrowed'].includes(mode));
const browser = await chromium.connectOverCDP(workerData.endpoint, { timeout: 5000 });
let page;
if (mode === 'borrowed') {
  page = browser.contexts().flatMap(context => context.pages()).find(page => page.url() === workerData.url);
  assert.ok(page, 'the exact externally owned fixture Page must already exist');
} else {
  const context = await browser.newContext();
  page = await context.newPage();
  await page.goto(workerData.url, { timeout: 5000 });
}
const clock = new Int32Array(new SharedArrayBuffer(4));
function blockAndReport() {
  if (mode === 'queued') {
    const deadline = Date.now() + 10000;
    while (browser.playwrightRustFfiMetrics().queuedCommands !== 1) {
      assert.ok(Date.now() < deadline, 'native Locator result did not reach settlement queue');
      Atomics.wait(clock, 0, 0, 5);
    }
  }
  parentPort.postMessage({ ffi: browser.playwrightRustFfiMetrics(), host: browser.playwrightRustMetrics(), conversion: browser.playwrightRustLocatorConversionMetrics() });
  Atomics.wait(clock, 0, 0, 30000);
}
const argument = { get bytes() {
  if (mode === 'binary' || mode === 'queued') queueMicrotask(blockAndReport);
  return mode === 'binary' ? new Uint8Array(256 * 1024) : 1;
} };
const outcome = page.locator(mode === 'raw' ? '#absent' : '#content').evaluate(
  mode === 'borrowed' ? async () => {
    globalThis.__playwrightRustBorrowedCalls = 1;
    await new Promise(resolve => { globalThis.__playwrightRustBorrowedResume = resolve; });
    globalThis.__playwrightRustBorrowedCalls++;
    return 42;
  } : mode === 'running' ? () => { document.title = 'playwright-rust-locator-evaluation-running'; return new Promise(() => {}); } : element => element.id,
  argument,
).catch(error => ({ error: error.code }));
if (mode === 'raw') blockAndReport();
if (mode === 'running' || mode === 'borrowed') parentPort.postMessage({ started: true });
await outcome;
await browser.close();
