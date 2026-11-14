use playwright_rust_plugin_ordered_handle_delivery::HandleCreationCommand;

fn main() {
    println!(
        "{}",
        <playwright_rust_cap_handle_evaluation::EvaluateHandle as HandleCreationCommand>::operation_name()
    );
}
