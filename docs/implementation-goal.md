# Implementation goal — revision 5, macOS first

The pre-JIT prerequisite is complete applicable upstream runtime-contract acceptance plus applicable interpreter correctness, as defined in Section 1.2.1. Diagnostic and compiler/tooling compatibility have separate gates; full-suite compatibility remains incomplete until all original contracts pass. Runtime-only mixed-case success is not a full-fixture pass, and external rustc evidence never counts as interpreter execution. This supersedes historical wording requiring every compiler/tooling category before JIT.

Coordinate parallel human/agent contributions through the [12 worktracks](../worktracks/README.md), bounded claims and PR/evidence history. Worktracks divide ownership and preserve the fixed ledger order; they do not open guest JIT or establish acceptance.

Implement LRR 0.2.0 and `lrr-2024-v2` according to
[lrr_rust2024_macros_traits_implementation_spec.md](../lrr_rust2024_macros_traits_implementation_spec.md).
Complete every T/H/M task and its acceptance requirements in the fixed Section 57
cross-ledger order, preserving the selected tagged VM, lazy LRIR, observed-region
Cranelift and LLVM tiers, demand-driven trait system, macro frontend, capabilities
and compiler-described direct Rust ABI generations. Interpreter convergence precedes
guest JIT implementation; the later native/optimization requirements remain mandatory. Keep exact dependency pins, deterministic indexes,
typed outcomes, checked resource sizes and stable owned encodings. Add acceptance
before task completion and run the current milestone gate after integration.
Do not substitute another frontend, query framework, backend or language profile.

Implement and test on macOS first with Rust 1.99.0 on `aarch64-apple-darwin`.
Linux x86-64 is a later port. Docker is supplementary and unavailable Linux
validation must never block macOS progress. Require actual Linux acceptance
before claiming Linux support; retain and label deferred platform evidence.
These current user instructions supersede the old revision-3/Linux-first text
stored in the thread's goal metadata. They preserve the full project scope.

Only explicitly authorized preparation may run build tools. Procedural macros
require separate macro authority and a matched external server; execution is
never a query-provider side effect. Native loading requires an immutable,
schema-checked generation in an authorized hybrid worker. Never compile a
selected live runtime unit or load arbitrary native symbols. Keep local production
unsafe confined to the five permitted JIT/native files. Record actual pinned
backend contradictions with native/minimal witnesses, regressions and audited
errata rather than silently changing the design.

Current verified checkpoint: H001–H003 retain component acceptance under Section57/E-57-001. M013 component acceptance is complete under Section57/E-57-001; full MX027/MX028 remain pending. M014 declaration scanning has component acceptance; full release integration remains pending: the pinned parser and immutable header inventory discover cold nested impls and retain pending producer demands without preparing executable bodies. Original-source derives have authorized execution, ordered append
integration, completion proofs, canonical declaration restoration and publication
for known inert/helper cases. Published original type queries invalidate after
source edits. Unknown outer annotations, qualified helpers, forged/duplicate/
foreign-run append and stale source retain their recorded refusal controls.

Enabled members can now demand exact helper metadata from later derive groups
on the same item before native execution. Source group-row resolution is shared
by ordinary selection and lookahead. The local scope deduplicates receipts and
records introduction positions/source contexts with checked limits. Members use
valid later introductions; outer annotations still require a preceding introduction.
Later metadata does not grant an early outer helper or a qualified helper path.
Current-member scopes preserve the existing queue when unrelated later names are
unresolved. Completed selections and source completion proofs retain replay checks.

A controlled two-group case prepares both declarations before dispatch, preserves
the later derive/member helper in the real input, executes twice and publishes
real constants. Qualified-member and early-outer variants remain incomplete with
zero/one native invocations respectively. The newly copied rustc helper-name
collision fixture's adaptation executes two same-named-helper exports, preserves
the intervening helper and publishes. These are adapted component controls;
original auxiliaries and full stock fixtures are not accepted by these observations.

The macOS prepared-server checkpoint records 1798 host/doc test passes and all 731
M013 controls. Gate00/01/02 and provisional Gate03 component, strict Clippy,
formatting, architecture and first/final fixture-manifest audits pass with
RUST_TEST_THREADS=2. Twenty unchanged rustc cases and all 43 imported file hashes
are verified at upstream commit ae0fe511b15ab760d7e963b13389d1bc2ab42b18. The
exercised subset retains 50 component assertions; the two M014 imports now have syntax/inventory assertions, with full fixture acceptance pending.
Both original license texts, directives and auxiliaries are retained. One unchanged
empty-derive case passes a whole-source CLI interpreter run and explicit native
metadata reference. Full compiletest and LRR fixture acceptance remain zero.
The installed server still fails E-M013-001's printing auxiliary; terminal refusal is
separate evidence. Explicit preparation now builds a versioned matched-source server
with a preserved protocol descriptor and macro stdout routed to diagnostics. The
unchanged printing auxiliary completes five occurrences; the unchanged tracked-env
auxiliary passes three environment/fallback cases. Typed schema4 provenance now enables explicit preparation and ordinary MacroRun
adoption of this exact repair. The integrated control executes identity and a
nested declarative macro, seals and publishes generated members. Changed repair
source refuses adoption before a run starts. The unchanged original derive-order source now completes all five native
derives through the worklist and publishes canonical Foo. Macro-host registration
headers generated by macro_rules expand on demand through owned syntax events;
one declaration expansion is reused. Definition/call editions remain 2015/2024.
Full guest/tier and CLI oracles remain pending.

Following original-source replacing attributes now execute after replayed completed derives. Three controls cover preserved append output, prior macro bindings and stale-source refusal. Original-source helpers now follow identity and chained attributes with sibling/function-like isolation controls. Full legacy ordering/lint/helper diagnostics and
resource/world/edition/service/CLI matrices remain pending. M014–M016 follow M013
in the required order. The parser/AST and macro engines reuse rust-analyzer. The
custom interpreter runs a limited primitive subset including loops, calls and
recursion. Cranelift has synthetic bootstrap coverage; guest LRIR lowering,
region recording, guards and interpreter/JIT transitions are unimplemented.
Trait/runtime/native/security, performance, full-fixture/release and Linux
acceptance remain incomplete.

See [the helper scope](m013-source-future-helpers.md) and
[the checkpoint](../artifacts/macos-first/m014-procedural-arguments-checkpoint.json).

M007 revision-4 component acceptance is complete on macOS. Subsequent component
progress notes are historical; see docs/m007-acceptance-audit.md for the current
verified scope. M008 pure-query suspension component acceptance is complete; see docs/expansion-record-publication.md. M009 procedural requests/sessions has component acceptance; full integration remains pending under E-57-001. M010 and M011 component acceptance are complete under E-57-001; full integration remains pending.

At the first M007 checkpoint, The owned textual/path namespace component now has nine
acceptance checks, including a standalone macOS native comparison with actual
pinned expansions. Scope order, raw names, generated-declaration atomicity,
reexport identity, dependency renames and long-chain lookup are exercised.
See [the namespace checkpoint](../artifacts/macos-first/m007-namespace-checkpoint.json).
Full MX011/MX012 acceptance remains pending.

The actual pinned hygiene store now adds five passing component checks for
mark normalization, mixed-site inheritance, unit/edition ancestry, remapping
and depth/atomicity boundaries. Gate 01 and strict Clippy pass with 181 host/doc
tests. The fifth audited API patch preserves the archived mark algorithm.
See [the hygiene checkpoint](../artifacts/macos-first/m007-hygiene-checkpoint.json).

Marked transcription and exact ancestor-aware byte preflight now add seven
passing component checks, including generated marked definitions, explicit
owner metadata, `$crate` transport, rollback and actual 32 MiB admission.
Gate 01 and strict Clippy pass with 188 host/doc tests; five patched upstream
files reconstruct from their original preimages. See
[the marked-transcription checkpoint](../artifacts/macos-first/m007-marked-checkpoint.json).

Producer-complete reexport cycles now resolve to absence under Section 50.2,
while pending producers still demand input and publish atomically. Four new
component checks exercise that correction and transfer actual pinned item/local
normal forms into owned runtime facts. Runtime validation checks references,
normal-form transparency and root authority; `$crate` retains the emitting
definition owner. Gate 01, strict Clippy and the upstream patch proof pass with
192 host/doc tests. See [the normalization checkpoint](../artifacts/macos-first/m007-resolution-checkpoint.json).
Final source-aware selection and full native collision acceptance remain pending.

Owned macro namespace names now use Unicode 17 NFC identity after pinned
parser validation, with exact supplemental unicode-normalization 0.1.25.
Two regressions include a real Rust 1.99.0 macOS compiler comparison with actual
pinned expansion. Gate 01, Clippy and format checks pass with 194 host/doc tests.
See [the Unicode checkpoint](../artifacts/macos-first/m007-unicode-checkpoint.json).
The later token-adapter checkpoint below completes that component integration.

Macro token import/export now canonicalizes identifiers while retaining original
source spans, raw flags, contexts, origins, editions and literal spellings.
Five new checks include actual pinned matching versus macOS native Rust in all
four editions and exact marked byte admission for NFC shrink/expansion.
Gate 01, Clippy, formatting and four patch-proof checks pass with 199 host/doc
tests. See [the token checkpoint](../artifacts/macos-first/m007-unicode-tokens-checkpoint.json).
Authorized procedural replies and generated-source integration remain pending.

Owned local-name lookup now follows the pinned declaration-rib rule with
actual local normalization facts and exact expansion/definition metadata.
It preserves definition/caller/generated-local separation, function boundaries,
atomic pending publication and following source frontiers. Six checks include
4,096 nested scopes and an actual marked expansion compared with native macOS
execution. Gate 01, Clippy, formatting and four patch proofs pass with 205
host/doc tests. See [the local checkpoint](../artifacts/macos-first/m007-local-hygiene-checkpoint.json).
The fixture supplies source facts; discovery and guest VM integration remain pending.

Marked `$crate` now routes into ordinary source name buckets with the emitting
definition unit and explicit namespace. Two checks include real native Cargo
renames/reexports and caller/facade helper collisions; actual pinned marked
output selects matching source helper identities. Known macro_export metadata
remains in the macro header. Gate 01, Clippy and formatting pass with 207
host/doc tests. See [the owner checkpoint](../artifacts/macos-first/m007-dollar-crate-checkpoint.json).
Unit/import facts are supplied; Cargo collection, export options/visibility and
guest VM integration remain pending.

Expansion-order ancestry and compatible textual-candidate ambiguity now
agree with seven native macOS cases, including four E0659 rejections and
three successful pinned rule selections. Four new checks cover atomic DAG
registration/depth, stable fingerprints and actual pinned root-edition name
comparison without erasing ownership. Gate 01, Clippy and formatting pass with
211 host/doc tests. See [the origin checkpoint](../artifacts/macos-first/m007-macro-origins-checkpoint.json).
Candidate/name/origin facts are supplied; full source-index and VM acceptance
remain pending. Direct owned context-key equality is insufficient for name
comparison across root units/editions.

Owned name-comparison classes now cross the adapter/runtime boundary while
preserving source owners and editions. Three acceptance checks prove agreement
with the pinned comparison algorithm, stability across private-ID remapping,
strict missing/duplicate/unknown-row rejection and cross-unit/edition local lookup.
Gate 01, Clippy and formatting pass with 214 host/doc tests. See
[the name-class checkpoint](../artifacts/macos-first/m007-name-classes-checkpoint.json).
Source facts are supplied; source discovery and guest VM acceptance remain pending.

Textual lookup now joins source order with owned pinned name classes and explicit
declaration origins before selecting a macro. Four regressions cover parent
visibility, pending producers, metadata consistency, publication and preservation
of same-output binding history. The seven native cases exercise this joined
lookup. Gate 01, Clippy and formatting pass with 218 host/doc tests. See
[the textual-selection checkpoint](../artifacts/macos-first/m007-textual-selection-checkpoint.json).
Facts remain supplied; automatic source-index publication and guest VM are pending.

The original-source macro index now derives declarations, cfg-filtered source
order and module openings from the pinned parser outline, with function bodies
and macro token trees opaque. Five acceptance checks include typed source/macro/
attribute demand and actual native r#gen selection of [17,29]. Gate 01, Clippy
and formatting pass with 223 host/doc tests. See
[the source-index checkpoint](../artifacts/macos-first/m007-source-index-checkpoint.json).
Immutable source publication, entered bodies and generated epochs remain pending.

Original-source declaration and logical module identity now use bounded NFC
through source outlining and immutable context publication. Two regressions
preserve stable DefKeys/duplicate ordinals, raw reserved names and full-width
identity. Both original and Unicode/raw native Cargo collision cases match
[17,17,53] through actual marked expansion and source buckets. Gate 01, Clippy
and formatting pass with 225 host/doc tests. See
[the source-name checkpoint](../artifacts/macos-first/m007-source-names-checkpoint.json).
Entered-body/import/generated-source and guest VM integration remain pending.

Macro indexes now select and retain exact immutable prepared source contexts,
including profile/cfg as well as content/unit/path/module identity. Two checks
exercise edits/deletion, cfg-only changes, mismatch rejection and retained
source-byte charging/release. The native [17,29] fixture now uses a published
source view. Gate 01, Clippy and formatting pass with 227 host/doc tests. See
[the published-index checkpoint](../artifacts/macos-first/m007-published-index-checkpoint.json).
Entered-body/import/generated-source and guest VM integration remain pending.

Entered-body macro discovery now uses exact retained BodyKeys and the pinned
AST, with ordered lexical blocks, checked iterative frontiers, cfg/export
metadata and explicit statement producer demand. Opaque nested executables and
templates cannot be queried through an outer entered frame. Four acceptance
checks include native source shadowing [17,17,41,29]. Gate 01, Clippy and
formatting pass with 231 host/doc tests. See
[the entered-body checkpoint](../artifacts/macos-first/m007-entered-body-checkpoint.json).
Source local bindings, nested entry/import/export/generated epochs and guest VM
integration remain pending.

Selected external-module views now link recursively within the same immutable
publication, capturing parent macro visibility at declaration. Four checks
exercise exact child/grandchild body lookup, stale/wrong/foreign links and full
inherited identity consistency. The three-file native comparison matches
[17,53,41,61,29] from full original token forests preserving source offsets.
Gate 01, Clippy and formatting pass with 235 host/doc tests. See
[the module-link checkpoint](../artifacts/macos-first/m007-linked-module-checkpoint.json).
Source file selections are supplied; path resolution/seals, local bindings,
imports/exports/generated epochs and guest VM integration remain pending.

Original-source import facts now retain pinned AST grouping/path/alias/glob facts
with bounded metadata, cfg filtering and cold-body opacity. Two checks pass;
Gate 01, strict Clippy and formatting pass with 237 host/doc tests. See
[the source-import checkpoint](../artifacts/macos-first/m007-source-imports-checkpoint.json).
Namespace and export resolution remain pending.

Parsed source macro export and named-import contributions now preserve original
definition identities across textual/dependency/facade routes. Deferred facts
prevent graph construction. The actual three-package native Cargo oracle uses
parsed routes with pinned hygiene and matches [17,17,53] in both cases. Gate 01,
Clippy and formatting pass with 239 host/doc tests. See
[the source-route checkpoint](../artifacts/macos-first/m007-source-routes-checkpoint.json).
Glob/module alias fixed points, visibility and membership seals remain pending.

Original source module aliases now resolve with deterministic prefix wakeups,
including forward chains, root/dependency/raw aliases and macro target rewriting.
A 400-link chain and ambiguity/cycle tests pass. Actual selected pinned expansion
from full original token forests matches native macOS [17,17,17]. Gate 01, Clippy
and formatting pass with 242 host/doc tests. See
[the module-alias checkpoint](../artifacts/macos-first/m007-module-alias-checkpoint.json).
Module aliases are candidate facts; visibility and membership seals remain pending.

Caller-aware macro binding lookup now checks canonical private/self/super/crate/
ancestor ranges and reexport width. E-M007-011 preserves the native E0364 witness
and regression. Native macOS executes seven allowed calls and rejects five private
accesses with E0603 plus widening with E0364. Gate 01, Clippy and formatting pass
with 246 host/doc tests. See
[the visibility checkpoint](../artifacts/macos-first/m007-visibility-checkpoint.json).
Next integrate module-prefix/alias visibility and direct qualified alias calls,
then glob propagation and the remaining source/local/generated integration.

Retained module alias paths and import origins now support direct qualified
macro calls with module-prefix/private-alias visibility and reexport-width
checks. Native E0603/E0365/E0364 cases match; public escapes through private
ancestors remain callable. Six selected expansions from full original token
forests match native direct/renamed/raw alias calls. Gate 01, Clippy and formatting
pass with 249 host/doc tests. See
[the module-path checkpoint](../artifacts/macos-first/m007-module-path-checkpoint.json).
Continue glob propagation and remaining source/local/generated integration;
relative invocation AST integration and membership seals remain pending.

Original source macro-only globs now propagate canonical definition identities,
respect explicit bindings and member visibility, and retain typed pending,
ambiguity and cycle outcomes. Three source checks and actual native macOS
forwarding/shadowing/rooted-cycle/public-member execution pass, including exact
ambiguous_glob_imports and E0603 rejections. Gate 01, strict Clippy and formatting
pass with 253 host/doc tests. See
[the macro-glob checkpoint](../artifacts/macos-first/m007-globs-checkpoint.json).
Module and ordinary-item glob facets, generated completion, membership seals,
relative invocation AST integration, local bindings and guest VM remain pending.

Direct disclosed source module lookup now returns canonical identity and final
binding visibility with typed absence/access outcomes. Macro prefix and glob
source checks share the iterative walker; glob validation no longer appends a
dummy macro member. Two checks cover parsed alias/privacy/root facts and module
depth 256 with rejection at 257. Gate 01, strict Clippy and formatting pass with
255 host/doc tests. See
[the module-lookup checkpoint](../artifacts/macos-first/m007-module-lookup-checkpoint.json).
Next use this typed module result for module glob contributions; ordinary-item
facets, generated completion, membership seals and guest VM remain pending.

Original source module globs now join named alias closure with weak canonical
module candidates, visibility intersection, explicit shadowing and deterministic
ambiguity/cycle outcomes. Three source checks cover forwarding, diamonds,
rooted/empty cycles, privacy and renamed provider/facade identity. Native macOS
executes five permitted paths with [17,17,31,17,17] and rejects ambiguity/privacy
with ambiguous_glob_imports/E0603. Gate 01, strict Clippy and formatting pass
with 259 host/doc tests. See
[the module-glob checkpoint](../artifacts/macos-first/m007-module-globs-checkpoint.json).
Ordinary-item facets, generated completion, membership seals, relative invocation
AST integration, local bindings and guest VM remain pending.

Selected original source invocation AST records now preserve exact parse/range,
raw path spelling, absolute qualification and enclosing module identity. Original
item-position lookup checks source/module/body-entry guards, textual precedence
and pending producers, then routes relative/raw/dependency paths through the
retained graph. Four source checks and actual pinned-hygiene/native macOS generated
function comparison pass with [17,17,17,31]. Gate 01, strict Clippy and formatting
pass with 264 host/doc tests. See
[the invocation checkpoint](../artifacts/macos-first/m007-invocations-checkpoint.json).
Continue attribute/cfg and entered-body invocation integration, source local
bindings and membership completion. Generated tokens and guest VM remain pending.

Cfg-filtered original item invocation inventory now separates disabled calls,
ordinary item producers and active transforming attribute producers. Disabled
calls/modules are Absent; active attributes suspend after prior source demand.
Two source regressions and actual pinned-hygiene/native macOS cfg comparison pass
with [99,17]. Gate 01, strict Clippy and formatting pass with 267 host/doc tests.
See [the cfg/attribute checkpoint](../artifacts/macos-first/m007-invocation-cfg-checkpoint.json).
Active attribute execution/publication, generated tokens, entered-body invocation
selection, membership completion, local bindings and guest VM remain pending.

Entered-body invocation selection now reads exact calls from the retained source
view, validates body/source/range ownership, rejects opaque nested syntax and
preserves local textual order, cfg absence and attribute suspension. Qualified
fallback uses the containing module and supplied namespace graph. Two source
checks pass; the native macOS shadowing oracle now uses this API and selected
pinned expansions match [17,17,41,29]. Gate 01, strict Clippy and formatting pass
with 269 host/doc tests. See
[the entered-invocation checkpoint](../artifacts/macos-first/m007-entered-invocation-checkpoint.json).
Linked-module outer-chain invocation selection, body-local imports, local-binding
ribs, generated publication, membership completion and guest VM remain pending.

Linked external-module item/body invocation selection now validates complete
retained source chains and shares exact range/eligibility guards while selecting
through captured ancestor scopes. A source check proves parent order, local
shadowing and missing/stale ancestor rejection. The three-file native macOS
comparison now selects all five calls through invocation APIs; pinned original
expansions match [17,53,41,61,29]. Gate 01, strict Clippy and formatting pass with
270 host/doc tests. See
[the linked-invocation checkpoint](../artifacts/macos-first/m007-linked-invocation-checkpoint.json).
Body-local imports, source local-binding ribs, generated publication, module path
discovery/seals and guest VM remain pending.

Entered body discovery now retains cfg-enabled original import leaves through
the existing pinned collector, with raw/grouped paths, block ordinals, exact
body owner/ranges and attribute input. A source check proves nested scopes,
self/glob syntax, cfg exclusion and nested executable/template opacity. Gate 01,
strict Clippy and formatting pass with 271 host/doc tests. See
[the body-import facts checkpoint](../artifacts/macos-first/m007-body-imports-checkpoint.json).
Next integrate these facts with body-local namespace construction and invocation
lookup; import resolution itself is still pending.

Entered invocation lookup now consumes original body named imports with scope-wide
visibility, nearest-block shadowing, forward/module alias routes and complete
empty cycle outcomes. Two source checks and actual pinned-hygiene/parsed-source
native macOS comparison pass with [17,31,17]. Gate 01, strict Clippy and formatting
pass with 274 host/doc tests. See
[the body named-import checkpoint](../artifacts/macos-first/m007-body-named-imports-checkpoint.json).
Body-local textual import targets, globs, additional ambiguity/visibility/cycle
integration, generated publication, source local ribs, seals and guest VM remain pending.

Continue M007 source local-binding/definition-rib discovery, remaining source import/export
ordinary-item facets, visibility and modular ambiguity, followed by local/owner routing,
macro path/modular ambiguity and generated exports, including
hygienic dispatch from the source/import index, `$crate`, dependency renames,
raw identifiers, reexports and definition/call-site collisions. Then continue
M008–M012 before advancing the Section 57 batch. Preserve exact source/origin
editions and immutable snapshot ownership throughout later trait/VM/JIT/native
integration. The full goal remains active; release_complete remains false.

Completion requires every named feature, fixture, security/performance gate and
release deliverable to be proved at its required scope. Finish with reproducible
clean-checkout macOS release artifacts and an honest feature/performance/security
report. Planned, skipped, indirect or unexercised checks are not passed. Linux
acceptance is a separate later platform phase, never an inferred success from
macOS or a Docker availability check.

## M007 body textual import declaration capture

Body textual import targets resolve in the retained use declaration scope and position through the existing lexical resolver and captured outer frames. Later and nested redefinitions do not replace the imported definition; forwarding through body aliases preserves each declaration context. Public opaque-syntax and source/body guards remain enforced. New source regression and actual pinned-hygiene/macOS Rust1.99 native comparison pass. Body globs, additional ambiguity/cycle/visibility integration, generated publication, source local-binding ribs, membership seals, guest VM and M012 aggregate accounting remain pending. M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 276 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-body-textual-import-checkpoint.json`. Release remains incomplete.

## M007 entered-body source-module glob contributions

Original body globs contribute macro candidates from disclosed source modules after independent target-module visibility checks. Body named module aliases and self-as-module routes work. Same-block named imports suppress weak glob candidates; inner globs retain distinct outer-import ambiguity and same-definition diamonds deduplicate. Private members do not contribute; transforming imports and pending member facts preserve NeedInput. Checked shared work/metadata budgets and pre-insertion candidate admission bound the walk. Two source checks and actual pinned-hygiene/parsed-source/macOS Rust1.99 native acceptance pass, including E0659 ambiguity and separate private-member filtering execution. Module names supplied by other entered-body globs, body item/module catalogues, additional ambiguity/cycle/visibility integration, generated publication, source local-binding ribs, membership seals, guest VM and M012 aggregate accounting remain pending. Undisclosed plain-name glob targets fail explicitly; no complete body namespace or release claim is made.

MacOS Rust1.99 Gate01 passes with 279 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-body-globs-checkpoint.json`. Release remains incomplete.

## M007 selected-scope body module membership

Synchronous ephemeral module-route tables resolve original body named/glob imports only in the selected scope and ancestors. Chained glob targets, forward/same-name imports, qualified calls and separate macro-only facets work. Canonical module identity remains separate from original access routes, preserving public escapes across private ancestors and module ambiguity even when exported macro identity matches. Import-source lookup excludes its own contribution; other unknown strong imports cannot seed a cycle through a weak glob. Complete empty cycles settle candidate absence; unstable states fail deterministically. Pending and denial facts remain typed, with checked table/index/candidate/route/state-history sizes and work. Four source checks and actual pinned-hygiene/parsed-source/macOS Rust1.99 native acceptance pass; native execution records four calls, with E0432 strong-cycle and E0659 module-ambiguity witnesses. No guest VM or complete native/guest diagnostic equivalence is claimed. Body item/module catalogues, generated publication, local-binding ribs, membership seals, guest VM integration and M012 request accounting remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 284 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-body-module-membership-checkpoint.json`. Release remains incomplete.

## M007 original entered-body item headers

Original cfg-enabled body item headers now retain lexical block ownership, stable source DefKeys, raw token recipes, semantic names, visibility/attributes, namespace facets, exact executable handles and opaque container ranges. Header traversal prunes executable/container subtrees, with checked metadata/count/stack and allocation admission. Body textual macro bindings now use the same DefKey fingerprint as their item records, removing the provisional separate body-macro identity encoding. Two source acceptance checks cover ordinary classes, constructors, scopes, cfg, derive demand, opacity, shared macro identity, header/identity stability across body edits, exact changed body hashes and transforming entry rejection. Existing source/native macro selection regressions pass in the current gate. This component is source-fact evidence, not guest execution or local type checking. Nested body/container entry APIs, body module/import catalogue publication, local-binding ribs, generated publication, namespace seals, guest VM and M012 request accounting remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 286 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-body-items-checkpoint.json`. Release remains incomplete.

## M007 explicit nested body entry

Explicit original nested-function entry validates exact source/parent/child body revisions, declaration membership, range/hash and transform eligibility. Captured textual frames retain declaration order through grandchildren; parent lookup keeps nested syntax opaque. Borrowed producer ancestry and an owned scope/import namespace view preserve original import identity, privacy and declaration-time textual contexts, including inherited frozen aliases and child-glob/outer-import ambiguity. Shared existing resolvers consume the combined namespace; source/module ancestry stays separate from declaration bodies. Two source acceptance checks and actual pinned-hygiene/macOS Rust1.99 native capture comparison pass. The linked wrapper preserves full source-chain checks and module ancestry but additional linked-nested acceptance remains pending. Container entry, local module/item publication, local-binding ribs, generated publication, namespace seals, guest VM and M012 request accounting remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 289 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-nested-body-checkpoint.json`. Release remains incomplete.

## M007 linked nested entry acceptance

Linked nested/grandchild entry now has actual three-file source-chain acceptance. Captured external-module/body declaration order and inherited frozen alias contexts select the original macro rather than later module/body redefinitions. Changed current/child/root ancestors, missing ancestors and opaque-parent lookup are rejected. A macOS Rust1.99 native program executes nine calls; every selected original macro is expanded by the pinned matcher/transcriber from full original forests, preserving source content/path/offsets, and all observed values match native execution. This closes the previously unexercised linked-nested wrapper integration; no guest VM claim is made. Container entry, local module/item publication, local-binding ribs, generated publication, namespace seals, guest VM and M012 request accounting remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 291 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-linked-nested-checkpoint.json`. Release remains incomplete.

## M007 explicit original container entry

Explicit original module, trait, impl and extern-block container entry validates retained source/parent/declaration/kind/range/content and transformation eligibility. Stable container keys keep definition identity separate from exact content. Recursive entry retains module paths and associated generic/header context while executable bodies, nested containers and macro templates remain opaque. Checked context/header metadata and allocation admission bound collection. Two source-fact acceptance checks pass; no native comparison or guest execution is claimed for this component. Body-local namespace publication, executable entry from associated containers, local-binding ribs, generated publication, namespace seals, guest VM and M012 request accounting remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 293 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-container-entry-checkpoint.json`. Release remains incomplete.

## M007 function entry from original containers

Explicit function entry from original module/trait/impl containers retains exact source/container/body keys and declaration membership, transform guards, captured textual frames and producer ancestry. Module entry starts a fresh import namespace while associated containers inherit enclosing lexical imports; retained declaration contexts preserve frozen aliases. Shared child-entry machinery bounds the combined declaration chain. Macro calls directly in module/trait/impl/extern item lists now register declaration-producing frontiers; prior demand survives function entry with the original producer identity. Four source checks pass. Actual pinned hygiene and selected expansions from full original definition forests match seven native macOS calls; a separate native witness rejects an enclosing body alias in a module. No guest VM or complete diagnostic equivalence is claimed. Linked external-module wrapper integration for this entry path and source local-binding discovery remain pending M007 integration; generated publication, seals, guest VM and M012 request accounting remain later required work. M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 298 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-container-body-checkpoint.json`. Release remains incomplete.

## M007 linked original container entry

Linked original container entry wraps the existing retained catalogue and preserves the complete external-module source chain. Local/recursive-container and function entry validate every current/child/root ancestor and exact container/body/declaration revisions before access. Existing linked invocation and import-target resolvers retain module outer frames alongside captured body/container frames. A three-file source acceptance rejects stale/missing ancestors, wrong parent containers and opaque-parent lookup. Actual pinned expansion from full original definition forests matches nine native macOS calls, including a container-local macro captured by its deeper module and local frozen imports across file/body redefinitions. This closes the linked-container wrapper integration gap; no guest VM or body-local global namespace publication is claimed. Source local-binding discovery and full source integration remain pending M007 work; generated publication, discovery seals, VM integration and M012 request accounting remain later required work. M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 300 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-linked-container-checkpoint.json`. Release remains incomplete.

## M007 original parameter/let binding candidates

Explicit original body entry retains owned parameter/let pattern candidates with stable definition/block/form/name/duplicate identities, raw and NFC names, ranges and visibility positions. Parameter candidates start at body entry; let candidates start after the complete statement, preserving shadowing initializer and let-else ordering. cfg/cfg_attr filter parameters and whole statements; nested executable/template opacity remains. Bounded iterative pattern traversal prunes expression/item/token-tree subtrees, with checked candidate/metadata/syntax/frontier admission. Mut/ref/at and record shorthand are explicit syntactic bindings; other names remain BindingOrConstant candidates and alternative occurrences remain distinct attachments until resolution. Three source checks cover patterns, scope/visibility, cfg, opacity, Unicode and identity stability across initializer edits. These are source facts, not local-variable publication, live type checking, native differential or VM execution. Receiver/loop/match/let-chain bindings, pattern-constant and alternative equivalence resolution, source declaration-rib lookup and generated-local integration remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 303 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-original-bindings-checkpoint.json`. Release remains incomplete.

## M007 original source local-name/rib lookup

Exact entered source/body now builds an immutable original SourceLocalIndex from owned candidates, typed pattern outcomes, declaration ribs and producer frontiers. Missing ambiguous resolutions preserve name/context-specific NeedInput; constants do not publish locals. Resolutions must belong to retained source, explicit bindings cannot become constants and duplicate resolved patterns require equivalence/diagnostic resolution. Checked logical source slots admit simultaneous parameters/destructuring and preserve initializer/block capture order; prior producers begin after their declaration start so the current call is not its own frontier. Named-function roots remain isolated; source/body/range/opaque guards apply before lookup. Existing LocalScopes gains known-name unresolved rows, while admitted pinned hygiene facts are shared by Arc without cloning/reimplementing marks. Four source acceptance checks pass. The actual native collision fixture now uses original source binding discovery/ribs with pinned marked tokens; source-derived original initializer selections match native definition-site/caller/outer values [7,31,7]. Generated-local selection remains the separate supplied-fact kernel comparison, not generated-source publication or guest VM. Additional binding forms, pattern equivalence, closure capture, linked/query/service integration and generated-local publication remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 307 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-source-locals-checkpoint.json`. Release remains incomplete.

## M007 linked original local-name lookup

LinkedEnteredMacroIndex now builds a borrowed LinkedSourceLocalIndex retaining the same complete external-module source lease. Construction and every local lookup validate current/parent/root selections before delegation; existing exact body/range/opaque and typed pattern/hygiene/declaration-order guards remain. Nested-function and local-container function entry reuse the linked body type with isolated local roots. One three-file source acceptance covers shadowing, parameters, named-function isolation, container entry, stale/missing ancestors, body-key changes and opaque-parent rejection. Four selected original literal initializer values match actual native macOS execution using pinned root hygiene and source-derived binding identities. This closes linked original-local wrapper integration, not arbitrary constant evaluation or guest VM execution. Additional binding forms, closure capture, pattern equivalence, generated-local publication and query/service consumption remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 309 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-linked-locals-checkpoint.json`. Release remains incomplete.

## M007 original for-pattern binding scopes

Original for-pattern candidates now use an explicit stable origin tag and the pinned actual loop-body block scope. Iterators retain enclosing/iterator-block scope; tuple/nested loop body bindings restore outer names after exit. Whole-loop cfg removes iterator/body facts; missing iterator/body boundaries fail explicitly before pinned recovery blocks can masquerade as bodies. Pattern macros retain enclosing invocation scope and source range while a bounded pruned producer scan adds a body-entry declaration demand, preventing successful local absence before expansion. Three source acceptance checks pass. Six original variable selections using pinned root hygiene and source literal initializer/iterator elements match actual native macOS execution. This is scope/identity evidence, not guest loop execution or general iterator/destructuring evaluation. Pattern-expansion publication, additional binding forms, closure capture, pattern equivalence and query/service/VM integration remain pending; M007 and release remain incomplete.

MacOS Rust1.99 Gate01 passes with 313 actual host/doc tests, strict Clippy
and format checks. Evidence: `artifacts/macos-first/m007-for-bindings-checkpoint.json`. Release remains incomplete.

## M007 original match-arm binding scopes

Original match-arm candidates use explicit stable origin tag Match=4 and distinct pinned arm scopes. Guard and expression visibility starts after the pattern; scrutinee, neighboring arms and post-match names retain their own bindings. Disabled arms are omitted, missing pattern/arrow/expression boundaries fail, and bounded pattern scans record typed declaration-producing demands at the arm visibility frontier. Three source checks and five source-derived original binding selections versus actual native macOS pass. This is binding scope evidence, not guest match execution or general pattern evaluation. Generated publication, additional binding forms, closure capture, equivalence and query/service/VM integration remain pending; M007 and release remain incomplete.

macOS Rust1.99 Gate01 passes with 317 actual host/doc tests, strict Clippy and format checks. Evidence: `artifacts/macos-first/m007-match-bindings-checkpoint.json`. Release remains incomplete.

## M007 original method receiver bindings

Original method receiver recipes use explicit stable origin tag Receiver=5, pinned SelfParam name and header ranges, root body visibility and explicit binding classification. Header cfg filters candidates; body edits preserve identity; nested blocks inherit while separately entered named functions do not. Local lookup admits the exact self keyword without weakening macro-name or raw-identifier validation. Three source checks cover six receiver forms and four standalone native macOS programs agree with source-selected receiver identity and fixture instance literal field observations. Every native case retains its own source/compiler/output/report. This is receiver binding scope evidence, not guest method dispatch, ownership checking, general field evaluation or receiver typing. Generated publication, other binding forms, closure capture, equivalence and query/service/VM integration remain pending; M007 and release remain incomplete.

macOS Rust1.99 Gate01 passes with 321 actual host/doc tests, strict Clippy and format checks. Evidence: `artifacts/macos-first/m007-receiver-bindings-checkpoint.json`. Release remains incomplete.

## M007 supplied statement-local publications

The source local index now builds an immutable supplied statement-declaration view with exact source/body/producer/registered-expansion/output provenance. Emitted binding identities use explicit stable origin encoding; declaration order is preserved after the retained call, including source argument visibility and repeated same-name shadowing. Empty batches clear the original producer, emitted macro ribs and pending rows preserve typed demand, and malformed, duplicate, stale, oversized, pattern and attribute publications refuse without changing existing indexes. Deterministic bounded indexes avoid repeated whole-input scans. Linked construction and lookup validate all ancestors. Four source checks and a linked three-file check pass; actual pinned marked declaration name/context/literal output and source-selected caller identity agree with two standalone native macOS observations. This is supplied statement-local/hygiene integration, not general generated AST discovery, nested generated scopes, service epoch/output attestation or guest VM execution. M007 and release remain incomplete.

macOS Rust1.99 Gate01 passes with 327 actual host/doc tests, strict Clippy and formatting. Evidence: `artifacts/macos-first/m007-local-publications-checkpoint.json`. Release remains incomplete.

## M007 acceptance audit and next task

M007 revision-4 component acceptance is complete on macOS Rust1.99.0: textual/path scopes, owned pinned hygiene and name-class facts, raw/NFC identifiers, $crate defining-unit routing, dependency renames/reexports, original-source selection, definition/call-site collisions and supplied marked statement-local publication. The named MX011 and combined MX012 cases match actual native macOS; every recorded M007 acceptance and registered fixture test is observed passed. General generated AST discovery and complete executable source forms belong to subsequent T020/M013 integration; service epochs, coordinator output attestation, guest VM execution, JIT/native/runtime and Linux release acceptance remain unimplemented or unverified. This closes the Section57 M007 dependency without claiming later tasks or release completion.

All 160 acceptance entries and 126 M007 registry references are accounted for. Gate01, strict Clippy and formatting pass with 328 actual macOS host/doc tests. Evidence: `artifacts/macos-first/m007-final-acceptance-audit.json`. M008 is next; the full project goal remains active.

## M008 typed query polling and dependency-stack suspension

M008 is in progress. The provider and Database::fetch interfaces now return QueryPoll::Ready(Result<...,Diagnostic>) or typed InputRequest demand. Missing input unwinds provider and memo-validation collectors without recording a completed dependency/value; stale memos are removed, parent queries cannot ignore a suspended child, and re-entry observes newly published immutable inputs. Structured terminal errors preserve completed error memo behavior, cycle/depth non-cacheability and retained allocation accounting. Explicit owned expansion/captured-source requests validate bounded identity/path metadata and use stable encoding; request payload ownership keeps the poll enum compact. Suspended provider runs are counted honestly. Five framework tests cover nested suspension, ignored children, revalidation, invalid request eviction and request identity; prior fixtures were migrated to typed ready/error assertions without weakening checks. MacOS Rust1.99 Gate01, strict Clippy and formatting pass with 333 actual host/doc tests. Actual expansion-record query integration, coordinator fulfilment/epoch/cancellation/late-reply contracts, saved VM phase dispatch and the full MX013 acceptance remain pending; no guest effect or macro-helper invocation claim is made.

Evidence: `artifacts/macos-first/m008-query-poll-checkpoint.json`. Full MX013, coordinator and VM phase integration remain pending; the full goal stays active.

## M008 frozen demand lifecycle and saved phase ownership

Frozen demand context binds request/session/consumer unit, base revision, source and optional native generation, epoch and compile environment. Each owned token binds pure query, exact frame/body/phase, input identity and checked sequence. Waiters share one operation; reply admission requires the originating start token and exact context/input. Cancellation closes the scope; stale, cross-epoch, wrong-waiter, unstarted and duplicate replies cannot publish. Failures share within the context without another start. Saved phase payloads retain ownership on wait/rejection; successful token claims are consumed once even when a token is copied. Retained key capacities, request counts and failure-message metadata are checked. Nine framework tests pass. Scheduling decisions do not grant native authority or execute a helper. Actual session lifecycle/serial scheduling, schema-checked ExpansionRecord adoption/query re-entry and concrete guest VM phase/operand integration remain pending; M008 and release remain incomplete.

Gate01, strict Clippy and formatting pass with 342 actual macOS Rust1.99 host/doc tests. Evidence: `artifacts/macos-first/m008-demand-tokens-checkpoint.json`. Full MX013/record/query/VM integration remains pending; the project goal remains active.

## M008 component acceptance and next task

M008 revision-4 component acceptance is complete on macOS Rust1.99.0. Every provider/fetch entry returns typed Ready(Result) or NeedInput. Missing input unwinds incomplete collectors and memo verification without cached absence or old-value fallback; parent suppression is rejected. Frozen request/session/unit/revision/source/native-generation/epoch/query/phase tokens validate originating replies, reject cancellation and stale/duplicate/cross-epoch replies, and share one operation/failure per input. Owned saved payloads require checked record installation and successful token consumption once. Schema-checked ExpansionRecord transport is adopted only under exact key/output/base-source revision; derived publication is append-only, idempotent, query-stack guarded and bounded by active-run metadata limits. Pure-provider import audit rejects ambient filesystem/process/environment/clock/macro-service/adapter/VM paths. MX013 exercises actual pinned declarative expansion outside SourceDb once, immutable adoption, pure lookup re-entry/cache reuse and native macOS value agreement. These are implemented coordination/query/ownership interfaces. Procedural helper authority/session/termination/serial scheduling is M009, generated grammar/discovery is M010-M013, and concrete VM/JIT/effect fault injection is later T/M integration. No full release, guest-effect execution or Linux acceptance is claimed.

All 23 selected acceptance checks pass. Gate01, strict Clippy and formatting pass with 351 actual macOS host/doc tests. Evidence: `artifacts/macos-first/m008-final-acceptance-audit.json`. M009 is next; the full project goal remains active.

### M009 bounded transport and helper lifetime — in progress

The matched-server probe now places a finite 256 MiB wire allowance before the pinned decoder and keeps a 64 KiB stdout preview. The bounded reader refuses an unterminated or oversized protocol line rather than inventing EOF. Owned helper lifetime has a watchdog that kills and reaps the actual child on its ten-second probe deadline; cancellation reaps once and joins the watcher. Five IO checks and four real synthetic process checks cover boundaries, pinned protocol decoding, timeout, cancellation, actual exit status and invalid deadline cleanup. The normal-exit check waits for the OS process status, because pipe EOF alone is insufficient. This is M009 infrastructure, not session completion: per-unit epochs, all three procedural forms, exact-once requests, stderr quotas, per-invocation deadlines and MX014–MX017 remain pending. Initial version/raw handshake limits also remain pending. Full release and Linux acceptance remain unverified.

Gate01, strict Clippy and formatting pass with 360 actual macOS host/doc tests. The matched-server probe passes with the guarded transport. Evidence: `artifacts/macos-first/m009-transport-lifetime-checkpoint.json`. M009 and the full project remain active and incomplete.

### M009 real procedural sessions — in progress

Bounded version/protocol probes now preserve separate stdout/stderr and actual exit status, time out hanging helpers, and refuse oversized replies. Request-scoped watchdog deadlines can be disarmed between operations and rearmed without restarting a consuming-unit/epoch helper. The session loads verified Cargo-reported proc tools only in the matched external server, freezes env/cwd/unit/epoch, executes serially, caches one owned outcome per exact invocation, and refuses changed identities or stale unit/epoch requests. Native failures terminate/reap the helper and never retry. Actual owned returned span ranges and registered contexts are remapped without exposing private IDs; all seven grammar slots use the pinned token parser, with associated-item scaffolding retaining original returned token/spans. The controlled edition2024 `lrr_fixture_macros` crate has all three forms and native panic/crash/hang/oversize/malformed fixtures. Actual native outputs and generated test names agree, and unit/epoch counters and asynchronous cancellation are exercised. The service constructor consumes schema4 preparation, reachable proc units and frozen compile env; fulfil checks the exact frozen demand and returns an immutable record for later accept/adopt. Its denial/staleness guards are tested. Positive service fulfil/adoption, noisy output limits, general span operations, child-process supervision and complete generated discovery/execution/phase telemetry remain pending. M009 and release remain incomplete.

Gate01, strict Clippy and formatting pass with 374 actual macOS Rust1.99 host/doc tests. All 23 selected M009 acceptance checks are observed passed. Evidence: `artifacts/macos-first/m009-session-checkpoint.json`. The full M009 contract and project goal remain active/incomplete.

### M009 service adoption and bounded supervision — in progress

An actual pinned Cargo plan/preparation now builds only the synthetic proc dependency while its live consumer retains a cold undefined call. Compiler argv evidence verifies that the ordinary live source is never compiled/check-analyzed. Stable schema4 publication and frozen compile env feed the authorized service; its exact demand starts/adopts one native counter output, supplies immutable checked records to pure lookup re-entry, and resumes the saved payload only after adoption. Same-operation reuse reads the session cache without permitting a new native call; stale context, cancelled coordinator and wrong engine refuse. Artifact identity binds both exact compilation unit and artifact content. The pinned planner’s measured resolved Named strip JSON form is checked without discarding profile fields; malformed variants refuse.

Native stdout/stderr noise limits, cached-result cancellation, owned helper-group descendant cleanup and nonblocking inherited reader/writer holders now have actual macOS checks. Request/reply wire allowances are independent, serial and bounded. Native faults terminate/reap the direct helper and never retry. The descendant test verifies its own child PID disappears; blocked /bin/ps is not a pass. E-M009-001 preserves the actual standard-library-only inherited-pipe witness and the narrow safe Rustix1.1.5 OS API extension, keeping std ownership/threads/timers, all selected backends and local unsafe boundaries. Session record accounting includes actual token/text/vector capacities and bounded diagnostics while stable fingerprints remain allocation independent. No hardened native containment against deliberate process-group escape is claimed. Full span capability/protocol robustness, generated discovery/resolution/guest VM and phase telemetry remain pending. M009 and the project goal remain incomplete.

Gate01, strict Clippy and formatting pass with 384 actual macOS Rust1.99 host/doc tests; all 33 selected M009 checks are observed passed. Evidence: `artifacts/macos-first/m009-service-supervision-checkpoint.json`. M009 and the full project goal remain active/incomplete.

## Edition scope amendment — October 13, 2026

Support every released official Rust edition: 2015, 2018, 2021 and 2024 for roots, dependencies and macro origins. Preserve the owning edition in parsing, lookup, prelude and macro contexts, patterns, temporary lifetimes and unit/cache identities. Standalone --edition selects it explicitly (default2024); Cargo target declarations remain authoritative. The experimental future selector is excluded. This supersedes former root2024-only acceptance. Use unchanged official rustc cases for edition-paired interpreter acceptance; parser admission and scoped program controls alone are not full language/compiletest compatibility. The active implementation goal remains incomplete; interpreter convergence still precedes guest JIT.
