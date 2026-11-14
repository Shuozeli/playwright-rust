import assert from 'node:assert/strict';
import { test } from 'node:test';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { installNativeNotices } from '../scripts/native-notices.mjs';

test('native packaging preserves both host license texts and upstream attribution', () => {
  // Arrange
  const workspace = fileURLToPath(new URL('../../../', import.meta.url));
  const destination = mkdtempSync(join(tmpdir(), 'playwright-rust-native-notices-'));
  try {
    // Act
    installNativeNotices(workspace, destination);
    // Assert
    for (const name of ['NOTICE', 'LICENSE-MIT', 'LICENSE-APACHE-2.0']) {
      assert.deepEqual(readFileSync(join(destination, `playwright-rust-node-host.${name}`)),
        readFileSync(join(workspace, 'crates/playwright-rust-node-host', name)));
    }
    assert.match(readFileSync(join(destination, 'playwright-rust-node-host.NOTICE'), 'utf8'), /Copyright \(c\) Microsoft Corporation/);
    assert.equal(existsSync(join(destination, 'playwright-rust-file-input-fs.MIME-LICENSE')), false);
  } finally { rmSync(destination, { recursive: true }); }
});

test('selected upload packaging preserves MIME data and Playwright helper notices', () => {
  // Arrange
  const workspace = fileURLToPath(new URL('../../../', import.meta.url));
  const destination = mkdtempSync(join(tmpdir(), 'playwright-rust-upload-notices-'));
  try {
    // Act
    installNativeNotices(workspace, destination, ['file-input']);
    // Assert
    for (const [crate, names] of [
      ['playwright-rust-file-input-fs', ['MIME-LICENSE']],
      ['playwright-rust-backend-cdp-file-input', ['NOTICE', 'LICENSE.playwright']],
    ]) {
      for (const name of names)
        assert.deepEqual(readFileSync(join(destination, `${crate}.${name}`)),
          readFileSync(join(workspace, 'crates', crate, name)));
    }
  } finally { rmSync(destination, { recursive: true }); }
});
