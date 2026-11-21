'use strict';
const { types: { isRegExp } } = require('node:util');

function isString(value) {
  return typeof value === 'string' || value instanceof String;
}

function matcher(value, field) {
  if (isString(value)) return { [field]: value };
  if (isRegExp(value)) return {
    [`${field}RegexSource`]: value.source,
    [`${field}RegexFlags`]: value.flags,
  };
  return {};
}

module.exports = ({ mapError, UnsupportedOperationError }) => Object.freeze({
  async cookies(context, urls) {
    if (!urls) urls = [];
    if (urls && typeof urls === 'string') urls = [urls];
    const session = context.browser()._session;
    if (typeof session.contextCookies !== 'function')
      throw new UnsupportedOperationError('browserContext.cookies (select context-cookies native profile)');
    try { return await session.contextCookies(context._id, urls); }
    catch (error) { throw mapError(error, 'browserContext.cookies'); }
  },
  async add(context, cookies) {
    const session = context.browser()._session;
    if (typeof session.contextAddCookies !== 'function')
      throw new UnsupportedOperationError('browserContext.addCookies (select context-cookies native profile)');
    try { await session.contextAddCookies(context._id, cookies); }
    catch (error) { throw mapError(error, 'browserContext.addCookies'); }
  },
  async clear(context, options = {}) {
    const session = context.browser()._session;
    if (typeof session.contextClearCookies !== 'function')
      throw new UnsupportedOperationError('browserContext.clearCookies (select context-cookies native profile)');
    const filter = {
      ...matcher(options.name, 'name'),
      ...matcher(options.domain, 'domain'),
      ...matcher(options.path, 'path'),
    };
    try { await session.contextClearCookies(context._id, filter); }
    catch (error) { throw mapError(error, 'browserContext.clearCookies'); }
  },
});
