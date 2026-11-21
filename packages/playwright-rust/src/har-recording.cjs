'use strict';

module.exports = ({ UnsupportedOperationError }) => {
  function prepare(recordHar) {
    if (recordHar === undefined) return undefined;
    if (recordHar === null || typeof recordHar !== 'object' || Array.isArray(recordHar))
      throw new TypeError('browser.newContext: recordHar must be an object');

    recordHar = { ...recordHar };
    const allowed = new Set(['path', 'omitContent', 'content', 'mode', 'urlFilter']);
    for (const key of Object.keys(recordHar)) {
      if (!allowed.has(key)) throw new TypeError(`browser.newContext: recordHar.${key} is not supported`);
    }
    if (typeof recordHar.path !== 'string')
      throw new TypeError('browser.newContext: recordHar.path must be a string');
    if (recordHar.omitContent !== undefined && typeof recordHar.omitContent !== 'boolean')
      throw new TypeError('browser.newContext: recordHar.omitContent must be a boolean');
    if (recordHar.content !== undefined && !['attach', 'embed', 'omit'].includes(recordHar.content))
      throw new TypeError("browser.newContext: recordHar.content must be 'attach', 'embed', or 'omit'");
    if (recordHar.mode !== undefined && !['full', 'minimal'].includes(recordHar.mode))
      throw new TypeError("browser.newContext: recordHar.mode must be 'full' or 'minimal'");
    if (recordHar.urlFilter !== undefined && typeof recordHar.urlFilter !== 'string')
      if (!(recordHar.urlFilter instanceof RegExp))
        throw new TypeError('browser.newContext: recordHar.urlFilter must be a string or RegExp');

    const defaultContent = recordHar.path.endsWith('.zip') ? 'attach' : 'embed';
    const regex = recordHar.urlFilter instanceof RegExp ? recordHar.urlFilter : undefined;
    return Object.freeze({
      path: recordHar.path,
      pattern: regex?.source ?? recordHar.urlFilter,
      ...(regex === undefined ? {} : { regexFlags: regex.flags }),
      content: recordHar.content ?? (recordHar.omitContent ? 'omit' : defaultContent),
      mode: recordHar.mode ?? 'full',
    });
  }

  async function initialize(session, contextId, plan) {
    if (plan === undefined) return;
    if (plan.regexFlags === undefined) {
      if (typeof session.contextRecordHar !== 'function')
        throw new UnsupportedOperationError('browser.newContext recordHar (select har-recording native profile)');
      await session.contextRecordHar(contextId, plan.path, plan.pattern, plan.content, plan.mode);
    } else {
      if (typeof session.contextRecordHarRegExp !== 'function')
        throw new UnsupportedOperationError(
          'browser.newContext recordHar.urlFilter RegExp (select har-recording-regexp native profile)',
        );
      await session.contextRecordHarRegExp(
        contextId,
        plan.path,
        plan.pattern,
        plan.regexFlags,
        { content: plan.content, mode: plan.mode },
      );
    }
  }

  return { prepare, initialize };
};
