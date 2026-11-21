//! N-API conversion boundary for the optional BrowserContext tracing leaf.

use std::path::PathBuf;

use napi::{Env, JsString, Unknown, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use playwright_rust_node_host::ffi_budget::FfiLane;

use crate::{NativeSession, binding_error, boundary};

fn input_allowance(lengths: &[usize]) -> napi::Result<u64> {
    lengths
        .iter()
        .try_fold(boundary::SMALL_ALLOWANCE, |total, length| {
            let length = u64::try_from(*length).map_err(|_| {
                binding_error(playwright_rust_node_host::HostError::ResourceExhausted(
                    "Tracing input byte length overflow".into(),
                ))
            })?;
            total.checked_add(length).ok_or_else(|| {
                binding_error(playwright_rust_node_host::HostError::ResourceExhausted(
                    "Tracing FFI input allowance overflow".into(),
                ))
            })
        })
}

#[napi]
impl NativeSession {
    #[napi]
    pub fn context_tracing_start(
        &self,
        context_id: JsString<'_>,
        screenshots: bool,
        snapshots: bool,
        sources: bool,
        name: Option<JsString<'_>>,
        title: Option<JsString<'_>>,
    ) -> napi::Result<()> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let name = name
            .map(|name| name.into_utf8()?.into_owned())
            .transpose()?;
        let title = title
            .map(|title| title.into_utf8()?.into_owned())
            .transpose()?;
        self.host
            .context_tracing_start(&context_id, screenshots, snapshots, sources, name, title)
            .map_err(binding_error)
    }

    #[napi]
    pub fn context_tracing_stop<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        path: Option<JsString<'env>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let path_length = path
            .as_ref()
            .map(JsString::utf8_len)
            .transpose()?
            .unwrap_or(0);
        let execution = self
            .ffi
            .admit(FfiLane::Command, input_allowance(&[path_length])?)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let path = path
            .map(|path| path.into_utf8()?.into_owned())
            .transpose()?
            .map(PathBuf::from);
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_tracing_stop(&context_id, path).await
        })
    }

    #[cfg(feature = "context-tracing-chunks")]
    #[napi]
    pub fn context_tracing_start_chunk(
        &self,
        context_id: JsString<'_>,
        name: Option<JsString<'_>>,
        title: Option<JsString<'_>>,
    ) -> napi::Result<()> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let name = name
            .map(|name| name.into_utf8()?.into_owned())
            .transpose()?;
        let title = title
            .map(|title| title.into_utf8()?.into_owned())
            .transpose()?;
        self.host
            .context_tracing_start_chunk(&context_id, name, title)
            .map_err(binding_error)
    }

    #[cfg(feature = "context-tracing-chunks")]
    #[napi]
    pub fn context_tracing_stop_chunk<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        path: Option<JsString<'env>>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let path_length = path
            .as_ref()
            .map(JsString::utf8_len)
            .transpose()?
            .unwrap_or(0);
        let execution = self
            .ffi
            .admit(FfiLane::Command, input_allowance(&[path_length])?)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let path = path
            .map(|path| path.into_utf8()?.into_owned())
            .transpose()?
            .map(PathBuf::from);
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_tracing_stop_chunk(&context_id, path).await
        })
    }

    #[cfg(feature = "context-tracing-chunks")]
    #[napi]
    pub fn context_tracing_group(
        &self,
        context_id: JsString<'_>,
        title: JsString<'_>,
    ) -> napi::Result<()> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let title = title.into_utf8()?.into_owned()?;
        self.host
            .context_tracing_group(&context_id, title)
            .map_err(binding_error)
    }

    #[cfg(feature = "context-tracing-chunks")]
    #[napi]
    pub fn context_tracing_group_end(&self, context_id: JsString<'_>) -> napi::Result<()> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        self.host
            .context_tracing_group_end(&context_id)
            .map_err(binding_error)
    }

    #[cfg(feature = "context-tracing-actions")]
    #[napi]
    pub fn context_tracing_action_begin(
        &self,
        context_id: JsString<'_>,
        page_id: Option<JsString<'_>>,
        action: JsString<'_>,
        detail: Option<JsString<'_>>,
        source_file: Option<JsString<'_>>,
        stack_json: Option<JsString<'_>>,
    ) -> napi::Result<Option<String>> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let page_id = page_id
            .map(|page_id| boundary::bounded_string(page_id, 20, "Page ID"))
            .transpose()?;
        let action = boundary::bounded_string(action, 32, "trace action")?;
        let detail = detail
            .map(|detail| detail.into_utf8()?.into_owned())
            .transpose()?;
        let source_file = source_file
            .map(|source_file| source_file.into_utf8()?.into_owned())
            .transpose()?;
        let stack_json = stack_json
            .map(|stack_json| stack_json.into_utf8()?.into_owned())
            .transpose()?;
        self.host
            .context_tracing_action_begin(
                &context_id,
                page_id.as_deref(),
                &action,
                detail.as_deref(),
                source_file.as_deref(),
                stack_json.as_deref(),
            )
            .map_err(binding_error)
    }

    #[cfg(feature = "context-tracing-actions")]
    #[napi]
    pub fn context_tracing_action_end(
        &self,
        context_id: JsString<'_>,
        call_id: JsString<'_>,
        result_json: Option<JsString<'_>>,
    ) -> napi::Result<()> {
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let call_id = boundary::bounded_string(call_id, 64, "trace call ID")?;
        let result_json = result_json
            .map(|result| result.into_utf8()?.into_owned())
            .transpose()?;
        self.host
            .context_tracing_action_end(&context_id, &call_id, result_json.as_deref())
            .map_err(binding_error)
    }

    #[cfg(feature = "context-tracing-snapshots")]
    #[napi]
    pub fn context_tracing_capture_snapshot<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        page_id: JsString<'env>,
        call_id: JsString<'env>,
        phase: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, boundary::READ_ALLOWANCE)
            .map_err(binding_error)?;
        let context_id = boundary::bounded_string(context_id, 20, "BrowserContext ID")?;
        let page_id = boundary::bounded_string(page_id, 20, "Page ID")?;
        let call_id = boundary::bounded_string(call_id, 64, "trace call ID")?;
        let phase = boundary::bounded_string(phase, 6, "trace snapshot phase")?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.context_tracing_capture_snapshot(&context_id, &page_id, &call_id, &phase)
                .await
        })
    }
}
