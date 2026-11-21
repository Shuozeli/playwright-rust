import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

class UnsupportedOperationError extends Error {}

const device = createRequire(import.meta.url)('../src/context-device.cjs')({
  UnsupportedOperationError,
});

test('hasTouch validates the public boolean before native selection', () => {
  // Arrange
  const options = { hasTouch: 'yes' };

  // Act
  const validate = () => device.validate(options);

  // Assert
  assert.throws(validate, /hasTouch must be a boolean/);
});

test('disabled touch emulation preserves ordinary Context creation without a capability', () => {
  // Arrange
  const session = {};
  const options = { hasTouch: false };

  // Act
  device.validate(options);
  const pending = device.create(session, options, undefined);

  // Assert
  assert.equal(pending, undefined);
});

test('viewport and device settings delegate once to the Rust Context owner', async () => {
  // Arrange
  const calls = [];
  const expected = Promise.resolve('context');
  const session = {
    newContextWithDeviceOptions(...args) {
      calls.push(args);
      return expected;
    },
  };

  const options = {
    userAgent: 'playwright-rust-mobile',
    viewport: { width: 320, height: 240 },
    screen: { width: 640, height: 480 },
    deviceScaleFactor: 3,
    isMobile: true,
    hasTouch: true,
  };

  // Act
  device.validate(options);
  const pending = device.create(session, options, 'https://example.test/base/');

  // Assert
  assert.equal(pending, expected);
  assert.deepEqual(calls, [['https://example.test/base/', {
    userAgent: 'playwright-rust-mobile',
    viewportWidth: 320,
    viewportHeight: 240,
    screenWidth: 640,
    screenHeight: 480,
    deviceScaleFactor: 3,
    isMobile: true,
    hasTouch: true,
  }]]);
  assert.equal(await pending, 'context');
});

test('device settings compose HTTPS error policy in one Rust Context creation', async () => {
  // Arrange
  const calls = [];
  const session = {
    newContextWithDeviceOptionsAndIgnoreHttpsErrors(...args) {
      calls.push(args);
      return Promise.resolve('context');
    },
  };
  const options = {
    viewport: { width: 1280, height: 720 },
    ignoreHTTPSErrors: true,
  };

  // Act
  device.validate(options);
  const value = await device.create(session, options, 'https://example.test/base/');

  // Assert
  assert.equal(value, 'context');
  assert.deepEqual(calls, [[
    'https://example.test/base/',
    {
      userAgent: undefined,
      viewportWidth: 1280,
      viewportHeight: 720,
      screenWidth: undefined,
      screenHeight: undefined,
      deviceScaleFactor: undefined,
      isMobile: undefined,
      hasTouch: undefined,
    },
    true,
  ]]);
});

test('device and HTTPS error composition requires its exact native port', () => {
  // Arrange
  const options = {
    viewport: { width: 1280, height: 720 },
    ignoreHTTPSErrors: true,
  };

  // Act
  const create = () => device.create({ newContextWithDeviceOptions() {} }, options, undefined);

  // Assert
  assert.throws(create, /select device-emulation and ignore-https-errors native profile/);
});

test('enabled touch emulation rejects a missing capability and permits policy composition', () => {
  // Arrange
  const session = { newContextWithDeviceOptions() { throw new Error('must not run'); } };

  // Act
  const missing = () => device.create({}, { hasTouch: true }, undefined);
  const combined = () => device.validate({ hasTouch: true, bypassCSP: true });

  // Assert
  assert.throws(missing, UnsupportedOperationError);
  assert.doesNotThrow(combined);
});

test('viewport validation accepts null and rejects malformed dimensions before native selection', () => {
  // Arrange
  const cases = [
    { viewport: { width: 320 } },
    { viewport: { width: 0, height: 240 } },
    { viewport: { width: 320.5, height: 240 } },
    { viewport: { width: 320, height: 240, depth: 1 } },
    { deviceScaleFactor: Number.NaN },
    { isMobile: 'yes' },
    { userAgent: 42 },
  ];

  // Act and Assert
  assert.equal(device.validate({ viewport: null }), undefined);
  for (const options of cases)
    assert.throws(() => device.validate(options), TypeError);
});
