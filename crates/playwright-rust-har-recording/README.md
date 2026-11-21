<!-- agent-updated: 2026-11-19T08:21:53Z -->

# playwright-rust-har-recording

Backend-neutral Playwright 1.58.0 HAR recording policy. The crate consumes
normalized request/response lifecycle snapshots and returns serialized HAR
bytes plus deduplicated attachments. It does not own a browser, CDP session,
runtime, filesystem, or ZIP writer.

Minimal and full policies are distinct. Full mode accepts Page lifecycle,
cookies, raw headers, protocol/server/TLS metadata, timing, sizes, compression,
and redirect information. Page IDs use Playwright's opaque `page@...` shape;
the embedding host supplies the clock and absolute lifecycle timestamps, while
this crate stores and exports deterministic relative page timings.

Attachment names match Playwright's bundled `mime@3.0.0` policy: SHA-1 of the
content plus the preferred MIME extension, or `.dat` when the MIME database
has no entry. The generated preferred-extension table is covered by
[MIME-LICENSE](MIME-LICENSE).
