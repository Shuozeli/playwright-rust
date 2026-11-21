import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

class UnsupportedOperationError extends Error {}

const contextDevice = createRequire(import.meta.url)('../src/context-device.cjs')({
  UnsupportedOperationError,
});
const policies = createRequire(import.meta.url)('../src/context-policies.cjs')({
  UnsupportedOperationError,
  contextDevice,
});
const artifacts = createRequire(import.meta.url)('../src/native-artifact.cjs');

test('locale validates the public string before native selection', () => {
  // Arrange
  const options = { locale: 42 };

  // Act
  const validate = () => policies.validate(options);

  // Assert
  assert.throws(validate, /locale must be a string/);
});

test('locale delegates arbitrary strings without a binding-size policy', async () => {
  // Arrange
  const calls = [];
  const locale = 'x'.repeat(4096);
  const expected = Promise.resolve('context');
  const session = {
    newContextWithLocale(...args) {
      calls.push(args);
      return expected;
    },
  };

  // Act
  policies.validate({ locale });
  const pending = policies.create(session, { locale }, 'https://example.test/base/');

  // Assert
  assert.equal(pending, expected);
  assert.deepEqual(calls, [['https://example.test/base/', locale]]);
  assert.equal(await pending, 'context');
});

test('empty locale preserves ordinary Context creation without a capability', () => {
  // Arrange
  const session = {};
  const options = { locale: '' };

  // Act
  policies.validate(options);
  const pending = policies.create(session, options, undefined);

  // Assert
  assert.equal(pending, undefined);
});

test('enabled locale fails closed for a missing capability', () => {
  // Arrange
  const session = {};

  // Act
  const missing = () => policies.create(session, { locale: 'fr-FR' }, undefined);

  // Assert
  assert.throws(missing, /select context-locale native profile/);
});

test('locale composes with every selected Context policy in one Rust creation', async () => {
  // Arrange
  const calls = [];
  const headers = { 'x-test': 'one' };
  const proxy = { server: 'http://proxy.test:8080' };
  const headerChannelError = () => {};
  const session = {
    newContextWithLocale() {},
    newContextWithPolicies() {},
    newContextWithDeviceOptions() {},
    newContextWithOffline() {},
    newContextWithIgnoreHttpsErrors() {},
    newContextWithBypassCsp() {},
    newContextWithExtraHttpHeaders() {},
    newContextWithProxy() {},
    newContextWithPoliciesAndExtraHttpHeaders(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };
  const options = {
    locale: 'fr-FR',
    timezoneId: 'America/Jamaica',
    geolocation: { longitude: 10, latitude: 20 },
    permissions: ['geolocation', 'camera'],
    viewport: { width: 1280, height: 720 },
    offline: true,
    ignoreHTTPSErrors: true,
    bypassCSP: true,
    extraHTTPHeaders: headers,
    proxy,
  };

  // Act
  policies.validate(options);
  contextDevice.validate(options);
  const value = await policies.create(
    session,
    options,
    'https://example.test/base/',
    headerChannelError,
  );

  // Assert
  assert.equal(value, 'context');
  assert.deepEqual(calls, [[
    headers,
    headerChannelError,
    'https://example.test/base/',
    {
      locale: 'fr-FR',
      timezoneId: 'America/Jamaica',
      geolocation: { longitude: 10, latitude: 20, accuracy: 0 },
      permissions: ['geolocation', 'camera'],
      device: {
        userAgent: undefined,
        viewportWidth: 1280,
        viewportHeight: 720,
        screenWidth: undefined,
        screenHeight: undefined,
        deviceScaleFactor: undefined,
        isMobile: undefined,
        hasTouch: undefined,
      },
      offline: true,
      ignoreHttpsErrors: true,
      bypassCsp: true,
      proxy,
    },
  ]]);
});

test('composed policies without headers use the generic native port', async () => {
  // Arrange
  const calls = [];
  const session = {
    newContextWithLocale() {},
    newContextWithOffline() {},
    newContextWithPolicies(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };

  // Act
  const value = await policies.create(
    session,
    { locale: 'de-DE', offline: true },
    undefined,
  );

  // Assert
  assert.equal(value, 'context');
  assert.deepEqual(calls, [[undefined, { locale: 'de-DE', offline: true }]]);
});

test('locale native profile is independently installable', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const selected = artifacts.nativeFeatures('context-locale');

  // Assert
  assert.deepEqual(selected, ['context-locale']);
  assert.ok(!defaultFeatures.includes('context-locale'));
});

test('timezone validates the public string before native selection', () => {
  // Arrange
  const options = { timezoneId: 42 };

  // Act
  const validate = () => policies.validate(options);

  // Assert
  assert.throws(validate, /timezoneId must be a string/);
});

test('timezone delegates arbitrary strings through the generic Rust policy port', async () => {
  // Arrange
  const calls = [];
  const timezoneId = 'x'.repeat(4096);
  const session = {
    newContextWithPolicies(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };

  // Act
  policies.validate({ timezoneId });
  const value = await policies.create(session, { timezoneId }, undefined);

  // Assert
  assert.equal(value, 'context');
  assert.deepEqual(calls, [[undefined, { timezoneId }]]);
});

test('empty timezone preserves ordinary Context creation without a capability', () => {
  // Arrange
  const options = { timezoneId: '' };

  // Act
  policies.validate(options);
  const pending = policies.create({}, options, undefined);

  // Assert
  assert.equal(pending, undefined);
});

test('enabled timezone fails closed for a missing capability', () => {
  // Arrange
  const options = { timezoneId: 'America/Jamaica' };

  // Act
  const missing = () => policies.create({}, options, undefined);

  // Assert
  assert.throws(missing, /select context-timezone native profile/);
});

test('timezone native profile is independently installable', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const selected = artifacts.nativeFeatures('context-timezone');

  // Assert
  assert.deepEqual(selected, ['context-timezone']);
  assert.ok(!defaultFeatures.includes('context-timezone'));
});

test('geolocation validates its public object and scalar fields before native work', () => {
  // Arrange
  const invalid = [
    null,
    'London',
    { longitude: '10', latitude: 20 },
    { longitude: 10, latitude: '20' },
    { longitude: 10, latitude: 20, accuracy: '3' },
  ];

  // Act and Assert
  assert.throws(() => policies.validate({ geolocation: invalid[0] }), /geolocation must be an object/);
  assert.throws(() => policies.validate({ geolocation: invalid[1] }), /geolocation must be an object/);
  for (const geolocation of invalid.slice(2))
    assert.throws(() => policies.create({ newContextWithPolicies() {} }, { geolocation }, undefined), /must be a number/);
});

test('geolocation defaults accuracy and delegates one typed Rust policy object', async () => {
  // Arrange
  const calls = [];
  const session = {
    newContextWithPolicies(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };

  // Act
  const value = await policies.create(
    session,
    { geolocation: { longitude: 10, latitude: 20 } },
    'https://example.test/base/',
  );

  // Assert
  assert.equal(value, 'context');
  assert.deepEqual(calls, [[
    'https://example.test/base/',
    { geolocation: { longitude: 10, latitude: 20, accuracy: 0 } },
  ]]);
});

test('enabled geolocation fails closed for a missing capability', () => {
  // Arrange
  const options = { geolocation: { longitude: 10, latitude: 20 } };

  // Act
  const missing = () => policies.create({}, options, undefined);

  // Assert
  assert.throws(missing, /select context-geolocation native profile/);
});

test('geolocation native profile is independently installable', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const selected = artifacts.nativeFeatures('context-geolocation');

  // Assert
  assert.deepEqual(selected, ['context-geolocation']);
  assert.ok(!defaultFeatures.includes('context-geolocation'));
});

test('permissions validate their public array and string entries before native work', () => {
  // Arrange
  const session = { newContextWithPolicies() {} };

  // Act / Assert
  assert.throws(() => policies.validate({ permissions: 'camera' }), /permissions must be an array/);
  assert.throws(
    () => policies.create(session, { permissions: ['camera', 42] }, undefined),
    /permissions\[1\] must be a string/,
  );
});

test('permissions delegate one typed Rust policy object', async () => {
  // Arrange
  const calls = [];
  const session = {
    newContextWithPolicies(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };

  // Act
  const value = await policies.create(
    session,
    { permissions: ['geolocation', 'camera'] },
    undefined,
  );

  // Assert
  assert.equal(value, 'context');
  assert.deepEqual(calls, [[undefined, { permissions: ['geolocation', 'camera'] }]]);
});

test('permissions native profile is independently installable', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const selected = artifacts.nativeFeatures('context-permissions');

  // Assert
  assert.deepEqual(selected, ['context-geolocation-permission']);
  assert.ok(!defaultFeatures.includes('context-geolocation-permission'));
});

test('HTTP credentials validate their public fields before native work', () => {
  // Arrange
  const session = { newContextWithPolicies() {} };

  // Act / Assert
  assert.throws(
    () => policies.validate({ httpCredentials: { username: 'user' } }),
    /username and password must be strings/,
  );
  assert.throws(
    () => policies.create(session, {
      httpCredentials: { username: 'user', password: 'pass', send: 'eager' },
    }, undefined),
    /send must be "unauthorized" or "always"/,
  );
});

test('HTTP credentials delegate one typed Rust policy object', async () => {
  // Arrange
  const calls = [];
  const session = {
    newContextWithPolicies(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };

  // Act
  const value = await policies.create(session, {
    httpCredentials: {
      username: 'user', password: 'pass', origin: 'https://example.test', send: 'always',
    },
  }, undefined);

  // Assert
  assert.equal(value, 'context');
  assert.deepEqual(calls, [[undefined, {
    httpCredentials: {
      username: 'user', password: 'pass', origin: 'https://example.test', sendAlways: true,
    },
  }]]);
});

test('runtime HTTP credentials normalize replacement and strip unknown fields', () => {
  // Arrange
  const replacement = {
    username: 'user', password: 'pass', origin: 'https://example.test', ignored: true,
  };

  // Act
  const normalized = policies.normalizeRuntimeHttpCredentials(
    replacement,
    'browserContext.setHTTPCredentials',
  );

  // Assert
  assert.deepEqual(normalized, {
    username: 'user', password: 'pass', origin: 'https://example.test',
  });
});

test('runtime HTTP credentials normalize an omitted value to clear', () => {
  // Arrange
  const credentials = undefined;

  // Act
  const cleared = policies.normalizeRuntimeHttpCredentials(
    credentials,
    'browserContext.setHTTPCredentials',
  );

  // Assert
  assert.equal(cleared, undefined);
});

test('HTTP credentials native profile is independently installable', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const selected = artifacts.nativeFeatures('context-http-credentials');

  // Assert
  assert.deepEqual(selected, ['context-http-credentials']);
  assert.ok(!defaultFeatures.includes('context-http-credentials'));
});

test('a generic policy port cannot smuggle an unselected policy leaf', () => {
  // Arrange
  const timezoneOnly = createRequire(import.meta.url)('../src/context-policies.cjs')({
    UnsupportedOperationError,
    contextDevice,
    featureSelected: feature => feature === 'context-timezone',
  });
  const session = { newContextWithPolicies() {} };

  // Act / Assert
  assert.throws(
    () => timezoneOnly.create(session, { geolocation: { longitude: 10, latitude: 20 } }, undefined),
    /select context-geolocation native profile/,
  );
  assert.throws(
    () => timezoneOnly.create(session, { permissions: ['camera'] }, undefined),
    /select context-permissions native profile/,
  );
  assert.throws(
    () => timezoneOnly.create(session, {
      httpCredentials: { username: 'user', password: 'pass' },
    }, undefined),
    /select context-http-credentials native profile/,
  );
});

test('CSP bypass validates the public boolean before native selection', () => {
  // Arrange
  const options = { bypassCSP: 'yes' };

  // Act
  const validate = () => policies.validate(options);

  // Assert
  assert.throws(validate, /bypassCSP must be a boolean/);
});

test('disabled CSP bypass preserves ordinary Context creation without a capability', () => {
  // Arrange
  const session = {};
  const options = { bypassCSP: false };

  // Act
  policies.validate(options);
  const pending = policies.create(session, options, undefined);

  // Assert
  assert.equal(pending, undefined);
});

test('enabled CSP bypass delegates the base URL once to the native Rust owner', async () => {
  // Arrange
  const calls = [];
  const expected = Promise.resolve('context');
  const session = {
    newContextWithBypassCsp(...args) {
      calls.push(args);
      return expected;
    },
  };

  // Act
  const pending = policies.create(session, { bypassCSP: true }, 'https://example.test/base/');

  // Assert
  assert.equal(pending, expected);
  assert.deepEqual(calls, [['https://example.test/base/', true]]);
  assert.equal(await pending, 'context');
});

test('enabled CSP bypass rejects a missing capability explicitly', () => {
  // Arrange
  const session = { newContextWithBypassCsp() { throw new Error('must not run'); } };

  // Act
  const missing = () => policies.create({}, { bypassCSP: true }, undefined);

  // Assert
  assert.throws(missing, UnsupportedOperationError);
});

test('offline validates the public boolean before native selection', () => {
  // Arrange
  const options = { offline: 'yes' };

  // Act
  const validate = () => policies.validate(options);

  // Assert
  assert.throws(validate, /offline must be a boolean/);
});

test('offline Context creation selects only its opt-in native capability', async () => {
  // Arrange
  const calls = [];
  const session = {
    newContextWithOffline(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };

  // Act
  const pending = policies.create(session, { offline: true }, 'https://example.test/base/');

  // Assert
  assert.equal(await pending, 'context');
  assert.deepEqual(calls, [['https://example.test/base/', true]]);
  assert.throws(() => policies.create({}, { offline: true }, undefined), /select context-offline native profile/);
});

test('offline and device selection require the generic native composition port', () => {
  // Arrange
  const session = {
    newContextWithOffline() {},
    newContextWithDeviceOptions() {},
  };
  const options = { offline: true, hasTouch: true };

  // Act
  const create = () => policies.create(session, options, undefined);

  // Assert
  assert.throws(create, /composed policies \(rebuild native profile\)/);
});

test('offline native profile is independently installable', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const offlineFeatures = artifacts.nativeFeatures('context-offline');

  // Assert
  assert.deepEqual(offlineFeatures, ['context-offline']);
  assert.ok(!defaultFeatures.includes('context-offline'));
});

test('HTTPS error policy validates its public boolean before native selection', () => {
  // Arrange
  const options = { ignoreHTTPSErrors: 'yes' };

  // Act
  const validate = () => policies.validate(options);

  // Assert
  assert.throws(validate, /ignoreHTTPSErrors must be a boolean/);
});

test('disabled HTTPS error policy preserves ordinary Context creation without a capability', () => {
  // Arrange
  const session = {};
  const options = { ignoreHTTPSErrors: false };

  // Act
  policies.validate(options);
  const pending = policies.create(session, options, undefined);

  // Assert
  assert.equal(pending, undefined);
});

test('enabled HTTPS error policy delegates the base URL once to the Rust owner', async () => {
  // Arrange
  const calls = [];
  const expected = Promise.resolve('context');
  const session = {
    newContextWithIgnoreHttpsErrors(...args) {
      calls.push(args);
      return expected;
    },
  };

  // Act
  const pending = policies.create(session, { ignoreHTTPSErrors: true }, 'https://example.test/base/');

  // Assert
  assert.equal(pending, expected);
  assert.deepEqual(calls, [['https://example.test/base/', true]]);
  assert.equal(await pending, 'context');
});

test('enabled HTTPS error policy fails closed for a missing capability', () => {
  // Arrange
  const session = { newContextWithIgnoreHttpsErrors() { throw new Error('must not run'); } };

  // Act
  const missing = () => policies.create({}, { ignoreHTTPSErrors: true }, undefined);

  // Assert
  assert.throws(missing, /select ignore-https-errors native profile/);
});

test('HTTPS error policy native profile is independently installable', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const selected = artifacts.nativeFeatures('ignore-https-errors');

  // Assert
  assert.deepEqual(selected, ['ignore-https-errors']);
  assert.ok(!defaultFeatures.includes('ignore-https-errors'));
});
