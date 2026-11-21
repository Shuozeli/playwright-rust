//! Thin Node conversion port for Context-owned HAR recording.

use std::sync::Arc;

use crate::{NativeSession, binding_error, boundary};
use napi::{Env, JsString, Unknown, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use playwright_rust_node_host::{HarUpdateContent, HarUpdateMode, HostError, ffi_budget::FfiLane};

fn parse_content(value: JsString<'_>) -> napi::Result<HarUpdateContent> {
    let value = boundary::bounded_string(value, 16, "HAR content policy")?;
    parse_content_value(&value)
}

fn parse_content_value(value: &str) -> napi::Result<HarUpdateContent> {
    match value {
        "attach" => Ok(HarUpdateContent::Attach),
        "embed" => Ok(HarUpdateContent::Embed),
        "omit" => Ok(HarUpdateContent::Omit),
        value => Err(binding_error(HostError::InvalidArgument(format!(
            "unsupported HAR content policy {value:?}"
        )))),
    }
}

fn parse_mode(value: JsString<'_>) -> napi::Result<HarUpdateMode> {
    let value = boundary::bounded_string(value, 16, "HAR mode")?;
    parse_mode_value(&value)
}

fn parse_mode_value(value: &str) -> napi::Result<HarUpdateMode> {
    match value {
        "full" => Ok(HarUpdateMode::Full),
        "minimal" => Ok(HarUpdateMode::Minimal),
        value => Err(binding_error(HostError::InvalidArgument(format!(
            "unsupported HAR mode {value:?}"
        )))),
    }
}

#[cfg(feature = "har-recording-regexp")]
#[napi(object)]
pub struct NativeHarRecordingPolicy {
    pub content: String,
    pub mode: String,
}

#[napi]
impl NativeSession {
    #[napi]
    pub fn page_record_har<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        path: JsString<'env>,
        pattern: Option<JsString<'env>>,
        content: JsString<'env>,
        mode: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let path_bytes = u64::try_from(path.utf8_len()?).map_err(|_| {
            binding_error(HostError::ResourceExhausted(
                "HAR path byte length overflow".into(),
            ))
        })?;
        let pattern_bytes = pattern
            .as_ref()
            .map(JsString::utf8_len)
            .transpose()?
            .map(u64::try_from)
            .transpose()
            .map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "HAR URL filter byte length overflow".into(),
                ))
            })?
            .unwrap_or(0);
        let allowance = boundary::SMALL_ALLOWANCE
            .checked_add(path_bytes)
            .and_then(|value| value.checked_add(pattern_bytes))
            .ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "HAR FFI input byte allowance overflow".into(),
                ))
            })?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let path = path.into_utf8()?.into_owned()?;
        let pattern = pattern
            .map(|value| value.into_utf8()?.into_owned())
            .transpose()?;
        let content = parse_content(content)?;
        let mode = parse_mode(mode)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_record_har(&page_id, &path, pattern, content, mode)
                .await
                .map(|registration| registration.to_string())
        })
    }

    #[napi]
    pub fn context_record_har<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        path: JsString<'env>,
        pattern: Option<JsString<'env>>,
        content: JsString<'env>,
        mode: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let path_bytes = u64::try_from(path.utf8_len()?).map_err(|_| {
            binding_error(HostError::ResourceExhausted(
                "HAR path byte length overflow".into(),
            ))
        })?;
        let pattern_bytes = pattern
            .as_ref()
            .map(JsString::utf8_len)
            .transpose()?
            .map(u64::try_from)
            .transpose()
            .map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "HAR URL filter byte length overflow".into(),
                ))
            })?
            .unwrap_or(0);
        let allowance = boundary::SMALL_ALLOWANCE
            .checked_add(path_bytes)
            .and_then(|value| value.checked_add(pattern_bytes))
            .ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "HAR FFI input byte allowance overflow".into(),
                ))
            })?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, allowance)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let path = path.into_utf8()?.into_owned()?;
        let pattern = pattern
            .map(|value| value.into_utf8()?.into_owned())
            .transpose()?;
        let content = parse_content(content)?;
        let mode = parse_mode(mode)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_record_har(&context_id, &path, pattern, content, mode)
                .await
                .map(|registration| registration.to_string())
        })
    }
}

#[cfg(feature = "har-recording-regexp")]
#[napi]
impl NativeSession {
    #[napi(js_name = "pageRecordHarRegExp")]
    pub fn page_record_har_reg_exp<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        path: JsString<'env>,
        source: JsString<'env>,
        flags: JsString<'env>,
        policy: NativeHarRecordingPolicy,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let input_bytes = [path.utf8_len()?, source.utf8_len()?, flags.utf8_len()?]
            .into_iter()
            .try_fold(boundary::SMALL_ALLOWANCE, |total, bytes| {
                let bytes = u64::try_from(bytes).map_err(|_| {
                    binding_error(HostError::ResourceExhausted(
                        "HAR RegExp input byte length overflow".into(),
                    ))
                })?;
                total.checked_add(bytes).ok_or_else(|| {
                    binding_error(HostError::ResourceExhausted(
                        "HAR RegExp FFI input byte allowance overflow".into(),
                    ))
                })
            })?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, input_bytes)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "page ID")?;
        let path = path.into_utf8()?.into_owned()?;
        let source = source.into_utf8()?.into_owned()?;
        let flags = flags.into_utf8()?.into_owned()?;
        let content = parse_content_value(&policy.content)?;
        let mode = parse_mode_value(&policy.mode)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_record_har_regexp(&page_id, &path, source, flags, content, mode)
                .await
                .map(|registration| registration.to_string())
        })
    }

    #[napi(js_name = "contextRecordHarRegExp")]
    pub fn context_record_har_reg_exp<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        path: JsString<'env>,
        source: JsString<'env>,
        flags: JsString<'env>,
        policy: NativeHarRecordingPolicy,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let input_bytes = [path.utf8_len()?, source.utf8_len()?, flags.utf8_len()?]
            .into_iter()
            .try_fold(boundary::SMALL_ALLOWANCE, |total, bytes| {
                let bytes = u64::try_from(bytes).map_err(|_| {
                    binding_error(HostError::ResourceExhausted(
                        "HAR RegExp input byte length overflow".into(),
                    ))
                })?;
                total.checked_add(bytes).ok_or_else(|| {
                    binding_error(HostError::ResourceExhausted(
                        "HAR RegExp FFI input byte allowance overflow".into(),
                    ))
                })
            })?;
        let execution = self
            .ffi
            .admit(FfiLane::Command, input_bytes)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "context ID")?;
        let path = path.into_utf8()?.into_owned()?;
        let source = source.into_utf8()?.into_owned()?;
        let flags = flags.into_utf8()?.into_owned()?;
        let content = parse_content_value(&policy.content)?;
        let mode = parse_mode_value(&policy.mode)?;
        let host = Arc::clone(&self.host);
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_record_har_regexp(&context_id, &path, source, flags, content, mode)
                .await
                .map(|registration| registration.to_string())
        })
    }
}
