use playwright_rust::client_locator::LocatorCountExt;
use playwright_rust::locator::CssSelector;

fn main() {
    let selector = CssSelector::new("button").expect("static selector should be valid");
    println!(
        "{}: {selector:?}",
        std::any::type_name::<dyn LocatorCountExt>()
    );
}
