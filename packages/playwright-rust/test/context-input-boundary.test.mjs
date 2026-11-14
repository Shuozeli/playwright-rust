import assert from 'node:assert/strict';
import { test } from 'node:test';
import { setImmediate } from 'node:timers/promises';
import { chromium } from '@shuozeli/playwright-rust';
import artifactConfig from '../src/native-artifact.cjs';
import { diagnosticDeadline } from '../scripts/probe-deadline.mjs';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;
assert.equal(endpoint, 'http://127.0.0.1:9222');
const selectedHeaders = artifactConfig.nativeFeatures(artifactConfig.nativeArtifact.profile).includes('extra-headers');

async function close(browser) {
  await diagnosticDeadline(browser.close(), 'Context input boundary cleanup');
  for (let turn = 0; turn < 100 && browser.playwrightRustFfiMetrics().pending; turn++) await setImmediate();
  assert.equal(browser.playwrightRustFfiMetrics().pending, 0);
  assert.equal(browser.playwrightRustFfiMetrics().reservedBytes, 0);
  assert.equal(browser.playwrightRustMetrics().activeLeases, 0);
}

for (const headers of [false, true]) {
  for (const [encoding, baseURL] of [['ASCII', 'x'.repeat(16385)], ['UTF-8', '界'.repeat(5462)]]) {
    test(`${headers ? 'header' : 'plain'} Context rejects oversized ${encoding} baseURL as TypeError`,
      { timeout: 15000, skip: headers && !selectedHeaders }, async () => {
        // Arrange
        const browser = await chromium.connectOverCDP(endpoint, { timeout: 5000 });
        try {
          const contexts = browser.contexts();
          const before = browser.playwrightRustContextMetadataMetrics();
          const reserved = browser.playwrightRustFfiMetrics().reservedBytes;
          const options = headers ? { baseURL, extraHTTPHeaders: {} } : { baseURL };
          // Act
          const pending = browser.newContext(options);
          // Assert
          await assert.rejects(pending, error => {
            assert.ok(error instanceof TypeError);
            assert.equal(error.code, 'PLAYWRIGHT_RUST_INVALID_ARGUMENT');
            assert.match(error.message, /baseURL exceeds 16384 UTF-8 bytes/);
            return true;
          });
          assert.deepEqual(browser.playwrightRustContextMetadataMetrics(), before);
          assert.deepEqual(browser.contexts(), contexts);
          assert.equal(browser.playwrightRustFfiMetrics().reservedBytes, reserved);
          assert.equal(browser.playwrightRustFfiMetrics().commands, 0);
        } finally { await close(browser); }
      });
  }
}

test('header getter exceptions retain identity before oversized baseURL validation',
  { timeout: 15000, skip: !selectedHeaders }, async () => {
    // Arrange
    const browser = await chromium.connectOverCDP(endpoint, { timeout: 5000 });
    try {
      const before = browser.playwrightRustContextMetadataMetrics();
      const reserved = browser.playwrightRustFfiMetrics().reservedBytes;
      const sentinel = new Error('PLAYWRIGHT_RUST_INVALID_ARGUMENT: user getter sentinel');
      const options = { baseURL: 'x'.repeat(16385), extraHTTPHeaders: {
        get 'X-Test'() { throw sentinel; },
      } };
      // Act
      const pending = browser.newContext(options);
      // Assert
      await assert.rejects(pending, error => error === sentinel);
      assert.deepEqual(browser.playwrightRustContextMetadataMetrics(), before);
      assert.equal(browser.playwrightRustFfiMetrics().reservedBytes, reserved);
      assert.equal(browser.playwrightRustFfiMetrics().commands, 0);
    } finally { await close(browser); }
  });
