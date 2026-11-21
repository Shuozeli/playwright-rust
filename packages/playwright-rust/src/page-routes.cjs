'use strict';
const { types: { isRegExp } } = require('node:util');

module.exports = function pageRoutes({ mapError, runTracingAction = (_owner, _action, _detail, operation) => operation(), validateOptions, UnsupportedOperationError, createResponseFromNotice, canonicalizeRequest }) {
  function isContextOwner(owner) { return owner?._browser != null && owner?._context == null; }
  function routeApi(owner) { return isContextOwner(owner) ? 'browserContext.route' : 'page.route'; }
  function unrouteApi(owner) { return isContextOwner(owner) ? 'browserContext.unroute' : 'page.unroute'; }
  function ownerSession(owner) { return isContextOwner(owner) ? owner._browser._session : owner._context._browser._session; }

  function routeMatcher(url, session, api, regexpMethod) {
    if (typeof url === 'string') return { kind: 'string', url };
    if (!isRegExp(url))
      throw new UnsupportedOperationError(`${api} non-string URL matcher`);
    if (typeof session[regexpMethod] !== 'function')
      throw new UnsupportedOperationError(`${api} RegExp URL matcher (select route-regexp native profile)`);
    const source = url.source;
    const flags = url.flags;
    return { kind: 'regexp', url, source, flags };
  }

  function sameRouteMatcher(registration, matcher) {
    if (matcher.kind === 'string')
      return typeof registration.url === 'string' && registration.url === matcher.url;
    return registration.regexpSource === matcher.source && registration.regexpFlags === matcher.flags;
  }

  function registerRoute(session, owner, matcher, times) {
    const context = isContextOwner(owner);
    return matcher.kind === 'string'
      ? session[context ? 'contextRoute' : 'pageRoute'](owner._id, matcher.url, times)
      : session[context ? 'contextRouteRegExp' : 'pageRouteRegExp'](
        owner._id, matcher.source, matcher.flags, times,
      );
  }

  function enqueueRouteMutation(owner, api, mutation) {
    const previous = owner._routeMutationTail ?? Promise.resolve();
    const result = previous.then(mutation);
    owner._routeMutationTail = result.then(() => undefined, () => undefined);
    return result;
  }

  class Request {
    constructor(owner, call, historyOwner = null) {
      const contextOwned = owner?._browser != null && owner?._context == null;
      this._page = contextOwned ? null : owner;
      this._context = contextOwned ? owner : owner?._context;
      this._call = call;
      this._frame = this._captureFrame(call.frameId);
      this._historyOwner = historyOwner;
      this._response = null;
      this._responsePromise = null;
      this._failureText = null;
      this._redirectedFrom = null;
      this._redirectedTo = null;
      this._headers = Object.fromEntries(
        call.headers.map(({ name, value }) => [name.toLowerCase(), value]),
      );
    }

    _adoptNetworkNotice(call, historyOwner) {
      // Network.requestWillBeSent may omit a body already captured by Fetch
      // for this hop. Keep that body when a later notice updates its metadata.
      this._call = typeof call.id === 'string' && call.id === this._call.id
        && call.postData == null && this._call.postData != null
        ? { ...call, postData: this._call.postData }
        : call;
      if (this._frame == null) this._frame = this._captureFrame(this._call.frameId);
      if (historyOwner != null) this._historyOwner = historyOwner;
      this._headers = Object.fromEntries(
        call.headers.map(({ name, value }) => [name.toLowerCase(), value]),
      );
    }

    _setResponse(response) { this._response = response; }
    _setFailure(failureText) { this._failureText = failureText; }
    _linkRedirectedFrom(previous) {
      this._redirectedFrom = previous;
      previous._redirectedTo = this;
    }

    url() { return this._call.url; }
    method() { return this._call.method; }
    resourceType() { return this._call.resourceType; }
    headers() { return { ...this._headers }; }
    allHeaders() { return Promise.resolve({ ...this._headers }); }
    headersArray() { return this._call.headers.map(({ name, value }) => ({ name, value })); }
    postDataBuffer() { return this._call.postData ?? null; }
    postData() { return this._call.postData?.toString('utf8') ?? null; }
    postDataJSON() {
      if (this._call.postDataJsonAvailable !== true)
        throw new UnsupportedOperationError('request.postDataJSON (select request-post-data-json native profile)');
      if (this._call.postDataJsonError !== null && this._call.postDataJsonError !== undefined)
        throw new Error(this._call.postDataJsonError);
      return this._call.postDataJson ?? null;
    }
    isNavigationRequest() { return this._call.isNavigationRequest; }
    response() {
      if (this._response) return Promise.resolve(this._response);
      if (this._responsePromise) return this._responsePromise;
      const session = this._context._browser._session;
      if (this._historyOwner == null || typeof session.requestResponse !== 'function'
          || typeof createResponseFromNotice !== 'function')
        return Promise.reject(new UnsupportedOperationError('request.response (select request-lifecycle native profile)'));
      this._responsePromise = Promise.resolve().then(() => session.requestResponse(this._historyOwner))
        .then(notice => notice == null ? null : createResponseFromNotice(this._page ?? this._context, notice, this))
        .catch(error => { throw mapError(error, 'request.response'); });
      return this._responsePromise;
    }
    failure() {
      const session = this._context._browser._session;
      let failureText = this._failureText;
      if (this._historyOwner != null && typeof session.requestFailure === 'function') {
        try { failureText = session.requestFailure(this._historyOwner); }
        catch (error) { throw mapError(error, 'request.failure'); }
      }
      return failureText == null ? null : { errorText: failureText };
    }
    redirectedFrom() { return this._redirectedFrom; }
    redirectedTo() { return this._redirectedTo; }
    serviceWorker() {
      const id = this._call.serviceWorkerId;
      if (typeof id !== 'string') return null;
      const known = this._context._serviceWorkers?.get(id);
      if (known) return known;
      if (typeof this._context.serviceWorkers !== 'function') return null;
      return this._context.serviceWorkers().find(worker => worker._id === id) ?? null;
    }
    _captureFrame(frameId) {
      // Capturing an existing wrapper must not enumerate native Frames or
      // require a capability that this Page did not select.
      if (frameId == null) return null;
      if (!this._page) return null;
      return this._page._frameCache?.knownFrame?.(frameId) ?? null;
    }
    frame() {
      if (this._call.serviceWorkerId != null)
        throw new Error('Service Worker requests do not have an associated frame.');
      if (this._frame != null) return this._frame;
      if (this._call.frameId == null || !this._page) return null;
      // A network notice may precede its Frame publication. Resolve it later
      // if it was not known when this Request wrapper was created.
      this._frame = this._page.frames().find(frame => frame._id === this._call.frameId) ?? null;
      return this._frame;
    }
  }

  class Route {
    constructor(routeOwner, requestOwner, call) {
      this._routeOwner = routeOwner;
      this._context = isContextOwner(routeOwner) ? routeOwner : routeOwner._context;
      this._callId = call.callId;
      // The opt-in native history owner carries the network event's exact hop
      // identity. Share that Request with the event stream so a redirect that
      // is not re-intercepted still links back to the routed Request object.
      this._request = call.historyOwner != null && typeof call.requestId === 'string'
        && typeof canonicalizeRequest === 'function'
        ? canonicalizeRequest(requestOwner, {
          ...call,
          id: call.requestId,
          redirectedFrom: call.redirectedFrom ?? null,
        }, call.historyOwner)
        : new Request(requestOwner, call, call.historyOwner ?? null);
      this._handled = false;
    }

    request() { return this._request; }

    async continue(options = {}) {
      validateOptions(options, ['headers'], 'route.continue');
      const session = this._context._browser._session;
      let headers;
      if (options.headers !== undefined) {
        if (options.headers === null || typeof options.headers !== 'object' || Array.isArray(options.headers))
          throw new TypeError('route.continue: headers must be an object');
        if (typeof session.pageRouteContinueWithHeaders !== 'function')
          throw new UnsupportedOperationError('route.continue headers (select route-header-url-overrides native profile)');
        const entries = Object.entries(options.headers);
        if (entries.length > 256)
          throw new TypeError('route.continue: headers exceed count limit (256)');
        let bytes = 0;
        headers = entries.map(([name, value]) => {
          if (typeof value !== 'string')
            throw new TypeError(`route.continue: header ${name} must be a string`);
          bytes += Buffer.byteLength(name) + Buffer.byteLength(value);
          if (bytes > 64 * 1024)
            throw new TypeError('route.continue: headers exceed byte limit (65536)');
          return { name, value };
        });
      }
      this._claim('route.continue');
      try {
        await runTracingAction(this._request._page ?? this._context, 'route-continue', undefined,
          async () => {
            if (headers === undefined) await session.pageRouteContinue(this._callId);
            else await session.pageRouteContinueWithHeaders(this._callId, headers);
          });
      } catch (error) {
        this._handled = false;
        throw mapError(error, 'route.continue');
      }
    }

    async fallback(options = {}) {
      validateOptions(options, ['url', 'method', 'headers', 'postData'], 'route.fallback');
      const session = this._context._browser._session;
      if (options.url !== undefined) {
        if (typeof options.url !== 'string')
          throw new TypeError('route.fallback: url must be a string');
      }
      if (options.method !== undefined && typeof options.method !== 'string')
        throw new TypeError('route.fallback: method must be a string');
      let headers;
      if (options.headers !== undefined) {
        if (options.headers === null || typeof options.headers !== 'object' || Array.isArray(options.headers))
          throw new TypeError('route.fallback: headers must be an object');
        const entries = Object.entries(options.headers);
        if (entries.length > 256)
          throw new TypeError('route.fallback: headers exceed count limit (256)');
        let bytes = 0;
        headers = entries.map(([name, value]) => {
          const normalized = String(value);
          bytes += Buffer.byteLength(name) + Buffer.byteLength(normalized);
          if (bytes > 64 * 1024)
            throw new TypeError('route.fallback: headers exceed byte limit (65536)');
          return { name, value: normalized };
        });
      }
      let postData;
      if (options.postData !== undefined) {
        if (Buffer.isBuffer(options.postData)) postData = options.postData;
        else if (typeof options.postData === 'string') postData = Buffer.from(options.postData);
        else {
          const encoded = JSON.stringify(options.postData);
          if (encoded === undefined) throw new TypeError('route.fallback: postData must be serializable');
          postData = Buffer.from(encoded);
        }
      }
      const hasOverrides = options.url !== undefined || options.method !== undefined
        || headers !== undefined || postData !== undefined;
      this._claim('route.fallback');
      try {
        if (!hasOverrides) await session.pageRouteFallback(this._callId);
        else if (typeof session.pageRouteFallbackWithOverrides === 'function') {
          await session.pageRouteFallbackWithOverrides(
            this._callId, options.url, options.method, headers, postData,
          );
        } else if (options.url !== undefined && options.method === undefined
            && headers === undefined && postData === undefined
            && typeof session.pageRouteFallbackWithUrl === 'function') {
          await session.pageRouteFallbackWithUrl(this._callId, options.url);
        } else {
          throw new UnsupportedOperationError('route.fallback overrides (select route-header-url-overrides native profile)');
        }
      } catch (error) {
        this._handled = false;
        throw mapError(error, 'route.fallback');
      }
    }

    async abort(errorCode) {
      if (errorCode !== undefined && typeof errorCode !== 'string')
        throw new TypeError('route.abort: errorCode must be a string');
      this._claim('route.abort');
      try {
        await this._context._browser._session.pageRouteAbort(this._callId, errorCode);
      } catch (error) {
        this._handled = false;
        throw mapError(error, 'route.abort');
      }
    }

    async fulfill(options = {}) {
      const api = 'route.fulfill';
      validateOptions(options, ['status', 'headers', 'body', 'json', 'path', 'contentType', 'response'], api);
      if (options.path !== undefined)
        throw new UnsupportedOperationError(`${api} path (select route-fulfill-path native profile)`);
      if (options.response !== undefined)
        throw new UnsupportedOperationError(`${api} response (select route-fulfill-api-response native profile)`);
      if (options.body !== undefined && options.json !== undefined)
        throw new Error(`${api}: json and body are mutually exclusive`);
      const status = options.status ?? 200;
      if (!Number.isInteger(status) || status < 100 || status > 999)
        throw new TypeError(`${api}: status must be an integer in [100, 999]`);
      if (options.contentType !== undefined && typeof options.contentType !== 'string')
        throw new TypeError(`${api}: contentType must be a string`);
      if (options.headers !== undefined &&
          (options.headers === null || typeof options.headers !== 'object' || Array.isArray(options.headers)))
        throw new TypeError(`${api}: headers must be an object`);
      let headers = Object.entries(options.headers ?? {}).map(([name, value]) => {
        if (typeof value !== 'string') throw new TypeError(`${api}: header ${name} must be a string`);
        return { name, value };
      });
      let body = options.body;
      if (options.json !== undefined) {
        const encoded = JSON.stringify(options.json);
        if (encoded === undefined) throw new TypeError(`${api}: json must be serializable`);
        body = encoded;
      }
      let contentType = options.contentType;
      if (contentType === undefined && options.json !== undefined) contentType = 'application/json';
      if (contentType !== undefined) {
        headers = headers.filter(header => header.name.toLowerCase() !== 'content-type');
        headers.push({ name: 'content-type', value: contentType });
      }
      if (body === undefined) body = Buffer.alloc(0);
      else if (typeof body === 'string') body = Buffer.from(body);
      else if (!Buffer.isBuffer(body)) throw new TypeError(`${api}: body must be a string or Buffer`);
      this._claim(api);
      try {
        await this._context._browser._session.pageRouteFulfill(
          this._callId, status, headers, body,
        );
      } catch (error) {
        this._handled = false;
        throw mapError(error, api);
      }
    }
    async fetch() { throw new UnsupportedOperationError('route.fetch'); }

    _claim(api) {
      if (this._handled) throw new Error(`${api}: Route is already handled!`);
      this._handled = true;
    }
  }

  async function route(owner, url, handler, options = {}) {
    const api = routeApi(owner);
    validateOptions(options, ['times'], api);
    const session = ownerSession(owner);
    const context = isContextOwner(owner);
    const matcher = routeMatcher(
      url, session, api, context ? 'contextRouteRegExp' : 'pageRouteRegExp',
    );
    if (typeof handler !== 'function') throw new TypeError(`${api}: handler must be a function`);
    const times = options.times;
    if (times !== undefined && (!Number.isInteger(times) || times < 1 || times > 0xffff_ffff))
      throw new TypeError(`${api}: times must be an integer in [1, 4294967295]`);
    if (typeof session[context ? 'contextRoute' : 'pageRoute'] !== 'function')
      throw new UnsupportedOperationError(`${api} (select page-routes native profile)`);
    if (typeof session[context ? 'contextUnroute' : 'pageUnroute'] !== 'function') {
      let registration;
      try { registration = await registerRoute(session, owner, matcher, times); }
      catch (error) { throw mapError(error, api); }
      owner._routes.set(String(registration), handler);
      return;
    }
    return enqueueRouteMutation(owner, api, async () => {
      let registration;
      try { registration = await registerRoute(session, owner, matcher, times); }
      catch (error) { throw mapError(error, api); }
      const id = String(registration);
      owner._routes.set(id, handler);
      owner._routeRegistrations ??= new Map();
      owner._routeRegistrations.set(id, matcher.kind === 'string'
        ? { url, handler }
        : { url, handler, regexpSource: matcher.source, regexpFlags: matcher.flags });
    });
  }

  async function routeFromHAR(owner, har, options = {}) {
    const api = isContextOwner(owner) ? 'browserContext.routeFromHAR' : 'page.routeFromHAR';
    if (typeof har !== 'string') throw new TypeError(`${api}: har must be a string`);
    if (options === null || typeof options !== 'object' || Array.isArray(options))
      throw new TypeError(`${api}: options must be an object`);
    validateOptions(
      options,
      ['url', 'notFound', 'update', 'updateContent', 'updateMode'],
      api,
    );
    if (options.notFound !== undefined && !['abort', 'fallback'].includes(options.notFound))
      throw new TypeError(`${api}: notFound must be "abort" or "fallback"`);
    if (options.update !== undefined && typeof options.update !== 'boolean')
      throw new TypeError(`${api}: update must be a boolean`);
    if (options.updateContent !== undefined && !['attach', 'embed'].includes(options.updateContent))
      throw new TypeError(`${api}: updateContent must be "attach" or "embed"`);
    if (options.updateMode !== undefined && !['minimal', 'full'].includes(options.updateMode))
      throw new TypeError(`${api}: updateMode must be "minimal" or "full"`);

    const session = ownerSession(owner);
    const context = isContextOwner(owner);
    const update = options.update === true;
    let matcher;
    if (options.url === undefined) matcher = { kind: 'string', url: '**/*' };
    else if (typeof options.url === 'string') matcher = { kind: 'string', url: options.url };
    else if (isRegExp(options.url)) {
      matcher = {
        kind: 'regexp',
        url: options.url,
        source: options.url.source,
        flags: options.url.flags,
      };
    } else {
      throw new UnsupportedOperationError(`${api} non-string URL matcher`);
    }
    const stringMethod = update
      ? (context ? 'contextRecordHar' : 'pageRecordHar')
      : (context ? 'contextRouteFromHar' : 'pageRouteFromHar');
    const regexpMethod = update
      ? (context ? 'contextRecordHarRegExp' : 'pageRecordHarRegExp')
      : (context ? 'contextRouteFromHarRegExp' : 'pageRouteFromHarRegExp');
    const selectedMethod = matcher.kind === 'regexp' ? regexpMethod : stringMethod;
    if (typeof session[selectedMethod] !== 'function') {
      const profile = update ? 'har-recording' : 'har-replay';
      const regexp = matcher.kind === 'regexp' ? '-regexp' : '';
      throw new UnsupportedOperationError(`${api} (select ${profile}${regexp} native profile)`);
    }

    return enqueueRouteMutation(owner, api, async () => {
      let registration;
      try {
        if (update) {
          const content = options.updateContent ?? 'attach';
          const mode = options.updateMode ?? 'minimal';
          registration = matcher.kind === 'regexp'
            ? await session[regexpMethod](
              owner._id, har, matcher.source, matcher.flags, { content, mode },
            )
            : await session[stringMethod](
              owner._id,
              har,
              options.url === undefined ? undefined : matcher.url,
              content,
              mode,
            );
        } else {
          const notFound = options.notFound ?? 'abort';
          registration = matcher.kind === 'regexp'
            ? await session[regexpMethod](
              owner._id, har, matcher.source, matcher.flags, notFound,
            )
            : await session[stringMethod](
              owner._id,
              har,
              options.url === undefined ? undefined : matcher.url,
              notFound,
            );
        }
      } catch (error) { throw mapError(error, api); }
      const id = String(registration);
      owner._routeRegistrations ??= new Map();
      owner._routeRegistrations.set(id, matcher.kind === 'string'
        ? { url: matcher.url }
        : { url: matcher.url, regexpSource: matcher.source, regexpFlags: matcher.flags });
    });
  }

  async function unroute(owner, url, handler) {
    const api = unrouteApi(owner);
    const session = ownerSession(owner);
    const context = isContextOwner(owner);
    const matcher = routeMatcher(
      url, session, api, context ? 'contextRouteRegExp' : 'pageRouteRegExp',
    );
    if (handler !== undefined && typeof handler !== 'function')
      throw new TypeError(`${api}: handler must be a function`);
    const nativeUnroute = context ? 'contextUnroute' : 'pageUnroute';
    if (typeof session[nativeUnroute] !== 'function')
      throw new UnsupportedOperationError(`${api} (select page-unroute native profile)`);
    return enqueueRouteMutation(owner, api, async () => {
      const registrations = owner._routeRegistrations;
      if (!registrations) return;
      const ids = [...registrations]
        .filter(([, registration]) => sameRouteMatcher(registration, matcher)
          && (handler === undefined || registration.handler === handler))
        .map(([id]) => id);
      if (ids.length === 0) return;
      try { await session[nativeUnroute](owner._id, ids); }
      catch (error) { throw mapError(error, api); }
      for (const id of ids) {
        registrations.delete(id);
        owner._routes.delete(id);
      }
    });
  }

  async function unrouteAll(owner, options = {}) {
    const api = isContextOwner(owner) ? 'browserContext.unrouteAll' : 'page.unrouteAll';
    validateOptions(options, ['behavior'], api);
    const behavior = options.behavior ?? 'default';
    if (!['default', 'wait', 'ignoreErrors'].includes(behavior))
      throw new TypeError(`${api}: behavior must be "default", "wait", or "ignoreErrors"`);
    const session = ownerSession(owner);
    const context = isContextOwner(owner);
    const nativeUnrouteAll = context ? 'contextUnrouteAll' : 'pageUnrouteAll';
    if (typeof session[nativeUnrouteAll] !== 'function')
      throw new UnsupportedOperationError(`${api} (select page-unroute native profile)`);
    return enqueueRouteMutation(owner, api, async () => {
      const active = [...(owner._activeRouteHandlers ?? [])];
      if (behavior === 'ignoreErrors') {
        for (const token of active) token.ignoreErrors = true;
      }
      // Playwright removes language-side handlers before awaiting protocol
      // interception updates, so no new callback can start after unrouteAll.
      owner._routes.clear();
      owner._routeRegistrations?.clear();
      // The official wait behavior drains active callbacks before updating
      // interception patterns. Disabling CDP Fetch first would invalidate a
      // request that an active callback still owns and intends to settle.
      if (behavior === 'wait') await Promise.all(active.map(token => token.done));
      try { await session[nativeUnrouteAll](owner._id, false); }
      catch (error) { throw mapError(error, api); }
    });
  }

  async function dispatch(browser, call) {
    const context = browser._contexts.get(call.contextId);
    const page = context?._pages.get(call.pageId);
    const pageHandler = page?._routes?.get(call.registrationId);
    const contextHandler = context?._routes?.get(call.registrationId);
    const routeOwner = pageHandler ? page : contextHandler ? context : null;
    const handler = pageHandler ?? contextHandler;
    if (!routeOwner || !handler || (!page && !context)) {
      await browser._session.pageRouteFallback(call.callId);
      return;
    }
    if (call.expires) {
      routeOwner._routes.delete(call.registrationId);
      routeOwner._routeRegistrations?.delete(call.registrationId);
    }
    const intercepted = new Route(routeOwner, page ?? context, call);
    let finish;
    const token = {
      done: new Promise(resolve => { finish = resolve; }),
      ignoreErrors: false,
    };
    routeOwner._activeRouteHandlers ??= new Set();
    routeOwner._activeRouteHandlers.add(token);
    try {
      await handler(intercepted, intercepted.request());
    } catch (error) {
      if (!intercepted._handled) {
        try { await intercepted.abort('failed'); } catch {}
      }
      if (token.ignoreErrors) return;
      throw error;
    } finally {
      routeOwner._activeRouteHandlers.delete(token);
      finish();
    }
  }

  function watch(session, browserRef) {
    if (typeof session.nextPageRouteCall !== 'function') return;
    const next = () => {
      Promise.resolve().then(() => session.nextPageRouteCall()).then(call => {
        if (!call) return;
        const browser = browserRef.deref();
        if (!browser) {
          Promise.resolve(session.pageRouteFallback(call.callId)).catch(() => {});
          return;
        }
        dispatch(browser, call).then(next, error => browser._didWatchFail(error));
      }, error => browserRef.deref()?._didWatchFail(error));
    };
    next();
  }

  return { Request, Route, dispatch, route, routeFromHAR, unroute, unrouteAll, watch };
};
