# LOCAL-049 / WT05-02 / M020 comparison trait source conformance

Before-code record: baseline f4a74d3bf0f8ae1b0dc276c5f87cbdd7e708fda4.
Read current AGENTS.md, LOCAL-049 reservation, E-R5-014, Section53.0 and the
macro-specific architecture audit. Own only facade/core/cmp.rs trait declaration
headers/semantic attributes/imports, exact raw library/core/src/cmp.rs,
tools/bootstrap_trait_solver.py, vendor/rustc-trait-solver.files.json and this
record. Parent owns parser/collector/default policy/source query registration,
root and test integration. No tests/Cargo/compiler/interpreter/new agents/commit.

Invariant/minimal witness: unchanged enum-discriminant/repr128.rs reaches
DiscriminantKind::Discriminant and PartialEq<U> inference. The source adapter
must consume actual pinned comparison trait declarations, not guessed defaults
from facade names. Current facade PartialEq/PartialOrd use ?Sized without the
pinned PointeeSized supertrait, and Eq/Ord omit pinned const-qualified
supertraits. Do not strip const tokens to hide missing source-clause transport.

Exact source is library/core/src/cmp.rs from Rust pin
ae0fe511b15ab760d7e963b13389d1bc2ab42b18, retained unchanged under existing
vendor MIT OR Apache-2.0 licenses and manifest support-source registration.
Align PartialEq/Rhs at253, Eq at353, Ord at1045, PartialOrd at1458 with actual
const-trait syntax, PointeeSized subjects, original generic defaults and
[const] supertrait modifiers. Copy actual header attributes rather than
inventing flags from names. Preserve every existing facade trait method/body
and runtime impl/function/enum definition. This is bounded source metadata
alignment, not full library, method contract, const-evaluation or static-check
compatibility. Retain existing Sized import needed by method where clauses and
add actual marker::PointeeSized import.

Common admission source review before activation: vendored
ra-ap-parser::items::opt_item_without_modifiers defers const followed by trait;
opt_item consumes actual const modifier and dispatches traits::trait_ to an
ordinary TRAIT node. generic_params::path_type_bound consumes [const] and
retains it in an ordinary TYPE_BOUND. No parser bypass/drop/rewrite required.
Existing SourceDefinitionSemantics inspects actual direct header const token
for trait conditionality. Existing common source_predicates still returns
QualifiedBound pending for const TypeBounds; that real producer gap is exposed,
not patched by this slice. TraitHeader alone lacks a dedicated const field, so
Q83's retained direct syntax remains the actual constness proof.

Shared route: configured facade source -> ordinary module/import/prelude and
retained parse/outline -> actual Q75 header/TraitDeclaration/DeclarationGenerics
-> Q76 source lang identity, Q78/Q80 source clauses, Q83 semantic constness,
Q84 default policy and Q85 source flags -> future authenticated preflight facts.
Original/generated traits join the same source identities/hygiene and retained
AST route. No new registration/provider API, same-name flags, primitive operator
shortcut or independent solver is added. MA001/MA004 alternate evaluator paths
are not extended; legacy source/boolean runtime routes remain migration debt.
No obsolete route is removed by this source-only prerequisite.

Demand/cost: normal existing selected header parsing once per memo; extra
immutable attrs/imported marker identity and predicate dependencies only. No
body/signature preparation, runtime source scan, repeated callback planning,
whole-world census or BodyIndex copy. All execution/currentness remains with
existing prepared query dependency guards; hot dispatch is unchanged.

Acceptance required, not established: actual retained const TRAIT/TYPE_BOUND
nodes and semantic attrs; ordinary/user/wrapper/all-edition source controls;
actual identities/defaults/super and implied clauses with correct spans;
explicit typed holds for missing const bound transport; unchanged repr128 real
root/runtime. Parent/validator owns these observations. No selected original is
counted passed by import/source alignment, and interpreter/JIT gates stay held.
All-eight final source review/fingerprints to follow; no unsafe/cap/static
rejection or alternate runtime path is authorized by this slice.

Final source handoff (unexecuted component)

Actual pinned trait headers and immediately preceding attribute blocks were
copied as exact substrings into the existing facade, with source-line/license
comments. PartialEq keeps #[lang="eq"], diagnostic/stability/const-unstable attrs;
PartialOrd keeps #[lang="partial_ord"] and its original diagnostic/lint attrs;
Eq/Ord keep their actual diagnostic/stability/const-unstable attrs without
inventing language attributes. All four keep written const; PartialEq/Rhs and
PartialOrd/Rhs use actual PointeeSized, Self has explicit PointeeSized on all
four, and Eq/PartialOrd/Ord retain actual [const] comparison supertraits.
No trait name is used to assign a compiler semantic flag. Q76/Q83/Q85 consume
actual source receipts through existing shared entry points. Inert-attribute
admission uses syntax::outline::{attributes_from_meta,is_builtin_inert_attribute}
and predefined tool attributes; no new transformation whitelist is introduced.

Source admission inspected exactly: vendor/ra-ap-parser/src/grammar/items.rs:271–308
only classifies const IDENT/_/mut as a const item; opt_item:150–155 instead
retains the const modifier before dispatch to the same ordinary TRAIT node.
traits::trait_ retains ordinary generic/type-bound/where nodes. The existing
generic_params::path_type_bound:201–207 retains [const] in TypeBound. This is
source inspection, not a parser execution result. Q83's direct retained header
const token yields genuine written trait conditionality. Q78's ordinary collector
still returns QualifiedBound for const-modified TypeBounds, so complete
Eq/PartialOrd/Ord source clauses and const conditions remain pending. No const
modifier was dropped to make a smaller producer appear ready. The shared
const-bound transport is the next prerequisite; full implicit/default/super/
implied/predicate domains remain separately owned, including colon/where policy.

Removal boundary: no old runtime/member/operator path is changed or removed.
Facade traits now expose genuine required source inputs; old primitive/member
and bool paths still require upstream migration and cannot certify solver/root
acceptance. No new operator shortcut, builtin implementation or source-specific
runtime route is added. Existing method signatures/default bodies, Ordering,
Reverse, functions and all impl declarations remain unchanged, including the
facade method-contract differences not owned by this bounded header slice.

All-eight source review at hashes below: (1) no assertion evaluator; (2) no
macro-output executor; (3) no numeric/member inference rule, only actual source
headers; (4) no original/generated exclusion or separate parser; (5) no pattern,
static checker or eager body/signature demand; (6) ordinary retained header
queries prepare once, no installed-execution planning; (7) actual marker imports
add relevant existing source dependencies, no hot full-world scan; (8) no body
copy or whole-file node lookup. The bootstrap/raw manifest update has no runtime
entrypoint. Raw compiler/library source may contain upstream unsafe declarations;
no local production unsafe or runtime cap is introduced. Cold costs remain the
existing selected header/attribute/import preparation, with no measured scaling
claim. MA001–MA008 debt is not closed by this record or source alignment.

Read-only byte/source checks completed: all four attribute/header blocks match
the exact pinned Git object; reversing ONLY those four header replacements and
one marker import reproduces the prior entire facade byte-for-byte. Thus all
existing bodies and unrelated declarations are preserved. Exact raw bytes,
size, one manifest row and support-source membership agree; scoped git diff
--check returned 0. No test/build/compiler/interpreter was run. Full bootstrap
--verify-upstream audit and all parser/source-control/root/runtime matrices are
validator-owned and remain unexecuted by this worker.

Final SHA256 source fingerprints:
- facade/core/cmp.rs: 646abd37512e1ddf2953789dd072f305572fe928600d702f7a43f20427fd6bed
- tools/bootstrap_trait_solver.py: f445c7781cdbd9d7c934fe04581a8239747c009153089476da447c3bcaf94665
- vendor/rustc-trait-solver.files.json: 9f26ed8f7c6441c55d8259eb2f265435308114251f66185b5f1d99aba1b02073
- raw library/core/src/cmp.rs: 437c8518b75c076da2437f913d95a07ff8c2bbfe8ae1dcce2571db1dcb9e7980
Raw original size 85845, pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; retained
under existing vendor LICENSE-MIT and LICENSE-APACHE. No public API change and
no fabricated complete provider or readiness result. Primary unchanged repr128
runtime witness, broader trait headers/const clause transport and all gates stay
held until actual producer/root integration and terminal validation establish
that evidence.
