# E-M015-002: generated module names retain token resolution editions

A declarative macro defined in Rust 2021 emits `mod gen {}` into a Rust 2024
caller. The pinned macro engine and Items parser accept the expansion. Its bare
`gen` token retains a 2021 resolution context and the defining unit. LRR's cold
declaration scanner nevertheless rejected the name with `LRR-E-SOURCE-NAME`
because it used the caller's parse profile. The binding factory was never reached.
This is an implementation contradiction with the revision-4 token-edition contract,
not a language-profile change.

Generated declaration admission now attaches editions from the canonical owned
hygiene contexts through the exact materialized origin intervals. Attachment checks
contiguous coverage, identity lookup and bounded allocation. Physical source
location editions are independent of resolution editions. Original source keeps
its recorded parse profile. Module rows encode their checked name edition, and
owned module binding admission uses that edition. The explicit stable encoding
domain advances to `lrr/v2/declaration-skeleton-with-scoped-module-bindings-and-token-editions`.

The original failing witness is preserved in
`artifacts/macos-first/m015-module-token-edition-witness-v2-failure.json` and its
immutable source snapshot. The private V3 candidate passed two mixed-edition
regression, 173 library controls, 65 declaration-scan controls, seven enclosing
controls, formatting, strict workspace Clippy and the architecture audit. The V1
failure preserving the old encoding-domain assertion is also retained. A further
regression relocates the emitted tokens to the 2024 call site while preserving
their 2021 resolution contexts; it passes in the V3 receipt.

Acceptance is limited to cold generated declaration scanning and owned module
binding facts. Runtime outline edition propagation, annotation completion,
namespace route publication, full stock fixtures, full M015, guest JIT, Linux and
release acceptance remain pending. Main milestone integration is still required.

Component receipt: `artifacts/macos-first/m015-module-token-edition-v3-acceptance.json`.

The item-arena extension now attaches the same checked origin/context edition
map to ParsedFile::from_generated_items. Actual mixed 2021 definition and 2024
call-site tokens retain distinct editions under the live2024 profile; the strict
witness and compiler-lint failures remain preserved. Joint component checks pass
179 library tests plus the original/new upstream catalog controls. This is owned
syntax metadata, not guest or generated module-route acceptance. Fresh integrated
milestone pending; see m015-arena-panic-joint-v1-acceptance.json.
