import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { test } from 'node:test';

const require = createRequire(import.meta.url);
const { nativeArtifact, nativeFeatures, verifyNativeArtifact } = require('../src/native-artifact.cjs');

function selectedEnvironment() {
  assert.ok(nativeFeatures(nativeArtifact.profile).includes('cdp-handshake-headers'));
  const addon = require(nativeArtifact.path);
  verifyNativeArtifact(addon, nativeArtifact);
  return new addon.NativeEnvironment(() => {}, () => 0);
}

test('real Node addon rejects excessive CDP handshake header count before endpoint parsing', () => {
  // Arrange
  const environment = selectedEnvironment();
  const headers = Array.from({ length: 33 }, (_, index) => [`x-${index}`, 'v']);

  // Act
  const connect = () => environment.connectOverCDPWithHeaders('not-a-url', 100, headers);

  // Assert
  assert.throws(connect, /CDP handshake header count exceeds 32/);
});

test('real Node addon bounds CDP handshake values without revealing their content', () => {
  // Arrange
  const environment = selectedEnvironment();
  const secret = 'super-secret-' + 'x'.repeat(4097);

  // Act
  const connect = () => environment.connectOverCDPWithHeaders('not-a-url', 100, [['authorization', secret]]);

  // Assert
  assert.throws(connect, error =>
    /CDP handshake header value exceeds 4096 bytes/.test(error.message)
      && !error.message.includes(secret));
});

test('real Node addon accepts bounded CDP handshake pairs into Rust endpoint validation', async () => {
  // Arrange
  const environment = selectedEnvironment();

  // Act
  const pending = environment.connectOverCDPWithHeaders('not-a-url', 100, [['foo', 'bar']]);

  // Assert
  await assert.rejects(pending, /CDP endpoint must be an absolute HTTP\(S\) or WS\(S\) URL/);
});

test('real Node addon forwards custom headers on the WebSocket upgrade', async () => {
  // Arrange
  const environment = selectedEnvironment();
  let resolveUpgrade;
  const upgrade = new Promise(resolve => { resolveUpgrade = resolve; });
  const server = createServer();
  server.on('upgrade', (request, socket) => {
    resolveUpgrade(request.headers);
    socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n');
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  assert.notEqual(address, null);
  assert.equal(typeof address, 'object');

  try {
    // Act
    const pending = environment.connectOverCDPWithHeaders(
      `ws://127.0.0.1:${address.port}/cdp`,
      1000,
      [['X-BCP-Lease-Id', 'lease-node-7'], ['X-BCP-Fencing-Token', '42']],
    );
    const received = await upgrade;

    // Assert
    assert.equal(received['x-bcp-lease-id'], 'lease-node-7');
    assert.equal(received['x-bcp-fencing-token'], '42');
    await assert.rejects(pending);
  } finally {
    await new Promise(resolve => server.close(resolve));
  }
});
