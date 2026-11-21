import assert from 'node:assert/strict';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;
const fixture = process.env.PLAYWRIGHT_RUST_TEST_SERVICE_WORKER_URL;
if (!endpoint || !fixture)
  throw new Error('PLAYWRIGHT_RUST_TEST_CDP and PLAYWRIGHT_RUST_TEST_SERVICE_WORKER_URL are required');

const { chromium } = await import('../src/index.mjs');

async function exercise(browser) {
  // Arrange
  const context = browser.contexts()[0];
  const page = context.pages().find(candidate => candidate.url().startsWith(fixture));
  assert.ok(page, `fixture Page not found: ${fixture}`);
  const run = String(Date.now());
  const workerPromise = context.waitForEvent('serviceworker', {
    predicate: worker => worker.url().includes(`offline=${run}`),
    timeout: 10_000,
  });
  await page.evaluate(
    url => new Promise((resolve, reject) => {
      const scriptUrl = new URL(url, location.href).href;
      const timeout = setTimeout(
        () => reject(new Error(`Service Worker did not control the fixture Page: ${scriptUrl}`)),
        10_000,
      );
      const finish = () => {
        if (navigator.serviceWorker.controller?.scriptURL !== scriptUrl)
          return;
        clearTimeout(timeout);
        navigator.serviceWorker.removeEventListener('controllerchange', finish);
        resolve(navigator.serviceWorker.controller.scriptURL);
      };
      navigator.serviceWorker.addEventListener('controllerchange', finish);
      navigator.serviceWorker.register(scriptUrl).then(finish, reject);
    }),
    `./sw.js?offline=${run}`,
  );
  const worker = await workerPromise;
  const resource = `offline-probe-${run}.txt`;

  // Act
  await context.setOffline(true);
  const pageBlocked = await page.evaluate(name => fetch(name).then(
    response => ({ fulfilled: true, status: response.status }),
    error => ({ fulfilled: false, name: error.name }),
  ), `page-${resource}`);
  const blocked = await worker.evaluate(name => fetch(name).then(
    response => ({ fulfilled: true, status: response.status }),
    error => ({ fulfilled: false, name: error.name }),
  ), resource);
  const workerOnline = await worker.evaluate(() => navigator.onLine);
  await context.setOffline(false);
  const restoredStatus = await worker.evaluate(
    name => fetch(name).then(response => response.status),
    resource,
  );

  // Assert
  assert.deepEqual(pageBlocked, { fulfilled: false, name: 'TypeError' });
  assert.equal(workerOnline, false);
  assert.deepEqual(blocked, { fulfilled: false, name: 'TypeError' });
  assert.equal(typeof restoredStatus, 'number');
  assert.equal(context.serviceWorkers().find(candidate => candidate.url() === worker.url()), worker);
  return {
    worker: worker.url(),
    pageBlocked,
    workerOnline,
    blocked,
    restoredStatus,
    contextOwned: true,
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
