# M020 source header and lang identity review

Baseline `48792aa3361d872566cebb9a91c41e1fd12fbfe7`. Independent reviewer
`/root/wf_validation_review`; frozen V495 source fingerprint
`1c38fcfc5b93f01c94b74b7f77f8cb274000b4b8619f1aa53a9d01305a875864`
(8,669 source inputs, unchanged baseline HEAD).
The V496 review recheck is bound to
`c668d595f09f0daecc7134e92e85492e7f2f6d96391ce87b15d79dfae121bdf3`
with the same HEAD and input count. Its only source delta from V495 is the
test helper's explicit `super::model::Span` literal. The architecture findings
and representation review below carry to V496; V495's failed compilation
remains attached to its original fingerprint.
This records inspected source paths. Build, control and runtime terminals are
separate evidence; no complete source factory, upstream root, milestone or JIT
acceptance follows from this review.

The V497 review is bound to
`66ee98557aac5242fce35869b8412fb941f8d96268d70f459d75e42542e65f68`
(same HEAD and 8,669 inputs). Its three guarded deltas from V496 are the
retained original trait-parent join in `traits/index.rs`, the supplemental
explicit core extern-crate alias, and richer pending diagnostics in the test
helper. `PreparedScopeImplHeaders::from_current_inventory` now invokes the
existing canonical associated-declaration pass for trait-only source scans as
well as impl sources. Validated `owner.member_owner` scopes map to the actual
canonical trait DefKey fingerprint in the same retained `member_declarations`
map, which participates in the stable index fingerprint. Conflicting identities
are errors. Generated joins remain unchanged. Against all eight modes below,
this extends the existing common cold source index: no evaluator, contextual
selection, generated exclusion, body preparation, per-header source walk,
runtime namespace scan or immutable-body copy is introduced. Its additional
cold work is the existing associated-declaration query for previously skipped
trait-only sources plus ordered map insertion for actual trait owners. Terminal
evidence for repaired headers/aliases remains separate from this source review.
Both readers of the retained original member map were checked: the new
`PreparedDefinitionHeader::prepare` parent join and existing
`DeclarationGenericTypes::prepare` inherited-const-parameter join.
The latter still has the historical `.or(context.scope.owner)` fallback;
positive trait-map results now supply their canonical parent before that
fallback. The new header plan has no corresponding bare-owner fallback. This
remaining legacy reader is not removed or justified by the new metadata tests.

## Actual shared routes

`SyntaxProvider::DefinitionHeaderPlan` ->
`PreparedDefinitionHeader::prepare` -> ordinary `DeclarationGenerics` and
`DefinitionReceipt::observe` -> retained direct item syntax and owning token
edition. Functions use the existing `ParsedFile::function_header` admission
helper, which excludes the body from its header comparison; the new plan only
retains parameter/result/ABI/body ranges and source keywords. Constants/statics
retain declared Type ranges and the presence of `=`, without executing their
initializers. An actual unnamed impl retains `None` for its name.

Associated declarations join through
`PreparedScopeImplHeaders::declaration_for_member_scope`, or the actual
`GeneratedSyntax::declaration_for_member_scope` for generated source. Trait
headers reuse `TraitDeclaration`; actual canonical impl owners reuse
`ImplTraitIdentity`. `SourceBindings` and `SourceGenericLayouts` preserve owner
and parent generic identities. `encode_header` exports established header
inputs and explicit `DefinitionSemanticHold` entries; it does not fill missing
variance, predicate closure, typed signature, opaque, compiler constness or
other semantic fields with defaults. `CurrentDefinitionHeader` retains the
ordinary pure memo's transitive input guard.

`SyntaxProvider::SourceLangIdentity` ->
`PreparedSourceLangIdentity::prepare` -> the same definition header plan.
Effective `HeaderRecipe.attributes` and the common
`syntax::ast_adapter::string_value` token service classify actual `#[lang]`
values. The generated vocabulary retains exact pinned `rustc_attr_ir` and
`rustc_span` sources, including both `sym` and `kw` spellings. This is metadata
decoding, not another expression or macro evaluator.

The explicit embedded facade contract binds the actual defining unit, module,
item kind, declaration key, source bytes/path/profile and current publication.
The existing catalog constructor first authenticates the embedded source set
through `CapturedTraitFacadeSources::capture`; reserved units or caller-visible
names alone cannot establish that receipt. Ordinary aliases select the same
definition through the common name service. Per-declaration `Ok(None)` means
that declaration has no classified role, never that a whole-world lang-item
registry is complete. Missing header/syntax/vocabulary/facet inputs remain
typed pending.

During the draft review, missing `kw::Fn`/`kw::Pin` vocabulary rows, the facade
`FnPtr` trait's incorrect reflection-struct role, source-view selection without
the defining crate, and silent absence after an unmapped explicit contract
were identified. The inspected updated draft maps keyword spellings from the
pinned symbol source, maps facade `FnPtr` to `FnPtrTrait`, binds the exact
crate/path view, and reports an unknown contract mapping as pending. Final
freeze review rechecked these fixes at the fingerprint above.
The parent review also removed `FromResidual` from the facet candidates: the
pinned corresponding lang item is an associated method, not that trait.

## Eight recurring architecture modes

1. `PreparedDefinitionHeader` and `PreparedSourceLangIdentity` retain ordinary
   declaration metadata. They add no assertion operand compiler, synthetic
   executable body or expression evaluator. No macro identity selects their
   semantics.
2. Actual handwritten and generated declarations use the same retained
   parser/header queries. No literal/break-only executable output profile or
   macro-only Type representation is added. Source syntax is distinct from the
   explicitly unimplemented compiler semantic producers.
3. Source roles use actual header attributes or an authenticated explicit
   library facet. Generic identities use existing source bindings. No numeric
   hint, method whitelist or caller spelling selects a lang role. The facade
   contract is an identity mapping, not a replacement trait solver.
4. Generated declarations retain their real `GeneratedSyntax` parent join and
   owning token edition. There is no original-only refusal. Missing parent or
   source joins are pending rather than guessed ownership. Full origin and
   standard-macro matrices remain unexecuted until terminal evidence exists.
5. Declaration discovery and source header facts are demanded without lowering
   executable bodies or unselected arms. Body/initializer presence is only a
   source range/keyword fact. No pattern engine or static/borrow checker is
   introduced. The source controls must independently reject executable
   preparation; source inspection alone is not that terminal evidence.
6. Both plans use the existing immutable pure memo framework and retain Arc
   handles. Header and attribute parsing stays in cold query preparation.
   `encode_header` only converts retained source facts in a prepared context;
   runtime callbacks must consume retained results and cannot re-enter these
   plans. A retained input guard is currentness evidence, not authority to
   resolve or execute new source.
7. Cold header preparation traverses actual header/parent/generic dependencies.
   Facet selection may scan retained source views during its memo computation;
   it is not dispatch work. No new namespace census, whole-world comparison or
   per-operation source planning is introduced. A future complete registry and
   fact-publication owner still must retain relevant positive/negative lookup
   dependencies and require guards current before root entry.
8. Existing parsed/query Arcs, direct syntax ranges and context-owned immutable
   symbol/generic handles are reused. There is no `BodyIndex` deep copy or
   repeated whole-file node search. Source ancestor/header traversal is cold.
   No new unsafe, string leak, memory accounting or resource cap is introduced.

## Transmute integration inspection

The arriving existing-file integration replaces the last
`SolverClients::transmutable` function pointer with the pinned
`transmute::is_transmutable` implementation and removes the empty client-holder
types. `PreparedFacts` instead requires typed `PreparedTransmuteInputs`; the
provider accessor reads immutable target/layout/variant/tag/primitive/
discriminant inputs. Missing preparation remains a distinct dependency; the
delegate's missing-admission panic is an integration invariant failure, not
semantic `None`/`NoSolution` or an implemented suspension path.

The inspected adapter retains full compiler tree construction, enum direct and
niche tags, field memory order/padding, union alternatives, source field binder
projection, references and source/destination error precedence. The generic
crate retains the pinned tree pruning, automata and memoized comparison
algorithm. Stable map/never/iterator and public API changes are registered
representation/packaging adapters. The compiler's exact `Answer::If` ->
`NoSolution` delegate mapping is retained. No equal-size/primitive shortcut or
independent layout calculator is added. The actual source/target/layout and
Assume/lang-item producer remains required; fabricated layout tables cannot
establish root acceptance.

The draft Endian adapter ordering issue was reported to the worker and fixed
before its handoff. Helper provenance now includes extracted compiler safety
marker behavior and `ScalarInt::TRUE`, not handwritten substitute policy.
Final integration and the full generator outputs still require frozen-source
audit/build evidence.
At V495, full inference, transmute, language-vocabulary and vendor bootstrap
audits returned zero. The library build returned 101 for one test-only
`Span` name collision at `source_definition_headers.rs:401`; controls and
runtimes did not run. Closing source/HEAD guards matched; the CLI receipt
records the old binary, not a successful build of this source. Actual logs
and receipts remain in `artifacts/validation-current-v495`.

## Remaining boundaries

The embedded facade lacks several pinned declarations and differs in some
header shapes. Authenticated present-item identity does not prove full
upstream header parity or permit invented missing IDs. A complete lang-item
registry, real semantic `DefinitionFacts`, typed query-result provenance,
admission/suspension/currentness owner, upstream root evaluation and ordinary
answer/member/VM integration remain held. Existing custom interpreter
resolution remains an alternate route for later removal.

Controls, source/epoch withdrawal, aliases/shadows, cold-body variants,
malformed/foreign inputs and complete edition/origin matrices must retain
their actual terminal scope. Imported source or green metadata controls are
not unchanged main execution, full-fixture, diagnostic/tooling, Linux or
performance acceptance. MA001-MA008 remain separately numbered macro audit
obligations; none is closed by this component review.

The inspected new CLI control selects actual IDs using independent retained
Outline owner/scope attachments. Its planned primary matrix is 36 source
worlds and 276 root declaration headers, with six requested facade roles once
per edition. Facade gaps are explicitly logged and the assertion only requires some
facet evidence, so the terminal's actual facet count/gaps must be reported.
The separate four-edition alias/`#[lang]` control is supplemental source metadata
evidence. Neither control installs complete `PreparedFacts` or enters main.

## V496 terminal scope

At fingerprint `c668d595f09f0daecc7134e92e85492e7f2f6d96391ce87b15d79dfae121bdf3`
and HEAD `48792aa3361d872566cebb9a91c41e1fd12fbfe7`, Rust 1.99.0 offline,
locked library, CLI and source-digester builds returned zero. Full inference,
transmute, language-vocabulary and bootstrap audits returned zero. The optional
`cargo test -p rustc_transmute --offline --locked` returned 101 before tests:
the vendored package requires dev-dependencies and is not a workspace member.
No component algorithm tests were executed or inferred from the audit.

The declaration-header control returned 101 after the first three
generic-derived-type 2015 origin worlds. The next ordinary trait-member source
reported `ParentJoin`. Independent inspection found that
`PreparedScopeImplHeaders::from_current_inventory` retained original impl
member parents but omitted original trait member parents; the existing
`PreparedOriginalAssociatedDeclarations` can supply their authentic canonical
scopes. This is a common source-index prerequisite, not a permissible header
special case. The exact failed declaration was not logged in this terminal.
The lang alias control returned 101 before classification: its supplemental
2015 `use core::marker::Copy` lacked an explicit extern-crate binding. No
alias, attribute or shadow matrix pass is claimed from that terminal.

The retained field control returned zero (85.846 seconds). All three control
receipts report one selected body and unchanged inputs. The unchanged stock
runtime batch remained one runtime-contract pass (`generic-derived-type`,
owning 2015, completed main, stdout `10\n10\n`, closed capture/cleanup) and one
interpreter failure (`repr128`, owning 2015, `LRR-E-OP-TYPE`). These are ordinary
interpreter outcomes, not execution through the newly prepared solver facts.

All V496 processes were terminal before closing guards. `after-runtime`
matched the before-source fingerprint, HEAD and after-build binaries, with no
changed guarded inputs. Test SHA256 was
`29a0a23e3f82609861649a948967d34ab8069f82991e4f2a514a247dde74016c`;
CLI SHA256 was
`8fa443a6700f6eca3eadc3bafd60a3976b5f35c50602599a29c3996d02b8a7ce`.
Receipts and exact commands are under `artifacts/validation-current-v496`;
runtime report is `/private/tmp/lrr-v496-source-header-runtime/report.json`.
The original parent join, origin matrix and supplemental alias results require
fresh evidence after repair; V496 is not header/lang/root acceptance.

## V497 terminal scope

At the V497 fingerprint recorded above, Rust 1.99.0 offline, locked library,
CLI and source-digester builds returned zero. Full inference, transmute,
language-vocabulary and bootstrap audits returned zero. The known non-workspace
generic-crate test command was not repeated; no new algorithm unit-test result
is claimed.

The header control returned zero (96.314 seconds): 36 source worlds, 276 root
declaration headers and 24 facade headers, with zero gaps among its six requested
facade roles per edition. It crossed the former original trait-member parent
failure and exercised the same original/user-macro/wrapper paths in all four
editions. Independent Outline parent expectations, same-plan Arc memo identity,
currentness/withdrawal and absence of executable-preparation queries are host
metadata assertions. The supplemental alias/attribute control returned zero
(12.263 seconds) in all four editions, preserving the actual Copy target ID,
distinct source-attribute declaration identity and revoked-guard rejection.
Neither is upstream solver root entry or execution of main.

The unchanged two-case runtime replay returned one runtime-contract pass
(`generic-derived-type`, owning 2015, all capture/main/cleanup checks true,
stdout `10\n10\n`) and one interpreter failure (`repr128`, owning 2015,
`LRR-E-OP-TYPE`). The report records no native execution and unchanged imported
originals/manifest/binary; this remains ordinary interpreter evidence, not
execution through new complete solver facts.

The retained field control returned zero (91.053 seconds): 36 worlds, 72 encoded
fields and 12 explicitly pending declarations. Its terminal logs the pending
count but not the exact pending type, so no particular pending representation
variant is established by this result. All three control receipts report one
selected body and unchanged inputs.

All V497 commands were terminal before `after-runtime` captured unchanged
source and HEAD, no changed guarded inputs, and matching after-build binaries.
The library test SHA256 was
`60eb156af79ab1528c627334ccd57f6c80ee7cf2b100e3350fde014a3c248cf7`;
CLI SHA256 was
`ac355e83fc027d344fd87e7a098e26f3cd1e613dd99735691cf3e9872963fb7d`.
Exact build/control/audit receipts are in `artifacts/validation-current-v497`;
runtime report is `/private/tmp/lrr-v497-source-header-runtime/report.json`.
The metadata matrix establishes its selected component contracts, while the
remaining boundaries and unexecuted standard-macro/full-runtime matrix above
remain held. Source and Cargo ownership returned to the lead after the closing
guard; this review introduced no production source changes.
