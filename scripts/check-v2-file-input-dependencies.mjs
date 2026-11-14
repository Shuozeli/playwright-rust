import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
function tree(name, features = [], { format = '{p}', defaultFeatures = false } = {}) {
  const result = spawnSync('cargo', ['tree', '--locked', '-p', name,
    ...(defaultFeatures ? [] : ['--no-default-features']),
    '--edges', 'normal', '--prefix', 'none', '--format', format,
    ...(features.length ? ['--features', features.join(',')] : [])], {
    cwd: root, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim().split('\n');
}
const model = tree('playwright-rust-file-input-model');
assert.equal(model.length, 1, 'file-input policy must remain dependency-free');
assert.match(model[0], /^playwright-rust-file-input-model v\S+/);
const source = tree('playwright-rust-file-input-source');
assert.deepEqual([...new Set(source.map(line => line.split(' ')[0]))].sort(),
  ['playwright-rust-core', 'playwright-rust-file-input-model', 'playwright-rust-file-input-source']);
const filesystem = tree('playwright-rust-file-input-fs');
const filesystemNames = [...new Set(filesystem.map(line => line.split(' ')[0]))].sort();
const filesystemExpected = ['playwright-rust-core', 'playwright-rust-file-input-fs', 'playwright-rust-file-input-model', 'playwright-rust-file-input-source'];
if (process.platform !== 'win32') filesystemExpected.unshift('libc');
assert.deepEqual(filesystemNames, filesystemExpected);
const executor = tree('playwright-rust-file-input-tokio');
assert.deepEqual([...new Set(executor.map(line => line.split(' ')[0]))].sort(),
  [...filesystemExpected, 'playwright-rust-file-input-tokio', 'tokio', 'pin-project-lite'].sort());
const resolutionNames = tree('playwright-rust-backend-cdp-locator-resolution').map(line => line.split(' ')[0]);
const backendNames = [...new Set(tree('playwright-rust-backend-cdp-file-input').map(line => line.split(' ')[0]))].sort();
assert.deepEqual(backendNames, [...new Set([...resolutionNames, 'playwright-rust-backend-cdp-file-input', 'playwright-rust-file-input-model', 'playwright-rust-file-input-source', 'base64'])].sort());
assert.deepEqual([...new Set(tree('playwright-rust-backend-cdp-file-input', ['large-commands']).map(line => line.split(' ')[0]))].sort(), backendNames,
  'selected large commands must not install another transport, runtime, or Node dependency');
const compositionResolution = tree('playwright-rust-composition-chromium-locator-resolution').map(line => line.split(' ')[0]);
const compositionNames = [...new Set(tree('playwright-rust-composition-chromium-file-input').map(line => line.split(' ')[0]))].sort();
assert.deepEqual(compositionNames, [...new Set([...backendNames, ...compositionResolution, 'playwright-rust-composition-chromium-file-input'])].sort());

// Check resolved normal features, not just manifest declarations or package
// counts. Ordinary defaults and large-command graphs must keep prevalidation
// off; selecting it may change only the exact flags below, without widening an
// existing runtime/transport feature or adding a Node/filesystem/executor leaf.
const backendPreflight = 'playwright-rust-backend-cdp-file-input/retained-preflight';
const compositionPreflight = [
  'playwright-rust-composition-chromium-file-input/retained-preflight',
  backendPreflight,
  'playwright-rust-composition-chromium-locator-resolution/retained-reference',
  'playwright-rust-backend-cdp-locator-resolution/retained-reference',
];
function resolvedFeatures(name, features) {
  return tree(name, features, { format: '{p} features:{f}', defaultFeatures: true })
    .map(line => line.replace(/ \(\*\)$/, ''))
    .map(line => {
      const [pkg, selected] = line.split(' features:');
      assert.notEqual(selected, undefined, 'Cargo tree must include resolved features');
      return { pkg, name: pkg.split(' ')[0], features: selected.split(',').filter(Boolean).sort() };
    });
}
function retainedLocations(graph) {
  return [...new Set(graph.flatMap(entry => entry.features
    .filter(feature => ['retained-preflight', 'retained-reference', 'retained-target-marking',
      'retained-target-prevalidation', 'retained-input-instrumentation',
      'retained-call-argument-serialization', 'retained-serialized-payload-import',
      'retained-serialized-payload-mutation'].includes(feature))
    .map(feature => `${entry.name}/${feature}`)))].sort();
}
function fingerprint(graph, removed = []) {
  return [...new Set(graph.map(entry => `${entry.pkg} features:${entry.features
    .filter(feature => !removed.includes(`${entry.name}/${feature}`)).join(',')}`))].sort();
}
for (const [name, largeCommands, expected] of [
  ['playwright-rust-backend-cdp-file-input', 'large-commands', [backendPreflight]],
  ['playwright-rust-composition-chromium-file-input', 'playwright-rust-backend-cdp-file-input/large-commands', compositionPreflight],
]) {
  for (const base of [[], [largeCommands]]) {
    const label = `${name}${base.length ? ' with large commands' : ' with defaults'}`;
    const ordinary = resolvedFeatures(name, base);
    const selected = resolvedFeatures(name, [...base, 'retained-preflight']);
    assert.deepEqual(retainedLocations(ordinary), [], `${label} must not select retained prevalidation or adoption implicitly`);
    assert.deepEqual(retainedLocations(selected), [...expected].sort(),
      `${label} must select exactly its declared retained-preflight/adoption flags`);
    assert.deepEqual(fingerprint(selected, expected), fingerprint(ordinary),
      `${label} retained prevalidation must add no normal packages or unrelated features`);
    const packages = new Set(selected.map(entry => entry.name));
    for (const forbidden of ['napi', 'napi-derive', 'playwright-rust-node-host', 'playwright-rust-node-addon',
      'playwright-rust-file-input-fs', 'playwright-rust-file-input-tokio', 'playwright-rust-server',
      'playwright-rust-bridge', 'playwright-rust-cli', 'playwright-rust-script']) {
      assert.equal(packages.has(forbidden), false,
        `${label} retained prevalidation must not install ${forbidden}`);
    }
  }
}
// Persistent marking is independent of prevalidation and large commands. Its
// exact-world target group requires retained-reference, but no new package,
// filesystem, executor, Node runtime or wider transport selection is allowed.
const backendMarking = [
  'playwright-rust-backend-cdp-file-input/retained-target-marking',
  'playwright-rust-backend-cdp-locator-resolution/retained-reference',
];
const compositionMarking = [
  ...backendMarking,
  'playwright-rust-composition-chromium-file-input/retained-target-marking',
  'playwright-rust-composition-chromium-locator-resolution/retained-reference',
];
for (const [name, largeCommands, expected] of [
  ['playwright-rust-backend-cdp-file-input', 'large-commands', backendMarking],
  ['playwright-rust-composition-chromium-file-input', 'playwright-rust-backend-cdp-file-input/large-commands', compositionMarking],
]) {
  for (const base of [[], [largeCommands], ['retained-preflight'], [largeCommands, 'retained-preflight']]) {
    const label = `${name} marker with ${base.join(',') || 'defaults'}`;
    const ordinary = resolvedFeatures(name, base);
    const selected = resolvedFeatures(name, [...base, 'retained-target-marking']);
    const prior = retainedLocations(ordinary);
    const added = expected.filter(flag => !prior.includes(flag));
    assert.equal(prior.some(flag => flag.endsWith('/retained-target-marking')), false,
      `${label} must not install marking implicitly`);
    assert.deepEqual(retainedLocations(selected), [...new Set([...prior, ...expected])].sort(),
      `${label} must select exactly its marker/adoption flags`);
    assert.deepEqual(fingerprint(selected, added), fingerprint(ordinary),
      `${label} must add no packages or unrelated features`);
  }
}
const ordinaryHost = tree('playwright-rust-node-host').map(line => line.split(' ')[0]);
const selectedHost = tree('playwright-rust-node-host', ['file-input']).map(line => line.split(' ')[0]);
assert.deepEqual([...new Set(selectedHost)].sort(), [...new Set([...ordinaryHost, ...compositionNames,
  ...executor.map(line => line.split(' ')[0])])].sort(),
  'selected safe host must compose the existing upload/executor closures without a new runtime or driver');
const ordinaryAddon = tree('playwright-rust-node-addon').map(line => line.split(' ')[0]);
const selectedAddon = tree('playwright-rust-node-addon', ['file-input']).map(line => line.split(' ')[0]);
assert.deepEqual([...new Set(selectedAddon)].sort(),
  [...new Set([...ordinaryAddon, ...selectedHost])].sort(),
  'selected addon must reuse the exact safe-host closure and existing N-API runtime');
for (const name of ['playwright-rust-node-host', 'playwright-rust-node-addon']) {
  for (const features of [[], ['file-input'], ['retained-file-input']]) {
    const retained = retainedLocations(resolvedFeatures(name, features));
    assert.equal(retained.some(flag => flag.endsWith('/retained-target-marking')), false,
      `${name} ${features.join(',') || 'default'} has not selected native marker integration`);
    assert.equal(retained.some(flag => flag.endsWith('/retained-target-prevalidation')), false,
      `${name} ${features.join(',') || 'default'} has not selected native target prevalidation`);
    assert.equal(retained.some(flag => flag.endsWith('/retained-input-instrumentation')), false,
      `${name} ${features.join(',') || 'default'} has not selected input instrumentation`);
    assert.equal(retained.some(flag => flag.endsWith('/retained-call-argument-serialization')), false,
      `${name} ${features.join(',') || 'default'} has not selected call-argument serialization`);
    assert.equal(retained.some(flag => flag.endsWith('/retained-serialized-payload-import')), false,
      `${name} ${features.join(',') || 'default'} has not selected serialized payload import`);
    assert.equal(retained.some(flag => flag.endsWith('/retained-serialized-payload-mutation')), false,
      `${name} ${features.join(',') || 'default'} has not selected serialized payload mutation`);
    assert.equal(retained.some(flag => flag.endsWith('/retained-preflight')), false,
      `${name} ${features.join(',') || 'default'} has not selected DOM prevalidation`);
  }
  assert.ok(!tree(name).some(line => /^playwright-rust-file-input-(model|source|fs|tokio) /.test(line)),
    `${name} must not install file-input policy without explicit selection`);
  assert.ok(!tree(name).some(line => /^playwright-rust-backend-cdp-file-input /.test(line)), `${name} must not install the file-input backend implicitly`);
  assert.ok(!tree(name).some(line => /^playwright-rust-composition-chromium-file-input /.test(line)), `${name} must not install upload continuations implicitly`);
}
const hostMarkerFlags = [
  'playwright-rust-node-host/retained-target-marking',
  'playwright-rust-composition-chromium-file-input/retained-target-marking',
  'playwright-rust-backend-cdp-file-input/retained-target-marking',
];
for (const base of [[], ['concurrent-close', 'extra-headers', 'drag-runtime', 'frame-navigation']]) {
  const retainedHost = resolvedFeatures('playwright-rust-node-host', ['retained-file-input', ...base]);
  const markingHost = resolvedFeatures('playwright-rust-node-host', ['retained-target-marking', ...base]);
  assert.deepEqual(fingerprint(markingHost, hostMarkerFlags), fingerprint(retainedHost),
    `selected Rust host marking ${base.join(',') || 'standalone'} must add only exact marker flags, not packages or other features`);
  assert.deepEqual(retainedLocations(markingHost),
    [...new Set([...retainedLocations(retainedHost), ...hostMarkerFlags])].sort());
}
const hostPrevalidationFlags = [
  'playwright-rust-node-host/retained-target-prevalidation',
  'playwright-rust-composition-chromium-file-input/retained-target-prevalidation',
  'playwright-rust-composition-chromium-file-input/retained-preflight',
  'playwright-rust-backend-cdp-file-input/retained-preflight',
];
for (const name of ['playwright-rust-node-host', 'playwright-rust-node-addon']) {
  for (const base of [[], ['concurrent-close', 'extra-headers', 'drag-runtime', 'frame-navigation']]) {
    const marking = resolvedFeatures(name, ['retained-target-marking', ...base]);
    const prevalidation = resolvedFeatures(name, ['retained-target-prevalidation', ...base]);
    const expected = name === 'playwright-rust-node-addon'
      ? ['playwright-rust-node-addon/retained-target-prevalidation', ...hostPrevalidationFlags]
      : hostPrevalidationFlags;
    assert.deepEqual(fingerprint(prevalidation, expected), fingerprint(marking),
      `${name} target prevalidation ${base.join(',') || 'standalone'} must add only exact prevalidation flags`);
    assert.deepEqual(retainedLocations(prevalidation),
      [...new Set([...retainedLocations(marking), ...expected])].sort(),
      `${name} target prevalidation must compose on the exact marker graph`);
  }
}
for (const base of [[], ['concurrent-close', 'extra-headers', 'drag-runtime', 'frame-navigation']]) {
  const prevalidation = resolvedFeatures('playwright-rust-node-addon', ['retained-target-prevalidation', ...base]);
  const instrumentation = resolvedFeatures('playwright-rust-node-addon', ['retained-input-instrumentation', ...base]);
  const expected = ['playwright-rust-node-addon/retained-input-instrumentation'];
  assert.deepEqual(fingerprint(instrumentation, expected), fingerprint(prevalidation),
    `addon input instrumentation ${base.join(',') || 'standalone'} must add only its exact phase flag`);
  assert.deepEqual(retainedLocations(instrumentation),
    [...new Set([...retainedLocations(prevalidation), ...expected])].sort(),
    'addon input instrumentation must compose on the exact prevalidation graph');
}
for (const base of [[], ['concurrent-close', 'extra-headers', 'drag-runtime', 'frame-navigation']]) {
  const instrumentation = resolvedFeatures('playwright-rust-node-addon', ['retained-input-instrumentation', ...base]);
  const serialization = resolvedFeatures('playwright-rust-node-addon', ['retained-call-argument-serialization', ...base]);
  const expected = ['playwright-rust-node-addon/retained-call-argument-serialization'];
  assert.deepEqual(fingerprint(serialization, expected), fingerprint(instrumentation),
    `addon call-argument serialization ${base.join(',') || 'standalone'} must add only its exact phase flag`);
  assert.deepEqual(retainedLocations(serialization),
    [...new Set([...retainedLocations(instrumentation), ...expected])].sort(),
    'addon call-argument serialization must compose on the exact instrumentation graph');
}
for (const base of [[], ['concurrent-close', 'extra-headers', 'drag-runtime', 'frame-navigation']]) {
  const serialization = resolvedFeatures('playwright-rust-node-addon', ['retained-call-argument-serialization', ...base]);
  const imported = resolvedFeatures('playwright-rust-node-addon', ['retained-serialized-payload-import', ...base]);
  const expected = [
    'playwright-rust-node-addon/retained-serialized-payload-import',
    'playwright-rust-node-host/retained-serialized-payload-import',
  ];
  assert.deepEqual(fingerprint(imported, expected), fingerprint(serialization),
    `serialized payload import ${base.join(',') || 'standalone'} must add only its addon/host owner flags`);
  assert.deepEqual(retainedLocations(imported),
    [...new Set([...retainedLocations(serialization), ...expected])].sort(),
    'serialized payload import must compose on the exact serialization graph');
}
for (const base of [[], ['concurrent-close', 'extra-headers', 'drag-runtime', 'frame-navigation']]) {
  const imported = resolvedFeatures('playwright-rust-node-addon', ['retained-serialized-payload-import', ...base]);
  const mutated = resolvedFeatures('playwright-rust-node-addon', ['retained-serialized-payload-mutation', ...base]);
  const expected = [
    'playwright-rust-node-addon/retained-serialized-payload-mutation',
    'playwright-rust-node-host/retained-serialized-payload-mutation',
    'playwright-rust-composition-chromium-file-input/retained-serialized-payload-mutation',
    'playwright-rust-backend-cdp-file-input/retained-serialized-payload-mutation',
  ];
  assert.deepEqual(fingerprint(mutated, expected), fingerprint(imported),
    `serialized payload mutation ${base.join(',') || 'standalone'} must add only exact mutation flags`);
  assert.deepEqual(retainedLocations(mutated),
    [...new Set([...retainedLocations(imported), ...expected])].sort(),
    'serialized payload mutation must compose on the exact import graph');
}
process.stdout.write('File-input closures and independent preflight/marking/prevalidation/instrumentation/serialization/import/mutation features verified; only explicit target selections install each leaf. Existing host/addon profiles remain unselected and ordinary profiles exclude all leaves.\n');
