# E-M015-014: original doc comments keep their lexical attribute positions

M015/revision4 section49.3 with the retained original-source attribute pipeline:
procedural input preserves the ordered inert/doc attributes and remaining macro
annotations after removal of only the selected transformation and resolved
compiler prefix. Source trivia lowered by the pinned converter must remain at its
original lexical position. Token display text grants no reconstruction authority.

The unchanged official proc-macro/attributes-included source and auxiliary
provide the witness. The auxiliary's first bar callback expects inline/doc/foo/
inline/doc before fnfoo, and the second foo callback expects inline/doc/inline/
doc before replacing literal"foo" with3. On the independently tested baseline,
firstbar executed and panicked: expected three tokens in doc. Instrumentation
showed both docs had valid three-token shapes. The actual order was inline/foo/
inline/doc/doc, so the auxiliary inspected the remaining foo annotation where a
doc annotation was expected. No expansion record was admitted; later foo/output/
catalog/filter assertions remained unexecuted. No selected live consumer compiled,
and body queries remained0. V3 and V4 strict failures and the earlier mechanical
observer compile failures are preserved separately.

The original materializer enumerated explicit ast::Attr facts before appending
other item tokens. Doc comments lowered into #[doc=...] are not ast::Attr rows
in the original raw source, so that assembly silently moved them behind explicit
attributes. Generated callback output already has explicit doc attribute tokens;
its continuation is a distinct path requiring an actual acceptance callback.

The parent installed unchanged-source and supplemental cfg_attr interleaving
regressions before correction. Their strict source-order assertions both failed on the unchanged baseline
and both passed after the correction. The correction replays each resolved
explicit wrapper at its original token position, retaining intervening doc token
subtrees, spans and contexts exactly. Disabled cfg wrappers and applied/compiler
transformations remain omitted; effective cfg_attr metas remain ordered within
their original wrapper. Pinned doc lowering, parser, hygiene, grammar and explicit
macro authority remain unchanged. Private validation passed196 library,30 adapter,14 attribute,67 scan and61 source
discovery controls plus formatting and strict Clippy. Five unchanged native
original/derive controls passed separately. Both parent and independent worker
passed the final source/native harness: two actual callbacks bar→foo, two accepted
records, correct interleaved inputs/outputs, original invalid literal replaced by3,
current sealed selected-test identity, repeated reads and known/absent filters,
with zero body/signature/thunk queries or macro reexecution. The initial corrected
harness reached successful bar output but lacked the public generated continuation;
that partial failure is preserved. Its corrected observer records the actual
NeedGeneratedExpansion and retained parent request before using the existing
MacroRun::fulfil_generated_discovery path; no production authority was broadened.
See artifacts/macos-first/m015-required-admission-repair-attribute-doc-order-native-final-v1-acceptance.json and
artifacts/macos-first/m015-proc-attributes-included-v6-acceptance-worker-delivery.json.
Main integration and fresh milestone remain pending; this is no stock fixture or
full M015/JIT/release/Linux pass.

Fresh integrated macOS milestone passed 2046 host/doc checks with3523 fixed source/include inputs and membership/all9checks0. Receipt artifacts/macos-first/m015-original-doc-attribute-order-main-acceptance.json; full stock/JIT/release/Linux pending.
