import assert from 'node:assert/strict';
import test from 'node:test';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

class UnsupportedOperationError extends Error {}

const videoRecording = require('../src/video-recording.cjs')({
  UnsupportedOperationError,
  mapError: error => error,
  resolvePath: value => `/absolute/${value}`,
  ffmpegPath: () => '/registry/ffmpeg-1011/ffmpeg',
});

test('recordVideo validates the required directory before native work', () => {
  // Arrange
  const value = {};

  // Act / Assert
  assert.throws(
    () => videoRecording.prepare(value, undefined, 'browser.newContext'),
    /recordVideo\.dir: expected string, got undefined/,
  );
});

test('legacy videoSize requires videosPath with the pinned error', () => {
  // Arrange
  const size = { width: 320, height: 240 };

  // Act / Assert
  assert.throws(
    () => videoRecording.prepare(undefined, undefined, 'browser.newContext', undefined, size),
    /"videoSize" option requires "videosPath" to be specified/,
  );
});

test('legacy videosPath and videoSize normalize to the same recording plan', () => {
  // Arrange
  const size = { width: 320, height: 240 };

  // Act
  const plan = videoRecording.prepare(
    undefined,
    size,
    'browser.newContext',
    'legacy-videos',
    size,
  );

  // Assert
  assert.deepEqual(plan, {
    directory: '/absolute/legacy-videos',
    ffmpeg: '/registry/ffmpeg-1011/ffmpeg',
    width: 320,
    height: 240,
  });
});

test('recordVideo derives the Playwright default 800x450 size', () => {
  // Arrange
  const value = { dir: 'videos' };

  // Act
  const plan = videoRecording.prepare(value, undefined, 'browser.newContext');

  // Assert
  assert.deepEqual(plan, {
    directory: '/absolute/videos',
    ffmpeg: '/registry/ffmpeg-1011/ffmpeg',
    width: 800,
    height: 450,
  });
});

test('recordVideo with null viewport derives the Playwright default 800x600 size', () => {
  // Arrange
  const value = { dir: 'videos' };

  // Act
  const plan = videoRecording.prepare(value, null, 'browser.newContext');

  // Assert
  assert.deepEqual(plan, {
    directory: '/absolute/videos',
    ffmpeg: '/registry/ffmpeg-1011/ffmpeg',
    width: 800,
    height: 600,
  });
});

test('Context installation forwards one normalized Rust recording plan', async () => {
  // Arrange
  const calls = [];
  const session = {
    async installContextVideo(...args) { calls.push(args); },
  };
  const plan = videoRecording.prepare(
    { dir: 'videos', size: { width: 640, height: 360 } },
    undefined,
    'browser.newContext',
  );

  // Act
  await videoRecording.initialize(session, 'context-1', plan);

  // Assert
  assert.deepEqual(calls, [[
    'context-1', '/absolute/videos', '/registry/ffmpeg-1011/ffmpeg', 640, 360,
  ]]);
});

test('Video delegates path, saveAs and delete to the exact Page artifact', async () => {
  // Arrange
  const calls = [];
  const session = {
    async pageVideoPath(page) { calls.push(['path', page]); return '/source/video.webm'; },
    async pageVideoSaveAs(page, path) { calls.push(['saveAs', page, path]); },
    async pageVideoDelete(page) { calls.push(['delete', page]); },
  };
  const page = { _id: 'page-7', _context: { _browser: { _session: session } } };
  const video = videoRecording.create(page);

  // Act
  const path = await video.path();
  await video.saveAs('copy.webm');
  await video.delete();

  // Assert
  assert.equal(path, '/source/video.webm');
  assert.deepEqual(calls, [
    ['path', 'page-7'],
    ['saveAs', 'page-7', '/absolute/copy.webm'],
    ['delete', 'page-7'],
  ]);
});
