//! Backend-neutral Playwright-compatible HAR recording policy.
//!
//! The recorder consumes already-normalized HTTP lifecycle snapshots. It owns
//! no browser, CDP session, event subscription, task, runtime, filesystem, or
//! archive writer. Callers retain those capabilities and decide when to fetch
//! response bodies and where to persist the returned export.

use std::{collections::BTreeMap, time::UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use sha1::{Digest as _, Sha1};
use thiserror::Error;
use url::Url;

mod mime_extensions;

const PLAYWRIGHT_COMPAT_VERSION: &str = "1.58.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HarContentPolicy {
    Omit,
    Embed,
    Attach,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HarMode {
    Full,
    Minimal,
}

pub fn playwright_page_id(native_id: &str) -> String {
    let digest = format!("{:x}", Sha1::digest(native_id.as_bytes()));
    format!("page@{}", &digest[..32])
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarRecorderOptions {
    pub content: HarContentPolicy,
    pub mode: HarMode,
    pub browser_name: String,
    pub browser_version: String,
}

impl Default for HarRecorderOptions {
    fn default() -> Self {
        Self {
            content: HarContentPolicy::Embed,
            mode: HarMode::Minimal,
            browser_name: "chromium".to_owned(),
            browser_version: String::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarHeader {
    pub name: String,
    pub value: String,
}

impl HarHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HarTiming {
    pub start_time_micros: i64,
    pub domain_lookup_start_micros: i64,
    pub domain_lookup_end_micros: i64,
    pub connect_start_micros: i64,
    pub secure_connection_start_micros: i64,
    pub connect_end_micros: i64,
    pub request_start_micros: i64,
    pub response_start_micros: i64,
    pub response_end_micros: i64,
}

impl HarTiming {
    pub const UNAVAILABLE_MICROS: i64 = -1_000;

    pub const fn unavailable() -> Self {
        Self {
            start_time_micros: 0,
            domain_lookup_start_micros: Self::UNAVAILABLE_MICROS,
            domain_lookup_end_micros: Self::UNAVAILABLE_MICROS,
            connect_start_micros: Self::UNAVAILABLE_MICROS,
            secure_connection_start_micros: Self::UNAVAILABLE_MICROS,
            connect_end_micros: Self::UNAVAILABLE_MICROS,
            request_start_micros: Self::UNAVAILABLE_MICROS,
            response_start_micros: Self::UNAVAILABLE_MICROS,
            response_end_micros: Self::UNAVAILABLE_MICROS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarPageSnapshot {
    pub id: String,
    pub started_time_micros: i64,
    pub title: String,
    pub on_content_load_micros: Option<i64>,
    pub on_load_micros: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarSecurityDetails {
    pub protocol: Option<String>,
    pub subject_name: Option<String>,
    pub issuer: Option<String>,
    pub valid_from_micros: Option<i64>,
    pub valid_to_micros: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HarSizes {
    pub request_headers: i64,
    pub response_body: i64,
    pub response_headers: i64,
    pub transfer: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarRequestSnapshot {
    pub id: String,
    pub page_id: Option<String>,
    pub service_worker_id: Option<String>,
    pub redirected_from: Option<String>,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub headers: Vec<HarHeader>,
    pub post_data: Option<Vec<u8>>,
    pub timing: HarTiming,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarResponseSnapshot {
    pub status: i64,
    pub status_text: String,
    pub headers: Vec<HarHeader>,
    pub body: Option<Vec<u8>>,
    pub timing: HarTiming,
    pub http_version: String,
    pub server_ip_address: Option<String>,
    pub server_port: Option<u16>,
    pub security_details: Option<HarSecurityDetails>,
    pub sizes: Option<HarSizes>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarAttachment {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarExport {
    pub json: Vec<u8>,
    pub attachments: Vec<HarAttachment>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum HarRecordingError {
    #[error("HAR request {0} was observed more than once")]
    DuplicateRequest(String),
    #[error("HAR event references unknown request {0}")]
    UnknownRequest(String),
    #[error("HAR request {0} has already reached a terminal state")]
    RequestAlreadyFinished(String),
    #[error("HAR URL is invalid: {0}")]
    InvalidUrl(String),
    #[error("HAR serialization failed: {0}")]
    Serialization(String),
}

#[derive(Clone, Debug)]
struct EntryState {
    request: HarRequestSnapshot,
    response: Option<HarResponseSnapshot>,
    failure_text: Option<String>,
    redirect_url: String,
    finished: bool,
}

pub struct HarRecorder {
    options: HarRecorderOptions,
    entries: Vec<EntryState>,
    pages: Vec<HarPageSnapshot>,
    active: BTreeMap<String, usize>,
}

impl HarRecorder {
    pub fn new(options: HarRecorderOptions) -> Self {
        Self {
            options,
            entries: Vec::new(),
            pages: Vec::new(),
            active: BTreeMap::new(),
        }
    }

    pub const fn mode(&self) -> HarMode {
        self.options.mode
    }

    pub fn request_started(
        &mut self,
        request: HarRequestSnapshot,
    ) -> Result<(), HarRecordingError> {
        if self.active.contains_key(&request.id)
            || self
                .entries
                .iter()
                .any(|entry| entry.request.id == request.id)
        {
            return Err(HarRecordingError::DuplicateRequest(request.id));
        }
        if self.options.mode == HarMode::Full
            && let Some(page_id) = request.page_id.as_ref()
            && !self.pages.iter().any(|page| page.id == *page_id)
        {
            self.pages.push(HarPageSnapshot {
                id: page_id.clone(),
                started_time_micros: request.timing.start_time_micros,
                title: String::new(),
                on_content_load_micros: None,
                on_load_micros: None,
            });
        }
        if let Some(redirected_from) = request.redirected_from.as_ref()
            && let Some(previous) = self
                .entries
                .iter_mut()
                .find(|entry| entry.request.id == *redirected_from)
        {
            previous.redirect_url.clone_from(&request.url);
        }
        let index = self.entries.len();
        self.active.insert(request.id.clone(), index);
        self.entries.push(EntryState {
            request,
            response: None,
            failure_text: None,
            redirect_url: String::new(),
            finished: false,
        });
        Ok(())
    }

    pub fn record_page(&mut self, page: HarPageSnapshot) {
        if self.options.mode != HarMode::Full {
            return;
        }
        if let Some(existing) = self
            .pages
            .iter_mut()
            .find(|existing| existing.id == page.id)
        {
            *existing = page;
        } else {
            self.pages.push(page);
        }
    }

    pub fn ensure_page(&mut self, page: HarPageSnapshot) {
        if self.options.mode == HarMode::Full
            && !self.pages.iter().any(|existing| existing.id == page.id)
        {
            self.pages.push(page);
        }
    }

    pub fn update_page_lifecycle(
        &mut self,
        page_id: &str,
        title: String,
        on_content_load_micros: Option<i64>,
        on_load_micros: Option<i64>,
    ) {
        if self.options.mode != HarMode::Full {
            return;
        }
        let Some(page) = self.pages.iter_mut().find(|page| page.id == page_id) else {
            return;
        };
        page.title = title;
        if let Some(on_content_load_micros) = on_content_load_micros {
            page.on_content_load_micros =
                Some(on_content_load_micros.saturating_sub(page.started_time_micros));
        }
        if let Some(on_load_micros) = on_load_micros {
            page.on_load_micros = Some(on_load_micros.saturating_sub(page.started_time_micros));
        }
    }

    pub fn update_request(
        &mut self,
        request_id: &str,
        request: HarRequestSnapshot,
    ) -> Result<(), HarRecordingError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.request.id == request_id)
            .ok_or_else(|| HarRecordingError::UnknownRequest(request_id.to_owned()))?;
        entry.request = request;
        Ok(())
    }

    pub fn response_received(
        &mut self,
        request_id: &str,
        response: HarResponseSnapshot,
    ) -> Result<(), HarRecordingError> {
        let entry = self.active_entry_mut(request_id)?;
        update_response(entry, response);
        Ok(())
    }

    pub fn request_finished(
        &mut self,
        request_id: &str,
        response: Option<HarResponseSnapshot>,
        body: Option<Vec<u8>>,
    ) -> Result<(), HarRecordingError> {
        let index = self.active_index(request_id)?;
        let entry = &mut self.entries[index];
        if let Some(response) = response {
            update_response(entry, response);
        }
        if let Some(body) = body
            && let Some(response) = entry.response.as_mut()
        {
            response.body = Some(body);
        }
        entry.finished = true;
        self.active.remove(request_id);
        Ok(())
    }

    pub fn request_failed(
        &mut self,
        request_id: &str,
        response: Option<HarResponseSnapshot>,
        failure_text: impl Into<String>,
    ) -> Result<(), HarRecordingError> {
        let index = self.active_index(request_id)?;
        let entry = &mut self.entries[index];
        if let Some(response) = response {
            update_response(entry, response);
        }
        entry.failure_text = Some(failure_text.into());
        entry.finished = true;
        self.active.remove(request_id);
        Ok(())
    }

    /// Retires correlations owned by a closing Service Worker without
    /// discarding the already-observed request. Playwright records these as
    /// failed/incomplete entries rather than duplicating them after restart.
    pub fn retire_service_worker(&mut self, service_worker_id: &str) {
        let retired = self
            .active
            .iter()
            .filter_map(|(request_id, index)| {
                (self.entries[*index].request.service_worker_id.as_deref()
                    == Some(service_worker_id))
                .then_some((request_id.clone(), *index))
            })
            .collect::<Vec<_>>();
        for (request_id, index) in retired {
            let entry = &mut self.entries[index];
            entry.failure_text = Some("Service Worker closed".to_owned());
            entry.finished = true;
            self.active.remove(&request_id);
        }
    }

    pub fn active_request_count(&self) -> usize {
        self.active.len()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn contains_request(&self, request_id: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.request.id == request_id)
    }

    /// Whether this recorder needs response bytes from the browser backend.
    /// Metadata-only recordings deliberately avoid issuing a body command.
    pub const fn wants_response_bodies(&self) -> bool {
        !matches!(self.options.content, HarContentPolicy::Omit)
    }

    /// Whether an observed request still needs response bytes. This remains
    /// valid after the request reached a terminal state so callers may first
    /// commit lifecycle order and then perform the asynchronous body fetch.
    pub fn needs_response_body(&self, request_id: &str) -> bool {
        self.wants_response_bodies()
            && self
                .entries
                .iter()
                .find(|entry| entry.request.id == request_id)
                .and_then(|entry| entry.response.as_ref())
                .is_some_and(|response| response.body.is_none())
    }

    /// Supplies body bytes after terminal lifecycle publication. Backends such
    /// as CDP only permit `Network.getResponseBody` after loading finished.
    pub fn set_response_body(
        &mut self,
        request_id: &str,
        body: &[u8],
    ) -> Result<(), HarRecordingError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.request.id == request_id)
            .ok_or_else(|| HarRecordingError::UnknownRequest(request_id.to_owned()))?;
        let response = entry
            .response
            .as_mut()
            .ok_or_else(|| HarRecordingError::UnknownRequest(request_id.to_owned()))?;
        response.body = Some(body.to_vec());
        Ok(())
    }

    pub fn export(&self) -> Result<HarExport, HarRecordingError> {
        let mut attachments = BTreeMap::new();
        let entries = self
            .entries
            .iter()
            .map(|entry| self.export_entry(entry, &mut attachments))
            .collect::<Result<Vec<_>, _>>()?;
        let document = Document {
            log: Log {
                version: "1.2",
                creator: Product {
                    name: "Playwright",
                    version: PLAYWRIGHT_COMPAT_VERSION,
                },
                browser: ProductOwned {
                    name: self.options.browser_name.clone(),
                    version: self.options.browser_version.clone(),
                },
                pages: (self.options.mode == HarMode::Full && !self.pages.is_empty()).then(|| {
                    self.pages
                        .iter()
                        .map(|page| Page {
                            started_date_time: format_timestamp(page.started_time_micros),
                            id: page.id.clone(),
                            title: page.title.clone(),
                            page_timings: PageTimings {
                                on_content_load: page_timing(page.on_content_load_micros),
                                on_load: page_timing(page.on_load_micros),
                            },
                        })
                        .collect()
                }),
                entries,
            },
        };
        let json = serde_json::to_vec_pretty(&document)
            .map_err(|error| HarRecordingError::Serialization(error.to_string()))?;
        Ok(HarExport {
            json,
            attachments: attachments.into_values().collect(),
        })
    }

    fn active_index(&self, request_id: &str) -> Result<usize, HarRecordingError> {
        if let Some(index) = self.active.get(request_id) {
            return Ok(*index);
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.request.id == request_id && entry.finished)
        {
            return Err(HarRecordingError::RequestAlreadyFinished(
                request_id.to_owned(),
            ));
        }
        Err(HarRecordingError::UnknownRequest(request_id.to_owned()))
    }

    fn active_entry_mut(&mut self, request_id: &str) -> Result<&mut EntryState, HarRecordingError> {
        let index = self.active_index(request_id)?;
        Ok(&mut self.entries[index])
    }

    fn export_entry(
        &self,
        entry: &EntryState,
        attachments: &mut BTreeMap<String, HarAttachment>,
    ) -> Result<Entry, HarRecordingError> {
        let parsed = Url::parse(&entry.request.url)
            .map_err(|error| HarRecordingError::InvalidUrl(error.to_string()))?;
        let request_mime = header_value(&entry.request.headers, "content-type")
            .unwrap_or("application/octet-stream");
        let post_data =
            entry.request.post_data.as_deref().map(|bytes| {
                post_data_payload(request_mime, bytes, self.options.content, attachments)
            });
        let response = entry.response.as_ref();
        let response_headers = response.map_or(&[][..], |response| response.headers.as_slice());
        let response_mime = header_value(response_headers, "content-type").unwrap_or("x-unknown");
        let response_body = response.and_then(|response| response.body.as_deref());
        let content = response_body.map_or_else(
            || Content {
                size: -1,
                compression: None,
                mime_type: response_mime.to_owned(),
                text: None,
                encoding: None,
                file: None,
            },
            |bytes| {
                content_payload(
                    response_mime,
                    &entry.request.resource_type,
                    bytes,
                    self.options.content,
                    self.options.mode,
                    attachments,
                )
            },
        );
        let response_timing = response
            .map(|response| response.timing)
            .unwrap_or(entry.request.timing);
        let timings = export_timings(response_timing, self.options.mode);
        let time = if self.options.mode == HarMode::Minimal {
            -1.0
        } else {
            [
                timings.dns.unwrap_or(-1.0),
                timings.connect.unwrap_or(-1.0),
                timings.ssl.unwrap_or(-1.0),
                timings.wait,
                timings.receive,
            ]
            .into_iter()
            .filter(|value| *value > 0.0)
            .sum()
        };
        let full = self.options.mode == HarMode::Full;
        let http_version = response
            .map(|response| response.http_version.as_str())
            .filter(|version| !version.is_empty())
            .unwrap_or("HTTP/1.1")
            .to_owned();
        let sizes = full
            .then(|| response.and_then(|response| response.sizes))
            .flatten();
        let reference_time_micros = response_timing
            .start_time_micros
            .saturating_add(response_timing.response_start_micros.max(0));
        let request_cookies = if full {
            request_cookies(&entry.request.headers, reference_time_micros)
        } else {
            Vec::new()
        };
        let response_cookies = if full {
            response_cookies(response_headers, reference_time_micros)
        } else {
            Vec::new()
        };
        let compression = if full {
            response_body.zip(sizes).map(|(body, sizes)| {
                i64::try_from(body.len())
                    .unwrap_or(i64::MAX)
                    .saturating_sub(sizes.response_body)
                    .max(0)
            })
        } else {
            None
        };
        let security_details = full
            .then(|| response.and_then(|response| response.security_details.as_ref()))
            .flatten()
            .map(export_security_details);
        Ok(Entry {
            page_ref: (self.options.mode == HarMode::Full)
                .then(|| entry.request.page_id.clone())
                .flatten(),
            started_date_time: format_timestamp(entry.request.timing.start_time_micros),
            time,
            request: Request {
                method: entry.request.method.clone(),
                url: entry.request.url.clone(),
                http_version: http_version.clone(),
                cookies: request_cookies,
                headers: export_headers(&entry.request.headers),
                query_string: parsed
                    .query_pairs()
                    .map(|(name, value)| NameValue {
                        name: name.into_owned(),
                        value: value.into_owned(),
                    })
                    .collect(),
                post_data,
                headers_size: sizes.map_or(-1, |sizes| sizes.request_headers),
                body_size: request_body_size(entry.request.post_data.as_deref(), self.options.mode),
            },
            response: Response {
                status: response.map_or(-1, |value| value.status),
                status_text: response.map_or_else(String::new, |value| value.status_text.clone()),
                http_version,
                cookies: response_cookies,
                headers: export_headers(response_headers),
                content: Content {
                    compression,
                    ..content
                },
                headers_size: sizes.map_or(-1, |sizes| sizes.response_headers),
                body_size: sizes.map_or_else(
                    || body_size(response_body, self.options.mode),
                    |sizes| sizes.response_body,
                ),
                redirect_url: entry.redirect_url.clone(),
                transfer_size: full.then(|| sizes.map_or(-1, |sizes| sizes.transfer)),
                failure_text: entry.failure_text.clone(),
            },
            cache: Cache {},
            timings,
            server_ip_address: full
                .then(|| response.and_then(|response| response.server_ip_address.clone()))
                .flatten(),
            server_port: full
                .then(|| response.and_then(|response| response.server_port))
                .flatten(),
            security_details,
        })
    }
}

fn update_response(entry: &mut EntryState, response: HarResponseSnapshot) {
    if let Some(location) = header_value(&response.headers, "location") {
        entry.redirect_url = location.to_owned();
    }
    entry.response = Some(response);
}

fn header_value<'a>(headers: &'a [HarHeader], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

fn export_headers(headers: &[HarHeader]) -> Vec<NameValue> {
    headers
        .iter()
        .map(|header| NameValue {
            name: header.name.clone(),
            value: header.value.clone(),
        })
        .collect()
}

fn request_cookies(headers: &[HarHeader], reference_time_micros: i64) -> Vec<Cookie> {
    headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("cookie"))
        .flat_map(|header| header.value.split(';'))
        .map(|value| parse_cookie(value, reference_time_micros))
        .collect()
}

fn response_cookies(headers: &[HarHeader], reference_time_micros: i64) -> Vec<Cookie> {
    headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("set-cookie"))
        .map(|header| parse_cookie(&header.value, reference_time_micros))
        .collect()
}

fn parse_cookie(value: &str, reference_time_micros: i64) -> Cookie {
    let mut cookie = Cookie::default();
    for (index, pair) in value.split(';').map(str::trim).enumerate() {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let name = name.trim();
        let value = value.trim();
        if index == 0 {
            cookie.name = name.to_owned();
            cookie.value = value.to_owned();
            continue;
        }
        match name {
            "Domain" => cookie.domain = Some(value.to_owned()),
            "Expires" => cookie.expires = parse_cookie_date(value),
            "HttpOnly" => cookie.http_only = Some(true),
            "Max-Age" => {
                cookie.expires = value.parse::<i64>().ok().map(|seconds| {
                    format_timestamp(
                        reference_time_micros.saturating_add(seconds.saturating_mul(1_000_000)),
                    )
                });
            }
            "Path" => cookie.path = Some(value.to_owned()),
            "SameSite" => cookie.same_site = Some(value.to_owned()),
            "Secure" => cookie.secure = Some(true),
            _ => {}
        }
    }
    cookie
}

fn parse_cookie_date(value: &str) -> Option<String> {
    let timestamp = httpdate::parse_http_date(value).ok()?;
    let micros = timestamp
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_micros()
        .try_into()
        .ok()?;
    Some(format_timestamp(micros))
}

fn export_security_details(details: &HarSecurityDetails) -> SecurityDetails {
    SecurityDetails {
        protocol: details.protocol.clone(),
        subject_name: details.subject_name.clone(),
        issuer: details.issuer.clone(),
        valid_from: details.valid_from_micros.map(seconds),
        valid_to: details.valid_to_micros.map(seconds),
    }
}

fn seconds(micros: i64) -> f64 {
    micros as f64 / 1_000_000.0
}

fn page_timing(micros: Option<i64>) -> f64 {
    micros.map_or(-1.0, |micros| micros as f64 / 1_000.0)
}

fn content_payload(
    mime_type: &str,
    resource_type: &str,
    bytes: &[u8],
    policy: HarContentPolicy,
    mode: HarMode,
    attachments: &mut BTreeMap<String, HarAttachment>,
) -> Content {
    let mut content = Content {
        size: if mode == HarMode::Minimal {
            -1
        } else {
            i64::try_from(bytes.len()).unwrap_or(i64::MAX)
        },
        compression: None,
        mime_type: mime_type.to_owned(),
        text: None,
        encoding: None,
        file: None,
    };
    match policy {
        HarContentPolicy::Omit => {}
        HarContentPolicy::Embed
            if is_textual_mime(mime_type) && !resource_type.eq_ignore_ascii_case("font") =>
        {
            content.text = Some(String::from_utf8_lossy(bytes).into_owned());
        }
        HarContentPolicy::Embed => {
            content.text = Some(STANDARD.encode(bytes));
            content.encoding = Some("base64");
        }
        HarContentPolicy::Attach => {
            let name = attachment_name(bytes, mime_type);
            content.file = Some(name.clone());
            attachments
                .entry(name.clone())
                .or_insert_with(|| HarAttachment {
                    name,
                    bytes: bytes.to_vec(),
                });
        }
    }
    content
}

fn post_data_payload(
    mime_type: &str,
    bytes: &[u8],
    policy: HarContentPolicy,
    attachments: &mut BTreeMap<String, HarAttachment>,
) -> PostData {
    let mut post_data = PostData {
        mime_type: mime_type.to_owned(),
        text: String::new(),
        params: Vec::new(),
        file: None,
    };
    if policy == HarContentPolicy::Embed && mime_type != "application/octet-stream" {
        post_data.text = String::from_utf8_lossy(bytes).into_owned();
    }
    if policy == HarContentPolicy::Attach {
        let name = attachment_name(bytes, mime_type);
        post_data.file = Some(name.clone());
        attachments
            .entry(name.clone())
            .or_insert_with(|| HarAttachment {
                name,
                bytes: bytes.to_vec(),
            });
    }
    if mime_type == "application/x-www-form-urlencoded" {
        post_data.params = url::form_urlencoded::parse(bytes)
            .map(|(name, value)| NameValue {
                name: name.into_owned(),
                value: value.into_owned(),
            })
            .collect();
    }
    post_data
}

fn attachment_name(bytes: &[u8], mime_type: &str) -> String {
    let digest = Sha1::digest(bytes);
    let mime_type = mime_type
        .split(';')
        .next()
        .unwrap_or(mime_type)
        .trim()
        .to_ascii_lowercase();
    let extension = mime_extensions::preferred_extension(&mime_type).unwrap_or("dat");
    format!("{digest:x}.{extension}")
}

fn is_textual_mime(mime_type: &str) -> bool {
    let mime = mime_type.to_ascii_lowercase();
    let (essence, parameter) = mime
        .split_once(';')
        .map_or((mime.as_str(), None), |(essence, parameter)| {
            (essence, Some(parameter.trim()))
        });
    if parameter.is_some_and(|parameter| !parameter.starts_with("charset=")) {
        return false;
    }
    essence.starts_with("text/")
        || matches!(
            essence,
            "application/json"
                | "application/javascript"
                | "application/x-javascript"
                | "application/ecmascript"
                | "application/graphql"
                | "application/x-www-form-urlencoded"
                | "image/svg"
                | "image/svg+xml"
        )
        || essence.starts_with("application/xml")
        || (essence.starts_with("application/")
            && (essence.ends_with("+json") || essence.ends_with("+xml")))
}

fn body_size(body: Option<&[u8]>, mode: HarMode) -> i64 {
    if mode == HarMode::Minimal {
        -1
    } else {
        body.map_or(-1, |bytes| i64::try_from(bytes.len()).unwrap_or(i64::MAX))
    }
}

fn request_body_size(body: Option<&[u8]>, mode: HarMode) -> i64 {
    if mode == HarMode::Minimal {
        -1
    } else {
        body.map_or(0, |bytes| i64::try_from(bytes.len()).unwrap_or(i64::MAX))
    }
}

fn export_timings(timing: HarTiming, mode: HarMode) -> Timings {
    if mode == HarMode::Minimal {
        return Timings {
            dns: None,
            connect: None,
            send: -1.0,
            wait: -1.0,
            receive: -1.0,
            ssl: None,
        };
    }
    let request_start = timing.request_start_micros;
    let response_start = timing.response_start_micros;
    let response_end = timing.response_end_micros;
    Timings {
        dns: Some(delta_millis_if_end_available(
            timing.domain_lookup_start_micros,
            timing.domain_lookup_end_micros,
        )),
        connect: Some(delta_millis_if_end_available(
            timing.connect_start_micros,
            timing.connect_end_micros,
        )),
        send: 0.0,
        wait: delta_millis_if_end_available(request_start, response_start),
        receive: delta_millis_if_end_available(response_start, response_end),
        ssl: Some(delta_millis_if_end_available(
            timing.secure_connection_start_micros,
            timing.connect_end_micros,
        )),
    }
}

fn delta_millis_if_end_available(start: i64, end: i64) -> f64 {
    if end < 0 {
        -1.0
    } else {
        end.saturating_sub(start) as f64 / 1_000.0
    }
}

fn format_timestamp(micros: i64) -> String {
    let micros = micros.max(0);
    let seconds = micros.div_euclid(1_000_000);
    let milliseconds = micros.rem_euclid(1_000_000) / 1_000;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    let minute = day_seconds.rem_euclid(3_600) / 60;
    let second = day_seconds.rem_euclid(60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[derive(Serialize)]
struct Document {
    log: Log,
}

#[derive(Serialize)]
struct Log {
    version: &'static str,
    creator: Product,
    browser: ProductOwned,
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<Vec<Page>>,
    entries: Vec<Entry>,
}

#[derive(Serialize)]
struct Product {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct ProductOwned {
    name: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    started_date_time: String,
    id: String,
    title: String,
    page_timings: PageTimings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PageTimings {
    on_content_load: f64,
    on_load: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(rename = "pageref", skip_serializing_if = "Option::is_none")]
    page_ref: Option<String>,
    started_date_time: String,
    time: f64,
    request: Request,
    response: Response,
    cache: Cache,
    timings: Timings,
    #[serde(rename = "serverIPAddress", skip_serializing_if = "Option::is_none")]
    server_ip_address: Option<String>,
    #[serde(rename = "_serverPort", skip_serializing_if = "Option::is_none")]
    server_port: Option<u16>,
    #[serde(rename = "_securityDetails", skip_serializing_if = "Option::is_none")]
    security_details: Option<SecurityDetails>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    method: String,
    url: String,
    http_version: String,
    cookies: Vec<Cookie>,
    headers: Vec<NameValue>,
    query_string: Vec<NameValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post_data: Option<PostData>,
    headers_size: i64,
    body_size: i64,
}

#[derive(Serialize)]
struct PostData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    text: String,
    params: Vec<NameValue>,
    #[serde(rename = "_file", skip_serializing_if = "Option::is_none")]
    file: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    status: i64,
    status_text: String,
    http_version: String,
    cookies: Vec<Cookie>,
    headers: Vec<NameValue>,
    content: Content,
    headers_size: i64,
    body_size: i64,
    #[serde(rename = "redirectURL")]
    redirect_url: String,
    #[serde(rename = "_transferSize", skip_serializing_if = "Option::is_none")]
    transfer_size: Option<i64>,
    #[serde(rename = "_failureText", skip_serializing_if = "Option::is_none")]
    failure_text: Option<String>,
}

#[derive(Serialize)]
struct Content {
    size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    compression: Option<i64>,
    #[serde(rename = "mimeType")]
    mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding: Option<&'static str>,
    #[serde(rename = "_file", skip_serializing_if = "Option::is_none")]
    file: Option<String>,
}

#[derive(Serialize)]
struct NameValue {
    name: String,
    value: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Cookie {
    name: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    same_site: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecurityDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subject_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    valid_from: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    valid_to: Option<f64>,
}

#[derive(Serialize)]
struct Cache {}

#[derive(Serialize)]
struct Timings {
    #[serde(skip_serializing_if = "Option::is_none")]
    dns: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connect: Option<f64>,
    send: f64,
    wait: f64,
    receive: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    ssl: Option<f64>,
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn request(id: &str, worker: Option<&str>) -> HarRequestSnapshot {
        HarRequestSnapshot {
            id: id.to_owned(),
            page_id: None,
            service_worker_id: worker.map(str::to_owned),
            redirected_from: None,
            url: "https://example.test/data?item=7".to_owned(),
            method: "GET".to_owned(),
            resource_type: "fetch".to_owned(),
            headers: vec![HarHeader::new("accept", "application/json")],
            post_data: None,
            timing: HarTiming {
                start_time_micros: 1_700_000_000_123_000,
                domain_lookup_start_micros: 1_000,
                domain_lookup_end_micros: 2_000,
                connect_start_micros: 2_000,
                secure_connection_start_micros: 3_000,
                connect_end_micros: 5_000,
                request_start_micros: 10_000,
                response_start_micros: 12_000,
                response_end_micros: 15_000,
            },
        }
    }

    fn response(body: Option<&[u8]>) -> HarResponseSnapshot {
        HarResponseSnapshot {
            status: 200,
            status_text: "OK".to_owned(),
            headers: vec![HarHeader::new("content-type", "application/json")],
            body: body.map(ToOwned::to_owned),
            timing: request("timing", None).timing,
            http_version: "HTTP/2".to_owned(),
            server_ip_address: None,
            server_port: None,
            security_details: None,
            sizes: None,
        }
    }

    #[test]
    fn service_worker_lifecycle_exports_one_replayable_entry_with_body() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions::default());
        recorder
            .request_started(request("request-1", Some("worker-1")))
            .unwrap();
        recorder
            .response_received("request-1", response(None))
            .unwrap();

        // Act
        recorder
            .request_finished("request-1", None, Some(br#"{"ok":true}"#.to_vec()))
            .unwrap();
        let export = recorder.export().unwrap();
        let document: Value = serde_json::from_slice(&export.json).unwrap();

        // Assert
        assert_eq!(recorder.entry_count(), 1);
        assert_eq!(recorder.active_request_count(), 0);
        assert_eq!(document["log"]["entries"].as_array().unwrap().len(), 1);
        assert_eq!(document["log"]["entries"][0]["response"]["status"], 200);
        assert_eq!(document["log"]["creator"]["version"], "1.58.0");
        assert_eq!(document["log"]["entries"][0]["time"], -1.0);
        assert_eq!(document["log"]["entries"][0]["request"]["bodySize"], -1);
        assert_eq!(document["log"]["entries"][0]["response"]["bodySize"], -1);
        assert_eq!(
            document["log"]["entries"][0]["response"]["content"]["size"],
            -1
        );
        assert_eq!(
            document["log"]["entries"][0]["response"]["content"]["text"],
            r#"{"ok":true}"#
        );
        assert_eq!(
            document["log"]["entries"][0]["request"]["queryString"][0]["name"],
            "item"
        );
        assert!(export.attachments.is_empty());
    }

    #[test]
    fn full_mode_request_without_post_data_has_zero_body_size() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions {
            mode: HarMode::Full,
            ..HarRecorderOptions::default()
        });
        recorder
            .request_started(request("request-1", None))
            .unwrap();

        // Act
        recorder
            .request_finished("request-1", Some(response(Some(b"done"))), None)
            .unwrap();
        let document: Value = serde_json::from_slice(&recorder.export().unwrap().json).unwrap();

        // Assert
        assert_eq!(document["log"]["entries"][0]["request"]["bodySize"], 0);
    }

    #[test]
    fn duplicate_or_post_terminal_events_never_create_a_second_entry() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions::default());
        recorder
            .request_started(request("request-1", None))
            .unwrap();
        recorder
            .request_finished("request-1", Some(response(Some(b"done"))), None)
            .unwrap();

        // Act
        let duplicate = recorder.request_started(request("request-1", None));
        let terminal = recorder.request_finished("request-1", None, None);

        // Assert
        assert_eq!(
            duplicate,
            Err(HarRecordingError::DuplicateRequest("request-1".to_owned()))
        );
        assert_eq!(
            terminal,
            Err(HarRecordingError::RequestAlreadyFinished(
                "request-1".to_owned()
            ))
        );
        assert_eq!(recorder.entry_count(), 1);
    }

    #[test]
    fn response_body_can_be_supplied_after_terminal_publication() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions::default());
        recorder
            .request_started(request("request-1", None))
            .unwrap();
        recorder
            .response_received("request-1", response(None))
            .unwrap();
        recorder.request_finished("request-1", None, None).unwrap();

        // Act
        let needed_before = recorder.needs_response_body("request-1");
        recorder
            .set_response_body("request-1", b"late body")
            .unwrap();
        let needed_after = recorder.needs_response_body("request-1");
        let export = recorder.export().unwrap();
        let document: Value = serde_json::from_slice(&export.json).unwrap();

        // Assert
        assert!(needed_before);
        assert!(!needed_after);
        assert_eq!(
            document["log"]["entries"][0]["response"]["content"]["text"],
            "late body"
        );
    }

    #[test]
    fn service_worker_retirement_finishes_only_its_active_correlations() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions::default());
        recorder
            .request_started(request("worker-request", Some("worker-1")))
            .unwrap();
        recorder
            .request_started(request("page-request", None))
            .unwrap();

        // Act
        recorder.retire_service_worker("worker-1");
        let export = recorder.export().unwrap();
        let document: Value = serde_json::from_slice(&export.json).unwrap();

        // Assert
        assert_eq!(recorder.active_request_count(), 1);
        assert_eq!(
            document["log"]["entries"][0]["response"]["_failureText"],
            "Service Worker closed"
        );
        assert!(document["log"]["entries"][1]["response"]["_failureText"].is_null());
    }

    #[test]
    fn attach_policy_returns_body_artifacts_without_filesystem_ownership() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions {
            content: HarContentPolicy::Attach,
            ..HarRecorderOptions::default()
        });
        recorder
            .request_started(request("request-1", None))
            .unwrap();
        recorder
            .request_finished("request-1", Some(response(Some(b"body"))), None)
            .unwrap();

        // Act
        let export = recorder.export().unwrap();
        let document: Value = serde_json::from_slice(&export.json).unwrap();

        // Assert
        assert_eq!(export.attachments.len(), 1);
        assert_eq!(
            export.attachments[0].name,
            "02083f4579e08a612425c0c1a17ee47add783b94.json"
        );
        assert_eq!(export.attachments[0].bytes, b"body");
        assert_eq!(
            document["log"]["entries"][0]["response"]["content"]["_file"],
            "02083f4579e08a612425c0c1a17ee47add783b94.json"
        );
    }

    #[test]
    fn attach_policy_deduplicates_identical_content_by_official_sha1_name() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions {
            content: HarContentPolicy::Attach,
            ..HarRecorderOptions::default()
        });
        for id in ["request-1", "request-2"] {
            recorder.request_started(request(id, None)).unwrap();
            recorder
                .request_finished(id, Some(response(Some(b"shared"))), None)
                .unwrap();
        }

        // Act
        let export = recorder.export().unwrap();
        let document: Value = serde_json::from_slice(&export.json).unwrap();

        // Assert
        assert_eq!(export.attachments.len(), 1);
        assert_eq!(
            export.attachments[0].name,
            "d18aac96b905b4b3c839891b7a91c9414149514c.json"
        );
        assert_eq!(
            document["log"]["entries"][0]["response"]["content"]["_file"],
            document["log"]["entries"][1]["response"]["content"]["_file"]
        );
    }

    #[test]
    fn attached_form_post_data_uses_official_post_data_shape_and_params() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions {
            content: HarContentPolicy::Attach,
            ..HarRecorderOptions::default()
        });
        let mut request = request("request-1", None);
        request.method = "POST".to_owned();
        request.headers = vec![HarHeader::new(
            "content-type",
            "application/x-www-form-urlencoded",
        )];
        request.post_data = Some(b"first=one&second=two".to_vec());
        recorder.request_started(request).unwrap();
        recorder
            .request_finished("request-1", Some(response(Some(b"done"))), None)
            .unwrap();

        // Act
        let export = recorder.export().unwrap();
        let document: Value = serde_json::from_slice(&export.json).unwrap();
        let post_data = &document["log"]["entries"][0]["request"]["postData"];

        // Assert
        assert_eq!(post_data["mimeType"], "application/x-www-form-urlencoded");
        assert_eq!(post_data["text"], "");
        assert_eq!(post_data["params"][0]["name"], "first");
        assert_eq!(post_data["params"][0]["value"], "one");
        assert_eq!(post_data["params"][1]["name"], "second");
        assert_eq!(post_data["params"][1]["value"], "two");
        assert_eq!(
            post_data["_file"],
            "e47d7a7e0fdadc2a76fdb6e5e06badf2c199dc0e.dat"
        );
        assert!(post_data.get("size").is_none());
        assert!(post_data.get("encoding").is_none());
        assert!(export.attachments.iter().any(|attachment| {
            attachment.name == "e47d7a7e0fdadc2a76fdb6e5e06badf2c199dc0e.dat"
                && attachment.bytes == b"first=one&second=two"
        }));
    }

    #[test]
    fn font_response_is_base64_even_when_server_claims_text_mime() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions::default());
        let mut request = request("request-1", None);
        request.resource_type = "font".to_owned();
        recorder.request_started(request).unwrap();
        let mut response = response(Some(b"font bytes"));
        response.headers = vec![HarHeader::new("content-type", "text/plain")];

        // Act
        recorder
            .request_finished("request-1", Some(response), None)
            .unwrap();
        let export = recorder.export().unwrap();
        let document: Value = serde_json::from_slice(&export.json).unwrap();
        let content = &document["log"]["entries"][0]["response"]["content"];

        // Assert
        assert_eq!(content["text"], "Zm9udCBieXRlcw==");
        assert_eq!(content["encoding"], "base64");
    }

    #[test]
    fn timestamp_formatter_preserves_epoch_and_millisecond_precision() {
        // Arrange
        let epoch = 0;
        let known = 1_700_000_000_123_456;

        // Act
        let epoch = format_timestamp(epoch);
        let known = format_timestamp(known);

        // Assert
        assert_eq!(epoch, "1970-01-01T00:00:00.000Z");
        assert_eq!(known, "2023-11-14T22:13:20.123Z");
    }

    #[test]
    fn page_identity_uses_playwrights_opaque_page_guid_shape() {
        // Arrange
        let native_id = "6";

        // Act
        let page_id = playwright_page_id(native_id);

        // Assert
        assert_eq!(page_id, "page@c1dfd96eea8cc2b62785275bca38ac26");
    }

    #[test]
    fn full_mode_exports_playwright_pages_cookies_timing_sizes_and_security_shape() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions {
            mode: HarMode::Full,
            browser_version: "157.0".to_owned(),
            ..HarRecorderOptions::default()
        });
        recorder.record_page(HarPageSnapshot {
            id: "page-1".to_owned(),
            started_time_micros: 1_700_000_000_000_000,
            title: "Example".to_owned(),
            on_content_load_micros: Some(1_234_000),
            on_load_micros: Some(2_345_000),
        });
        let mut request = request("request-1", None);
        request.page_id = Some("page-1".to_owned());
        request.method = "POST".to_owned();
        request.headers = vec![
            HarHeader::new("cookie", "session=one; theme=dark"),
            HarHeader::new("content-type", "application/x-www-form-urlencoded"),
        ];
        request.post_data = Some(b"x=1".to_vec());
        let mut response = response(Some(b"0123456789"));
        response.headers = vec![
            HarHeader::new("content-type", "text/plain"),
            HarHeader::new(
                "set-cookie",
                "sid=abc; Domain=example.test; Path=/; HttpOnly; Secure; SameSite=Lax; Expires=Wed, 21 Oct 2015 07:28:00 GMT",
            ),
        ];
        response.server_ip_address = Some("203.0.113.7".to_owned());
        response.server_port = Some(443);
        response.security_details = Some(HarSecurityDetails {
            protocol: Some("TLS 1.3".to_owned()),
            subject_name: Some("example.test".to_owned()),
            issuer: Some("Test CA".to_owned()),
            valid_from_micros: Some(1_700_000_000_000_000),
            valid_to_micros: Some(1_800_000_000_000_000),
        });
        response.sizes = Some(HarSizes {
            request_headers: 17,
            response_body: 6,
            response_headers: 23,
            transfer: 42,
        });
        recorder.request_started(request).unwrap();

        // Act
        recorder
            .request_finished("request-1", Some(response), None)
            .unwrap();
        let document: Value = serde_json::from_slice(&recorder.export().unwrap().json).unwrap();
        let page = &document["log"]["pages"][0];
        let entry = &document["log"]["entries"][0];

        // Assert
        assert_eq!(page["id"], "page-1");
        assert_eq!(page["title"], "Example");
        assert_eq!(page["pageTimings"]["onContentLoad"], 1234.0);
        assert_eq!(page["pageTimings"]["onLoad"], 2345.0);
        assert_eq!(entry["pageref"], "page-1");
        assert_eq!(entry["time"], 11.0);
        assert_eq!(entry["timings"]["dns"], 1.0);
        assert_eq!(entry["timings"]["connect"], 3.0);
        assert_eq!(entry["timings"]["ssl"], 2.0);
        assert_eq!(entry["timings"]["wait"], 2.0);
        assert_eq!(entry["timings"]["receive"], 3.0);
        assert_eq!(entry["request"]["httpVersion"], "HTTP/2");
        assert_eq!(entry["request"]["cookies"][0]["name"], "session");
        assert_eq!(entry["request"]["cookies"][1]["name"], "theme");
        assert_eq!(entry["request"]["headersSize"], 17);
        assert_eq!(entry["request"]["bodySize"], 3);
        assert_eq!(entry["response"]["cookies"][0]["domain"], "example.test");
        assert_eq!(entry["response"]["cookies"][0]["httpOnly"], true);
        assert_eq!(entry["response"]["cookies"][0]["secure"], true);
        assert_eq!(entry["response"]["cookies"][0]["sameSite"], "Lax");
        assert_eq!(
            entry["response"]["cookies"][0]["expires"],
            "2015-10-21T07:28:00.000Z"
        );
        assert_eq!(entry["response"]["content"]["size"], 10);
        assert_eq!(entry["response"]["content"]["compression"], 4);
        assert_eq!(entry["response"]["headersSize"], 23);
        assert_eq!(entry["response"]["bodySize"], 6);
        assert_eq!(entry["response"]["_transferSize"], 42);
        assert_eq!(entry["serverIPAddress"], "203.0.113.7");
        assert_eq!(entry["_serverPort"], 443);
        assert_eq!(entry["_securityDetails"]["protocol"], "TLS 1.3");
        assert_eq!(entry["_securityDetails"]["validFrom"], 1_700_000_000.0);
    }

    #[test]
    fn page_lifecycle_updates_keep_creation_time_and_export_relative_timings() {
        // Arrange
        let started = 1_700_000_000_000_000;
        let mut recorder = HarRecorder::new(HarRecorderOptions {
            mode: HarMode::Full,
            ..HarRecorderOptions::default()
        });
        recorder.record_page(HarPageSnapshot {
            id: "page-1".to_owned(),
            started_time_micros: started,
            title: String::new(),
            on_content_load_micros: None,
            on_load_micros: None,
        });

        // Act
        recorder.update_page_lifecycle(
            "page-1",
            "Ready".to_owned(),
            Some(started + 1_250_000),
            Some(started + 2_500_000),
        );
        let document: Value = serde_json::from_slice(&recorder.export().unwrap().json).unwrap();
        let page = &document["log"]["pages"][0];

        // Assert
        assert_eq!(page["startedDateTime"], "2023-11-14T22:13:20.000Z");
        assert_eq!(page["title"], "Ready");
        assert_eq!(page["pageTimings"]["onContentLoad"], 1250.0);
        assert_eq!(page["pageTimings"]["onLoad"], 2500.0);
    }

    #[test]
    fn redirect_target_updates_predecessor_redirect_url() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions::default());
        recorder
            .request_started(request("request-1", None))
            .unwrap();
        recorder
            .response_received("request-1", response(None))
            .unwrap();
        let mut redirected = request("request-2", None);
        redirected.url = "https://example.test/final".to_owned();
        redirected.redirected_from = Some("request-1".to_owned());

        // Act
        recorder.request_started(redirected).unwrap();
        let document: Value = serde_json::from_slice(&recorder.export().unwrap().json).unwrap();

        // Assert
        assert_eq!(
            document["log"]["entries"][0]["response"]["redirectURL"],
            "https://example.test/final"
        );
    }

    #[test]
    fn response_location_is_exported_without_a_followup_request() {
        // Arrange
        let mut recorder = HarRecorder::new(HarRecorderOptions::default());
        recorder
            .request_started(request("request-1", None))
            .unwrap();
        let mut redirect = response(None);
        redirect.status = 302;
        redirect.headers = vec![HarHeader::new("Location", "/login")];

        // Act
        recorder
            .request_finished("request-1", Some(redirect), None)
            .unwrap();
        let document: Value = serde_json::from_slice(&recorder.export().unwrap().json).unwrap();

        // Assert
        assert_eq!(
            document["log"]["entries"][0]["response"]["redirectURL"],
            "/login"
        );
    }
}
