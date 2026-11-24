<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Agent guide

This guide defines the invariants coding agents must preserve.

## Before editing

1. Read the relevant current guide or concept document.
2. Read the matching item in `docs/tasks.md` or `docs/status/roadmap.md`.
3. Inspect the live checkout and preserve unrelated dirty work.
4. Identify the exact ownership and dependency boundary affected.

## Non-negotiable invariants

- Chromium/CDP is the only product browser backend.
- Workspace crates keep `default = []`.
- Remote attach never owns or terminates external Chrome.
- Pre-existing Contexts and Pages are borrowed, not cleanup targets.
- BCP-specific acquisition, renewal, fencing, and protos remain external.
- Rust owns browser state; bindings do not create parallel state machines.
- Compatibility claims require scoped behavior evidence.
- Public compatibility profiles do not invent finite object-count limits that
  upstream Playwright does not have.

## Lifecycle review checklist

- What operation establishes ownership?
- Can cancellation consume authority without publishing terminal state?
- Can a timeout multiply by target count or lifecycle phase?
- Can peer disconnect strand a caller or driver task?
- Are all retained map/queue entries retired at terminal state?
- Does repeated close observe a stable result?
- Does Drop have a documented, tested contract?

## Capability workflow

1. Start with one public behavior.
2. Add the smallest portable contract.
3. Add the Chromium implementation and opt-in composition.
4. Add thin Node/Python translation only when required.
5. Test contract, transport, live browser, and upstream semantics as needed.
6. Update reference, support matrix, tasks, and evidence.

## Documentation workflow

Current documents explain the current product. ADRs explain durable decisions.
Status documents explain open work. Evidence documents record bounded results.
Archive documents preserve superseded history. Do not combine those roles into
one growing ledger.

## Current priority

1. Popup and indirect-target attribution.
2. Operation-drain race.
3. Cancellation-safe single-deadline close.
4. Retry-stable lease release and explicit Drop contract.
5. Extract the Rust facade from the Node compatibility host.
6. Improve typed closure errors and failure-matrix coverage.

Use [Testing](testing.md) for commands and evidence requirements.
