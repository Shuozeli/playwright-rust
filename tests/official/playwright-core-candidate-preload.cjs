'use strict';

const Module = require('node:module');
const path = require('node:path');

const candidate = process.env.PLAYWRIGHT_RUST_NODE_CANDIDATE;
const upstream = process.env.PLAYWRIGHT_RUST_PLAYWRIGHT_CORE_UPSTREAM;
if (!candidate) throw new Error('PLAYWRIGHT_RUST_NODE_CANDIDATE is required');
if (!upstream) throw new Error('PLAYWRIGHT_RUST_PLAYWRIGHT_CORE_UPSTREAM is required');

const candidateEntry = require.resolve(path.resolve(candidate));
const testModeSource = path.join(path.resolve(upstream), 'tests', 'config', 'testMode.ts');
const originalLoad = Module._load;
let adaptedCandidate;

function candidateForOfficialFixtures(parent, isMain) {
  if (adaptedCandidate) return adaptedCandidate;
  const implementation = originalLoad(candidateEntry, parent, isMain);
  if (!implementation?.chromium)
    throw new Error(`Playwright Rust candidate has no chromium export: ${candidateEntry}`);

  // @playwright/test's fixture recorder consumes these Playwright-client
  // internals even when trace and screenshot fixtures are unused. Keep the
  // browser adapter test-only, but preserve the candidate-owned APIRequest
  // implementation so official tests exercise it rather than a placeholder.
  const listeners = new Set();
  const instrumentation = new Proxy({}, {
    get(target, property) {
      if (typeof property !== 'string') return target[property];
      if (property === 'addListener') return listener => listeners.add(listener);
      if (property === 'removeListener') return listener => listeners.delete(listener);
      if (property === 'removeAllListeners') return () => listeners.clear();
      if (property.startsWith('run')) {
        return async (...args) => {
          for (const listener of listeners) await listener[property]?.(...args);
        };
      }
      if (property.startsWith('on')) {
        return (...args) => {
          for (const listener of listeners) listener[property]?.(...args);
        };
      }
      return target[property];
    },
  });
  const contexts = new Set();
  const request = Object.create(implementation.request);
  Object.defineProperty(request, 'newContext', {
    configurable: false,
    enumerable: true,
    writable: false,
    value: async (...args) => {
      const context = await implementation.request.newContext(...args);
      // The upstream @playwright/test artifact recorder reaches into these
      // client internals for request contexts that a test intentionally leaves
      // open. They are test-harness protocol, not public request behavior.
      context._wrapApiCall ??= async callback => callback({ internal: true });
      context._tracing ??= {
        async start() {},
        async startChunk() {},
        async stop() {},
        async stopChunk() {},
      };
      return context;
    },
  });
  const adaptedContexts = new WeakSet();
  const adaptContext = async context => {
    if (adaptedContexts.has(context)) return context;
    adaptedContexts.add(context);
    contexts.add(context);
    context._wrapApiCall ??= async callback => callback();
    context.tracing ??= {
      async start() { throw new implementation.errors.UnsupportedOperationError('browserContext.tracing.start'); },
      async startChunk() { throw new implementation.errors.UnsupportedOperationError('browserContext.tracing.startChunk'); },
      async stop() { throw new implementation.errors.UnsupportedOperationError('browserContext.tracing.stop'); },
      async stopChunk() { throw new implementation.errors.UnsupportedOperationError('browserContext.tracing.stopChunk'); },
    };
    context._tracing ??= context.tracing;
    const closeContext = context.close.bind(context);
    context.close = async (...args) => {
      try {
        await instrumentation.runBeforeCloseBrowserContext(context);
        return await closeContext(...args);
      } finally {
        contexts.delete(context);
      }
    };
    return context;
  };
  const adaptBrowser = browser => {
    browser._connection = { headers: [] };
    browser._wrapApiCall ??= async callback => callback();
    if (typeof browser._session?.killOwnedBrowserForTests === 'function') {
      browser._channel = {
        killForTests: () => browser._session.killOwnedBrowserForTests(),
      };
    }
    const newContext = browser.newContext.bind(browser);
    browser.newContext = async options => {
      // @playwright/test applies per-test context options through this
      // client-instrumentation hook before Browser.newContext. The candidate
      // has no upstream ChannelOwner, so the official adapter drives the same
      // already-registered fixture hook explicitly.
      const preparedOptions = { ...(options ?? {}) };
      await instrumentation.runBeforeCreateBrowserContext(preparedOptions);
      return adaptContext(await newContext(preparedOptions));
    };
    return browser;
  };
  const endpoint = process.env.PLAYWRIGHT_RUST_CANDIDATE_CDP_ENDPOINT;
  const ownedLaunch = process.env.PLAYWRIGHT_RUST_CANDIDATE_OWNED_LAUNCH === '1';
  if (endpoint && ownedLaunch)
    throw new Error('Playwright Rust candidate cannot select CDP and owned launch together');
  const chromium = Object.create(implementation.chromium);
  if (ownedLaunch) {
    const launch = implementation.chromium.launch.bind(implementation.chromium);
    chromium.launch = async options => adaptBrowser(await launch(options));
    const launchPersistentContext = implementation.chromium.launchPersistentContext
      .bind(implementation.chromium);
    chromium.launchPersistentContext = async (userDataDir, options) => {
      const preparedOptions = { ...(options ?? {}) };
      await instrumentation.runBeforeCreateBrowserContext(preparedOptions);
      return adaptContext(await launchPersistentContext(userDataDir, preparedOptions));
    };
  } else if (endpoint) {
    chromium.launch = async () => {
      process.stderr.write(
        `[playwright-rust-candidate] browserType.launch fixture adapter -> connectOverCDP(${endpoint})\n`,
      );
      return adaptBrowser(await implementation.chromium.connectOverCDP(endpoint));
    };
    const launchPersistentContext = implementation.chromium.launchPersistentContext
      .bind(implementation.chromium);
    chromium.launchPersistentContext = async (userDataDir, options) => {
      const preparedOptions = { ...(options ?? {}) };
      await instrumentation.runBeforeCreateBrowserContext(preparedOptions);
      return adaptContext(await launchPersistentContext(userDataDir, preparedOptions));
    };
  }
  adaptedCandidate = Object.assign(Object.create(implementation), implementation, {
    chromium,
    _allContexts: () => [...contexts],
    _allPages: () => [...contexts].flatMap(context => context.pages()),
    _instrumentation: instrumentation,
    request,
  });
  return adaptedCandidate;
}

Module._load = function playwrightRustCandidateLoad(request, parent, isMain) {
  if (request === 'playwright-core' && parent?.filename === testModeSource) {
    process.stderr.write(
      `[playwright-rust-candidate] ${testModeSource} -> ${candidateEntry}\n`,
    );
    return candidateForOfficialFixtures(parent, isMain);
  }
  return originalLoad(request, parent, isMain);
};
