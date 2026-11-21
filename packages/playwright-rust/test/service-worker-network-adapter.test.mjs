import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { createRequire } from 'node:module';
import { test } from 'node:test';

const require = createRequire(import.meta.url);
class UnsupportedOperationError extends Error {}
const mapError = error => error;
const serviceWorkerEvents = require('../src/service-worker-events.cjs')({
  mapError,
  UnsupportedOperationError,
});
const pageRoutes = require('../src/page-routes.cjs')({
  mapError,
  validateOptions() {},
  UnsupportedOperationError,
});
const requestEvents = require('../src/request-events.cjs')({
  Request: pageRoutes.Request,
  mapError,
  validateOptions() {},
  UnsupportedOperationError,
  waitForEvent() {},
});
const responseEvents = require('../src/response-events.cjs')({
  canonicalizeRequest: requestEvents.canonicalize,
  createResponse(_owner, metadata, _frame, request) {
    return {
      request: () => request,
      fromServiceWorker: () => metadata.fromServiceWorker === true,
    };
  },
  mapError,
  validateOptions() {},
  UnsupportedOperationError,
  waitForEvent() {},
});

function fixture() {
  const nativeOwner = Object.freeze({ native: 'service-worker-owner' });
  const session = {
    contextServiceWorkers: () => [{ id: 'sw-1', url: 'https://example.test/sw.js', owner: nativeOwner }],
    workerEvaluate: async (...args) => args,
  };
  const browser = { _session: session, _contexts: new Map() };
  const context = Object.assign(new EventEmitter(), {
    _id: 'context-1',
    _browser: browser,
    _pages: new Map(),
    _serviceWorkers: new Map(),
    _networkRequests: new Map(),
  });
  context.serviceWorkers = () => serviceWorkerEvents.snapshot(context);
  const page = Object.assign(new EventEmitter(), {
    _id: 'page-1',
    _context: context,
    _networkRequests: new Map(),
    frames: () => [],
  });
  context._pages.set(page._id, page);
  browser._contexts.set(context._id, context);
  return { browser, context, nativeOwner, page, session };
}

function requestMetadata() {
  return {
    id: 'request-1',
    contextId: 'context-1',
    serviceWorkerId: 'sw-1',
    url: 'https://example.test/data.json',
    method: 'GET',
    resourceType: 'fetch',
    headers: [],
    postData: null,
    isNavigationRequest: false,
    frameId: null,
    frameAvailable: false,
    redirectedFrom: null,
  };
}

test('Context-owned Service Worker request lifecycle preserves exact identities', () => {
  // Arrange
  const { browser, context, page } = fixture();
  serviceWorkerEvents.dispatch(browser, {
    kind: 'created',
    contextId: context._id,
    worker: context._browser._session.contextServiceWorkers()[0],
  });
  const worker = context.serviceWorkers()[0];
  const requests = [];
  const responses = [];
  const pageEvents = [];
  context.on('request', request => requests.push(request));
  context.on('response', response => responses.push(response));
  context.on('requestfinished', request => requests.push(request));
  page.on('request', value => pageEvents.push(value));
  page.on('response', value => pageEvents.push(value));
  const metadata = requestMetadata();

  // Act
  requestEvents.dispatch(browser, {
    kind: 'request',
    pageId: null,
    contextId: context._id,
    serviceWorkerId: worker._id,
    request: metadata,
  });
  responseEvents.dispatch(browser, {
    pageId: null,
    contextId: context._id,
    serviceWorkerId: worker._id,
    nativeRequestId: 'native-1',
    response: {
      requestId: metadata.id,
      request: metadata,
      contextId: context._id,
      serviceWorkerId: worker._id,
      fromServiceWorker: false,
      url: metadata.url,
      status: 200,
      statusText: 'OK',
      ok: true,
      headers: {},
    },
  });
  requestEvents.dispatch(browser, {
    kind: 'requestfinished',
    pageId: null,
    contextId: context._id,
    serviceWorkerId: worker._id,
    request: metadata,
  });

  // Assert
  assert.equal(requests.length, 2);
  assert.equal(requests[0], requests[1]);
  assert.equal(requests[0].serviceWorker(), worker);
  assert.throws(
    () => requests[0].frame(),
    /Service Worker requests do not have an associated frame\./,
  );
  assert.equal(responses.length, 1);
  assert.equal(responses[0].request(), requests[0]);
  assert.equal(responses[0].fromServiceWorker(), false);
  assert.equal(pageEvents.length, 0);
  assert.equal(context._networkRequests.size, 0);
});

test('Service Worker snapshots, evaluation, and close notices reuse one wrapper', async () => {
  // Arrange
  const { browser, context, nativeOwner, session } = fixture();
  const created = [];
  context.on('serviceworker', worker => created.push(worker));
  const descriptor = session.contextServiceWorkers()[0];
  serviceWorkerEvents.dispatch(browser, {
    kind: 'created',
    contextId: context._id,
    worker: descriptor,
  });
  const worker = context.serviceWorkers()[0];
  let closed = 0;
  worker.on('close', () => { closed += 1; });

  // Act
  const forwarded = await worker.evaluate(value => value + 1, 41);
  serviceWorkerEvents.dispatch(browser, {
    kind: 'closed',
    contextId: context._id,
    worker: descriptor,
  });

  // Assert
  assert.equal(created[0], worker);
  assert.equal(forwarded[0], nativeOwner);
  assert.equal(forwarded[2], true);
  assert.equal(forwarded[3], 41);
  assert.equal(closed, 1);
  assert.equal(context._serviceWorkers.size, 0);
});
