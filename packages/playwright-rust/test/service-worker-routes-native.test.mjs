import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';

const require = createRequire(import.meta.url);
const { nativeArtifact, verifyNativeArtifact } = require('../src/native-artifact.cjs');

test('service-worker-routes profile exposes Context routing and Service Worker ports', () => {
  // Arrange
  assert.equal(nativeArtifact.profile, 'service-worker-routes');
  const binary = require(nativeArtifact.path);

  // Act
  verifyNativeArtifact(binary, nativeArtifact);

  // Assert
  assert.equal(binary.serviceWorkerEventsEnabled(), true);
  assert.equal(binary.pageRoutesEnabled(), true);
  assert.equal(binary.pageUnrouteEnabled(), true);
  assert.equal(binary.routeRegExpEnabled(), true);
  assert.equal(binary.routeFulfillCorsEnabled(), true);
  for (const method of [
    'nextServiceWorkerNotice',
    'contextServiceWorkers',
    'workerEvaluate',
    'contextRoute',
    'contextRouteRegExp',
    'contextUnroute',
    'contextUnrouteAll',
    'nextPageRouteCall',
  ]) assert.equal(typeof binary.NativeSession.prototype[method], 'function', method);
});
