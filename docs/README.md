<!-- agent-updated: 2026-11-24T19:10:05Z -->

# Playwright Rust documentation

Playwright Rust is a pre-release, Chromium-only automation stack. Start with
the remote-CDP workflow, then use the reference and concept documents when you
need exact behavior or implementation boundaries.

## Start here

- [Remote-CDP getting started](getting-started/remote-cdp.md)
- [Rust API reference](reference/rust-api.md)
- [Support matrix](reference/support-matrix.md)
- [Known gaps](status/known-gaps.md)

## Language guides

- [Node.js](guides/node.md)
- [Python](guides/python.md)

## Concepts

- [Architecture](concepts/architecture.md)
- [Ownership and lifecycle](concepts/ownership-lifecycle.md)

## Contributing

- [Development](contributing/development.md)
- [Testing](contributing/testing.md)
- [Agent guide](contributing/agent-guide.md)

## Decisions and status

- [Architecture decision records](adr/README.md)
- [Roadmap](status/roadmap.md)
- [Active tasks](tasks.md)

## Engineering evidence

The [evidence index](evidence/README.md) contains source-pinned conformance and
binding ledgers. These files record bounded experiments; they are not blanket
compatibility claims.

## Archive

The [archive](archive/README.md) preserves superseded v2 design documents,
working notes, and the previous task ledger. Archive files are historical and
may contain stale links or results.

## Browser previews

The Markdown files in this repository are the source of truth. When a rendered
preview is needed, serve it with `serve-lib` on Tailscale.
