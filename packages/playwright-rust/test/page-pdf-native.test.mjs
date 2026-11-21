import assert from 'node:assert/strict';
import { readFile, rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { chromium } from '@shuozeli/playwright-rust';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;
const outputPath = process.env.PLAYWRIGHT_RUST_TEST_PDF_PATH;

test('real Chromium PDF bytes and optional path are owned by the Rust leaf', { timeout: 30_000 }, async () => {
  // Arrange
  assert.ok(endpoint, 'PLAYWRIGHT_RUST_TEST_CDP must name the dedicated Chrome endpoint');
  assert.ok(outputPath, 'PLAYWRIGHT_RUST_TEST_PDF_PATH must name a disposable output path');
  const browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
  const nestedRoot = join(dirname(outputPath), 'nested-page-pdf-e2e');
  const nestedOutput = join(nestedRoot, 'documents', 'full-options.pdf');

  try {
    const page = browser.contexts()
      .flatMap(context => context.pages())
      .find(candidate => candidate.url().includes('Playwright%20Rust%20PDF'));
    assert.ok(page, 'Pwright must open the dedicated PDF fixture page before this test');

    // Act
    const returned = await page.pdf({
      path: nestedOutput,
      scale: 0.75,
      displayHeaderFooter: true,
      headerTemplate: '<span class="title"></span>',
      footerTemplate: '<span class="pageNumber"></span>',
      printBackground: true,
      landscape: true,
      pageRanges: '1',
      format: 'a4',
      width: 192,
      height: '25.4mm',
      preferCSSPageSize: false,
      margin: { top: 96, right: '1in', bottom: '2.54cm', left: '25.4mm' },
      tagged: true,
      outline: true,
    });
    const persisted = await readFile(nestedOutput);
    const explicitDimensions = await page.pdf({ width: 384, height: '6in' });
    const withoutOutline = await page.pdf({ format: 'A4' });
    const withOutline = await page.pdf({ format: 'A4', tagged: true, outline: true });

    // Assert
    assert.equal(returned.subarray(0, 5).toString('ascii'), '%PDF-');
    assert.deepEqual(persisted, returned);
    assert.ok(returned.length > 1_000, 'Chromium should return a non-empty rendered document');
    assert.equal(explicitDimensions.subarray(0, 5).toString('ascii'), '%PDF-');
    assert.ok(explicitDimensions.length > 1_000);
    assert.ok(
      withOutline.length > withoutOutline.length,
      'the pinned upstream Chromium PDF test expects tagged outline output to be larger',
    );
  } finally {
    await browser.close();
    await rm(outputPath, { force: true });
    await rm(nestedRoot, { recursive: true, force: true });
  }
});
