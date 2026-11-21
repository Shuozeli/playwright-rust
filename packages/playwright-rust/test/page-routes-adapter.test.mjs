import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
class UnsupportedOperationError extends Error {}
const pageRoutes = require('../src/page-routes.cjs')({
  mapError(error, api) { return new Error(`${api}: ${error.message}`); },
  validateOptions(options, supported, api) {
    if (!options || typeof options !== 'object' || Array.isArray(options))
      throw new TypeError(`${api}: options must be an object`);
    for (const key of Object.keys(options)) {
      if (!supported.includes(key)) throw new UnsupportedOperationError(`${api} option ${key}`);
    }
  },
  UnsupportedOperationError,
  createResponseFromNotice(_page, notice, request) { return { notice, request }; },
});

function fixture() {
  const calls = [];
  const session = {
    async pageRoute(...args) { calls.push(['route', ...args]); return '17'; },
    async contextRoute(...args) { calls.push(['contextRoute', ...args]); return '27'; },
    async pageRouteContinue(...args) { calls.push(['continue', ...args]); },
    async pageRouteContinueWithHeaders(...args) { calls.push(['continueWithHeaders', ...args]); },
    async pageRouteFallback(...args) { calls.push(['fallback', ...args]); },
    async pageRouteFallbackWithUrl(...args) { calls.push(['fallbackWithUrl', ...args]); },
    async pageRouteFallbackWithOverrides(...args) { calls.push(['fallbackWithOverrides', ...args]); },
    async pageRouteAbort(...args) { calls.push(['abort', ...args]); },
    async pageRouteFulfill(...args) { calls.push(['fulfill', ...args]); },
    async contextUnroute(...args) { calls.push(['contextUnroute', ...args]); },
    async contextUnrouteAll(...args) { calls.push(['contextUnrouteAll', ...args]); },
    async pageUnrouteAll(...args) { calls.push(['pageUnrouteAll', ...args]); },
  };
  const browser = { _session: session, _contexts: new Map() };
  const context = {
    _browser: browser,
    _id: '1',
    _pages: new Map(),
    _routes: new Map(),
    _serviceWorkers: new Map(),
  };
  const page = {
    _context: context,
    _id: '2',
    _routes: new Map(),
    frames: () => [{ _id: '3' }],
  };
  context._pages.set('2', page);
  browser._contexts.set('1', context);
  return { calls, session, browser, context, page };
}

function routeCall(overrides = {}) {
  return {
    callId: '9',
    registrationId: '17',
    contextId: '1',
    pageId: '2',
    frameId: '3',
    url: 'https://example.test/app.css',
    method: 'GET',
    resourceType: 'Stylesheet',
    headers: [{ name: 'accept', value: 'text/css' }],
    postData: undefined,
    isNavigationRequest: false,
    expires: false,
    ...overrides,
  };
}

function enableUnroute({ calls, session }) {
  let nextRegistration = 17;
  session.pageRoute = async (...args) => {
    calls.push(['route', ...args]);
    return String(nextRegistration++);
  };
  session.pageUnroute = async (...args) => { calls.push(['unroute', ...args]); };
}

test('BrowserContext routeFromHAR delegates string replay and remains removable by URL', async () => {
  // Arrange
  const { calls, context, session } = fixture();
  session.contextRouteFromHar = async (...args) => {
    calls.push(['contextRouteFromHar', ...args]);
    return '61';
  };

  // Act
  await pageRoutes.routeFromHAR(context, '/tmp/archive.har', {
    url: '**/*.js',
    notFound: 'fallback',
  });
  await pageRoutes.unroute(context, '**/*.js');

  // Assert
  assert.deepEqual(calls, [
    ['contextRouteFromHar', '1', '/tmp/archive.har', '**/*.js', 'fallback'],
    ['contextUnroute', '1', ['61']],
  ]);
  assert.equal(context._routeRegistrations.size, 0);
});

test('Page routeFromHAR delegates RegExp replay without copying HAR matching into JavaScript', async () => {
  // Arrange
  const { calls, page, session } = fixture();
  session.pageRouteFromHarRegExp = async (...args) => {
    calls.push(['pageRouteFromHarRegExp', ...args]);
    return '62';
  };

  // Act
  await pageRoutes.routeFromHAR(page, '/tmp/archive.zip', {
    url: /\.(js|css)$/iu,
  });

  // Assert
  assert.deepEqual(calls, [[
    'pageRouteFromHarRegExp', '2', '/tmp/archive.zip', '\\.(js|css)$', 'iu', 'abort',
  ]]);
  assert.deepEqual(page._routeRegistrations.get('62'), {
    url: /\.(js|css)$/iu,
    regexpSource: '\\.(js|css)$',
    regexpFlags: 'iu',
  });
});

test('Page and Context routeFromHAR update delegate recording policy to Rust', async () => {
  // Arrange
  const { calls, context, page, session } = fixture();
  session.pageRecordHar = async (...args) => {
    calls.push(['pageRecordHar', ...args]);
    return '63';
  };
  session.contextRecordHarRegExp = async (...args) => {
    calls.push(['contextRecordHarRegExp', ...args]);
    return '64';
  };

  // Act
  await pageRoutes.routeFromHAR(page, '/tmp/page.har', { update: true });
  await pageRoutes.routeFromHAR(context, '/tmp/context.zip', {
    update: true,
    updateContent: 'embed',
    updateMode: 'full',
    url: /api/u,
  });

  // Assert
  assert.deepEqual(calls, [
    ['pageRecordHar', '2', '/tmp/page.har', undefined, 'attach', 'minimal'],
    ['contextRecordHarRegExp', '1', '/tmp/context.zip', 'api', 'u', {
      content: 'embed', mode: 'full',
    }],
  ]);
});

test('routeFromHAR validates options and fails closed for an unselected native leaf', async () => {
  // Arrange
  const { context } = fixture();

  // Act / Assert
  await assert.rejects(pageRoutes.routeFromHAR(context, '/tmp/archive.har'), /har-replay native profile/);
  await assert.rejects(
    pageRoutes.routeFromHAR(context, '/tmp/archive.har', { update: 'yes' }),
    /update must be a boolean/,
  );
  await assert.rejects(
    pageRoutes.routeFromHAR(context, '/tmp/archive.har', { notFound: 'continue' }),
    /notFound must be "abort" or "fallback"/,
  );
  await assert.rejects(
    pageRoutes.routeFromHAR(context, '/tmp/archive.har', { url: /api/ }),
    /har-replay-regexp native profile/,
  );
});

test('routed Request retains captured POST data when a later network notice omits it', async () => {
  // Arrange
  let requestEvents;
  const historyRoutes = require('../src/page-routes.cjs')({
    mapError: error => error,
    validateOptions(_options, _supported, _api) {},
    UnsupportedOperationError,
    canonicalizeRequest: (...args) => requestEvents.canonicalize(...args),
  });
  requestEvents = require('../src/request-events.cjs')({
    Request: historyRoutes.Request,
    mapError: error => error,
    validateOptions(_options, _supported, _api) {},
    UnsupportedOperationError,
    waitForEvent() {},
  });
  const { browser, page } = fixture();
  page._networkRequests = new Map();
  let emittedRequest;
  page.emit = (_kind, request) => { emittedRequest = request; };
  const body = Buffer.alloc(2 * 1024 * 1024, 0x30);
  const owner = Object.freeze({ id: 'post-owner' });
  let routedRequest;
  page._routes.set('17', async route => {
    routedRequest = route.request();
    await route.abort();
  });
  await historyRoutes.dispatch(browser, routeCall({
    requestId: 'post-1', historyOwner: owner, method: 'POST',
    resourceType: 'Fetch', postData: body,
  }));

  // Act
  requestEvents.dispatch(browser, {
    pageId: '2', kind: 'request', historyOwner: owner,
    request: {
      id: 'post-1', url: 'https://example.test/app.css', method: 'POST',
      resourceType: 'fetch', headers: [], postData: null,
      isNavigationRequest: false, frameId: '3', redirectedFrom: null,
    },
  });

  // Assert
  assert.equal(emittedRequest, routedRequest);
  assert.equal(routedRequest.postDataBuffer(), body);
  assert.equal(routedRequest.postData(), body.toString());
});

test('routed Request shares native hop identity with un-intercepted redirect successors', async () => {
  // Arrange
  let requestEvents;
  const historyRoutes = require('../src/page-routes.cjs')({
    mapError: error => error,
    validateOptions(_options, _supported, _api) {},
    UnsupportedOperationError,
    canonicalizeRequest: (...args) => requestEvents.canonicalize(...args),
  });
  requestEvents = require('../src/request-events.cjs')({
    Request: historyRoutes.Request,
    mapError: error => error,
    validateOptions(_options, _supported, _api) {},
    UnsupportedOperationError,
    waitForEvent() {},
  });
  const { browser, page } = fixture();
  page._networkRequests = new Map();
  const observed = [];
  page.emit = (event, request) => { if (event === 'request') observed.push(request); };
  const intercepted = [];
  page._routes.set('17', async route => {
    intercepted.push(route.request());
    await route.continue();
  });
  const owners = [1, 2, 3].map(id => Object.freeze({ id: `owner-${id}` }));
  const urls = ['/one.css', '/two.css', '/three.css'].map(path => `https://example.test${path}`);
  const notice = (index, kind) => ({
    pageId: '2', kind, historyOwner: owners[index],
    request: {
      id: `css-${index + 1}`,
      url: urls[index],
      method: 'GET',
      resourceType: 'stylesheet',
      headers: [],
      postData: null,
      isNavigationRequest: false,
      frameId: '3',
      redirectedFrom: index === 0 ? null : `css-${index}`,
    },
  });

  // Act
  await historyRoutes.dispatch(browser, routeCall({
    url: urls[0], requestId: 'css-1', redirectedFrom: null,
    historyOwner: owners[0],
  }));
  requestEvents.dispatch(browser, notice(0, 'request'));
  requestEvents.dispatch(browser, notice(0, 'requestfinished'));
  requestEvents.dispatch(browser, notice(1, 'request'));
  requestEvents.dispatch(browser, notice(1, 'requestfinished'));
  requestEvents.dispatch(browser, notice(2, 'request'));

  // Assert
  assert.equal(intercepted.length, 1);
  assert.equal(intercepted[0], observed[0]);
  assert.equal(intercepted[0].redirectedTo(), observed[1]);
  assert.equal(observed[1].redirectedTo(), observed[2]);
  assert.equal(observed[2].redirectedTo(), null);
  assert.deepEqual([intercepted[0], observed[1], observed[2]].map(request => request.url()), urls);
  assert.deepEqual([intercepted[0], observed[1], observed[2]].map(request => request.resourceType()),
    ['stylesheet', 'stylesheet', 'stylesheet']);
});

test('Page route registration stores the handler only after Rust returns its identity', async () => {
  // Arrange
  const { calls, page } = fixture();
  const handler = () => {};

  // Act
  await pageRoutes.route(page, '**/*.css', handler, { times: 2 });

  // Assert
  assert.deepEqual(calls, [['route', '2', '**/*.css', 2]]);
  assert.equal(page._routes.get('17'), handler);
});

test('BrowserContext route registration is owned by the Context and handles Page requests', async () => {
  // Arrange
  const { browser, calls, context, page } = fixture();
  const observed = [];
  const handler = async (route, request) => {
    observed.push([route, request, request.frame()]);
    await route.continue();
  };

  // Act
  await pageRoutes.route(context, '**/*.css', handler, { times: 2 });
  await pageRoutes.dispatch(browser, routeCall({ registrationId: '27' }));

  // Assert
  assert.deepEqual(calls[0], ['contextRoute', '1', '**/*.css', 2]);
  assert.equal(context._routes.get('27'), handler);
  assert.equal(page._routes.has('27'), false);
  assert.equal(observed.length, 1);
  assert.equal(observed[0][1].url(), 'https://example.test/app.css');
  assert.equal(observed[0][2]._id, '3');
  assert.deepEqual(calls.at(-1), ['continue', '9']);
});

test('BrowserContext route handles Service Worker requests without exposing them to Page routes', async () => {
  // Arrange
  const { browser, calls, context, page } = fixture();
  const worker = { _id: 'sw-1' };
  context._serviceWorkers.set(worker._id, worker);
  let pageHits = 0;
  let contextHits = 0;
  page._routes.set('27', async () => { pageHits += 1; });
  context._routes.set('27', async (route, request) => {
    contextHits += 1;
    assert.equal(request.serviceWorker(), worker);
    assert.throws(
      () => request.frame(),
      /Service Worker requests do not have an associated frame\./,
    );
    await route.continue();
  });

  // Act
  await pageRoutes.dispatch(browser, routeCall({
    registrationId: '27',
    pageId: undefined,
    frameId: undefined,
    serviceWorkerId: worker._id,
    resourceType: 'Fetch',
  }));

  // Assert
  assert.equal(contextHits, 1);
  assert.equal(pageHits, 0);
  assert.deepEqual(calls, [['continue', '9']]);
});

test('BrowserContext route delegates a RegExp source and supported flags to its native owner', async () => {
  // Arrange
  const { calls, context, session } = fixture();
  const handler = () => {};
  session.contextRouteRegExp = async (...args) => {
    calls.push(['contextRouteRegExp', ...args]);
    return '29';
  };

  // Act
  await pageRoutes.route(context, /assets\/.*\.css/imsu, handler, { times: 3 });

  // Assert
  assert.deepEqual(calls, [['contextRouteRegExp', '1', 'assets\\/.*\\.css', 'imsu', 3]]);
  assert.equal(context._routes.get('29'), handler);
});

test('BrowserContext unroute removes only matching Context registrations', async () => {
  // Arrange
  const { calls, context } = fixture();
  const handler = () => {};
  await pageRoutes.route(context, '**/*.css', handler);

  // Act
  await pageRoutes.unroute(context, '**/*.css', handler);

  // Assert
  assert.deepEqual(calls.at(-1), ['contextUnroute', '1', ['27']]);
  assert.equal(context._routes.size, 0);
  assert.equal(context._routeRegistrations.size, 0);
});

test('BrowserContext unrouteAll wait waits for already running handlers', async () => {
  // Arrange
  const { browser, calls, context } = fixture();
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  await pageRoutes.route(context, '**/*', async route => {
    await gate;
    await route.continue();
  });
  const dispatch = pageRoutes.dispatch(browser, routeCall({ registrationId: '27' }));
  await new Promise(resolve => setImmediate(resolve));

  // Act
  let settled = false;
  const unrouted = pageRoutes.unrouteAll(context, { behavior: 'wait' })
    .then(() => { settled = true; });
  await new Promise(resolve => setImmediate(resolve));

  // Assert
  assert.equal(settled, false);
  assert.equal(calls.some(call => call[0] === 'contextUnrouteAll'), false);
  release();
  await Promise.all([dispatch, unrouted]);
  assert.equal(settled, true);
  assert.deepEqual(calls.at(-1), ['contextUnrouteAll', '1', false]);
  assert.equal(context._routes.size, 0);
});

test('BrowserContext unrouteAll ignoreErrors suppresses failures from running handlers', async () => {
  // Arrange
  const { browser, context } = fixture();
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  await pageRoutes.route(context, '**/*', async () => {
    await gate;
    throw new Error('late handler failure');
  });
  const dispatch = pageRoutes.dispatch(browser, routeCall({ registrationId: '27' }));
  await new Promise(resolve => setImmediate(resolve));

  // Act
  await pageRoutes.unrouteAll(context, { behavior: 'ignoreErrors' });
  release();

  // Assert
  await assert.doesNotReject(dispatch);
  assert.equal(context._routes.size, 0);
});

test('Page unroute with a handler removes only its exact registration identities', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  const first = () => {};
  const second = () => {};
  await pageRoutes.route(value.page, '**/*.css', first, { times: 2 });
  await pageRoutes.route(value.page, '**/*.css', second, { times: 3 });
  await pageRoutes.route(value.page, '**/*.js', first);

  // Act
  await pageRoutes.unroute(value.page, '**/*.css', first);

  // Assert
  assert.deepEqual(value.calls.at(-1), ['unroute', '2', ['17']]);
  assert.deepEqual([...value.page._routes.keys()], ['18', '19']);
  assert.deepEqual([...value.page._routeRegistrations.keys()], ['18', '19']);
  assert.equal(value.page._routeRegistrations.get('18').handler, second);
});

test('Page unroute without a handler removes all matching URL registrations in order', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  const handler = () => {};
  await pageRoutes.route(value.page, '**/*.css', handler);
  await pageRoutes.route(value.page, '**/*.js', handler);
  await pageRoutes.route(value.page, '**/*.css', handler);

  // Act
  await pageRoutes.unroute(value.page, '**/*.css');

  // Assert
  assert.deepEqual(value.calls.at(-1), ['unroute', '2', ['17', '19']]);
  assert.deepEqual([...value.page._routes.keys()], ['18']);
  assert.deepEqual([...value.page._routeRegistrations.keys()], ['18']);
});

test('Page unroute waits for an earlier registration before selecting its ID', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  let releaseRegistration;
  value.session.pageRoute = async (...args) => {
    value.calls.push(['route', ...args]);
    await new Promise(resolve => { releaseRegistration = resolve; });
    return '17';
  };
  const handler = () => {};
  const registration = pageRoutes.route(value.page, '**/*.css', handler);
  const removal = pageRoutes.unroute(value.page, '**/*.css', handler);
  await Promise.resolve();

  // Act
  releaseRegistration();
  await Promise.all([registration, removal]);

  // Assert
  assert.deepEqual(value.calls, [
    ['route', '2', '**/*.css', undefined],
    ['unroute', '2', ['17']],
  ]);
  assert.equal(value.page._routes.size, 0);
});

test('Page unroute keeps registration metadata when the Rust owner rejects removal', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  const handler = () => {};
  await pageRoutes.route(value.page, '**/*.css', handler);
  value.session.pageUnroute = async () => { throw new Error('native removal failed'); };

  // Act
  const removal = pageRoutes.unroute(value.page, '**/*.css', handler);

  // Assert
  await assert.rejects(removal, /native removal failed/);
  assert.equal(value.page._routes.get('17'), handler);
  assert.deepEqual([...value.page._routeRegistrations.keys()], ['17']);
});

test('Page unroute does not invoke Rust for a nonmatching URL or handler', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  const handler = () => {};
  await pageRoutes.route(value.page, '**/*.css', handler);
  const initialCalls = value.calls.length;

  // Act
  await pageRoutes.unroute(value.page, '**/*.css', () => {});

  // Assert
  assert.equal(value.calls.length, initialCalls);
  assert.equal(value.page._routes.get('17'), handler);
});

test('Page unroute rejects RegExp matchers without the opt-in native binding', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);

  // Act
  const nonString = pageRoutes.unroute(value.page, /css/);

  // Assert
  await assert.rejects(nonString, /route-regexp native profile/);
  assert.deepEqual(value.calls, []);
});

test('Page route delegates a RegExp source and supported flags to its opt-in native binding', async () => {
  // Arrange
  const value = fixture();
  const handler = () => {};
  value.session.pageRouteRegExp = async (...args) => {
    value.calls.push(['regexp', ...args]);
    return '23';
  };

  // Act
  await pageRoutes.route(value.page, /empty\.html/imsu, handler, { times: 2 });

  // Assert
  assert.deepEqual(value.calls, [['regexp', '2', 'empty\\.html', 'imsu', 2]]);
  assert.equal(value.page._routes.get('23'), handler);
  assert.equal(value.page._routeRegistrations, undefined);
});

test('Page route rejects a RegExp without the opt-in native binding before registration', async () => {
  // Arrange
  const value = fixture();

  // Act
  const registration = pageRoutes.route(value.page, /empty\.html/, () => {});

  // Assert
  await assert.rejects(registration, /route-regexp native profile/);
  assert.deepEqual(value.calls, []);
  assert.equal(value.page._routes.size, 0);
});

for (const flag of ['g', 'y', 'd']) {
  test(`Page route forwards ECMAScript RegExp flag ${flag} to native work`, async () => {
    // Arrange
    const value = fixture();
    value.session.pageRouteRegExp = async (...args) => {
      value.calls.push(['regexp', ...args]);
      return '23';
    };

    // Act
    await pageRoutes.route(value.page, new RegExp('empty', flag), () => {});

    // Assert
    assert.deepEqual(value.calls, [['regexp', '2', 'empty', flag, undefined]]);
    assert.equal(value.page._routes.size, 1);
  });
}

let unicodeSetsFlagAvailable = true;
try { new RegExp('empty', 'v'); }
catch { unicodeSetsFlagAvailable = false; }
test('Page route forwards the ECMAScript RegExp v flag to native work', { skip: !unicodeSetsFlagAvailable }, async () => {
  // Arrange
  const value = fixture();
  value.session.pageRouteRegExp = async (...args) => {
    value.calls.push(['regexp', ...args]);
    return '23';
  };

  // Act
  await pageRoutes.route(value.page, new RegExp('empty', 'v'), () => {});

  // Assert
  assert.deepEqual(value.calls, [['regexp', '2', 'empty', 'v', undefined]]);
  assert.equal(value.page._routes.size, 1);
});

test('Page route forwards a RegExp source beyond the former arbitrary limit', async () => {
  // Arrange
  const value = fixture();
  value.session.pageRouteRegExp = async (...args) => {
    value.calls.push(['regexp', ...args]);
    return '23';
  };
  const source = 'x'.repeat(4097);

  // Act
  await pageRoutes.route(value.page, new RegExp(source), () => {});

  // Assert
  assert.deepEqual(value.calls, [['regexp', '2', source, '', undefined]]);
  assert.equal(value.page._routes.size, 1);
});

test('Page unroute compares RegExp source and flags, handler identity, and matcher kind', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  let nextRegistration = 20;
  value.session.pageRouteRegExp = async (...args) => {
    value.calls.push(['regexp', ...args]);
    return String(nextRegistration++);
  };
  const first = () => {};
  const second = () => {};
  await pageRoutes.route(value.page, 'empty', first);
  await pageRoutes.route(value.page, /empty/i, first);
  await pageRoutes.route(value.page, /empty/i, second);
  await pageRoutes.route(value.page, /empty/m, first);
  await pageRoutes.unroute(value.page, /empty/i, () => {});
  await pageRoutes.unroute(value.page, /empty/, first);

  // Act
  await pageRoutes.unroute(value.page, /empty/i, first);

  // Assert
  assert.deepEqual(value.calls.at(-1), ['unroute', '2', ['20']]);
  assert.deepEqual([...value.page._routes.keys()], ['17', '21', '22']);
  assert.deepEqual([...value.page._routeRegistrations.keys()], ['17', '21', '22']);
});

test('Page unroute waits for an earlier RegExp registration and removes only its returned ID', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  let releaseRegistration;
  value.session.pageRouteRegExp = async (...args) => {
    value.calls.push(['regexp', ...args]);
    return new Promise(resolve => { releaseRegistration = resolve; });
  };
  const handler = () => {};
  const registration = pageRoutes.route(value.page, /\.css$/, handler);
  const removal = pageRoutes.unroute(value.page, /\.css$/, handler);
  await Promise.resolve();

  // Act
  releaseRegistration('24');
  await Promise.all([registration, removal]);

  // Assert
  assert.deepEqual(value.calls, [
    ['regexp', '2', '\\.css$', '', undefined],
    ['unroute', '2', ['24']],
  ]);
  assert.equal(value.page._routes.size, 0);
  assert.equal(value.page._routeRegistrations.size, 0);
});

test('Page route leaves no RegExp registration when its native owner rejects', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  value.session.pageRouteRegExp = async () => { throw new Error('native regex limit'); };

  // Act
  const registration = pageRoutes.route(value.page, /\.css$/, () => {});

  // Assert
  await assert.rejects(registration, /native regex limit/);
  assert.equal(value.page._routes.size, 0);
  assert.equal(value.page._routeRegistrations, undefined);
});

test('Page route dispatches the native-selected RegExp registration identity', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  let nextRegistration = 20;
  value.session.pageRouteRegExp = async (...args) => {
    value.calls.push(['regexp', ...args]);
    return String(nextRegistration++);
  };
  const observed = [];
  await pageRoutes.route(value.page, /\.css$/, async route => {
    observed.push('first');
    await route.continue();
  });
  await pageRoutes.route(value.page, /\.css$/, async route => {
    observed.push('second');
    await route.continue();
  });

  // Act
  await pageRoutes.dispatch(value.browser, routeCall({ registrationId: '21' }));

  // Assert
  assert.deepEqual(observed, ['second']);
  assert.deepEqual(value.calls.at(-1), ['continue', '9']);
  assert.deepEqual([...value.page._routeRegistrations.keys()], ['20', '21']);
});

test('Page unroute forwards a string matcher beyond the former arbitrary limit', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  const matcher = 'x'.repeat(4097);
  value.page._routeRegistrations = new Map([
    ['20', { url: matcher, handler: () => {} }],
  ]);
  value.page._routes.set('20', value.page._routeRegistrations.get('20').handler);

  // Act
  await pageRoutes.unroute(value.page, matcher);

  // Assert
  assert.deepEqual(value.calls, [['unroute', '2', ['20']]]);
  assert.equal(value.page._routes.size, 0);
});

test('Page unroute requires a callable handler before native work', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);

  // Act
  const invalidHandler = pageRoutes.unroute(value.page, '**/*.css', 42);

  // Assert
  await assert.rejects(invalidHandler, /handler must be a function/);
  assert.deepEqual(value.calls, []);
});

test('Page route mutations queue past the former 256-operation rejection', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  let releaseRegistration;
  value.session.pageRoute = async () => new Promise(resolve => { releaseRegistration = resolve; });
  const pending = [pageRoutes.route(value.page, '**/*.css', () => {})];
  for (let index = 0; index < 256; index++)
    pending.push(pageRoutes.unroute(value.page, '**/*.js'));
  await Promise.resolve();

  // Act
  pending.push(pageRoutes.unroute(value.page, '**/*.js'));
  releaseRegistration('17');

  // Assert
  await Promise.all(pending);
  assert.equal(value.page._routes.get('17') instanceof Function, true);
});

test('Page unroute requires its distinct opt-in native binding', async () => {
  // Arrange
  const { calls, page } = fixture();

  // Act
  const unsupported = pageRoutes.unroute(page, '**/*.css');

  // Assert
  await assert.rejects(unsupported, /page-unroute native profile/);
  assert.deepEqual(calls, []);
});

test('Route call keeps the request paused after a synchronous handler publishes the wrapper', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });

  // Act
  await pageRoutes.dispatch(browser, routeCall());

  // Assert
  assert.equal(published.request().url(), 'https://example.test/app.css');
  assert.equal(published.request().frame()._id, '3');
  assert.deepEqual(published.request().headers(), { accept: 'text/css' });
  assert.deepEqual(calls, []);
});

test('Route request allHeaders resolves a normalized native snapshot including cookies', async () => {
  // Arrange
  const { browser, page } = fixture();
  const nativeHeaders = [
    { name: 'Accept', value: 'text/css' },
    { name: 'Cookie', value: 'session=original' },
    { name: 'X-Trace', value: 'retained' },
  ];
  let headerPromise;
  page._routes.set('17', route => { headerPromise = route.request().allHeaders(); });

  // Act
  await pageRoutes.dispatch(browser, routeCall({ headers: nativeHeaders }));

  // Assert
  assert.ok(headerPromise instanceof Promise);
  assert.deepEqual(await headerPromise, {
    accept: 'text/css',
    cookie: 'session=original',
    'x-trace': 'retained',
  });
});

test('Route request allHeaders cannot mutate its retained headers or native snapshot', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  const nativeHeaders = [
    { name: 'Cookie', value: 'session=original' },
    { name: 'Accept', value: 'text/css' },
  ];
  let retainedRequest;
  let secondRead;
  page._routes.set('17', async route => {
    retainedRequest = route.request();
    const override = await retainedRequest.allHeaders();
    override.cookie = 'session=overridden';
    override.accept = 'application/json';
    secondRead = await retainedRequest.allHeaders();
    await route.continue({ headers: override });
  });

  // Act
  await pageRoutes.dispatch(browser, routeCall({ headers: nativeHeaders }));

  // Assert
  assert.deepEqual(secondRead, { cookie: 'session=original', accept: 'text/css' });
  assert.deepEqual(retainedRequest.headers(), secondRead);
  assert.deepEqual(nativeHeaders, [
    { name: 'Cookie', value: 'session=original' },
    { name: 'Accept', value: 'text/css' },
  ]);
  assert.deepEqual(calls, [['continueWithHeaders', '9', [
    { name: 'cookie', value: 'session=overridden' },
    { name: 'accept', value: 'application/json' },
  ]]]);
});

test('Route request allHeaders preserves handler errors and aborts an unhandled call', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  const failure = new Error('handler failed after reading headers');
  let observedHeaders;
  page._routes.set('17', async route => {
    observedHeaders = await route.request().allHeaders();
    throw failure;
  });

  // Act
  const dispatched = pageRoutes.dispatch(browser, routeCall());

  // Assert
  await assert.rejects(dispatched, error => error === failure);
  assert.deepEqual(observedHeaders, { accept: 'text/css' });
  assert.deepEqual(calls, [['abort', '9', 'failed']]);
});

test('Route request response uses the exact Rust history owner captured before callback', async () => {
  // Arrange
  const { browser, page, session } = fixture();
  const firstHopOwner = { hop: 'first', chargedByRust: true };
  const responseNotice = { response: { status: 404 } };
  const observedOwners = [];
  session.requestResponse = async owner => {
    observedOwners.push(owner);
    return responseNotice;
  };
  let retainedRequest;
  page._routes.set('17', route => { retainedRequest = route.request(); });

  // Act
  await pageRoutes.dispatch(browser, routeCall({ historyOwner: firstHopOwner }));
  const response = await retainedRequest.response();

  // Assert
  assert.deepEqual(observedOwners, [firstHopOwner]);
  assert.equal(response.notice, responseNotice);
  assert.equal(response.request, retainedRequest);
  assert.equal((await retainedRequest.response()), response);
  assert.deepEqual(observedOwners, [firstHopOwner]);
});

test('Route request response fails closed when Rust did not capture a history owner', async () => {
  // Arrange
  const { browser, page, session } = fixture();
  session.requestResponse = async () => { throw new Error('must not look up by URL or native ID'); };
  let retainedRequest;
  page._routes.set('17', route => { retainedRequest = route.request(); });

  // Act
  await pageRoutes.dispatch(browser, routeCall({ historyOwner: null }));

  // Assert
  await assert.rejects(retainedRequest.response(), /request\.response/);
});

test('Published Route continue delegates one settlement to its Rust call owner', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  await published.continue();

  // Assert
  assert.deepEqual(calls, [['continue', '9']]);
  await assert.rejects(published.continue(), /Route is already handled!/);
});

test('Published Route continue forwards bounded headers to the opt-in Rust owner', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  await published.continue({ headers: { accept: 'text/css', 'x-test': 'yes' } });

  // Assert
  assert.deepEqual(calls, [['continueWithHeaders', '9', [
    { name: 'accept', value: 'text/css' },
    { name: 'x-test', value: 'yes' },
  ]]]);
});

test('Published Route continue rejects malformed headers before claiming the Rust call', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  const invalidValue = published.continue({ headers: { accept: 42 } });

  // Assert
  await assert.rejects(invalidValue, /header accept must be a string/);
  assert.deepEqual(calls, []);
});

test('Published Route continue bounds header bytes before native conversion', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  const oversized = published.continue({ headers: { accept: 'x'.repeat(64 * 1024 + 1) } });

  // Assert
  await assert.rejects(oversized, /headers exceed byte limit/);
  assert.deepEqual(calls, []);
});

test('Published Route continue does not silently discard headers with an old native profile', async () => {
  // Arrange
  const { calls, browser, page, session } = fixture();
  delete session.pageRouteContinueWithHeaders;
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  const unsupported = published.continue({ headers: { 'x-test': 'yes' } });

  // Assert
  await assert.rejects(unsupported, /route-header-url-overrides native profile/);
  assert.deepEqual(calls, []);
});

test('Published Route fallback forwards its replacement URL to the opt-in Rust owner', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  await published.fallback({ url: 'https://example.test/bar' });

  // Assert
  assert.deepEqual(calls, [[
    'fallbackWithOverrides', '9', 'https://example.test/bar', undefined, undefined, undefined,
  ]]);
});

test('Published Route fallback composes method, headers, and serialized POST data in Rust', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  await published.fallback({
    url: 'https://example.test/bar',
    method: 'POST',
    headers: { 'x-test': 'yes' },
    postData: 11,
  });

  // Assert
  assert.deepEqual(calls, [[
    'fallbackWithOverrides', '9', 'https://example.test/bar', 'POST',
    [{ name: 'x-test', value: 'yes' }], Buffer.from('11'),
  ]]);
});

test('Published Route fallback rejects a non-string URL before native conversion', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  const invalid = published.fallback({ url: 42 });

  // Assert
  await assert.rejects(invalid, /url must be a string/);
  assert.deepEqual(calls, []);
});

test('Published Route fallback does not silently discard URL with an old native profile', async () => {
  // Arrange
  const { calls, browser, page, session } = fixture();
  delete session.pageRouteFallbackWithUrl;
  delete session.pageRouteFallbackWithOverrides;
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  const unsupported = published.fallback({ url: 'https://example.test/bar' });

  // Assert
  await assert.rejects(unsupported, /route-header-url-overrides native profile/);
  assert.deepEqual(calls, []);
});

test('Published Route fulfill normalizes public values before one Rust settlement', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  await published.fulfill({
    status: 201,
    headers: { 'Content-Type': 'ignored', 'x-test': 'yes' },
    contentType: 'text/plain',
    body: 'héllo',
  });

  // Assert
  assert.deepEqual(calls, [[
    'fulfill', '9', 201,
    [{ name: 'x-test', value: 'yes' }, { name: 'content-type', value: 'text/plain' }],
    Buffer.from('héllo'),
  ]]);
  await assert.rejects(published.fulfill(), /Route is already handled!/);
});

for (const method of ['fulfill', 'continue', 'fallback', 'abort']) {
  test(`Published Route ${method} rejects a second settlement without calling Rust again`, async () => {
    // Arrange
    const { calls, browser, page } = fixture();
    let published;
    page._routes.set('17', route => { published = route; });
    await pageRoutes.dispatch(browser, routeCall());
    await published[method]();
    const settledCalls = calls.length;

    // Act
    const repeat = published[method]();

    // Assert
    await assert.rejects(repeat, {
      message: `route.${method}: Route is already handled!`,
    });
    assert.equal(settledCalls, 1);
    assert.equal(calls.length, settledCalls);
    assert.equal(calls[0][0], method);
  });
}

test('Published Route fulfill JSON selects the default content type', async () => {
  // Arrange
  const { calls, browser, page } = fixture();
  let published;
  page._routes.set('17', route => { published = route; });
  await pageRoutes.dispatch(browser, routeCall());

  // Act
  await published.fulfill({ json: { answer: 42 } });

  // Assert
  assert.deepEqual(calls, [[
    'fulfill', '9', 200,
    [{ name: 'content-type', value: 'application/json' }],
    Buffer.from('{"answer":42}'),
  ]]);
});

test('Missing registration falls back through Rust instead of leaving Chromium paused', async () => {
  // Arrange
  const { calls, browser } = fixture();

  // Act
  await pageRoutes.dispatch(browser, routeCall({ registrationId: '404' }));

  // Assert
  assert.deepEqual(calls, [['fallback', '9']]);
});

test('Expiring route registration is retired before user dispatch', async () => {
  // Arrange
  const { browser, page } = fixture();
  let presentDuringHandler = true;
  page._routes.set('17', () => { presentDuringHandler = page._routes.has('17'); });

  // Act
  await pageRoutes.dispatch(browser, routeCall({ expires: true }));

  // Assert
  assert.equal(presentDuringHandler, false);
  assert.equal(page._routes.has('17'), false);
});

test('Expiring finite-times registration removes its unroute identity before dispatch', async () => {
  // Arrange
  const value = fixture();
  enableUnroute(value);
  const handler = () => {};
  await pageRoutes.route(value.page, '**/*.css', handler, { times: 1 });
  const registrationCalls = value.calls.length;

  // Act
  await pageRoutes.dispatch(value.browser, routeCall({ expires: true }));

  // Assert
  assert.equal(value.page._routes.has('17'), false);
  assert.equal(value.page._routeRegistrations.has('17'), false);
  assert.equal(value.calls.length, registrationCalls);
});
