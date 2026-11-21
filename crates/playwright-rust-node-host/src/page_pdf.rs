//! Optional Chromium Page PDF export.
//!
//! Page/session routing, CDP dispatch, response decoding and retained output
//! ownership stay in Rust. Language bindings may copy the allocator-backed result or
//! persist it, but they do not own browser or protocol semantics.

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use playwright_rust_cdp::CdpSession;
use playwright_rust_core::{ObjectId, ProductionResourceLedger, ResourceRequirements};
use serde::Deserialize;

use crate::{HostError, HostSession};

const PRINT_METHOD: &str = "Page.printToPDF";
const READ_METHOD: &str = "IO.read";
const CLOSE_METHOD: &str = "IO.close";
const READ_CHUNK_BYTES: usize = 64 * 1024;
const MAX_STREAM_HANDLE_BYTES: usize = 1024;
pub const FFI_ALLOWANCE: u64 = 4096;

pub(crate) const fn requirements() -> ResourceRequirements {
    ResourceRequirements::none()
}

pub(crate) struct Installation;

impl Installation {
    pub(crate) fn new(resources: &ProductionResourceLedger) -> Result<Arc<Self>, HostError> {
        resources
            .claim(requirements())
            .map_err(HostError::backend)?;
        Ok(Arc::new(Self))
    }

    fn accumulator(&self) -> StreamAccumulator {
        StreamAccumulator { bytes: Vec::new() }
    }
}

struct StreamAccumulator {
    bytes: Vec<u8>,
}

impl StreamAccumulator {
    fn push(&mut self, data: &str, base64_encoded: bool) -> Result<(), HostError> {
        if data.is_empty() {
            return Ok(());
        }
        if !base64_encoded {
            self.bytes
                .try_reserve_exact(data.len())
                .map_err(|_| HostError::ResourceExhausted("PDF output allocation failed".into()))?;
            self.bytes.extend_from_slice(data.as_bytes());
            return Ok(());
        }
        if !data.len().is_multiple_of(4) {
            return Err(HostError::Backend(
                "IO.read returned malformed base64 PDF data".into(),
            ));
        }
        let padding = data
            .as_bytes()
            .iter()
            .rev()
            .take_while(|byte| **byte == b'=')
            .count();
        if padding > 2 {
            return Err(HostError::Backend(
                "IO.read returned malformed base64 PDF padding".into(),
            ));
        }
        let decoded_bound = data
            .len()
            .checked_div(4)
            .and_then(|groups| groups.checked_mul(3))
            .and_then(|bytes| bytes.checked_sub(padding))
            .ok_or_else(|| HostError::ResourceExhausted("PDF size arithmetic overflow".into()))?;
        self.bytes
            .try_reserve_exact(decoded_bound)
            .map_err(|_| HostError::ResourceExhausted("PDF output allocation failed".into()))?;
        STANDARD
            .decode_vec(data, &mut self.bytes)
            .map_err(|_| HostError::Backend("IO.read returned invalid base64 PDF data".into()))?;
        Ok(())
    }

    fn finish(self) -> Result<PdfOutput, HostError> {
        if self.bytes.is_empty() {
            return Err(HostError::Backend(
                "Page.printToPDF returned an empty document".into(),
            ));
        }
        Ok(PdfOutput(self.bytes))
    }
}

#[derive(Debug)]
pub struct PdfOutput(Vec<u8>);

impl PdfOutput {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrintToPdfReturns {
    stream: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IoReadReturns {
    data: String,
    #[serde(default)]
    base64_encoded: bool,
    #[serde(default)]
    eof: bool,
}

struct StreamCloseGuard {
    stream: Option<(CdpSession, String)>,
}

impl StreamCloseGuard {
    fn new(session: CdpSession, handle: String) -> Self {
        Self {
            stream: Some((session, handle)),
        }
    }

    fn session(&self) -> &CdpSession {
        &self.stream.as_ref().expect("live PDF stream").0
    }

    fn handle(&self) -> &str {
        &self.stream.as_ref().expect("live PDF stream").1
    }

    async fn close(&mut self) -> Result<(), HostError> {
        let params = serde_json::json!({ "handle": self.handle() });
        let result = self
            .session()
            .send(CLOSE_METHOD, params)
            .await
            .map(|_| ())
            .map_err(HostError::backend);
        self.stream = None;
        result
    }
}

impl Drop for StreamCloseGuard {
    fn drop(&mut self) {
        let Some((session, handle)) = self.stream.take() else {
            return;
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            std::mem::drop(runtime.spawn(async move {
                let _ = session
                    .send(CLOSE_METHOD, serde_json::json!({ "handle": handle }))
                    .await;
            }));
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PdfMargins {
    pub top: Option<String>,
    pub right: Option<String>,
    pub bottom: Option<String>,
    pub left: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PdfOptions {
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
    pub margins: PdfMargins,
    pub tagged: bool,
    pub outline: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            display_header_footer: false,
            header_template: String::new(),
            footer_template: String::new(),
            print_background: false,
            landscape: false,
            page_ranges: String::new(),
            format: None,
            width: None,
            height: None,
            prefer_css_page_size: false,
            margins: PdfMargins::default(),
            tagged: false,
            outline: false,
        }
    }
}

fn paper_format(format: &str) -> Option<(f64, f64)> {
    match format.to_ascii_lowercase().as_str() {
        "letter" => Some((8.5, 11.0)),
        "legal" => Some((8.5, 14.0)),
        "tabloid" => Some((11.0, 17.0)),
        "ledger" => Some((17.0, 11.0)),
        "a0" => Some((33.1, 46.8)),
        "a1" => Some((23.4, 33.1)),
        "a2" => Some((16.54, 23.4)),
        "a3" => Some((11.7, 16.54)),
        "a4" => Some((8.27, 11.7)),
        "a5" => Some((5.83, 8.27)),
        "a6" => Some((4.13, 5.83)),
        _ => None,
    }
}

fn print_parameter_inches(text: Option<&str>) -> Result<Option<f64>, HostError> {
    let Some(text) = text else {
        return Ok(None);
    };
    let (value_text, pixels_per_unit) = text
        .len()
        .checked_sub(2)
        .and_then(|index| text.get(index..).map(|suffix| (index, suffix)))
        .and_then(
            |(index, suffix)| match suffix.to_ascii_lowercase().as_str() {
                "px" => Some((&text[..index], 1.0)),
                "in" => Some((&text[..index], 96.0)),
                "cm" => Some((&text[..index], 37.8)),
                "mm" => Some((&text[..index], 3.78)),
                _ => None,
            },
        )
        .unwrap_or((text, 1.0));
    let value_text = value_text.trim();
    let value = if value_text.is_empty() {
        0.0
    } else {
        value_text.parse::<f64>().map_err(|_| {
            HostError::InvalidArgument(format!("Failed to parse parameter value: {text}"))
        })?
    };
    if value.is_nan() {
        return Err(HostError::InvalidArgument(format!(
            "Failed to parse parameter value: {text}"
        )));
    }
    Ok(Some(value * pixels_per_unit / 96.0))
}

fn default_if_zero(value: Option<f64>, default: f64) -> f64 {
    match value {
        Some(value) if value != 0.0 => value,
        _ => default,
    }
}

fn print_params(options: &PdfOptions) -> Result<serde_json::Value, HostError> {
    let (paper_width, paper_height) = if let Some(format) = options.format.as_deref() {
        paper_format(format)
            .ok_or_else(|| HostError::InvalidArgument(format!("Unknown paper format: {format}")))?
    } else {
        (
            default_if_zero(print_parameter_inches(options.width.as_deref())?, 8.5),
            default_if_zero(print_parameter_inches(options.height.as_deref())?, 11.0),
        )
    };
    let margin_top = default_if_zero(print_parameter_inches(options.margins.top.as_deref())?, 0.0);
    let margin_right = default_if_zero(
        print_parameter_inches(options.margins.right.as_deref())?,
        0.0,
    );
    let margin_bottom = default_if_zero(
        print_parameter_inches(options.margins.bottom.as_deref())?,
        0.0,
    );
    let margin_left = default_if_zero(
        print_parameter_inches(options.margins.left.as_deref())?,
        0.0,
    );
    Ok(serde_json::json!({
        "landscape": options.landscape,
        "displayHeaderFooter": options.display_header_footer,
        "headerTemplate": options.header_template,
        "footerTemplate": options.footer_template,
        "printBackground": options.print_background,
        "scale": options.scale,
        "paperWidth": paper_width,
        "paperHeight": paper_height,
        "marginTop": margin_top,
        "marginBottom": margin_bottom,
        "marginLeft": margin_left,
        "marginRight": margin_right,
        "pageRanges": options.page_ranges,
        "preferCSSPageSize": options.prefer_css_page_size,
        "generateTaggedPDF": options.tagged,
        "generateDocumentOutline": options.outline,
        "transferMode": "ReturnAsStream"
    }))
}

async fn read_stream(
    installation: &Installation,
    _page: ObjectId,
    guard: &StreamCloseGuard,
) -> Result<StreamAccumulator, HostError> {
    let mut output = installation.accumulator();
    loop {
        let response = guard
            .session()
            .send(
                READ_METHOD,
                serde_json::json!({
                    "handle": guard.handle(),
                    "size": READ_CHUNK_BYTES
                }),
            )
            .await
            .map_err(HostError::backend)?;
        let response: IoReadReturns =
            serde_json::from_value(response).map_err(HostError::backend)?;
        output.push(&response.data, response.base64_encoded)?;
        if response.eof {
            return Ok(output);
        }
    }
}

impl HostSession {
    pub async fn page_pdf(
        &self,
        page_id: &str,
        options: PdfOptions,
    ) -> Result<PdfOutput, HostError> {
        let (page, _call, _operation) = self.admit_operation(page_id)?;
        let (session, installation, queue) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            let topology = live.composition.topology().map_err(HostError::backend)?;
            let route = topology
                .topology()
                .page_session_routes()
                .find(|route| route.object_id() == page)
                .ok_or(HostError::TargetClosed)?;
            (
                CdpSession::new(
                    live.composition.connection().clone(),
                    route.session_id().to_owned(),
                    route.target_id().to_owned(),
                ),
                Arc::clone(&live.page_pdf),
                Arc::clone(&live.screenshot_queue),
            )
        };
        let params = print_params(&options)?;
        queue
            .run(page, 0, &self.stop, async move {
                let response = session
                    .send(PRINT_METHOD, params)
                    .await
                    .map_err(HostError::backend)?;
                let response: PrintToPdfReturns =
                    serde_json::from_value(response).map_err(HostError::backend)?;
                let mut guard = StreamCloseGuard::new(session, response.stream);
                if guard.handle().len() > MAX_STREAM_HANDLE_BYTES {
                    let _ = guard.close().await;
                    return Err(HostError::ResourceExhausted(format!(
                        "PDF stream handle exceeds {MAX_STREAM_HANDLE_BYTES} bytes"
                    )));
                }
                let read = read_stream(&installation, page, &guard).await;
                let close = guard.close().await;
                match read {
                    Err(error) => Err(error),
                    Ok(output) => {
                        close?;
                        output.finish()
                    }
                }
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playwright_rust_core::ResourceLimits;

    fn resources() -> ProductionResourceLedger {
        let required = requirements();
        ProductionResourceLedger::new(
            ResourceLimits::deny_all(required.scopes, required.leases),
            required,
        )
        .unwrap()
    }

    #[test]
    fn default_payload_matches_playwright_letter_defaults() {
        // Arrange
        let expected = serde_json::json!({
            "landscape": false,
            "displayHeaderFooter": false,
            "headerTemplate": "",
            "footerTemplate": "",
            "printBackground": false,
            "scale": 1.0,
            "paperWidth": 8.5,
            "paperHeight": 11.0,
            "marginTop": 0.0,
            "marginBottom": 0.0,
            "marginLeft": 0.0,
            "marginRight": 0.0,
            "pageRanges": "",
            "preferCSSPageSize": false,
            "generateTaggedPDF": false,
            "generateDocumentOutline": false,
            "transferMode": "ReturnAsStream"
        });

        // Act
        let payload = print_params(&PdfOptions::default()).unwrap();

        // Assert
        assert_eq!(PRINT_METHOD, "Page.printToPDF");
        assert_eq!(payload, expected);
    }

    #[test]
    fn explicit_tagged_and_outline_options_map_to_chromium_fields() {
        // Arrange
        let options = PdfOptions {
            tagged: true,
            outline: true,
            ..PdfOptions::default()
        };

        // Act
        let payload = print_params(&options).unwrap();

        // Assert
        assert_eq!(payload["generateTaggedPDF"], true);
        assert_eq!(payload["generateDocumentOutline"], true);
        assert_eq!(payload["transferMode"], "ReturnAsStream");
    }

    #[test]
    fn complete_options_map_to_chromium_fields_and_units() {
        // Arrange
        let options = PdfOptions {
            scale: 0.75,
            display_header_footer: true,
            header_template: "<span class=title></span>".to_owned(),
            footer_template: "<span class=pageNumber></span>".to_owned(),
            print_background: true,
            landscape: true,
            page_ranges: "1-3, 5".to_owned(),
            width: Some("192px".to_owned()),
            height: Some("25.4mm".to_owned()),
            prefer_css_page_size: true,
            margins: PdfMargins {
                top: Some("2.54cm".to_owned()),
                right: Some("1in".to_owned()),
                bottom: Some("96px".to_owned()),
                left: Some("25.4mm".to_owned()),
            },
            tagged: true,
            outline: true,
            ..PdfOptions::default()
        };

        // Act
        let payload = print_params(&options).unwrap();

        // Assert
        assert_eq!(payload["scale"], 0.75);
        assert_eq!(payload["displayHeaderFooter"], true);
        assert_eq!(payload["headerTemplate"], options.header_template);
        assert_eq!(payload["footerTemplate"], options.footer_template);
        assert_eq!(payload["printBackground"], true);
        assert_eq!(payload["landscape"], true);
        assert_eq!(payload["pageRanges"], "1-3, 5");
        assert_eq!(payload["preferCSSPageSize"], true);
        assert_eq!(payload["paperWidth"], 2.0);
        assert_eq!(payload["paperHeight"], 1.000125);
        assert_eq!(payload["marginTop"], 1.000125);
        assert_eq!(payload["marginRight"], 1.0);
        assert_eq!(payload["marginBottom"], 1.0);
        assert_eq!(payload["marginLeft"], 1.000125);
        assert_eq!(payload["generateTaggedPDF"], true);
        assert_eq!(payload["generateDocumentOutline"], true);
    }

    #[test]
    fn format_is_case_insensitive_and_overrides_explicit_dimensions() {
        // Arrange
        let options = PdfOptions {
            format: Some("a4".to_owned()),
            width: Some("1in".to_owned()),
            height: Some("2in".to_owned()),
            ..PdfOptions::default()
        };

        // Act
        let payload = print_params(&options).unwrap();

        // Assert
        assert_eq!(payload["paperWidth"], 8.27);
        assert_eq!(payload["paperHeight"], 11.7);
    }

    #[test]
    fn unknown_format_and_invalid_dimension_match_playwright_errors() {
        // Arrange
        let format = PdfOptions {
            format: Some("poster".to_owned()),
            ..PdfOptions::default()
        };
        let dimension = PdfOptions {
            width: Some("wide".to_owned()),
            ..PdfOptions::default()
        };

        // Act
        let format_error = print_params(&format).unwrap_err();
        let dimension_error = print_params(&dimension).unwrap_err();

        // Assert
        assert!(
            format_error
                .to_string()
                .contains("Unknown paper format: poster")
        );
        assert!(
            dimension_error
                .to_string()
                .contains("Failed to parse parameter value: wide")
        );
    }

    #[test]
    fn streamed_output_preserves_plain_and_base64_chunks() {
        // Arrange
        let resources = resources();
        let installation = Installation::new(&resources).unwrap();

        // Act
        let mut streamed = installation.accumulator();
        streamed.push("%PDF", false).unwrap();
        streamed.push(&STANDARD.encode(b"-1.7\n"), true).unwrap();
        let output = streamed.finish().unwrap();
        // Assert
        assert_eq!(output.as_bytes(), b"%PDF-1.7\n");
    }

    #[test]
    fn stream_grows_past_the_legacy_two_megabyte_limit() {
        // Arrange
        let resources = resources();
        let installation = Installation::new(&resources).unwrap();
        let mut streamed = installation.accumulator();
        streamed.bytes.resize(2 * 1024 * 1024, b'A');

        // Act
        streamed.push("B", false).unwrap();
        let output = streamed.finish().unwrap();

        // Assert
        assert_eq!(output.as_bytes().len(), 2 * 1024 * 1024 + 1);
    }
}
