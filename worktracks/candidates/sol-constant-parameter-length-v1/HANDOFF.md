# T054 parameter-independent array-length source admission

Status: source implementation and preregistered controls only. No Cargo, compiler, guest, target, canonical source or frozen predecessor mutation. No runtime fixture passes. Root must collect an immutable macOS Rust1.99 build/test/milestone terminal before integration claims.

## Scope and invariant

Only `syntax/pure_constants.rs` and new private `syntax/constant_parameter_lengths.rs` production paths. Existing parse, capture, array_length, plan and evaluator interfaces remain compatible. The legacy content-only API still grants no current source/namespace authority. No DB/provider/evaluator/IR/VM changes or unchecked public proof factories.

A reached ArrayLength under an exact original, non-generic module function may mint an opaque source plan only after a closed AST independence proof. The helper captures potential parameter names from the actual parameter pattern (including raw/destructured forms); this is a conservative shadow set, not guessed Binding/pattern/prelude authority. It scans the selected length alone. Literal, parenthesis, nonassignment primitive operators, Type-certified primitive casts, and exact unique direct module const/const-fn roots are allowed. Every root is selected from the existing source map, attributes and exact declaration/context, with the current producer's published Value seal joining that map. A potential parameter match refuses *before* module lookup. Module/local imports, unresolved or qualified roots, projections, generic/self/where/local-fn contexts, unestablished patterns/attributes/control/borrow syntax remain typed prerequisites. Existing preceding-local blanket guard remains unchanged; this slice does not establish body-local binding semantics. Existing privately admitted static panic operands are accepted only through their genuine Node proof, never spelling.

No callee BodyStructure/Thunk query or unentered body operations are inspected. Function body range is used only to bound the exact written header; array repeat element is not read. Exact root declaration identity is captured, but its initializer/callee effects/type errors still occur only when actually reached by the evaluator. Independently static lengths require no lexical caller-argument evaluator hookup; parameter-dependent lengths remain blocked.

## Identity/currentness/budget

The helper privately mints `ParameterIndependentLength` and directly folds its digest into the opaque plan identity; no extra retained heap table or public accessor is introduced. Digest domain `lrr/v1/parameter-independent-constant-length` binds exact declaration/context/profile/cfg, original header range/text hash, reached site, canonical potential parameter/referenced names and exact root-definition footprint. The immutable existing source lease retains the original AST/header. Current ConstantSourceAdmission revalidation recomputes this proof against actual retained source/Type/Value/macro evidence and current published input stamps, including epoch/edition/source changes.

Source plan domain changes `lrr/v3/pure-constant-source-plan` to `lrr/v4/pure-constant-source-plan`. Old semantic plan/cache identities must not be accepted as current; existing exact plan-identity keys/replay provide invalidation. This introduces no wire value shape/tag or persistent reader. No checkpoint/restore acceptance is claimed. One mechanical `StableEncoder::option(&Option)` correction matches root's actual compilation finding; skip that already-repaired hunk when applying to the canonical successor.

Checked work <=100,000; <=256 params and <=256 unique names per set; spelling <=4096 bytes before name allocation; evidence plus copied original header <=64KiB; pending AST worklist <=64KiB and each argument list <=256. Roots are charged before insertion; exact header copied only after its checked bound, never full source text. These are closed transient producer guards; retained source/admission metadata remains charged by existing current producer. No VM ticket/value/descriptor recovery is involved.

## Patches and root attachments

Apply production.patch as a two-path successor to the frozen macro-compatible pure source baseline; root's Rc/SelectedEntry/Clippy mechanics are preserved. Root's concurrent evaluator/query/memo changes must remain intact: review/apply hunks, do not replace canonical files wholesale.

acceptance.patch adds ten controls under actual current result-type fixture helpers; attachment.patch attaches that child. expectation-migrations.patch changes exactly two earlier literal-parameter-header tests to require actual current success/replay/Usize readiness (previous blanket prerequisite intentionally removed). Negative parameter/raw/destructuring/name-shadow cases live in the new test child; no error swallowing or failure-as-success.

complete.patch is an alternative exact-baseline aggregate, not an additional patch after applying the split set. Root's already-repaired test NeedExpansion/NeedGeneratedExpansion arms must remain split because request types differ; the new child consumes existing genuine world helpers and does not reintroduce those arms. Root also retains its actual OutlineKey stable encoding/sources metadata fixes. Neither frontend child nor producer queries a provider.

originals.patch contains pristine sources that were absent from MAIN at capture; root should use its unchanged-source importer and update manifest/ledgers, deduplicating any concurrent imports. Both upstream licenses are preserved separately. No compiler directives/revisions/source edits. Fixtures use checkout ae0fe511b15ab760d7e963b13389d1bc2ab42b18; this is not installed Rust1.99 b940084d7e library/native authority.

## Acceptance pending

Unchanged `const_let_irrefutable.rs` [a,b] parameter/[i32;2] and `min_const_fn/cast_fn.rs` x:&[u8;3] exact lengths in all editions. Unchanged run-pass `mir_codegen_array_2.rs` into_inner(x:u64) admits header1024 and repeat2*4*8*16 without element x/body work. Unchanged run-pass issue-70773 selects HASH_LEN through exact wildcard-parameter header and module declaration in all editions. NRVO init's inline attribute remains an asserted explicit frontier; its zero-param return header is an unchanged control. No full original main or language/compiler fixture acceptance is claimed.

LRR negatives: parameter/raw/tuple/slice potential shadows, preceding local/import/qualified/generic/where/unproven pattern/absent/alias-cast/local fn, cold callee nonentry, source/epoch invalidation, wrong owner/nonlength range, >256 params, >64KiB exact header, current-admission quota. Actual provider trace remains unchanged in genuine current controls. Result-type and actual parent child-reselection controls are attached separately through two precise migrations.

Actual tooling: pinned Rust1.99 rustfmt --check exit0 chunk980469; private exact-baseline git apply --check complete.patch exit0 chunk2f9c06. Upstream HEAD read ae0fe... chunk1e2fc3. These are formatting/textual checks only; all ten new tests and two changed controls are uncompiled/unexecuted.
