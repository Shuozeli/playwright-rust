import assert from 'node:assert/strict';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;
const fixture = process.env.PLAYWRIGHT_RUST_TEST_SERVICE_WORKER_URL;
if (!endpoint || !fixture)
  throw new Error('PLAYWRIGHT_RUST_TEST_CDP and PLAYWRIGHT_RUST_TEST_SERVICE_WORKER_URL are required');

const { chromium } = await import('../src/index.mjs');
async function exercise(browser) {
  // Arrange
  const context = browser.contexts()[0];
  const page = context.pages().find(candidate => candidate.url().includes('playwright-rust-sw-e2e'));
  assert.ok(page, `fixture Page not found: ${fixture}`);
  const run = String(Date.now());
  const workerPromise = context.waitForEvent('serviceworker', {
    predicate: worker => worker.url().includes(`node=${run}`),
    timeout: 10_000,
  });

  // Act
  await page.evaluate(
    ([url, scope]) => navigator.serviceWorker.register(url, { scope }).then(value => value.scope),
    [`./sw.js?node=${run}`, `./node-${run}/`],
  );
  const worker = await workerPromise;
  const snapshotWorker = context.serviceWorkers().find(candidate => candidate.url() === worker.url());
  const marker = await worker.evaluate(() => self.playwrightRustMarker);
  const fetchName = `rust-node-network-${run}.txt`;
  const requestPromise = context.waitForEvent('request', {
    predicate: request => request.url().endsWith(fetchName),
    timeout: 10_000,
  });
  const responsePromise = context.waitForEvent('response', {
    predicate: response => response.url().endsWith(fetchName),
    timeout: 10_000,
  });
  const finishedPromise = context.waitForEvent('requestfinished', {
    predicate: request => request.url().endsWith(fetchName),
    timeout: 10_000,
  });
  const [status, request, response, finished] = await Promise.all([
    worker.evaluate(name => fetch(name).then(value => value.status), fetchName),
    requestPromise,
    responsePromise,
    finishedPromise,
  ]);

  // Assert
  assert.equal(snapshotWorker, worker);
  assert.equal(marker, 'service-worker-ready');
  assert.equal(typeof status, 'number');
  assert.equal(request, finished);
  assert.equal(request.serviceWorker(), worker);
  assert.throws(
    () => request.frame(),
    /Service Worker requests do not have an associated frame\./,
  );
  assert.equal(response.request(), request);
  assert.equal(response.fromServiceWorker(), false);
  assert.equal(await request.response(), response);
  return {
    worker: worker.url(),
    marker,
    request: request.url(),
    status,
    contextOnly: true,
    exactIdentity: true,
  };
}

const browser = await chromium.connectOverCDP(endpoint);
try {
  console.log(JSON.stringify(await exercise(browser)));
} finally {
  await browser.close();
  await new Promise(resolve => setImmediate(resolve));
  global.gc?.();
  await new Promise(resolve => setImmediate(resolve));
  global.gc?.();
}
