# Vendored regress provenance

This directory vendors the minimal library source of
[`regress`](https://github.com/ridiculousfish/regress) at commit
`95c04041cd9b30249961eb80d950102c29c9586c`. The upstream MIT and Apache-2.0
license files are preserved next to the source.

The sole data-policy change is `src/unicodetables.rs`: it was regenerated with
the upstream `gen-unicode` tool using the official Unicode 16.0.0 UCD archive
and Emoji 16.0 sequence files. Node 24.12.0 reports Unicode 16.0, and this table
keeps Playwright v1.58.0 RegExp routing semantics aligned with that pinned
runtime while retaining later upstream fixes for duplicate named-group
backreferences, UTF-16 matching, Unicode sets, and backtracking control.

The generation inputs are:

- `https://www.unicode.org/Public/zipped/16.0.0/UCD.zip`
- `https://www.unicode.org/Public/emoji/16.0/emoji-sequences.txt`
- `https://www.unicode.org/Public/emoji/16.0/emoji-zwj-sequences.txt`

Do not replace the table with `latest`: Unicode-version changes are observable
through JavaScript `RegExp.test()` and must be upgraded together with the
declared Node/Playwright compatibility baseline and regenerated conformance
corpus.
