<!-- agent-updated: 2026-11-23T03:34:47Z -->

# ADR 0001: Chromium/CDP only

**Status:** Accepted

## Context

The project exists to provide a smaller, composable client for Chromium
deployments. CDP is Chromium-specific; Firefox and WebKit require different
protocols, fixtures, and semantic work.

## Decision

Chromium over CDP is the only product browser target. Portable contracts may
remain engine-neutral when that improves composition, but no Firefox or WebKit
implementation is planned.

## Consequences

- Public language facades expose only Chromium.
- Compatibility denominators use the Chromium portion of pinned Playwright.
- Firefox/WebKit gaps are non-goals, not missing features.
- CDP-specific behavior must not be presented as universal browser behavior.
