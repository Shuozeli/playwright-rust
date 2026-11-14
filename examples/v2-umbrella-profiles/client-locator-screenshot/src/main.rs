use playwright_rust::client_locator_screenshot::LocatorScreenshotExt;
use playwright_rust::locator_screenshot::ScreenshotOptions;

fn main() {
    let options = ScreenshotOptions::default();
    let extension = std::any::type_name::<dyn LocatorScreenshotExt>();
    println!("{extension}: {options:?}");
}
