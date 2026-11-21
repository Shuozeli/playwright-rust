import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';

const require = createRequire(import.meta.url);
const { nativeArtifact, verifyNativeArtifact } = require('../src/native-artifact.cjs');

test('worker-network-events profile exposes its exact Service Worker ports', () => {
  // Arrange
  assert.equal(nativeArtifact.profile, 'worker-network-events');
  const binary = require(nativeArtifact.path);

  // Act
  verifyNativeArtifact(binary, nativeArtifact);

  // Assert
  assert.equal(binary.serviceWorkerEventsEnabled(), true);
  assert.equal(binary.workerNetworkEventsEnabled(), true);
  for (const method of [
    'nextServiceWorkerNotice',
    'contextServiceWorkers',
    'workerEvaluate',
    'nextNetworkNotice',
  ]) assert.equal(typeof binary.NativeSession.prototype[method], 'function', method);
});
