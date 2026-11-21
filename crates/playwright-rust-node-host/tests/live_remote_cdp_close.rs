//! Explicit remote-CDP lifecycle proof against the dedicated Chrome profile.

use std::sync::Arc;
use std::time::Duration;

use playwright_rust_node_host::{
    HostError, HostSession, SessionMetrics,
    evaluation::{EvaluateParams, EvaluationValue},
    screenshot::ScreenshotOptions,
};

async fn pages_with_title(host: &HostSession, expected: &str) -> Result<usize, HostError> {
    let mut matches = 0;
    for context in host.contexts()? {
        for page in host.pages(&context.id)? {
            if host.frame_title(&page.main_frame_id).await? == expected {
                matches += 1;
            }
        }
    }
    Ok(matches)
}

async fn context_with_title(host: &HostSession, expected: &str) -> Result<String, HostError> {
    for context in host.contexts()? {
        for page in host.pages(&context.id)? {
            if host.frame_title(&page.main_frame_id).await? == expected {
                return Ok(context.id);
            }
        }
    }
    Err(HostError::Backend(format!(
        "no Context contains the marked Page `{expected}`"
    )))
}

fn page_count(host: &HostSession) -> Result<usize, HostError> {
    host.contexts()?
        .into_iter()
        .try_fold(0usize, |total, context| {
            host.pages(&context.id)
                .map(|pages| total.saturating_add(pages.len()))
        })
}

#[tokio::test]
#[ignore = "requires chrome-cdp preflight, a marked pre-existing tab, and PLAYWRIGHT_RUST_TEST_CDP"]
async fn remote_close_retires_an_owned_wsj_page_and_preserves_the_shared_browser()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let endpoint = std::env::var("PLAYWRIGHT_RUST_TEST_CDP")?;
    assert_eq!(endpoint, "http://127.0.0.1:9222");
    let marker_title = std::env::var("PLAYWRIGHT_RUST_PREEXISTING_TITLE")?;
    let test_url = std::env::var("PLAYWRIGHT_RUST_TEST_URL")?;
    let baseline = HostSession::connect_over_cdp(&endpoint, 5_000).await?;
    let baseline_contexts = baseline.contexts()?.len();
    assert_eq!(pages_with_title(&baseline, &marker_title).await?, 1);
    baseline.close().await?;
    assert_eq!(baseline.metrics()?, SessionMetrics::default());

    let subject = HostSession::connect_over_cdp(&endpoint, 5_000).await?;
    let context = subject.new_context().await?;
    let page = subject.new_page(&context.id).await?;
    let exercise = tokio::time::timeout(Duration::from_secs(60), async {
        subject
            .page_goto(&page.id, test_url, "commit", None, 30_000)
            .await?;
        tokio::time::sleep(Duration::from_secs(3)).await;
        let content = subject.frame_content(&page.main_frame_id).await?;
        let summary = subject
            .frame_evaluate(
                &page.main_frame_id,
                EvaluateParams::expression("JSON.stringify([document.title, location.href])"),
            )
            .await?;
        let screenshot = subject
            .page_screenshot(&page.id, ScreenshotOptions::default(), 15_000)
            .await?;
        Ok::<_, HostError>((
            content,
            summary,
            screenshot.as_bytes().len(),
            page.main_frame_metadata.url()?,
        ))
    })
    .await;

    // Act: Browser close owns this Context but not the shared Chrome process or
    // tabs that existed before this HostSession attached.
    let close = tokio::time::timeout(Duration::from_secs(15), subject.close()).await;

    // Assert
    let (content, summary, screenshot_bytes, final_url) =
        exercise.expect("WSJ exercise exceeded its outer deadline")?;
    assert!(content.contains("<html"), "rendered document was empty");
    assert!(matches!(summary, EvaluationValue::String(value) if !value.is_empty()));
    assert!(
        screenshot_bytes > 1_000,
        "screenshot was unexpectedly empty"
    );
    assert!(!final_url.is_empty(), "final URL was empty");
    close.expect("remote close exceeded its deadline")?;
    assert_eq!(subject.metrics()?, SessionMetrics::default());

    let verifier = HostSession::connect_over_cdp(&endpoint, 5_000).await?;
    assert_eq!(verifier.contexts()?.len(), baseline_contexts);
    assert_eq!(pages_with_title(&verifier, &marker_title).await?, 1);
    verifier.close().await?;
    assert_eq!(verifier.metrics()?, SessionMetrics::default());
    Ok(())
}

#[tokio::test]
#[ignore = "requires chrome-cdp preflight, a marked pre-existing tab, and PLAYWRIGHT_RUST_TEST_CDP"]
async fn remote_close_retires_a_page_created_in_the_shared_default_context()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let endpoint = std::env::var("PLAYWRIGHT_RUST_TEST_CDP")?;
    assert_eq!(endpoint, "http://127.0.0.1:9222");
    let marker_title = std::env::var("PLAYWRIGHT_RUST_PREEXISTING_TITLE")?;
    let created_title = "PW-RUST-SESSION-OWNED-DEFAULT-PAGE";
    let subject = HostSession::connect_over_cdp(&endpoint, 5_000).await?;
    let shared_context = context_with_title(&subject, &marker_title).await?;
    let page = subject.new_page(&shared_context).await?;
    subject
        .page_goto(
            &page.id,
            format!("data:text/html,<title>{created_title}</title>"),
            "load",
            None,
            5_000,
        )
        .await?;
    assert_eq!(pages_with_title(&subject, created_title).await?, 1);

    // Act
    tokio::time::timeout(Duration::from_secs(15), subject.close()).await??;

    // Assert
    assert_eq!(subject.metrics()?, SessionMetrics::default());
    let verifier = HostSession::connect_over_cdp(&endpoint, 5_000).await?;
    assert_eq!(pages_with_title(&verifier, &marker_title).await?, 1);
    assert_eq!(pages_with_title(&verifier, created_title).await?, 0);
    verifier.close().await?;
    assert_eq!(verifier.metrics()?, SessionMetrics::default());
    Ok(())
}

#[tokio::test]
#[ignore = "requires chrome-cdp preflight, a marked pre-existing tab, and PLAYWRIGHT_RUST_TEST_CDP"]
async fn remote_close_drains_concurrent_page_creation_before_its_ownership_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let endpoint = std::env::var("PLAYWRIGHT_RUST_TEST_CDP")?;
    assert_eq!(endpoint, "http://127.0.0.1:9222");
    let marker_title = std::env::var("PLAYWRIGHT_RUST_PREEXISTING_TITLE")?;
    let subject = Arc::new(HostSession::connect_over_cdp(&endpoint, 5_000).await?);
    let shared_context = context_with_title(&subject, &marker_title).await?;
    let baseline_pages = page_count(&subject)?;
    let start = Arc::new(tokio::sync::Barrier::new(17));
    let mut creations = Vec::new();
    for _ in 0..16 {
        let host = Arc::clone(&subject);
        let context = shared_context.clone();
        let start = Arc::clone(&start);
        creations.push(tokio::spawn(async move {
            start.wait().await;
            host.new_page(&context).await
        }));
    }
    start.wait().await;
    tokio::task::yield_now().await;

    // Act
    tokio::time::timeout(Duration::from_secs(15), subject.close()).await??;
    for creation in creations {
        let _ = creation.await?;
    }

    // Assert
    assert_eq!(subject.metrics()?, SessionMetrics::default());
    let verifier = HostSession::connect_over_cdp(&endpoint, 5_000).await?;
    assert_eq!(pages_with_title(&verifier, &marker_title).await?, 1);
    assert_eq!(page_count(&verifier)?, baseline_pages);
    verifier.close().await?;
    assert_eq!(verifier.metrics()?, SessionMetrics::default());
    Ok(())
}
