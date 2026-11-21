import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';

const require = createRequire(import.meta.url);
const candidate = require('../src/index.cjs');

const rejectionCases = [
  {
    name: 'missing certificate origin',
    options: { clientCertificates: [{ origin: '', cert: Buffer.from('cert'), key: Buffer.from('key') }] },
    reason: 'clientCertificates.origin is required',
  },
  {
    name: 'missing certificate material',
    options: { clientCertificates: [{ origin: 'https://example.test' }] },
    reason: 'None of cert, key, passphrase or pfx is specified',
  },
  {
    name: 'boxed Chromium page argument',
    options: { args: [new String('https://example.invalid/')] },
    reason: 'Arguments can not specify page to be opened',
  },
  {
    name: 'Chromium user-data-dir argument',
    options: { args: ['--user-data-dir=unused'] },
    reason: "Pass userDataDir parameter to 'browserType.launchPersistentContext(userDataDir, options)' instead of specifying '--user-data-dir' argument",
  },
  {
    name: 'Chromium remote-debugging pipe',
    options: { args: ['--remote-debugging-pipe'] },
    reason: 'Playwright manages remote debugging connection itself.',
  },
];

for (const { name, options, reason } of rejectionCases) {
  test(`${name} rejects before browser acquisition`, async () => {
    // Arrange
    const browserType = candidate.chromium;

    // Act
    const launch = browserType.launchPersistentContext('unused-profile', options);

    // Assert
    await assert.rejects(launch, {
      name: 'Error',
      message: `browserType.launchPersistentContext: ${reason}`,
    });
  });
}

test('truthy non-array ignoreDefaultArgs bypasses argument validation', async () => {
  // Arrange
  const options = { ignoreDefaultArgs: true, args: ['https://example.invalid/'] };

  // Act
  const launch = candidate.chromium.launchPersistentContext('unused-profile', options);

  // Assert
  await assert.rejects(launch, { code: 'PLAYWRIGHT_RUST_UNSUPPORTED' });
});

test('Firefox and WebKit providers are absent from the preflight surface', () => {
  // Arrange
  const api = candidate;

  // Act
  const engines = ['firefox', 'webkit'].filter(name => name in api);

  // Assert
  assert.deepEqual(engines, []);
});
