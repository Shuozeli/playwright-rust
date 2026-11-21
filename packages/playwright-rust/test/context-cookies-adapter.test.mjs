import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';
import { runInNewContext } from 'node:vm';

const require = createRequire(import.meta.url);
const contextCookies = require('../src/context-cookies.cjs')({
  mapError: (error, api) => new Error(`${api}: ${error.message}`),
  UnsupportedOperationError: class UnsupportedOperationError extends Error {},
});

test('BrowserContext.cookies normalizes an omitted URL list and returns the Rust snapshot', async () => {
  // Arrange
  const expected = [{ name: 'foo', value: 'bar', domain: 'example.test', path: '/', expires: -1,
    httpOnly: false, secure: false, sameSite: 'Lax' }];
  const calls = [];
  const session = { async contextCookies(...args) { calls.push(args); return expected; } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };

  // Act
  const actual = await contextCookies.cookies(context);

  // Assert
  assert.equal(actual, expected);
  assert.deepEqual(calls, [['context-7', []]]);
});

test('BrowserContext.cookies wraps one URL while preserving an explicit URL array', async () => {
  // Arrange
  const arrays = [];
  const session = { async contextCookies(_contextId, urls) { arrays.push(urls); return []; } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };
  const explicit = ['https://foo.test/', 'https://bar.test/'];

  // Act
  await contextCookies.cookies(context, 'https://one.test/');
  await contextCookies.cookies(context, explicit);

  // Assert
  assert.deepEqual(arrays[0], ['https://one.test/']);
  assert.equal(arrays[1], explicit);
});

test('BrowserContext.cookies remains unavailable when the selected native leaf is absent', async () => {
  // Arrange
  const context = { _id: 'context-7', browser: () => ({ _session: {} }) };

  // Act
  const pending = contextCookies.cookies(context);

  // Assert
  await assert.rejects(pending, /context-cookies native profile/);
});

test('BrowserContext.cookies maps a Rust failure to its public API name', async () => {
  // Arrange
  const session = { async contextCookies() { throw new Error('invalid cookie URL'); } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };

  // Act
  const pending = contextCookies.cookies(context, ':');

  // Assert
  await assert.rejects(pending, { message: 'browserContext.cookies: invalid cookie URL' });
});

test('BrowserContext.addCookies forwards its exact Context and cookie inputs to Rust', async () => {
  // Arrange
  const cookies = [{ url: 'https://example.test/', name: 'foo', value: 'bar' }];
  const calls = [];
  const session = { async contextAddCookies(...args) { calls.push(args); } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };

  // Act
  await contextCookies.add(context, cookies);

  // Assert
  assert.deepEqual(calls, [['context-7', cookies]]);
  assert.equal(calls[0][1], cookies);
});

test('BrowserContext.addCookies remains unavailable when the selected native leaf is absent', async () => {
  // Arrange
  const context = { _id: 'context-7', browser: () => ({ _session: {} }) };

  // Act
  const pending = contextCookies.add(context, []);

  // Assert
  await assert.rejects(pending, /context-cookies native profile/);
});

test('BrowserContext.addCookies maps a Rust failure to its public API name', async () => {
  // Arrange
  const session = { async contextAddCookies() { throw new Error('invalid cookie URL'); } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };

  // Act
  const pending = contextCookies.add(context, [{ name: 'foo', value: 'bar', url: ':' }]);

  // Assert
  await assert.rejects(pending, { message: 'browserContext.addCookies: invalid cookie URL' });
});

test('BrowserContext.clearCookies serializes string and cross-realm RegExp filters', async () => {
  // Arrange
  const calls = [];
  const session = { async contextClearCookies(...args) { calls.push(args); } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };
  const domain = runInNewContext('/example\\.test$/gi');
  const path = /^\/api/u;

  // Act
  await contextCookies.clear(context, { name: 'session', domain, path });

  // Assert
  assert.deepEqual(calls, [['context-7', {
    name: 'session',
    domainRegexSource: 'example\\.test$',
    domainRegexFlags: 'gi',
    pathRegexSource: path.source,
    pathRegexFlags: 'u',
  }]]);
});

test('BrowserContext.clearCookies omits unsupported JavaScript matcher values like Playwright', async () => {
  // Arrange
  const calls = [];
  const session = { async contextClearCookies(...args) { calls.push(args); } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };

  // Act
  await contextCookies.clear(context, { name: 7, domain: null, path: {}, ignored: 'value' });

  // Assert
  assert.deepEqual(calls, [['context-7', {}]]);
});

test('BrowserContext.clearCookies remains unavailable when the selected native leaf is absent', async () => {
  // Arrange
  const context = { _id: 'context-7', browser: () => ({ _session: {} }) };

  // Act
  const pending = contextCookies.clear(context);

  // Assert
  await assert.rejects(pending, /context-cookies native profile/);
});

test('BrowserContext.clearCookies maps a Rust failure to its public API name', async () => {
  // Arrange
  const session = { async contextClearCookies() { throw new Error('invalid cookie clear RegExp'); } };
  const context = { _id: 'context-7', browser: () => ({ _session: session }) };

  // Act
  const pending = contextCookies.clear(context, { name: /session/ });

  // Assert
  await assert.rejects(pending, { message: 'browserContext.clearCookies: invalid cookie clear RegExp' });
});
