import assert from 'node:assert/strict';
import { test } from 'node:test';

const endpoint = process.env.PLAYWRIGHT_RUST_TEST_CDP;
if (!endpoint) throw new Error('PLAYWRIGHT_RUST_TEST_CDP is required');

async function isolatedContext() {
  const { chromium } = await import('../src/index.mjs');
  const browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
  const context = await browser.newContext();
  return { browser, context };
}

async function closeFixture(fixture) {
  await fixture.context.close();
  await fixture.browser.close();
}

test('real addon returns Playwright Cookie fields and filters one URL', async () => {
  // Arrange
  const fixture = await isolatedContext();
  try {
    await fixture.context.addCookies([
      { name: 'api', value: 'one', domain: '.example.test', path: '/api', secure: true },
      { name: 'other', value: 'two', domain: 'other.test', path: '/' },
    ]);

    // Act
    const cookies = await fixture.context.cookies('https://sub.example.test/api/value');

    // Assert
    assert.deepEqual(cookies, [{
      name: 'api',
      value: 'one',
      domain: '.example.test',
      path: '/api',
      expires: -1,
      httpOnly: false,
      secure: true,
      sameSite: 'Lax',
    }]);
  } finally {
    await closeFixture(fixture);
  }
});

test('real addon clears only cookies matching every string and RegExp filter', async () => {
  // Arrange
  const fixture = await isolatedContext();
  try {
    await fixture.context.addCookies([
      { name: 'cookie1', value: 'one', domain: 'example.test', path: '/api/v1' },
      { name: 'cookie2', value: 'two', domain: 'example.test', path: '/api/v2' },
      { name: 'cookie1', value: 'three', domain: 'other.test', path: '/api/v1' },
    ]);

    // Act
    await fixture.context.clearCookies({ name: /^cookie1$/, domain: 'example.test', path: /v1$/ });

    // Assert
    assert.deepEqual(
      (await fixture.context.cookies()).map(({ name, value, domain, path }) => ({ name, value, domain, path })),
      [
        { name: 'cookie2', value: 'two', domain: 'example.test', path: '/api/v2' },
        { name: 'cookie1', value: 'three', domain: 'other.test', path: '/api/v1' },
      ],
    );
  } finally {
    await closeFixture(fixture);
  }
});

test('real addon clears one BrowserContext without changing another', async () => {
  // Arrange
  const { chromium } = await import('../src/index.mjs');
  const browser = await chromium.connectOverCDP(endpoint, { timeout: 5_000 });
  const context = await browser.newContext();
  const other = await browser.newContext();
  try {
    const cookie = { name: 'session', value: 'value', domain: 'example.test', path: '/' };
    await context.addCookies([cookie]);
    await other.addCookies([cookie]);

    // Act
    await context.clearCookies();

    // Assert
    assert.deepEqual(await context.cookies(), []);
    assert.equal((await other.cookies()).length, 1);
  } finally {
    await context.close();
    await other.close();
    await browser.close();
  }
});

test('real addon roundtrips Chromium partition ancestor identity', async () => {
  // Arrange
  const fixture = await isolatedContext();
  try {
    await fixture.context.addCookies([{
      name: 'partitioned',
      value: 'value',
      domain: 'example.test',
      path: '/',
      secure: true,
      sameSite: 'None',
      partitionKey: 'https://top.test',
      _crHasCrossSiteAncestor: false,
    }]);

    // Act
    const [cookie] = await fixture.context.cookies();

    // Assert
    assert.equal(cookie.partitionKey, 'https://top.test');
    assert.equal(cookie._crHasCrossSiteAncestor, false);
  } finally {
    await closeFixture(fixture);
  }
});
