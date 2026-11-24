<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Testing guide

## Fast local checks

```sh
cargo fmt --all --check
CARGO_INCREMENTAL=0 cargo test --release --locked \
  -p playwright-rust-remote-cdp --features cdp-handshake-headers
CARGO_INCREMENTAL=0 cargo clippy --release --locked \
  -p playwright-rust-remote-cdp --features cdp-handshake-headers \
  --all-targets -- -D warnings
```

## Live remote-CDP tests

Use only the dedicated automation profile:

```sh
chrome-cdp start
chrome-cdp status
pwright --cdp http://127.0.0.1:9222 open about:blank

export PLAYWRIGHT_RUST_TEST_CDP=http://127.0.0.1:9222
export PLAYWRIGHT_RUST_TEST_CDP_WS="$(curl -fsS \
  "$PLAYWRIGHT_RUST_TEST_CDP/json/version" | jq -r .webSocketDebuggerUrl)"

CARGO_INCREMENTAL=0 cargo test --release --locked \
  -p playwright-rust-remote-cdp --features cdp-handshake-headers \
  --test live_facade -- --ignored --test-threads=1
```

## Test structure

Use Arrange, Act, Assert comments and one primary action per test. Tests must be
isolated, deterministic, and assert postconditions—not just successful return.

## Evidence ladder

1. Contract/unit test.
2. Mock transport or fake provider.
3. Real Chromium regression when browser behavior matters.
4. Pinned upstream comparison for compatibility claims.

Each layer proves only its declared scope.

## Official-test records

An official-test receipt records:

- upstream repository and immutable commit;
- exact source file or manifest rows;
- unchanged or adapted classification;
- candidate and oracle commands;
- browser identity and selected native profile;
- pass, fail, skip, and not-run counts; and
- hashes for source and result artifacts.

Local package tests are contract evidence, not a complete upstream corpus.
Historical receipts live under [docs/evidence](../evidence/README.md).

## Dependency checks

```sh
cargo tree --locked -p playwright-rust --no-default-features -e normal
cargo tree --locked -p playwright-rust \
  --no-default-features --features remote-cdp -e normal
```

The second tree is currently broader than the target because the facade still
uses the Node compatibility host. Treat that as known architecture debt.
