'use strict';

// Public option validation only. Chromium protocol dispatch, stream ownership,
// decoding, optional file persistence, and cleanup remain in Rust.
module.exports = function pagePdfAdapter({ mapError, validateOptions, UnsupportedOperationError }) {
  return async function pdf(session, pageId, options = {}) {
    const api = 'page.pdf';
    validateOptions(options, [
      'displayHeaderFooter', 'footerTemplate', 'format', 'headerTemplate',
      'height', 'landscape', 'margin', 'outline', 'pageRanges', 'path',
      'preferCSSPageSize', 'printBackground', 'scale', 'tagged', 'width',
    ], api);
    if (options.path !== undefined && typeof options.path !== 'string')
      throw new TypeError(`${api}: path must be a string`);
    for (const name of [
      'displayHeaderFooter', 'landscape', 'outline', 'preferCSSPageSize',
      'printBackground', 'tagged',
    ]) {
      if (options[name] !== undefined && typeof options[name] !== 'boolean')
        throw new TypeError(`${api}: ${name} must be a boolean`);
    }
    for (const name of ['footerTemplate', 'format', 'headerTemplate', 'pageRanges']) {
      if (options[name] !== undefined && typeof options[name] !== 'string')
        throw new TypeError(`${api}: ${name} must be a string`);
    }
    if (options.scale !== undefined && (typeof options.scale !== 'number' || !Number.isFinite(options.scale)))
      throw new TypeError(`${api}: scale must be a finite number`);

    const dimension = (value, name) => {
      if (value === undefined) return undefined;
      if (typeof value === 'number' && Number.isFinite(value)) return `${value}px`;
      if (typeof value === 'string') return value;
      throw new TypeError(`${api}: ${name} must be a string or finite number`);
    };
    let margin;
    if (options.margin !== undefined) {
      validateOptions(options.margin, ['top', 'right', 'bottom', 'left'], `${api} margin`);
      margin = {
        top: dimension(options.margin.top, 'margin.top'),
        right: dimension(options.margin.right, 'margin.right'),
        bottom: dimension(options.margin.bottom, 'margin.bottom'),
        left: dimension(options.margin.left, 'margin.left'),
      };
    }
    if (typeof session.pagePdf !== 'function')
      throw new UnsupportedOperationError(`${api} (select page-pdf native profile)`);
    try {
      return await session.pagePdf(pageId, {
        path: options.path || undefined,
        scale: options.scale ?? 1,
        displayHeaderFooter: options.displayHeaderFooter ?? false,
        headerTemplate: options.headerTemplate ?? '',
        footerTemplate: options.footerTemplate ?? '',
        printBackground: options.printBackground ?? false,
        landscape: options.landscape ?? false,
        pageRanges: options.pageRanges ?? '',
        format: options.format,
        width: dimension(options.width, 'width'),
        height: dimension(options.height, 'height'),
        preferCssPageSize: options.preferCSSPageSize ?? false,
        margin,
        tagged: options.tagged ?? false,
        outline: options.outline ?? false,
      });
    } catch (error) {
      throw mapError(error, api);
    }
  };
};
