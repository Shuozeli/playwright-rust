import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

import { chromium } from '../src/index.mjs';

function videoSize(path) {
  const result = spawnSync('ffprobe', [
    '-v', 'error',
    '-select_streams', 'v:0',
    '-show_entries', 'stream=width,height',
    '-of', 'csv=s=x:p=0',
    path,
  ], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}

async function paintFrames(page, color) {
  await page.evaluate(value => {
    document.documentElement.style.background = value;
    document.body.innerHTML = '<h1>Playwright Rust</h1>';
  }, color);
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 750)));
}

test('owned Context records, saves and deletes the exact Page video', async () => {
  // Arrange
  const root = mkdtempSync(join(tmpdir(), 'playwright-rust-node-video-context-'));
  const recorded = join(root, 'recorded');
  const copy = join(root, 'copy.webm');
  const browser = await chromium.launch({
    executablePath: chromium.executablePath(),
    headless: true,
    handleSIGINT: false,
  });
  let primaryError;

  try {
    const context = await browser.newContext({
      recordVideo: { dir: recorded, size: { width: 640, height: 360 } },
    });
    const page = await context.newPage();
    await paintFrames(page, '#0b6');
    const video = page.video();
    assert.ok(video);
    assert.equal(page.video(), video);
    const source = await video.path();

    // Act
    const saving = video.saveAs(copy);
    await context.close();
    await saving;

    // Assert
    assert.ok(statSync(source).size > 0);
    assert.ok(statSync(copy).size > 0);
    assert.equal(videoSize(copy), '640x360');
    await video.delete();
    await video.delete();
    assert.equal(existsSync(source), false);
  } catch (error) {
    primaryError = error;
    throw error;
  } finally {
    try {
      await browser.close();
      assert.equal(browser.playwrightRustMetrics().activeLeases, 0);
      assert.equal(browser.playwrightRustMetrics().liveScopes, 0);
    }
    catch (error) { if (!primaryError) throw error; }
    rmSync(root, { recursive: true, force: true });
  }
});

test('Browser.newPage owns and finalizes its implicit video Context', async () => {
  // Arrange
  const root = mkdtempSync(join(tmpdir(), 'playwright-rust-node-video-page-'));
  const recorded = join(root, 'recorded');
  const copy = join(root, 'copy.webm');
  const browser = await chromium.launch({
    executablePath: chromium.executablePath(),
    headless: true,
    handleSIGINT: false,
  });
  let primaryError;

  try {
    const page = await browser.newPage({ recordVideo: { dir: recorded } });
    await paintFrames(page, '#06b');
    const video = page.video();
    assert.ok(video);
    const source = await video.path();

    // Act
    const saving = video.saveAs(copy);
    await page.close();
    await saving;

    // Assert
    assert.ok(statSync(source).size > 0);
    assert.ok(statSync(copy).size > 0);
    assert.equal(videoSize(copy), '800x450');
    await video.delete();
    assert.equal(existsSync(source), false);
  } catch (error) {
    primaryError = error;
    throw error;
  } finally {
    try {
      await browser.close();
      assert.equal(browser.playwrightRustMetrics().activeLeases, 0);
      assert.equal(browser.playwrightRustMetrics().liveScopes, 0);
    }
    catch (error) { if (!primaryError) throw error; }
    rmSync(root, { recursive: true, force: true });
  }
});
