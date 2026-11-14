// Native Mouse stays optional; generated contract proofs run separately.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const result = spawnSync('cargo', ['tree', '--locked', '-p', 'playwright-rust-backend-cdp-mouse',
  '--edges', 'normal', '--prefix', 'none', '--format', '{p}'], {
  cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 4 * 1024 * 1024,
});
if (result.error) throw result.error;
assert.equal(result.status, 0, result.stderr);
const packages = [...new Set(result.stdout.split('\n').map(line => line.split(' ')[0]))].sort();
assert.deepEqual(packages.filter(name => name.startsWith('playwright-rust-')), [
  'playwright-rust-backend-cdp-common', 'playwright-rust-backend-cdp-input', 'playwright-rust-backend-cdp-mouse',
  'playwright-rust-browser-backend', 'playwright-rust-cap-mouse', 'playwright-rust-cdp', 'playwright-rust-core', 'playwright-rust-input-model',
]);
for (const forbidden of ['napi', 'napi-derive', 'serde_yaml', 'playwright-rust-schema-gen',
  'playwright-rust-runtime', 'playwright-rust-backend-cdp-locator', 'playwright-rust-node-host']) {
  assert.equal(packages.includes(forbidden), false, `Mouse backend must not require ${forbidden}`);
}
console.log('Mouse backend normal closure: exactly 8 Playwright Rust packages, no runtime/Locator/Node driver');
