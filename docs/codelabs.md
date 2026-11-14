<!-- agent-updated: 2026-11-14T02:34:45Z -->

# Codelab: inspect an opt-in dependency boundary

Run these commands from the repository root:

```sh
cargo metadata --locked --format-version 1 --no-deps
cargo tree --locked -p pwright --no-default-features -e normal
cargo tree --locked -p pwright-node-host --no-default-features -e normal
cargo tree --locked -p pwright-transport-ws --no-default-features -e normal
cargo tree --locked -p pwright-transport-ws --features live-tokio -e normal
```

Compare the two WebSocket trees: the forked WebSocket transport should appear
only in the selected `live-tokio` profile. Use the same method for each
capability before claiming it is independently installable. Metadata success
only checks manifest resolution; it is not a compile, runtime, memory, or
Playwright-compatibility result.

For a small code check, run:

```sh
CARGO_INCREMENTAL=0 cargo check --release --locked -p pwright --no-default-features
CARGO_INCREMENTAL=0 cargo test --release --locked -p pwright-core --lib
```

Browser tests need the dedicated Chrome CDP profile described by the machine
rules; never attach to a user's normal browser profile.
