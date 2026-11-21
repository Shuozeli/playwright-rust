use super::*;
use playwright_rust_core::ResourceLimits;

#[test]
fn initialization_failures_latch_first_error_without_adding_duplicate_owners() {
    // Arrange
    let mut failures = InitializationFailures::new();
    let page = ObjectId::new(1);
    failures
        .record(page, HeaderSyntaxError::InvalidValue)
        .unwrap();
    // Act
    failures
        .record(page, HeaderSyntaxError::InvalidName)
        .unwrap();
    // Assert
    assert_eq!(failures.get(page), Some(HeaderSyntaxError::InvalidValue));
    assert_eq!(failures.0.len(), 1);
}

#[test]
fn initialization_failures_grow_past_legacy_page_count_and_reclaim_retired_pages() {
    // Arrange
    let mut failures = InitializationFailures::new();
    for id in 1..=129 {
        failures
            .record(ObjectId::new(id), HeaderSyntaxError::InvalidName)
            .unwrap();
    }
    let extra = ObjectId::new(1_000);
    // Act
    failures
        .record(extra, HeaderSyntaxError::NonAsciiName)
        .unwrap();
    failures.retain(|id| id != ObjectId::new(1));
    // Assert
    assert_eq!(failures.get(ObjectId::new(1)), None);
    assert_eq!(
        failures.get(ObjectId::new(2)),
        Some(HeaderSyntaxError::InvalidName)
    );
    assert_eq!(failures.get(extra), Some(HeaderSyntaxError::NonAsciiName));
    assert_eq!(failures.0.len(), 129);
}

#[test]
fn header_syntax_errors_preserve_native_messages() {
    // Arrange
    let cases = [
        (HeaderSyntaxError::NonAsciiName, "Invalid parameters"),
        (HeaderSyntaxError::InvalidName, "Invalid header name"),
        (HeaderSyntaxError::InvalidValue, "Invalid header value"),
    ];
    for (error, expected) in cases {
        // Act
        let result = syntax_error(error);
        // Assert
        assert!(matches!(result, HostError::Backend(message)
            if message == format!("Protocol error (Network.setExtraHTTPHeaders): {expected}")));
    }
}

#[test]
fn header_capacity_errors_remain_resource_failures() {
    // Arrange
    let cases = [
        HeaderError::CountLimit,
        HeaderError::ByteLimit,
        HeaderError::OwnerLimit,
    ];
    for error in cases {
        // Act
        let result = input_error(error);
        // Assert
        assert!(matches!(result, HostError::ResourceExhausted(_)));
    }
}

#[test]
fn persistent_launch_input_rejects_count_and_byte_overflow() {
    // Arrange
    let too_many = (0..=MAX_EXTRA_HEADERS)
        .map(|index| (format!("x-{index}"), String::new()))
        .collect();
    let too_large = vec![("x-large".to_owned(), "v".repeat(MAX_EXTRA_HEADER_BYTES))];

    // Act
    let count_result = PersistentContextExtraHeaders::new(too_many);
    let byte_result = PersistentContextExtraHeaders::new(too_large);

    // Assert
    assert!(matches!(
        count_result,
        Err(HostError::ResourceExhausted(message)) if message.contains("count exceeds 256")
    ));
    assert!(matches!(
        byte_result,
        Err(HostError::ResourceExhausted(message)) if message.contains("bytes exceed 65536")
    ));
}

pub(super) fn ledger() -> ProductionResourceLedger {
    let req = requirements().unwrap();
    let mut limits = ResourceLimits::deny_all(req.scopes, req.leases);
    for class in ResourceClass::ALL {
        let bound = req.class(class);
        limits = limits.with_class(class, bound.global, bound.per_scope);
    }
    ProductionResourceLedger::new(limits, req).unwrap()
}

#[test]
fn persistent_headers_seed_only_default_context_and_explicit_clear_wins() {
    // Arrange
    let resources = ledger();
    {
        let pool = HeaderPool::new(value_profile().unwrap(), &resources).unwrap();
        let headers = pool
            .page(&[HeaderRef {
                name: "X-Persistent",
                value: "selected",
            }])
            .unwrap();
        let default = PersistentDefault::new(Some(headers));
        let settings =
            Mutex::new(HeaderSettings::new(settings_profile().unwrap(), pool, &resources).unwrap());
        let default_context = ObjectId::new(1);
        let owned_context = ObjectId::new(2);

        // Act
        default
            .seed(&settings, owned_context, CdpContextOwnership::Owned)
            .unwrap();
        default
            .seed(&settings, default_context, CdpContextOwnership::Default)
            .unwrap();
        default
            .seed(&settings, default_context, CdpContextOwnership::Default)
            .unwrap();
        let selected = settings
            .lock()
            .unwrap()
            .effective(default_context, ObjectId::new(11))
            .unwrap();
        let not_inherited = settings
            .lock()
            .unwrap()
            .effective(owned_context, ObjectId::new(12))
            .unwrap();
        settings
            .lock()
            .unwrap()
            .begin_context(default_context, &[])
            .unwrap()
            .commit()
            .unwrap();
        let cleared = settings
            .lock()
            .unwrap()
            .effective(default_context, ObjectId::new(11))
            .unwrap();

        // Assert
        let selected: Vec<_> = selected.iter().collect();
        assert_eq!(
            selected,
            [HeaderRef {
                name: "X-Persistent",
                value: "selected",
            }]
        );
        assert_eq!(not_inherited.iter().len(), 0);
        assert_eq!(cleared.iter().len(), 0);
    }
    for class in ResourceClass::ALL {
        assert_eq!(resources.metrics().unwrap().class(class).current, 0);
    }
}

#[test]
fn native_parameters_preserve_page_winning_case_and_clear_to_empty_object() {
    // Arrange
    let resources = ledger();
    let pool = HeaderPool::new(value_profile().unwrap(), &resources).unwrap();
    let context = pool
        .context(&[HeaderRef {
            name: "X-A",
            value: "context",
        }])
        .unwrap();
    let page = pool
        .page(&[HeaderRef {
            name: "x-a",
            value: "page",
        }])
        .unwrap();
    let merged = pool.merge(Some(&context), Some(&page)).unwrap();
    let empty = pool.merge(None, None).unwrap();
    // Act
    let values = [native::encode(&merged), native::encode(&empty)];
    // Assert
    assert_eq!(
        values,
        [
            serde_json::json!({"headers":{"x-a":"page"}}),
            serde_json::json!({"headers":{}})
        ]
    );
}

#[test]
#[cfg(feature = "page-routes")]
fn route_request_adds_page_navigation_referrer_missing_from_fetch_headers() {
    // Arrange
    let resources = ledger();
    let pool = HeaderPool::new(value_profile().unwrap(), &resources).unwrap();
    let page = pool
        .page(&[HeaderRef {
            name: "Referer",
            value: "https://example.test/empty.html",
        }])
        .unwrap();
    let mut headers = vec![("accept".to_owned(), "*/*".to_owned())];

    // Act
    include_navigation_referrer(&mut headers, Some(&page)).unwrap();

    // Assert
    assert_eq!(
        headers,
        [
            ("accept".to_owned(), "*/*".to_owned()),
            (
                "referer".to_owned(),
                "https://example.test/empty.html".to_owned()
            )
        ]
    );
}

#[test]
#[cfg(feature = "page-routes")]
fn route_request_keeps_native_referrer_instead_of_adding_a_second_value() {
    // Arrange
    let resources = ledger();
    let pool = HeaderPool::new(value_profile().unwrap(), &resources).unwrap();
    let page = pool
        .page(&[HeaderRef {
            name: "referer",
            value: "https://configured.test/",
        }])
        .unwrap();
    let mut headers = vec![("Referer".to_owned(), "https://native.test/".to_owned())];

    // Act
    include_navigation_referrer(&mut headers, Some(&page)).unwrap();

    // Assert
    assert_eq!(
        headers,
        [("Referer".to_owned(), "https://native.test/".to_owned())]
    );
}

#[test]
fn production_header_ownership_profiles_are_allocator_backed() {
    // Arrange
    let req = body_requirements();
    // Act
    let values = value_profile().unwrap();
    let settings = settings_profile().unwrap();
    // Assert
    assert!(values.allocator_backed());
    assert!(settings.allocator_backed());
    assert_eq!(
        values
            .requirements()
            .unwrap()
            .class(ResourceClass::LiveObject)
            .global,
        0
    );
    assert!(req.class(ResourceClass::BackendStateBytes).global > 0);
}

#[test]
fn production_pool_grows_past_legacy_owner_count_and_reclaims_all_owners() {
    // Arrange
    let resources = ledger();
    let pool = HeaderPool::new(value_profile().unwrap(), &resources).unwrap();
    // Act
    let owners: Vec<_> = (0..129).map(|_| pool.page(&[]).unwrap()).collect();
    // Assert
    assert_eq!(pool.active_owners(), 129);
    drop(owners);
    assert_eq!(pool.active_owners(), 0);
    drop(pool);
    for class in ResourceClass::ALL {
        assert_eq!(resources.metrics().unwrap().class(class).current, 0);
    }
}

#[test]
fn selected_pool_and_settings_release_every_charge_after_last_owner() {
    // Arrange
    let resources = ledger();
    let pool = HeaderPool::new(value_profile().unwrap(), &resources).unwrap();
    let settings =
        HeaderSettings::new(settings_profile().unwrap(), pool.clone(), &resources).unwrap();
    settings
        .begin_context(
            ObjectId::new(2),
            &[HeaderRef {
                name: "X-A",
                value: "one",
            }],
        )
        .unwrap()
        .commit()
        .unwrap();
    let escaped = settings
        .effective(ObjectId::new(2), ObjectId::new(3))
        .unwrap();
    drop(settings);
    drop(pool);
    // Act
    drop(escaped);
    // Assert
    for class in ResourceClass::ALL {
        assert_eq!(resources.metrics().unwrap().class(class).current, 0);
    }
}

#[tokio::test]
async fn production_rejection_precedes_held_compensation_and_drains_primary_siblings() {
    for compensation_fails in [false, true] {
        // Arrange
        let resources = ledger();
        {
            let pool = HeaderPool::new(value_profile().unwrap(), &resources).unwrap();
            let settings = Arc::new(Mutex::new(
                HeaderSettings::new(settings_profile().unwrap(), pool, &resources).unwrap(),
            ));
            let context = ObjectId::new(1);
            let page = ObjectId::new(2);
            let value = |value| {
                [HeaderRef {
                    name: "X-Layer",
                    value,
                }]
            };
            settings
                .lock()
                .unwrap()
                .begin_context(context, &value("old"))
                .unwrap()
                .commit()
                .unwrap();
            let update = settings
                .lock()
                .unwrap()
                .begin_context(context, &value("new"))
                .unwrap();
            let mut update = PreparedUpdate::new(settings.clone(), update);
            let plan = DispatchPlan::capture(
                &settings.lock().unwrap(),
                vec![Route {
                    context,
                    page: Some(page),
                    watch: session_retirement::Watch::new("session", "session", "target"),
                }],
            )
            .unwrap();
            let (primary_ack, primary_done) = oneshot::channel();
            let mut primary = fanout::Fanout::new();
            primary
                .push(async { Err(HostError::Backend("primary failure".into())) })
                .unwrap();
            primary
                .push(async {
                    primary_done.await.unwrap();
                    Ok(())
                })
                .unwrap();
            let error = primary.first_result().await.unwrap_err();
            let compensation_plan = update.restore_plan(&plan).unwrap();
            let (compensation_ack, compensation_done) = oneshot::channel();
            let (reply, mut received) = oneshot::channel();
            let mut compensation = fanout::Fanout::new();
            compensation
                .push(async {
                    assert_eq!(
                        compensation_plan.pages[0]
                            .1
                            .as_ref()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap()
                            .value,
                        "old"
                    );
                    compensation_done.await.unwrap();
                    if compensation_fails {
                        Err(HostError::Backend("secondary compensation failure".into()))
                    } else {
                        Ok(())
                    }
                })
                .unwrap();
            let mut work = std::pin::pin!(reject_and_drain(
                &mut update,
                &mut primary,
                Some(&mut compensation),
                reply,
                error
            ));
            // Act: poll the actual production rejection/drain coordinator once.
            std::future::poll_fn(|cx| {
                assert!(work.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            // Assert: primary is visible while both native gates are still held.
            assert!(
                matches!(received.try_recv().unwrap(), Err(HostError::Backend(message)) if message == "primary failure")
            );
            assert_eq!(
                settings
                    .lock()
                    .unwrap()
                    .effective(context, page)
                    .unwrap()
                    .iter()
                    .next()
                    .unwrap()
                    .value,
                "old"
            );
            settings
                .lock()
                .unwrap()
                .begin_context(context, &value("later"))
                .unwrap()
                .commit()
                .unwrap();
            compensation_ack.send(()).unwrap();
            std::future::poll_fn(|cx| {
                assert!(work.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            primary_ack.send(()).unwrap();
            work.await;
            assert_eq!(
                settings
                    .lock()
                    .unwrap()
                    .effective(context, page)
                    .unwrap()
                    .iter()
                    .next()
                    .unwrap()
                    .value,
                "later"
            );
        }
        for class in ResourceClass::ALL {
            assert_eq!(resources.metrics().unwrap().class(class).current, 0);
        }
    }
}
