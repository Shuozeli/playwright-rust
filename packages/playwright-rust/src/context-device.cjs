'use strict';

// Public option validation and capability selection only. Rust owns device
// option retention, Context installation, CDP emulation and teardown.
module.exports = ({ UnsupportedOperationError }) => ({
  validate(options) {
    const api = 'browser.newContext';
    const validateSize = (value, name) => {
      if (value === undefined) return;
      if (value === null && name === 'viewport') return;
      if (!value || typeof value !== 'object' || Array.isArray(value))
        throw new TypeError(`${api}: ${name} must be an object`);
      const copy = { ...value };
      for (const key of Object.keys(copy)) {
        if (!['width', 'height'].includes(key))
          throw new TypeError(`${api}: ${name}.${key} is not supported`);
      }
      for (const key of ['width', 'height']) {
        if (!Number.isInteger(copy[key]) || copy[key] < 1 || copy[key] > 10_000_000)
          throw new TypeError(`${api}: ${name}.${key} must be an integer in [1, 10000000]`);
      }
    };
    validateSize(options.viewport, 'viewport');
    validateSize(options.screen, 'screen');
    if (options.deviceScaleFactor !== undefined
        && (typeof options.deviceScaleFactor !== 'number'
          || !Number.isFinite(options.deviceScaleFactor)
          || options.deviceScaleFactor < 0))
      throw new TypeError(`${api}: deviceScaleFactor must be a finite non-negative number`);
    if (options.isMobile !== undefined && typeof options.isMobile !== 'boolean')
      throw new TypeError(`${api}: isMobile must be a boolean`);
    if (options.hasTouch !== undefined && typeof options.hasTouch !== 'boolean')
      throw new TypeError(`${api}: hasTouch must be a boolean`);
    if (options.userAgent !== undefined && typeof options.userAgent !== 'string')
      throw new TypeError(`${api}: userAgent must be a string`);

  },

  selected(options) {
    return options.viewport !== undefined && options.viewport !== null
      || options.screen !== undefined
      || options.deviceScaleFactor !== undefined
      || options.isMobile === true
      || options.hasTouch === true
      || options.userAgent !== undefined;
  },

  nativeOptions(options) {
    return {
      userAgent: options.userAgent,
      viewportWidth: options.viewport?.width,
      viewportHeight: options.viewport?.height,
      screenWidth: options.screen?.width,
      screenHeight: options.screen?.height,
      deviceScaleFactor: options.deviceScaleFactor,
      isMobile: options.isMobile,
      hasTouch: options.hasTouch,
    };
  },

  create(session, options, baseURL) {
    const selected = options.viewport !== undefined && options.viewport !== null
      || options.screen !== undefined
      || options.deviceScaleFactor !== undefined
      || options.isMobile === true
      || options.hasTouch === true
      || options.userAgent !== undefined;
    if (!selected) return undefined;
    const nativeOptions = this.nativeOptions(options);
    if (options.ignoreHTTPSErrors) {
      if (!session.newContextWithDeviceOptionsAndIgnoreHttpsErrors)
        throw new UnsupportedOperationError('browser.newContext device emulation with ignoreHTTPSErrors (select device-emulation and ignore-https-errors native profile)');
      return session.newContextWithDeviceOptionsAndIgnoreHttpsErrors(baseURL, nativeOptions, true);
    }
    if (!session.newContextWithDeviceOptions)
      throw new UnsupportedOperationError('browser.newContext device emulation (select device-emulation native profile)');
    return session.newContextWithDeviceOptions(baseURL, nativeOptions);
  },
});
