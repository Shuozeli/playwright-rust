use playwright_rust::client_handle_lifecycle::HandleDisposeExt;

fn main() {
    let _ = std::any::type_name::<dyn HandleDisposeExt>();
}
