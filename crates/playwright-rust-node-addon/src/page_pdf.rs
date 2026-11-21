//! Node-owned Buffer conversion for the optional Chromium Page PDF leaf.

use napi::{
    Env, JsString, Unknown,
    bindgen_prelude::{BufferSlice, PromiseRaw, ToNapiValue},
    sys,
};
use napi_derive::napi;
use playwright_rust_node_host::{
    HostError,
    ffi_budget::FfiLane,
    page_pdf::{self, PdfMargins, PdfOptions, PdfOutput as HostPdfOutput},
};

use crate::{NativeSession, binding_error, boundary};

struct PdfOutput(HostPdfOutput);

#[napi(object)]
pub struct NativePdfMargins {
    pub top: Option<String>,
    pub right: Option<String>,
    pub bottom: Option<String>,
    pub left: Option<String>,
}

#[napi(object)]
pub struct NativePdfOptions {
    pub path: Option<String>,
    pub scale: f64,
    pub display_header_footer: bool,
    pub header_template: String,
    pub footer_template: String,
    pub print_background: bool,
    pub landscape: bool,
    pub page_ranges: String,
    pub format: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
    pub prefer_css_page_size: bool,
    pub margin: Option<NativePdfMargins>,
    pub tagged: bool,
    pub outline: bool,
}

impl ToNapiValue for PdfOutput {
    unsafe fn to_napi_value(env: sys::napi_env, value: Self) -> napi::Result<sys::napi_value> {
        // The FFI settlement ticket covers the complete Rust-to-Node copy.
        // Node never aliases the allocator-backed Rust output.
        let scope = Env::from_raw(env);
        let buffer = BufferSlice::copy_from(&scope, value.0.as_bytes())?;
        // SAFETY: the buffer was created in this live callback's environment.
        unsafe { BufferSlice::to_napi_value(env, buffer) }
    }
}

#[napi]
impl NativeSession {
    #[napi]
    pub fn page_pdf<'env>(
        &self,
        env: &'env Env,
        page_id: JsString<'env>,
        options: NativePdfOptions,
    ) -> napi::Result<PromiseRaw<'env, Unknown<'env>>> {
        let execution = self
            .ffi
            .admit(FfiLane::Command, page_pdf::FFI_ALLOWANCE)
            .map_err(binding_error)?;
        let page_id = boundary::bounded_string(page_id, 20, "Page ID")?;
        let path = options.path.filter(|path| !path.is_empty());
        let margin = options.margin.unwrap_or(NativePdfMargins {
            top: None,
            right: None,
            bottom: None,
            left: None,
        });
        let pdf_options = PdfOptions {
            scale: options.scale,
            display_header_footer: options.display_header_footer,
            header_template: options.header_template,
            footer_template: options.footer_template,
            print_background: options.print_background,
            landscape: options.landscape,
            page_ranges: options.page_ranges,
            format: options.format,
            width: options.width,
            height: options.height,
            prefer_css_page_size: options.prefer_css_page_size,
            margins: PdfMargins {
                top: margin.top,
                right: margin.right,
                bottom: margin.bottom,
                left: margin.left,
            },
            tagged: options.tagged,
            outline: options.outline,
        };
        let host = self.host.clone();
        boundary::spawn_published_sized(
            env,
            execution,
            self.publications.clone(),
            async move {
                let output = host.page_pdf(&page_id, pdf_options).await?;
                if let Some(path) = path {
                    let path = std::path::PathBuf::from(path);
                    if let Some(parent) = path
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                    {
                        tokio::fs::create_dir_all(parent).await.map_err(|error| {
                            HostError::Backend(format!(
                                "Page.pdf could not create output directory: {error}"
                            ))
                        })?;
                    }
                    tokio::fs::write(path, output.as_bytes())
                        .await
                        .map_err(|error| {
                            HostError::Backend(format!("Page.pdf could not write output: {error}"))
                        })?;
                }
                Ok(PdfOutput(output))
            },
            |output| u64::try_from(output.0.as_bytes().len()).unwrap_or(u64::MAX),
        )
    }
}
