use playwright_rust::client_context_creation::BrowserNewContextExt;

fn main() {
    println!("{}", std::any::type_name::<dyn BrowserNewContextExt<Context = ()>>());
}
