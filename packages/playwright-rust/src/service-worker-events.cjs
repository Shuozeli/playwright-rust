'use strict';

const { EventEmitter } = require('node:events');

module.exports = function serviceWorkerEvents({ mapError, UnsupportedOperationError }) {
  class Worker extends EventEmitter {
    constructor(context, descriptor) {
      super();
      this._context = context;
      this._id = descriptor.id;
      this._url = descriptor.url;
      this._owner = descriptor.owner;
      this._closed = false;
    }

    _adopt(descriptor) {
      this._url = descriptor.url;
      this._owner = descriptor.owner;
    }

    _didClose() {
      if (this._closed) return;
      this._closed = true;
      this.emit('close', this);
    }

    url() { return this._url; }

    async evaluate(pageFunction, arg) {
      const session = this._context._browser._session;
      if (typeof session.workerEvaluate !== 'function')
        throw new UnsupportedOperationError('worker.evaluate (select worker-network-events native profile)');
      try {
        return await session.workerEvaluate(
          this._owner,
          String(pageFunction),
          typeof pageFunction === 'function',
          arg,
        );
      } catch (error) {
        throw mapError(error, 'worker.evaluate');
      }
    }
  }

  function wrap(context, descriptor) {
    let worker = context._serviceWorkers.get(descriptor.id);
    if (worker) worker._adopt(descriptor);
    else {
      worker = new Worker(context, descriptor);
      context._serviceWorkers.set(descriptor.id, worker);
    }
    return worker;
  }

  function snapshot(context) {
    const session = context._browser._session;
    if (typeof session.contextServiceWorkers !== 'function')
      throw new UnsupportedOperationError('browserContext.serviceWorkers (select worker-network-events native profile)');
    let descriptors;
    try { descriptors = session.contextServiceWorkers(context._id); }
    catch (error) { throw mapError(error, 'browserContext.serviceWorkers'); }
    const active = new Set(descriptors.map(descriptor => descriptor.id));
    for (const [id, worker] of context._serviceWorkers) {
      if (active.has(id)) continue;
      context._serviceWorkers.delete(id);
      worker._didClose();
    }
    return descriptors.map(descriptor => wrap(context, descriptor));
  }

  function dispatch(browser, notice) {
    const context = browser._contexts.get(notice.contextId);
    if (!context) return;
    const worker = wrap(context, notice.worker);
    if (notice.kind === 'created') {
      context.emit('serviceworker', worker);
      return;
    }
    if (notice.kind !== 'closed')
      throw new Error(`Unsupported native service-worker notice kind: ${notice.kind}`);
    context._serviceWorkers.delete(worker._id);
    worker._didClose();
  }

  function watch(session, browserRef) {
    if (typeof session.nextServiceWorkerNotice !== 'function') return;
    const next = () => {
      Promise.resolve().then(() => session.nextServiceWorkerNotice()).then(notice => {
        if (!notice) return;
        const browser = browserRef.deref();
        if (!browser) return;
        dispatch(browser, notice);
        next();
      }).catch(error => {
        const browser = browserRef.deref();
        if (browser?.isConnected()) browser._didWatchFail(error);
      });
    };
    next();
  }

  function retire(context) {
    const workers = [...context._serviceWorkers.values()];
    context._serviceWorkers.clear();
    for (const worker of workers) worker._didClose();
  }

  return { Worker, dispatch, retire, snapshot, watch, wrap };
};
