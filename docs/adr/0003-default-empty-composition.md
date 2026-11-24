<!-- agent-updated: 2026-11-23T03:34:47Z -->

# ADR 0003: default-empty composition

**Status:** Accepted

## Context

A monolithic automation dependency carries browser, runtime, transport, and
feature costs even when an application needs only a narrow workflow.

## Decision

Workspace crates use `default = []`. Capability contracts, implementations,
runtimes, transports, and language profiles are selected explicitly.

## Consequences

- API presence and installed runtime behavior are separate checks.
- Umbrella features document dependency composition.
- Negative dependency-closure tests matter as much as positive builds.
- Convenience facades may compose a useful slice but may not silently enable
  every workspace capability.
