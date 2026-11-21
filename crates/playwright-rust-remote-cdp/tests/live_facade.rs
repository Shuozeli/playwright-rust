use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use playwright_rust_remote_cdp::{
    ClickOptionsV2, GotoOptions, LocatorWaitState, RemoteCdpConnectOptions, RemoteCdpError,
    RemoteLease, RemoteLeaseFuture, ScreenshotOptions, WaitUntil, connect_over_cdp,
};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

struct RecordingLease(Arc<AtomicUsize>);

impl RemoteLease for RecordingLease {
    fn release(&self) -> RemoteLeaseFuture {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
#[ignore = "requires chrome-cdp preflight and PLAYWRIGHT_RUST_TEST_CDP"]
async fn facade_runs_the_scraper_workflow_and_releases_the_lease_once()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let endpoint = std::env::var("PLAYWRIGHT_RUST_TEST_CDP")?;
    assert_eq!(endpoint, "http://127.0.0.1:9222");
    let releases = Arc::new(AtomicUsize::new(0));
    let options = RemoteCdpConnectOptions::default()
        .with_remote_lease(Arc::new(RecordingLease(Arc::clone(&releases))));
    let browser = connect_over_cdp(&endpoint, options).await?;
    let page = browser.new_page().await?;
    let fixture = "data:text/html,<title>remote facade</title><input><button onclick=\"document.querySelector('main').textContent=document.querySelector('input').value\">go</button><main>empty</main>";

    // Act
    page.goto(
        fixture,
        GotoOptions {
            wait_until: WaitUntil::Load,
            timeout: Duration::from_secs(10),
        },
    )
    .await?;
    let input = page.locator("input")?;
    input.wait_for(LocatorWaitState::Visible).await?;
    input.fill("composable").await?;
    page.locator("button")?
        .click(ClickOptionsV2::default())
        .await?;
    let output = page.locator("main")?;
    let count = output.count().await?;
    let text = output.text_content().await?;
    let content = page.content().await?;
    let evaluated = page.evaluate("document.title").await?;
    let final_url = page.url().await?;
    let screenshot = page.screenshot(ScreenshotOptions::default()).await?;
    browser.close().await?;
    browser.close().await?;

    // Assert
    assert_eq!(count, 1);
    assert_eq!(text.as_deref(), Some("composable"));
    assert!(content.contains("composable"));
    assert!(
        matches!(evaluated, playwright_rust_remote_cdp::EvaluationValue::String(value) if value == "remote facade")
    );
    assert!(final_url.starts_with("data:text/html,"));
    assert!(screenshot.as_bytes().len() > 1_000);
    assert!(!browser.is_connected());
    assert_eq!(releases.load(Ordering::SeqCst), 1);

    let verifier = connect_over_cdp(&endpoint, RemoteCdpConnectOptions::default()).await?;
    verifier.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires chrome-cdp preflight plus PLAYWRIGHT_RUST_TEST_CDP and PLAYWRIGHT_RUST_TEST_CDP_WS"]
async fn revoked_proxy_fails_a_pending_call_and_close_does_not_hang()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let endpoint = std::env::var("PLAYWRIGHT_RUST_TEST_CDP")?;
    let upstream = std::env::var("PLAYWRIGHT_RUST_TEST_CDP_WS")?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let proxy_address = listener.local_addr()?;
    let (revoke_tx, mut revoke_rx) = tokio::sync::oneshot::channel();
    let proxy = tokio::spawn(async move {
        let (client, _) = listener.accept().await?;
        let downstream = tokio_tungstenite::accept_async(client).await?;
        let (upstream, _) = tokio_tungstenite::connect_async(upstream).await?;
        let (mut downstream_sink, mut downstream_stream) = downstream.split();
        let (mut upstream_sink, mut upstream_stream) = upstream.split();
        loop {
            tokio::select! {
                biased;
                _ = &mut revoke_rx => {
                    let _ = downstream_sink.send(Message::Close(None)).await;
                    break;
                }
                message = downstream_stream.next() => match message {
                    Some(Ok(message)) => upstream_sink.send(message).await?,
                    Some(Err(_)) | None => break,
                },
                message = upstream_stream.next() => match message {
                    Some(Ok(message)) => downstream_sink.send(message).await?,
                    Some(Err(_)) | None => break,
                },
            }
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });
    let releases = Arc::new(AtomicUsize::new(0));
    let browser = connect_over_cdp(
        format!("ws://{proxy_address}/cdp"),
        RemoteCdpConnectOptions::default()
            .with_remote_lease(Arc::new(RecordingLease(Arc::clone(&releases)))),
    )
    .await?;
    let page = browser.new_page().await?;
    page.goto(
        "data:text/html,<title>revocation fixture</title>",
        GotoOptions {
            wait_until: WaitUntil::Load,
            timeout: Duration::from_secs(10),
        },
    )
    .await?;
    let pending_page = page.clone();
    let pending = tokio::spawn(async move { pending_page.evaluate("new Promise(() => {})").await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Act
    revoke_tx.send(()).expect("proxy remains active");
    let pending_result = tokio::time::timeout(Duration::from_secs(5), pending).await??;
    let close_result = tokio::time::timeout(Duration::from_secs(10), browser.close()).await;
    proxy
        .await?
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    // Assert
    assert!(
        matches!(pending_result, Err(RemoteCdpError::Disconnected)),
        "pending result was {pending_result:?}"
    );
    assert!(close_result.is_ok(), "close hung after proxy revocation");
    assert_eq!(releases.load(Ordering::SeqCst), 1);
    let verifier = connect_over_cdp(endpoint, RemoteCdpConnectOptions::default()).await?;
    verifier.close().await?;
    Ok(())
}
