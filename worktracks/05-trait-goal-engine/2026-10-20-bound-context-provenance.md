# M020 / LOCAL-049 — preserve bound context before shared lowering

Invariant: trait default bounds inspect actual colon bounds, while type
parameter defaults additionally inspect matching parameter where bounds. The
current Q78 flattens both before Q84, changing the pinned compiler answer for
`trait Inject where Self: Sized { ... }`: implicit MetaSized must remain beside
the written Sized. Unchanged official witness:
`higher-ranked/trait-bounds/normalize-under-binder/issue-62529-1.rs::Inject`.
The already-imported delegation/free-to-trait-pass test_11 is a runtime witness
of the same distinction but has a separate unadmitted delegation grammar gap.
No original is edited or replaced with a synthetic declaration.

Task WT05-02 / M019–M020, source baseline f4a74d3bf0. Lead owns the ordinary
source_types::actual_declaration_bound_syntax collector, frontend/source_predicates
common lowering/default-input views, Q78/Q84/Q82 integration and registrations.
Worker Q88 consumes the shared filtered preparation; independent facade worker
owns only comparison trait source/pin provenance; validator owns new upstream
controls and asynchronous validation after a complete source freeze.

Shared route: admitted original/generated ParseFile -> DeclarationGenerics /
DefinitionHeaderPlan -> WrittenTypeDeclaration + the existing declaration-bound
collector -> one bound lowering path -> source recipes/common upstream encoder.
Attach actual site tags during collection (trait colon, parameter, where,
associated output) and retain their source/owner/parameter identity beside the
recipes. Sealed borrowed default-input views select from this retained plan.
TraitSelf reads colon only; TypeParameter reads its actual inline/where rows;
associated output reads output rows. Remove the flattened-slice Q84 interface.
Q78 keeps every explicit written row, including pending rows, independently.

Filter before resolving unrelated source bounds. SelfOnly and
SelfAndAssociatedTypeBounds share this collector and lowering; they are not
another parser or trait solver. Actual nested associated constraints stay
pending until common member/projection/binder transport exists; do not discard
equality or bound rows to make a compiler-query answer ready. Q88 is immediate
source preparation, not a transitive closure certificate or permission to run
an upstream callback with missing facts.

Cold cost: one actual bound-site walk, source resolution only for selected rows,
retained recipe/site metadata and pure query memoization. Defaults select a
borrowed view; no repeated parser pass. Execution/callbacks read frozen values;
no body/index clones, global runtime namespace census or installed planning.
Currentness remains real source/scope/edition/epoch/query dependencies; no new
source-access sandbox. No static compiler rejection or runtime resource cap.

Acceptance: unchanged original spans/trait Self identity and both expected
MetaSized/Sized rows; relevant type parameter/default/associated output
regressions; filtered unused-bound coldness; all four editions and original,
user macro and wrapper origins; memo and epoch/source invalidation; pinned
encoding route and no body/signature preparation. Runtime repr128 remains the
required real integration witness. Full predicate families, fact publication,
root-bound readiness/inferred answer export and runtime/member migration remain
open. Existing macro audit MA001–MA008 and the eight recurring failure modes
are separate ledgers; a final fingerprint-bound source pass must inspect all
eight before checkpoint, with actual terminals and unexecuted gaps recorded.

Ordering follow-up before validation: shared `PreparedWrittenBounds::with_default_sets`
prefixes actual TraitSelf defaults before the colon rows. Shared
`order_explicit_sized` uses Q84's authenticated language identity and inserts
each explicit Sized at the start of its actual owner/site list (preserving
repeated-bound span order as pinned front insertion requires). Q78, Q82 and Q88
use this retained preparation; no runtime sort, source parse or spelling test.
Pinned policies: `hir_ty_lowering/mod.rs:1055-1068` and
`hir_ty_lowering/bounds.rs:125-137`. Existing unchanged witnesses are
`traits/default-method/supervtable.rs::Z` and
`union/union-manuallydrop-rpass.rs::ImpliesCopy`. Default vector order remains
Q84's own compiler policy. Cold insertion can be quadratic within a list, as
in the pinned compiler; no measured performance claim.

Explicit remaining hold: Q84's `rustc_no_implicit_bounds` early return does not
prepare optional language receipts. A positive written list without an actual
Sized-role receipt stays `DefaultBoundsPending::LanguageIdentity`, rather than
claiming completed compiler order. Preparing that role independently remains
open. Legacy non-compiler consumers still read raw recipes and are not claimed
complete compiler-query producers. No production runtime root is enabled here.

Second review refinement: the initial per-predicate grouping was correct only
for Q78 declaration clauses. `collect/clauses_of.rs:988-1046` shares one vector
across filtered generic predicates. The retained common plan now records
`BoundOrdering::{Declaration,TraitProbe}` from its actual collector caller;
`BoundList::GenericProbe` coalesces only filtered inline/where rows. Colon
bounds/defaults remain a separate prefix. All prepare_written_rows call sites
select the actual domain, encoded into the immutable plan. Cross-predicate
filtered ordering is source-reviewed but unexecuted: no suitable repeated-Self
trait header was found among the imported originals. No synthetic replacement
fixture or complete-query/architecture claim is made.
