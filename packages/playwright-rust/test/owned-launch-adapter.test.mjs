import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { compileFunction } from 'node:vm';

const indexUrl = new URL('../src/index.cjs', import.meta.url);
const indexSource = readFileSync(indexUrl, 'utf8');
const sourceRequire = createRequire(indexUrl);

function fixture(profile = 'owned-launch', executableExists = () => true,
    startupFailure = new Error('PLAYWRIGHT_RUST_BACKEND: Failed to launch the browser process.\nBrowser logs:\n[exit=1]')) {
  const calls = [];
  const stacks = [];
  const contexts = new Set();
  let connected = true;
  const session = {
    isConnected: () => connected,
    sessionSignal: () => ({ wait: () => new Promise(() => {}) }),
    setLifecycleCallback(callback) { this.lifecycleCallback = callback; },
    contextSnapshot(ids) {
      return {
        ids: [...contexts],
        newContexts: [...contexts].filter(id => !ids.includes(id)).map(id => ({ id, settings: {} })),
      };
    },
    async newContext(baseURL) {
      calls.push(['newContext', baseURL]);
      contexts.add('fresh-context');
      return { id: 'fresh-context', settings: { resolveUrl: url => url } };
    },
    async newPage(contextId) {
      calls.push(['newPage', contextId]);
      return {
        id: 'page-1',
        mainFrameId: 'frame-1',
        mainFrameMetadata: {
          url: () => 'about:blank',
          name: () => '',
          isDetached: () => false,
        },
      };
    },
    pageRoute(pageId, pattern, times) {
      calls.push(['pageRoute', pageId, pattern, times]);
      return 1;
    },
    async frameGoto(target, url, waitUntil, timeout, referer) {
      calls.push(['frameGoto', target, url, waitUntil, timeout, referer]);
      return { response: null };
    },
    async pageReload(pageId, waitUntil, timeout) {
      calls.push(['pageReload', pageId, waitUntil, timeout]);
      return { response: null };
    },
    async closePage(pageId) {
      calls.push(['closePage', pageId]);
    },
    async closeContext(contextId) {
      calls.push(['closeContext', contextId]);
      contexts.delete(contextId);
      connected = false;
    },
    async pageScreenshot(pageId, params) {
      calls.push(['pageScreenshot', pageId, params]);
      return Buffer.from('screenshot');
    },
    publishDisconnect: () => true,
    contextTracingStart(contextId, screenshots, snapshots, sources, name, title) {
      calls.push(['contextTracingStart', contextId, screenshots, snapshots, sources, name, title]);
    },
    async contextTracingStop(contextId, path) {
      calls.push(['contextTracingStop', contextId, path]);
    },
    contextTracingStartChunk(contextId, name, title) {
      calls.push(['contextTracingStartChunk', contextId, name, title]);
    },
    async contextTracingStopChunk(contextId, path) {
      calls.push(['contextTracingStopChunk', contextId, path]);
    },
    contextTracingGroup(contextId, title) {
      calls.push(['contextTracingGroup', contextId, title]);
    },
    contextTracingGroupEnd(contextId) {
      calls.push(['contextTracingGroupEnd', contextId]);
    },
    contextTracingActionBegin(contextId, pageId, action, detail, sourceFile, stackJson) {
      const call = ['contextTracingActionBegin', contextId, pageId, action, detail];
      if (sourceFile !== undefined) call.push(sourceFile);
      calls.push(call);
      if (stackJson !== undefined) stacks.push(JSON.parse(stackJson));
      return 'call@1';
    },
    contextTracingActionEnd(contextId, callId, resultJson) {
      const call = ['contextTracingActionEnd', contextId, callId];
      if (resultJson !== undefined) call.push(resultJson);
      calls.push(call);
    },
    async contextStorageState(contextId, indexedDB) {
      calls.push(['contextStorageState', contextId, indexedDB]);
      return JSON.stringify({ cookies: [], origins: [] });
    },
    async contextSetStorageState(contextId, stateJson) {
      calls.push(['contextSetStorageState', contextId, JSON.parse(stateJson)]);
    },
    async frameWaitForTitle(pageId, frameId, matcher, timeout) {
      calls.push(['frameWaitForTitle', pageId, frameId, timeout]);
      const title = 'Hello from Rust';
      assert.equal(matcher(title), true);
      return title;
    },
    async close() { calls.push(['close']); connected = false; },
  };
  if (profile === 'context-tracing-snapshots' || profile === 'official-conformance-page-context-tracing') {
    session.contextTracingCaptureSnapshot = async (contextId, pageId, callId, phase) => {
      calls.push(['contextTracingCaptureSnapshot', contextId, pageId, callId, phase]);
    };
  }
  class NativeEnvironment {
    async launchChromium(...args) {
      calls.push(['launchChromium', ...args]);
      return session;
    }
    async launchChromiumWithTracesDir(...args) {
      calls.push(['launchChromiumWithTracesDir', ...args]);
      return session;
    }
    async launchPersistentChromium(...args) {
      calls.push(['launchPersistentChromium', ...args]);
      contexts.add('persistent-context');
      return session;
    }
    async launchPersistentChromiumWithContextDefaults(...args) {
      calls.push(['launchPersistentChromiumWithContextDefaults', ...args]);
      contexts.add('persistent-context');
      return session;
    }
    async launchPersistentChromiumWithVideo(...args) {
      calls.push(['launchPersistentChromiumWithVideo', ...args]);
      contexts.add('persistent-context');
      return session;
    }
    async launchPersistentChromiumWithVideoAndContextDefaults(...args) {
      calls.push(['launchPersistentChromiumWithVideoAndContextDefaults', ...args]);
      contexts.add('persistent-context');
      return session;
    }
    async observeBidiStartupFailure(...args) {
      calls.push(['observeBidiStartupFailure', ...args]);
      if (startupFailure) throw startupFailure;
    }
    async connectOverCDP(...args) {
      calls.push(['connectOverCDP', ...args]);
      return session;
    }
  }
  const persistentLocaleProfiles = new Set([
    'persistent-context-locale',
    'persistent-context-timezone',
    'persistent-context-geolocation',
    'persistent-context-permissions',
    'persistent-context-http-credentials',
    'persistent-context-offline',
    'persistent-context-bypass-csp',
    'persistent-context-javascript-disabled',
    'persistent-context-policies',
    'owned-launch-video-recording',
    'official-conformance-browsercontext-cookies',
    'official-conformance-page-video-recording',
  ]);
  if (!persistentLocaleProfiles.has(profile)) {
    delete NativeEnvironment.prototype.launchPersistentChromiumWithContextDefaults;
    delete NativeEnvironment.prototype.launchPersistentChromiumWithVideoAndContextDefaults;
  } else if (profile !== 'owned-launch-video-recording'
      && profile !== 'official-conformance-page-video-recording') {
    delete NativeEnvironment.prototype.launchPersistentChromiumWithVideoAndContextDefaults;
  }
  const artifact = '/test-only/owned-launch.node';
  const module = { exports: {} };
  function require(specifier) {
    if (specifier === 'node:fs') return { existsSync: executableExists };
    if (specifier === './native-artifact.cjs') return {
      nativeArtifact: { path: artifact, profile },
      nativeFeatures: selected => selected === 'owned-launch' ? ['owned-launch']
        : selected === 'persistent-context-locale'
          ? ['owned-launch', 'persistent-context-locale']
        : selected === 'persistent-context-timezone'
          ? ['owned-launch', 'persistent-context-timezone']
        : selected === 'persistent-context-geolocation'
          ? ['owned-launch', 'context-geolocation', 'persistent-context-geolocation']
        : selected === 'persistent-context-permissions'
          ? ['owned-launch', 'context-geolocation-permission', 'persistent-context-geolocation-permission']
        : selected === 'persistent-context-http-credentials'
          ? ['owned-launch', 'context-http-credentials', 'persistent-context-http-credentials']
        : selected === 'persistent-context-offline'
          ? ['owned-launch', 'persistent-context-offline']
        : selected === 'persistent-context-bypass-csp'
          ? ['owned-launch', 'persistent-context-bypass-csp']
        : selected === 'persistent-context-javascript-disabled'
          ? ['owned-launch', 'persistent-context-javascript-disabled']
        : selected === 'persistent-context-policies'
          ? ['owned-launch', 'persistent-context-ignore-https-errors', 'persistent-context-locale', 'persistent-context-timezone', 'context-geolocation', 'persistent-context-geolocation', 'context-geolocation-permission', 'persistent-context-geolocation-permission', 'context-http-credentials', 'persistent-context-http-credentials', 'persistent-context-offline', 'persistent-context-bypass-csp', 'persistent-context-javascript-disabled']
        : selected === 'owned-launch-video-recording'
          ? ['owned-launch', 'persistent-context-ignore-https-errors', 'persistent-context-locale', 'persistent-context-timezone', 'context-geolocation', 'persistent-context-geolocation', 'context-geolocation-permission', 'persistent-context-geolocation-permission', 'context-http-credentials', 'persistent-context-http-credentials', 'persistent-context-offline', 'persistent-context-bypass-csp', 'persistent-context-javascript-disabled', 'browser-registry', 'video-recording']
        : selected === 'playwright-trace' ? ['owned-launch', 'playwright-trace']
          : selected === 'context-tracing-screenshots'
            ? ['owned-launch', 'playwright-trace', 'context-tracing-screenshots']
          : selected === 'context-tracing-chunks'
            ? ['owned-launch', 'playwright-trace', 'context-tracing-screenshots', 'context-tracing-chunks']
          : selected === 'context-tracing-actions'
            ? ['owned-launch', 'playwright-trace', 'context-tracing-screenshots', 'context-tracing-chunks', 'context-tracing-actions']
          : selected === 'context-tracing-snapshots'
            ? ['owned-launch', 'playwright-trace', 'context-tracing-screenshots', 'context-tracing-chunks', 'context-tracing-actions', 'context-tracing-snapshots']
          : selected === 'official-conformance-page-context-tracing'
            ? ['owned-launch', 'playwright-trace', 'context-tracing-screenshots', 'context-tracing-chunks', 'context-tracing-actions', 'context-tracing-snapshots', 'context-tracing-network', 'context-cookies', 'context-storage-state', 'wait-for-url', 'frame-expect-title']
          : selected === 'bidi-startup-failure'
            ? ['owned-launch', 'playwright-trace', 'bidi-startup-failure'] : [],
      verifyNativeArtifact() {},
    };
    if (specifier === artifact) return {
      NativeEnvironment,
      ffmpegExecutablePathSnapshot: () => '/test-only/ffmpeg',
    };
    return sourceRequire(specifier);
  }
  compileFunction(indexSource, ['require', 'module', 'exports', '__dirname'], {
    filename: fileURLToPath(indexUrl),
  })(require, module, module.exports, fileURLToPath(new URL('../src/', import.meta.url)));
  return { api: module.exports, calls, session, stacks };
}

test('fresh Chromium launch delegates explicit options and returns a Browser with no default Context', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium', headless: false, timeout: 42 });
  assert.deepEqual(browser.contexts(), []);
  const context = await browser.newContext();
  await browser.close();

  // Assert
  assert.equal(browser.browserType(), api.chromium);
  assert.equal(context.browser(), browser);
  assert.deepEqual(calls, [
    ['launchChromium', '/test-only/chromium', false, 42],
    ['newContext', undefined],
    ['close'],
  ]);
  assert.equal(api.compatibility.browserOwnership, 'external-and-owned');
});

test('owned launch accepts the Chromium channel that matches its registry executable', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const browser = await api.chromium.launch({
    channel: 'chromium',
    executablePath: '/test-only/chromium',
  });
  await browser.close();

  // Assert
  assert.deepEqual(calls, [
    ['launchChromium', '/test-only/chromium', true, 30_000],
    ['close'],
  ]);
});

test('context tracing screenshot leaf delegates start and resolved ZIP stop paths', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-screenshots');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();

  // Act
  await context.tracing.start({ screenshots: true });
  await context.tracing.stop({ path: 'trace.zip' });

  // Assert
  assert.deepEqual(calls.slice(-2), [
    ['contextTracingStart', 'fresh-context', true, false, false, undefined, undefined],
    ['contextTracingStop', 'fresh-context', resolve('trace.zip')],
  ]);
});

test('context tracing delegates an unbounded start title to Rust', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-screenshots');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const title = 'x'.repeat(1024 * 1024 + 1);

  // Act
  await context.tracing.start({ title });

  // Assert
  assert.deepEqual(calls.at(-1), ['contextTracingStart', 'fresh-context', false, false, false, undefined, title]);
});

test('context tracing delegates Playwright capture options to the Rust lifecycle state', async () => {
  // Arrange
  const { api, calls } = fixture('official-conformance-page-context-tracing');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();

  // Act
  await context.tracing.start({
    screenshots: true,
    snapshots: true,
    sources: true,
    name: 'named-trace',
    title: 'trace title',
  });

  // Assert
  assert.deepEqual(calls.at(-1), [
    'contextTracingStart',
    'fresh-context',
    true,
    true,
    true,
    'named-trace',
    'trace title',
  ]);
});

test('context tracing validates a Playwright trace name before crossing FFI', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-screenshots');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const before = calls.length;

  // Act
  const error = await context.tracing.start({ name: 42 }).catch(value => value);

  // Assert
  assert.match(error.message, /name must be a string/);
  assert.equal(calls.length, before);
});

test('context tracing stop delegates an omitted path for Rust-owned discard semantics', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-screenshots');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();

  // Act
  await context.tracing.start({ screenshots: true });
  await context.tracing.stop();

  // Assert
  assert.deepEqual(calls.slice(-2), [
    ['contextTracingStart', 'fresh-context', true, false, false, undefined, undefined],
    ['contextTracingStop', 'fresh-context', undefined],
  ]);
});

test('context tracing chunks and groups delegate exact Context identity and paths', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-chunks');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();

  // Act
  await context.tracing.start();
  await context.tracing.group('outer');
  await context.tracing.startChunk({ name: 'name2', title: 'second' });
  await context.tracing.groupEnd();
  await context.tracing.stopChunk({ path: 'chunk.zip' });

  // Assert
  assert.deepEqual(calls.slice(-5), [
    ['contextTracingStart', 'fresh-context', false, false, false, undefined, undefined],
    ['contextTracingGroup', 'fresh-context', 'outer'],
    ['contextTracingStartChunk', 'fresh-context', 'name2', 'second'],
    ['contextTracingGroupEnd', 'fresh-context'],
    ['contextTracingStopChunk', 'fresh-context', resolve('chunk.zip')],
  ]);
});

test('context tracing chunk and group titles have no private byte ceiling', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-chunks');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const title = 'x'.repeat(1024 * 1024 + 1);

  // Act
  await context.tracing.start();
  await context.tracing.startChunk({ title });
  await context.tracing.group(title);

  // Assert
  assert.equal(calls.at(-2)[3], title);
  assert.equal(calls.at(-1)[2], title);
});

test('context tracing actions bracket Page calls with Rust action tokens', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-actions');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const page = await context.newPage();
  await context.tracing.start();

  // Act
  await page.route('**/*', () => {});
  await page.goto('https://example.test/empty.html');
  await page.reload();
  await page.close();

  // Assert
  assert.deepEqual(calls.filter(call => call[0].startsWith('contextTracingAction')), [
    ['contextTracingActionBegin', 'fresh-context', 'page-1', 'page-route', undefined],
    ['contextTracingActionEnd', 'fresh-context', 'call@1'],
    ['contextTracingActionBegin', 'fresh-context', 'page-1', 'page-goto', 'https://example.test/empty.html'],
    ['contextTracingActionEnd', 'fresh-context', 'call@1'],
    ['contextTracingActionBegin', 'fresh-context', 'page-1', 'page-reload', undefined],
    ['contextTracingActionEnd', 'fresh-context', 'call@1'],
    ['contextTracingActionBegin', 'fresh-context', 'page-1', 'page-close', undefined],
    ['contextTracingActionEnd', 'fresh-context', 'call@1'],
  ]);
});

test('context tracing sources forward the external callsite only when requested', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-actions');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const page = await context.newPage();
  await context.tracing.start({ sources: true });

  // Act
  await page.goto('https://example.test/empty.html');

  // Assert
  const begin = calls.find(call => call[0] === 'contextTracingActionBegin');
  assert.equal(begin[5], fileURLToPath(import.meta.url));
});

test('context tracing actions forward one external stack frame without enabling sources', async () => {
  // Arrange
  const { api, stacks } = fixture('context-tracing-actions');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const page = await context.newPage();
  await context.tracing.start();

  // Act
  await page.goto('https://example.test/empty.html');

  // Assert
  assert.equal(stacks.length, 1);
  assert.equal(stacks[0].file, fileURLToPath(import.meta.url));
  assert.equal(Number.isInteger(stacks[0].line), true);
  assert.equal(Number.isInteger(stacks[0].column), true);
});

test('Context snapshot leaf captures Page screenshot before and after with a buffer placeholder', async () => {
  // Arrange
  const { api, calls } = fixture('context-tracing-snapshots');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const page = await context.newPage();
  await context.tracing.start({ snapshots: true });

  // Act
  const image = await page.screenshot();

  // Assert
  assert.deepEqual(image, Buffer.from('screenshot'));
  assert.deepEqual(calls.filter(call =>
    call[0].startsWith('contextTracingAction')
      || call[0] === 'contextTracingCaptureSnapshot'
      || call[0] === 'pageScreenshot'), [
    ['contextTracingActionBegin', 'fresh-context', 'page-1', 'page-screenshot', undefined],
    ['contextTracingCaptureSnapshot', 'fresh-context', 'page-1', 'call@1', 'before'],
    ['pageScreenshot', 'page-1', {
      clip: undefined,
      format: 0,
      fullPage: false,
      quality: undefined,
      scale: 0,
      timeoutMs: 30_000,
    }],
    ['contextTracingActionEnd', 'fresh-context', 'call@1', '{"binary":"<Buffer>"}'],
    ['contextTracingCaptureSnapshot', 'fresh-context', 'page-1', 'call@1', 'after'],
  ]);
});

test('official title expectation is traced without snapshot capture around the matcher wait', async () => {
  // Arrange
  const { api, calls } = fixture('official-conformance-page-context-tracing');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const page = await context.newPage();
  await context.tracing.start({ snapshots: true });

  // Act
  const result = await page.mainFrame()._expect('to.have.title', {
    expectedText: [{ string: 'Hello from Rust' }], timeout: 789,
  });

  // Assert
  assert.deepEqual(result, { matches: true, received: { value: 'Hello from Rust' } });
  assert.deepEqual(calls.filter(call =>
    call[0].startsWith('contextTracingAction')
      || call[0] === 'contextTracingCaptureSnapshot'
      || call[0] === 'frameWaitForTitle'), [
    ['contextTracingActionBegin', 'fresh-context', 'page-1', 'frame-expect-title', undefined],
    ['frameWaitForTitle', 'page-1', 'frame-1', 789],
    ['contextTracingActionEnd', 'fresh-context', 'call@1', '{"value":"Hello from Rust"}'],
  ]);
});

test('dialog event waiter resolves before user dismissal without action snapshot capture', async () => {
  // Arrange
  const { api, calls } = fixture('official-conformance-page-context-tracing');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const page = await context.newPage();
  await context.tracing.start({ snapshots: true });
  const dialog = { dismiss() {} };

  // Act
  const pending = page.waitForEvent('dialog');
  page.emit('dialog', dialog);
  const received = await pending;

  // Assert
  assert.equal(received, dialog);
  assert.deepEqual(calls.filter(call =>
    call[0].startsWith('contextTracingAction')
      || call[0] === 'contextTracingCaptureSnapshot'), [
    ['contextTracingActionBegin', 'fresh-context', 'page-1', 'page-wait-for-event', 'dialog'],
    ['contextTracingActionEnd', 'fresh-context', 'call@1'],
  ]);
});

test('Context storage state is a Context-level traced Rust operation', async () => {
  // Arrange
  const { api, calls } = fixture('official-conformance-page-context-tracing');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  await context.tracing.start();

  // Act
  const state = await context.storageState({ indexedDB: true });

  // Assert
  assert.deepEqual(state, { cookies: [], origins: [] });
  assert.deepEqual(calls.slice(-3), [
    ['contextTracingActionBegin', 'fresh-context', undefined, 'browser-context-storage-state', undefined],
    ['contextStorageState', 'fresh-context', true],
    ['contextTracingActionEnd', 'fresh-context', 'call@1'],
  ]);
});

test('Context storage state is restored before newContext publishes the Context', async () => {
  // Arrange
  const { api, calls } = fixture('official-conformance-page-context-tracing');
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const storageState = {
    cookies: [{
      name: 'session', value: 'value', domain: 'example.test', path: '/',
      expires: -1, httpOnly: false, secure: true, sameSite: 'None',
    }],
    origins: [],
  };

  // Act
  const context = await browser.newContext({ storageState });

  // Assert
  assert.equal(context.browser(), browser);
  assert.deepEqual(calls.slice(-2), [
    ['newContext', undefined],
    ['contextSetStorageState', 'fresh-context', storageState],
  ]);
});

test('opt-in BiDi Chromium startup failure uses the explicit executable and inherited defaults', async () => {
  // Arrange
  const { api, calls } = fixture('bidi-startup-failure');
  api._defaultLaunchOptions = {
    channel: 'bidi-chrome-canary',
    tracesDir: '/test-only/traces',
    handleSIGINT: false,
    headless: true,
  };

  // Act
  const launching = api.chromium.launch({ executablePath: '/test-only/exit-one.js' });

  // Assert
  await assert.rejects(launching, {
    message: 'browserType.launch: Failed to launch the browser process.\nBrowser logs:\n[exit=1]',
  });
  assert.deepEqual(calls, [[
    'observeBidiStartupFailure', 'chromium', '/test-only/exit-one.js', 30_000,
    '/test-only/traces', process.cwd(),
  ]]);
});

for (const channel of ['bidi-chromium', 'bidi-chrome-canary', 'bidi-chrome-stable']) {
  test(`BiDi startup failure selects the bounded ${channel} channel`, async () => {
    // Arrange
    const { api, calls } = fixture('bidi-startup-failure');
    api._defaultLaunchOptions = { channel };

    // Act
    const launching = api.chromium.launch({ executablePath: '/test-only/exit-one.js' });

    // Assert
    await assert.rejects(launching, /Browser logs:/);
    assert.equal(calls.length, 1);
    assert.equal(calls[0][1], 'chromium');
  });
}

test('BiDi startup failure remains unavailable in the older owned-launch profile', async () => {
  // Arrange
  const { api, calls } = fixture('owned-launch');
  api._defaultLaunchOptions = { channel: 'bidi-chromium' };

  // Act
  const launching = api.chromium.launch({ executablePath: '/test-only/exit-one.js' });

  // Assert
  await assert.rejects(launching, { code: 'PLAYWRIGHT_RUST_UNSUPPORTED' });
  assert.deepEqual(calls, []);
});

test('BiDi startup route defers missing-executable validation to the Rust boundary', async () => {
  // Arrange
  const { api, calls } = fixture('bidi-startup-failure',
    path => path === '/test-only/owned-launch.node');
  api._defaultLaunchOptions = { channel: 'bidi-chromium', tracesDir: '/test-only/traces' };

  // Act
  const launching = api.chromium.launch({ executablePath: '/test-only/missing-browser' });

  // Assert
  await assert.rejects(launching, /Browser logs:/);
  assert.deepEqual(calls, [[
    'observeBidiStartupFailure', 'chromium', '/test-only/missing-browser', 30_000,
    '/test-only/traces', process.cwd(),
  ]]);
});

test('BiDi startup failure refuses unknown channels and inherited-only executables', async () => {
  // Arrange
  const unknown = fixture('bidi-startup-failure');
  unknown.api._defaultLaunchOptions = { channel: 'chrome' };
  const inherited = fixture('bidi-startup-failure');
  inherited.api._defaultLaunchOptions = {
    channel: 'bidi-chromium', executablePath: '/test-only/exit-one.js',
  };

  // Act
  const unknownLaunch = unknown.api.chromium.launch({ executablePath: '/test-only/exit-one.js' });
  const inheritedLaunch = inherited.api.chromium.launch();

  // Assert
  await assert.rejects(unknownLaunch, { code: 'PLAYWRIGHT_RUST_UNSUPPORTED' });
  await assert.rejects(inheritedLaunch, { code: 'PLAYWRIGHT_RUST_UNSUPPORTED' });
  assert.deepEqual(unknown.calls, []);
  assert.deepEqual(inherited.calls, []);
});

test('BiDi startup route never returns a Browser when its native observer resolves', async () => {
  // Arrange
  const { api, calls } = fixture('bidi-startup-failure', () => true, null);
  api._defaultLaunchOptions = { channel: 'bidi-chromium' };

  // Act
  const launching = api.chromium.launch({ executablePath: '/test-only/exit-one.js' });

  // Assert
  await assert.rejects(launching, { code: 'PLAYWRIGHT_RUST_UNSUPPORTED' });
  assert.equal(calls.length, 1);
});

test('fresh Chromium launch uses inherited worker defaults when explicit options are absent', async () => {
  // Arrange
  const { api, calls } = fixture();
  api._defaultLaunchOptions = {
    executablePath: '/test-only/chromium',
    headless: false,
    timeout: 42,
  };

  // Act
  const browser = await api.chromium.launch();
  await browser.close();

  // Assert
  assert.deepEqual(calls, [
    ['launchChromium', '/test-only/chromium', false, 42],
    ['close'],
  ]);
});

test('explicit launch options override inherited defaults after reading each default getter once', async () => {
  // Arrange
  const { api, calls } = fixture();
  let headlessReads = 0;
  api._defaultLaunchOptions = {
    executablePath: '/test-only/default-chromium',
    get headless() { headlessReads++; return false; },
    timeout: 100,
  };

  // Act
  const browser = await api.chromium.launch({
    executablePath: '/test-only/explicit-chromium',
    headless: true,
    timeout: 17,
  });
  await browser.close();

  // Assert
  assert.equal(headlessReads, 1);
  assert.deepEqual(calls, [
    ['launchChromium', '/test-only/explicit-chromium', true, 17],
    ['close'],
  ]);
});

test('an explicit port rejection precedes invalid inherited launch arguments', async () => {
  // Arrange
  const { api, calls } = fixture();
  api._defaultLaunchOptions = { args: ['http://example.test/default-page'] };

  // Act
  const launching = api.chromium.launch({ port: 1234 });

  // Assert
  await assert.rejects(launching, { message: 'Cannot specify a port without launching as a server.' });
  assert.deepEqual(calls, []);
});

test('inherited launch arguments reach the same pre-spawn validation as explicit arguments', async () => {
  // Arrange
  const { api, calls } = fixture();
  api._defaultLaunchOptions = {
    executablePath: '/test-only/chromium',
    args: ['http://example.test/default-page'],
  };

  // Act
  const launching = api.chromium.launch();

  // Assert
  await assert.rejects(launching, { message: 'Arguments can not specify page to be opened' });
  assert.deepEqual(calls, []);
});

test('unsupported inherited options remain explicit failures after the merge', async () => {
  // Arrange
  const { api, calls } = fixture();
  api._defaultLaunchOptions = {
    executablePath: '/test-only/chromium',
    handleSIGINT: false,
    tracesDir: '/test-only/traces',
  };

  // Act
  const launching = api.chromium.launch();

  // Assert
  await assert.rejects(launching, { code: 'PLAYWRIGHT_RUST_UNSUPPORTED', message: 'browserType.launch option tracesDir is not supported by the current Playwright Rust native profile' });
  assert.deepEqual(calls, []);
});

test('opt-in trace path preserves inherited relative tracesDir for the Rust launch boundary', async () => {
  // Arrange
  const { api, calls } = fixture('playwright-trace');
  api._defaultLaunchOptions = {
    executablePath: '/test-only/chromium',
    tracesDir: 'relative/traces',
  };

  // Act
  const browser = await api.chromium.launch();
  await browser.close();

  // Assert
  assert.deepEqual(calls, [
    ['launchChromiumWithTracesDir', '/test-only/chromium', true, 30_000, 'relative/traces', process.cwd()],
    ['close'],
  ]);
});

test('opt-in trace path validates type before entering the Rust launch boundary', async () => {
  // Arrange
  const { api, calls } = fixture('playwright-trace');

  // Act
  const launching = api.chromium.launch({ executablePath: '/test-only/chromium', tracesDir: 42 });

  // Assert
  await assert.rejects(launching, { name: 'TypeError', message: 'browserType.launch: tracesDir must be a string' });
  assert.deepEqual(calls, []);
});

test('empty tracesDir reaches the selected Rust method without inventing a caller path', async () => {
  // Arrange
  const { api, calls } = fixture('playwright-trace');

  // Act
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium', tracesDir: '' });
  await browser.close();

  // Assert
  assert.deepEqual(calls, [
    ['launchChromiumWithTracesDir', '/test-only/chromium', true, 30_000, '', process.cwd()],
    ['close'],
  ]);
});

test('explicit handleSIGINT false selects the existing no-handler launch policy', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium', handleSIGINT: false });
  await browser.close();

  // Assert
  assert.deepEqual(calls, [
    ['launchChromium', '/test-only/chromium', true, 30_000],
    ['close'],
  ]);
});

test('explicit handleSIGINT false overrides inherited true before launch', async () => {
  // Arrange
  const { api, calls } = fixture();
  api._defaultLaunchOptions = { executablePath: '/test-only/chromium', handleSIGINT: true };

  // Act
  const browser = await api.chromium.launch({ handleSIGINT: false });
  await browser.close();

  // Assert
  assert.deepEqual(calls, [
    ['launchChromium', '/test-only/chromium', true, 30_000],
    ['close'],
  ]);
});

test('handleSIGINT true stays unsupported before acquiring the native environment', async () => {
  // Arrange
  const { api, calls } = fixture();
  api._defaultLaunchOptions = { executablePath: '/test-only/chromium', handleSIGINT: true };

  // Act
  const launching = api.chromium.launch();

  // Assert
  await assert.rejects(launching, {
    code: 'PLAYWRIGHT_RUST_UNSUPPORTED',
    message: 'browserType.launch option handleSIGINT=true is not supported by the current Playwright Rust native profile',
  });
  assert.deepEqual(calls, []);
});

test('nonboolean handleSIGINT fails preflight before acquiring the native environment', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const launching = api.chromium.launch({ executablePath: '/test-only/chromium', handleSIGINT: 'false' });

  // Assert
  await assert.rejects(launching, { name: 'TypeError', message: 'browserType.launch: handleSIGINT must be a boolean' });
  assert.deepEqual(calls, []);
});

test('inherited launch options never invent a missing browser executable', async () => {
  // Arrange
  const { api, calls } = fixture();
  api._defaultLaunchOptions = { headless: false };

  // Act
  const launching = api.chromium.launch();

  // Assert
  await assert.rejects(launching, { name: 'TypeError', message: 'browserType.launch: executablePath must be a nonempty absolute path' });
  assert.deepEqual(calls, []);
});

test('fresh-owned Browser.close emits one Context close after native shutdown, including repeated calls', async () => {
  // Arrange
  const { api, calls, session } = fixture();
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const events = [];
  const order = [];
  context.on('close', function (closed) {
    events.push([this, closed, browser._contexts.has(context._id)]);
    order.push('context');
  });
  browser.on('disconnected', () => order.push('browser'));
  context._request = { _disposeFromOwner(reason) { calls.push(['disposeRequest', reason]); } };
  const nativeClose = session.close;
  let nativeCloses = 0;
  session.close = async () => {
    if (nativeCloses++ === 0) assert.deepEqual(events, []);
    await nativeClose();
  };

  // Act
  await Promise.all([browser.close(), browser.close()]);
  await browser.close();

  // Assert
  assert.deepEqual(events, [[context, context, false]]);
  assert.deepEqual(order, ['context', 'browser']);
  assert.deepEqual(calls.filter(([kind]) => kind === 'disposeRequest'), [['disposeRequest', 'Browser closed']]);
  assert.equal(browser.isConnected(), false);
});

test('fresh-owned Browser.close accepts an optional reason and rejects non-strings before shutdown', async () => {
  // Arrange
  const accepted = fixture();
  const rejected = fixture();
  const browser = await accepted.api.chromium.launch({ executablePath: '/test-only/chromium' });
  const invalid = await rejected.api.chromium.launch({ executablePath: '/test-only/chromium' });

  // Act
  await browser.close({ reason: 'Test ended.' });
  const rejection = invalid.close({ reason: 42 });

  // Assert
  assert.deepEqual(accepted.calls.at(-1), ['close']);
  await assert.rejects(rejection, {
    name: 'TypeError',
    message: 'browser.close: reason must be a string',
  });
  assert.ok(!rejected.calls.some(call => call[0] === 'close'));
});

test('fresh-owned Browser.close does not repeat a native Context close notice', async () => {
  // Arrange
  const { api, calls, session } = fixture();
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const events = [];
  context.on('close', closed => events.push(closed));
  context._request = { _disposeFromOwner(reason) { calls.push(['disposeRequest', reason]); } };
  const nativeClose = session.close;
  session.close = async () => {
    const commit = session.lifecycleCallback(1, context._id, '');
    commit();
    await nativeClose();
  };

  // Act
  await browser.close();

  // Assert
  assert.deepEqual(events, [context]);
  assert.deepEqual(calls.filter(([kind]) => kind === 'disposeRequest'), [['disposeRequest', 'Browser closed']]);
});

test('fresh-owned disconnect watcher emits Context close before Browser disconnected during shutdown', async () => {
  // Arrange
  const { api, session } = fixture();
  let releaseSignal;
  session.sessionSignal = () => ({ wait: () => new Promise(resolve => { releaseSignal = resolve; }) });
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  await Promise.resolve();
  const context = await browser.newContext();
  const events = [];
  context.on('close', () => events.push('context'));
  browser.on('disconnected', () => events.push('browser'));
  const nativeClose = session.close;
  session.close = async () => {
    releaseSignal(false);
    await Promise.resolve();
    await nativeClose();
  };

  // Act
  await browser.close();

  // Assert
  assert.deepEqual(events, ['context', 'browser']);
});

test('fresh-owned Context listener exception preserves shutdown and reaches the native exception channel', async () => {
  // Arrange
  const { api, session } = fixture();
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const listenerError = new Error('listener failed');
  const reported = [];
  const events = [];
  session.reportCallbackException = error => reported.push(error);
  context.on('close', () => { throw listenerError; });
  browser.on('disconnected', () => events.push('disconnected'));

  // Act
  await browser.close();

  // Assert
  assert.deepEqual(reported, [listenerError]);
  assert.deepEqual(events, ['disconnected']);
  assert.equal(browser.isConnected(), false);
});

test('failed fresh-owned shutdown does not fabricate a Context close event', async () => {
  // Arrange
  const { api, session } = fixture();
  const browser = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  const context = await browser.newContext();
  const events = [];
  context.on('close', closed => events.push(closed));
  session.close = async () => { throw new Error('native shutdown failed'); };

  // Act
  const closing = browser.close();

  // Assert
  await assert.rejects(closing, /native shutdown failed/);
  assert.deepEqual(events, []);
  assert.equal(browser._contexts.get(context._id), context);
});

for (const profile of ['default', 'owned-launch']) {
  test(`${profile} CDP attachment does not synthesize Context close on Browser.close`, async () => {
    // Arrange
    const { api } = fixture(profile);
    const browser = await api.chromium.connectOverCDP('http://example.invalid:9222');
    const context = await browser.newContext();
    const events = [];
    context.on('close', closed => events.push(closed));

    // Act
    await browser.close();

    // Assert
    assert.deepEqual(events, []);
  });
}

test('opt-in profile preserves CDP attach and launch defaults', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const owned = await api.chromium.launch({ executablePath: '/test-only/chromium' });
  await owned.close();
  const attached = await api.chromium.connectOverCDP('http://example.invalid:9222');
  await attached.close();

  // Assert
  assert.deepEqual(calls, [
    ['launchChromium', '/test-only/chromium', true, 30_000],
    ['close'],
    ['connectOverCDP', 'http://example.invalid:9222', 30_000],
    ['close'],
  ]);
});

test('owned-launch profile does not enter a browser for invalid persistent-context arguments', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const launch = api.chromium.launchPersistentContext('unused-profile', {
    args: ['http://example.test/page'],
  });

  // Assert
  await assert.rejects(launch, {
    message: 'browserType.launchPersistentContext: Arguments can not specify page to be opened',
  });
  assert.deepEqual(calls, []);
});

test('owned-launch profile delegates a valid persistent-context launch without video', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const context = await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    headless: false,
    timeout: 42,
    baseURL: 'https://example.test/root/',
  });

  // Assert
  assert.equal(context.browser()?.browserType(), api.chromium);
  assert.deepEqual(calls, [[
    'launchPersistentChromium',
    '/test-only/chromium',
    resolve('persistent-profile'),
    42,
    'https://example.test/root/',
    0,
  ]]);
});

test('persistent locale uses one Rust-owned default-Context launch port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-locale');
  const locale = 'x'.repeat(4096);

  // Act
  const context = await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    headless: false,
    timeout: 42,
    baseURL: 'https://example.test/root/',
    locale,
  });

  // Assert
  assert.equal(context.browser()?.browserType(), api.chromium);
  assert.deepEqual(calls, [[
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    42,
    'https://example.test/root/',
    0,
    { locale, timezoneId: undefined, geolocation: undefined, permissions: undefined, offline: false, bypassCsp: false, javaScriptEnabled: undefined },
  ]]);
});

test('empty persistent locale remains a no-op without selecting the locale port', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    locale: '',
  });

  // Assert
  assert.equal(calls[0][0], 'launchPersistentChromium');
});

test('nonempty persistent locale fails closed when its native leaf is absent', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  const launch = api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    locale: 'fr-FR',
  });

  // Assert
  await assert.rejects(launch, {
    message: 'browserType.launchPersistentContext option locale is not supported by the current Playwright Rust native profile',
  });
  assert.deepEqual(calls, []);
});

test('persistent timezone preserves arbitrary strings in the typed defaults port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-timezone');
  const timezoneId = 'x'.repeat(4096);

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    timezoneId,
  });

  // Assert
  assert.deepEqual(calls.at(0), [
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    1,
    { locale: undefined, timezoneId, geolocation: undefined, permissions: undefined, offline: false, bypassCsp: false, javaScriptEnabled: undefined },
  ]);
});

test('empty persistent timezone is a no-op and a selected value fails closed without its leaf', async () => {
  // Arrange
  const empty = fixture();
  const missing = fixture();

  // Act
  await empty.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    timezoneId: '',
  });
  const launch = missing.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    timezoneId: 'America/Jamaica',
  });

  // Assert
  assert.equal(empty.calls.at(0)[0], 'launchPersistentChromium');
  await assert.rejects(launch, /option timezoneId is not supported/);
  assert.deepEqual(missing.calls, []);
});

test('persistent geolocation uses the shared typed defaults launch port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-geolocation');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    geolocation: { longitude: 10, latitude: 20 },
  });

  // Assert
  assert.deepEqual(calls.at(0), [
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    1,
    {
      locale: undefined,
      timezoneId: undefined,
      geolocation: { longitude: 10, latitude: 20, accuracy: 0 },
      permissions: undefined,
      offline: false,
      bypassCsp: false,
      javaScriptEnabled: undefined,
    },
  ]);
});

test('persistent geolocation rejects invalid input and a missing native leaf before launch', async () => {
  // Arrange
  const invalid = fixture('persistent-context-geolocation');
  const missing = fixture();

  // Act
  const invalidLaunch = invalid.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    geolocation: { longitude: '10', latitude: 20 },
  });
  await assert.rejects(invalidLaunch, /geolocation.longitude must be a number/);
  const missingLaunch = missing.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    geolocation: { longitude: 10, latitude: 20 },
  });

  // Assert
  await assert.rejects(missingLaunch, /option geolocation is not supported/);
  assert.deepEqual(invalid.calls, []);
  assert.deepEqual(missing.calls, []);
});

test('persistent permissions use the shared typed defaults launch port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-permissions');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    permissions: ['geolocation', 'camera'],
  });

  // Assert
  assert.deepEqual(calls.at(-1), [
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    1,
    {
      locale: undefined,
      timezoneId: undefined,
      geolocation: undefined,
      permissions: ['geolocation', 'camera'],
      offline: false,
      bypassCsp: false,
      javaScriptEnabled: undefined,
    },
  ]);
});

test('persistent permissions reject invalid input and a missing native leaf before launch', async () => {
  // Arrange
  const invalid = fixture('persistent-context-permissions');
  const missing = fixture();

  // Act
  const invalidLaunch = invalid.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    permissions: ['camera', 42],
  });
  await assert.rejects(invalidLaunch, /permissions\[1\] must be a string/);
  const missingLaunch = missing.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    permissions: ['camera'],
  });

  // Assert
  await assert.rejects(missingLaunch, /option permissions is not supported/);
  assert.deepEqual(invalid.calls, []);
  assert.deepEqual(missing.calls, []);
});

test('persistent HTTP credentials use the shared typed defaults launch port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-http-credentials');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    httpCredentials: {
      username: 'user', password: 'pass', origin: 'https://example.test', send: 'always',
    },
  });

  // Assert
  assert.deepEqual(calls.at(-1), [
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    1,
    {
      locale: undefined,
      timezoneId: undefined,
      geolocation: undefined,
      permissions: undefined,
      offline: false,
      bypassCsp: false,
      javaScriptEnabled: undefined,
      httpCredentials: {
        username: 'user', password: 'pass', origin: 'https://example.test', sendAlways: true,
      },
    },
  ]);
});

test('persistent HTTP credentials reject invalid input and a missing native leaf before launch', async () => {
  // Arrange
  const invalid = fixture('persistent-context-http-credentials');
  const missing = fixture();

  // Act
  const invalidLaunch = invalid.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    httpCredentials: { username: 'user', password: 'pass', send: 'eager' },
  });
  const missingLaunch = missing.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    httpCredentials: { username: 'user', password: 'pass' },
  });

  // Assert
  await assert.rejects(invalidLaunch, /send must be "unauthorized" or "always"/);
  await assert.rejects(missingLaunch, /option httpCredentials is not supported/);
  assert.deepEqual(invalid.calls, []);
  assert.deepEqual(missing.calls, []);
});

test('persistent offline uses the shared typed defaults launch port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-offline');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    offline: true,
  });

  // Assert
  assert.deepEqual(calls, [[
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    1,
    { locale: undefined, timezoneId: undefined, geolocation: undefined, permissions: undefined, offline: true, bypassCsp: false, javaScriptEnabled: undefined },
  ]]);
});

test('false persistent offline remains a no-op and true fails closed without its leaf', async () => {
  // Arrange
  const disabled = fixture();
  const missing = fixture();

  // Act
  await disabled.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    offline: false,
  });
  const launch = missing.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    offline: true,
  });

  // Assert
  assert.equal(disabled.calls[0][0], 'launchPersistentChromium');
  await assert.rejects(launch, {
    message: 'browserType.launchPersistentContext option offline is not supported by the current Playwright Rust native profile',
  });
  assert.deepEqual(missing.calls, []);
});

test('persistent CSP bypass uses the shared typed defaults launch port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-bypass-csp');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    bypassCSP: true,
  });

  // Assert
  assert.deepEqual(calls.at(0), [
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    1,
    { locale: undefined, timezoneId: undefined, geolocation: undefined, permissions: undefined, offline: false, bypassCsp: true, javaScriptEnabled: undefined },
  ]);
});

test('persistent JavaScript disablement uses the shared typed defaults launch port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-javascript-disabled');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    javaScriptEnabled: false,
  });

  // Assert
  assert.deepEqual(calls.at(0), [
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    1,
    { locale: undefined, timezoneId: undefined, geolocation: undefined, permissions: undefined, offline: false, bypassCsp: false, javaScriptEnabled: false },
  ]);
});

test('default CSP and JavaScript values do not select persistent policy leaves', async () => {
  // Arrange
  const { api, calls } = fixture();

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    bypassCSP: false,
    javaScriptEnabled: true,
  });

  // Assert
  assert.equal(calls.at(0)[0], 'launchPersistentChromium');
});

test('persistent CSP and JavaScript policies fail closed without their leaves', async () => {
  // Arrange
  const csp = fixture();
  const javascript = fixture();

  // Act
  const cspLaunch = csp.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    bypassCSP: true,
  });
  const javascriptLaunch = javascript.api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    javaScriptEnabled: false,
  });

  // Assert
  await assert.rejects(cspLaunch, /option bypassCSP is not supported/);
  await assert.rejects(javascriptLaunch, /option javaScriptEnabled is not supported/);
  assert.deepEqual(csp.calls, []);
  assert.deepEqual(javascript.calls, []);
});

test('persistent policies compose atomically through one typed defaults port', async () => {
  // Arrange
  const { api, calls } = fixture('persistent-context-policies');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    ignoreHTTPSErrors: true,
    locale: 'fr-FR',
    timezoneId: 'x'.repeat(4096),
    geolocation: { longitude: 10, latitude: 20, accuracy: 3 },
    permissions: ['geolocation', 'camera'],
    httpCredentials: { username: 'user', password: 'pass', send: 'always' },
    offline: true,
    bypassCSP: true,
    javaScriptEnabled: false,
  });

  // Assert
  assert.deepEqual(calls, [[
    'launchPersistentChromiumWithContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    30_000,
    undefined,
    3,
    { locale: 'fr-FR', timezoneId: 'x'.repeat(4096), geolocation: { longitude: 10, latitude: 20, accuracy: 3 }, permissions: ['geolocation', 'camera'], offline: true, bypassCsp: true, javaScriptEnabled: false, httpCredentials: { username: 'user', password: 'pass', origin: undefined, sendAlways: true } },
  ]]);
});

test('owned video launch carries persistent HTTPS policy through the Rust port', async () => {
  // Arrange
  const { api, calls } = fixture('owned-launch-video-recording');

  // Act
  const context = await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    headless: false,
    timeout: 42,
    baseURL: 'https://example.test/root/',
    ignoreHTTPSErrors: true,
    recordVideo: { dir: 'videos', size: { width: 640, height: 360 } },
  });

  // Assert
  assert.equal(context.browser()?.browserType(), api.chromium);
  assert.deepEqual(calls, [[
    'launchPersistentChromiumWithVideo',
    '/test-only/chromium',
    resolve('persistent-profile'),
    2,
    42,
    'https://example.test/root/',
    resolve('videos'),
    '/test-only/ffmpeg',
    640,
    360,
  ]]);
});

test('owned video launch composes persistent locale in the same Rust port', async () => {
  // Arrange
  const { api, calls } = fixture('owned-launch-video-recording');

  // Act
  await api.chromium.launchPersistentContext('persistent-profile', {
    executablePath: '/test-only/chromium',
    locale: 'fr-FR',
    recordVideo: { dir: 'videos', size: { width: 640, height: 360 } },
  });

  // Assert
  assert.deepEqual(calls, [[
    'launchPersistentChromiumWithVideoAndContextDefaults',
    '/test-only/chromium',
    resolve('persistent-profile'),
    1,
    30_000,
    undefined,
    { locale: 'fr-FR', timezoneId: undefined, geolocation: undefined, permissions: undefined, offline: false, bypassCsp: false, javaScriptEnabled: undefined },
    resolve('videos'),
    '/test-only/ffmpeg',
    640,
    360,
  ]]);
});

test('launch rejects unsupported options and invalid values before native acquisition', async () => {
  // Arrange
  const { api, calls } = fixture();
  const invalid = [
    api.chromium.launch({ executablePath: '/test-only/chromium', args: ['--unsupported'] }),
    api.chromium.launch({ executablePath: 'relative/chromium' }),
    api.chromium.launch({ executablePath: '/test-only/chromium', headless: 'yes' }),
    api.chromium.launch({ executablePath: '/test-only/chromium', timeout: -1 }),
  ];

  // Act
  const results = await Promise.allSettled(invalid);

  // Assert
  assert.ok(results.every(result => result.status === 'rejected'));
  assert.deepEqual(results.map(result => result.reason?.code ?? result.reason?.name), [
    'PLAYWRIGHT_RUST_UNSUPPORTED',
    'TypeError',
    'TypeError',
    'TypeError',
  ]);
  assert.deepEqual(calls, []);
});

test('launch rejects a nonexistent relative executable before native acquisition', async () => {
  // Arrange
  const { api, calls } = fixture('owned-launch', path => path !== 'random-invalid-path');

  // Act
  const result = api.chromium.launch({ executablePath: 'random-invalid-path' });

  // Assert
  await assert.rejects(result, error => error.constructor === Error &&
    error.message === "browserType.launch: Failed to launch chromium because executable doesn't exist at random-invalid-path");
  assert.deepEqual(calls, []);
});

test('launch rejects a nonexistent absolute executable before native acquisition', async () => {
  // Arrange
  const { api, calls } = fixture('owned-launch', path => path !== '/test-only/missing-chromium');

  // Act
  const result = api.chromium.launch({ executablePath: '/test-only/missing-chromium' });

  // Assert
  await assert.rejects(result, error => error.constructor === Error &&
    error.message === "browserType.launch: Failed to launch chromium because executable doesn't exist at /test-only/missing-chromium");
  assert.deepEqual(calls, []);
});

test('default profile keeps launch unsupported before loading the addon', async () => {
  // Arrange
  const { api, calls } = fixture('default');

  // Act
  const result = api.chromium.launch({ executablePath: '/test-only/chromium' });

  // Assert
  await assert.rejects(result, { code: 'PLAYWRIGHT_RUST_UNSUPPORTED' });
  assert.deepEqual(calls, []);
  assert.equal(api.compatibility.browserOwnership, 'external');
});
