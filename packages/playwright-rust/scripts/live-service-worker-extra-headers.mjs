import assert from 'node:assert/strict';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;
const fixture = process.env.PLAYWRIGHT_RUST_TEST_SERVICE_WORKER_URL;
if (!endpoint || !fixture)
  throw new Error('PLAYWRIGHT_RUST_TEST_CDP and PLAYWRIGHT_RUST_TEST_SERVICE_WORKER_URL are required');

const { chromium } = await import('../src/index.mjs');

async function controllingWorker(context, page, run) {
  const workerPromise = context.waitForEvent('serviceworker', {
    predicate: worker => worker.url().includes(`headers=${run}`),
    timeout: 10_000,
  });
  await page.evaluate(url => new Promise((resolve, reject) => {
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
  }), `./sw.js?headers=${run}`);
  return workerPromise;
}

async function workerRequest(context, worker, name) {
  const requestPromise = context.waitForEvent('request', {
    predicate: request => request.url().endsWith(name),
    timeout: 10_000,
  });
  const [status, request] = await Promise.all([
    worker.evaluate(value => fetch(value).then(response => response.status), name),
    requestPromise,
  ]);
  assert.equal(request.serviceWorker(), worker);
  return { status, headers: request.headers() };
}

async function exercise(browser) {
  // Arrange
  const context = browser.contexts()[0];
  const page = context.pages().find(candidate => candidate.url().startsWith(fixture));
  assert.ok(page, `fixture Page not found: ${fixture}`);
  const run = String(Date.now());
  await context.setExtraHTTPHeaders({ 'X-Service-Worker-Creation': run });
  const worker = await controllingWorker(context, page, run);

  // Act
  const creation = await workerRequest(context, worker, `headers-creation-${run}.txt`);
  await context.setExtraHTTPHeaders({ 'X-Service-Worker-Updated': run });
  const updated = await workerRequest(context, worker, `headers-updated-${run}.txt`);
  await context.setExtraHTTPHeaders({});

  // Assert
  assert.equal(creation.headers['x-service-worker-creation'], run);
  assert.equal(creation.headers['x-service-worker-updated'], undefined);
  assert.equal(updated.headers['x-service-worker-creation'], undefined);
  assert.equal(updated.headers['x-service-worker-updated'], run);
  return {
    worker: worker.url(),
    creationStatus: creation.status,
    updatedStatus: updated.status,
    creationHeader: creation.headers['x-service-worker-creation'],
    updatedHeader: updated.headers['x-service-worker-updated'],
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
