# M020 retained ordinary declaration header plan

Before-code record, baseline `48792aa3361d872566cebb9a91c41e1fd12fbfe7`.
Owner `/root/wf_adt_metadata`, delegated by `/root`; only new
`crates/lrr/src/traits/upstream/source_definition_headers.rs` and this record.
Lead owns query registration, source language-item contract and overall fact
factory. Validation owns Cargo. No commits, Cargo or subagents by this owner.

Invariant: `DefinitionFacts` must originate in the selected declaration's
actual source/header/context/generic facts, never caller-supplied defaults for
unprepared compiler semantics. A source `const fn` keyword or present body is
a source fact; it does not supply inferred variance, typed signature, const
conditions, hidden opaque Type or solver readiness. An unnamed impl remains
unnamed rather than receiving a fabricated source symbol.

Shared route: `DefinitionReceipt::observe` and `DeclarationGenerics` retain
actual source inputs and direct declaration syntax; `PreparedScopeImplHeaders`
supplies canonical associated-parent joins, and generated syntax uses its
actual declaration-for-member-scope join. `TraitDeclaration` supplies ordinary
trait header facts; demanded impl membership uses the existing retained
`ImplTraitIdentity` route for its actual canonical owner. The new declaration
query retains these immutable handles, source name/span/kind, generic owner,
signature Type ranges and syntactic qualifiers/attributes/opaque headers.
`SourceBindings` and `SourceGenericLayouts` provide the existing real generic
conversion context. Source and generated declarations use this same route.

No new parser, evaluator, solver, whole-body/static/borrow check, macro rule,
native tool invocation or runtime descriptor heuristic. Bodies and constant
initializers remain cold; only source header shape/presence is observed.
Actual missing joins remain typed pending or preparation diagnostics; absence
of a completed semantic producer never becomes false, empty, NoSolution or a
complete `DefinitionFacts`. Conversion exports established header inputs and
enumerated remaining semantic holds, not a fabricated complete record.

Cold cost is a bounded direct syntax lookup, ancestor/header traversal,
attribute/header metadata capture and required generic/source joins. No
runtime planning/IO, global namespace census, immutable body clone or whole
file node search is added. The completed memo's transitive input guard is
captured outside provider computation; source/currentness is checked before
conversion and retained results. No performance claim follows from inspection.

Unchanged imported source witnesses: `generics/generic-derived-type.rs`
(`Pair<T>` nominal header), `traits/default-method/bound-subst.rs` (trait,
generic impl and associated function header), and
`const-generics/array-wrapper-struct-ctor.rs` (actual const parameter Type).
`enum-discriminant/repr128.rs` remains the broader runtime witness. Additional
origin/currentness controls wrap source preparation only; they are not full
fixture, interpreter, diagnostic/tooling or JIT acceptance.

Closure evidence requires source memo reuse, actual name/span/edition/parent
and generic ownership, attributed/header qualifiers separated from semantic
holds, trait/impl membership provenance, cold bodies/initializers, source and
epoch withdrawal, all editions and original/user-macro/wrapper contexts.
All eight recurring architecture modes require a final separate source pass
against the final fingerprint. Full `DefinitionFacts`, variance/predicate/
typed signature/opaque/const/coroutine/alias semantics, genuine upstream root,
ordinary member/VM joins and interpreter/JIT gates remain held.

Implemented source-only API: `DefinitionHeaderKey`,
`PreparedDefinitionHeader::prepare`, `CurrentDefinitionHeader::capture`,
`SourceHeaderParent`, `SourceFunctionHeader`, `DefinitionHeaderPending` and
`DefinitionSemanticHold`. Accessors expose the retained actual generic owner,
source generic layout, attributed HeaderRecipe, trait header, impl membership,
opaque-return header and source ranges/qualifiers. `generic_layouts` uses the
real SourceBindings closure. `encode_header` requires that actual generic
context and exports `DefinitionHeaderInputs` (identity, optional actual name,
span, generics, proven associated parent and syntactic role/header only).
No constructor creates `DefinitionFacts`. The input descriptor carries the
same semantic-hold list; an unnamed impl never gets a synthetic symbol.

Actual associated-parent preparation uses the existing original scope join
or matching generated-unit scope join, retains the parent's real generic
query and seeds its actual identity in SourceBindings. A source associated
item with no join is `ParentJoin`, not a lexical-parent guess. Foreign and
lexical ownership remain separately tagged; their lack of an associated
parent is not a claim about rustc's full lexical DefId tree. Impl membership
uses the exact raw header selected by `header_by_owner`, then checks actual
canonical owner and parse/header/context against `ImplTraitIdentity`.
Incomplete membership remains `ImplMembership`.

Separate source-review pass against SHA256
`source_definition_headers.rs=47b7a14799fea6739e58f6a1c91631c23a77d3c932308497e18ed4033ecccf2e`:

1. Only retained header/generic/trait/impl/source services are used; no
   assertion or other expression compiler/evaluator.
2. Header syntax comes from direct retained locations, not literal/break-only
   generated payloads; execution paths are unchanged.
3. Written qualifiers and source facts are exposed without method selection
   or numeric inference policies. Semantic flags remain explicit holds.
4. Matching generated syntax supplies its actual associated-parent join;
   no ordinary-only semantic admission rule is added. The origin matrix is
   unexecuted and remains required.
5. Function bodies are inspected only for direct source presence/range;
   no body plan, pattern engine, cold arm or initializer is prepared/executed.
6. `encode_header` calls no DB, parser or source planner. Preparation retains
   immutable query handles; full factory installation/reuse is still held.
7. Existing source lookup/catalog dependencies are retained, and the completed
   memo's transitive input guard is captured outside provider computation.
   No global namespace comparison or scan is added to installed execution.
8. Parser/header/generic roots use Arc and direct syntax locations. No body
   structure is cloned or complete parsed file searched for each phase.

`git diff --check` passed. No build/test command, source fixture control,
currentness withdrawal, full DefinitionFacts production, upstream root or
runtime/JIT acceptance was executed by this subagent. Lead-owned query/module
registration and actual validation are still required. Later Rust source edits
must receive a new fingerprint/review and keep this evidence with its source.

Metadata-control successor SHA256
`source_definition_headers.rs=8e09c00f9fcc75d046d95255bef66fe819164a6f5480b743c51d4f2c303326a3`
adds only `cfg(test) check_actual_definition_headers`. The caller supplies
actual declaration IDs, expected role/name/associated-parent tuples from its
existing CLI source environment. The helper checks memo Arc reuse/currentness,
source span/edition/owner and associated-parent joins, actual generic argument
namespaces, header conversion, retained semantic holds and closed ready gate.
It rejects FunctionSignature/BodyStructure/Thunk demand and source queries
during encoding, and returns current guards for the caller's withdrawal test.
No production fixture names, semantic shortcut or complete facts are added.
All eight production source-review conclusions above remain unchanged for
this cfg(test)-only successor; metadata and runtime tests remain unexecuted
by this subagent. `git diff --check` passed; parent validation owns terminals.

Host-control candidate in the new child module
`cli/preentry_loader_tests/upstream_definition_headers.rs`, SHA256
`eaa6bf116c5f4be83e7e8fe4e3246de1da20bd485424c047f0d9a8469bae6f95`:

- `unchanged_declarations_prepare_actual_header_inputs_all_editions_and_origins`
  specifies 36 worlds/276 root source header observations from unchanged
  generic-derived-type, method-self-arg-trait and array-wrapper-struct-ctor
  item bytes. Expected associated owners come independently from actual
  original/generated Outline scope/range attachments and real canonical
  generated IDs. The frozen production header plan is not consulted to
  choose the expected parent.
- Core Copy/Sized/Clone/Fn/FnPtr/Pin lang-role observations are demanded once
  per edition in the actual Pair fixture's original world (up to24 observed
  headers); unavailable facet declarations are printed/count as explicit
  gaps. Ordinary Pair has `Ok(None)` only after actual source classification.
- `source_lang_attributes_and_aliases_preserve_actual_definition_identity`
  supplements the unchanged Pair fixture in temporary source with one real
  import alias and one source `#[lang="copy"]` trait, all four editions.
  It checks alias target ID preservation, distinct declaration identity,
  EmbeddedFacet versus SourceAttribute origin, and guard withdrawal. This is
  source metadata evidence only, not a rustc diagnostic or runtime fixture.

Both controls assert source memo reuse/currentness, retained semantic/root
holds, and absence of executable signature/body/thunk demand, then revoke
source inputs and check held guards. Temporary wrapper/source-control files
are removed after successful terminal assertions. The main matrix deliberately
does not repeat unchanged facade declarations for every root macro origin.
Full standard-macro contexts, arbitrary generated/local owners, complete
compiler semantic factories and all runtime gates remain unexecuted gaps.

Source review: the new file is host-only metadata control, invokes the existing
catalog/ordinary-source services and frozen header helper, does not change
language semantics or add a parser/evaluator, uses actual immutable source
attachments and IDs, and contains no complete-provider construction. Its cold
Outline traversal is test expectation construction, not runtime dispatch.
The eight recurring production failure-mode conclusions above are unchanged.
`git diff --check` passed; no Cargo/build/test was run by this subagent. Child
module registration and actual asynchronous validation are parent-owned.

## V496 common source-parent prerequisite (before repair)

The unchanged method-self-arg-trait.rs header matrix exposes ParentJoin after
the three edition2015 Pair worlds. Source review finds that the retained
PreparedScopeImplHeaders.member_declarations indexes original impl scopes
only, while HeaderPlan requires the owning trait for associated methods too.
Extend the existing once-per-source PreparedOriginalAssociatedDeclarations
join (already supports actual Trait/Impl owner kinds) to retain trait scopes
in the same index. Do not infer a declaration ID from a bare scope digest,
scan the complete source for each method, or add a generated-only evaluator.
GeneratedSyntax already retains both associated scope kinds. Cold cost adds
a pass over scanned trait headers within the existing source preparation;
lookup remains a retained BTreeMap access. Required closure evidence is the
same unchanged fixture in all editions/origins plus memo/cold/withdrawal
controls. Root/runtime/header-parity/semantic-factory holds remain.

The separate alias/attribute metadata harness used a core import without
extern crate core, which is not valid Rust2015 name setup. Correct only the
supplemental harness to bind an explicit extern-crate alias for every edition;
keep upstream originals and production edition/name resolution unchanged.
V495 Span shadowing and V496 failed controls remain separate evidence.
