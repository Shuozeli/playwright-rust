use playwright_rust::client_page_creation::BrowserContextNewPageExt;

fn main() {
    println!(
        "{}",
        std::any::type_name::<dyn BrowserContextNewPageExt<Page = ()>>()
    );
}
