'use strict';

// Chromium implements null viewport by leaving device metrics unmodified.
// The ordinary Rust Context path already has that behavior, so this adapter
// owns only the Playwright-compatible option conflicts.
module.exports = () => ({
  validate(options) {
    const api = 'browser.newContext';
    if (options.noViewport !== undefined && typeof options.noViewport !== 'boolean')
      throw new TypeError(`${api}: noViewport must be a boolean`);
    if (options.noViewport && options.deviceScaleFactor !== undefined)
      throw new Error(`${api}: "deviceScaleFactor" option is not supported with null "viewport"`);
    if (options.noViewport && options.isMobile === true)
      throw new Error(`${api}: "isMobile" option is not supported with null "viewport"`);
  },
});
