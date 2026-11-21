'use strict';

// Public option validation and capability selection only. Rust owns policy
// retention, Context creation, per-session installation and teardown.
function normalizeGeolocation(value, api, allowNull = false) {
  if (value === null && allowNull) return undefined;
  if (value === null || typeof value !== 'object' || Array.isArray(value))
    throw new TypeError(`${api}: geolocation must be an object`);
  const { longitude, latitude, accuracy = 0 } = value;
  if (typeof longitude !== 'number')
    throw new TypeError(`${api}: geolocation.longitude must be a number`);
  if (typeof latitude !== 'number')
    throw new TypeError(`${api}: geolocation.latitude must be a number`);
  if (typeof accuracy !== 'number')
    throw new TypeError(`${api}: geolocation.accuracy must be a number`);
  return { longitude, latitude, accuracy };
}

function normalizePermissions(value, api) {
  if (!Array.isArray(value))
    throw new TypeError(`${api}: permissions must be an array`);
  return value.map((permission, index) => {
    if (typeof permission !== 'string')
      throw new TypeError(`${api}: permissions[${index}] must be a string`);
    return permission;
  });
}

function normalizeHttpCredentials(value, api) {
  if (value === null || typeof value !== 'object' || Array.isArray(value))
    throw new TypeError(`${api}: httpCredentials must be an object`);
  const credentials = { ...value };
  for (const key of Object.keys(credentials)) {
    if (!['username', 'password', 'origin', 'send'].includes(key))
      throw new TypeError(`${api}: httpCredentials.${key} is not supported`);
  }
  if (typeof credentials.username !== 'string' || typeof credentials.password !== 'string')
    throw new TypeError(`${api}: httpCredentials username and password must be strings`);
  if (credentials.origin !== undefined && typeof credentials.origin !== 'string')
    throw new TypeError(`${api}: httpCredentials.origin must be a string`);
  if (credentials.send !== undefined && !['unauthorized', 'always'].includes(credentials.send))
    throw new TypeError(`${api}: httpCredentials.send must be "unauthorized" or "always"`);
  return {
    username: credentials.username,
    password: credentials.password,
    origin: credentials.origin,
    sendAlways: credentials.send === 'always',
  };
}

function normalizeRuntimeHttpCredentials(value, api) {
  // Pinned Playwright forwards `httpCredentials || undefined`, so omitted and
  // other falsy JavaScript values all clear the deprecated runtime setting.
  if (!value) return undefined;
  if (typeof value !== 'object' || Array.isArray(value))
    throw new TypeError(`${api}: httpCredentials must be an object or null`);
  if (typeof value.username !== 'string' || typeof value.password !== 'string')
    throw new TypeError(`${api}: httpCredentials username and password must be strings`);
  if (value.origin !== undefined && typeof value.origin !== 'string')
    throw new TypeError(`${api}: httpCredentials.origin must be a string`);
  return {
    username: value.username,
    password: value.password,
    ...(value.origin === undefined ? {} : { origin: value.origin }),
  };
}

module.exports = ({ UnsupportedOperationError, contextDevice, featureSelected = () => true }) => ({
  normalizeGeolocation,
  normalizePermissions,
  normalizeHttpCredentials,
  normalizeRuntimeHttpCredentials,

  validate(options) {
    if (options.bypassCSP !== undefined && typeof options.bypassCSP !== 'boolean')
      throw new TypeError('browser.newContext: bypassCSP must be a boolean');
    if (options.offline !== undefined && typeof options.offline !== 'boolean')
      throw new TypeError('browser.newContext: offline must be a boolean');
    if (options.ignoreHTTPSErrors !== undefined && typeof options.ignoreHTTPSErrors !== 'boolean')
      throw new TypeError('browser.newContext: ignoreHTTPSErrors must be a boolean');
    if (options.locale !== undefined && typeof options.locale !== 'string')
      throw new TypeError('browser.newContext: locale must be a string');
    if (options.timezoneId !== undefined && typeof options.timezoneId !== 'string')
      throw new TypeError('browser.newContext: timezoneId must be a string');
    if (options.geolocation !== undefined &&
        (options.geolocation === null || typeof options.geolocation !== 'object' || Array.isArray(options.geolocation)))
      throw new TypeError('browser.newContext: geolocation must be an object');
    if (options.permissions !== undefined && !Array.isArray(options.permissions))
      throw new TypeError('browser.newContext: permissions must be an array');
    if (options.httpCredentials !== undefined)
      normalizeHttpCredentials(options.httpCredentials, 'browser.newContext');
  },

  create(session, options, baseURL, headerChannelError) {
    const locale = options.locale !== undefined && options.locale !== '';
    const timezone = options.timezoneId !== undefined && options.timezoneId !== '';
    const geolocation = options.geolocation !== undefined;
    const permissions = options.permissions === undefined
      ? undefined
      : normalizePermissions(options.permissions, 'browser.newContext');
    const permissionPolicy = permissions !== undefined && permissions.length !== 0;
    const httpCredentials = options.httpCredentials === undefined
      ? undefined
      : normalizeHttpCredentials(options.httpCredentials, 'browser.newContext');
    const device = contextDevice.selected(options);
    const headers = options.extraHTTPHeaders !== undefined;
    const proxy = options.proxy !== undefined;
    const selections = [options.bypassCSP, options.offline, options.ignoreHTTPSErrors,
      locale, timezone, geolocation, permissionPolicy, httpCredentials, device, headers, proxy].filter(Boolean).length;
    if (locale && !featureSelected('context-locale'))
      throw new UnsupportedOperationError('browser.newContext locale (select context-locale native profile)');
    if (timezone && !featureSelected('context-timezone'))
      throw new UnsupportedOperationError('browser.newContext timezoneId (select context-timezone native profile)');
    if (geolocation && !featureSelected('context-geolocation'))
      throw new UnsupportedOperationError('browser.newContext geolocation (select context-geolocation native profile)');
    if (permissionPolicy && !featureSelected('context-geolocation-permission'))
      throw new UnsupportedOperationError('browser.newContext permissions (select context-permissions native profile)');
    if (httpCredentials && !featureSelected('context-http-credentials'))
      throw new UnsupportedOperationError('browser.newContext httpCredentials (select context-http-credentials native profile)');
    if (device && !featureSelected('device-emulation'))
      throw new UnsupportedOperationError('browser.newContext device emulation (select device-emulation native profile)');
    if (options.offline && !featureSelected('context-offline'))
      throw new UnsupportedOperationError('browser.newContext offline (select context-offline native profile)');
    if (options.ignoreHTTPSErrors && !featureSelected('ignore-https-errors'))
      throw new UnsupportedOperationError('browser.newContext ignoreHTTPSErrors (select ignore-https-errors native profile)');
    if (options.bypassCSP && !featureSelected('bypass-csp'))
      throw new UnsupportedOperationError('browser.newContext bypassCSP (select bypass-csp native profile)');
    if (headers && !featureSelected('extra-headers'))
      throw new UnsupportedOperationError('browser.newContext extraHTTPHeaders (select extra-headers native profile)');
    if (proxy && !featureSelected('context-proxy'))
      throw new UnsupportedOperationError('browser.newContext proxy (select context-proxy native profile)');
    if (selections > 1) {
      if (locale && !session.newContextWithLocale)
        throw new UnsupportedOperationError('browser.newContext locale (select context-locale native profile)');
      if (timezone && !session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext timezoneId (select context-timezone native profile)');
      if (geolocation && !session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext geolocation (select context-geolocation native profile)');
      if (permissionPolicy && !session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext permissions (select context-permissions native profile)');
      if (httpCredentials && !session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext httpCredentials (select context-http-credentials native profile)');
      if (device && !session.newContextWithDeviceOptions)
        throw new UnsupportedOperationError('browser.newContext device emulation (select device-emulation native profile)');
      if (options.offline && !session.newContextWithOffline)
        throw new UnsupportedOperationError('browser.newContext offline (select context-offline native profile)');
      if (options.ignoreHTTPSErrors && !session.newContextWithIgnoreHttpsErrors)
        throw new UnsupportedOperationError('browser.newContext ignoreHTTPSErrors (select ignore-https-errors native profile)');
      if (options.bypassCSP && !session.newContextWithBypassCsp)
        throw new UnsupportedOperationError('browser.newContext bypassCSP (select bypass-csp native profile)');
      if (headers && !session.newContextWithExtraHttpHeaders)
        throw new UnsupportedOperationError('browser.newContext extraHTTPHeaders (select extra-headers native profile)');
      if (proxy && !session.newContextWithProxy)
        throw new UnsupportedOperationError('browser.newContext proxy (select context-proxy native profile)');
      const nativeOptions = {};
      if (locale) nativeOptions.locale = options.locale;
      if (timezone) nativeOptions.timezoneId = options.timezoneId;
      if (geolocation)
        nativeOptions.geolocation = normalizeGeolocation(options.geolocation, 'browser.newContext');
      if (permissionPolicy) nativeOptions.permissions = permissions;
      if (httpCredentials) nativeOptions.httpCredentials = httpCredentials;
      if (device) nativeOptions.device = contextDevice.nativeOptions(options);
      if (options.offline) nativeOptions.offline = true;
      if (options.ignoreHTTPSErrors) nativeOptions.ignoreHttpsErrors = true;
      if (options.bypassCSP) nativeOptions.bypassCsp = true;
      if (proxy) nativeOptions.proxy = options.proxy;
      if (headers) {
        if (!session.newContextWithPoliciesAndExtraHttpHeaders)
          throw new UnsupportedOperationError('browser.newContext composed policies with extraHTTPHeaders (rebuild native profile)');
        return session.newContextWithPoliciesAndExtraHttpHeaders(
          options.extraHTTPHeaders,
          headerChannelError,
          baseURL,
          nativeOptions,
        );
      }
      if (!session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext composed policies (rebuild native profile)');
      return session.newContextWithPolicies(baseURL, nativeOptions);
    }
    if (locale) {
      if (!session.newContextWithLocale)
        throw new UnsupportedOperationError('browser.newContext locale (select context-locale native profile)');
      return session.newContextWithLocale(baseURL, options.locale);
    }
    if (timezone) {
      if (!session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext timezoneId (select context-timezone native profile)');
      return session.newContextWithPolicies(baseURL, { timezoneId: options.timezoneId });
    }
    if (geolocation) {
      if (!session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext geolocation (select context-geolocation native profile)');
      return session.newContextWithPolicies(baseURL, {
        geolocation: normalizeGeolocation(options.geolocation, 'browser.newContext'),
      });
    }
    if (permissionPolicy) {
      if (!session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext permissions (select context-permissions native profile)');
      return session.newContextWithPolicies(baseURL, { permissions });
    }
    if (httpCredentials) {
      if (!session.newContextWithPolicies)
        throw new UnsupportedOperationError('browser.newContext httpCredentials (select context-http-credentials native profile)');
      return session.newContextWithPolicies(baseURL, { httpCredentials });
    }
    if (options.bypassCSP) {
      if (!session.newContextWithBypassCsp)
        throw new UnsupportedOperationError('browser.newContext bypassCSP (select bypass-csp native profile)');
      return session.newContextWithBypassCsp(baseURL, true);
    }
    if (options.offline) {
      if (!session.newContextWithOffline)
        throw new UnsupportedOperationError('browser.newContext offline (select context-offline native profile)');
      return session.newContextWithOffline(baseURL, true);
    }
    if (options.ignoreHTTPSErrors) {
      if (!session.newContextWithIgnoreHttpsErrors)
        throw new UnsupportedOperationError('browser.newContext ignoreHTTPSErrors (select ignore-https-errors native profile)');
      return session.newContextWithIgnoreHttpsErrors(baseURL, true);
    }
    return undefined;
  },
});
