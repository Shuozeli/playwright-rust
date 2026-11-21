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
  const existingWorkerPromise = context.waitForEvent('serviceworker', {
    predicate: candidate => candidate.url().includes(`route-existing=${run}`),
    timeout: 10_000,
  });
  await page.evaluate(
    ([url, scope]) => navigator.serviceWorker.register(url, { scope }),
    [`./sw.js?route-existing=${run}`, `./route-existing-${run}/`],
  );
  const worker = await existingWorkerPromise;
  const resource = `rust-worker-route-${run}.txt`;
  const matcher = `**/${resource}`;
  const body = `context-route-${run}`;
  let pageHits = 0;
  let contextHits = 0;
  const pageHandler = async route => {
    pageHits += 1;
    await route.fulfill({ status: 200, contentType: 'text/plain', body: 'page-route' });
  };
  const contextHandler = async (route, request) => {
    contextHits += 1;
    assert.equal(request.serviceWorker(), worker);
    assert.throws(
      () => request.frame(),
      /Service Worker requests do not have an associated frame\./,
    );
    await route.fulfill({ status: 201, contentType: 'text/plain', body });
  };
  await page.route(matcher, pageHandler);
  await context.route(matcher, contextHandler);

  let existingResult;
  try {
    // Act
    const result = await worker.evaluate(async name => {
      const response = await fetch(name);
      return { status: response.status, body: await response.text() };
    }, resource);

    // Assert
    assert.deepEqual(result, { status: 201, body });
    assert.equal(contextHits, 1);
    assert.equal(pageHits, 0);
    existingResult = {
      worker: worker.url(),
      resource,
      status: result.status,
      body: result.body,
      contextHits,
      pageHits,
      existingWorkerUpdated: true,
    };
  } finally {
    await context.unroute(matcher, contextHandler);
    await page.unroute(matcher, pageHandler);
  }

  // Arrange a Context route before Chromium attaches a second Worker. This
  // exercises the pre-resume Fetch.enable path rather than the runtime update.
  const creationRun = `${run}-creation`;
  const creationResource = `rust-worker-route-${creationRun}.txt`;
  const creationMatcher = `**/${creationResource}`;
  const creationBody = `context-route-${creationRun}`;
  let createdWorker;
  let creationContextHits = 0;
  let creationPageHits = 0;
  const creationPageHandler = async route => {
    creationPageHits += 1;
    await route.fulfill({ status: 200, contentType: 'text/plain', body: 'page-route' });
  };
  const creationContextHandler = async (route, request) => {
    creationContextHits += 1;
    assert.equal(request.serviceWorker(), createdWorker);
    await route.fulfill({
      status: 202,
      contentType: 'text/plain',
      body: creationBody,
    });
  };
  await page.route(creationMatcher, creationPageHandler);
  await context.route(creationMatcher, creationContextHandler);

  try {
    const workerPromise = context.waitForEvent('serviceworker', {
      predicate: candidate => candidate.url().includes(`route-creation=${creationRun}`),
      timeout: 10_000,
    });
    await page.evaluate(
      ([url, scope]) => navigator.serviceWorker.register(url, { scope }),
      [
        `./sw.js?route-creation=${creationRun}`,
        `./route-creation-${creationRun}/`,
      ],
    );
    createdWorker = await workerPromise;

    // Act
    const creationResult = await createdWorker.evaluate(async name => {
      const response = await fetch(name);
      return { status: response.status, body: await response.text() };
    }, creationResource);

    // Assert
    assert.deepEqual(creationResult, { status: 202, body: creationBody });
    assert.equal(creationContextHits, 1);
    assert.equal(creationPageHits, 0);
    return {
      ...existingResult,
      creationWorker: createdWorker.url(),
      creationStatus: creationResult.status,
      creationContextHits,
      creationPageHits,
      preResumeWorkerConfigured: true,
    };
  } finally {
    await context.unroute(creationMatcher, creationContextHandler);
    await page.unroute(creationMatcher, creationPageHandler);
  }
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
