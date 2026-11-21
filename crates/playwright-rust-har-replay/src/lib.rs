//! Backend-neutral HAR replay policy with opt-in resource limits.
//!
//! This crate parses an already acquired plain HAR byte slice and matches
//! requests to replay effects. It does not read files, open ZIP archives,
//! intercept browser traffic, send protocol commands, own a runtime, or call
//! a language binding.

use std::sync::Arc;

use base64::Engine as _;
use serde::Deserialize;
use thiserror::Error;
use url::Url;

const REDIRECT_STATUSES: [i64; 5] = [301, 302, 303, 307, 308];
type Header = (Box<str>, Box<str>);
type Headers = Arc<[Header]>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayLimits {
    pub archive_bytes: usize,
    pub entries: usize,
    pub url_bytes: usize,
    pub method_bytes: usize,
    pub headers_per_message: usize,
    pub header_bytes_per_message: usize,
    pub post_data_bytes: usize,
    pub response_body_bytes: usize,
    pub retained_bytes: usize,
    pub redirects: usize,
}

impl Default for ReplayLimits {
    fn default() -> Self {
        Self {
            archive_bytes: usize::MAX,
            entries: usize::MAX,
            url_bytes: usize::MAX,
            method_bytes: usize::MAX,
            headers_per_message: usize::MAX,
            header_bytes_per_message: usize::MAX,
            post_data_bytes: usize::MAX,
            response_body_bytes: usize::MAX,
            retained_bytes: usize::MAX,
            redirects: usize::MAX,
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ReplayError {
    #[error("HAR input exceeds the {limit}-byte archive limit")]
    ArchiveTooLarge { limit: usize },
    #[error("HAR JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("HAR contains {actual} entries, exceeding the {limit}-entry limit")]
    TooManyEntries { actual: usize, limit: usize },
    #[error("HAR entry {entry} {field} exceeds its {limit}-byte limit")]
    FieldTooLarge {
        entry: usize,
        field: &'static str,
        limit: usize,
    },
    #[error("HAR entry {entry} contains too many {field}")]
    TooManyHeaders { entry: usize, field: &'static str },
    #[error("HAR entry {entry} {field} headers exceed their byte limit")]
    HeadersTooLarge { entry: usize, field: &'static str },
    #[error(
        "HAR entry {entry} uses external {field} content, which requires a separate content-source plugin"
    )]
    ExternalContentUnsupported { entry: usize, field: &'static str },
    #[error("HAR entry {entry} could not read external {field} content {file:?}: {message}")]
    ExternalContentReadFailed {
        entry: usize,
        field: &'static str,
        file: String,
        message: String,
    },
    #[error("HAR entry {entry} contains invalid base64 {field} content")]
    InvalidBase64 { entry: usize, field: &'static str },
    #[error("HAR retained representation exceeds the {limit}-byte limit")]
    RetainedBytesExceeded { limit: usize },
    #[error("HAR redirect target is invalid: {0}")]
    InvalidRedirect(String),
    #[error("HAR redirect chain exceeds the {limit}-redirect limit")]
    TooManyRedirects { limit: usize },
    #[error("HAR redirect cycle detected for {url}")]
    RedirectCycle { url: String },
}

/// Backend supplied by the capability-owning layer for HAR `_file` content.
///
/// The replay policy deliberately owns neither filesystem paths nor ZIP
/// readers. The caller must apply containment before returning bytes. `limit`
/// is the maximum decoded byte count accepted for this field; the default
/// policy passes `usize::MAX`, which is an unlimited sentinel and must never be
/// used as a reservation size.
pub trait ReplayContentSource: Send + Sync {
    fn read(&self, file: &str, limit: usize) -> Result<Vec<u8>, String>;
}

impl<F> ReplayContentSource for F
where
    F: Fn(&str, usize) -> Result<Vec<u8>, String> + Send + Sync,
{
    fn read(&self, file: &str, limit: usize) -> Result<Vec<u8>, String> {
        self(file, limit)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayResponse {
    status: i64,
    headers: Headers,
    body: Arc<[u8]>,
}

impl ReplayResponse {
    pub const fn status(&self) -> i64 {
        self.status
    }

    pub fn headers(&self) -> &[(Box<str>, Box<str>)] {
        &self.headers
    }

    pub fn body(&self) -> Arc<[u8]> {
        Arc::clone(&self.body)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayAction {
    Fulfill(ReplayResponse),
    Stall,
    NoEntry,
}

#[derive(Debug)]
struct Entry {
    request_url: Box<str>,
    request_method: Box<str>,
    request_headers: Headers,
    request_post_data: Option<RawContent>,
    response_status: i64,
    response_headers: Headers,
    response_content: RawContent,
}

#[derive(Clone)]
pub struct ReplayArchive {
    entries: Arc<[Entry]>,
    limits: ReplayLimits,
    retained_bytes: usize,
    content_source: Option<Arc<dyn ReplayContentSource>>,
}

impl std::fmt::Debug for ReplayArchive {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReplayArchive")
            .field("entries", &self.entries)
            .field("limits", &self.limits)
            .field("retained_bytes", &self.retained_bytes)
            .field("has_content_source", &self.content_source.is_some())
            .finish()
    }
}

impl ReplayArchive {
    pub fn from_json(bytes: &[u8], limits: ReplayLimits) -> Result<Self, ReplayError> {
        Self::parse(bytes, limits, None)
    }

    pub fn from_json_with_content_source<S>(
        bytes: &[u8],
        limits: ReplayLimits,
        source: S,
    ) -> Result<Self, ReplayError>
    where
        S: ReplayContentSource + 'static,
    {
        Self::parse(bytes, limits, Some(Arc::new(source)))
    }

    fn parse(
        bytes: &[u8],
        limits: ReplayLimits,
        content_source: Option<Arc<dyn ReplayContentSource>>,
    ) -> Result<Self, ReplayError> {
        if bytes.len() > limits.archive_bytes {
            return Err(ReplayError::ArchiveTooLarge {
                limit: limits.archive_bytes,
            });
        }
        let document: RawDocument = serde_json::from_slice(bytes)
            .map_err(|error| ReplayError::InvalidJson(error.to_string()))?;
        if document.log.entries.len() > limits.entries {
            return Err(ReplayError::TooManyEntries {
                actual: document.log.entries.len(),
                limit: limits.entries,
            });
        }

        let mut retained_bytes = 0usize;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(document.log.entries.len())
            .map_err(|_| ReplayError::RetainedBytesExceeded {
                limit: limits.retained_bytes,
            })?;
        for (index, raw) in document.log.entries.into_iter().enumerate() {
            check_field(index, "request URL", &raw.request.url, limits.url_bytes)?;
            check_field(
                index,
                "request method",
                &raw.request.method,
                limits.method_bytes,
            )?;
            let request_headers = convert_headers(
                index,
                "request",
                raw.request.headers,
                limits.headers_per_message,
                limits.header_bytes_per_message,
            )?;
            let response_headers = convert_headers(
                index,
                "response",
                raw.response.headers,
                limits.headers_per_message,
                limits.header_bytes_per_message,
            )?;
            let request_post_data = raw.request.post_data;
            let response_content = raw.response.content;
            let request_content_bytes = request_post_data
                .as_ref()
                .map_or(Some(0), retained_content_bytes)
                .ok_or(ReplayError::RetainedBytesExceeded {
                    limit: limits.retained_bytes,
                })?;
            let response_content_bytes = retained_content_bytes(&response_content).ok_or(
                ReplayError::RetainedBytesExceeded {
                    limit: limits.retained_bytes,
                },
            )?;
            retained_bytes = retained_bytes
                .checked_add(std::mem::size_of::<Entry>())
                .and_then(|total| total.checked_add(raw.request.url.len()))
                .and_then(|total| total.checked_add(raw.request.method.len()))
                .and_then(|total| total.checked_add(header_bytes(&request_headers)))
                .and_then(|total| total.checked_add(header_bytes(&response_headers)))
                .and_then(|total| total.checked_add(request_content_bytes))
                .and_then(|total| total.checked_add(response_content_bytes))
                .ok_or(ReplayError::RetainedBytesExceeded {
                    limit: limits.retained_bytes,
                })?;
            if retained_bytes > limits.retained_bytes {
                return Err(ReplayError::RetainedBytesExceeded {
                    limit: limits.retained_bytes,
                });
            }
            entries.push(Entry {
                request_url: raw.request.url.into_boxed_str(),
                request_method: raw.request.method.into_boxed_str(),
                request_headers,
                request_post_data,
                response_status: raw.response.status,
                response_headers,
                response_content,
            });
        }
        Ok(Self {
            entries: entries.into(),
            limits,
            retained_bytes,
            content_source,
        })
    }

    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub fn lookup(
        &self,
        url: &str,
        method: &str,
        headers: &[(String, String)],
        post_data: Option<&[u8]>,
        is_navigation_request: bool,
    ) -> Result<ReplayAction, ReplayError> {
        let mut current_url = url.to_owned();
        let mut current_method = method.to_owned();
        let mut visited = Vec::new();
        let redirect_capacity = self
            .entries
            .len()
            .min(self.limits.redirects.saturating_add(1));
        visited.try_reserve_exact(redirect_capacity).map_err(|_| {
            ReplayError::RetainedBytesExceeded {
                limit: self.limits.retained_bytes,
            }
        })?;

        loop {
            let selected = self.find_response(&current_url, &current_method, headers, post_data)?;
            let Some(index) = selected else {
                return Ok(ReplayAction::NoEntry);
            };
            if visited.contains(&index) {
                return Err(ReplayError::RedirectCycle { url: current_url });
            }
            visited.push(index);
            if visited.len() > self.limits.redirects.saturating_add(1) {
                return Err(ReplayError::TooManyRedirects {
                    limit: self.limits.redirects,
                });
            }
            let entry = &self.entries[index];
            if REDIRECT_STATUSES.contains(&entry.response_status)
                && let Some(location) = header_value(&entry.response_headers, "location")
            {
                let parsed = Url::parse(&current_url)
                    .and_then(|base| base.join(location))
                    .map_err(|error| ReplayError::InvalidRedirect(error.to_string()))?;
                if is_navigation_request {
                    let body = decode_content(
                        index,
                        "response",
                        &entry.response_content,
                        self.limits.response_body_bytes,
                        self.content_source.as_deref(),
                    )?;
                    return Ok(ReplayAction::Fulfill(ReplayResponse {
                        status: entry.response_status,
                        headers: Arc::clone(&entry.response_headers),
                        body: body.into(),
                    }));
                }
                current_url = parsed.to_string();
                if ((entry.response_status == 301 || entry.response_status == 302)
                    && current_method == "POST")
                    || (entry.response_status == 303
                        && current_method != "GET"
                        && current_method != "HEAD")
                {
                    current_method = "GET".to_owned();
                }
                continue;
            }
            if entry.response_status == -1 {
                return Ok(ReplayAction::Stall);
            }
            let body = decode_content(
                index,
                "response",
                &entry.response_content,
                self.limits.response_body_bytes,
                self.content_source.as_deref(),
            )?;
            return Ok(ReplayAction::Fulfill(ReplayResponse {
                status: entry.response_status,
                headers: Arc::clone(&entry.response_headers),
                body: body.into(),
            }));
        }
    }

    fn find_response(
        &self,
        url: &str,
        method: &str,
        headers: &[(String, String)],
        post_data: Option<&[u8]>,
    ) -> Result<Option<usize>, ReplayError> {
        let mut selected = None;
        let mut selected_matches = 0usize;
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.request_url.as_ref() != url
                || entry.request_method.as_ref() != method
                || !post_data_matches(
                    index,
                    entry,
                    headers,
                    post_data,
                    self.limits.post_data_bytes,
                    self.content_source.as_deref(),
                )?
            {
                continue;
            }
            let matches = count_matching_headers(&entry.request_headers, headers);
            if selected.is_none() || matches > selected_matches {
                selected = Some(index);
                selected_matches = matches;
            }
        }
        Ok(selected)
    }
}

fn check_field(
    entry: usize,
    field: &'static str,
    value: &str,
    limit: usize,
) -> Result<(), ReplayError> {
    if value.len() > limit {
        return Err(ReplayError::FieldTooLarge {
            entry,
            field,
            limit,
        });
    }
    Ok(())
}

fn convert_headers(
    entry: usize,
    field: &'static str,
    headers: Vec<RawHeader>,
    count_limit: usize,
    byte_limit: usize,
) -> Result<Headers, ReplayError> {
    if headers.len() > count_limit {
        return Err(ReplayError::TooManyHeaders { entry, field });
    }
    let mut bytes = 0usize;
    let mut converted = Vec::new();
    converted
        .try_reserve_exact(headers.len())
        .map_err(|_| ReplayError::HeadersTooLarge { entry, field })?;
    for header in headers {
        bytes = bytes
            .checked_add(header.name.len())
            .and_then(|total| total.checked_add(header.value.len()))
            .ok_or(ReplayError::HeadersTooLarge { entry, field })?;
        if bytes > byte_limit {
            return Err(ReplayError::HeadersTooLarge { entry, field });
        }
        converted.push((header.name.into_boxed_str(), header.value.into_boxed_str()));
    }
    Ok(converted.into())
}

fn decode_content(
    entry: usize,
    field: &'static str,
    content: &RawContent,
    byte_limit: usize,
    source: Option<&dyn ReplayContentSource>,
) -> Result<Vec<u8>, ReplayError> {
    let bytes = if let Some(file) = &content.file {
        let Some(source) = source else {
            return Err(ReplayError::ExternalContentUnsupported { entry, field });
        };
        source
            .read(file, byte_limit)
            .map_err(|message| ReplayError::ExternalContentReadFailed {
                entry,
                field,
                file: file.clone(),
                message,
            })?
    } else {
        let text = content.text.as_deref().unwrap_or_default();
        if content.encoding.as_deref() == Some("base64") {
            base64::engine::general_purpose::STANDARD
                .decode(text)
                .map_err(|_| ReplayError::InvalidBase64 { entry, field })?
        } else {
            text.as_bytes().to_vec()
        }
    };
    if bytes.len() > byte_limit {
        return Err(ReplayError::FieldTooLarge {
            entry,
            field,
            limit: byte_limit,
        });
    }
    Ok(bytes)
}

fn retained_content_bytes(content: &RawContent) -> Option<usize> {
    std::mem::size_of::<RawContent>()
        .checked_add(content.text.as_ref().map_or(0, String::len))?
        .checked_add(content.encoding.as_ref().map_or(0, String::len))?
        .checked_add(content.file.as_ref().map_or(0, String::len))
}

fn header_bytes(headers: &[(Box<str>, Box<str>)]) -> usize {
    headers.iter().fold(0usize, |total, (name, value)| {
        total.saturating_add(name.len()).saturating_add(value.len())
    })
}

fn header_value<'a>(headers: &'a [(Box<str>, Box<str>)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_ref())
}

fn count_matching_headers(expected: &[(Box<str>, Box<str>)], actual: &[(String, String)]) -> usize {
    expected
        .iter()
        .filter(|(expected_name, expected_value)| {
            actual.iter().any(|(actual_name, actual_value)| {
                expected_name.eq_ignore_ascii_case(actual_name)
                    && expected_value.as_ref() == actual_value
            })
        })
        .count()
}

fn post_data_matches(
    index: usize,
    entry: &Entry,
    headers: &[(String, String)],
    post_data: Option<&[u8]>,
    byte_limit: usize,
    source: Option<&dyn ReplayContentSource>,
) -> Result<bool, ReplayError> {
    if entry.request_method.as_ref() != "POST" {
        return Ok(true);
    }
    let (Some(actual), Some(expected_content)) = (post_data, entry.request_post_data.as_ref())
    else {
        return Ok(true);
    };
    let expected = decode_content(index, "request", expected_content, byte_limit, source)?;
    if actual == expected {
        return Ok(true);
    }
    let Some(actual_boundary) = multipart_boundary(
        headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
    ) else {
        return Ok(false);
    };
    let Some(expected_boundary) = multipart_boundary(
        entry
            .request_headers
            .iter()
            .map(|(name, value)| (name.as_ref(), value.as_ref())),
    ) else {
        return Ok(false);
    };
    Ok(String::from_utf8_lossy(actual).replace(actual_boundary, "")
        == String::from_utf8_lossy(&expected).replace(expected_boundary, ""))
}

fn multipart_boundary<'a>(headers: impl Iterator<Item = (&'a str, &'a str)>) -> Option<&'a str> {
    let content_type = headers
        .into_iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))?
        .1;
    if !content_type.contains("multipart/form-data") {
        return None;
    }
    content_type
        .split("boundary=")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
}

#[derive(Deserialize)]
struct RawDocument {
    log: RawLog,
}

#[derive(Deserialize)]
struct RawLog {
    #[serde(default)]
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    request: RawRequest,
    response: RawResponse,
}

#[derive(Deserialize)]
struct RawRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<RawHeader>,
    #[serde(default, rename = "postData")]
    post_data: Option<RawContent>,
}

#[derive(Deserialize)]
struct RawResponse {
    status: i64,
    #[serde(default)]
    headers: Vec<RawHeader>,
    content: RawContent,
}

#[derive(Deserialize)]
struct RawHeader {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct RawContent {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default, rename = "_file")]
    file: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{ReplayAction, ReplayArchive, ReplayError, ReplayLimits};

    fn archive(entries: &str) -> ReplayArchive {
        ReplayArchive::from_json(
            format!(r#"{{"log":{{"entries":[{entries}]}}}}"#).as_bytes(),
            ReplayLimits::default(),
        )
        .unwrap()
    }

    fn entry(method: &str, url: &str, status: i64, headers: &str, body: &str) -> String {
        format!(
            r#"{{"request":{{"method":"{method}","url":"{url}","headers":[]}},"response":{{"status":{status},"headers":[{headers}],"content":{{"text":"{body}"}}}}}}"#
        )
    }

    #[test]
    fn matches_exact_request_and_fulfills_embedded_body() {
        // Arrange
        let replay = archive(&entry(
            "GET",
            "http://example.test/",
            200,
            r#"{"name":"content-type","value":"text/plain"}"#,
            "hello",
        ));

        // Act
        let action = replay
            .lookup("http://example.test/", "GET", &[], None, true)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected a fulfilled HAR response");
        };
        assert_eq!(response.status(), 200);
        assert_eq!(response.body().as_ref(), b"hello");
    }

    #[test]
    fn follows_recorded_redirect_before_fulfilling_subresource() {
        // Arrange
        let redirect = entry(
            "GET",
            "http://example.test/script.js",
            301,
            r#"{"name":"location","value":"script2.js"}"#,
            "",
        );
        let target = entry(
            "GET",
            "http://example.test/script2.js",
            200,
            "",
            "window.value = 'foo'",
        );
        let replay = archive(&format!("{redirect},{target}"));

        // Act
        let action = replay
            .lookup("http://example.test/script.js", "GET", &[], None, false)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected redirect resolution to fulfill the final response");
        };
        assert_eq!(response.body().as_ref(), b"window.value = 'foo'");
    }

    #[test]
    fn fulfills_navigation_redirect_for_chromium_to_follow() {
        // Arrange
        let redirect = entry(
            "GET",
            "https://example.test/",
            301,
            r#"{"name":"location","value":"https://www.example.test/"}"#,
            "",
        );
        let target = entry("GET", "https://www.example.test/", 200, "", "final body");
        let replay = archive(&format!("{redirect},{target}"));

        // Act
        let action = replay
            .lookup("https://example.test/", "GET", &[], None, true)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected the recorded redirect response to be fulfilled");
        };
        assert_eq!(response.status(), 301);
        assert_eq!(
            response.headers()[0],
            ("location".into(), "https://www.example.test/".into())
        );
    }

    #[test]
    fn missing_entries_is_an_empty_archive_for_not_found_policy() {
        // Arrange
        let json = br#"{"log":{}}"#;

        // Act
        let replay = ReplayArchive::from_json(json, ReplayLimits::default()).unwrap();
        let action = replay
            .lookup("http://example.test/", "GET", &[], None, true)
            .unwrap();

        // Assert
        assert_eq!(action, ReplayAction::NoEntry);
        assert_eq!(replay.retained_bytes(), 0);
    }

    #[test]
    fn default_limits_match_upstream_unlimited_replay_policy() {
        // Arrange
        let unlimited = usize::MAX;

        // Act
        let limits = ReplayLimits::default();

        // Assert
        assert_eq!(limits.archive_bytes, unlimited);
        assert_eq!(limits.entries, unlimited);
        assert_eq!(limits.url_bytes, unlimited);
        assert_eq!(limits.method_bytes, unlimited);
        assert_eq!(limits.headers_per_message, unlimited);
        assert_eq!(limits.header_bytes_per_message, unlimited);
        assert_eq!(limits.post_data_bytes, unlimited);
        assert_eq!(limits.response_body_bytes, unlimited);
        assert_eq!(limits.retained_bytes, unlimited);
        assert_eq!(limits.redirects, unlimited);
    }

    #[test]
    fn default_replay_accepts_body_larger_than_legacy_one_megabyte_cap() {
        // Arrange
        let body = "x".repeat(1024 * 1024 + 1);
        let json = format!(
            r#"{{"log":{{"entries":[{}]}}}}"#,
            entry("GET", "http://example.test/large", 200, "", &body)
        );

        // Act
        let replay = ReplayArchive::from_json(json.as_bytes(), ReplayLimits::default()).unwrap();
        let action = replay
            .lookup("http://example.test/large", "GET", &[], None, false)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected the large HAR response to be fulfilled");
        };
        assert_eq!(response.body().len(), body.len());
    }

    #[test]
    fn default_replay_follows_more_than_legacy_twenty_redirect_cap() {
        // Arrange
        let mut entries = Vec::new();
        for index in 0..21 {
            entries.push(entry(
                "GET",
                &format!("http://example.test/{index}"),
                301,
                &format!(r#"{{"name":"location","value":"/{}"}}"#, index + 1),
                "",
            ));
        }
        entries.push(entry(
            "GET",
            "http://example.test/21",
            200,
            "",
            "final body",
        ));
        let replay = archive(&entries.join(","));

        // Act
        let action = replay
            .lookup("http://example.test/0", "GET", &[], None, false)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected the redirect chain to reach its final response");
        };
        assert_eq!(response.body().as_ref(), b"final body");
    }

    #[test]
    fn response_status_is_deferred_to_route_fulfillment_like_upstream() {
        // Arrange
        let replay = archive(&entry(
            "GET",
            "http://example.test/status-zero",
            0,
            "",
            "body",
        ));

        // Act
        let action = replay
            .lookup("http://example.test/status-zero", "GET", &[], None, false)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected HAR lookup to return the recorded response");
        };
        assert_eq!(response.status(), 0);
    }

    #[test]
    fn rejects_external_content_without_content_source_plugin() {
        // Arrange
        let json = br#"{"log":{"entries":[{"request":{"method":"GET","url":"http://example.test/","headers":[]},"response":{"status":200,"headers":[],"content":{"_file":"body.txt"}}}]}}"#;
        let replay = ReplayArchive::from_json(json, ReplayLimits::default()).unwrap();

        // Act
        let result = replay.lookup("http://example.test/", "GET", &[], None, true);

        // Assert
        assert_eq!(
            result.unwrap_err(),
            ReplayError::ExternalContentUnsupported {
                entry: 0,
                field: "response",
            }
        );
    }

    #[test]
    fn external_content_is_loaded_only_after_matching_lookup() {
        // Arrange
        let json = br#"{"log":{"entries":[{"request":{"method":"GET","url":"http://example.test/","headers":[]},"response":{"status":200,"headers":[],"content":{"_file":"body.txt"}}}]}}"#;
        let reads = Arc::new(AtomicUsize::new(0));
        let source_reads = Arc::clone(&reads);
        let source = move |file: &str, limit: usize| {
            assert_eq!(file, "body.txt");
            assert_eq!(limit, ReplayLimits::default().response_body_bytes);
            source_reads.fetch_add(1, Ordering::SeqCst);
            Ok(b"attached body".to_vec())
        };
        let replay =
            ReplayArchive::from_json_with_content_source(json, ReplayLimits::default(), source)
                .unwrap();
        assert_eq!(reads.load(Ordering::SeqCst), 0);

        // Act
        let action = replay
            .lookup("http://example.test/", "GET", &[], None, true)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected attached response to be fulfilled");
        };
        assert_eq!(response.body().as_ref(), b"attached body");
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unused_external_content_does_not_break_another_matching_entry() {
        // Arrange
        let missing = r#"{"request":{"method":"GET","url":"http://example.test/missing","headers":[]},"response":{"status":200,"headers":[],"content":{"_file":"missing.txt"}}}"#;
        let valid = entry("GET", "http://example.test/valid", 200, "", "valid body");
        let replay = archive(&format!("{missing},{valid}"));

        // Act
        let action = replay
            .lookup("http://example.test/valid", "GET", &[], None, false)
            .unwrap();

        // Assert
        let ReplayAction::Fulfill(response) = action else {
            panic!("expected the independent valid entry to fulfill");
        };
        assert_eq!(response.body().as_ref(), b"valid body");
    }

    #[test]
    fn external_content_is_still_checked_against_field_limit() {
        // Arrange
        let json = br#"{"log":{"entries":[{"request":{"method":"GET","url":"http://example.test/","headers":[]},"response":{"status":200,"headers":[],"content":{"_file":"body.bin"}}}]}}"#;
        let limits = ReplayLimits {
            response_body_bytes: 3,
            ..ReplayLimits::default()
        };
        let source = |_file: &str, _limit: usize| Ok(vec![0; 4]);
        let replay = ReplayArchive::from_json_with_content_source(json, limits, source).unwrap();

        // Act
        let result = replay.lookup("http://example.test/", "GET", &[], None, true);

        // Assert
        assert_eq!(
            result.unwrap_err(),
            ReplayError::FieldTooLarge {
                entry: 0,
                field: "response",
                limit: 3,
            }
        );
    }

    #[test]
    fn rejects_archive_before_parsing_when_raw_limit_is_exceeded() {
        // Arrange
        let limits = ReplayLimits {
            archive_bytes: 3,
            ..ReplayLimits::default()
        };

        // Act
        let result = ReplayArchive::from_json(b"{}  ", limits);

        // Assert
        assert_eq!(
            result.unwrap_err(),
            ReplayError::ArchiveTooLarge { limit: 3 }
        );
    }
}
