use playwright_rust::ordered_handle_delivery::HandleCreationCommand;

fn main() {
    println!(
        "{}",
        <playwright_rust::handle_evaluation::EvaluateHandle as HandleCreationCommand>::operation_name()
    );
}
