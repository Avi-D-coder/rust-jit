# Separate source review — shared declaration table/import/prefix kernel

LOCAL-049, solo /root; parent 6fe2db1686f8f034bad19bad77f7ac26c4eefe9f.
Reviewed V286 source fingerprint:
0dc71cd87a30cb90bfad4001940d9d11a1836d696d35ab836927d6f10053cfce
(8,481 inputs). Recheck affected conclusions if these sources change.

## Actual integrated route and removed duplicate location

`EnteredMacroIndex::local_index_inner` now calls
`DeclarationNameTables::prepare` for the ordinary declaration/import tables.
`SourceLocalTables` retains that one immutable table allocation with its runtime
binding/scopes/context data; PreparedSourceLocalPlan retains its existing Arc.
`SourceLocalIndex::declaration_index` creates only an O(1) borrowed view of the
actual tables, namespace DeclarationFacts, owning module and edition.

`SourceLocalIndex::lookup_prefix_inner` preserves exact source/site/entry,
header occurrence, macro lookup, runtime binding and generic-owner checks. Its
item/import tail calls `DeclarationNameIndex::lookup_prefix`. Runtime Value
binding selection is supplied by the existing checked LocalScopes result;
Type item comparison invokes the actual pinned item-context lookup lazily at
the same point as before. An initial eager callback preparation was corrected
before runtime validation, preserving pending-frontier/error ordering.

The actual `anonymous_import`, `import_routes`, `named_import_routes`,
`glob_lookup` and `import_lookup` bodies moved out of SourceLocalIndex into
DeclarationNameIndex. Remaining SourceLocalIndex forwarding methods borrow that
same kernel; they contain no second import algorithm. Scope ancestry, strong/
weak import priority, cycles, ambiguity, pending and per-call ImportMemo remain.
A separate review compared 4,816 normalized tokens of those five ported methods
against the parent: identical after receiver/module/edition/visibility and
formatting substitutions. This is source-review evidence, not runtime acceptance.

`LocalTypeNames::prepare` now uses shared `capture_items` and `finish` for its
actual original/generated/parent declaration occurrences. Real parsed syntax,
header tokens, owning editions and pinned name contexts remain the inputs.
The unused item_bytes preflight accumulator is removed; no cap is raised or
added. Generic/owner capture is unchanged. No raw-root cold factory remains.

## All eight recurring architecture failure modes

1. No assertion/expression evaluator, special runtime expression compiler or
   fixture branch is introduced. This is the ordinary source-local route.
2. No literal/break/scalar-only generated executor. Existing generated/retained
   local plans use the same actual item/import traversal as handwritten plans.
3. No context-selected numeric inference or method spelling. Namespace priority
   and MacroName normalization remain their ordinary identifier/edition rules.
4. No new original-source-only restriction for supported execution. Cold owned
   Type lookup remains its pre-existing explicit hold; the new kernel grants
   no authority to bypass it. Generated cold origin/currentness joins remain
   unimplemented and are not claimed from neighboring entered-path evidence.
5. No pattern evaluator is duplicated, no body entered to prepare a declaration
   and no cold branch executable shape checked. Runtime binding selection is
   still owned by LocalScopes/entered source; only its result is passed to the
   shared item/import tail. Item-context comparison remains demand-driven.
6. Tables and occurrences are built once in the existing retained plan. No new
   parser, source-root adapter call, deep AST reconstruction or per-operation
   planner is added. Helper forwarding borrows; it does not prepare a plan.
7. QualifiedNames/ObservedNames calls are preserved in the actual port; caller
   source validation remains before them. No new namespace-world map or census
   is constructed. Existing O(scope-depth * named-import-map) traversal and
   cold import work remain; this is not a performance/scaling acceptance claim.
8. No immutable BodyIndex copy or repeated whole-file node search. Existing
   declaration occurrences use direct syntax_at/header checks. The declaration
   maps are one nested immutable value, shared by the existing Arc owner.

These recurring numbers are separate from audit MA001–MA008. MA007 actual
source/macro selection and MA008 common language/source route constrain this
slice; neither finding is closed. No legacy assertion/numeric/pattern execution
route is extended, removed or promoted by this bounded refactor.

## Required cold client and remaining integration

The shared implementation is used by the ordinary entered path now. A cold
client still needs a real published original/generated hygiene receipt, exact
Type occurrence/canonical scope, genuine producer/name-table completion,
relevant positive/negative namespace dependencies and an observed preparation
input for suspension/invalidation. It must not call source_root inside a
semantic provider, accept a raw synthetic root, manufacture a BodyIndex,
ignore producer frontiers or strip an owner to force module lookup.

`signature_names::prepare_type_name` and generated declaration Type owner guards
remain. No new standalone cold resolver is retained in parallel. The complete
32-head control stays enabled; M019/M020, actual upstream root solving, full
architecture/runtime acceptance and interpreter/JIT gates remain held.

## Collected evidence

V284/V285 library-only stages build; no runtime acceptance attached to them.
V286 library session 67115 and CLI session 73745 exit 0. Runner 46486 exits 1:
23/25 controls pass. The complete head control still fails at 20/32. The wider
local import case also fails module source/namespace/owner classification.
Exact parent source V287 (same 893e2... source hash as its earlier V283 evidence)
builds in session 16689 and reproduces that identical failure in runner 7104.
It is an existing open gap, not an extracted-kernel regression or passed case.

Production CLI session 61589 exits 0: unchanged blind-item-mixed-use-item.rs
completes all four editions, each 37 operations completed, five return scopes
cleaned and complete capture. Actual pinned bytes/directives/hash are checked.
The existing unchanged record regression completes all four editions; Copy
origin regression completes twelve worlds. Baseline stock gain unmeasured:
zero new distinct stock gains claimed. No full milestone refresh.

All V286 source/HEAD/binary guards match. After parent attribution, every tested
source file is restored byte-for-byte; V288 library rebuild session 45315 exits
0. CLI stays the byte-identical V286 runtime-tested binary. The V288 test binary
has a different hash; no V286 control outcome is labeled as a V288 execution.
No broad rerun is needed to attribute the unchanged source evidence.

[Bounded evidence](../evidence/2026-10-19-v284-v288-shared-declaration-prefix.json)
retains exact commands/selectors, hashes, terminals, stock runtime captures and
the parent-source attribution. All full feature/architecture/interpreter/JIT
gates remain held; the cold client and complete origin/currentness matrix have
not been executed or accepted.
