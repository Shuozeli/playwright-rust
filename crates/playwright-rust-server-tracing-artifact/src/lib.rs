//! Generated opt-in handler for tracing Artifact production.

#[doc(hidden)]
#[rustfmt::skip]
mod generated_server;

pub use generated_server::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use playwright_rust_browser_model::BROWSER_OBJECT_KIND_CATALOG;
    use playwright_rust_cap_artifact::{
        ArtifactDelete, ArtifactFailure, ArtifactOpenStream, StreamClose, StreamRead,
    };
    use playwright_rust_cap_tracing::{
        TraceGroup, TraceGroupEnd, TraceStart, TraceStartChunk, TraceStop, TraceStopChunk,
    };
    use playwright_rust_codec_tracing_artifact::{
        TracingArtifactCodec, TracingArtifactCodecLimits,
    };
    use playwright_rust_core::ProtocolVersion;
    use playwright_rust_server_wire::{
        ServerCapabilityInventory, ServerCapabilityInventoryError, ServerCapabilitySource,
        ServerHandlerChain,
    };
    use playwright_rust_transport::{
        FrameLimits, NegotiationLimits, VersionPolicy, WireCapabilityId, WireProtocol,
    };

    use super::{TracingArtifactServerServices, tracing_artifact_server_handlers};

    struct RequiredCapabilities;

    impl ServerCapabilitySource for RequiredCapabilities {
        fn register_capabilities(
            &self,
            inventory: &mut ServerCapabilityInventory,
        ) -> Result<(), ServerCapabilityInventoryError> {
            inventory.register::<ArtifactDelete>()?;
            inventory.register::<ArtifactFailure>()?;
            inventory.register::<ArtifactOpenStream>()?;
            inventory.register::<StreamClose>()?;
            inventory.register::<StreamRead>()?;
            inventory.register::<TraceGroup>()?;
            inventory.register::<TraceGroupEnd>()?;
            inventory.register::<TraceStart>()?;
            inventory.register::<TraceStartChunk>()?;
            inventory.register::<TraceStop>()?;
            inventory.register::<TraceStopChunk>()
        }
    }

    #[test]
    fn generated_handler_advertises_only_when_composed() {
        // Arrange
        let version = ProtocolVersion::new(1, 0);
        let limits = FrameLimits::new(4_096, 0, 0, 4_096);
        let protocol = Arc::new(WireProtocol::new_exact_directional(
            TracingArtifactCodec::new(TracingArtifactCodecLimits::new(256)),
            version,
            limits,
            limits,
        ));
        let handlers = tracing_artifact_server_handlers(
            protocol,
            TracingArtifactServerServices {
                trace_stop_chunk_artifact: Arc::new(()),
            },
        );
        let empty =
            ServerCapabilityInventory::with_object_kinds(BROWSER_OBJECT_KIND_CATALOG).unwrap();
        let capability = WireCapabilityId::new("playwright.rust.tracing_artifact", 64).unwrap();

        // Act
        let producer_alone = handlers.capability_manifest_with_object_kinds(
            BROWSER_OBJECT_KIND_CATALOG,
            NegotiationLimits::new(8, 64),
        );
        let complete = ServerHandlerChain::new(RequiredCapabilities, handlers);
        let present = complete
            .capability_manifest_with_object_kinds(
                BROWSER_OBJECT_KIND_CATALOG,
                NegotiationLimits::new(8, 64),
            )
            .unwrap();
        let absent = empty.manifest(NegotiationLimits::new(8, 64)).unwrap();

        // Assert
        assert!(producer_alone.is_err());
        assert_eq!(
            present.support(&capability),
            Some(VersionPolicy::compatible_through(version))
        );
        assert_eq!(absent.support(&capability), None);
    }
}
