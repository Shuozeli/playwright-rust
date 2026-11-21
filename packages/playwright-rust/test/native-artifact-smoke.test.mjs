import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const {
  nativeAbiVersion,
  nativeArtifact,
  nativeFeatures,
} = require('../src/native-artifact.cjs');

async function inspectNativeArtifactInChild() {
  const child = spawn(process.execPath, [
    fileURLToPath(new URL('./native-artifact-smoke-child.mjs', import.meta.url)),
    nativeArtifact.path,
    nativeArtifact.profile,
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '';
  let stderr = '';
  let timer;
  try {
    return await new Promise((resolve, reject) => {
      child.stdout.setEncoding('utf8');
      child.stderr.setEncoding('utf8');
      child.stdout.on('data', (chunk) => { stdout += chunk; });
      child.stderr.on('data', (chunk) => { stderr += chunk; });
      timer = setTimeout(() => reject(new Error('native artifact child did not respond within 20 seconds')), 20000);
      child.once('error', reject);
      child.once('exit', (code, signal) => {
        if (code !== 0) {
          reject(new Error(`native artifact child exited with code ${code}, signal ${signal}: ${stderr.trim()}`));
          return;
        }
        try {
          resolve(JSON.parse(stdout));
        } catch (error) {
          reject(new Error(`native artifact child returned invalid JSON: ${stdout}`, { cause: error }));
        }
      });
    });
  } finally {
    clearTimeout(timer);
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
  }
}

test('built native artifact exports the current ABI and core environment', { timeout: 25000 }, async () => {
  // Arrange
  const features = nativeFeatures(nativeArtifact.profile);
  const expectsHarReplay = features.includes('har-replay') || features.includes('har-recording');
  const expected = {
    abi: nativeAbiVersion,
    environment: 'function',
    profile: nativeArtifact.profile,
    traceDirEnabled: features.includes('playwright-trace'),
    burst20Enabled: features.includes('frame-navigation-burst-20'),
    fillNavigationCommitEnabled: features.includes('fill-navigation-commit'),
    routeRequestHistoryEnabled: features.includes('route-request-history'),
    contextCookiesEnabled: features.includes('context-cookies'),
    contextLocaleEnabled: features.includes('context-locale'),
    contextTimezoneEnabled: features.includes('context-timezone'),
    contextGeolocationEnabled: features.includes('context-geolocation'),
    contextPermissionsEnabled: features.includes('context-geolocation-permission'),
    contextHttpCredentialsEnabled: features.includes('context-http-credentials'),
    contextBindingsEnabled: features.includes('context-bindings'),
    ignoreHttpsErrorsEnabled: features.includes('ignore-https-errors'),
    largeEvaluationInputsEnabled: features.includes('large-evaluation-inputs'),
    largeRoutePostDataEnabled: features.includes('large-route-post-data'),
    elementHandleScrollIntoViewEnabled: features.includes('element-handle-scroll-into-view'),
    elementHandleWaitForSelectorEnabled: features.includes('element-handle-wait-for-selector'),
    elementHandleSelectOptionValueEnabled: features.includes('element-handle-select-option-value'),
    lightSelectorsEnabled: features.includes('light-selectors'),
    frameEventsEnabled: features.includes('frame-events'),
    harReplayEnabled: expectsHarReplay,
    harRecordingEnabled: features.includes('har-recording'),
    harRecordingRegExpEnabled: features.includes('har-recording-regexp'),
    cdpSessionLease: features.includes('cdp-session') ? 'function' : 'undefined',
    cdpEventBatch: features.includes('cdp-session') ? 'function' : 'undefined',
    methods: {
      cdpSessionCreate: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpBrowserSessionCreate: features.includes('browser-cdp-session') ? 'function' : 'undefined',
      cdpSessionSend: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpSessionClaim: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpSessionDetach: features.includes('cdp-session') ? 'function' : 'undefined',
      nextCdpSessionEventGeneration: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpReportCallbackException: 'function',
      cdpLeaseToken: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpLeaseClaim: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpLeaseAbort: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpBatchPeek: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpBatchAckNext: features.includes('cdp-session') ? 'function' : 'undefined',
      cdpBatchRollback: features.includes('cdp-session') ? 'function' : 'undefined',
      launchChromiumWithTracesDir: features.includes('playwright-trace') ? 'function' : 'undefined',
      contextRequest: features.includes('context-api-request') ? 'function' : 'undefined',
      contextCookies: features.includes('context-cookies') ? 'function' : 'undefined',
      contextAddCookies: features.includes('context-cookies') ? 'function' : 'undefined',
      contextClearCookies: features.includes('context-cookies') ? 'function' : 'undefined',
      contextStorageState: features.includes('context-storage-state') ? 'function' : 'undefined',
      contextSetStorageState: features.includes('context-storage-state') ? 'function' : 'undefined',
      newContextWithIgnoreHttpsErrors: features.includes('ignore-https-errors') ? 'function' : 'undefined',
      newContextWithLocale: features.includes('context-locale') ? 'function' : 'undefined',
      newContextWithPolicies: features.some(feature => [
        'extra-headers', 'context-locale', 'context-timezone', 'context-geolocation', 'context-geolocation-permission', 'context-http-credentials', 'context-offline', 'context-proxy',
        'ignore-https-errors', 'bypass-csp',
      ].includes(feature)) ? 'function' : 'undefined',
      newContextWithPoliciesAndExtraHttpHeaders: features.includes('extra-headers') ? 'function' : 'undefined',
      setContextGeolocation: features.includes('context-geolocation') ? 'function' : 'undefined',
      setContextHttpCredentials: features.includes('context-http-credentials') ? 'function' : 'undefined',
      contextExposeBinding: features.includes('context-bindings') ? 'function' : 'undefined',
      grantContextPermissions: features.includes('context-geolocation-permission') ? 'function' : 'undefined',
      clearContextPermissions: features.includes('context-geolocation-permission') ? 'function' : 'undefined',
      newContextWithDeviceOptionsAndIgnoreHttpsErrors: features.includes('ignore-https-errors')
        && features.includes('device-emulation') ? 'function' : 'undefined',
      pageGoBack: features.includes('history-navigation') ? 'function' : 'undefined',
      pageGoForward: features.includes('history-navigation') ? 'function' : 'undefined',
      pagePdf: features.includes('page-pdf') ? 'function' : 'undefined',
      newContextWithHasTouch: features.includes('device-emulation') ? 'function' : 'undefined',
      newContextWithDeviceOptions: features.includes('device-emulation') ? 'function' : 'undefined',
      touchscreenTap: features.includes('touchscreen-input') ? 'function' : 'undefined',
      elementHandleTap: features.includes('touchscreen-input') && features.includes('element-handle-dom')
        ? 'function'
        : 'undefined',
      elementHandleScrollIntoViewIfNeeded: features.includes('element-handle-scroll-into-view')
        ? 'function'
        : 'undefined',
      elementHandleWaitForSelector: features.includes('element-handle-wait-for-selector')
        ? 'function'
        : 'undefined',
      elementHandleSelectOptionValue: features.includes('element-handle-select-option-value')
        ? 'function'
        : 'undefined',
      frameElement: features.includes('frame-element') ? 'function' : 'undefined',
      locatorWaitForElement: features.includes('handles') ? 'function' : 'undefined',
      locatorWaitForSelectorElement: features.includes('handles') ? 'function' : 'undefined',
      ...(['official-conformance-page-cdp-session', 'official-conformance-page-dispatch-event', 'official-conformance-page-goto-burst-20', 'official-conformance-page-goto-frame-detach', 'official-conformance-page-route-header-url', 'official-conformance-page-route-header-url-unroute', 'official-conformance-page-fill-navigation-commit'].includes(nativeArtifact.profile)
        ? { elementHandleDispatchEvent: 'function' }
        : {}),
      pageRouteContinueWithHeaders: features.includes('route-header-url-overrides') ? 'function' : 'undefined',
      pageRouteFallbackWithUrl: features.includes('route-header-url-overrides') ? 'function' : 'undefined',
      pageRouteFallbackWithOverrides: features.includes('route-header-url-overrides') ? 'function' : 'undefined',
      pageUnroute: features.includes('page-unroute') ? 'function' : 'undefined',
      responseRawHeaders: features.includes('route-request-history') ? 'function' : 'undefined',
      newContextWithProxy: features.includes('context-proxy') ? 'function' : 'undefined',
      pageRouteFromHar: expectsHarReplay ? 'function' : 'undefined',
      contextRouteFromHar: expectsHarReplay ? 'function' : 'undefined',
      pageRouteFromHarRegExp: expectsHarReplay && features.includes('route-regexp') ? 'function' : 'undefined',
      contextRouteFromHarRegExp: expectsHarReplay && features.includes('route-regexp') ? 'function' : 'undefined',
      pageRecordHar: features.includes('har-recording') ? 'function' : 'undefined',
      contextRecordHar: features.includes('har-recording') ? 'function' : 'undefined',
      pageRecordHarRegExp: features.includes('har-recording-regexp') ? 'function' : 'undefined',
      contextRecordHarRegExp: features.includes('har-recording-regexp') ? 'function' : 'undefined',
    },
  };
  // Act
  const result = await inspectNativeArtifactInChild();
  // Assert
  assert.deepEqual(result, expected);
});
