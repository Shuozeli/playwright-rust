import assert from 'node:assert/strict';

import { chromium } from '../src/index.mjs';

assert.equal(process.env.PLAYWRIGHT_RUST_NATIVE_PROFILE, 'context-bindings');

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP ?? 'http://127.0.0.1:9222';
const browser = await chromium.connectOverCDP(endpoint);
const context = await browser.newContext();

try {
  // Arrange
  const first = await context.newPage();
  let observedSource;
  await context.exposeFunction('sum', (left, right) => left + right);
  await context.exposeBinding('describeSource', (source, value) => {
    observedSource = source;
    return `${source.page === first}:${source.frame === first.mainFrame()}:${value}`;
  });

  // Act
  const existingPageResult = await first.evaluate(async () => ({
    sum: await globalThis.sum(19, 23),
    source: await globalThis.describeSource('existing'),
  }));
  const second = await context.newPage();
  const futurePageResult = await second.evaluate(async () => globalThis.sum(20, 22));
  const pageConflict = await first.exposeFunction('sum', () => 0).catch(error => error);
  await first.exposeFunction('pageOnly', () => 17);
  const contextConflict = await context.exposeFunction('pageOnly', () => 0).catch(error => error);

  // Assert
  assert.deepEqual(existingPageResult, { sum: 42, source: 'true:true:existing' });
  assert.equal(futurePageResult, 42);
  assert.equal(observedSource.context, context);
  assert.equal(observedSource.page, first);
  assert.equal(observedSource.frame, first.mainFrame());
  assert.match(pageConflict.message, /already registered in the browser context/);
  assert.match(contextConflict.message, /already registered in one of the pages/);
  await second.close();
} finally {
  await context.close();
  await browser.close();
}

process.stdout.write('BrowserContext binding live checks passed.\n');
