import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRequire } from 'node:module';

function adapter(calls = []) {
  return createRequire(import.meta.url)('../src/page-pdf.cjs')({
    mapError: (error, api) => new Error(`${api}: ${error.message}`),
    validateOptions(options, allowed, api) {
      calls.push(['validateOptions', options, allowed, api]);
      for (const key of Object.keys(options)) {
        if (!allowed.includes(key)) throw new Error(`${api} option ${key}`);
      }
    },
    UnsupportedOperationError: class UnsupportedOperationError extends Error {},
  });
}

test('Page PDF delegates validated options and one Page identity to Rust', async () => {
  // Arrange
  const calls = [];
  const expected = Buffer.from('%PDF-1.7');
  const options = {
    path: '/tmp/output.pdf',
    scale: 0.75,
    displayHeaderFooter: true,
    headerTemplate: '<span class="title"></span>',
    footerTemplate: '<span class="pageNumber"></span>',
    printBackground: true,
    landscape: true,
    pageRanges: '1-3, 5',
    format: 'A4',
    width: 192,
    height: '25.4mm',
    preferCSSPageSize: true,
    margin: { top: 96, right: '1in', bottom: '2.54cm', left: '25.4mm' },
    tagged: true,
    outline: true,
  };
  const session = {
    async pagePdf(pageId, nativeOptions) {
      calls.push(['pagePdf', pageId, nativeOptions]);
      return expected;
    },
  };

  // Act
  const actual = await adapter(calls)(session, 'page-7', options);

  // Assert
  assert.equal(actual, expected);
  assert.deepEqual(calls, [
    ['validateOptions', options, [
      'displayHeaderFooter', 'footerTemplate', 'format', 'headerTemplate',
      'height', 'landscape', 'margin', 'outline', 'pageRanges', 'path',
      'preferCSSPageSize', 'printBackground', 'scale', 'tagged', 'width',
    ], 'page.pdf'],
    ['validateOptions', options.margin, ['top', 'right', 'bottom', 'left'], 'page.pdf margin'],
    ['pagePdf', 'page-7', {
      path: '/tmp/output.pdf',
      scale: 0.75,
      displayHeaderFooter: true,
      headerTemplate: '<span class="title"></span>',
      footerTemplate: '<span class="pageNumber"></span>',
      printBackground: true,
      landscape: true,
      pageRanges: '1-3, 5',
      format: 'A4',
      width: '192px',
      height: '25.4mm',
      preferCssPageSize: true,
      margin: { top: '96px', right: '1in', bottom: '2.54cm', left: '25.4mm' },
      tagged: true,
      outline: true,
    }],
  ]);
});

test('Page PDF supplies Playwright-compatible false defaults', async () => {
  // Arrange
  const calls = [];
  const session = {
    async pagePdf(...args) {
      calls.push(args);
      return Buffer.from('%PDF');
    },
  };

  // Act
  await adapter()(session, 'page-8');

  // Assert
  assert.deepEqual(calls, [['page-8', {
    path: undefined,
    scale: 1,
    displayHeaderFooter: false,
    headerTemplate: '',
    footerTemplate: '',
    printBackground: false,
    landscape: false,
    pageRanges: '',
    format: undefined,
    width: undefined,
    height: undefined,
    preferCssPageSize: false,
    margin: undefined,
    tagged: false,
    outline: false,
  }]]);
});

test('Page PDF rejects invalid options before native execution', async () => {
  // Arrange
  let called = false;
  const session = { async pagePdf() { called = true; } };

  // Act
  const operation = adapter()(session, 'page-9', { tagged: 'yes' });

  // Assert
  await assert.rejects(operation, /tagged must be a boolean/);
  assert.equal(called, false);
});

test('Page PDF rejects invalid dimensions and margins before native execution', async () => {
  // Arrange
  let called = false;
  const session = { async pagePdf() { called = true; } };

  // Act
  const width = adapter()(session, 'page-9', { width: Number.POSITIVE_INFINITY });
  const margin = adapter()(session, 'page-9', { margin: { top: false } });

  // Assert
  await assert.rejects(width, /width must be a string or finite number/);
  await assert.rejects(margin, /margin\.top must be a string or finite number/);
  assert.equal(called, false);
});

test('Page PDF reports the missing optional native capability explicitly', async () => {
  // Arrange
  const session = {};

  // Act
  const operation = adapter()(session, 'page-10');

  // Assert
  await assert.rejects(operation, /select page-pdf native profile/);
});
