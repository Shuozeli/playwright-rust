import assert from 'node:assert/strict';
import { test } from 'node:test';
import { chromium } from '@shuozeli/playwright-rust';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;

async function targets() {
  const response = await fetch(`${endpoint.replace(/\/$/, '')}/json/list`);
  assert.equal(response.ok, true);
  return response.json();
}

test('connectOverCDP close retires only the Page created by this attachment', {
  skip: !endpoint,
}, async () => {
  // Arrange
  const before = await targets();
  const beforeIds = new Set(before.map(target => target.id));
  const marker = `pw-rust-node-owned-${Date.now()}`;
  const browser = await chromium.connectOverCDP(endpoint, {
    headers: {
      'X-BCP-Lease-Id': 'node-attach-safe-close',
      'X-BCP-Fencing-Token': '1',
    },
  });
  const context = browser.contexts()[0];
  assert.ok(context, 'the shared Chrome default Context must exist');
  const page = await context.newPage();
  await page.goto(`data:text/html,<title>${marker}</title><main>${marker}</main>`, {
    waitUntil: 'commit',
  });
  assert.match(await page.content(), new RegExp(marker));
  assert.equal(await page.evaluate('document.title'), marker);
  const createdId = (await targets()).find(target => target.title === marker)?.id;
  assert.ok(createdId, 'the attachment-created target must be observable before close');

  // Act
  await Promise.race([
    browser.close(),
    new Promise((_, reject) => setTimeout(() => reject(new Error('browser.close timed out')), 5000)),
  ]);

  // Assert
  const after = await targets();
  const afterIds = new Set(after.map(target => target.id));
  assert.equal(afterIds.has(createdId), false, 'the attachment-created target must be closed');
  for (const id of beforeIds)
    assert.equal(afterIds.has(id), true, `pre-existing target ${id} must survive`);
  assert.equal(browser.isConnected(), false);
});
