use playwright_rust::client_content::PageScreenshotExt;
use playwright_rust::content::ScreenshotOptions;

fn main() {
    let options = ScreenshotOptions::default();
    let extension = std::any::type_name::<dyn PageScreenshotExt>();
    println!("{extension}: {options:?}");
}
