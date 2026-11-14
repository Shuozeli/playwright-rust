use playwright_rust::client_network_events::PageNetworkSubscriptionExt;

fn main() {
    let _ = std::any::type_name::<dyn PageNetworkSubscriptionExt>();
}
