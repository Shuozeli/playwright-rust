use playwright_rust::chromium_handle_evaluation::ChromiumHandleEvaluationProfile;

fn main() {
    println!(
        "{}",
        std::mem::size_of::<ChromiumHandleEvaluationProfile>()
    );
}
