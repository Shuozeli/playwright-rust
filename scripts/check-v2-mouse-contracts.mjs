// Explicit offline contract proof; no browser or production runtime starts.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
function cargo(args) {
  const result = spawnSync('cargo', args, { cwd: root, encoding: 'utf8',
    env: { ...process.env, CARGO_INCREMENTAL: '0' }, timeout: 120000, maxBuffer: 4 * 1024 * 1024 });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
}
const closures = {
  cap: ['playwright-rust-cap-mouse', 'playwright-rust-core', 'playwright-rust-input-model'],
  client: ['playwright-rust-browser-model', 'playwright-rust-cap-mouse', 'playwright-rust-client', 'playwright-rust-client-mouse', 'playwright-rust-core', 'playwright-rust-input-model'],
  codec: ['playwright-rust-cap-mouse', 'playwright-rust-codec-binary', 'playwright-rust-codec-common', 'playwright-rust-codec-mouse', 'playwright-rust-core', 'playwright-rust-input-model', 'playwright-rust-transport'],
  server: ['playwright-rust-cap-mouse', 'playwright-rust-codec-binary', 'playwright-rust-codec-common', 'playwright-rust-codec-mouse', 'playwright-rust-connection-control', 'playwright-rust-core', 'playwright-rust-input-model', 'playwright-rust-server-mouse', 'playwright-rust-server-wire', 'playwright-rust-transport'],
};
for (const [kind, expected] of Object.entries(closures)) {
  const tree = cargo(['tree', '--locked', '-p', `playwright-rust-${kind}-mouse`, '--edges', 'normal', '--prefix', 'none', '--format', '{p}']);
  const actual = [...new Set(tree.split('\n').filter(line => line.startsWith('playwright-rust-')).map(line => line.split(' ')[0]))].sort();
  const allPackages = new Set(tree.split('\n').map(line => line.split(' ')[0]));
  for (const forbidden of ['tokio', 'napi', 'napi-derive', 'serde_yaml', 'playwright-rust-schema-gen'])
    assert.equal(allPackages.has(forbidden), false, `${kind} mouse must not require ${forbidden}`);
  assert.deepEqual(actual, expected, `${kind} mouse dependency boundary changed`);
  console.log(`Mouse ${kind} normal closure: ${actual.length} Playwright Rust packages`);
}

// Inventory top-level canonical capability schemas, not frozen old baselines.
// Full YAML/model validation and output freshness belong to the generator below.
const owners = new Map();
for (const directory of readdirSync(join(root, 'crates')).filter(name => name.startsWith('playwright-rust-cap-'))) {
  const schemaDirectory = join(root, 'crates', directory, 'schema');
  let entries;
  try { entries = readdirSync(schemaDirectory); }
  catch (error) { if (error.code === 'ENOENT') continue; throw error; }
  for (const entry of entries.filter(name => name.endsWith('.yml'))) {
    const source = readFileSync(join(schemaDirectory, entry), 'utf8');
    const capability = source.match(/^capability: (.+)$/m)?.[1];
    if (!capability) continue;
    let command;
    for (const line of source.split('\n')) {
      const id = line.match(/^  - id: (.+)$/);
      if (id) command = id[1];
      const method = line.match(/^    method: (\d+)$/);
      if (!method) continue;
      assert.ok(command, `${entry}: command method without identity`);
      const owner = `${capability}/${command}`;
      const previous = owners.get(method[1]);
      assert.ok(previous === undefined || previous === owner, `method ${method[1]} conflicts: ${previous} vs ${owner}`);
      owners.set(method[1], owner);
    }
  }
}
for (const [offset, name] of ['move', 'down', 'up', 'click', 'dblclick', 'wheel'].entries())
  assert.equal(owners.get(String(917505 + offset)), `playwright.rust.mouse/mouse.${name}`);
console.log('Mouse method range ownership verified against canonical capability schemas');

cargo(['run', '--release', '--locked', '-p', 'playwright-rust-schema-gen', '--',
  '--input', 'crates/playwright-rust-cap-mouse/schema/mouse.yml',
  '--output', 'crates/playwright-rust-codec-mouse/src/generated_schema.rs',
  '--capability-output', 'crates/playwright-rust-cap-mouse/src/generated_capability.rs',
  '--server-output', 'crates/playwright-rust-server-mouse/src/generated_server.rs',
  '--reference-output', 'crates/playwright-rust-cap-mouse/schema/protocol-reference.md',
  '--import', 'crates/playwright-rust-codec-common/schema/common.yml', '--check']);
console.log('Mouse schema, capability, codec model, server chain and reference are fresh');
