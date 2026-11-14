use playwright_rust::client_dom_content::{FrameContentExt, PageContentExt};
use playwright_rust::dom_content::DomContentParams;

fn main() {
    let request = DomContentParams;
    let frame_extension = std::any::type_name::<dyn FrameContentExt>();
    let page_extension = std::any::type_name::<dyn PageContentExt>();
    println!("{frame_extension} + {page_extension}: {request:?}");
}
