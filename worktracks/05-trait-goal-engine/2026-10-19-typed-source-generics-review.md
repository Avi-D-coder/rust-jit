# Typed source generics — independent solo source review; gates held

Baseline c388c7754cb28c974627058cc6852b07e86b1ada. Reviewed V333 source
fingerprint `3eb94e867b4573c806c8950fd61fbc3470f7cacc5708bdeff3dc2d1d4eb8cff1`.
M019/M020, LOCAL-049, E-R5-014. Validation is pending at this review;
attach actual terminal evidence separately. This is source metadata/IR work,
not upstream goal evaluation or completed runtime feature acceptance.

The unchanged `type-dependent/issue-61936.rs` exposed two distinct failures:
V330 attempted to resolve an inherited const Type using the child method's
lexical range; V331 treated the impl's member scope identity as its declaration
ID. V332's original-source control passes after retaining the genuine original
associated-owner join. V333 adds arbitrary emit/wrapper source contexts; their
acceptance remains pending until the actual control terminal. The originals are
unchanged; temporary wrappers live outside the imported tree.

## All eight recurring failure modes (separate from MA audit numbering)

1. **Separate assertion/expression evaluator:** no expression compiler, runtime
   evaluator, VM operation or assertion handling changes. Ordinary
   `DeclarationGenerics::prepare` -> `ParsedFile::declaration_generics` and
   `declaration_const_type_ranges` capture actual header syntax. New
   `DeclarationGenericTypes::prepare` uses `WrittenTypeDeclaration::declaration`
   -> `resolve_actual_type`, the existing shared Type walk. MA001 and its broader
   removal obligations are not closed by this work.
2. **Literal/break-only macro execution:** none added. `GeneratedSyntaxInput::
   prepare_using` extends its existing admitted Outline/GeneratedHeader join
   with a scoped member-container -> actual generated declaration map. Reads
   use the actual `GeneratedSyntax` input dependency and exact ParseKey. This
   map supplies source identity, not runtime semantics (MA004/MA007/MA008).
3. **Context-specific method/numeric behavior:** `DeclarationParameters` is the
   single source/upstream index transport used by `SignatureParameters` and
   actual declaration/impl plans; the separate `SourceParameter::from_impl`
   algorithm and constructor-only argument-kind ordering are removed. Trait
   Self is upstream index0 and retains its actual source ordinal. Const Types
   use the same `source_terms::Encoder` as impl Types. No macro/method spelling
   chooses a Type or solver answer. Supplemental host tests name real fixture
   declarations only to inspect them; production has no fixture names.
4. **Generated-source exclusions:** admitted generated and original parent
   metadata join the same inherited Type path, with only their actual source
   receipt transport differing. Original member scopes use retained
   `PreparedScopeImplHeaders::declaration_for_member_scope`; generated scopes
   use retained `GeneratedSyntaxInput::declaration_for_member_scope`. Parent
   layout, kinds/names and header context are checked before the existing
   Type/array-const parameter leaves rebase to child source ordinals. There is
   no primitive-name fallback, synthetic function or generated-only evaluator.
   Missing source/late binders/unsupported symbolic Types stay typed pending,
   never a semantic negative trait answer or runtime pass.
5. **Patterns/eager executable checks:** no pattern machinery changes. Demands
   read declared const parameter Types, not return/body expressions; unused
   function generic-const bodies and `next`'s unsafe operations stay cold. The
   source trace assertions check no FunctionSignature/BodyStructure/Thunk/
   PatternPlan demand. This does not certify full MA002/runtime cleanup matrices.
6. **Repeated parsing/planning:** Q70 `DeclarationGenericTypes` is a pure memoized
   query keyed by the actual definition and catalog seal. It retains Q69 and
   the real source environment; syntax locations come from retained parser
   indexes. `SourceBindings` follows only demanded declarations and referenced
   const-Type metadata, not every raw catalog Type. `SourceGenericLayouts`
   retains query Arcs and source/upstream parameter plans. Context construction
   materializes typed Generics once; callbacks borrow indexed immutable facts.
   No installed operation or ordinary VM dispatch re-enters this preparation.
7. **Full-world scans/unrelated invalidation:** actual catalog preparation already
   visits source headers; the new original parent map is built in that existing
   pass and filtered to active owners. Generated parent rows are built in the
   existing admitted Outline loop. Parent and generic callbacks use scoped/ID
   maps, not namespace-map construction/comparison. Whole catalog identities
   remain cold context dependencies; final per-declaration/transitive currentness
   closure and cached runtime answer invalidation remain integration obligations.
8. **Immutable copies/whole-file node search:** parser/query/environment objects
   stay shared through Arc. New indexes contain scalar IDs/scopes/parameter
   plans and direct Type syntax ranges; no BodyIndex copy or whole-file search
   is added to each phase. Small parameter lists are sorted/copied once in cold
   source-context construction. No new budgets/accounting/sandbox/static checker
   or unsafe code is introduced; inherited metadata accounting is not certified.

## Actual upstream callbacks and lifecycle

`PreparedProvider::generics_of` serves the context-owned typed table.
`Args::identity_for_item` consumes it. Full `publish` calls
`require_generic_value`, comparing exact actual parameter source identities,
indices, parent count and const Types rather than count/kind alone. Host controls
exercise that same validation method with changed parent counts, const Types
and another real declaration's same-kind/same-index parameter.
`with_provider` uses the existing ScopedCache owner guard so typed generic lists
are destroyed before the interner/context; no new unsafe ownership route.
`ready` explicitly rejects absent full facts. A generic table is not a ready
solver and cannot authorize `evaluate_prepared`.

Cold work is proportional to demanded declaration/const-Type closure, existing
catalog header preparation and parameter lists (ordered-map lookups; per-plan
sort). Materialization interns each retained parameter list once per context.
`generics_of` is an ID-map lookup; source parameter lookup is an ID-map lookup
and direct ordinal access. These are structural observations, not measurements
of release speed/scaling. Source/body preparation and solver root costs are not
benchmarked here.

## Remaining holds and next actual integration

No `PreparedFacts::new` producer, real compatible inference delegate or actual
root evaluation is supplied. DefinitionFacts/ADT/trait flags, predicates,
ParamEnv, lang items, parameter DefIds, function early/late binders, arbitrary
symbolic const Types and full transitive/currentness closure remain necessary.
References are explicitly pending in this symbolic path rather than accepting
the legacy runtime-erased region representation. Existing held runtime trait
heuristics/custom inference are not promoted or expanded. M019/M020, all MA audit
closures, interpreter correctness/runtime acceptance, JIT and Linux stay open.
No new distinct stock runtime gain or refreshed suite total may be inferred from
source/IR constructor checks. Unsupported/unexecuted requirements remain gaps.

Final V333 validation: library75948, CLI14418 and controls12114 all terminal exit0.15/15focused controls pass; final source/HEAD/test/CLI guards match the reviewed fingerprint. New source matrix has20worlds/48generic rows/36const parameter rows/24early-region rows, original+arbitrary emit+wrapper all4editions. Actual140head/136TraitRef/48world and existing runtime/projection/cold/order/cleanup regressions pass. The scoped-cache host control tests its existing destructor guard, not complete upstream cache semantics. No production source changed after this review. All remaining holds above stay open; this is not a complete milestone, real solver invocation or new distinct runtime gain. Exact earlier failures and final commands/terminals are retained in worktracks/evidence/2026-10-19-v328-v333-typed-source-generics.json.
