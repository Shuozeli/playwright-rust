import { writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const [artifact, profile] = process.argv.slice(2);
const {
  resolveNativeArtifact,
  verifyNativeArtifact,
} = require('../src/native-artifact.cjs');
const addon = require(artifact);
verifyNativeArtifact(addon, resolveNativeArtifact(profile));
writeFileSync(process.stdout.fd, JSON.stringify({
  abi: addon.nativeAbiVersion(),
  environment: typeof addon.NativeEnvironment,
  profile,
  traceDirEnabled: typeof addon.playwrightTraceDirEnabled === 'function'
    && addon.playwrightTraceDirEnabled(),
  burst20Enabled: typeof addon.frameNavigationBurst20Enabled === 'function'
    && addon.frameNavigationBurst20Enabled(),
  fillNavigationCommitEnabled: typeof addon.fillNavigationCommitEnabled === 'function'
    && addon.fillNavigationCommitEnabled(),
  routeRequestHistoryEnabled: typeof addon.routeRequestHistoryEnabled === 'function'
    && addon.routeRequestHistoryEnabled(),
  contextCookiesEnabled: typeof addon.contextCookiesEnabled === 'function'
    && addon.contextCookiesEnabled(),
  contextLocaleEnabled: typeof addon.contextLocaleEnabled === 'function'
    && addon.contextLocaleEnabled(),
  contextTimezoneEnabled: typeof addon.contextTimezoneEnabled === 'function'
    && addon.contextTimezoneEnabled(),
  contextGeolocationEnabled: typeof addon.contextGeolocationEnabled === 'function'
    && addon.contextGeolocationEnabled(),
  contextPermissionsEnabled: typeof addon.contextGeolocationPermissionEnabled === 'function'
    && addon.contextGeolocationPermissionEnabled(),
  contextHttpCredentialsEnabled: typeof addon.contextHttpCredentialsEnabled === 'function'
    && addon.contextHttpCredentialsEnabled(),
  contextBindingsEnabled: typeof addon.contextBindingsEnabled === 'function'
    && addon.contextBindingsEnabled(),
  ignoreHttpsErrorsEnabled: typeof addon.ignoreHttpsErrorsEnabled === 'function'
    && addon.ignoreHttpsErrorsEnabled(),
  largeEvaluationInputsEnabled: typeof addon.largeEvaluationInputsEnabled === 'function'
    && addon.largeEvaluationInputsEnabled(),
  largeRoutePostDataEnabled: typeof addon.largeRoutePostDataEnabled === 'function'
    && addon.largeRoutePostDataEnabled(),
  elementHandleScrollIntoViewEnabled: typeof addon.elementHandleScrollIntoViewEnabled === 'function'
    && addon.elementHandleScrollIntoViewEnabled(),
  elementHandleWaitForSelectorEnabled: typeof addon.elementHandleWaitForSelectorEnabled === 'function'
    && addon.elementHandleWaitForSelectorEnabled(),
  elementHandleSelectOptionValueEnabled: typeof addon.elementHandleSelectOptionValueEnabled === 'function'
    && addon.elementHandleSelectOptionValueEnabled(),
  lightSelectorsEnabled: typeof addon.lightSelectorsEnabled === 'function'
    && addon.lightSelectorsEnabled(),
  frameEventsEnabled: typeof addon.frameEventsEnabled === 'function'
    && addon.frameEventsEnabled(),
  harReplayEnabled: typeof addon.harReplayEnabled === 'function'
    && addon.harReplayEnabled(),
  harRecordingEnabled: typeof addon.harRecordingEnabled === 'function'
    && addon.harRecordingEnabled(),
  harRecordingRegExpEnabled: typeof addon.harRecordingRegExpEnabled === 'function'
    && addon.harRecordingRegExpEnabled(),
  cdpSessionLease: typeof addon.NativeCdpSessionLease,
  cdpEventBatch: typeof addon.NativeCdpEventBatch,
  methods: {
    cdpSessionCreate: typeof addon.NativeSession.prototype.cdpSessionCreate,
    cdpBrowserSessionCreate: typeof addon.NativeSession.prototype.cdpBrowserSessionCreate,
    cdpSessionSend: typeof addon.NativeSession.prototype.cdpSessionSend,
    cdpSessionClaim: typeof addon.NativeSession.prototype.cdpSessionClaim,
    cdpSessionDetach: typeof addon.NativeSession.prototype.cdpSessionDetach,
    nextCdpSessionEventGeneration: typeof addon.NativeSession.prototype.nextCdpSessionEventGeneration,
    cdpReportCallbackException: typeof addon.NativeSession.prototype.reportCallbackException,
    cdpLeaseToken: typeof addon.NativeCdpSessionLease?.prototype?.token,
    cdpLeaseClaim: typeof addon.NativeCdpSessionLease?.prototype?.claim,
    cdpLeaseAbort: typeof addon.NativeCdpSessionLease?.prototype?.abort,
    cdpBatchPeek: typeof addon.NativeCdpEventBatch?.prototype?.peek,
    cdpBatchAckNext: typeof addon.NativeCdpEventBatch?.prototype?.ackNext,
    cdpBatchRollback: typeof addon.NativeCdpEventBatch?.prototype?.rollback,
    launchChromiumWithTracesDir: typeof addon.NativeEnvironment.prototype.launchChromiumWithTracesDir,
    contextRequest: typeof addon.NativeSession.prototype.contextRequest,
    contextCookies: typeof addon.NativeSession.prototype.contextCookies,
    contextAddCookies: typeof addon.NativeSession.prototype.contextAddCookies,
    contextClearCookies: typeof addon.NativeSession.prototype.contextClearCookies,
    contextStorageState: typeof addon.NativeSession.prototype.contextStorageState,
    contextSetStorageState: typeof addon.NativeSession.prototype.contextSetStorageState,
    newContextWithIgnoreHttpsErrors: typeof addon.NativeSession.prototype.newContextWithIgnoreHttpsErrors,
    newContextWithLocale: typeof addon.NativeSession.prototype.newContextWithLocale,
    newContextWithPolicies: typeof addon.NativeSession.prototype.newContextWithPolicies,
    newContextWithPoliciesAndExtraHttpHeaders: typeof addon.NativeSession.prototype.newContextWithPoliciesAndExtraHttpHeaders,
    setContextGeolocation: typeof addon.NativeSession.prototype.setContextGeolocation,
    setContextHttpCredentials: typeof addon.NativeSession.prototype.setContextHttpCredentials,
    contextExposeBinding: typeof addon.NativeSession.prototype.contextExposeBinding,
    grantContextPermissions: typeof addon.NativeSession.prototype.grantContextPermissions,
    clearContextPermissions: typeof addon.NativeSession.prototype.clearContextPermissions,
    newContextWithDeviceOptionsAndIgnoreHttpsErrors: typeof addon.NativeSession.prototype.newContextWithDeviceOptionsAndIgnoreHttpsErrors,
    pageGoBack: typeof addon.NativeSession.prototype.pageGoBack,
    pageGoForward: typeof addon.NativeSession.prototype.pageGoForward,
    pagePdf: typeof addon.NativeSession.prototype.pagePdf,
    newContextWithHasTouch: typeof addon.NativeSession.prototype.newContextWithHasTouch,
    newContextWithDeviceOptions: typeof addon.NativeSession.prototype.newContextWithDeviceOptions,
    touchscreenTap: typeof addon.NativeSession.prototype.touchscreenTap,
    elementHandleTap: typeof addon.NativeSession.prototype.elementHandleTap,
    elementHandleScrollIntoViewIfNeeded: typeof addon.NativeSession.prototype.elementHandleScrollIntoViewIfNeeded,
    elementHandleWaitForSelector: typeof addon.NativeSession.prototype.elementHandleWaitForSelector,
    elementHandleSelectOptionValue: typeof addon.NativeSession.prototype.elementHandleSelectOptionValue,
    frameElement: typeof addon.NativeSession.prototype.frameElement,
    locatorWaitForElement: typeof addon.NativeLocator.prototype.waitForElement,
    locatorWaitForSelectorElement: typeof addon.NativeLocator.prototype.waitForSelectorElement,
    ...(['official-conformance-page-cdp-session', 'official-conformance-page-dispatch-event', 'official-conformance-page-goto-burst-20', 'official-conformance-page-goto-frame-detach', 'official-conformance-page-route-header-url', 'official-conformance-page-route-header-url-unroute', 'official-conformance-page-fill-navigation-commit'].includes(profile)
      ? { elementHandleDispatchEvent: typeof addon.NativeSession.prototype.elementHandleDispatchEvent }
      : {}),
    pageRouteContinueWithHeaders: typeof addon.NativeSession.prototype.pageRouteContinueWithHeaders,
    pageRouteFallbackWithUrl: typeof addon.NativeSession.prototype.pageRouteFallbackWithUrl,
    pageRouteFallbackWithOverrides: typeof addon.NativeSession.prototype.pageRouteFallbackWithOverrides,
    pageUnroute: typeof addon.NativeSession.prototype.pageUnroute,
    responseRawHeaders: typeof addon.NativeSession.prototype.responseRawHeaders,
    newContextWithProxy: typeof addon.NativeSession.prototype.newContextWithProxy,
    pageRouteFromHar: typeof addon.NativeSession.prototype.pageRouteFromHar,
    contextRouteFromHar: typeof addon.NativeSession.prototype.contextRouteFromHar,
    pageRouteFromHarRegExp: typeof addon.NativeSession.prototype.pageRouteFromHarRegExp,
    contextRouteFromHarRegExp: typeof addon.NativeSession.prototype.contextRouteFromHarRegExp,
    pageRecordHar: typeof addon.NativeSession.prototype.pageRecordHar,
    contextRecordHar: typeof addon.NativeSession.prototype.contextRecordHar,
    pageRecordHarRegExp: typeof addon.NativeSession.prototype.pageRecordHarRegExp,
    contextRecordHarRegExp: typeof addon.NativeSession.prototype.contextRecordHarRegExp,
  },
}));
process.exit(0);
