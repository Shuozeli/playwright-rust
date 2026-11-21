# playwright-rust-ecmascript-regexp

Optional ECMAScript regular-expression matching for Playwright Rust. This leaf
wraps the Rust-native `regress` engine and preserves JavaScript `RegExp.test()`
behavior needed by Playwright URL filters, including lookaround,
backreferences, UTF-16 versus Unicode matching, Unicode-set `v` syntax, all
current `d/g/i/m/s/u/v/y` flags, and stateful `g`/`y` `lastIndex` updates.

The crate owns no browser, runtime, FFI, routing, HAR, or cookie behavior.
`regress` is a backtracking engine, like JavaScript regular expressions, so
matching intentionally has no linear-time guarantee. Consumers select this
crate only with the corresponding optional RegExp capability. The leaf adds no
arbitrary source, flag, compiled-program, or matcher-count quota; an embedder
that needs a constrained policy must compose one explicitly.

The engine is source-pinned at upstream `regress` commit `95c04041` with its
Unicode tables regenerated from Unicode 16.0, matching the Node 24.12 runtime
used by the Playwright v1.58.0 conformance baseline. See the vendored
`PLAYWRIGHT_RUST_PROVENANCE.md` for inputs and upgrade policy.

The checked-in source-pinned Test262/Node 24.12 corpus covers 5,135 static
literal/constructor patterns and 97,198 `test()`/`lastIndex` steps. Regenerating
it with `scripts/generate_test262_regexp_corpus.mjs` is byte-for-byte
reproducible; the compressed corpus SHA-256 is
`90b348d0c0bc63cca448976a3bfd95299c2a56b158b68b5181c82a40ac85c3f6`.
Dynamic Test262 programs and a live upstream route oracle remain separate open
proof obligations, so this corpus is not a blanket ECMAScript parity claim.
