<!-- agent-updated: 2026-11-23T03:34:47Z -->

# ADR 0002: attach-safe remote CDP

**Status:** Accepted

## Context

Remote deployments use shared, long-lived, often logged-in Chrome profiles.
An automation client must not terminate the process or close tabs it did not
create.

## Decision

A CDP attachment borrows the Chrome process. Ownership is attributed only to
successful resource creation through that attachment. Close retires exact
owned resources, disconnects transport, and may notify a generic external
lease hook. It never sends `Browser.close`.

## Consequences

- Pre-existing Contexts and Pages survive attachment close.
- Borrowed default Contexts expose profile state but cannot be closed.
- Popup lineage and cancellation safety are correctness requirements.
- Control-plane lease and fencing logic remains outside this repository.
