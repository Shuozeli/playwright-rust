<!-- agent-updated: 2026-11-14T02:35:39Z -->

# Open tasks

- [ ] Verify a clean standalone `--locked` release check, focused Rust tests,
  Node build/tests, Python wheel/tests, and relevant dependency proofs here.
- [ ] Resolve the known `cargo fmt --all -- --check` failures in three own
  source files; assess vendored WebSocket formatting separately without
  obscuring upstream provenance.
- [ ] Replace transitional `pwright-fake`/`pwright-js` test support with a
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
- [ ] Decide final Rust crate, Node package, and Python distribution names and
  publishability. Current names are provisional compatibility names.
- [ ] Design any new v2 CLI as a separate opt-in surface; the original v1 CLI
  remains in `Shuozeli/pwright`.
