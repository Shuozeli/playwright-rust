'use strict';

const { EventEmitter } = require('node:events');
const NativeConsoleEventEmitter = require('./console-subscriptions.cjs');
const FrameCache = require('./frame-cache.cjs');
const lifecycleEvents = require('./lifecycle-events.cjs');
const { existsSync } = require('node:fs');
const { readFile, writeFile } = require('node:fs/promises');
const { isAbsolute, resolve, sep } = require('node:path');
const { fileURLToPath } = require('node:url');
const { nativeArtifact, nativeFeatures, verifyNativeArtifact } = require('./native-artifact.cjs');
const cdpHandshakeHeadersSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('cdp-handshake-headers');
const cdpDefaultUserAgentSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('cdp-default-user-agent');
const ownedLaunchSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('owned-launch');
const bidiStartupFailureSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('bidi-startup-failure');
const playwrightTraceDirSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('playwright-trace');
const contextTracingScreenshotsSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('context-tracing-screenshots');
const contextTracingChunksSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('context-tracing-chunks');
const contextTracingActionsSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('context-tracing-actions');
const contextTracingSnapshotsSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('context-tracing-snapshots');
const contextTracingNetworkSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('context-tracing-network');
const contextStorageStateSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('context-storage-state');
const persistentContextPreflightSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-preflight');
const persistentContextLocaleSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-locale');
const persistentContextTimezoneSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-timezone');
const persistentContextGeolocationSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-geolocation');
const persistentContextPermissionsSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-geolocation-permission');
const persistentContextHttpCredentialsSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-http-credentials');
const persistentContextOfflineSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-offline');
const persistentContextBypassCspSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-bypass-csp');
const persistentContextJavascriptDisabledSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('persistent-context-javascript-disabled');
const serverConnectWsErrorSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('playwright-server-connect-ws-error');
const serverConnectCloseErrorSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('playwright-server-connect-close-error');
const serverConnectHeadersSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('playwright-server-connect-headers');
const serverConnectSessionSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('playwright-server-connect-session');
const serverConnectPageEventsSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('playwright-server-connect-page-events');
const frameEventsSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('frame-events');
const frameUrlExpectationSelected = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('wait-for-url');
const browserRegistrySnapshot = typeof nativeFeatures === 'function'
  && nativeFeatures(nativeArtifact.profile).includes('browser-registry')
  ? require('./browser-registry-snapshot.cjs')()
  : undefined;
const { setImmediate: scheduleConversion } = require('node:timers');
const fileInputDebug = require('./file-input-debug.cjs');
const arrayBufferMaximum = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, 'maxByteLength')?.get;

class TimeoutError extends Error {
  constructor(message) { super(message); this.name = 'TimeoutError'; }
}

class ValidationError extends Error {}

class UnsupportedOperationError extends Error {
  constructor(api) {
    super(`${api} is not supported by the current Playwright Rust native profile`);
    this.name = 'UnsupportedOperationError';
    this.code = 'PLAYWRIGHT_RUST_UNSUPPORTED';
  }
}

const contextDevice = require('./context-device.cjs')({ UnsupportedOperationError });
const contextNullViewport = require('./context-null-viewport.cjs')({ UnsupportedOperationError });
const harRecording = require('./har-recording.cjs')({ UnsupportedOperationError });
const videoRecording = require('./video-recording.cjs')({
  UnsupportedOperationError,
  mapError,
  resolvePath: resolve,
  ffmpegPath: () => {
    if (!browserRegistrySnapshot)
      throw new UnsupportedOperationError('recordVideo (select browser-registry native profile)');
    return nativeAddon().ffmpegExecutablePathSnapshot(
      resolve(__dirname, '..'),
      browserRegistrySnapshot,
    );
  },
});

const compatibility = Object.freeze({
  oracle: 'playwright@1.58.2',
  profile: 'chromium-attach-navigation-read-evaluation-locator-actions',
  browserOwnership: ownedLaunchSelected ? 'external-and-owned' : 'external',
  apiCoverage: 'experimental-subset',
  nativeProfile: nativeArtifact.profile,
});

const bidiStartupChannels = Object.freeze({
  chromium: Object.freeze(['bidi-chromium', 'bidi-chrome-canary', 'bidi-chrome-stable']),
});

let defaultLaunchOptions;

let addon;
let nativeEnvironment;
function nativeAddon() {
  if (addon) return addon;
  const artifact = nativeArtifact.path;
  if (!existsSync(artifact)) {
    throw new Error(`Playwright Rust native artifact is missing (${process.platform}-${process.arch}, profile=${nativeArtifact.profile}). Run PLAYWRIGHT_RUST_NATIVE_PROFILE=${nativeArtifact.profile} pnpm build in packages/playwright-rust; no browser or binary is downloaded at import time.`);
  }
  const loaded = require(artifact);
  verifyNativeArtifact(loaded, nativeArtifact);
  addon = loaded;
  return addon;
}

function getNativeEnvironment() {
  const { NativeEnvironment } = nativeAddon();
  nativeEnvironment ??= new NativeEnvironment(scheduleConversion, arrayBufferMaximum);
  return nativeEnvironment;
}

function mapError(error, api) {
  if (error instanceof UnsupportedOperationError) return error;
  const match = /^(PLAYWRIGHT_RUST_[A-Z_]+): ([\s\S]*)$/.exec(error.message);
  if (!match) return error;
  if (match[1] === 'PLAYWRIGHT_RUST_UNSUPPORTED') return new UnsupportedOperationError(`${api}: ${match[2]}`);
  let message = match[2];
  if (message.includes('CDP error -32000: Inspected target navigated or closed'))
    message = 'Execution context was destroyed, most likely because of a navigation.';
  else if (message.includes('CDP error -32000: Execution context was destroyed'))
    message = 'Execution context was destroyed.';
  else if (message === 'evaluation depth limit exceeded')
    message = 'Cannot serialize result: object reference chain is too long.';
  else if (message === 'JSHandle is disposed')
    message = 'no object with guid: JSHandle is disposed';
  else if (/^ProtocolViolation: Handle `[^`]+` belongs to a different execution world$/.test(message))
    message = 'JSHandles can be evaluated only in the context they were created!';
  if (match[1] === 'PLAYWRIGHT_RUST_TIMEOUT' && /^Timeout \d+ms exceeded$/.test(message))
    message += '.';
  const mapped = match[1] === 'PLAYWRIGHT_RUST_TIMEOUT'
    ? new TimeoutError(`${api}: ${message}`)
    : match[1] === 'PLAYWRIGHT_RUST_INVALID_ARGUMENT'
      ? new TypeError(`${api}: ${message}`)
      : new Error(`${api}: ${message}`);
  mapped.code = match[1];
  return mapped;
}

function tracingCallsite() {
  const ownSourceFile = typeof __filename === 'string' ? __filename : undefined;
  const ownSourceDirectory = typeof __dirname === 'string' ? resolve(__dirname) : undefined;
  const captured = {};
  if (typeof Error.captureStackTrace === 'function')
    Error.captureStackTrace(captured, runTracingAction);
  else
    captured.stack = new Error().stack;
  for (const line of String(captured.stack ?? '').split('\n').slice(1)) {
    const match = line.match(/\(?((?:file:\/\/\/|\/)[^()\n]+):(\d+):(\d+)\)?$/);
    if (!match) continue;
    let file = match[1];
    if (file.startsWith('file://')) {
      try { file = fileURLToPath(file); }
      catch { continue; }
    }
    const isOwnSource = file === ownSourceFile
      || (ownSourceDirectory !== undefined
        && (file === ownSourceDirectory || file.startsWith(`${ownSourceDirectory}${sep}`)))
      || /[\\/]src[\\/][^\\/]+\.cjs$/.test(file);
    const isPlaywrightInternal = /[\\/]packages[\\/](?:playwright|playwright-core)[\\/](?:lib|src)[\\/]/.test(file)
      || /[\\/]node_modules[\\/](?:playwright|playwright-core)[\\/](?:lib|src)[\\/]/.test(file);
    const isRuntimeInternal = file.startsWith('/async_local_storage/')
      || file.startsWith('/internal/');
    if (!isOwnSource && !isPlaywrightInternal && !isRuntimeInternal && isAbsolute(file)) {
      const prefix = line.slice(0, match.index).replace(/^\s*at\s*/, '').replace(/\s*\($/, '');
      return {
        file,
        line: Number(match[2]),
        column: Number(match[3]),
        function: prefix,
      };
    }
  }
  return undefined;
}

async function runTracingAction(owner, action, detail, operation) {
  const context = owner?._context ?? owner;
  const pageId = owner?._context ? owner._id : undefined;
  const snapshotsEligible = !action.startsWith('dialog-')
    && action !== 'frame-expect-title'
    && !(action === 'page-wait-for-event' && detail === 'dialog');
  const session = context?._browser?._session;
  if (!contextTracingActionsSelected || !session?.contextTracingActionBegin
      || !session?.contextTracingActionEnd)
    return operation();
  let callId;
  try {
    const callsite = tracingCallsite();
    const sourceFile = context._traceSources ? callsite?.file : undefined;
    callId = session.contextTracingActionBegin(
      context._id,
      pageId,
      action,
      detail,
      sourceFile,
      callsite === undefined ? undefined : JSON.stringify(callsite),
    );
    if (snapshotsEligible && callId !== undefined && callId !== null && pageId !== undefined
        && typeof session.contextTracingCaptureSnapshot === 'function') {
      try { await session.contextTracingCaptureSnapshot(context._id, pageId, callId, 'before'); }
      catch {}
    }
  } catch (error) {
    throw mapError(error, 'browserContext.tracing');
  }
  try {
    const result = await operation();
    if (callId !== undefined && callId !== null) {
      const resultType = typeof result;
      const resultJson = action === 'page-screenshot' && Buffer.isBuffer(result)
        ? JSON.stringify({ binary: '<Buffer>' })
        : result === null || resultType === 'string' || resultType === 'number' || resultType === 'boolean'
          ? JSON.stringify({ value: result })
          : undefined;
      if (resultJson === undefined)
        session.contextTracingActionEnd(context._id, callId);
      else
        session.contextTracingActionEnd(context._id, callId, resultJson);
      if (snapshotsEligible && pageId !== undefined && typeof session.contextTracingCaptureSnapshot === 'function') {
        try { await session.contextTracingCaptureSnapshot(context._id, pageId, callId, 'after'); }
        catch {}
      }
    }
    return result;
  } catch (error) {
    if (callId !== undefined && callId !== null) {
      try { session.contextTracingActionEnd(context._id, callId); }
      catch {}
    }
    throw error;
  }
}

const RemoteBrowser = require('./remote-browser.cjs')({
  mapError, UnsupportedOperationError, TimeoutError,
  pageEventsSelected: serverConnectPageEventsSelected,
});

const mapNavigationError = require('./navigation-error.cjs')({ mapError, TimeoutError });

// Language exception port only. Rust chooses the channel-validation phase;
// strict JS assignment preserves setters, read-only properties and primitive
// throws without serializing or replacing the user's exception object.
function headerChannelError(error, api) {
  error.message = api + ': ' + error.message;
  throw error;
}

function validateOptions(options, supported, api) {
  if (!options || typeof options !== 'object' || Array.isArray(options))
    throw new TypeError(`${api}: options must be an object`);
  for (const key of Object.keys(options)) {
    if (!supported.includes(key)) throw new UnsupportedOperationError(`${api} option ${key}`);
  }
}

function boundedCdpHandshakeHeaders(headers, api) {
  if (!headers || typeof headers !== 'object' || Array.isArray(headers))
    throw new TypeError(`${api}: headers must be an object`);
  const pairs = [];
  let bytes = 0;
  for (const name in headers) {
    if (!Object.hasOwn(headers, name)) continue;
    if (pairs.length === 32)
      throw new RangeError(`${api}: CDP handshake header count exceeds 32`);
    const nameBytes = Buffer.byteLength(name, 'utf8');
    if (nameBytes > 256)
      throw new RangeError(`${api}: CDP handshake header name exceeds 256 bytes`);
    const value = headers[name];
    if (typeof value !== 'string')
      throw new TypeError(`${api}: CDP handshake header values must be strings`);
    const valueBytes = Buffer.byteLength(value, 'utf8');
    if (valueBytes > 4096)
      throw new RangeError(`${api}: CDP handshake header value exceeds 4096 bytes`);
    bytes += nameBytes + valueBytes;
    if (bytes > 8192)
      throw new RangeError(`${api}: CDP handshake headers exceed 8192 bytes`);
    pairs.push([name, value]);
  }
  return pairs;
}

function navigationTimeout(session, pageId, explicitTimeout) {
  return explicitTimeout ?? (typeof session.pageDefaultTimeout === 'function'
    ? session.pageDefaultTimeout(pageId, true)
    : 30_000);
}

function navigationWaitUntil(value, api) {
  if (value === undefined) return 'load';
  if (value === 'networkidle0') return 'networkidle';
  if (value === 'commit' || value === 'domcontentloaded' || value === 'load'
    || value === 'networkidle') return value;
  throw new TypeError(`${api}: waitUntil: expected one of (load|domcontentloaded|networkidle|commit)`);
}

const selectors = require('./selectors.cjs')({ getNativeEnvironment, mapError, validateOptions });

const handles = require('./handles.cjs')({ mapError, validateOptions, UnsupportedOperationError });
const locatorApi = require('./locator.cjs')({
  mapError, validateOptions, UnsupportedOperationError,
  handleKind: handles.handleKind,
  runTracingAction,
});
const frameLocatorApi = require('./frame-locator.cjs')(locatorApi);
const fileInput = require('./file-input.cjs')({ mapError, validateOptions, UnsupportedOperationError });
const screenshotOptions = require('./screenshot.cjs')({ validateOptions });
const Keyboard = require('./keyboard.cjs')({ mapError, validateOptions, runTracingAction });
const Mouse = require('./mouse.cjs')({ mapError, validateOptions, runTracingAction });
const Touchscreen = require('./touchscreen.cjs')({ mapError, UnsupportedOperationError });
const pageActions = require('./page-actions.cjs')({
  mapError,
  validateOptions,
  UnsupportedOperationError,
  locatorAction: locatorApi.action,
});
const waitForEvent = require('./wait-for-event.cjs')({ TimeoutError, validateOptions });
const pageBindings = require('./page-bindings.cjs')({
  mapError,
  UnsupportedOperationError,
  createHandle: handles.create,
});
const initScripts = require('./init-scripts.cjs')({ mapError, UnsupportedOperationError });
const scriptTags = require('./script-tags.cjs')({ validateOptions });
const waitForFunction = require('./wait-for-function.cjs')({
  mapError,
  validateOptions,
  UnsupportedOperationError,
  createHandle: handles.create,
});
const waitForUrl = require('./wait-for-url.cjs')({ mapError, validateOptions, UnsupportedOperationError });
const frameUrlExpectation = require('./frame-expect-url.cjs')({
  enabled: frameUrlExpectationSelected,
  mapError,
  validateOptions,
  UnsupportedOperationError,
  runTracingAction,
});
const querySelector = require('./query-selector.cjs')();
const pageWaitForSelector = require('./page-wait-for-selector.cjs')({ validateOptions, mapError });
const waitForTimeout = require('./wait-for-timeout.cjs')({ mapError, UnsupportedOperationError });
const timeoutSettings = require('./timeout-settings.cjs')({ mapError, UnsupportedOperationError });
const emulateMedia = require('./emulate-media.cjs')({ mapError, validateOptions, UnsupportedOperationError });
const requestGC = require('./request-gc.cjs')({ mapError, UnsupportedOperationError });
const pagePdf = require('./page-pdf.cjs')({ mapError, validateOptions, UnsupportedOperationError });
const pageViewport = require('./page-viewport.cjs')({ mapError, validateOptions, UnsupportedOperationError });
const responseBodies = require('./response-body.cjs')({ mapError, UnsupportedOperationError });
const responseFinished = require('./response-finished.cjs')({ mapError, UnsupportedOperationError });
const responseHeaders = require('./response-headers.cjs')({ mapError, UnsupportedOperationError });
const loadState = require('./load-state.cjs')({
  mapError,
  validateOptions,
  UnsupportedOperationError,
});
const request = require('./api-request.cjs')({
  nativeAddon,
  validateOptions,
  UnsupportedOperationError,
  runTracingAction,
});
const preflightPersistentContext = require('./persistent-context-preflight.cjs');
const Dialog = require('./dialog.cjs')({ mapError, runTracingAction });
const pageRoutes = require('./page-routes.cjs')({
  mapError,
  runTracingAction,
  validateOptions,
  UnsupportedOperationError,
  createResponseFromNotice: (page, notice, request) => responseEvents.fromNotice(page, notice, request),
  canonicalizeRequest: (...args) => requestEvents.canonicalize(...args),
});
const requestEvents = require('./request-events.cjs')({
  Request: pageRoutes.Request,
  mapError,
  validateOptions,
  UnsupportedOperationError,
  waitForEvent,
});
const responseRequests = require('./response-request.cjs')({
  Request: pageRoutes.Request,
  UnsupportedOperationError,
  canonicalizeRequest: requestEvents.canonicalize,
});
const responseEvents = require('./response-events.cjs')({
  createResponse,
  canonicalizeRequest: requestEvents.canonicalize,
  mapError,
  validateOptions,
  UnsupportedOperationError,
  waitForEvent,
});
const serviceWorkerEvents = require('./service-worker-events.cjs')({
  mapError,
  UnsupportedOperationError,
});
const contextPolicies = require('./context-policies.cjs')({
  UnsupportedOperationError,
  contextDevice,
  featureSelected: feature => typeof nativeFeatures !== 'function'
    || nativeFeatures(nativeArtifact.profile).includes(feature),
});
const contextCookies = require('./context-cookies.cjs')({ mapError, UnsupportedOperationError });
const cdpSessions = require('./cdp-session.cjs')({ mapError, UnsupportedOperationError });
const devices = require('./device-descriptors.json');

async function flushBrowserAfterPageCreation(browser, created, api, owner) {
  try { await cdpSessions.flushBrowser(browser); }
  catch (primary) {
    // Native has already committed the Page. Retire its exact owner before
    // rejecting an API call whose caller never received the new wrapper.
    try { await created.close(); }
    catch (cleanup) {
      throw new AggregateError([primary, cleanup], `${api}: CDP flush and ${owner} cleanup failed`, { cause: primary });
    }
    throw primary;
  }
}

// This continuation captures only a WeakRef, never Browser or NativeSession.
// GC is fallback cleanup; explicit close remains the deterministic contract.
function watchSession(signal, browserRef) {
  Promise.resolve().then(() => signal.wait()).then(connected => {
    if (!connected) browserRef.deref()?._didDisconnect();
    else if (browserRef.deref()) watchSession(signal, browserRef);
  }, error => browserRef.deref()?._didWatchFail(error));
}

class BrowserType {
  constructor() {
    this._name = 'chromium';
    this._playwright = { _defaultTracesDir: undefined };
  }
  name() { return this._name; }
  executablePath() {
    const api = 'browserType.executablePath';
    if (!browserRegistrySnapshot)
      throw new UnsupportedOperationError(api);
    try {
      // The package root is a location input; all browser path policy is Rust-owned.
      return nativeAddon().browserExecutablePathSnapshot(
        this._name, resolve(__dirname, '..'), browserRegistrySnapshot,
      );
    } catch (error) { throw mapError(error, api); }
  }

  async connectOverCDP(endpointURL, options = {}) {
    const api = 'browserType.connectOverCDP';
    if (endpointURL !== null && typeof endpointURL === 'object') {
      validateOptions(endpointURL, ['endpointURL', 'wsEndpoint', 'timeout', 'headers'], api);
      options = { ...endpointURL };
      endpointURL = options.endpointURL ?? options.wsEndpoint;
      delete options.endpointURL;
      delete options.wsEndpoint;
    }
    validateOptions(options, ['timeout', 'headers'], api);
    if (typeof endpointURL !== 'string') throw new TypeError(`${api}: endpointURL must be a string`);
    const timeout = options.timeout ?? 30_000;
    if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
      throw new TypeError(`${api}: timeout must be an integer in [0, 4294967295]`);
    const hasHeaders = Object.hasOwn(options, 'headers');
    if (hasHeaders && !cdpHandshakeHeadersSelected)
      throw new UnsupportedOperationError(`${api} option headers`);
    const headers = hasHeaders ? boundedCdpHandshakeHeaders(options.headers, api) : undefined;
    try {
      let session;
      if (cdpDefaultUserAgentSelected) {
        // Node only supplies runtime identity data; Rust owns UA policy and
        // merges the same bounded header set for discovery and WS upgrade.
        const os = require('node:os');
        session = await getNativeEnvironment().connectOverCDPWithDefaultUserAgent(
          endpointURL, timeout, headers ?? [], [os.platform(), os.arch(), os.release()]);
      } else if (headers === undefined) {
        session = await getNativeEnvironment().connectOverCDP(endpointURL, timeout);
      } else {
        session = await getNativeEnvironment().connectOverCDPWithHeaders(endpointURL, timeout, headers);
      }
      try {
        if (typeof session.initializeFileInputTimeouts === 'function')
          session.initializeFileInputTimeouts(...fileInputDebug);
      } catch (primary) {
        // No public Browser exists yet. The acquired session still owns its
        // resources, including when selected metadata initialization fails.
        try { await session.close(); }
        catch (cleanup) {
          throw new AggregateError([primary, cleanup], 'File-input timeout initialization and session cleanup failed', { cause: primary });
        }
        throw primary;
      }
      return new Browser(this, session);
    } catch (error) { throw mapError(error, api); }
  }

  async launch(options = {}) {
    const api = 'browserType.launch';
    // These are Playwright's client-side assertions, before any launch work.
    if (options.userDataDir)
      throw new Error('userDataDir option is not supported in `browserType.launch`. Use `browserType.launchPersistentContext` instead');
    if (options.port)
      throw new Error('Cannot specify a port without launching as a server.');
    const explicitExecutablePath = Object.hasOwn(options, 'executablePath');
    // Pinned Playwright applies worker defaults after these explicit-option
    // assertions. Keep unsupported merged fields visible to validation.
    options = { ...defaultLaunchOptions, ...options };
    const bidiStartupFailureRoute = bidiStartupFailureSelected && explicitExecutablePath &&
      bidiStartupChannels.chromium.includes(options.channel);
    if (Array.isArray(options.args)) {
      const hasProfileArg = options.args.some(arg => typeof arg === 'string' &&
        arg.startsWith('--user-data-dir'));
      if (hasProfileArg)
        throw new Error("Pass userDataDir parameter to 'browserType.launchPersistentContext(userDataDir, options)' instead of specifying '--user-data-dir' argument");
      if (options.args.some(arg => typeof arg === 'string' && !arg.startsWith('-')))
        throw new Error('Arguments can not specify page to be opened');
    }
    if (options.proxy && typeof options.proxy === 'object' &&
        Object.hasOwn(options.proxy, 'server') && typeof options.proxy.server !== 'string')
      throw new ValidationError(`proxy.server: expected string, got ${typeof options.proxy.server}`);
    validateOptions(options, [
      'executablePath', 'headless', 'timeout', 'handleSIGINT',
      ...(playwrightTraceDirSelected ? ['tracesDir'] : []),
      ...(ownedLaunchSelected || bidiStartupFailureRoute ? ['channel'] : []),
    ], api);
    if (options.channel !== undefined && typeof options.channel !== 'string')
      throw new TypeError(`${api}: channel must be a string`);
    if (playwrightTraceDirSelected && options.tracesDir !== undefined && typeof options.tracesDir !== 'string')
      throw new TypeError(`${api}: tracesDir must be a string`);
    if (options.handleSIGINT !== undefined) {
      if (typeof options.handleSIGINT !== 'boolean')
        throw new TypeError(`${api}: handleSIGINT must be a boolean`);
      // The current owned-launch profile does not install a SIGINT handler.
      // Only the explicit opt-out matches that process policy.
      if (options.handleSIGINT)
        throw new UnsupportedOperationError(`${api} option handleSIGINT=true`);
    }
    if (bidiStartupFailureRoute) {
      // Failure-only startup observation does not create a browser or apply
      // preferences. Keep the channel and executable scope explicit.
      const executablePath = options.executablePath;
      if (typeof executablePath !== 'string' || !isAbsolute(executablePath))
        throw new TypeError(`${api}: executablePath must be a nonempty absolute path`);
      const headless = options.headless ?? true;
      if (typeof headless !== 'boolean')
        throw new TypeError(`${api}: headless must be a boolean`);
      if (!headless)
        throw new UnsupportedOperationError(`${api} option headless=false`);
      const timeout = options.timeout ?? 30_000;
      if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
        throw new TypeError(`${api}: timeout must be an integer in [0, 4294967295]`);
      try {
        await getNativeEnvironment().observeBidiStartupFailure(
          'chromium', executablePath, timeout, options.tracesDir ?? '', process.cwd());
        throw new UnsupportedOperationError(`${api} startup observation returned without a browser protocol`);
      } catch (error) { throw mapError(error, api); }
    }
    if (!ownedLaunchSelected)
      throw new UnsupportedOperationError(api);
    if (options.channel !== undefined && options.channel !== 'chromium')
      throw new UnsupportedOperationError(`${api} channel ${options.channel}`);
    const executablePath = options.executablePath ??
      (browserRegistrySnapshot ? this.executablePath() : undefined);
    if (typeof executablePath !== 'string' || executablePath.length === 0)
      throw new TypeError(`${api}: executablePath must be a nonempty absolute path`);
    // Playwright creates an explicit tracesDir before checking the executable.
    // The selected Rust trace-path leaf owns that ordering and path policy.
    const hasTracesDirOption = playwrightTraceDirSelected && options.tracesDir !== undefined;
    if (!hasTracesDirOption && !existsSync(executablePath))
      throw new Error(`${api}: Failed to launch ${this._name} because executable doesn't exist at ${executablePath}`);
    if (!isAbsolute(executablePath))
      throw new TypeError(`${api}: executablePath must be a nonempty absolute path`);
    const headless = options.headless ?? true;
    if (typeof headless !== 'boolean')
      throw new TypeError(`${api}: headless must be a boolean`);
    const timeout = options.timeout ?? 30_000;
    if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
      throw new TypeError(`${api}: timeout must be an integer in [0, 4294967295]`);
    try {
      const native = getNativeEnvironment();
      const session = hasTracesDirOption
        ? await native.launchChromiumWithTracesDir(executablePath, headless, timeout, options.tracesDir, process.cwd())
        : await native.launchChromium(executablePath, headless, timeout);
      try {
        if (typeof session.initializeFileInputTimeouts === 'function')
          session.initializeFileInputTimeouts(...fileInputDebug);
      } catch (primary) {
        try { await session.close(); }
        catch (cleanup) {
          throw new AggregateError([primary, cleanup], 'File-input timeout initialization and session cleanup failed', { cause: primary });
        }
        throw primary;
      }
      return new Browser(this, session, true);
    } catch (error) { throw mapError(error, api); }
  }
  async launchServer(options = {}) {
    // Match Playwright's pre-launch channel validation without claiming the
    // BrowserServer/process capability that this native profile lacks.
    const channel = { ...options }.channel;
    if (channel !== undefined && typeof channel !== 'string' && !(channel instanceof String))
      throw new ValidationError(`channel: expected string, got ${typeof channel} Failed to launch browser.`);
    // Chromium rejects this user argument while constructing default launch
    // arguments, before any browser process is started. Keep the public
    // preflight separate from the unsupported BrowserServer capability.
    if (this._name === 'chromium' && Array.isArray(options.args) &&
        options.args.some(arg => typeof arg === 'string' && arg.startsWith('--remote-debugging-pipe')))
      throw new Error('Playwright manages remote debugging connection itself.');
    throw new UnsupportedOperationError('browserType.launchServer');
  }
  async launchPersistentContext(userDataDir, options = {}) {
    const api = 'browserType.launchPersistentContext';
    if (options.port)
      throw new Error('Cannot specify a port without launching as a server.');
    options = { ...defaultLaunchOptions, ...options };
    await preflightPersistentContext(this._name, options, undefined,
      persistentContextPreflightSelected ? nativeAddon : undefined);
    if (options.channel !== undefined && options.channel !== 'chromium')
      throw new UnsupportedOperationError(`${api} channel ${options.channel}`);
    const plan = videoRecording.prepare(
      options.recordVideo,
      options.viewport,
      api,
      options.videosPath,
      options.videoSize,
    );
    if (!ownedLaunchSelected)
      throw new UnsupportedOperationError(api);
    if (typeof userDataDir !== 'string' && !(userDataDir instanceof String))
      throw new TypeError(`${api}: userDataDir must be a string`);
    const executablePath = options.executablePath ?? this.executablePath();
    if (typeof executablePath !== 'string' || !isAbsolute(executablePath))
      throw new TypeError(`${api}: executablePath must be a nonempty absolute path`);
    if (!existsSync(executablePath))
      throw new Error(`${api}: Failed to launch ${this._name} because executable doesn't exist at ${executablePath}`);
    const headless = options.headless ?? true;
    if (typeof headless !== 'boolean')
      throw new TypeError(`${api}: headless must be a boolean`);
    const timeout = options.timeout ?? 30_000;
    if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
      throw new TypeError(`${api}: timeout must be an integer in [0, 4294967295]`);
    if (options.baseURL !== undefined && typeof options.baseURL !== 'string')
      throw new TypeError(`${api}: baseURL must be a string`);
    if (options.locale !== undefined && typeof options.locale !== 'string')
      throw new TypeError(`${api}: locale must be a string`);
    if (options.timezoneId !== undefined && typeof options.timezoneId !== 'string')
      throw new TypeError(`${api}: timezoneId must be a string`);
    const geolocation = options.geolocation === undefined
      ? undefined
      : contextPolicies.normalizeGeolocation(options.geolocation, api);
    const permissions = options.permissions === undefined
      ? undefined
      : contextPolicies.normalizePermissions(options.permissions, api);
    const httpCredentials = options.httpCredentials === undefined
      ? undefined
      : contextPolicies.normalizeHttpCredentials(options.httpCredentials, api);
    if (options.offline !== undefined && typeof options.offline !== 'boolean')
      throw new TypeError(`${api}: offline must be a boolean`);
    if (options.bypassCSP !== undefined && typeof options.bypassCSP !== 'boolean')
      throw new TypeError(`${api}: bypassCSP must be a boolean`);
    if (options.javaScriptEnabled !== undefined && typeof options.javaScriptEnabled !== 'boolean')
      throw new TypeError(`${api}: javaScriptEnabled must be a boolean`);
    const locale = options.locale === '' ? undefined : options.locale;
    const timezoneId = options.timezoneId === '' ? undefined : options.timezoneId;
    const offline = options.offline === true;
    const bypassCsp = options.bypassCSP === true;
    const javaScriptEnabled = options.javaScriptEnabled;
    if (locale !== undefined && !persistentContextLocaleSelected)
      throw new UnsupportedOperationError(`${api} option locale`);
    if (timezoneId !== undefined && !persistentContextTimezoneSelected)
      throw new UnsupportedOperationError(`${api} option timezoneId`);
    if (geolocation !== undefined && !persistentContextGeolocationSelected)
      throw new UnsupportedOperationError(`${api} option geolocation`);
    if (permissions?.length && !persistentContextPermissionsSelected)
      throw new UnsupportedOperationError(`${api} option permissions`);
    if (httpCredentials !== undefined && !persistentContextHttpCredentialsSelected)
      throw new UnsupportedOperationError(`${api} option httpCredentials`);
    if (offline && !persistentContextOfflineSelected)
      throw new UnsupportedOperationError(`${api} option offline`);
    if (bypassCsp && !persistentContextBypassCspSelected)
      throw new UnsupportedOperationError(`${api} option bypassCSP`);
    if (javaScriptEnabled === false && !persistentContextJavascriptDisabledSelected)
      throw new UnsupportedOperationError(`${api} option javaScriptEnabled`);
    const hasContextDefaults = locale !== undefined || timezoneId !== undefined || geolocation !== undefined
      || permissions?.length || httpCredentials !== undefined
      || offline || bypassCsp || javaScriptEnabled === false;
    try {
      const environment = getNativeEnvironment();
      let session;
      if (plan === undefined) {
        if (!hasContextDefaults) {
          if (typeof environment.launchPersistentChromium !== 'function')
            throw new UnsupportedOperationError(api);
          session = await environment.launchPersistentChromium(
            executablePath,
            resolve(String(userDataDir)),
            timeout,
            options.baseURL,
            (headless ? 1 : 0) | (options.ignoreHTTPSErrors === true ? 2 : 0),
          );
        } else {
          if (typeof environment.launchPersistentChromiumWithContextDefaults !== 'function')
            throw new UnsupportedOperationError(`${api} context defaults`);
          session = await environment.launchPersistentChromiumWithContextDefaults(
            executablePath,
            resolve(String(userDataDir)),
            timeout,
            options.baseURL,
            (headless ? 1 : 0) | (options.ignoreHTTPSErrors === true ? 2 : 0),
            { locale, timezoneId, geolocation, permissions, offline, bypassCsp, javaScriptEnabled,
              ...(httpCredentials === undefined ? {} : { httpCredentials }) },
          );
        }
      } else {
        if (!hasContextDefaults) {
          if (typeof environment.launchPersistentChromiumWithVideo !== 'function')
            throw new UnsupportedOperationError(`${api} recordVideo`);
          session = await environment.launchPersistentChromiumWithVideo(
              executablePath,
              resolve(String(userDataDir)),
              (headless ? 1 : 0) | (options.ignoreHTTPSErrors === true ? 2 : 0),
              timeout,
              options.baseURL,
              plan.directory,
              plan.ffmpeg,
              plan.width,
              plan.height,
          );
        } else {
          if (typeof environment.launchPersistentChromiumWithVideoAndContextDefaults !== 'function')
            throw new UnsupportedOperationError(`${api} options recordVideo and context defaults`);
          session = await environment.launchPersistentChromiumWithVideoAndContextDefaults(
              executablePath,
              resolve(String(userDataDir)),
              (headless ? 1 : 0) | (options.ignoreHTTPSErrors === true ? 2 : 0),
              timeout,
              options.baseURL,
              { locale, timezoneId, geolocation, permissions, offline, bypassCsp, javaScriptEnabled,
                ...(httpCredentials === undefined ? {} : { httpCredentials }) },
              plan.directory,
              plan.ffmpeg,
              plan.width,
              plan.height,
          );
        }
      }
      const browser = new Browser(this, session, true);
      const contexts = browser.contexts();
      if (contexts.length !== 1) {
        try { await browser.close(); } catch {}
        throw new Error(`${api}: persistent Chromium did not publish exactly one default context`);
      }
      const context = contexts[0];
      if (plan !== undefined) context._recordVideo = plan;
      frameUrlExpectation.setContextBaseURL(context, options.baseURL);
      return context;
    } catch (error) { throw mapError(error, api); }
  }
  async connect(wsEndpoint, options = {}) {
    const api = 'browserType.connect';
    if (!serverConnectWsErrorSelected && !serverConnectCloseErrorSelected && !serverConnectHeadersSelected && !serverConnectSessionSelected)
      throw new UnsupportedOperationError(api);
    if (wsEndpoint !== null && typeof wsEndpoint === 'object') {
      validateOptions(wsEndpoint, serverConnectSessionSelected
        ? ['wsEndpoint', 'timeout', 'headers', 'slowMo', 'exposeNetwork', '_exposeNetwork', 'logger', '__testHookRedirectPortForwarding', '__testHookBeforeCreateBrowser']
        : ['wsEndpoint', 'timeout', 'headers'], api);
      options = { ...wsEndpoint };
      wsEndpoint = options.wsEndpoint;
      delete options.wsEndpoint;
    }
    validateOptions(options, serverConnectSessionSelected
      ? ['timeout', 'headers', 'slowMo', 'exposeNetwork', '_exposeNetwork', 'logger', '__testHookRedirectPortForwarding', '__testHookBeforeCreateBrowser']
      : ['timeout', 'headers', '__testHookRedirectPortForwarding'], api);
    if (options.__testHookRedirectPortForwarding !== undefined)
      throw new UnsupportedOperationError(`${api} option __testHookRedirectPortForwarding`);
    if (serverConnectSessionSelected) {
      for (const name of ['slowMo', 'exposeNetwork', '_exposeNetwork', 'logger', '__testHookBeforeCreateBrowser']) {
        if (options[name] !== undefined) throw new UnsupportedOperationError(`${api} option ${name}`);
      }
    }
    if (typeof wsEndpoint !== 'string')
      throw new TypeError(`${api}: wsEndpoint must be a string`);
    const timeout = options.timeout ?? 30_000;
    const minimumTimeout = serverConnectSessionSelected ? 0 : 1;
    if (!Number.isInteger(timeout) || timeout < minimumTimeout || timeout > 120_000)
      throw new TypeError(`${api}: timeout must be an integer in [${minimumTimeout}, 120000]`);
    const headers = options.headers ?? {};
    if (!headers || typeof headers !== 'object' || Array.isArray(headers))
      throw new TypeError(`${api}: headers must be an object`);
    try {
      const native = getNativeEnvironment();
      if (serverConnectSessionSelected) {
        const os = require('node:os');
        const connectRemote = serverConnectPageEventsSelected
          ? native.connectPlaywrightServerPageEvents.bind(native)
          : native.connectPlaywrightServer.bind(native);
        const remote = await connectRemote(
          wsEndpoint, timeout, Object.entries(headers), this._name,
          [os.platform(), os.arch(), os.release()]);
        return new RemoteBrowser(this, remote, serverConnectPageEventsSelected);
      } else if (serverConnectHeadersSelected) {
        // Runtime identity is observed here; Rust owns header composition.
        const os = require('node:os');
        await native.connectPlaywrightServerHeaders(
          wsEndpoint, timeout, Object.entries(headers), this._name,
          [os.platform(), os.arch(), os.release()]);
      } else if (serverConnectCloseErrorSelected)
        await native.connectPlaywrightServerCloseError(wsEndpoint, timeout, Object.entries(headers));
      else
        await native.connectPlaywrightServerError(wsEndpoint, timeout, Object.entries(headers));
      throw new UnsupportedOperationError(api);
    } catch (error) { throw mapError(error, api); }
  }
}

class Browser extends EventEmitter {
  constructor(type, session, freshOwned = false) {
    super();
    this._type = type;
    this._session = session;
    this._freshOwned = freshOwned;
    this._contexts = new Map();
    this._browserCdpSessions = new Map();
    this._disconnected = false;
    this._watchFailure = undefined;
    this._watchCleanup = undefined;
    const browserRef = new WeakRef(this);
    if (session.setLifecycleCallback) session.setLifecycleCallback(lifecycleEvents(browserRef, {
      context: (browser, value) => new BrowserContext(browser, value.id, value.settings),
      page: (context, value) => new Page(context, value),
      dialog: (page, native, type, message, defaultValue) =>
        new Dialog(page, native, type, message, defaultValue),
    }, frameEventsSelected));
    watchSession(session.sessionSignal(), browserRef);
    pageBindings.watch(session, browserRef);
    pageRoutes.watch(session, browserRef);
    serviceWorkerEvents.watch(session, browserRef);
    if (!requestEvents.watchCombined(session, browserRef, responseEvents.dispatch)) {
      requestEvents.watch(session, browserRef);
      responseEvents.watch(session, browserRef);
    }
  }

  browserType() { return this._type; }
  newBrowserCDPSession() { return cdpSessions.createBrowser(this); }
  isConnected() { return !this._watchFailure && !this._disconnected && this._session.isConnected(); }
  version() { return this._session.version(); }
  contexts() {
    if (this._session.contextSnapshot) {
      try {
        const snapshot = this._session.contextSnapshot([...this._contexts.keys()]);
        const active = new Set(snapshot.ids);
        for (const [id, context] of this._contexts) {
          if (active.has(id)) continue;
          context._retireAllCdpSessions();
          context._retireNetworkRequestCache();
          serviceWorkerEvents.retire(context);
          for (const page of context._pages.values()) page._retireNetworkRequestCache();
          this._contexts.delete(id);
        }
        for (const value of snapshot.newContexts) this._wrapContext(value);
        return snapshot.ids.map(id => this._contexts.get(id));
      } catch (error) { throw mapError(error, 'browser.contexts'); }
    }
    const descriptors = this._session.contexts();
    const active = new Set(descriptors.map(value => value.id));
    for (const [id, context] of this._contexts) {
      if (active.has(id)) continue;
      context._retireAllCdpSessions();
      context._retireNetworkRequestCache();
      serviceWorkerEvents.retire(context);
      for (const page of context._pages.values()) page._retireNetworkRequestCache();
      this._contexts.delete(id);
    }
    return descriptors.map(value => this._wrapContext(value));
  }
  _wrapContext(value) {
    if (!this._contexts.has(value.id)) this._contexts.set(value.id, new BrowserContext(this, value.id, value.settings));
    return this._contexts.get(value.id);
  }

  async close(options = {}) {
    validateOptions(options, ['reason'], 'browser.close');
    if (options.reason !== undefined && typeof options.reason !== 'string')
      throw new TypeError('browser.close: reason must be a string');
    this._cdpClosePending = (this._cdpClosePending ?? 0) + 1;
    cdpSessions.pauseBrowser(this);
    // Native shutdown can retire Context notices before JS observes them.
    // Keep wrapper identities across that await so an owned Browser still
    // delivers one close event for every Context it exposed to callers.
    const ownedContexts = this._freshOwned ? [...this._contexts.values()] : undefined;
    try { await this._session.close(); }
    catch (error) {
      this._cdpClosePending -= 1;
      if (!this._cdpClosePending) cdpSessions.resumeBrowser(this);
      throw mapError(error, 'browser.close');
    }
    this._cdpClosed = true;
    const notificationErrors = this._retireOwnedContexts(ownedContexts ?? []);
    this._didDisconnect();
    this._cdpClosePending -= 1;
    this._reportOwnedContextErrors(notificationErrors);
  }

  _retireOwnedContexts(contexts) {
    const notificationErrors = [];
    for (const context of contexts) {
      if (!context._ownedCloseCleanupDone) {
        context._ownedCloseCleanupDone = true;
        try {
          context._retireAllCdpSessions();
          context._retireNetworkRequestCache();
          serviceWorkerEvents.retire(context);
          for (const page of context._pages.values()) page._retireNetworkRequestCache();
          context._request?._disposeFromOwner('Browser closed');
        } catch (error) { notificationErrors.push(error); }
        context._pages.clear();
        this._contexts.delete(context._id);
      }
      if (context._closeNotified) continue;
      try { context._emitCloseOnce(); }
      catch (error) { notificationErrors.push(error); }
    }
    return notificationErrors;
  }

  _reportOwnedContextErrors(notificationErrors) {
    // The process and all Contexts are terminal even if a JS listener throws.
    // A real native session has the same exception channel used by lifecycle
    // notices; test doubles without it receive their original exception.
    if (this._session.reportCallbackException) {
      for (const error of notificationErrors) this._session.reportCallbackException(error);
    } else if (notificationErrors.length === 1) {
      throw notificationErrors[0];
    } else if (notificationErrors.length) {
      throw new AggregateError(notificationErrors, 'Context close listeners failed');
    }
  }

  _didDisconnect(force = false) {
    if (this._disconnected) return false;
    if (this._session.publishDisconnect && !force) {
      if (!this._session.publishDisconnect()) return;
    }
    cdpSessions.stopBrowser(this);
    cdpSessions.retireBrowser(this);
    this._cdpClosed = true;
    const notificationErrors = this._freshOwned
      ? this._retireOwnedContexts([...this._contexts.values()])
      : [];
    this._disconnected = true;
    for (const context of this._contexts.values()) {
      context._retireAllCdpSessions();
      context._retireNetworkRequestCache();
      serviceWorkerEvents.retire(context);
      for (const page of context._pages.values()) page._retireNetworkRequestCache();
    }
    this._contexts.clear();
    try { this.emit('disconnected', this); }
    catch (error) {
      // A native notification's listener error is not an API/watch rejection.
      // Rust reports the original value through Node's exception channel with
      // no retained callback or queue. Older artifacts retain their old behavior.
      if (!this._session.reportCallbackException) throw error;
      this._session.reportCallbackException(error);
    }
    this._reportOwnedContextErrors(notificationErrors);
    return true;
  }

  _didWatchFail(primary) {
    if (this._watchCleanup) return;
    cdpSessions.stopBrowser(this);
    this._watchFailure = primary;
    this._watchCleanup = Promise.resolve().then(() => this._session.close()).then(
      () => {
        // Closing must make terminal metadata publishable. Force a local
        // terminal state if an older or faulty artifact still refuses it.
        if (!this._didDisconnect()) this._didDisconnect(true);
      },
      cleanup => {
        this._watchFailure = new AggregateError(
          [primary, cleanup],
          'Session watch and cleanup failed',
          { cause: primary },
        );
        this._didDisconnect(true);
      },
    ).catch(notification => {
      this._watchFailure = new AggregateError(
        [this._watchFailure, notification],
        'Session watch cleanup and disconnect notification failed',
        { cause: this._watchFailure },
      );
    });
  }

  playwrightRustMetrics() { return this._session.metrics(); }
  playwrightRustMetadataMetrics() { return this._session.frameMetadataMetrics(); }
  playwrightRustFrameTreeMetrics() {
    if (!this._session.frameTreeMetrics) throw new UnsupportedOperationError('playwrightRustFrameTreeMetrics (rebuild native addon)');
    return this._session.frameTreeMetrics();
  }
  playwrightRustContextMetadataMetrics() {
    if (!this._session.contextMetadataMetrics) throw new UnsupportedOperationError('playwrightRustContextMetadataMetrics (rebuild native addon)');
    return this._session.contextMetadataMetrics();
  }
  playwrightRustFfiMetrics() { return this._session.ffiMetrics(); }
  playwrightRustConversionMetrics() { return this._session.conversionMetrics(); }
  playwrightRustLocatorConversionMetrics() { return this._session.locatorConversionMetrics(); }
  playwrightRustLocatorMetrics() { return this._session.locatorMetrics(); }
  async newContext(options = {}) {
    // Language-level copy mirrors the pinned selector-options adapter: read
    // own enumerable option getters once, without inheriting option fields.
    options = { ...options };
    validateOptions(options, [
      'baseURL', 'extraHTTPHeaders', 'proxy',
      'acceptDownloads', 'bypassCSP', 'colorScheme', 'hasTouch',
      'geolocation', 'permissions', 'httpCredentials', 'ignoreHTTPSErrors', 'isMobile', 'javaScriptEnabled', 'locale',
      'offline', 'serviceWorkers', 'timezoneId', 'userAgent', 'viewport', 'screen', 'noViewport', 'deviceScaleFactor',
      'storageState',
      'defaultBrowserType',
      'recordHar', 'recordVideo', 'videosPath', 'videoSize',
    ], 'browser.newContext');
    const harPlan = harRecording.prepare(options.recordHar);
    const videoPlan = videoRecording.prepare(
      options.recordVideo,
      options.viewport,
      'browser.newContext',
      options.videosPath,
      options.videoSize,
    );
    if (options.viewport === null) {
      options.noViewport = true;
      delete options.viewport;
    }
    contextNullViewport.validate(options);
    validateDefaultContextOptions(options);
    contextPolicies.validate(options);
    contextDevice.validate(options);
    if (options.storageState !== undefined
        && (typeof options.storageState !== 'string'
          && (!options.storageState || typeof options.storageState !== 'object'
            || Array.isArray(options.storageState))))
      throw new TypeError('browser.newContext: storageState must be an object or path');
    if (options.storageState !== undefined && !contextStorageStateSelected)
      throw new UnsupportedOperationError('browser.newContext storageState (select context-storage-state native profile)');
    if (options.baseURL !== undefined && typeof options.baseURL !== 'string')
      throw new TypeError('browser.newContext: baseURL must be a string');
    if (options.baseURL !== undefined && !this._session.contextSnapshot)
      throw new UnsupportedOperationError('browser.newContext baseURL (rebuild native addon)');
    if (options.extraHTTPHeaders && !this._session.newContextWithExtraHttpHeaders)
      throw new UnsupportedOperationError('browser.newContext extraHTTPHeaders (select extra-headers native profile)');
    if (options.proxy !== undefined) request._validateProxy(options.proxy, 'browser.newContext');
    if (options.proxy !== undefined && !this._session.newContextWithProxy)
      throw new UnsupportedOperationError('browser.newContext proxy (select context-proxy native profile)');
    const storageState = typeof options.storageState === 'string'
      ? JSON.parse(await readFile(options.storageState, 'utf8'))
      : options.storageState;
    // Do not map arbitrary synchronous user getter/conversion exceptions.
    const policyPending = contextPolicies.create(
      this._session,
      options,
      options.baseURL,
      headerChannelError,
    );
    const devicePending = policyPending === undefined
      ? contextDevice.create(this._session, options, options.baseURL)
      : undefined;
    const pending = policyPending !== undefined
      ? policyPending
      : devicePending !== undefined
        ? devicePending
        : options.proxy !== undefined
          ? this._session.newContextWithProxy(options.baseURL, options.proxy)
          : options.extraHTTPHeaders
            ? this._session.newContextWithExtraHttpHeaders(options.extraHTTPHeaders, headerChannelError, options.baseURL)
            : this._session.newContext(options.baseURL);
    try {
      const context = this._wrapContext(await pending);
      frameUrlExpectation.setContextBaseURL(context, options.baseURL);
      context._recordVideo = videoPlan;
      if (storageState !== undefined) {
        try {
          await this._session.contextSetStorageState(context._id, JSON.stringify(storageState));
        } catch (error) {
          try { await context.close(); }
          catch (cleanup) {
            throw new AggregateError(
              [error, cleanup],
              'browser.newContext storageState initialization and cleanup failed',
              { cause: error },
            );
          }
          throw error;
        }
      }
      try {
        await videoRecording.initialize(this._session, context._id, videoPlan);
      } catch (error) {
        try { await context.close(); }
        catch (cleanup) {
          throw new AggregateError(
            [error, cleanup],
            'browser.newContext Video initialization and cleanup failed',
            { cause: error },
          );
        }
        throw error;
      }
      try {
        await harRecording.initialize(this._session, context._id, harPlan);
      } catch (error) {
        try { await context.close(); }
        catch (cleanup) {
          throw new AggregateError(
            [error, cleanup],
            'browser.newContext HAR initialization and cleanup failed',
            { cause: error },
          );
        }
        throw error;
      }
      return context;
    }
    catch (error) { throw mapError(error, 'browser.newContext'); }
  }
  async newPage(options = {}) {
    options = { ...options };
    validateOptions(options, ['baseURL', 'acceptDownloads', 'recordVideo'], 'browser.newPage');
    const videoPlan = videoRecording.prepare(options.recordVideo, undefined, 'browser.newPage');
    if (options.baseURL !== undefined && typeof options.baseURL !== 'string')
      throw new TypeError('browser.newPage: baseURL must be a string');
    if (options.acceptDownloads !== undefined && typeof options.acceptDownloads !== 'boolean')
      throw new TypeError('browser.newPage: acceptDownloads must be a boolean');
    if (options.acceptDownloads === true)
      throw new UnsupportedOperationError('browser.newPage acceptDownloads=true (select download-events native profile)');
    if (!this._session.newBrowserPage)
      throw new UnsupportedOperationError('browser.newPage (rebuild native addon)');
    try {
      const value = videoPlan === undefined
        ? await this._session.newBrowserPage(options.baseURL, false)
        : await videoRecording.newBrowserPage(this._session, options.baseURL, false, videoPlan);
      const context = this._wrapContext(value.context);
      frameUrlExpectation.setContextBaseURL(context, options.baseURL);
      context._recordVideo = videoPlan;
      const page = context._wrapPage(value.page);
      await flushBrowserAfterPageCreation(this, context, 'browser.newPage', 'Context');
      return page;
    } catch (error) { throw mapError(error, 'browser.newPage'); }
  }
}

function validateDefaultContextOptions(options) {
  const api = 'browser.newContext';
  for (const [name, expected] of [
    ['acceptDownloads', true],
    ['javaScriptEnabled', true],
  ]) {
    const value = options[name];
    if (value === undefined) continue;
    if (typeof value !== 'boolean') throw new TypeError(`${api}: ${name} must be a boolean`);
    if (value !== expected) throw new UnsupportedOperationError(`${api} ${name}=${value}`);
  }
  for (const [name, expected] of [
    ['colorScheme', 'light'],
    ['serviceWorkers', 'allow'],
  ]) {
    const value = options[name];
    if (value === undefined) continue;
    if (typeof value !== 'string') throw new TypeError(`${api}: ${name} must be a string`);
    if (value !== expected) throw new UnsupportedOperationError(`${api} ${name}=${value}`);
  }
}

class Tracing {
  constructor(context) {
    this._context = context;
  }

  async start(options = {}) {
    validateOptions(options, ['screenshots', 'snapshots', 'sources', 'name', 'title'], 'browserContext.tracing.start');
    for (const name of ['screenshots', 'snapshots', 'sources']) {
      if (options[name] !== undefined && typeof options[name] !== 'boolean')
        throw new TypeError(`browserContext.tracing.start: ${name} must be a boolean`);
    }
    if (options.title !== undefined && typeof options.title !== 'string')
      throw new TypeError('browserContext.tracing.start: title must be a string');
    if (options.name !== undefined && typeof options.name !== 'string')
      throw new TypeError('browserContext.tracing.start: name must be a string');
    if (options.snapshots === true && !contextTracingSnapshotsSelected)
      throw new UnsupportedOperationError('browserContext.tracing.start snapshots (select context-tracing-snapshots native profile)');
    const session = this._context._browser._session;
    if (!contextTracingScreenshotsSelected || !session.contextTracingStart)
      throw new UnsupportedOperationError('browserContext.tracing.start (select context-tracing-screenshots native profile)');
    try {
      session.contextTracingStart(
        this._context._id,
        options.screenshots === true,
        options.snapshots === true,
        options.sources === true,
        options.name,
        options.title,
      );
      this._context._traceSources = options.sources === true;
    }
    catch (error) { throw mapError(error, 'browserContext.tracing.start'); }
  }

  async stop(options = {}) {
    validateOptions(options, ['path'], 'browserContext.tracing.stop');
    if (options.path !== undefined && (typeof options.path !== 'string' || options.path.length === 0))
      throw new TypeError('browserContext.tracing.stop: path must be a nonempty string');
    const session = this._context._browser._session;
    if (!contextTracingScreenshotsSelected || !session.contextTracingStop)
      throw new UnsupportedOperationError('browserContext.tracing.stop (select context-tracing-screenshots native profile)');
    const path = options.path === undefined ? undefined : resolve(options.path);
    try { await session.contextTracingStop(this._context._id, path); }
    catch (error) { throw mapError(error, 'browserContext.tracing.stop'); }
    finally { this._context._traceSources = false; }
  }

  async startChunk(options = {}) {
    validateOptions(options, ['name', 'title'], 'browserContext.tracing.startChunk');
    if (options.name !== undefined && typeof options.name !== 'string')
      throw new TypeError('browserContext.tracing.startChunk: name must be a string');
    if (options.title !== undefined && typeof options.title !== 'string')
      throw new TypeError('browserContext.tracing.startChunk: title must be a string');
    const session = this._context._browser._session;
    if (!contextTracingChunksSelected || !session.contextTracingStartChunk)
      throw new UnsupportedOperationError('browserContext.tracing.startChunk (select context-tracing-chunks native profile)');
    try { session.contextTracingStartChunk(this._context._id, options.name, options.title); }
    catch (error) { throw mapError(error, 'browserContext.tracing.startChunk'); }
  }

  async stopChunk(options = {}) {
    validateOptions(options, ['path'], 'browserContext.tracing.stopChunk');
    if (options.path !== undefined && (typeof options.path !== 'string' || options.path.length === 0))
      throw new TypeError('browserContext.tracing.stopChunk: path must be a nonempty string');
    const session = this._context._browser._session;
    if (!contextTracingChunksSelected || !session.contextTracingStopChunk)
      throw new UnsupportedOperationError('browserContext.tracing.stopChunk (select context-tracing-chunks native profile)');
    const path = options.path === undefined ? undefined : resolve(options.path);
    try { await session.contextTracingStopChunk(this._context._id, path); }
    catch (error) { throw mapError(error, 'browserContext.tracing.stopChunk'); }
  }

  async group(name, options = {}) {
    if (typeof name !== 'string')
      throw new TypeError('browserContext.tracing.group: name must be a string');
    validateOptions(options, ['location'], 'browserContext.tracing.group');
    if (options.location !== undefined)
      throw new UnsupportedOperationError('browserContext.tracing.group location');
    const session = this._context._browser._session;
    if (!contextTracingChunksSelected || !session.contextTracingGroup)
      throw new UnsupportedOperationError('browserContext.tracing.group (select context-tracing-chunks native profile)');
    try { session.contextTracingGroup(this._context._id, name); }
    catch (error) { throw mapError(error, 'browserContext.tracing.group'); }
  }

  async groupEnd() {
    const session = this._context._browser._session;
    if (!contextTracingChunksSelected || !session.contextTracingGroupEnd)
      throw new UnsupportedOperationError('browserContext.tracing.groupEnd (select context-tracing-chunks native profile)');
    try { session.contextTracingGroupEnd(this._context._id); }
    catch (error) { throw mapError(error, 'browserContext.tracing.groupEnd'); }
  }
}

class BrowserContext extends NativeConsoleEventEmitter {
  constructor(browser, id, settings) {
    super(browser._session, id, 'setContextConsoleListener');
    this._browser = browser;
    this._id = id;
    this._settings = settings;
    this._pages = new Map();
    this._serviceWorkers = new Map();
    this._bindings = new Map();
    this._routes = new Map();
    this._networkRequests = new Map();
    this._cdpSessions = new Map();
    this._request = undefined;
    this.tracing = new Tracing(this);
    frameUrlExpectation.setContextBaseURL(this, undefined);
    this._closeNotified = false;
    this._ownedCloseCleanupDone = false;
  }
  _emitCloseOnce() {
    if (this._closeNotified) return false;
    this._closeNotified = true;
    this.emit('close', this);
    return true;
  }
  browser() { return this._browser; }
  serviceWorkers() { return serviceWorkerEvents.snapshot(this); }
  _wrapServiceWorker(value) { return serviceWorkerEvents.wrap(this, value); }
  _retireNetworkRequestCache() {
    this._networkRequests.clear();
    this._networkRequestRefs?.clear();
  }
  get request() {
    if (this._request) return this._request;
    const session = this._browser._session;
    if (!session.contextRequest)
      throw new UnsupportedOperationError('browserContext.request (select context-api-request native profile)');
    try {
      this._request = request._fromNative(session.contextRequest(this._id), this._baseURL, this);
      return this._request;
    } catch (error) { throw mapError(error, 'browserContext.request'); }
  }
  waitForEvent(event, optionsOrPredicate = {}) {
    const apiStack = {};
    Error.captureStackTrace?.(apiStack, BrowserContext.prototype.waitForEvent);
    return waitForEvent(
      this,
      event,
      optionsOrPredicate,
      'browserContext.waitForEvent',
      30_000,
      { apiStack },
    );
  }
  setDefaultTimeout(timeout) {
    timeoutSettings.setContext(this, timeout, false, 'browserContext.setDefaultTimeout');
  }
  setDefaultNavigationTimeout(timeout) {
    timeoutSettings.setContext(this, timeout, true, 'browserContext.setDefaultNavigationTimeout');
  }
  async setExtraHTTPHeaders(headers) {
    const session = this._browser._session;
    if (!session.setContextExtraHttpHeaders) throw new UnsupportedOperationError('browserContext.setExtraHTTPHeaders (select extra-headers native profile)');
    // Synchronous language/getter exceptions must propagate unchanged.
    const pending = session.setContextExtraHttpHeaders(this._id, headers, headerChannelError);
    try { await pending; }
    catch (error) { throw mapError(error, 'browserContext.setExtraHTTPHeaders'); }
  }
  async setOffline(offline) {
    if (typeof offline !== 'boolean')
      throw new TypeError('browserContext.setOffline: offline must be a boolean');
    const session = this._browser._session;
    if (!session.setContextOffline)
      throw new UnsupportedOperationError('browserContext.setOffline (select context-offline native profile)');
    try { await session.setContextOffline(this._id, offline); }
    catch (error) { throw mapError(error, 'browserContext.setOffline'); }
  }
  async setHTTPCredentials(httpCredentials) {
    const normalized = contextPolicies.normalizeRuntimeHttpCredentials(
      httpCredentials,
      'browserContext.setHTTPCredentials',
    );
    const session = this._browser._session;
    if (!session.setContextHttpCredentials)
      throw new UnsupportedOperationError('browserContext.setHTTPCredentials (select context-http-credentials native profile)');
    try { await session.setContextHttpCredentials(this._id, normalized); }
    catch (error) { throw mapError(error, 'browserContext.setHTTPCredentials'); }
  }
  async setGeolocation(geolocation = null) {
    const normalized = contextPolicies.normalizeGeolocation(
      geolocation,
      'browserContext.setGeolocation',
      true,
    );
    const session = this._browser._session;
    if (!session.setContextGeolocation)
      throw new UnsupportedOperationError('browserContext.setGeolocation (select context-geolocation native profile)');
    try { await session.setContextGeolocation(this._id, normalized); }
    catch (error) { throw mapError(error, 'browserContext.setGeolocation'); }
  }
  async grantPermissions(permissions, options = {}) {
    permissions = contextPolicies.normalizePermissions(permissions, 'browserContext.grantPermissions');
    if (options === null || typeof options !== 'object' || Array.isArray(options))
      throw new TypeError('browserContext.grantPermissions: options must be an object');
    options = { ...options };
    validateOptions(options, ['origin'], 'browserContext.grantPermissions');
    if (options.origin !== undefined && typeof options.origin !== 'string')
      throw new TypeError('browserContext.grantPermissions: origin must be a string');
    const session = this._browser._session;
    if (!session.grantContextPermissions)
      throw new UnsupportedOperationError('browserContext.grantPermissions (select context-permissions native profile)');
    try { await session.grantContextPermissions(this._id, permissions, options.origin); }
    catch (error) { throw mapError(error, 'browserContext.grantPermissions'); }
  }
  async clearPermissions() {
    const session = this._browser._session;
    if (!session.clearContextPermissions)
      throw new UnsupportedOperationError('browserContext.clearPermissions (select context-permissions native profile)');
    try { await session.clearContextPermissions(this._id); }
    catch (error) { throw mapError(error, 'browserContext.clearPermissions'); }
  }
  addCookies(cookies) {
    return contextCookies.add(this, cookies);
  }
  cookies(urls) {
    return contextCookies.cookies(this, urls);
  }
  clearCookies(options = {}) {
    return contextCookies.clear(this, options);
  }
  async storageState(options = {}) {
    validateOptions(options, ['indexedDB', 'path'], 'browserContext.storageState');
    if (options.indexedDB !== undefined && typeof options.indexedDB !== 'boolean')
      throw new TypeError('browserContext.storageState: indexedDB must be a boolean');
    if (options.path !== undefined) {
      if (typeof options.path !== 'string')
        throw new TypeError('browserContext.storageState: path must be a string');
    }
    const session = this._browser._session;
    if (!contextStorageStateSelected || typeof session.contextStorageState !== 'function')
      throw new UnsupportedOperationError('browserContext.storageState (select context-storage-state native profile)');
    return runTracingAction(this, 'browser-context-storage-state', undefined, async () => {
      try {
        const state = JSON.parse(await session.contextStorageState(this._id, options.indexedDB === true));
        if (options.path !== undefined)
          await writeFile(options.path, JSON.stringify(state, undefined, 2));
        return state;
      } catch (error) { throw mapError(error, 'browserContext.storageState'); }
    });
  }
  addInitScript(script, arg) {
    return initScripts.addContext(this, script, arguments.length > 1, arg);
  }
  async exposeBinding(name, callback, options = {}) {
    validateOptions(options, ['handle'], 'browserContext.exposeBinding');
    if (typeof name !== 'string' || !name)
      throw new TypeError('browserContext.exposeBinding: name must be a non-empty string');
    if (typeof callback !== 'function')
      throw new TypeError('browserContext.exposeBinding: callback must be a function');
    if (options.handle !== undefined && typeof options.handle !== 'boolean')
      throw new TypeError('browserContext.exposeBinding: options.handle must be a boolean');
    return pageBindings.exposeContext(
      this, name, callback, 'browserContext.exposeBinding', options.handle === true,
    );
  }
  exposeFunction(name, callback) {
    if (typeof name !== 'string' || !name)
      throw new TypeError('browserContext.exposeFunction: name must be a non-empty string');
    if (typeof callback !== 'function')
      throw new TypeError('browserContext.exposeFunction: callback must be a function');
    return pageBindings.exposeContext(
      this, name, (_source, ...args) => callback(...args), 'browserContext.exposeFunction', false,
    );
  }
  route(url, handler, options = {}) { return pageRoutes.route(this, url, handler, options); }
  routeFromHAR(har, options = {}) { return pageRoutes.routeFromHAR(this, har, options); }
  unroute(url, handler) { return pageRoutes.unroute(this, url, handler); }
  unrouteAll(options = {}) { return pageRoutes.unrouteAll(this, options); }
  pages() {
    if (this._browser._session.pageSnapshot) {
      try {
        const snapshot = this._browser._session.pageSnapshot(this._id, [...this._pages.keys()]);
        const active = new Set(snapshot.ids);
        for (const [id, page] of this._pages) {
          if (active.has(id)) continue;
          this._retireCdpSessions(id);
          page._retireNetworkRequestCache();
          this._pages.delete(id);
        }
        for (const value of snapshot.newPages) this._wrapPage(value);
        return snapshot.ids.map(id => this._pages.get(id));
      } catch (error) { throw mapError(error, 'browserContext.pages'); }
    }
    const descriptors = this._browser._session.pages(this._id);
    const active = new Set(descriptors.map(value => value.id));
    for (const [id, page] of this._pages) {
      if (active.has(id)) continue;
      this._retireCdpSessions(id);
      page._retireNetworkRequestCache();
      this._pages.delete(id);
    }
    return descriptors.map(value => this._wrapPage(value));
  }
  _wrapPage(value) {
    if (!this._pages.has(value.id)) this._pages.set(value.id, new Page(this, value));
    return this._pages.get(value.id);
  }
  _resolveHandleFrame(id) {
    // Rust selects a portable Frame in this Context and publishes topology
    // first. This layer only retrieves its canonical cached wrapper. A Frame
    // retired between native settlement and lookup has no current owner.
    for (const page of this.pages()) {
      const frame = page.frames().find(value => value._id === id);
      if (frame) return frame;
    }
    return null;
  }
  async newPage(...args) {
    if (args.length) throw new UnsupportedOperationError('browserContext.newPage arguments');
    try {
      const page = this._wrapPage(await this._browser._session.newPage(this._id));
      await flushBrowserAfterPageCreation(this._browser, page, 'browserContext.newPage', 'Page');
      return page;
    }
    catch (error) { throw mapError(error, 'browserContext.newPage'); }
  }
  newCDPSession(page) { return cdpSessions.create(this, page, Page, Frame); }
  _flushCdpSessions(pageId) { return cdpSessions.flushPage(this, pageId); }
  _retireCdpSessions(pageId) { cdpSessions.retirePage(this, pageId); }
  _retireAllCdpSessions() { cdpSessions.retireContext(this); }
  async close(options = {}) {
    options = { ...options };
    validateOptions(options, ['reason'], 'browserContext.close');
    if (options.reason !== undefined && typeof options.reason !== 'string')
      throw new TypeError('browserContext.close: reason must be a string');
    this._cdpClosePending = (this._cdpClosePending ?? 0) + 1;
    try {
      await this._browser._session.closeContext(this._id);
      this._cdpClosed = true;
      this._retireAllCdpSessions();
      this._retireNetworkRequestCache();
      serviceWorkerEvents.retire(this);
      for (const page of this._pages.values()) page._retireNetworkRequestCache();
      this._request?._disposeFromOwner(options.reason ?? 'Browser context closed');
      this._pages.clear();
      this._browser._contexts.delete(this._id);
      if (!this._browser.isConnected()) this._browser._didDisconnect();
    } catch (error) { throw mapError(error, 'browserContext.close'); }
    finally { this._cdpClosePending -= 1; }
  }
}

class Page extends NativeConsoleEventEmitter {
  constructor(context, descriptor) {
    super(context._browser._session, descriptor.id, 'setPageConsoleListener'); this._context = context; this._id = descriptor.id;
    if (frameUrlExpectationSelected) this._apiName = 'Page';
    const metadata = descriptor.mainFrameBinding ?? (descriptor.frameTree
      ? descriptor.frameTree.metadata(descriptor.mainFrameId) : descriptor.mainFrameMetadata);
    this._mainFrame = new Frame(this, descriptor.mainFrameId, metadata, null);
    this._frameCache = descriptor.frameTree ? new FrameCache(descriptor.frameTree, this._mainFrame,
      (id, node, parent) => new Frame(this, id, node, parent)) : null;
    this.keyboard = new Keyboard(context._browser._session, this);
    this.mouse = new Mouse(context._browser._session, this);
    this.touchscreen = new Touchscreen(context._browser._session, this._id);
    this._opener = null;
    this._consoleMessages = [];
    this._bindings = new Map();
    this._routes = new Map();
    this._networkRequests = new Map();
  }
  _retireNetworkRequestCache() {
    // Do not mutate Request wrappers retained by callers: their immutable
    // metadata, cached Response, and redirect links remain readable.
    this._networkRequests.clear();
    this._networkRequestRefs?.clear();
    this._frameCache?.clearRetired();
  }
  get request() { return this._context.request; }
  context() { return this._context; }
  setDefaultTimeout(timeout) {
    timeoutSettings.setPage(this, timeout, false, 'page.setDefaultTimeout');
  }
  setDefaultNavigationTimeout(timeout) {
    timeoutSettings.setPage(this, timeout, true, 'page.setDefaultNavigationTimeout');
  }
  opener() { return Promise.resolve(this._opener); }
  video() {
    if (this._context._recordVideo === undefined) return null;
    return this._video ??= videoRecording.create(this);
  }
  waitForEvent(event, optionsOrPredicate = {}) {
    const apiStack = {};
    Error.captureStackTrace?.(apiStack, Page.prototype.waitForEvent);
    const session = this._context._browser._session;
    const timeout = typeof session.pageDefaultTimeout === 'function'
      ? session.pageDefaultTimeout(this._id, false)
      : 30_000;
    return runTracingAction(this, 'page-wait-for-event', event, () =>
      waitForEvent(
        this,
        event,
        optionsOrPredicate,
        'page.waitForEvent',
        timeout,
        { apiStack },
      ));
  }
  waitForRequest(urlOrPredicate, options = {}) {
    const apiStack = {};
    Error.captureStackTrace?.(apiStack, Page.prototype.waitForRequest);
    return requestEvents.wait(this, urlOrPredicate, options, apiStack);
  }
  waitForResponse(urlOrPredicate, options = {}) {
    const apiStack = {};
    Error.captureStackTrace?.(apiStack, Page.prototype.waitForResponse);
    return responseEvents.wait(this, urlOrPredicate, options, apiStack);
  }
  waitForLoadState(state = 'load', options = {}) {
    const apiStack = {};
    Error.captureStackTrace?.(apiStack, Page.prototype.waitForLoadState);
    return loadState.wait(this._mainFrame, state, options, 'page.waitForLoadState', apiStack);
  }
  async waitForNavigation(options = {}) {
    const api = 'page.waitForNavigation';
    validateOptions(options, ['waitUntil', 'timeout'], api);
    const timeout = options.timeout ?? 30_000;
    if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
      throw new TypeError(`${api}: timeout must be an integer in [0, 4294967295]`);
    const waitUntil = options.waitUntil ?? 'load';
    if (typeof waitUntil !== 'string') throw new TypeError(`${api}: waitUntil must be a string`);
    const session = this._context._browser._session;
    if (typeof session.frameWaitForNavigation !== 'function')
      throw new UnsupportedOperationError(`${api} (select frame-navigation native profile)`);
    try {
      const result = await session.frameWaitForNavigation(
        this._id, this._mainFrame._id, waitUntil, timeout,
      );
      return result.response ? createResponse(this, result.response) : null;
    } catch (error) { throw mapError(error, api); }
  }
  async consoleMessages() { return this._consoleMessages.slice(); }
  async exposeBinding(name, callback, options = {}) {
    validateOptions(options, ['handle'], 'page.exposeBinding');
    if (typeof name !== 'string' || !name) throw new TypeError('page.exposeBinding: name must be a non-empty string');
    if (typeof callback !== 'function') throw new TypeError('page.exposeBinding: callback must be a function');
    if (options.handle !== undefined && typeof options.handle !== 'boolean')
      throw new TypeError('page.exposeBinding: options.handle must be a boolean');
    return pageBindings.expose(this, name, callback, 'page.exposeBinding', options.handle === true);
  }
  exposeFunction(name, callback) {
    if (typeof name !== 'string' || !name) throw new TypeError('page.exposeFunction: name must be a non-empty string');
    if (typeof callback !== 'function') throw new TypeError('page.exposeFunction: callback must be a function');
    return pageBindings.expose(this, name, (_source, ...args) => callback(...args), 'page.exposeFunction', false);
  }
  route(url, handler, options = {}) {
    return runTracingAction(this, 'page-route', undefined,
      () => pageRoutes.route(this, url, handler, options));
  }
  routeFromHAR(har, options = {}) { return pageRoutes.routeFromHAR(this, har, options); }
  unroute(url, handler) { return pageRoutes.unroute(this, url, handler); }
  unrouteAll(options = {}) { return pageRoutes.unrouteAll(this, options); }
  addInitScript(script, arg) {
    return initScripts.add(this, script, arguments.length > 1, arg);
  }
  addScriptTag(options = {}) { return scriptTags.add(this._mainFrame, options); }
  $(selector) { return querySelector.one(this._mainFrame, selector, 'page.$'); }
  $$(selector) { return querySelector.all(this._mainFrame, selector, 'page.$$'); }
  waitForSelector(selector, options = {}) {
    return pageWaitForSelector.wait(this._mainFrame, selector, options, 'page.waitForSelector');
  }
  getAttribute(selector, name, options = {}) { return this.locator(selector).getAttribute(name, options); }
  innerHTML(selector, options = {}) { return this.locator(selector).innerHTML(options); }
  innerText(selector, options = {}) { return this.locator(selector).innerText(options); }
  inputValue(selector, options = {}) { return this.locator(selector).inputValue(options); }
  textContent(selector, options = {}) { return this.locator(selector).textContent(options); }
  isChecked(selector, options = {}) { return this.locator(selector).isChecked(options); }
  isDisabled(selector, options = {}) { return this.locator(selector).isDisabled(options); }
  isEditable(selector, options = {}) { return this.locator(selector).isEditable(options); }
  isEnabled(selector, options = {}) { return this.locator(selector).isEnabled(options); }
  isVisible(selector, options = {}) { return this.locator(selector).isVisible(options); }
  isHidden(selector, options = {}) { return this.locator(selector).isHidden(options); }
  waitForTimeout(timeout) {
    return runTracingAction(this, 'page-wait-for-timeout', undefined,
      () => waitForTimeout.wait(this._mainFrame, timeout, 'page.waitForTimeout'));
  }
  waitForFunction(pageFunction, arg, options = {}) {
    return waitForFunction.wait(this._mainFrame, pageFunction, arg, options, 'page.waitForFunction');
  }
  waitForURL(url, options = {}) {
    return waitForUrl.wait(this._mainFrame, url, options, 'page.waitForURL');
  }
  isClosed() {
    if (!this._frameCache?._tree.isClosed) throw new UnsupportedOperationError('page.isClosed (rebuild native addon)');
    try { return this._frameCache._tree.isClosed(); }
    catch (error) { throw mapError(error, 'page.isClosed'); }
  }
  async setExtraHTTPHeaders(headers) {
    const session = this._context._browser._session;
    if (!session.setPageExtraHttpHeaders) throw new UnsupportedOperationError('page.setExtraHTTPHeaders (select extra-headers native profile)');
    const pending = session.setPageExtraHttpHeaders(this._id, headers, headerChannelError);
    try { await pending; }
    catch (error) { throw mapError(error, 'page.setExtraHTTPHeaders'); }
  }
  mainFrame() { return this._mainFrame; }
  setInputFiles(selector, files, options = {}) {
    return fileInput.forFrame(this._context._browser._session, this._mainFrame._id,
      selector, files, options, 'page.setInputFiles');
  }
  frame(selector) {
    const name = typeof selector === 'string' || selector instanceof String ? selector : selector.name;
    const url = typeof selector === 'object' && selector !== null ? selector.url : undefined;
    if (!this._frameCache || !this._frameCache._tree.lookup || !this._context._settings)
      throw new UnsupportedOperationError('page.frame (rebuild native addon)');
    // Do not map arbitrary user-thrown values through the HostError adapter.
    return this._frameCache.lookup(name, url, this._context._settings);
  }
  frames() {
    if (!this._frameCache) throw new UnsupportedOperationError('page.frames (rebuild native addon)');
    try { return this._frameCache.frames(); }
    catch (error) { throw mapError(error, 'page.frames'); }
  }
  async screenshot(options = {}) {
    const params = screenshotOptions(options);
    try {
      return await runTracingAction(this, 'page-screenshot', undefined,
        () => this._context._browser._session.pageScreenshot(this._id, params));
    }
    catch (error) { throw mapError(error, 'page.screenshot'); }
  }
  pdf(options = {}) { return pagePdf(this._context._browser._session, this._id, options); }
  async goto(url, options = {}) {
    validateOptions(options, ['waitUntil', 'timeout', 'referer'], 'page.goto');
    if (typeof url !== 'string') throw new TypeError('page.goto: url must be a string');
    const waitUntil = navigationWaitUntil(options.waitUntil, 'page.goto');
    const session = this._context._browser._session;
    if (options.referer !== undefined && typeof options.referer !== 'string')
      throw new TypeError('page.goto: referer must be a string');
    let response;
    try {
      const timeout = navigationTimeout(session, this._id, options.timeout);
      if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
        throw new TypeError('page.goto: timeout must be an integer in [0, 4294967295]');
      if (this._context._settings) url = this._context._settings.resolveUrl(url);
      const result = await runTracingAction(this, 'page-goto', url, () =>
        typeof session.frameGoto === 'function'
          ? session.frameGoto([this._id, this._mainFrame._id], url, waitUntil, timeout, options.referer)
          : session.pageGoto(this._id, url, waitUntil, timeout, options.referer));
      response = result.response ? createResponse(this, result.response) : null;
    } catch (error) { throw mapNavigationError(error, 'page.goto', url, waitUntil); }
    await this._context._flushCdpSessions(this._id);
    return response;
  }
  async title() { return this._mainFrame._read('frameTitle', 'page.title'); }
  async reload(options = {}) {
    validateOptions(options, ['waitUntil', 'timeout'], 'page.reload');
    const waitUntil = navigationWaitUntil(options.waitUntil, 'page.reload');
    const session = this._context._browser._session;
    try {
      const timeout = navigationTimeout(session, this._id, options.timeout);
      if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
        throw new TypeError('page.reload: timeout must be an integer in [0, 4294967295]');
      const result = await runTracingAction(this, 'page-reload', undefined,
        () => session.pageReload(this._id, waitUntil, timeout));
      return result.response ? createResponse(this, result.response) : null;
    } catch (error) { throw mapError(error, 'page.reload'); }
  }
  async goBack(options = {}) {
    validateOptions(options, ['waitUntil', 'timeout'], 'page.goBack');
    const timeout = options.timeout ?? 30_000;
    if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
      throw new TypeError('page.goBack: timeout must be an integer in [0, 4294967295]');
    if (options.waitUntil !== undefined && typeof options.waitUntil !== 'string')
      throw new TypeError('page.goBack: waitUntil must be a string');
    const session = this._context._browser._session;
    if (!session.pageGoBack)
      throw new UnsupportedOperationError('page.goBack (select history-navigation native profile)');
    try {
      const result = await session.pageGoBack(this._id, options.waitUntil ?? 'load', timeout);
      return result?.response ? createResponse(this, result.response) : null;
    } catch (error) { throw mapError(error, 'page.goBack'); }
  }
  async goForward(options = {}) {
    validateOptions(options, ['waitUntil', 'timeout'], 'page.goForward');
    const waitUntil = navigationWaitUntil(options.waitUntil, 'page.goForward');
    const session = this._context._browser._session;
    if (!session.pageGoForward)
      throw new UnsupportedOperationError('page.goForward (select history-navigation native profile)');
    try {
      const timeout = navigationTimeout(session, this._id, options.timeout);
      if (!Number.isInteger(timeout) || timeout < 0 || timeout > 0xffff_ffff)
        throw new TypeError('page.goForward: timeout must be an integer in [0, 4294967295]');
      const result = await session.pageGoForward(this._id, waitUntil, timeout);
      return result?.response ? createResponse(this, result.response) : null;
    } catch (error) { throw mapError(error, 'page.goForward'); }
  }
  async content() { return this._mainFrame._read('frameContent', 'page.content'); }
  setContent(html, options = {}) {
    return runTracingAction(this, 'page-set-content', undefined,
      () => pageActions.setContent(this._context._browser._session, this._id, html, options));
  }
  emulateMedia(options = {}) {
    return emulateMedia(this._context._browser._session, this._id, options);
  }
  setViewportSize(viewport) {
    return pageViewport.set(this._context._browser._session, this._id, viewport);
  }
  requestGC() { return requestGC(this._context._browser._session, this._id); }
  press(selector, key, options = {}) { return pageActions.press(this._mainFrame, selector, key, options, 'page.press'); }
  type(selector, text, options = {}) { return pageActions.type(this._mainFrame, selector, text, options, 'page.type'); }
  focus(selector, options = {}) { return pageActions.focus(this._mainFrame, selector, options, 'page.focus'); }
  click(selector, options = {}) {
    return runTracingAction(this, 'page-click', undefined,
      () => pageActions.click(this._mainFrame, selector, options, 'page.click'));
  }
  dblclick(selector, options = {}) {
    return runTracingAction(this, 'page-dblclick', undefined,
      () => pageActions.dblclick(this._mainFrame, selector, options, 'page.dblclick'));
  }
  hover(selector, options = {}) {
    return runTracingAction(this, 'page-hover', undefined,
      () => pageActions.hover(this._mainFrame, selector, options, 'page.hover'));
  }
  tap(selector, options = {}) { return pageActions.tap(this._mainFrame, selector, options, 'page.tap'); }
  check(selector, options = {}) { return pageActions.check(this._mainFrame, selector, options, 'page.check'); }
  uncheck(selector, options = {}) { return pageActions.uncheck(this._mainFrame, selector, options, 'page.uncheck'); }
  setChecked(selector, checked, options = {}) { return pageActions.setChecked(this._mainFrame, selector, checked, options, 'page.setChecked'); }
  dispatchEvent(selector, type, eventInit, options = {}) {
    return pageActions.dispatchEvent(this._mainFrame, selector, type, eventInit, options, 'page.dispatchEvent');
  }
  fill(selector, value, options = {}) { return pageActions.fill(this._mainFrame, selector, value, options, 'page.fill'); }
  selectOption(selector, values, options = {}) { return this.locator(selector).selectOption(values, options); }
  url() { return this._mainFrame.url(); }
  async evaluate(pageFunction, arg) {
    if (arguments.length > 2) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return runTracingAction(this, 'page-evaluate', undefined,
      () => this._mainFrame._evaluate(pageFunction, arg, 'page.evaluate'));
  }
  async evaluateHandle(pageFunction, arg) {
    if (arguments.length > 2) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return runTracingAction(this, 'page-evaluate-handle', undefined,
      () => this._mainFrame._evaluateHandle(pageFunction, arg, 'page.evaluateHandle'));
  }
  async _evaluateFunction(functionDeclaration) {
    return this._mainFrame._evaluate(functionDeclaration, undefined, 'page._evaluateFunction', true);
  }
  async $eval(selector, pageFunction, arg) {
    if (arguments.length > 3) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return this._mainFrame.$eval(selector, pageFunction, arg);
  }
  async $$eval(selector, pageFunction, arg) {
    if (arguments.length > 3) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return this._mainFrame.$$eval(selector, pageFunction, arg);
  }
  async close(options = {}) {
    validateOptions(options, ['runBeforeUnload'], 'page.close');
    if (options.runBeforeUnload !== undefined && typeof options.runBeforeUnload !== 'boolean')
      throw new TypeError('page.close: runBeforeUnload must be a boolean');
    if (options.runBeforeUnload) throw new UnsupportedOperationError('page.close runBeforeUnload');
    return runTracingAction(this, 'page-close', undefined, async () => {
      this._cdpClosePending = (this._cdpClosePending ?? 0) + 1;
      try {
        await this._context._browser._session.closePage(this._id);
        this._cdpClosed = true;
        this._context._retireCdpSessions(this._id);
        this._retireNetworkRequestCache();
        this._context._pages.delete(this._id);
      } catch (error) { throw mapError(error, 'page.close'); }
      finally { this._cdpClosePending -= 1; }
    });
  }
  locator(selector, options = {}) { return this._mainFrame.locator(selector, options); }
  frameLocator(selector) { return this._mainFrame.frameLocator(selector); }
  getByText(value, options) { return this._mainFrame.getByText(value, options); }
  getByLabel(value, options) { return this._mainFrame.getByLabel(value, options); }
  getByPlaceholder(value, options) { return this._mainFrame.getByPlaceholder(value, options); }
  getByAltText(value, options) { return this._mainFrame.getByAltText(value, options); }
  getByTitle(value, options) { return this._mainFrame.getByTitle(value, options); }
  getByTestId(value) { return this._mainFrame.getByTestId(value); }
  getByRole(value, options) { return this._mainFrame.getByRole(value, options); }
}

class Frame {
  constructor(page, id, metadata, parent) {
    this._page = page; this._id = id; this._metadata = metadata; this._parent = parent;
  }
  _expect(expression, options = {}) { return frameUrlExpectation.expect(this, expression, options); }
  page() { return this._page; }
  getAttribute(selector, name, options = {}) { return this.locator(selector).getAttribute(name, options); }
  innerHTML(selector, options = {}) { return this.locator(selector).innerHTML(options); }
  innerText(selector, options = {}) { return this.locator(selector).innerText(options); }
  inputValue(selector, options = {}) { return this.locator(selector).inputValue(options); }
  textContent(selector, options = {}) { return this.locator(selector).textContent(options); }
  isChecked(selector, options = {}) { return this.locator(selector).isChecked(options); }
  isDisabled(selector, options = {}) { return this.locator(selector).isDisabled(options); }
  isEditable(selector, options = {}) { return this.locator(selector).isEditable(options); }
  isEnabled(selector, options = {}) { return this.locator(selector).isEnabled(options); }
  isVisible(selector, options = {}) { return this.locator(selector).isVisible(options); }
  isHidden(selector, options = {}) { return this.locator(selector).isHidden(options); }
  setInputFiles(selector, files, options = {}) {
    return fileInput.forFrame(this._page._context._browser._session, this._id,
      selector, files, options, 'frame.setInputFiles');
  }
  async goto(url, options = {}) {
    validateOptions(options, ['waitUntil', 'timeout', 'referer'], 'frame.goto');
    if (typeof url !== 'string') throw new TypeError('frame.goto: url must be a string');
    const waitUntil = navigationWaitUntil(options.waitUntil, 'frame.goto');
    const context = this._page._context;
    const session = context._browser._session;
    if (options.timeout != null && (!Number.isInteger(options.timeout) || options.timeout < 0 || options.timeout > 0xffff_ffff))
      throw new TypeError('frame.goto: timeout must be an integer in [0, 4294967295]');
    if (options.referer !== undefined && typeof options.referer !== 'string')
      throw new TypeError('frame.goto: referer must be a string');
    if (!session.frameGoto) throw new UnsupportedOperationError('frame.goto (select frame-navigation native profile)');
    try {
      const timeout = navigationTimeout(session, this._page._id, options.timeout);
      if (context._settings) url = context._settings.resolveUrl(url);
      const result = await session.frameGoto([this._page._id, this._id], url, waitUntil, timeout, options.referer);
      return result.response ? createResponse(this._page, result.response, this) : null;
    } catch (error) { throw mapNavigationError(error, 'frame.goto', url, waitUntil); }
  }
  name() {
    if (!this._metadata.name) throw new UnsupportedOperationError('frame.name (rebuild native addon)');
    try { return this._metadata.name(); }
    catch (error) { throw mapError(error, 'frame.name'); }
  }
  parentFrame() { return this._parent; }
  childFrames() { return this._page.frames().filter(frame => frame.parentFrame() === this); }
  isDetached() {
    if (!this._metadata.isDetached) throw new UnsupportedOperationError('frame.isDetached (rebuild native addon)');
    return this._metadata.isDetached();
  }
  async frameElement() {
    const session = this._page._context._browser._session;
    if (typeof session.frameElement !== 'function')
      throw new UnsupportedOperationError('frame.frameElement (select frame-element native profile)');
    try { return await this._wrapNativeHandle(await session.frameElement(this._id)); }
    catch (error) { throw mapError(error, 'frame.frameElement'); }
  }
  locator(selector, options = {}) { return locatorApi.create(this, selector, options); }
  frameLocator(selector) { return frameLocatorApi.create(this, selector); }
  getByText(value, options) { return locatorApi.semantic(this, 'text', value, options); }
  getByLabel(value, options) { return locatorApi.semantic(this, 'label', value, options); }
  getByPlaceholder(value, options) { return locatorApi.semantic(this, 'placeholder', value, options); }
  getByAltText(value, options) { return locatorApi.semantic(this, 'altText', value, options); }
  getByTitle(value, options) { return locatorApi.semantic(this, 'title', value, options); }
  getByTestId(value) { return locatorApi.semantic(this, 'testId', value); }
  getByRole(value, options) { return locatorApi.role(this, value, options); }
  url() { return this._metadata.url(); }
  playwrightRustMetadataMetrics() { return { unpublishedUrl: this._metadata.hasUnpublishedUrl() }; }
  async _read(method, api) {
    try { return await this._page._context._browser._session[method](this._id); }
    catch (error) { throw mapError(error, api); }
  }
  async title() { return this._read('frameTitle', 'frame.title'); }
  async content() { return this._read('frameContent', 'frame.content'); }
  addScriptTag(options = {}) { return scriptTags.add(this, options); }
  $(selector) { return querySelector.one(this, selector, 'frame.$'); }
  $$(selector) { return querySelector.all(this, selector, 'frame.$$'); }
  waitForSelector(selector, options = {}) {
    return pageWaitForSelector.wait(this, selector, options, 'frame.waitForSelector');
  }
  waitForTimeout(timeout) { return waitForTimeout.wait(this, timeout, 'frame.waitForTimeout'); }
  waitForFunction(pageFunction, arg, options = {}) {
    return waitForFunction.wait(this, pageFunction, arg, options, 'frame.waitForFunction');
  }
  waitForURL(url, options = {}) { return waitForUrl.wait(this, url, options, 'frame.waitForURL'); }
  waitForLoadState(state = 'load', options = {}) {
    const apiStack = {};
    Error.captureStackTrace?.(apiStack, Frame.prototype.waitForLoadState);
    return loadState.wait(this, state, options, 'frame.waitForLoadState', apiStack);
  }
  press(selector, key, options = {}) { return pageActions.press(this, selector, key, options, 'frame.press'); }
  type(selector, text, options = {}) { return pageActions.type(this, selector, text, options, 'frame.type'); }
  focus(selector, options = {}) { return pageActions.focus(this, selector, options, 'frame.focus'); }
  click(selector, options = {}) { return pageActions.click(this, selector, options, 'frame.click'); }
  dblclick(selector, options = {}) { return pageActions.dblclick(this, selector, options, 'frame.dblclick'); }
  hover(selector, options = {}) { return pageActions.hover(this, selector, options, 'frame.hover'); }
  tap(selector, options = {}) { return pageActions.tap(this, selector, options, 'frame.tap'); }
  fill(selector, value, options = {}) { return this.locator(selector).fill(value, options); }
  selectOption(selector, values, options = {}) { return this.locator(selector).selectOption(values, options); }
  async evaluate(pageFunction, arg) {
    if (arguments.length > 2) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return this._evaluate(pageFunction, arg, 'frame.evaluate');
  }
  async evaluateHandle(pageFunction, arg) {
    if (arguments.length > 2) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return this._evaluateHandle(pageFunction, arg, 'frame.evaluateHandle');
  }
  async $eval(selector, pageFunction, arg) {
    if (arguments.length > 3) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return this.locator(selector)._evaluateFirstFunction(pageFunction, arg);
  }
  async $$eval(selector, pageFunction, arg) {
    if (arguments.length > 3) throw new Error('Too many arguments. If you need to pass more than 1 argument to the function wrap them in an object.');
    return this.locator(selector).evaluateAll(pageFunction, arg);
  }
  async _evaluateHandle(pageFunction, arg, api) {
    const context = this._page._context;
    const session = context._browser._session;
    const scoped = typeof session.frameEvaluateHandleWithPageTimeout === 'function';
    if (!scoped && typeof session.frameEvaluateHandle !== 'function') throw new UnsupportedOperationError(api);
    try {
      // Forward identity only. Rust selects canonical Page policy and orders
      // any lookup failure after normal argument materialization.
      const native = scoped
        ? await session.frameEvaluateHandleWithPageTimeout(this._id, String(pageFunction), typeof pageFunction === 'function', arg, this._page._id)
        : await session.frameEvaluateHandle(this._id, String(pageFunction), typeof pageFunction === 'function', arg);
      return await this._wrapNativeHandle(native);
    } catch (error) { throw this._mapEvaluationError(error, api); }
  }
  async _wrapNativeHandle(native) {
    const context = this._page._context;
    return await handles.create(
      context._browser._session,
      native,
      id => context._resolveHandleFrame(id),
    );
  }
  async _wrapNativeHandles(native) {
    const context = this._page._context;
    return await handles.createList(
      context._browser._session,
      native,
      id => context._resolveHandleFrame(id),
    );
  }
  async _evaluate(pageFunction, arg, api, forceFunction = false) {
    try {
      return await this._page._context._browser._session.frameEvaluate(this._id, String(pageFunction), forceFunction || typeof pageFunction === 'function', arg);
    } catch (error) { throw this._mapEvaluationError(error, api); }
  }
  _mapEvaluationError(error, api) {
    const mapped = mapError(error, api);
    if (mapped.code !== 'PLAYWRIGHT_RUST_TARGET_CLOSED' || !api.startsWith('frame.')) return mapped;
    try {
      if (!this._metadata.isDetached?.()) return mapped;
    } catch (_) {
      return mapped;
    }
    const detached = new Error(`${api}: Frame was detached`);
    detached.code = mapped.code;
    return detached;
  }
}

function createResponse(owner, metadata, frame, requestOverride) {
  const page = owner._context ? owner : null;
  const context = page?._context ?? owner;
  const request = requestOverride ?? responseRequests.create(owner, metadata.request, metadata.historyOwner);
  if (request?._response) return request._response;
  if (frame === undefined && page) frame = request?.frame() ?? page.mainFrame();
  const response = new Response(context, page, metadata, frame ?? null, request);
  request?._setResponse(response);
  return response;
}

class Response {
  constructor(context, page, metadata, frame, request) {
    this._context = context;
    this._page = page;
    this._metadata = metadata;
    this._frame = frame;
    this._request = request;
    this._finished = responseFinished.create(
      context._browser._session,
      metadata.finishedOwner,
    );
    this._body = page && frame ? responseBodies.create(
      context._browser._session,
      page._id,
      frame._id,
      metadata.nativeRequestId ?? metadata.requestId,
      this._finished.beforeBody,
    ) : {
      body: () => Promise.reject(new UnsupportedOperationError('response.body for a Service Worker request')),
      text: () => Promise.reject(new UnsupportedOperationError('response.text for a Service Worker request')),
      json: () => Promise.reject(new UnsupportedOperationError('response.json for a Service Worker request')),
    };
  }
  url() { return this._metadata.url; }
  status() { return this._metadata.status; }
  statusText() { return this._metadata.statusText; }
  ok() { return this._metadata.ok; }
  headers() { return { ...this._metadata.headers }; }
  allHeaders() {
    return responseHeaders.all(this._context._browser._session, this._metadata.historyOwner);
  }
  headerValue(name) {
    return responseHeaders.value(this._context._browser._session, this._metadata.historyOwner, name);
  }
  request() { return responseRequests.get(this._request); }
  frame() { return this._frame ?? this._request?.frame() ?? null; }
  fromServiceWorker() { return this._metadata.fromServiceWorker === true; }
  body() { return this._body.body(); }
  text() { return this._body.text(); }
  json() { return this._body.json(); }
  finished() { return this._finished.finished(); }
}

const chromium = new BrowserType();
const implementation = { chromium, request, selectors, devices, errors: Object.freeze({ TimeoutError, UnsupportedOperationError }), compatibility };
// The official Playwright fixture writes this on Object.create(implementation).
// An inherited setter keeps its defaults scoped to this module and off the
// public enumerable API; the fixture clears them after each worker scope.
Object.defineProperty(implementation, '_defaultLaunchOptions', {
  get() { return defaultLaunchOptions; },
  set(options) { defaultLaunchOptions = options; },
});
module.exports = implementation;
