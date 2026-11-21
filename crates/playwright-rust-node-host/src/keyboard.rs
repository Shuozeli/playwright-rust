//! Page-owned Keyboard commands in the existing controlled host runtime.
//! No selector, focus step, native RPC in JavaScript, or private runtime.
use crate::{HostError, HostSession};
use playwright_rust_cap_keyboard::*;
pub use playwright_rust_cap_keyboard::{KEYBOARD_KEY_MAX_BYTES, KEYBOARD_TEXT_MAX_BYTES};
use playwright_rust_composition_chromium::CdpPageServices;
use playwright_rust_composition_chromium_keyboard::{
    CdpKeyboardDelay, CdpKeyboardProductionProfile, CdpKeyboardServiceFactory,
    ChromiumKeyboardPage, KeyboardPlatform,
};
use playwright_rust_core::{
    CallMetadata, CancellationReason, Command, CommandCancellation, CommandSpec, ObjectId,
    OperationContext, OperationError, ProductionResourceLedger, ResourceClass, ResourceLease,
    ResourceLimits, ResourceRequirements, ResourceScope,
};
use playwright_rust_runtime::TargetedService;
use std::{
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

pub(crate) const EXECUTING: usize = 8;
const STATE_SCOPE: ResourceScope = ResourceScope::extension(120, 18);
const STATE_BYTES: u64 = 4096;
// Test-only layout guard for invocation/native-binding frames. Retained calls
// themselves grow through the allocator instead of a fabricated aggregate cap.
#[cfg(test)]
const CALL_BYTES: u64 = 8192;

fn backend_profile() -> Result<CdpKeyboardProductionProfile, HostError> {
    CdpKeyboardProductionProfile::new(
        ResourceScope::extension(120, 19),
        KEYBOARD_TEXT_MAX_BYTES,
        NonZeroUsize::new(EXECUTING).expect("positive keyboard concurrency"),
    )
    .map_err(HostError::backend)
}
fn storage_requirements() -> ResourceRequirements {
    ResourceRequirements::none()
        .with_class(ResourceClass::BackendStateBytes, STATE_BYTES, STATE_BYTES)
        .with_metadata(1, 1)
}
pub(crate) fn requirements() -> Result<ResourceRequirements, HostError> {
    Ok(storage_requirements().combine_concurrent(backend_profile()?.requirements()))
}
fn route_resources() -> Result<ProductionResourceLedger, HostError> {
    let required = ChromiumKeyboardPage::route_requirements();
    let mut limits = ResourceLimits::deny_all(required.scopes, required.leases);
    for class in ResourceClass::ALL {
        let bound = required.class(class);
        limits = limits.with_class(class, bound.global, bound.per_scope);
    }
    ProductionResourceLedger::new(limits, required).map_err(HostError::backend)
}
struct HostDelay;
impl CdpKeyboardDelay for HostDelay {
    fn wait(
        &self,
        duration: Duration,
    ) -> Pin<
        Box<dyn Future<Output = Result<(), playwright_rust_cdp::connection::CdpError>> + Send + '_>,
    > {
        Box::pin(async move {
            tokio::time::sleep(duration).await;
            Ok(())
        })
    }
}
struct Slots {
    count: AtomicUsize,
    _state: ResourceLease,
}
struct CallLease {
    _route_resources: ProductionResourceLedger,
    slots: Arc<Slots>,
}
impl Drop for CallLease {
    fn drop(&mut self) {
        let previous = self.slots.count.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0);
    }
}
pub(crate) struct Installation {
    factory: CdpKeyboardServiceFactory,
    slots: Arc<Slots>,
    #[cfg(feature = "drag-runtime")]
    drag: Option<Arc<crate::drag::host::Installation>>,
}
impl Installation {
    #[cfg(feature = "drag-runtime")]
    pub(crate) fn with_drag(mut self, drag: Arc<crate::drag::host::Installation>) -> Self {
        self.drag = Some(drag);
        self
    }
    pub(crate) fn new(
        resources: &ProductionResourceLedger,
        host_os: &str,
    ) -> Result<Self, HostError> {
        resources
            .claim(requirements()?)
            .map_err(HostError::backend)?;
        let ledger = resources
            .claim(storage_requirements())
            .map_err(HostError::backend)?;
        let state = ledger
            .reserve(STATE_SCOPE, ResourceClass::BackendStateBytes, STATE_BYTES)
            .map_err(HostError::backend)?;
        let platform = if host_os == "macos" {
            KeyboardPlatform::Mac
        } else {
            KeyboardPlatform::Other
        };
        Ok(Self {
            factory: CdpKeyboardServiceFactory::new_with_production_resources(
                backend_profile()?,
                resources,
                Arc::new(HostDelay),
                platform,
            )
            .map_err(HostError::backend)?,
            slots: Arc::new(Slots {
                count: AtomicUsize::new(0),
                _state: state,
            }),
            #[cfg(feature = "drag-runtime")]
            drag: None,
        })
    }
    fn admit(&self, argument_bytes: usize) -> Result<CallLease, HostError> {
        if argument_bytes > KEYBOARD_TEXT_MAX_BYTES {
            return Err(HostError::ResourceExhausted(
                "Keyboard argument exceeds 1 MiB".into(),
            ));
        }
        let route_resources = route_resources()?;
        self.slots
            .count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_add(1)
            })
            .map_err(|_| {
                HostError::ResourceExhausted("Keyboard retained invocation count overflow".into())
            })?;
        Ok(CallLease {
            _route_resources: route_resources,
            slots: self.slots.clone(),
        })
    }
    fn bind(
        &self,
        services: &CdpPageServices,
        page: ObjectId,
        lease: CallLease,
    ) -> Result<Arc<Invocation>, HostError> {
        let binding = services
            .page_input_binding(page)
            .map_err(HostError::backend)?
            .ok_or(HostError::TargetClosed)?;
        let (client, input) = binding.into_parts();
        #[cfg(feature = "drag-runtime")]
        let drag = self
            .drag
            .as_ref()
            .map(|drag| drag.bind(page, &client))
            .transpose()?;
        #[cfg(feature = "drag-runtime")]
        let route = ChromiumKeyboardPage::bind_with_key_down_interceptor(
            page,
            client,
            input,
            &self.factory,
            &lease._route_resources,
            drag.as_ref().map(|controller| {
                controller.dispatcher.clone()
                    as Arc<dyn playwright_rust_composition_chromium_keyboard::CdpKeyDownInterceptor>
            }),
        )
        .map_err(HostError::backend)?;
        #[cfg(not(feature = "drag-runtime"))]
        let route =
            ChromiumKeyboardPage::bind(page, client, input, &self.factory, &lease._route_resources)
                .map_err(HostError::backend)?;
        Ok(Arc::new(Invocation {
            route,
            _lease: lease,
            #[cfg(feature = "drag-runtime")]
            drag,
        }))
    }
}
struct Invocation {
    route: Arc<ChromiumKeyboardPage>,
    #[cfg(feature = "drag-runtime")]
    drag: Option<Arc<crate::drag::host::PageController>>,
    // Retained with the exact cancellation handler, not just the host future;
    // drop retained provider handles before releasing this invocation's storage.
    _lease: CallLease,
}
macro_rules! route {
    ($command:ty, $params:ty) => {
        impl TargetedService<$params> for Invocation {
            type Response = ();
            type Error = OperationError<KeyboardErrorDetails>;
            type Future<'a> = <ChromiumKeyboardPage as TargetedService<$params>>::Future<'a>;
            fn call_for(&self, id: ObjectId, params: $params) -> Self::Future<'_> {
                self.route.call_for(id, params)
            }
        }
        impl CommandCancellation<$command> for Invocation {
            type Future<'a> = <ChromiumKeyboardPage as CommandCancellation<$command>>::Future<'a>;
            fn cancel(
                &self,
                context: OperationContext,
                reason: CancellationReason,
            ) -> Self::Future<'_> {
                let cleanup = <ChromiumKeyboardPage as CommandCancellation<$command>>::cancel(
                    &self.route,
                    context,
                    reason,
                );
                #[cfg(feature = "drag-runtime")]
                return Box::pin(async move {
                    let result = cleanup.await;
                    let retired = if let Some(drag) = &self.drag {
                        drag.drain().await.map_err(|error| {
                            OperationError::new(
                                playwright_rust_core::FailureKind::Cleanup,
                                error.to_string(),
                                None,
                            )
                        })
                    } else {
                        Ok(())
                    };
                    result.and(retired)
                });
                #[cfg(not(feature = "drag-runtime"))]
                cleanup
            }
        }
    };
}
route!(KeyboardDown, DownParams);
route!(KeyboardUp, UpParams);
route!(KeyboardPress, PressParams);
route!(KeyboardInsertText, InsertTextParams);
route!(KeyboardType, TypeParams);

impl HostSession {
    async fn keyboard_invoke<C>(
        &self,
        page: &str,
        argument_bytes: usize,
        convert: impl FnOnce() -> Result<C::Params, HostError>,
        timeout_ms: u32,
    ) -> Result<(), HostError>
    where
        C: CommandSpec<Result = (), ErrorDetails = KeyboardErrorDetails>,
        C::Params: Send + 'static,
        Invocation: TargetedService<C::Params, Response = (), Error = OperationError<KeyboardErrorDetails>>
            + CommandCancellation<C>,
    {
        let (page, call, _operation) = self.admit_operation(page)?;
        let (runtime, installation) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            (live.runtime.clone(), live.keyboard.clone())
        };
        // Admission precedes the owned argument copy, route snapshot and queuing.
        let lease = installation.admit(argument_bytes)?;
        let invocation = installation.bind(runtime.service(), page, lease)?;
        let params = convert()?;
        let timeout = (timeout_ms != 0).then(|| Duration::from_millis(u64::from(timeout_ms)));
        let command = Command::<C>::new(call, page, CallMetadata::new(timeout, None), params);
        let outcome = tokio::select! {
            biased;
            () = self.stop.cancelled() => return Err(HostError::TargetClosed),
            result = runtime.dispatch_with(command, invocation.as_ref(), invocation.clone()) => result,
        };
        let result = match outcome {
            Ok(result) => result,
            Err(never) => match never {},
        };
        result
            .into_outcome()
            .map_err(|error| keyboard_error(error, timeout_ms))
    }
    pub async fn keyboard_down(&self, page: &str, key: &str) -> Result<(), HostError> {
        self.keyboard_invoke::<KeyboardDown>(
            page,
            key.len(),
            || {
                Ok(DownParams {
                    key: keyboard_key(key)?,
                })
            },
            0,
        )
        .await
    }
    pub async fn keyboard_up(&self, page: &str, key: &str) -> Result<(), HostError> {
        self.keyboard_invoke::<KeyboardUp>(
            page,
            key.len(),
            || {
                Ok(UpParams {
                    key: keyboard_key(key)?,
                })
            },
            0,
        )
        .await
    }
    pub async fn keyboard_press(
        &self,
        page: &str,
        key: &str,
        delay: Duration,
    ) -> Result<(), HostError> {
        self.keyboard_invoke::<KeyboardPress>(
            page,
            key.len(),
            || {
                Ok(PressParams {
                    key: keyboard_key(key)?,
                    delay: Some(delay),
                })
            },
            0,
        )
        .await
    }
    pub async fn keyboard_insert_text(&self, page: &str, text: &str) -> Result<(), HostError> {
        self.keyboard_invoke::<KeyboardInsertText>(
            page,
            text.len(),
            || {
                Ok(InsertTextParams {
                    text: keyboard_text(text)?,
                })
            },
            0,
        )
        .await
    }
    pub async fn keyboard_type(
        &self,
        page: &str,
        text: &str,
        delay: Duration,
    ) -> Result<(), HostError> {
        self.keyboard_invoke::<KeyboardType>(
            page,
            text.len(),
            || {
                Ok(TypeParams {
                    text: keyboard_text(text)?,
                    delay: Some(delay),
                })
            },
            0,
        )
        .await
    }
}
fn keyboard_key(value: &str) -> Result<KeyboardKey, HostError> {
    KeyboardKey::copy_from_str(value)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))
}

fn keyboard_error(error: OperationError<KeyboardErrorDetails>, timeout_ms: u32) -> HostError {
    if matches!(error.details(), Some(KeyboardErrorDetails::InvalidKey(_))) {
        HostError::Backend(error.message().to_owned())
    } else {
        HostError::operation(error, timeout_ms)
    }
}
fn keyboard_text(value: &str) -> Result<KeyboardText, HostError> {
    KeyboardText::copy_from_str(value)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))
}

#[cfg(test)]
mod tests;
