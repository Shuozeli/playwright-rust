// Keyboard-only wire recipe must not pull in the full Chromium/Node installation.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const result = spawnSync('cargo', ['tree', '--locked', '-p', 'playwright-rust-composition-chromium-keyboard',
  '--edges', 'normal', '--prefix', 'none', '--format', '{p}'], {
  cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 4 * 1024 * 1024,
});
if (result.error) throw result.error;
assert.equal(result.status, 0, result.stderr);
const packages = [...new Set(result.stdout.split('\n').map(line => line.split(' ')[0]))].sort();
assert.deepEqual(packages.filter(name => name.startsWith('playwright-rust-')), [
  'playwright-rust-backend-cdp-common', 'playwright-rust-backend-cdp-input', 'playwright-rust-backend-cdp-keyboard',
  'playwright-rust-browser-backend', 'playwright-rust-cap-keyboard', 'playwright-rust-cdp', 'playwright-rust-codec-binary',
  'playwright-rust-codec-common', 'playwright-rust-codec-keyboard', 'playwright-rust-composition-chromium-keyboard',
  'playwright-rust-connection-control', 'playwright-rust-core', 'playwright-rust-input-model', 'playwright-rust-server-keyboard',
  'playwright-rust-server-wire', 'playwright-rust-transport',
]);
for (const forbidden of ['napi', 'napi-derive', 'serde_yaml', 'playwright-rust-schema-gen',
  'playwright-rust-runtime', 'playwright-rust-backend-cdp-locator', 'playwright-rust-node-host', 'playwright-rust-composition-chromium']) {
  assert.equal(packages.includes(forbidden), false, `Keyboard composition must not require ${forbidden}`);
}
console.log('Keyboard composition normal closure: exactly 16 Playwright Rust packages, no full Chromium/runtime/Locator/Node driver');

const controlled = spawnSync('cargo', ['tree', '--locked', '-p', 'playwright-rust-composition-chromium-keyboard',
  '--features', 'runtime', '--edges', 'normal', '--prefix', 'none', '--format', '{p}'], {
  cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 4 * 1024 * 1024,
});
if (controlled.error) throw controlled.error;
assert.equal(controlled.status, 0, controlled.stderr);
const controlledPackages = [...new Set(controlled.stdout.split('\n').map(line => line.split(' ')[0]))].sort();
assert.deepEqual(controlledPackages, [...packages, 'playwright-rust-runtime'].sort());
console.log('Keyboard runtime opt-in adds exactly playwright-rust-runtime: 17 Playwright Rust packages, no new executor or full host');
