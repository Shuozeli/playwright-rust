#![cfg(feature = "service-worker-events")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use playwright_rust_node_host::{HostSession, SessionMetrics, evaluation::EvaluateParams};

#[tokio::test]
#[ignore = "requires an explicitly selected dedicated Chrome service-worker fixture"]
async fn service_worker_registration_publishes_and_evaluates()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let endpoint =
        std::env::var("PLAYWRIGHT_RUST_TEST_CDP").expect("dedicated CDP endpoint required");
    // Python and Node bindings select lifecycle events when Page events are
    // installed, so exercise the same composition instead of the collection-
    // only convenience path.
    let host = HostSession::connect_over_cdp_with_lifecycle_events(&endpoint, 10_000).await?;
    let context = host
        .contexts()?
        .into_iter()
        .next()
        .expect("browser context");
    let page = host
        .pages(&context.id)?
        .into_iter()
        .find(|page| {
            page.main_frame_metadata
                .url()
                .is_ok_and(|url| url.contains("playwright-rust-sw-e2e"))
        })
        .expect("fixture page");
    let existing = host.context_service_workers(&context.id)?.len();
    for _ in 0..existing {
        tokio::time::timeout(Duration::from_secs(2), host.next_service_worker_notice()).await??;
    }
    let run = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let source = format!(
        "navigator.serviceWorker.register('./sw.js?rust={run}', {{scope:'./rust-{run}/'}}).then(r=>r.scope)"
    );

    // Act
    let (registration, notice) = tokio::try_join!(
        host.frame_evaluate(&page.main_frame_id, EvaluateParams::expression(source)),
        async {
            tokio::time::timeout(Duration::from_secs(10), host.next_service_worker_notice())
                .await
                .map_err(|error| playwright_rust_node_host::HostError::Backend(error.to_string()))?
        }
    )?;
    let marker = host
        .worker_evaluate(
            notice.owner.clone(),
            EvaluateParams::expression("self.playwrightRustMarker"),
        )
        .await?;

    // Assert
    println!(
        "registration={registration:?} worker={} marker={marker:?}",
        notice.owner.url()
    );
    assert!(notice.owner.url().contains(&format!("rust={run}")));
    match tokio::time::timeout(Duration::from_secs(10), host.close()).await {
        Ok(result) => result?,
        Err(_) => {
            eprintln!("close timeout metrics={:?}", host.metrics());
            return Err("service-worker host close timed out".into());
        }
    }
    drop(notice);
    drop(page);
    drop(context);
    assert_eq!(host.metrics()?, SessionMetrics::default());
    Ok(())
}

#[tokio::test]
#[ignore = "requires an explicitly selected dedicated Chrome service-worker fixture"]
async fn service_worker_registration_does_not_require_a_notice_consumer()
-> Result<(), Box<dyn std::error::Error>> {
    // Arrange
    let endpoint =
        std::env::var("PLAYWRIGHT_RUST_TEST_CDP").expect("dedicated CDP endpoint required");
    let host = HostSession::connect_over_cdp_with_lifecycle_events(&endpoint, 10_000).await?;
    let context = host
        .contexts()?
        .into_iter()
        .next()
        .expect("browser context");
    let page = host
        .pages(&context.id)?
        .into_iter()
        .find(|page| {
            page.main_frame_metadata
                .url()
                .is_ok_and(|url| url.contains("playwright-rust-sw-e2e"))
        })
        .expect("fixture page");
    let existing = host.context_service_workers(&context.id)?.len();
    for _ in 0..existing {
        tokio::time::timeout(Duration::from_secs(2), host.next_service_worker_notice()).await??;
    }
    let run = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let source = format!(
        "navigator.serviceWorker.register('./sw.js?unobserved={run}', {{scope:'./unobserved-{run}/'}}).then(r=>r.scope)"
    );

    // Act
    let registration = tokio::time::timeout(
        Duration::from_secs(10),
        host.frame_evaluate(&page.main_frame_id, EvaluateParams::expression(source)),
    )
    .await??;
    let notice =
        tokio::time::timeout(Duration::from_secs(2), host.next_service_worker_notice()).await??;

    // Assert
    assert!(matches!(
        registration,
        playwright_rust_node_host::evaluation::EvaluationValue::String(_)
    ));
    assert!(notice.owner.url().contains(&format!("unobserved={run}")));
    match tokio::time::timeout(Duration::from_secs(10), host.close()).await {
        Ok(result) => result?,
        Err(_) => {
            eprintln!("close timeout metrics={:?}", host.metrics());
            return Err("service-worker host close timed out".into());
        }
    }
    drop(notice);
    drop(page);
    drop(context);
    assert_eq!(host.metrics()?, SessionMetrics::default());
    Ok(())
}

#[cfg(all(feature = "worker-network-events", feature = "request-history"))]
#[tokio::test]
#[ignore = "requires an explicitly selected dedicated Chrome service-worker fixture"]
async fn service_worker_request_is_context_owned_and_keeps_its_response()
-> Result<(), Box<dyn std::error::Error>> {
    use playwright_rust_node_host::network_events::{NetworkEventKind, NetworkEventRecord};

    // Arrange
    let endpoint =
        std::env::var("PLAYWRIGHT_RUST_TEST_CDP").expect("dedicated CDP endpoint required");
    let host = HostSession::connect_over_cdp_with_lifecycle_events(&endpoint, 10_000).await?;
    let context = host
        .contexts()?
        .into_iter()
        .next()
        .expect("browser context");
    let page = host
        .pages(&context.id)?
        .into_iter()
        .find(|page| {
            page.main_frame_metadata
                .url()
                .is_ok_and(|url| url.contains("playwright-rust-sw-e2e"))
        })
        .expect("fixture page");
    let existing = host.context_service_workers(&context.id)?.len();
    for _ in 0..existing {
        tokio::time::timeout(Duration::from_secs(2), host.next_service_worker_notice()).await??;
    }
    let run = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let registration = format!(
        "navigator.serviceWorker.register('./sw.js?network={run}', {{scope:'./network-{run}/'}}).then(r=>r.scope)"
    );
    let (_, worker_notice) = tokio::try_join!(
        host.frame_evaluate(
            &page.main_frame_id,
            EvaluateParams::expression(registration),
        ),
        async {
            tokio::time::timeout(Duration::from_secs(10), host.next_service_worker_notice())
                .await
                .map_err(|error| playwright_rust_node_host::HostError::Backend(error.to_string()))?
        }
    )?;
    let worker = worker_notice.owner.clone();
    let context_waiter = host
        .context_arm_network_wait(&context.id, NetworkEventKind::Request, 10_000)
        .await?;
    let page_waiter = host
        .page_arm_network_wait(&page.id, NetworkEventKind::Request, 500)
        .await?;
    let fetch_url = format!("./rust-network-{run}.txt");
    let fetch = format!("fetch({fetch_url:?}).then(response => response.status)");

    // Act
    let (_, observed) = tokio::try_join!(
        host.worker_evaluate(worker.clone(), EvaluateParams::expression(fetch)),
        async {
            tokio::time::timeout(
                Duration::from_secs(10),
                host.page_next_network_event(context_waiter),
            )
            .await
            .map_err(|error| playwright_rust_node_host::HostError::Backend(error.to_string()))?
        }
    )?;
    let page_observed = host.page_next_network_event(page_waiter).await;
    let (event, history_owner) = observed.into_parts();
    let NetworkEventRecord::Request(request) = event else {
        return Err("Context waiter did not receive the Service Worker request".into());
    };
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        history_owner
            .expect("Service Worker request keeps a history owner")
            .response(),
    )
    .await??
    .expect("Service Worker fetch has a response");

    // Assert
    assert!(request.url.ends_with(&fetch_url));
    assert_eq!(request.context_id.as_deref(), Some(context.id.as_str()));
    assert_eq!(request.service_worker_id.as_deref(), Some(worker.id()));
    assert_eq!(request.frame_id, None);
    assert!(!request.frame_available);
    assert_eq!(response.context_id.as_deref(), Some(context.id.as_str()));
    assert_eq!(response.service_worker_id.as_deref(), Some(worker.id()));
    assert!(!response.from_service_worker);
    assert!(matches!(
        page_observed,
        Err(playwright_rust_node_host::HostError::Timeout(_))
    ));
    drop(response);
    drop(request);

    match tokio::time::timeout(Duration::from_secs(10), host.close()).await {
        Ok(result) => result?,
        Err(_) => {
            eprintln!("close timeout metrics={:?}", host.metrics());
            return Err("service-worker host close timed out".into());
        }
    }
    drop(worker);
    drop(worker_notice);
    drop(page);
    drop(context);
    assert_eq!(host.metrics()?, SessionMetrics::default());
    Ok(())
}

#[cfg(all(
    feature = "worker-handle-evaluation",
    feature = "worker-console-events",
    feature = "worker-console-argument-handles"
))]
#[tokio::test]
#[ignore = "requires an explicitly selected dedicated Chrome service-worker fixture"]
async fn service_worker_console_and_handles_are_context_owned()
-> Result<(), Box<dyn std::error::Error>> {
    use playwright_rust_cap_handle_evaluation::EvaluateHandleParams;

    // Arrange
    let endpoint =
        std::env::var("PLAYWRIGHT_RUST_TEST_CDP").expect("dedicated CDP endpoint required");
    let host = HostSession::connect_over_cdp_with_lifecycle_events(&endpoint, 10_000).await?;
    let context = host
        .contexts()?
        .into_iter()
        .next()
        .expect("browser context");
    let page = host
        .pages(&context.id)?
        .into_iter()
        .find(|page| {
            page.main_frame_metadata
                .url()
                .is_ok_and(|url| url.contains("playwright-rust-sw-e2e"))
        })
        .expect("fixture page");
    let existing = host.context_service_workers(&context.id)?.len();
    for _ in 0..existing {
        tokio::time::timeout(Duration::from_secs(2), host.next_service_worker_notice()).await??;
    }
    let run = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let source = format!(
        "navigator.serviceWorker.register('./sw.js?context-owned={run}', {{scope:'./context-owned-{run}/'}}).then(r=>r.scope)"
    );
    let (_, worker_notice) = tokio::try_join!(
        host.frame_evaluate(&page.main_frame_id, EvaluateParams::expression(source)),
        async {
            tokio::time::timeout(Duration::from_secs(10), host.next_service_worker_notice())
                .await
                .map_err(|error| playwright_rust_node_host::HostError::Backend(error.to_string()))?
        }
    )?;
    let worker = worker_notice.owner.clone();
    host.set_worker_console_listener(worker.clone(), true)?;

    // Act
    let (_, console_notice) = tokio::try_join!(
        host.worker_evaluate(
            worker.clone(),
            EvaluateParams::expression("console.log('rust-service-worker-console', {answer: 42})",),
        ),
        async {
            tokio::time::timeout(Duration::from_secs(10), host.next_worker_console_notice())
                .await
                .map_err(|error| playwright_rust_node_host::HostError::Backend(error.to_string()))?
        }
    )?;
    let evaluated = host
        .worker_evaluate_handle(
            worker.clone(),
            EvaluateHandleParams::expression("({answer: 42})"),
        )
        .await?;
    let evaluated_value = host.worker_handle_json_value(&evaluated).await?;
    let argument_value = host
        .worker_handle_json_value(
            console_notice
                .argument_handles
                .get(1)
                .expect("console object argument Handle"),
        )
        .await?;

    // Assert
    assert_eq!(console_notice.page, None);
    assert_eq!(console_notice.context, worker.context());
    assert!(std::sync::Arc::ptr_eq(&console_notice.owner, &worker));
    assert_eq!(console_notice.message.message_type(), "log");
    assert!(
        console_notice
            .message
            .text()
            .contains("rust-service-worker-console")
    );
    for value in [evaluated_value, argument_value] {
        assert!(matches!(
            value,
            playwright_rust_node_host::evaluation::EvaluationValue::Object {
                properties,
                ..
            } if properties == [
                playwright_rust_node_host::evaluation::EvaluationProperty::new(
                    "answer",
                    playwright_rust_node_host::evaluation::EvaluationValue::Number(
                        playwright_rust_node_host::evaluation::EvaluationNumber::new(42.0)?,
                    ),
                ),
            ]
        ));
    }

    host.set_worker_console_listener(worker, false)?;
    evaluated.request_dispose().wait().await?;
    for handle in &console_notice.argument_handles {
        handle.request_dispose().wait().await?;
    }
    match tokio::time::timeout(Duration::from_secs(10), host.close()).await {
        Ok(result) => result?,
        Err(_) => {
            eprintln!("close timeout metrics={:?}", host.metrics());
            return Err("service-worker host close timed out".into());
        }
    }
    drop(console_notice);
    drop(worker_notice);
    drop(page);
    drop(context);
    assert_eq!(host.metrics()?, SessionMetrics::default());
    Ok(())
}
