<!-- agent-updated: 2026-11-23T03:34:47Z -->

# ADR 0004: evidence-scoped compatibility

**Status:** Accepted

## Context

Matching method names or passing local contract tests does not establish
Playwright behavior. Adapted test setup can also change what a result proves.

## Decision

Every compatibility claim names its source pin, selected surface, candidate
profile, unchanged or adapted status, and result. No bounded result is promoted
to a complete-suite claim.

## Consequences

- Support matrices distinguish supported, incomplete, and non-goal behavior.
- Historical ledgers are evidence, not user documentation.
- Official-test receipts preserve commands and hashes.
- Local package tests remain contract evidence.
