//! High-level, attach-only Chromium CDP facade.
//!
//! The attached browser never owns the remote Chrome process. Close retires
//! only BrowserContexts created through this facade, disconnects the borrowed
//! CDP transport, and then invokes an optional external lease hook exactly
//! once. No BCP-specific protocol or dependency lives in this crate.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use playwright_rust_node_host::evaluation::EvaluateParams;
pub use playwright_rust_node_host::evaluation::EvaluationValue;
pub use playwright_rust_node_host::locator::LocatorWaitState;
use playwright_rust_node_host::locator::action::LocatorAction;
pub use playwright_rust_node_host::locator::action::{ClickOptionsV2, FillOptions};
use playwright_rust_node_host::locator::{HostLocator, LocatorRead, LocatorValue};
use playwright_rust_node_host::locator_query::{QueryBudget, QueryLimits};
pub use playwright_rust_node_host::screenshot::{
    ScreenshotFormat, ScreenshotOptions, ScreenshotOutput,
};
use playwright_rust_node_host::{HostError, HostSession, HostSessionCloseTimeouts};

#[cfg(feature = "cdp-handshake-headers")]
pub use playwright_rust_cdp::{CdpHandshakeHeaders, CdpHandshakeHeadersError};

pub type RemoteLeaseFuture =
    Pin<Box<dyn Future<Output = Result<(), RemoteLeaseError>> + Send + 'static>>;

/// External lease authority supplied by an adapter such as `bcp-playwright`.
///
/// The facade calls `release` at most once, after local target cleanup and
/// transport shutdown have been attempted. Implementations should make the
/// remote release idempotent because process or network failure is always
/// possible outside this client's ownership boundary.
pub trait RemoteLease: Send + Sync + 'static {
    fn release(&self) -> RemoteLeaseFuture;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteLeaseError(Arc<str>);

impl RemoteLeaseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(Arc::from(message.into()))
    }
}

impl Display for RemoteLeaseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RemoteLeaseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteCdpError {
    InvalidOptions(Arc<str>),
    ConnectTimeout(Duration),
    Disconnected,
    Operation(Arc<str>),
    Lease(Arc<str>),
    Close(Arc<str>),
}

impl Display for RemoteCdpError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(message) => {
                write!(formatter, "invalid remote CDP options: {message}")
            }
            Self::ConnectTimeout(timeout) => {
                write!(
                    formatter,
                    "remote CDP connection timed out after {timeout:?}"
                )
            }
            Self::Disconnected => formatter.write_str("remote CDP transport disconnected"),
            Self::Operation(message) => write!(formatter, "remote CDP operation failed: {message}"),
            Self::Lease(message) => write!(formatter, "remote lease release failed: {message}"),
            Self::Close(message) => write!(formatter, "remote CDP close failed: {message}"),
        }
    }
}

impl Error for RemoteCdpError {}

fn host_error(error: HostError) -> RemoteCdpError {
    if matches!(error, HostError::TargetClosed) {
        RemoteCdpError::Disconnected
    } else {
        RemoteCdpError::Operation(Arc::from(error.to_string()))
    }
}

fn timeout_ms(value: Duration, field: &'static str) -> Result<u32, RemoteCdpError> {
    if value.is_zero() {
        return Err(RemoteCdpError::InvalidOptions(Arc::from(format!(
            "{field} must be nonzero"
        ))));
    }
    u32::try_from(value.as_millis()).map_err(|_| {
        RemoteCdpError::InvalidOptions(Arc::from(format!("{field} exceeds u32 milliseconds")))
    })
}

pub struct RemoteCdpConnectOptions {
    pub connect_timeout: Duration,
    pub command_timeout: Duration,
    pub operation_timeout: Duration,
    pub close_timeouts: HostSessionCloseTimeouts,
    pub lease_release_timeout: Duration,
    pub remote_lease: Option<Arc<dyn RemoteLease>>,
    #[cfg(feature = "cdp-handshake-headers")]
    pub headers: CdpHandshakeHeaders,
}

impl Default for RemoteCdpConnectOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(30),
            command_timeout: Duration::from_secs(30),
            operation_timeout: Duration::from_secs(30),
            close_timeouts: HostSessionCloseTimeouts::default(),
            lease_release_timeout: Duration::from_secs(10),
            remote_lease: None,
            #[cfg(feature = "cdp-handshake-headers")]
            headers: CdpHandshakeHeaders::default(),
        }
    }
}

impl RemoteCdpConnectOptions {
    #[must_use]
    pub fn with_remote_lease(mut self, lease: Arc<dyn RemoteLease>) -> Self {
        self.remote_lease = Some(lease);
        self
    }

    #[cfg(feature = "cdp-handshake-headers")]
    #[must_use]
    pub fn with_headers(mut self, headers: CdpHandshakeHeaders) -> Self {
        self.headers = headers;
        self
    }

    fn validate(&self) -> Result<u32, RemoteCdpError> {
        timeout_ms(self.connect_timeout, "connect_timeout")?;
        let command_timeout = timeout_ms(self.command_timeout, "command_timeout")?;
        timeout_ms(self.operation_timeout, "operation_timeout")?;
        timeout_ms(self.lease_release_timeout, "lease_release_timeout")?;
        if self.close_timeouts.graceful.is_zero() || self.close_timeouts.forced.is_zero() {
            return Err(RemoteCdpError::InvalidOptions(Arc::from(
                "close timeouts must be nonzero",
            )));
        }
        Ok(command_timeout)
    }
}

struct LeaseSlot(Mutex<Option<Arc<dyn RemoteLease>>>);

impl LeaseSlot {
    fn new(lease: Option<Arc<dyn RemoteLease>>) -> Self {
        Self(Mutex::new(lease))
    }

    fn take(&self) -> Option<Arc<dyn RemoteLease>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

struct BrowserInner {
    host: Arc<HostSession>,
    operation_timeout: Duration,
    close_timeouts: HostSessionCloseTimeouts,
    lease_release_timeout: Duration,
    lease: LeaseSlot,
    owned_contexts: Mutex<BTreeSet<String>>,
    closing: AtomicBool,
    closed: AtomicBool,
    close_gate: tokio::sync::Mutex<()>,
}

impl BrowserInner {
    fn ensure_open(&self) -> Result<(), RemoteCdpError> {
        if self.closing.load(Ordering::Acquire)
            || self.closed.load(Ordering::Acquire)
            || !self.host.is_connected()
        {
            return Err(RemoteCdpError::Disconnected);
        }
        Ok(())
    }

    fn operation_timeout_ms(&self) -> Result<u32, RemoteCdpError> {
        timeout_ms(self.operation_timeout, "operation_timeout")
    }

    fn remember_context(&self, context: &str) {
        self.owned_contexts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(context.to_owned());
    }

    fn forget_context(&self, context: &str) {
        self.owned_contexts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(context);
    }
}

#[derive(Clone)]
pub struct Browser(Arc<BrowserInner>);

/// Attach to an externally owned Chromium CDP endpoint.
pub async fn connect_over_cdp(
    ws_url: impl AsRef<str>,
    options: RemoteCdpConnectOptions,
) -> Result<Browser, RemoteCdpError> {
    let command_timeout = options.validate()?;
    let connect = async {
        #[cfg(feature = "cdp-handshake-headers")]
        {
            let headers = options
                .headers
                .iter()
                .map(|(name, value)| {
                    value
                        .to_str()
                        .map(|value| (name.as_str().to_owned(), value.to_owned()))
                        .map_err(|_| {
                            RemoteCdpError::InvalidOptions(Arc::from(
                                "handshake header contains non-text bytes",
                            ))
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            HostSession::connect_over_cdp_with_headers(ws_url.as_ref(), command_timeout, headers)
                .await
                .map_err(host_error)
        }
        #[cfg(not(feature = "cdp-handshake-headers"))]
        {
            HostSession::connect_over_cdp(ws_url.as_ref(), command_timeout)
                .await
                .map_err(host_error)
        }
    };
    let host = tokio::time::timeout(options.connect_timeout, connect)
        .await
        .map_err(|_| RemoteCdpError::ConnectTimeout(options.connect_timeout))??;
    Ok(Browser(Arc::new(BrowserInner {
        host: Arc::new(host),
        operation_timeout: options.operation_timeout,
        close_timeouts: options.close_timeouts,
        lease_release_timeout: options.lease_release_timeout,
        lease: LeaseSlot::new(options.remote_lease),
        owned_contexts: Mutex::new(BTreeSet::new()),
        closing: AtomicBool::new(false),
        closed: AtomicBool::new(false),
        close_gate: tokio::sync::Mutex::new(()),
    })))
}

impl Browser {
    pub fn is_connected(&self) -> bool {
        !self.0.closed.load(Ordering::Acquire) && self.0.host.is_connected()
    }

    pub async fn new_context(&self) -> Result<BrowserContext, RemoteCdpError> {
        self.0.ensure_open()?;
        let descriptor = self.0.host.new_context().await.map_err(host_error)?;
        self.0.remember_context(&descriptor.id);
        Ok(BrowserContext {
            browser: Arc::downgrade(&self.0),
            id: Arc::from(descriptor.id),
        })
    }

    pub async fn new_page(&self) -> Result<Page, RemoteCdpError> {
        let context = self.new_context().await?;
        context.new_page_with_context_close(true).await
    }

    pub async fn close(&self) -> Result<(), RemoteCdpError> {
        let _gate = self.0.close_gate.lock().await;
        if self.0.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        self.0.closing.store(true, Ordering::Release);
        let contexts = self
            .0
            .owned_contexts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut failures = Vec::new();
        for context in contexts {
            match tokio::time::timeout(
                self.0.operation_timeout,
                self.0.host.close_context(&context),
            )
            .await
            {
                Ok(Ok(())) | Ok(Err(HostError::TargetClosed)) => self.0.forget_context(&context),
                Ok(Err(error)) => failures.push(format!("Context {context}: {error}")),
                Err(_) => failures.push(format!(
                    "Context {context}: cleanup exceeded {:?}",
                    self.0.operation_timeout
                )),
            }
        }
        if let Err(error) = self.0.host.close_with_timeouts(self.0.close_timeouts).await {
            failures.push(format!("transport: {error}"));
        }
        if let Some(lease) = self.0.lease.take() {
            match tokio::time::timeout(self.0.lease_release_timeout, lease.release()).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => failures.push(format!("lease: {error}")),
                Err(_) => failures.push(format!(
                    "lease: release exceeded {:?}",
                    self.0.lease_release_timeout
                )),
            }
        }
        self.0.closed.store(true, Ordering::Release);
        if failures.is_empty() {
            Ok(())
        } else {
            Err(RemoteCdpError::Close(Arc::from(failures.join("; "))))
        }
    }
}

#[derive(Clone)]
pub struct BrowserContext {
    browser: Weak<BrowserInner>,
    id: Arc<str>,
}

impl BrowserContext {
    pub fn id(&self) -> &str {
        &self.id
    }

    fn browser(&self) -> Result<Arc<BrowserInner>, RemoteCdpError> {
        let browser = self.browser.upgrade().ok_or(RemoteCdpError::Disconnected)?;
        browser.ensure_open()?;
        Ok(browser)
    }

    pub async fn new_page(&self) -> Result<Page, RemoteCdpError> {
        self.new_page_with_context_close(false).await
    }

    async fn new_page_with_context_close(
        &self,
        close_context_on_page_close: bool,
    ) -> Result<Page, RemoteCdpError> {
        let browser = self.browser()?;
        let descriptor = browser.host.new_page(&self.id).await.map_err(host_error)?;
        Ok(Page {
            browser: Arc::downgrade(&browser),
            context: self.clone(),
            id: Arc::from(descriptor.id),
            main_frame_id: Arc::from(descriptor.main_frame_id),
            close_context_on_page_close,
        })
    }

    pub async fn close(&self) -> Result<(), RemoteCdpError> {
        let browser = self.browser.upgrade().ok_or(RemoteCdpError::Disconnected)?;
        match browser.host.close_context(&self.id).await {
            Ok(()) | Err(HostError::TargetClosed) => {
                browser.forget_context(&self.id);
                Ok(())
            }
            Err(error) => Err(host_error(error)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WaitUntil {
    Commit,
    DomContentLoaded,
    #[default]
    Load,
    NetworkIdle,
}

impl WaitUntil {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::DomContentLoaded => "domcontentloaded",
            Self::Load => "load",
            Self::NetworkIdle => "networkidle",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GotoOptions {
    pub wait_until: WaitUntil,
    pub timeout: Duration,
}

impl Default for GotoOptions {
    fn default() -> Self {
        Self {
            wait_until: WaitUntil::Load,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Clone)]
pub struct Page {
    browser: Weak<BrowserInner>,
    context: BrowserContext,
    id: Arc<str>,
    main_frame_id: Arc<str>,
    close_context_on_page_close: bool,
}

impl Page {
    fn browser(&self) -> Result<Arc<BrowserInner>, RemoteCdpError> {
        let browser = self.browser.upgrade().ok_or(RemoteCdpError::Disconnected)?;
        browser.ensure_open()?;
        Ok(browser)
    }

    pub async fn url(&self) -> Result<String, RemoteCdpError> {
        match self.evaluate("location.href").await? {
            EvaluationValue::String(value) | EvaluationValue::Url(value) => Ok(value),
            _ => Err(RemoteCdpError::Operation(Arc::from(
                "Page.url returned an incompatible value",
            ))),
        }
    }

    pub async fn goto(
        &self,
        url: impl Into<String>,
        options: GotoOptions,
    ) -> Result<(), RemoteCdpError> {
        let browser = self.browser()?;
        let timeout = timeout_ms(options.timeout, "goto timeout")?;
        browser
            .host
            .page_goto(
                &self.id,
                url.into(),
                options.wait_until.as_str(),
                None,
                timeout,
            )
            .await
            .map(|_| ())
            .map_err(host_error)
    }

    pub async fn content(&self) -> Result<String, RemoteCdpError> {
        self.browser()?
            .host
            .frame_content(&self.main_frame_id)
            .await
            .map_err(host_error)
    }

    pub async fn evaluate(
        &self,
        expression: impl Into<String>,
    ) -> Result<EvaluationValue, RemoteCdpError> {
        self.browser()?
            .host
            .frame_evaluate(
                &self.main_frame_id,
                EvaluateParams::expression(expression.into()),
            )
            .await
            .map_err(host_error)
    }

    pub fn locator(&self, selector: impl Into<String>) -> Result<Locator, RemoteCdpError> {
        let browser = self.browser()?;
        let mut selector = selector.into();
        selector.shrink_to_fit();
        // This facade owns one independent budget per public Locator recipe.
        // Do not fabricate a global recipe/concurrency ceiling: the parser's
        // intrinsic selector depth/node/string validation remains authoritative,
        // while the budget still accounts exact live storage and temporary
        // recipe replacement during parsing.
        let budget = QueryBudget::new(QueryLimits {
            recipes: usize::MAX,
            bytes: usize::MAX,
        })
        .map_err(host_error)?;
        let query = budget
            .prepare_leaf(selector.len())
            .and_then(|draft| draft.css(selector))
            .map_err(host_error)?;
        let locator = browser
            .host
            .locator(&self.main_frame_id, query)
            .map_err(host_error)?;
        Ok(Locator {
            inner: locator,
            browser: Arc::downgrade(&browser),
        })
    }

    pub async fn screenshot(
        &self,
        options: ScreenshotOptions,
    ) -> Result<ScreenshotOutput, RemoteCdpError> {
        let browser = self.browser()?;
        browser
            .host
            .page_screenshot(&self.id, options, browser.operation_timeout_ms()?)
            .await
            .map_err(host_error)
    }

    pub async fn close(&self) -> Result<(), RemoteCdpError> {
        if self.close_context_on_page_close {
            return self.context.close().await;
        }
        let browser = self.browser.upgrade().ok_or(RemoteCdpError::Disconnected)?;
        match browser.host.close_page(&self.id).await {
            Ok(()) | Err(HostError::TargetClosed) => Ok(()),
            Err(error) => Err(host_error(error)),
        }
    }
}

#[derive(Clone)]
pub struct Locator {
    inner: HostLocator,
    browser: Weak<BrowserInner>,
}

impl Locator {
    fn browser(&self) -> Result<Arc<BrowserInner>, RemoteCdpError> {
        let browser = self.browser.upgrade().ok_or(RemoteCdpError::Disconnected)?;
        browser.ensure_open()?;
        Ok(browser)
    }

    pub async fn wait_for(&self, state: LocatorWaitState) -> Result<(), RemoteCdpError> {
        let browser = self.browser()?;
        self.inner
            .wait_for(state, browser.operation_timeout_ms()?)
            .await
            .map_err(host_error)
    }

    pub async fn click(&self, options: ClickOptionsV2) -> Result<(), RemoteCdpError> {
        let browser = self.browser()?;
        self.inner
            .act(
                LocatorAction::Click(options),
                browser.operation_timeout_ms()?,
            )
            .await
            .map(|_| ())
            .map_err(host_error)
    }

    pub async fn fill(&self, value: impl Into<String>) -> Result<(), RemoteCdpError> {
        let browser = self.browser()?;
        self.inner
            .act(
                LocatorAction::Fill {
                    value: value.into(),
                    options: FillOptions::default(),
                },
                browser.operation_timeout_ms()?,
            )
            .await
            .map(|_| ())
            .map_err(host_error)
    }

    pub async fn text_content(&self) -> Result<Option<String>, RemoteCdpError> {
        let browser = self.browser()?;
        match self
            .inner
            .read(
                LocatorRead::TextContent,
                browser.operation_timeout_ms()?,
                None,
            )
            .await
            .map_err(host_error)?
        {
            LocatorValue::Text(value) => Ok(value),
            _ => Err(RemoteCdpError::Operation(Arc::from(
                "Locator.text_content returned an incompatible value",
            ))),
        }
    }

    pub async fn count(&self) -> Result<u32, RemoteCdpError> {
        match self
            .inner
            .read(LocatorRead::Count, 0, None)
            .await
            .map_err(host_error)?
        {
            LocatorValue::Count(value) => Ok(value),
            _ => Err(RemoteCdpError::Operation(Arc::from(
                "Locator.count returned an incompatible value",
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "cdp-handshake-headers")]
    use futures_util::{SinkExt, StreamExt};
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(feature = "cdp-handshake-headers")]
    use tokio::net::TcpListener;
    #[cfg(feature = "cdp-handshake-headers")]
    use tokio_tungstenite::accept_hdr_async;
    #[cfg(feature = "cdp-handshake-headers")]
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    struct RecordingLease(Arc<AtomicUsize>);

    impl RemoteLease for RecordingLease {
        fn release(&self) -> RemoteLeaseFuture {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn lease_slot_yields_external_authority_exactly_once() {
        // Arrange
        let calls = Arc::new(AtomicUsize::new(0));
        let slot = LeaseSlot::new(Some(Arc::new(RecordingLease(Arc::clone(&calls)))));

        // Act
        let first = slot.take();
        let second = slot.take();

        // Assert
        assert!(first.is_some());
        assert!(second.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn zero_timeouts_are_rejected_before_network_work() {
        // Arrange
        let options = RemoteCdpConnectOptions {
            connect_timeout: Duration::ZERO,
            ..RemoteCdpConnectOptions::default()
        };

        // Act
        let error = options.validate().expect_err("zero timeout must fail");

        // Assert
        assert!(error.to_string().contains("connect_timeout"));
    }

    #[cfg(feature = "cdp-handshake-headers")]
    #[tokio::test]
    #[allow(clippy::result_large_err)] // tungstenite fixes the upgrade callback's Result shape.
    async fn facade_forwards_custom_headers_on_the_websocket_upgrade() {
        // Arrange
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("one-shot listener binds");
        let address = listener.local_addr().expect("listener address");
        let (receipt_tx, receipt_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("client connects");
            let mut receipt = Some(receipt_tx);
            let mut socket = accept_hdr_async(
                stream,
                move |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                      response| {
                    let value = request
                        .headers()
                        .get("x-bcp-lease-id")
                        .map(|value| value.as_bytes().to_vec());
                    receipt
                        .take()
                        .expect("one upgrade")
                        .send(value)
                        .expect("test waiter remains");
                    Ok(response)
                },
            )
            .await
            .expect("valid WebSocket upgrade");
            let command = socket
                .next()
                .await
                .expect("version command arrives")
                .expect("valid frame");
            assert!(command.is_text());
            socket
                .send(Message::Close(None))
                .await
                .expect("close writes");
        });
        let headers =
            CdpHandshakeHeaders::new(vec![("X-BCP-Lease-Id".into(), "lease-fixture".into())])
                .expect("valid fixture header");
        let options = RemoteCdpConnectOptions::default().with_headers(headers);

        // Act
        let result = connect_over_cdp(format!("ws://{address}/cdp"), options).await;
        let receipt = receipt_rx.await.expect("server reports header");
        server.await.expect("server retires");

        // Assert
        assert!(result.is_err(), "a peer closing during attach must fail");
        assert_eq!(receipt, Some(b"lease-fixture".to_vec()));
    }
}
