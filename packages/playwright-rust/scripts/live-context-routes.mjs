import assert from 'node:assert/strict';

import { chromium } from '../src/index.mjs';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;
if (!endpoint) throw new Error('PLAYWRIGHT_RUST_TEST_CDP is required');

const profile = process.env.PLAYWRIGHT_RUST_NATIVE_PROFILE;
assert.equal(
  profile,
  'official-conformance-page-route-header-url-unroute-cors-regexp',
  'live Context routing requires the full routing profile',
);

const browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
let context;
let primary;
try {
  context = await browser.newContext();
  const page = await context.newPage();

  let contextHits = 0;
  await context.route(/context-route\.js$/iu, async (route, request) => {
    contextHits += 1;
    assert.equal(request.frame().page(), page);
    await route.fulfill({
      status: 200,
      contentType: 'application/javascript',
      body: 'globalThis.__contextRoute = "context";',
    });
  }, { times: 1 });
  await page.setContent('<script src="https://pwright.test/context-route.js"></script>');
  assert.equal(await page.evaluate('globalThis.__contextRoute'), 'context');
  assert.equal(contextHits, 1);

  let contextPriorityHits = 0;
  let pagePriorityHits = 0;
  await context.route('**/priority.js', async route => {
    contextPriorityHits += 1;
    await route.fulfill({
      contentType: 'application/javascript',
      body: 'globalThis.__routePriority = "context";',
    });
  });
  await page.route('**/priority.js', async route => {
    pagePriorityHits += 1;
    await route.fulfill({
      contentType: 'application/javascript',
      body: 'globalThis.__routePriority = "page";',
    });
  });
  await page.setContent('<script src="https://pwright.test/priority.js"></script>');
  assert.equal(await page.evaluate('globalThis.__routePriority'), 'page');
  assert.equal(pagePriorityHits, 1);
  assert.equal(contextPriorityHits, 0);
  await page.unrouteAll();

  let removedHits = 0;
  let retainedHits = 0;
  const removed = async route => {
    removedHits += 1;
    await route.abort();
  };
  await context.route('**/unroute.js', removed);
  await context.route('**/unroute.js', async route => {
    retainedHits += 1;
    await route.fulfill({
      contentType: 'application/javascript',
      body: 'globalThis.__contextUnroute = "retained";',
    });
  });
  await context.unroute('**/unroute.js', removed);
  await page.setContent('<script src="https://pwright.test/unroute.js"></script>');
  assert.equal(await page.evaluate('globalThis.__contextUnroute'), 'retained');
  assert.equal(removedHits, 0);
  assert.equal(retainedHits, 1);

  let signalStarted;
  const started = new Promise(resolve => { signalStarted = resolve; });
  let releaseHandler;
  const handlerGate = new Promise(resolve => { releaseHandler = resolve; });
  await context.route('**/wait.js', async route => {
    signalStarted();
    await handlerGate;
    await route.fulfill({
      contentType: 'application/javascript',
      body: 'globalThis.__contextUnrouteAll = "released";',
    });
  });
  const loading = page.setContent('<script src="https://pwright.test/wait.js"></script>');
  await started;
  let unrouteAllSettled = false;
  const removing = context.unrouteAll({ behavior: 'wait' }).then(() => {
    unrouteAllSettled = true;
  });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(unrouteAllSettled, false);
  releaseHandler();
  await Promise.all([loading, removing]);
  assert.equal(await page.evaluate('globalThis.__contextUnrouteAll'), 'released');
  assert.equal(unrouteAllSettled, true);

  console.log(JSON.stringify({
    contextRegExp: 'fulfilled',
    precedence: 'page-over-context',
    exactUnroute: 'retained-handler-ran',
    unrouteAllWait: 'waited-for-active-handler',
  }));
} catch (error) {
  primary = error;
} finally {
  const cleanupErrors = [];
  try { await context?.close(); } catch (error) { cleanupErrors.push(error); }
  try { await browser.close(); } catch (error) { cleanupErrors.push(error); }
  await new Promise(resolve => setImmediate(resolve));
  global.gc?.();
  await new Promise(resolve => setImmediate(resolve));
  global.gc?.();
  if (primary && cleanupErrors.length)
    throw new AggregateError([primary, ...cleanupErrors], 'Context route exercise and cleanup failed', { cause: primary });
  if (primary) throw primary;
  if (cleanupErrors.length === 1) throw cleanupErrors[0];
  if (cleanupErrors.length) throw new AggregateError(cleanupErrors, 'Context route cleanup failed');
}
