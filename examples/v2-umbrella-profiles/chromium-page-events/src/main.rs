use playwright_rust::chromium_page_events::ChromiumPageEventsProfile;

fn main() {
    println!("{}", std::mem::size_of::<ChromiumPageEventsProfile>());
}
