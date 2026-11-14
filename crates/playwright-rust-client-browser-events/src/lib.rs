//! Opt-in client installation for Playwright-shaped Browser terminal events.

use playwright_rust_cap_browser_events::{BrowserDisconnected, BrowserDisconnectedEvent};
use playwright_rust_client::{Browser, CallIdSource};
use playwright_rust_client_events::{
    SubscriptionError, SubscriptionTracker, SubscriptionTransition, TypedEventHandler,
};
use playwright_rust_codec_browser_events::BrowserEventsCodec;
use playwright_rust_core::{Event, OverflowPolicy};
use playwright_rust_transport::WireProtocol;

/// Disconnection is terminal lifecycle state. Losing or replacing it would
/// leave the client's view inconsistent, so overflow rejects the new event.
pub fn browser_disconnected_handler(
    protocol: WireProtocol<BrowserEventsCodec>,
    capacity: usize,
) -> TypedEventHandler<BrowserDisconnected, BrowserEventsCodec> {
    TypedEventHandler::new(protocol, capacity, OverflowPolicy::RejectNewest)
}

pub const fn retained_browser_disconnected_bytes(_event: &Event<BrowserDisconnected>) -> usize {
    std::mem::size_of::<BrowserDisconnectedEvent>()
}

pub trait BrowserDisconnectedSubscriptionExt {
    fn subscribe_disconnected(
        &self,
        subscriptions: &mut SubscriptionTracker,
    ) -> Result<SubscriptionTransition, SubscriptionError>;

    fn unsubscribe_disconnected(
        &self,
        subscriptions: &mut SubscriptionTracker,
    ) -> Result<SubscriptionTransition, SubscriptionError>;
}

impl<Channel, Ids> BrowserDisconnectedSubscriptionExt for Browser<Channel, Ids>
where
    Ids: CallIdSource,
{
    fn subscribe_disconnected(
        &self,
        subscriptions: &mut SubscriptionTracker,
    ) -> Result<SubscriptionTransition, SubscriptionError> {
        subscriptions.subscribe(
            self.id(),
            <BrowserDisconnected as playwright_rust_core::EventSpec>::EVENT,
        )
    }

    fn unsubscribe_disconnected(
        &self,
        subscriptions: &mut SubscriptionTracker,
    ) -> Result<SubscriptionTransition, SubscriptionError> {
        subscriptions.unsubscribe(
            self.id(),
            <BrowserDisconnected as playwright_rust_core::EventSpec>::EVENT,
        )
    }
}

#[cfg(test)]
mod tests {
    use playwright_rust_browser_model::BROWSER_OBJECT_KIND;
    use playwright_rust_cap_browser_events::BROWSER_DISCONNECTED_EVENT_TAG;
    use playwright_rust_client::{AtomicCallIdSource, Client, RemoteInitializer};
    use playwright_rust_client_events::{SubscriptionTracker, SubscriptionTransition};
    use playwright_rust_codec_browser_events::BrowserEventsCodec;
    use playwright_rust_core::{ControlMessage, ObjectId, OverflowPolicy, ProtocolVersion};
    use playwright_rust_transport::{FrameLimits, WireProtocol};

    use super::{BrowserDisconnectedSubscriptionExt, browser_disconnected_handler};

    #[test]
    fn disconnected_installation_is_reliable_and_targets_the_browser() {
        // Arrange
        let client = Client::new((), AtomicCallIdSource::new(1));
        let browser = client
            .browser(RemoteInitializer::new(
                ObjectId::new(1),
                None,
                BROWSER_OBJECT_KIND.tag,
            ))
            .expect("browser should initialize");
        let protocol = WireProtocol::new_exact_directional(
            BrowserEventsCodec,
            ProtocolVersion::new(1, 0),
            FrameLimits::new(17, 0, 0, 17),
            FrameLimits::new(17, 0, 0, 17),
        );
        let mut subscriptions = SubscriptionTracker::new(1);

        // Act
        let transition = browser
            .subscribe_disconnected(&mut subscriptions)
            .expect("subscription should fit");

        // Assert
        let handler = browser_disconnected_handler(protocol, 1);
        assert_eq!(handler.overflow_policy(), OverflowPolicy::RejectNewest);
        assert_eq!(
            transition,
            SubscriptionTransition::Send(ControlMessage::SetSubscription {
                object_id: ObjectId::new(1),
                event: BROWSER_DISCONNECTED_EVENT_TAG,
                enabled: true,
            })
        );
    }
}
