import { readFileSync } from 'node:fs';
import { createServer } from 'node:https';

const host = process.env.PLAYWRIGHT_RUST_TEST_HOST;
const port = Number(process.env.PLAYWRIGHT_RUST_TEST_PORT);
const certPath = process.env.PLAYWRIGHT_RUST_TEST_CERT;
const keyPath = process.env.PLAYWRIGHT_RUST_TEST_KEY;

if (!host || !Number.isInteger(port) || !certPath || !keyPath)
  throw new Error('PLAYWRIGHT_RUST_TEST_HOST, PORT, CERT, and KEY are required');

const expectedAuthorization = `Basic ${Buffer.from('user:pass').toString('base64')}`;
const counts = {
  pageChallenges: 0,
  pageSuccesses: 0,
  protectedChallenges: 0,
  protectedSuccesses: 0,
  rejectedChallenges: 0,
};

const server = createServer({ cert: readFileSync(certPath), key: readFileSync(keyPath) }, (request, response) => {
  const url = new URL(request.url ?? '/', `https://${request.headers.host}`);
  if (url.pathname === '/index.html' || url.pathname === '/') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' });
    response.end('<!doctype html><meta charset="utf-8"><title>Service Worker HTTP credentials</title><p>ready</p>');
    return;
  }
  if (url.pathname === '/sw.js') {
    response.writeHead(200, {
      'content-type': 'text/javascript; charset=utf-8',
      'cache-control': 'no-store',
      'service-worker-allowed': '/',
    });
    response.end(`
self.addEventListener('install', event => event.waitUntil(self.skipWaiting()));
self.addEventListener('activate', event => event.waitUntil(self.clients.claim()));
self.addEventListener('message', event => {
  event.waitUntil((async () => {
    try {
      const response = await fetch(event.data.path, { cache: 'no-store' });
      event.ports[0].postMessage({ status: response.status, body: await response.text() });
    } catch (error) {
      event.ports[0].postMessage({ error: String(error) });
    }
  })());
});
`);
    return;
  }
  if (url.pathname === '/protected') {
    if (request.headers.authorization === expectedAuthorization) {
      counts.protectedSuccesses += 1;
      response.writeHead(200, { 'content-type': 'text/plain', 'cache-control': 'no-store' });
      response.end('authenticated-service-worker');
      return;
    }
    counts.protectedChallenges += 1;
    response.writeHead(401, {
      'www-authenticate': 'Basic realm="playwright-rust-service-worker"',
      'content-type': 'text/plain',
      'cache-control': 'no-store',
    });
    response.end('credentials required');
    return;
  }
  if (url.pathname === '/page-protected') {
    if (request.headers.authorization === expectedAuthorization) {
      counts.pageSuccesses += 1;
      response.writeHead(200, { 'content-type': 'text/plain', 'cache-control': 'no-store' });
      response.end('authenticated-page');
      return;
    }
    counts.pageChallenges += 1;
    response.writeHead(401, {
      'www-authenticate': 'Basic realm="playwright-rust-page"',
      'content-type': 'text/plain',
      'cache-control': 'no-store',
    });
    response.end('page credentials required');
    return;
  }
  if (url.pathname === '/reject') {
    counts.rejectedChallenges += 1;
    response.writeHead(401, {
      'www-authenticate': 'Basic realm="playwright-rust-service-worker-reject"',
      'content-type': 'text/plain',
      'cache-control': 'no-store',
    });
    response.end('always rejected');
    return;
  }
  if (url.pathname === '/stats') {
    response.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' });
    response.end(JSON.stringify(counts));
    return;
  }
  response.writeHead(404, { 'content-type': 'text/plain' });
  response.end('not found');
});

server.listen(port, host, () => {
  process.stdout.write(`READY https://${host}:${port}/index.html\n`);
});

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
