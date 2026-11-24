<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Known gaps

## Shared-browser safety

### Popup and indirect targets

Explicit `new_page` and `new_context` completions are attributed today. A popup
created by `window.open` or opener behavior is visible in topology but is not
yet guaranteed to join the creating attachment's owned set.

**Done when:** a deterministic shared-default-Context regression proves the
popup and its session-created opener are retired while pre-existing tabs live.

### Operation-drain race

An admitted creation can publish after the close path's owned-resource
snapshot if operation drain times out.

**Done when:** producers are fenced or late publications are reconciled under
the same close deadline, with zero surviving owned targets.

## Teardown correctness

### Cancellation safety

The disconnect guard is not installed before every cancellable close phase.

**Done when:** cancellation in drain, cleanup, driver join, and transport
retirement always disconnects and leaves one observable terminal state.

### Deadline multiplication

Facade and host cleanup can apply fresh timeouts per Context or phase.

**Done when:** one absolute graceful deadline plus one forced-local deadline
bounds close independently of target count.

### Lease release and Drop

Lease release lacks retry-stable shared state, and Drop is not a complete
asynchronous cleanup contract.

**Done when:** repeated close observes one stable release result and early
return/drop behavior is defined and tested across Rust, Node, and Python.

## Architecture

The high-level Rust facade still imports `HostSession` and public values from
the broad Node compatibility host.

**Done when:** a generic session layer has a small dependency closure and both
the Rust facade and Node host consume it.

## Compatibility

The high-level facade is intentionally a focused scraper slice. Navigation
response objects, evaluation arguments/typed returns, Context options, and the
complete Locator surface remain incomplete.

Add coverage from real consumer requirements, not by creating unsupported API
stubs.
