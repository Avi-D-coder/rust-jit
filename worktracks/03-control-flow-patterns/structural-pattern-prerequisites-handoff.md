# Structural pattern prerequisites — bounded architecture record

October 15, 2026, WT03/T050/T051/H003, LOCAL-024 coordination. This successor
starts after immutable cumulative70 source-plan packet
`1ddb4fe21f6d145e5afa2a73797cd2de1f0fb332264a285da6508511884c6112`;
that source and its pending validation remain unchanged.

Required invariant and primary unchanged witness:
`fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs`
must enumerate tuple-contained or-pattern products from left to right, including
dummy wildcard alternatives and @ bindings, evaluate the scrutinee once, and
retry after each false guard without consuming projected source fields. The
original fixture's eight-result and dummy-guard-count matrices remain the actual
runtime target. Its licensed original and provenance already reside in the
coordinator-owned manifest; no rewritten original or scalar substitute grants a
fixture pass.

The ordinary source route already attaches ParsedFile/BodyIndex through
PatternSourcePlan, selects shared scalar tests, verifies Op::Pattern and executes
Frame/Transactions with ordinary scope cleanup. Shared prerequisites found by
inspection: literal_match still requires primitive scrutinee context; structural
tuple source/member projection is unsupported when an actual TyTerm::Tuple is
present in Frame::prepare_projection; scalar binding namespace admission and
local lookup currently recognize only one direct arm identifier; BodyIndex node
attachment recognizes only scalar pattern shapes. Nested products also require
retained branch/leaf continuations rather than flattening only top-level OrPat.

Extend the existing source type/field/place mechanism before admitting structural
patterns. A source TyTerm::Tuple must select members by ordinary source ordinal
and verify the actual descriptor against that retained source; descriptor shape
cannot invent a source type. Projected pattern tests read intrinsic Copy fields
without consuming them, through the same GuestStore place checks as ordinary
field reads. Pending bindings retain checked projection identities and existing
transaction state, and only preview/commit publishes values. No source AST walk,
layout lookup or guest evaluation may occur inside installed pattern dispatch.

The complete nested-or route must preserve source product ordering and actual
source/owner/alternative identity; it cannot assign macro-only PCs, reparse a
display string, or execute a private pattern evaluator. Root owns BodyIndex,
thunks/source-local identity and hygienic namespace joins. Reference peer owns
runtime/cell.rs, context and source Type proof APIs. This worker owns retained
pattern/lowering leaves, ordinary Frame source projection and VM transaction
projection hooks after narrow interface coordination. Missing shared APIs must
be agreed before edits to another owner's files.

Cold work remains checked/bounded retained structural source preparation;
Cartesian alternatives require checked product sizes and typed outcomes rather
than unchecked eager expansion. Hot projection work is bounded by actual source
projection depth, with accounted address/state handles and ordinary cleanup.
Required obsolete routes are the primitive-only pattern-source refusal and
scalar-only direct binding checks once their sound shared replacement covers
the required source forms; they remain named holds until replaced, never bypassed
with descriptor or spelling authority. Existing scalar paths must continue
through the same plan/transaction implementation.

Closure needs actual unchanged stock runtime matrices, ordinary/generated/user/
wrapper/standard macro route evidence, exact-once effects, false-guard cleanup,
suspension/panic/source-currentness and all-eight source review on exact frozen
bytes. A shared prerequisite is a checkpoint with explicit remaining integration
work, not completion of structural patterns, T050/T051 or architecture acceptance.
This worker runs no Cargo/native tests, spawns no agents, publishes no commits or
ledgers; the existing sole validator owns exact-source execution.

## Reviewed prerequisite checkpoint (source only)

Exact reviewed implementation fingerprint:
`37e38ba0474f57a5a326b750a159b208e6ec95157e4d48cf3017a3b61a4595b0`.
The 14 sorted file hashes, byte lengths, encoding and source baseline are in
`worktracks/evidence/2026-10-15-structural-pattern-prerequisites-source.json`
(manifest SHA-256 `6204600c36c9a32dafdaf2d78856368cb7e36e54a5fb31771c8de5143e7b34c0`).
This list includes the coordinated full Frame/Step leaves, including the reference
worker's narrow prepared parameter/result-source integration. It also includes
literal_match's `.clone()` mechanical correction from the coordinator's earlier
failed compiler packet; that line is not structural-pattern acceptance evidence.
The source does not include or replace any frozen validation overlay.

Implemented ordinary prerequisites:

* `PatternBindings::prepare` creates retained canonical *candidate* groups once
  during BodyIndex preparation. It records actual normalized hygiene classes,
  owning token editions, written ref/mut modes, @ occurrences, alternative absence,
  conjunction duplicates and unresolved context. Bare identifiers retain
  BindingOrConstant uncertainty. Equal spelling is not binding authority. Stable
  encoding enumerates all semantic fields explicitly; checked metadata and work
  budgets cover source rows and preparation scratch. Structural wrappers do not
  create duplicate pattern roots. The coordinator embeds these facts into its v5
  BodyIndex encoding and consumes them in ordinary lexical candidate lookup.
* `Frame::require_structural_source_target` verifies already-admitted primitive,
  tuple and closed array source terms against actual runtime descriptors. It never
  derives a source term, nominal definition or Type-path meaning from a TypeId.
  `prepare_projection` now selects a tuple member from its actual retained source,
  reusing ordinary GuestStore place/lease/owner checks. Unknown nominal tuple
  members and unresolved symbolic source forms remain explicit gaps.
* `frame_source_storage.rs` measures actual closed-term heap without allocating a
  traversal stack. The existing two-node per-view reserve remains the fixed floor;
  checked excess covers Vec capacity and recursively owned boxes/terms. Slot,
  binding, parameter and return publications, clones, call batches, projections,
  source-aware borrow/deref and owning reads preflight their applicable budget.
  Place transfer moves its charge; both synchronous and bounded scope cleanup
  release annotations, including annotations whose guest cell never initialized.
  Returned source metadata remains owned after local cleanup. Current call-progress
  storage is included alongside other auxiliary Frame metadata.
* `PreparedConstructor` and `ActiveConstruction` retain immutable Arc field-slot
  rows from the actual admitted Tuple ConstructorPlan. Row bytes use the ordinary
  constructor mapping budget and cleanup lifetime. Active-state cloning shares
  these rows. Ordinary field transfer retains its actual source annotation on the
  moved slot; finish requires moved field cells, checks each retained source against
  the completed descriptor and builds a tuple source only from those source facts.
  Missing/unsupported member facts leave the source unresolved; runtime layout
  supplies no replacement authority. Publication preflights before guard commit.
* Ordinary aggregate Copy retains one checked source view in `OpActivation`,
  reserves its heap and destination publication against the Copy-machine budget,
  checks its identity on each resume and moves it on completion. The old
  post-completion fallible clone could fail after an owning ticket escaped; that
  route is removed. No copy/preview source owner is consumed merely to test it.

The unchanged primary original still hashes to
`38723ec8521136188a19a4b465362e979d95f59ed65009a358aff640a23a08b6`.
There are four canonical source-fact test bodies (including all four editions)
and eight Frame source/storage/constructor component bodies outside the original.
The Frame controls include mismatch/missing authority, checked quota and retry,
complete call-source batch refusal, place charge transfer, bounded cleanup yields
and return metadata lifetime. One actual inner tuple source constructor from the
original executes its ordinary literal recipes in the control setup and checks
source-preserving Copy/resume authority. It is explicitly a constructor component;
no helper substitutes an implementation of the original search/guard program.
Dummy aggregate graphs in projection controls are labelled setup, not source
execution. Invalid-Rust source-conflict controls are negative authority evidence,
not accepted Rust contracts.

## Separate all-eight source review

The recurring failure-mode numbers below are AGENTS.md's eight modes; they are
not the separately numbered MA audit findings. Applicable audit debt includes
MA002 (split patterns), with MA001/MA003/MA004/MA005/MA006/MA007/MA008 remaining
coordinator-owned architectural holds rather than implied closures here.

1. **Independent assertion/expression evaluator:** inspected new source helpers,
   `prepare_projection`, constructor preparation/finish, `begin_aggregate_copy`
   and `advance_aggregate_copy`. None dispatches on a macro/assertion identity or
   evaluates an AST. New production code handles source facts and ordinary VM
   state only. No MA001 closure is claimed; inherited held routes remain named.
2. **Literal/break-only generated output:** `PatternBindings::prepare` accepts
   ordinary indexed Pat locations of all structural kinds. Constructor facts come
   from the same admitted ConstructorPlan used for handwritten/generated syntax.
   This slice adds no generated executor, special PC or literal-only dispatch.
   Actual arbitrary-user/wrapper/standard-macro executions remain unexecuted.
3. **Context-specific methods or numeric constraints:** source correspondence
   recognizes intrinsic primitive identities and actual admitted structural terms.
   It does not select methods or derive numeric inference from a macro name,
   guard context, descriptor spelling or fixture name. Nominal/member authority
   remains in the existing source-proof service/reference-worker prerequisite.
4. **Generated-source rejection:** helper classification uses actual token edition
   and `ParsedFile::local_name_class`; incomplete evidence yields Unresolved
   occurrence-specific candidates. There is no original-source-only exemption.
   Generic generated authority gaps are held, not treated as supported contracts.
5. **Split patterns/eager cold arms:** canonical candidates and source projection
   extend ordinary BodyIndex/Frame paths consumed by the retained plan. They do
   not introduce a pattern interpreter. No executable result arm is inspected by
   these prerequisites. The primitive-only decision lowerer and structural
   transaction-product integration are still held; MA002 is not closed.
6. **Repeated planning:** source facts are prepared once in BodyIndex through the
   query path. Runtime type membership/heap/owner checks inspect retained terms
   and current guest facts; they perform no parsing, source traversal, lookup or
   pattern-plan reconstruction. Constructor finish may repeat bounded membership
   preflight, and Copy resume compares retained source terms; these are runtime
   guards, with no speed claim. The coordinator's older conditional lexical
   `condition_plan` preparation remains separately tracked debt.
7. **Full-world scans/unrelated invalidation:** no new namespace census is added.
   Groups are keyed by actual local normalized hygiene class and name; bare
   candidate namespace classification remains the reached source service's job.
   Runtime operations access only current retained source terms and ordinary
   places. BodyIndex/source-plan memo dependencies continue to supply source,
   body/profile/currentness invalidation; this worker does not alter DB guards.
8. **Deep immutable copies/whole-file node search:** PatternBindings attaches
   exact indexed ranges through `entered_node_syntax`/`syntax_at`, and occurrence
   lookup uses `nodes_at_range`. Root detection follows bounded indexed ancestry.
   BodyIndex is retained/shared; constructor active-state clones share Arc rows.
   Runtime owned TyTerm clones are bounded/accounted source-view ownership, never
   BodyIndex/parser copies. Canonical cold preparation is bounded by 100k work,
   depth256 and actual metadata; group/occurrence selection uses sorted rows.

Cold work is canonical source-fact collection and constructor operand-row capture.
Retained binding metadata is O(pattern occurrences + names/groups); range/group
lookups use the existing sorted indexes. Runtime source cloning/accounting and
membership checks are O(reached retained term nodes), depth/work bounded; tuple
finish is O(reached fields and their retained source terms). Frame operations do
not parse or query. These are source-level cost statements, not measurements.

## Held integration and reproducible validation plan

Still pending: structural Tuple/@/nested-Or decision plans, ordered product
alternative PCs and false-guard retry through the same transaction engine; source
binding/constant admission for multiple canonical groups; all binding-mode and
non-Copy preview contracts; full original guard-count matrices; ordinary/user/
wrapper/standard macro route matrix; actual panic/suspension/currentness/cleanup
integration and milestone evidence. No task, architecture or fixture is closed.
The coordinator's current BodyIndex/node attachment/lexical consumer and the
reference worker's context/opaque source proofs are required combined-source
joins; their exact bytes must be frozen together before validation.

Sole-validator command plan (not run by this worker): macOS Rust1.99.0 selected
frozen overlay, one target owner; run `syntax::pattern_bindings::tests::` (4
bodies), `vm::frame::source_types_frame::tests::` (8 bodies), then the existing
`vm::frame::pattern_tests::`, retained pattern-plan/query banks, ordinary Copy/
constructor/cleanup and reference source banks. Run strict and milestone gates
only on the exact combined frozen source. Preserve terminal failures with their
original fingerprint and compile a new successor after corrections.

Actual local terminals: rustfmt of authorized leaves exited0; `git diff --check`
exited0. No Rust/Cargo/native execution, no target mutation, no extra agents,
commits/push or ledger completion by this worker. All 12 new bodies are uncompiled
and unexecuted at handoff; upstream runtime-contract and full-fixture passes are0.
