<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Documentation manifest

| Document group | Update when |
| --- | --- |
| `../README.md` | Product scope, primary workflow, release state, or public entry points change |
| `README.md` | Documentation structure or canonical destinations change |
| `getting-started/` | Installation, prerequisites, or the first remote-CDP workflow changes |
| `guides/` | Node or Python public entry points and package status change |
| `reference/rust-api.md` | High-level Rust facade signatures or options change |
| `reference/support-matrix.md` | Supported, incomplete, or intentionally excluded behavior changes |
| `concepts/architecture.md` | Dependency direction, component ownership, or FFI boundaries change |
| `concepts/ownership-lifecycle.md` | Attach ownership, close ordering, disconnect, lease, or Drop behavior changes |
| `contributing/` | Development, testing, review, or agent workflow changes |
| `adr/` | A durable design decision is accepted, superseded, or reversed |
| `status/` and `tasks.md` | Priorities, milestones, or open work change |
| `evidence/` | A source-bound conformance result is independently re-run and recorded |
| `archive/` | Normally never; archived files preserve historical state |
| `../crates/playwright-rust-remote-cdp/README.md` | Remote-CDP crate API, features, or limitations change |

Every new or updated Markdown document starts with an `agent-updated` timestamp.
Do not turn evidence ledgers or task history into user-facing API contracts.
