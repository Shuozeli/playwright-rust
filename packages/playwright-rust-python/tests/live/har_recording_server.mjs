import { readFileSync } from 'node:fs';
import { createServer as createHttpServer } from 'node:http';
import { createServer as createHttpsServer } from 'node:https';

const host = process.env.PLAYWRIGHT_RUST_TEST_HOST;
const port = Number(process.env.PLAYWRIGHT_RUST_TEST_PORT);
const certPath = process.env.PLAYWRIGHT_RUST_TEST_CERT;
const keyPath = process.env.PLAYWRIGHT_RUST_TEST_KEY;
const prefix = '/playwright-rust-sw-e2e';

if (!host || !Number.isInteger(port))
  throw new Error('PLAYWRIGHT_RUST_TEST_HOST and PLAYWRIGHT_RUST_TEST_PORT are required');

let dataEnabled = true;

const handler = (request, response) => {
  const url = new URL(request.url ?? '/', `http://${request.headers.host}`);
  const path = url.pathname.startsWith(prefix) ? url.pathname.slice(prefix.length) || '/' : url.pathname;
  if (path === '/' || path === '/index.html') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' });
    response.end('<!doctype html><meta charset="utf-8"><title>HAR recording E2E</title><p>ready</p>');
    return;
  }
  if (path === '/sw.js') {
    response.writeHead(200, {
      'content-type': 'text/javascript; charset=utf-8',
      'cache-control': 'no-store',
      'service-worker-allowed': `${prefix}/`,
    });
    response.end(`
self.addEventListener('install', event => event.waitUntil(self.skipWaiting()));
self.addEventListener('activate', event => event.waitUntil(self.clients.claim()));
self.addEventListener('message', event => {
  event.waitUntil((async () => {
    const response = await fetch(event.data.url, { cache: 'no-store' });
    event.ports[0].postMessage({ status: response.status, body: await response.text() });
  })());
});
`);
    return;
  }
  if (path === '/data') {
    if (!dataEnabled) {
      response.writeHead(503, { 'content-type': 'text/plain; charset=utf-8', 'cache-control': 'no-store' });
      response.end('origin-data-disabled');
      return;
    }
    const source = url.searchParams.get('source') ?? 'unknown';
    response.writeHead(200, { 'content-type': 'text/plain; charset=utf-8', 'cache-control': 'no-store' });
    response.end(`${source}-response`);
    return;
  }
  if (path === '/shutdown') {
    response.writeHead(200, { 'content-type': 'text/plain' });
    response.end('data-disabled');
    dataEnabled = false;
    return;
  }
  response.writeHead(404, { 'content-type': 'text/plain' });
  response.end('not found');
};

const server = certPath && keyPath
  ? createHttpsServer({ cert: readFileSync(certPath), key: readFileSync(keyPath) }, handler)
  : createHttpServer(handler);

server.listen(port, host, () => {
  const scheme = certPath && keyPath ? 'https' : 'http';
  process.stdout.write(`READY ${scheme}://${host}:${port}${prefix}/index.html\n`);
});

for (const signal of ['SIGINT', 'SIGTERM'])
  process.on(signal, () => server.close(() => process.exit(0)));
