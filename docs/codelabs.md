<!-- agent-updated: 2026-11-21T21:04:31Z -->

# Codelab: inspect an opt-in dependency boundary

Run these commands from the repository root:

```sh
cargo metadata --locked --format-version 1 --no-deps
cargo tree --locked -p playwright-rust --no-default-features -e normal
cargo tree --locked -p playwright-rust-node-host --no-default-features -e normal
cargo tree --locked -p playwright-rust-transport-ws --no-default-features -e normal
cargo tree --locked -p playwright-rust-transport-ws --features live-tokio -e normal
```

Compare the two WebSocket trees: the forked WebSocket transport should appear
only in the selected `live-tokio` profile. Use the same method for each
capability before claiming it is independently installable. Metadata success
only checks manifest resolution; it is not a compile, runtime, memory, or
Playwright-compatibility result.

For a small code check, run:

```sh
CARGO_INCREMENTAL=0 cargo check --release --locked -p playwright-rust --no-default-features
CARGO_INCREMENTAL=0 cargo test --release --locked -p playwright-rust-core --lib
```

Browser tests need the dedicated Chrome CDP profile described by the machine
rules; never attach to a user's normal browser profile.

## Use the high-level Rust remote-CDP facade

Select only the facade and custom-header capability:

```toml
[dependencies]
playwright-rust = { git = "https://github.com/Shuozeli/playwright-rust", rev = "<commit>", default-features = false, features = [
  "cdp-handshake-headers",
] }
```

The `cdp-handshake-headers` umbrella feature includes `remote-cdp`. Connect to
an explicit WebSocket supplied by the external lease adapter:

```rust,no_run
use playwright_rust::remote_cdp::{
    CdpHandshakeHeaders, GotoOptions, RemoteCdpConnectOptions, connect_over_cdp,
};

# async fn example(ws_url: &str) -> Result<(), Box<dyn std::error::Error>> {
let headers = CdpHandshakeHeaders::new(vec![
    ("X-BCP-Lease-Id".into(), "lease-7".into()),
    ("X-BCP-Fencing-Token".into(), "42".into()),
])?;
let browser = connect_over_cdp(
    ws_url,
    RemoteCdpConnectOptions::default().with_headers(headers),
).await?;
let context = browser.contexts()?.into_iter()
    .find(|context| !context.is_owned())
    .ok_or_else(|| std::io::Error::other("no borrowed default Context"))?;
let page = context.new_page().await?;
page.goto("https://example.com", GotoOptions::default()).await?;
let html = page.content().await?;
assert!(html.contains("Example Domain"));
page.close().await?;
browser.close().await?;
# Ok(())
# }
```

For an external lease, implement `RemoteLease::release()` and pass it with
`with_remote_lease(Arc::new(...))`. The current close path takes and attempts
the hook at most once. The adapter remains responsible for renewal and must
make release idempotent; cancellation/retry-stable release state is still open.

Focused verification from the repository root:

```sh
CARGO_INCREMENTAL=0 cargo test --release --locked \
  -p playwright-rust-remote-cdp --features cdp-handshake-headers
CARGO_INCREMENTAL=0 cargo test --release --locked \
  -p playwright-rust-node-host --test live_remote_cdp_close -- --ignored
```

## Re-run the pinned BrowserContext cookie slice

Build the `official-conformance-browsercontext-cookies` release addon first.
Then initialize the dedicated CDP browser and run each complete upstream file
from its source/ID-pinned manifest:

```sh
chrome-cdp start
chrome-cdp status
pwright --cdp http://127.0.0.1:9222 open about:blank

manifest=tests/official/playwright-core-browsercontext-cookies-chromium-v1-manifest.json
for suite in \
  library-browsercontext-cookies-chromium-v1 \
  library-browsercontext-clearcookies-chromium-v1 \
  library-browsercontext-add-cookies-chromium-v1 \
  library-browsercontext-cookies-third-party-chromium-v1; do
  uv run --no-project python scripts/run_playwright_official_conformance.py \
    candidate /Users/cyuan/projects/thirdparty/playwright-v1.58.0 \
    --manifest "$manifest" \
    --suite "$suite" \
    --endpoint http://127.0.0.1:9222 \
    --candidate packages/playwright-rust \
    --candidate-profile official-conformance-browsercontext-cookies \
    --workers 1 \
    --result ".work/official-conformance/results/${suite}-candidate.json"
done
```

Do not merge the four JSON totals into a whole-library compatibility rate.
They cover only the 56 manifest-pinned tests, and upstream-declared skips stay
reported as skips.

## Verify Rust-owned video output

This owned-launch check does not attach to the dedicated CDP browser. It
requires the pinned Chromium and `ffmpeg-1011` artifacts in the Playwright
registry plus `ffprobe` on `PATH`:

```sh
for mode in persistent implicit; do
  for api in async sync; do
    video_output=$(mktemp -d "/tmp/playwright-rust-video-${mode}-${api}.XXXXXX")
    uv run --no-sync --project packages/playwright-rust-python python \
      packages/playwright-rust-python/tests/live/persistent_video_e2e.py \
      --mode "$mode" --api "$api" --output "$video_output"
  done
done
```

The script uses only `playwright_rust`. Persistent mode exercises
`launch_persistent_context()`; implicit mode exercises
`Browser.new_page(record_video_dir=...)` and its owned Context. Each async lane
starts `save_as()` while the Page is open and proves that it waits across
Context close. Each sync lane proves the exact pre-close rejection and a
successful post-close copy. All lanes call `path()` immediately, delete the
Rust-owned source twice, retain the saved copy, reject a post-delete copy with
Playwright's exact error, and require a nonempty VP8 WebM at 640x360. Remove
the temporary outputs after inspecting the printed paths.
