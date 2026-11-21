import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

class UnsupportedOperationError extends Error {}

const har = createRequire(import.meta.url)('../src/har-recording.cjs')({
  UnsupportedOperationError,
});
const artifacts = createRequire(import.meta.url)('../src/native-artifact.cjs');

test('HAR creation defaults match official Playwright for har and zip paths', () => {
  // Arrange
  const harOptions = { path: 'capture.har' };
  const zipOptions = { path: 'capture.zip' };

  // Act
  const harPlan = har.prepare(harOptions);
  const zipPlan = har.prepare(zipOptions);

  // Assert
  assert.deepEqual(harPlan, {
    path: 'capture.har', pattern: undefined, content: 'embed', mode: 'full',
  });
  assert.deepEqual(zipPlan, {
    path: 'capture.zip', pattern: undefined, content: 'attach', mode: 'full',
  });
});

test('explicit HAR content wins over legacy omitContent', () => {
  // Arrange
  const options = {
    path: 'capture.har', omitContent: true, content: 'attach', mode: 'minimal', urlFilter: '**/api/**',
  };

  // Act
  const plan = har.prepare(options);

  // Assert
  assert.deepEqual(plan, {
    path: 'capture.har', pattern: '**/api/**', content: 'attach', mode: 'minimal',
  });
});

test('HAR creation preserves RegExp source and flags for the native matcher', () => {
  // Arrange
  const options = { path: 'capture.har', urlFilter: /api/ };

  // Act
  const plan = har.prepare(options);

  // Assert
  assert.deepEqual(plan, {
    path: 'capture.har', pattern: 'api', regexFlags: '', content: 'embed', mode: 'full',
  });
});

test('HAR creation delegates the normalized plan to the Rust port', async () => {
  // Arrange
  const calls = [];
  const session = {
    contextRecordHar(...args) {
      calls.push(args);
      return Promise.resolve();
    },
  };
  const plan = har.prepare({ path: 'capture.har', urlFilter: '**/*.js' });

  // Act
  await har.initialize(session, 'context-1', plan);

  // Assert
  assert.deepEqual(calls, [['context-1', 'capture.har', '**/*.js', 'embed', 'full']]);
});

test('HAR RegExp creation delegates source and flags to the Rust port', async () => {
  // Arrange
  const calls = [];
  const session = {
    contextRecordHarRegExp(...args) {
      calls.push(args);
      return Promise.resolve();
    },
  };
  const plan = har.prepare({ path: 'capture.har', urlFilter: /HAR.X?HTML/is });

  // Act
  await har.initialize(session, 'context-1', plan);

  // Assert
  assert.deepEqual(calls, [[
    'context-1', 'capture.har', 'HAR.X?HTML', 'is', { content: 'embed', mode: 'full' },
  ]]);
});

test('HAR recording remains an explicit native profile', () => {
  // Arrange
  const defaultFeatures = artifacts.nativeFeatures('default');

  // Act
  const harFeatures = artifacts.nativeFeatures('har-recording');

  // Assert
  assert.deepEqual(harFeatures, ['page-routes', 'har-recording']);
  assert.equal(defaultFeatures.includes('har-recording'), false);
});

test('HAR RegExp filtering remains an independent native profile', () => {
  // Arrange
  const recordingFeatures = artifacts.nativeFeatures('har-recording');

  // Act
  const regexpFeatures = artifacts.nativeFeatures('har-recording-regexp');

  // Assert
  assert.deepEqual(regexpFeatures, [
    'page-routes', 'har-recording', 'har-recording-regexp',
  ]);
  assert.equal(recordingFeatures.includes('har-recording-regexp'), false);
});

test('HAR recording composes with the ordinary Page conformance surface', () => {
  // Arrange
  const baseFeatures = artifacts.nativeFeatures('official-conformance-page-bindings');

  // Act
  const harFeatures = artifacts.nativeFeatures('official-conformance-page-har-recording');

  // Assert
  assert.deepEqual(harFeatures, [
    ...baseFeatures, 'har-recording', 'har-recording-regexp',
  ]);
});
