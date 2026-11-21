//! Node conversion ports for the optional Rust-owned Video composition.

use std::path::PathBuf;

use napi::{Env, JsString, Unknown, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use playwright_rust_node_host::{HostError, PersistentContextVideoOptions, ffi_budget::FfiLane};

use crate::{BrowserPageDescriptor, NativeSession, binding_error, boundary};

#[napi]
pub fn video_recording_enabled() -> bool {
    true
}

fn input_allowance(lengths: &[usize]) -> napi::Result<u64> {
    lengths
        .iter()
        .try_fold(boundary::SMALL_ALLOWANCE, |total, length| {
            let length = u64::try_from(*length).map_err(|_| {
                binding_error(HostError::ResourceExhausted(
                    "Video input byte length overflow".into(),
                ))
            })?;
            total.checked_add(length).ok_or_else(|| {
                binding_error(HostError::ResourceExhausted(
                    "Video FFI input allowance overflow".into(),
                ))
            })
        })
}

fn options(
    directory: String,
    ffmpeg: String,
    width: u32,
    height: u32,
) -> napi::Result<PersistentContextVideoOptions> {
    PersistentContextVideoOptions::new(
        PathBuf::from(directory),
        PathBuf::from(ffmpeg),
        width,
        height,
    )
    .map_err(binding_error)
}

fn utf8_path(path: PathBuf, kind: &str) -> Result<String, HostError> {
    path.into_os_string()
        .into_string()
        .map_err(|_| HostError::InvalidArgument(format!("{kind} is not valid UTF-8")))
}

#[napi]
impl NativeSession {
    #[napi]
    pub fn install_context_video<'env>(
        &self,
        env: &'env Env,
        context_id: JsString<'env>,
        directory: JsString<'env>,
        ffmpeg: JsString<'env>,
        width: u32,
        height: u32,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(
                FfiLane::Command,
                input_allowance(&[
                    context_id.utf8_len()?,
                    directory.utf8_len()?,
                    ffmpeg.utf8_len()?,
                ])?,
            )
            .map_err(binding_error)?;
        let context_id = context_id.into_utf8()?.into_owned()?;
        let directory = directory.into_utf8()?.into_owned()?;
        let ffmpeg = ffmpeg.into_utf8()?.into_owned()?;
        let options = options(directory, ffmpeg, width, height)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.install_context_video(&context_id, options)
        })
    }

    // N-API counts the receiver and environment handle in addition to the six
    // Playwright values carried by this direct, allocation-free boundary.
    #[allow(clippy::too_many_arguments)]
    #[napi]
    pub fn new_browser_page_with_video<'env>(
        &self,
        env: &'env Env,
        base_url: Option<JsString<'env>>,
        accept_downloads: bool,
        directory: JsString<'env>,
        ffmpeg: JsString<'env>,
        width: u32,
        height: u32,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let base_url_length = base_url
            .as_ref()
            .map(JsString::utf8_len)
            .transpose()?
            .unwrap_or(0);
        let execution = self
            .ffi
            .admit(
                FfiLane::Command,
                input_allowance(&[base_url_length, directory.utf8_len()?, ffmpeg.utf8_len()?])?,
            )
            .map_err(binding_error)?;
        let base_url = base_url
            .map(|value| value.into_utf8()?.into_owned())
            .transpose()?;
        let directory = directory.into_utf8()?.into_owned()?;
        let ffmpeg = ffmpeg.into_utf8()?.into_owned()?;
        let options = options(directory, ffmpeg, width, height)?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            BrowserPageDescriptor::try_from(
                host.new_browser_page_with_video(base_url.as_deref(), accept_downloads, options)
                    .await?,
            )
        })
    }

    #[napi]
    pub fn page_video_path<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, input_allowance(&[page_id.utf8_len()?])?)
            .map_err(binding_error)?;
        let page_id = page_id.into_utf8()?.into_owned()?;
        let host = self.host.clone();
        boundary::spawn_published_sized(
            env,
            execution,
            self.publications.clone(),
            async move { utf8_path(host.page_video_path(&page_id).await?, "Video path") },
            |path| {
                u64::try_from(path.len())
                    .expect("usize fits in u64 on every supported Node.js platform")
            },
        )
    }

    #[napi]
    pub fn page_video_save_as<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        destination: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(
                FfiLane::Command,
                input_allowance(&[page_id.utf8_len()?, destination.utf8_len()?])?,
            )
            .map_err(binding_error)?;
        let page_id = page_id.into_utf8()?.into_owned()?;
        let destination = destination.into_utf8()?.into_owned()?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_video_save_as(&page_id, &PathBuf::from(destination))
                .await
        })
    }

    #[napi]
    pub fn page_video_delete<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, input_allowance(&[page_id.utf8_len()?])?)
            .map_err(binding_error)?;
        let page_id = page_id.into_utf8()?.into_owned()?;
        let host = self.host.clone();
        boundary::spawn_published(env, execution, self.publications.clone(), async move {
            host.page_video_delete(&page_id).await
        })
    }
}
