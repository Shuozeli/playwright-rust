<!-- agent-updated: 2026-11-14T22:25:00Z -->

# Open tasks

- [x] Match upstream Chromium's unbounded active Request count; add 129-request
  lifecycle/adapter regressions and verify that an unfiltered WSJ load no
  longer fails on request 65.
- [x] Treat a network Request whose observed frame has already left topology as
  frame-unavailable instead of terminating the Browser.
- [ ] Fix the independent HostSession teardown hang observed after remote CDP
  screenshot runs, then rerun the full WSJ navigation/screenshot/zero-resource
  cleanup path without diagnostic timeouts.

- [x] Verify standalone locked release checks, the Rust workspace tests, Node
  build/tests, Python wheel/tests, and relevant dependency proofs.
- [x] Resolve Rust and vendored WebSocket formatting failures without removing
  upstream provenance.
- [ ] Replace transitional `playwright-rust-fake`/`playwright-rust-js` test support with a
  v2-only CDP fake and prove the production and test dependency graphs.
- [ ] Review the copied scripts and create CI scoped to this repository;
  remove old-repo checks and path assumptions before enabling broad gates.
- [ ] Rebuild native packages from source. Do not import generated `.node`,
  `.so`, `.dylib`, wheel files, or local `.work/` evidence as source.
- [ ] Re-run source-bound official Playwright Core JS/TS and Python test suites
  from the new checkout; regenerate immutable receipts and report pass/fail,
  skip, error, and not-run separately. No full-parity claim is authorized yet.
- [ ] Measure the Rust host and browser process footprints separately under
  representative workloads; enforce retained-object and queue budgets.
- [ ] Audit copied detailed docs for source-era URLs and unresolved `.work/`
  links, preserving provenance without implying fresh verification.
- [x] Rename the repository, Rust crates, Node package, Python distribution,
  import namespace, environment variables, protocol namespace, documentation,
  and generated artifacts consistently as Playwright Rust.
- [ ] Decide publication policy for the Rust crates, Node package, and Python
  distribution.
- [ ] Design any future CLI as a separate opt-in surface.
