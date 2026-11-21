//! Optional Node conversion port for Rust-owned BrowserContext cookies.
//!
//! Read borrowed JS fields only after the FFI command is admitted. This port
//! limits both the count and the UTF-8 copies before scheduling the host call;
//! cookie validation and Chromium Storage commands remain in the safe host.

use napi::{
    Env, JsString,
    bindgen_prelude::{Array, JsObjectValue, Object, PromiseRaw},
};
use napi_derive::napi;
use playwright_rust_node_host::{
    HostError,
    context_cookies::{
        BrowserContextCookie, BrowserContextCookieClearFilter, BrowserContextCookieInput,
        CookieStringMatcher, FFI_ALLOWANCE, MAX_COOKIE_BATCH_BYTES, MAX_COOKIE_FIELD_BYTES,
        MAX_COOKIE_URL_BATCH_BYTES, MAX_COOKIE_URL_BYTES, MAX_COOKIE_URLS, MAX_COOKIES,
    },
    ffi_budget::FfiLane,
};

use crate::{NativeSession, binding_error, boundary};

#[napi(object)]
pub struct NativeBrowserContextCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: String,
    pub partition_key: Option<String>,
    #[napi(js_name = "_crHasCrossSiteAncestor")]
    pub partition_has_cross_site_ancestor: Option<bool>,
}

impl From<BrowserContextCookie> for NativeBrowserContextCookie {
    fn from(value: BrowserContextCookie) -> Self {
        Self {
            name: value.name,
            value: value.value,
            domain: value.domain,
            path: value.path,
            expires: value.expires,
            http_only: value.http_only,
            secure: value.secure,
            same_site: value.same_site,
            partition_key: value.partition_key,
            partition_has_cross_site_ancestor: value.partition_has_cross_site_ancestor,
        }
    }
}

fn output_bytes(cookies: &[NativeBrowserContextCookie]) -> u64 {
    cookies.iter().fold(0_u64, |total, cookie| {
        let strings = [
            cookie.name.as_str(),
            cookie.value.as_str(),
            cookie.domain.as_str(),
            cookie.path.as_str(),
            cookie.same_site.as_str(),
            cookie.partition_key.as_deref().unwrap_or_default(),
        ];
        strings.iter().fold(total, |bytes, value| {
            bytes.saturating_add(u64::try_from(value.len()).unwrap_or(u64::MAX))
        })
    })
}

fn cookie_string(
    value: JsString<'_>,
    maximum: usize,
    field: &str,
    remaining: &mut usize,
) -> napi::Result<String> {
    let bytes = value.utf8_len()?;
    if bytes > maximum {
        return Err(binding_error(HostError::ResourceExhausted(format!(
            "{field} exceeds {maximum} UTF-8 bytes"
        ))));
    }
    *remaining = remaining.checked_sub(bytes).ok_or_else(|| {
        binding_error(HostError::ResourceExhausted(format!(
            "cookie input batch exceeds {MAX_COOKIE_BATCH_BYTES} bytes"
        )))
    })?;
    value.into_utf8()?.into_owned()
}

fn optional_cookie_string(
    value: Option<JsString<'_>>,
    maximum: usize,
    field: &str,
    remaining: &mut usize,
) -> napi::Result<Option<String>> {
    value
        .map(|value| cookie_string(value, maximum, field, remaining))
        .transpose()
}

fn read_cookie(
    cookie: Object<'_>,
    remaining: &mut usize,
) -> napi::Result<BrowserContextCookieInput> {
    Ok(BrowserContextCookieInput {
        name: cookie_string(
            cookie.get_named_property("name")?,
            MAX_COOKIE_FIELD_BYTES,
            "cookie name",
            remaining,
        )?,
        value: cookie_string(
            cookie.get_named_property("value")?,
            MAX_COOKIE_FIELD_BYTES,
            "cookie value",
            remaining,
        )?,
        url: optional_cookie_string(
            cookie.get_named_property("url")?,
            MAX_COOKIE_URL_BYTES,
            "cookie URL",
            remaining,
        )?,
        domain: optional_cookie_string(
            cookie.get_named_property("domain")?,
            MAX_COOKIE_FIELD_BYTES,
            "cookie domain",
            remaining,
        )?,
        path: optional_cookie_string(
            cookie.get_named_property("path")?,
            MAX_COOKIE_FIELD_BYTES,
            "cookie path",
            remaining,
        )?,
        expires: cookie.get_named_property("expires")?,
        http_only: cookie.get_named_property("httpOnly")?,
        secure: cookie.get_named_property("secure")?,
        same_site: optional_cookie_string(
            cookie.get_named_property("sameSite")?,
            MAX_COOKIE_FIELD_BYTES,
            "cookie sameSite",
            remaining,
        )?,
        partition_key: optional_cookie_string(
            cookie.get_named_property("partitionKey")?,
            MAX_COOKIE_URL_BYTES,
            "cookie partitionKey",
            remaining,
        )?,
        partition_has_cross_site_ancestor: cookie.get_named_property("_crHasCrossSiteAncestor")?,
    })
}

fn read_cookies(cookies: Array<'_>) -> napi::Result<Vec<BrowserContextCookieInput>> {
    if cookies.len() > MAX_COOKIES as u32 {
        return Err(binding_error(HostError::ResourceExhausted(format!(
            "cookie input count exceeds {MAX_COOKIES}"
        ))));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(cookies.len() as usize)
        .map_err(|_| {
            binding_error(HostError::ResourceExhausted(
                "cookie input allocation failed".into(),
            ))
        })?;
    let mut remaining = MAX_COOKIE_BATCH_BYTES;
    for index in 0..cookies.len() {
        let cookie = cookies
            .get::<Object<'_>>(index)?
            .ok_or_else(|| binding_error(HostError::InvalidArgument("missing cookie".into())))?;
        output.push(read_cookie(cookie, &mut remaining)?);
    }
    Ok(output)
}

fn read_urls(urls: Array<'_>) -> napi::Result<Vec<String>> {
    if urls.len() > MAX_COOKIE_URLS as u32 {
        return Err(binding_error(HostError::ResourceExhausted(format!(
            "cookie URL count exceeds {MAX_COOKIE_URLS}"
        ))));
    }
    let mut output = Vec::new();
    output.try_reserve_exact(urls.len() as usize).map_err(|_| {
        binding_error(HostError::ResourceExhausted(
            "cookie URL allocation failed".into(),
        ))
    })?;
    let mut remaining = MAX_COOKIE_URL_BATCH_BYTES;
    for index in 0..urls.len() {
        let url = urls.get::<JsString<'_>>(index)?.ok_or_else(|| {
            binding_error(HostError::InvalidArgument("missing cookie URL".into()))
        })?;
        output.push(cookie_string(
            url,
            MAX_COOKIE_URL_BYTES,
            "cookie URL",
            &mut remaining,
        )?);
    }
    Ok(output)
}

fn read_matcher(
    exact: Option<JsString<'_>>,
    regex_source: Option<JsString<'_>>,
    regex_flags: Option<JsString<'_>>,
    field: &str,
    remaining: &mut usize,
) -> napi::Result<Option<CookieStringMatcher>> {
    if exact.is_some() && (regex_source.is_some() || regex_flags.is_some()) {
        return Err(binding_error(HostError::InvalidArgument(format!(
            "cookie clear {field} cannot contain both string and RegExp matchers"
        ))));
    }
    match (regex_source, regex_flags) {
        (Some(source), Some(flags)) => {
            let source = cookie_string(
                source,
                MAX_COOKIE_FIELD_BYTES,
                "cookie clear RegExp source",
                remaining,
            )?;
            let flags = cookie_string(
                flags,
                MAX_COOKIE_FIELD_BYTES,
                "cookie clear RegExp flags",
                remaining,
            )?;
            CookieStringMatcher::regex(source, &flags)
                .map(Some)
                .map_err(binding_error)
        }
        (None, None) => exact
            .map(|value| {
                cookie_string(
                    value,
                    MAX_COOKIE_FIELD_BYTES,
                    "cookie clear string matcher",
                    remaining,
                )
                .and_then(|value| CookieStringMatcher::exact(value).map_err(binding_error))
            })
            .transpose(),
        _ => Err(binding_error(HostError::InvalidArgument(format!(
            "cookie clear {field} RegExp requires source and flags"
        )))),
    }
}

#[napi]
impl NativeSession {
    #[napi]
    pub fn context_cookies<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        urls: Array<'env>,
    ) -> napi::Result<PromiseRaw<'env, napi::Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, FFI_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let urls = read_urls(urls)?;
        let host = self.host.clone();
        boundary::spawn_published_sized(
            env,
            execution,
            self.publications.clone(),
            async move {
                host.context_cookies(&context_id, urls)
                    .await
                    .map(|cookies| {
                        cookies
                            .into_iter()
                            .map(NativeBrowserContextCookie::from)
                            .collect()
                    })
            },
            |cookies: &Vec<NativeBrowserContextCookie>| output_bytes(cookies),
        )
    }

    #[napi]
    pub fn context_add_cookies<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        cookies: Array<'env>,
    ) -> napi::Result<PromiseRaw<'env, napi::Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, FFI_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let cookies = read_cookies(cookies)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_add_cookies(&context_id, cookies).await
        })
    }

    #[napi]
    pub fn context_clear_cookies<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        filter: Object<'env>,
    ) -> napi::Result<PromiseRaw<'env, napi::Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, FFI_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let mut remaining = MAX_COOKIE_BATCH_BYTES;
        let filter = BrowserContextCookieClearFilter {
            name: read_matcher(
                filter.get_named_property("name")?,
                filter.get_named_property("nameRegexSource")?,
                filter.get_named_property("nameRegexFlags")?,
                "name",
                &mut remaining,
            )?,
            domain: read_matcher(
                filter.get_named_property("domain")?,
                filter.get_named_property("domainRegexSource")?,
                filter.get_named_property("domainRegexFlags")?,
                "domain",
                &mut remaining,
            )?,
            path: read_matcher(
                filter.get_named_property("path")?,
                filter.get_named_property("pathRegexSource")?,
                filter.get_named_property("pathRegexFlags")?,
                "path",
                &mut remaining,
            )?,
        };
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_clear_cookies(&context_id, filter).await
        })
    }
}
