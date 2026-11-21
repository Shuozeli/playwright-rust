import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

class UnsupportedOperationError extends Error {}

const nullViewport = createRequire(import.meta.url)('../src/context-null-viewport.cjs')({
  UnsupportedOperationError,
});

test('null viewport rejects the two pinned option conflicts before native selection', () => {
  // Arrange
  const cases = [
    [{ noViewport: true, deviceScaleFactor: 1 }, '"deviceScaleFactor" option is not supported with null "viewport"'],
    [{ noViewport: true, isMobile: true }, '"isMobile" option is not supported with null "viewport"'],
  ];

  // Act and Assert
  for (const [options, message] of cases)
    assert.throws(() => nullViewport.validate(options), error => error.message === `browser.newContext: ${message}`);
});

test('null viewport execution uses the ordinary non-emulated Context path', () => {
  // Arrange
  const supported = [
    { noViewport: true },
    { noViewport: true, isMobile: false },
  ];

  // Act and Assert
  for (const options of supported)
    assert.equal(nullViewport.validate(options), undefined);
});

test('default viewport path remains unchanged and invalid option type fails', () => {
  // Arrange
  const defaults = [{}, { noViewport: false }];

  // Act and Assert
  for (const options of defaults)
    assert.equal(nullViewport.validate(options), undefined);
  assert.throws(() => nullViewport.validate({ noViewport: 'yes' }), /must be a boolean/);
});
