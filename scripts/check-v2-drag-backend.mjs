// Default drag primitives stay independent; Mouse policy is an explicit feature.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
for (const mouse of [false, true]) {
  const result = spawnSync('cargo', ['tree', '--locked', '-p', 'playwright-rust-backend-cdp-drag',
    '--no-default-features', ...(mouse ? ['--features', 'mouse-dispatch'] : []),
    '--edges', 'normal', '--prefix', 'none', '--format', '{p}'], {
    cwd: root, encoding: 'utf8', timeout: 120000, maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stderr);
  const packages = [...new Set(result.stdout.split('\n').map(line => line.split(' ')[0]))].sort();
  const expected = ['playwright-rust-backend-cdp-drag', 'playwright-rust-cdp', 'playwright-rust-core'];
  if (mouse) expected.push('playwright-rust-backend-cdp-input', 'playwright-rust-input-model');
  assert.deepEqual(packages.filter(name => name.startsWith('playwright-rust-')), expected.sort());
  for (const forbidden of ['napi', 'napi-derive', 'playwright-rust-runtime', 'playwright-rust-backend-cdp-mouse',
    'playwright-rust-backend-cdp-locator', 'playwright-rust-node-host']) {
    assert.equal(packages.includes(forbidden), false, `Drag leaf must not require ${forbidden}`);
  }
  console.log(`Drag ${mouse ? 'mouse-dispatch opt-in' : 'default'} closure: exactly ${expected.length} Playwright Rust packages; no runtime/Locator/Node installation`);
}
