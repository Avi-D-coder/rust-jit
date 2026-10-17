# Exact cold-header to reached lexical-body admission — LOCAL-024

Pre-code record, October 16, 2026. M014/M017/M018 common source admission;
unchanged upstream `tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs`
is the primary lexical witness. No original is edited. Owner prerequisite is
immutable `common-local-declaration-owner-v1`, COMPONENT SHA256
`854bf5b0f9122cf6096920a17731204262f64702f6405e1459d7f7a195cbf20b`,
source `17f855ff75fc14d19c5e534c01c3dfd32841f2896c29b1c3f7390ca122459660`.

## Invariant and concrete failure

Cold `Database::prepare_source_local_plan` registers producer-free entered item
source/header/context plus actual `LocalDeclarationOwner` BodyKey using opaque
PreparedBodyContext. A function's executable body remains lazy. Reached
`LexicalAdmission` independently certifies that exact function and its current
caller/source/namespace. `Database::publish_lexical_function` currently rejects
these genuine three-existing/one-missing derived facts as a partial publication.
Header registration must compose with body admission without admitting arbitrary
partial rows, guessing a parent from a lexical rib or weakening either proof.

## Shared route and bounded change

Keep the existing reached call route: entered BodyIndex/call node -> exact
LexicalFunctionPlan -> LexicalAdmission::validate/revalidate ->
Database::publish_lexical_function -> ordinary FunctionSignature/SourceCallable ->
ordinary call/return/cleanup. Extend only the existing publication kernel. A
three-row prior state is accepted only when the sole missing row is the actual
DefinitionBody; all existing source/header/context values and fingerprints exactly
match the genuine admission; and the private derived LocalDeclarationOwner equals
the proof's actual caller BodyKey. Publish only that missing executable row after
existing checked-budget/revision preflight. Preserve the zero/all-four cases and
all other partial refusals. Remove only the now-overbroad count-only rejection for
this precisely proved state; no second resolver, parser or evaluator.

Current lexical admission remains restricted to its historical exact original
source numeric plan. Generated/retained arbitrary local-call admission and nested
local-function BodyEnvironment capture remain explicit common prerequisites; this
repair introduces no new origin exclusion or claims those gaps complete.

## Authority and cost

Coordinator/active-run, query revision, source revision, owning caller rows,
namespace stamps, exact source syntax and selected declaration proofs remain
mandatory. The new compatibility check reads one exact fixed-size derived owner
row and compares existing three metadata rows. It performs no world scan, parser,
body clone or source preparation. Existing LexicalAdmission full namespace census
and cold plan reconstruction remain held MA audit/mode6/7 debt; this slice does not
extend them. No new memo/input roots or accounting registry. Budget failure leaves
all rows/revision unchanged. Reached code still uses ordinary body operations and
cleanup. Existing immutable owner publication remains attached to its frozen bytes.

## Required evidence / acceptance hold

Add LRR authority controls around the unchanged upstream source: genuine cold
header publication followed by reached admission in all four editions; missing or
wrong real owner; arbitrary partial metadata; conflicting attachment; checked
budget failure; idempotence/no child body or thunk preparation. Validator alone
runs Cargo on copied exact leaves. Source review/hashes follow after edits.
All controls are uncompiled/unexecuted until actual terminals. No full fixture,
architecture gate, generated/wrapper/standard macro or JIT acceptance is claimed.

## Separate source review and exact evidence boundary

Reviewed after formatting against the two-leaf fingerprint recorded in
`worktracks/evidence/2026-10-16-lexical-header-admission.json`. The changed
publication now additionally compares actual source/header/context/body values,
because semantic header fingerprints intentionally omit attachment locations.
Any present derived owner must have the exact value and fingerprint. In the
new three-existing case the staged map must contain exactly the genuine selected
DefinitionBody. Every comparison/allocation, metadata-cap and checked revision
operation remains before `inputs.extend`; only that one missing row is published.
The new tests distinguish exact typed refusal messages, including a forged stored
fingerprint paired with a conflicting actual context. They do not catch errors as
successful language execution. All controls and complete-main execution remain
uncompiled/unexecuted in this handoff; rustfmt alone completed.

Recurring modes (distinct from audit findings MA001–MA008):

1. `publish_lexical_function` only registers source facts already certified by
   `LexicalAdmission`; no assertion/expression evaluator is added. MA001 and
   MA007 broader routes remain held elsewhere.
2. The same ordinary DefinitionBody/FunctionSignature/SourceCallable route is
   retained; no literal or break output executor. MA004/MA005 not closed.
3. Actual selected lexical signature and ordinary parameter/result machinery
   remain authoritative. No assertion identity, method whitelist or new numeric
   inference rule. Historical closed numeric lexical profile is unchanged;
   MA003/MA006 broader inference/member obligations remain held.
4. No origin branch is added here. Existing generated/retained arbitrary lexical
   admission and actual nested owning-body capture are still missing common
   receipts. Tests exercise only the unchanged ordinary witness and explicitly
   do not claim the four-origin matrix passed.
5. No pattern algorithm or unselected executable arm is prepared; publishing
   the actual reached local body fact is not lowering or entering that body.
   The source trace controls forbid child BodyStructure/Thunk preparation.
   MA002 remains outside this repair.
6. New checks are exact fixed-row comparisons plus a constant-size owner digest.
   Full header equality is O(H) at the existing coordinator admission. Existing
   LexicalAdmission source reconstruction/full namespace census remains debt;
   there is no new planning on installed op dispatch and no speed claim.
7. The only new lookup is InputKey::LocalDeclarationOwner(exact definition).
   The owner7 metadata exclusion from lexical_namespace_stamps is preserved;
   source/epoch/record/namespace authority is unchanged. The existing global
   lexical census is held debt, not claimed a relevant-dependency replacement.
8. No parser/index/body deep copy or whole-file node search is added in
   production. Tests select an actual indexed witness call to construct its
   existing genuine source proof. No new runtime index/verifier path (MA008).

No public helper/proof getter, base-input authority, source input schema or memo
framework changes are needed. No Rust/Cargo/native command was executed. Sole
validator should compile the exact two leaves over owner7 and run the three new
controls plus the existing lexical admission/namespace controls only as needed.
A pass would establish this publication contract, not whole fixture/engine/JIT
acceptance. Original/retained/generated/wrapper local body admission, qualified
Type prefix, genuine Type entry integration and installed constructor preparation
remain separate active work; old failed sources/evidence stay unchanged.
