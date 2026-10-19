# M020 placeholder assumptions — separate solo source review

Baseline a55023600e0eade46616011d0170cfead210e7b9; LOCAL-049 / E-R5-014.
Initial review: stable region_constraint.rs declares panic-only relation types;
Assumptions::empty itself calls their default/freeze. This is compiler client
infrastructure, not evidence of a Rust runtime result. Validation and final source
fingerprint are pending below; this document alone closes no requirement.

The exact pinned rustc_data_structures helper uses FxIndexSet/FxHashSet and
rustc_index::BitMatrix, already available in the shared stable closure. Frozen
is a safe private owned wrapper with Deref, without DerefMut. Preserve all helper
method bodies and the original 18test module; only import/module aliases and a
doc example path change. The original region_constraint source must remain
retained; the compiled stable binding should select the actual helper instead of
its panic definitions. Nightly should retain its original rustc_data_structures
binding. A deterministic independently hash-pinned projection is needed in both
bootstrap import and audit; merely changing the installed-file digest would not
prove upstream reuse.

## Eight recurring failure modes (distinct from MA001–MA008)

1. No assertion/expression evaluator: changes are in the shared upstream library
   data structure and supplemental inference host controls. Ordinary VM routes
   and expression/pattern lowering are untouched.
2. No literal/break-only generated engine: source_terms -> actual Cx -> shared
   rustc_type_ir assumptions is the same host route for original/emit/wrapper.
   No macro-name/expansion grammar branch is admitted here.
3. No context-specific member/numeric inference: no member selection, defaulting,
   unification or inference-rule change; exact compiler relation methods supply
   graph reachability/closure. There is no additional local graph algorithm.
4. No generated-only refusal: existing configuration/header/generic/source guards
   retain ownership. Missing full facts remain a typed readiness hold. A helper
   control must never manufacture ReadyProvider or select an impl.
5. No duplicate patterns/eager branches: no source body, pattern-plan, thunk or
   unselected result preparation. Host relation controls are not guest programs.
6. No repeated source planning: assumptions construct/freeze actual IR relations
   on the cold inference path. The compiler's closure calculation retains its
   original allocation and iteration behavior; installed VM dispatch gains no
   parser, source IO, macro expansion or plan reconstruction.
7. No namespace scan: relation vertices are given interned region values. No
   global catalog construction/comparison or unrelated namespace invalidation.
   Existing prepared source currentness remains the owning dependency service.
8. No immutable body clones/full-file search: cloning assumptions clones their
   compiler-owned relation data, as upstream does. This is distinct from cloning
   BodyIndex for call entry; no frames, body indexes or node searches change.

The separate MA004/MA007/MA008 wider architecture findings remain open. No claim
that a component test repairs their runtime/origin/transactional requirements.
No unsafe, static borrow checker, resource policy, alternative solver or new
native path. Higher-ranked source binders, InferCtxtLike/delegate, solver region
constraints/rollback/visibility/leak checks, full typed-fact publication,
canonical responses, actual root evaluation and runtime/member migration remain
unimplemented or unverified prerequisites. Existing held custom consumers remain
held. Complete M020/milestone/interpreter/JIT/Linux/performance acceptance is not
established. Zero new stock-main gain follows from compiler helper tests.

## Final-source pass before host controls

Reviewed source fingerprint
`215b248cb2d7f0dc7dc129d09ae1829166d140bc3613d1dfa55351b82474cde9`.
Compiled Rust projections: four files differ from originals (two binding/module
files, helper import aliases, Frozen doc example path). Five registered outputs
include the unchanged copied test file. Eight shared crates/152files/2238379bytes;
all retained originals byte-identical to actual pinned Git objects. Compiler
helper bodies and 18tests unchanged. The removed production route is the stable
panic-only relation binding. Nightly imports remain unchanged.

InferenceContext::placeholder_assumptions_for_next_solver uses the compiler's
actual field ownership and initial state. variables.rs insert/get/has methods
come from exact compiler infer/mod.rs via sync_upstream_inference_context.py;
only Cx/lifetime translation and the stable std::range import. The compiler map
is deliberately outside snapshots upstream (its own FIXME); no local rollback
semantics is invented or advertised. Existing ReadyProvider guard remains Err.
Assumptions::new (elaboration) is unexercised because the full inference delegate
is still absent. Supplemental nonempty relation controls use actual source Type
and span, with explicit host placeholder Regions, not a fake source declaration.
No guest semantic result or binder source inference follows from these controls.

V342 reproduced the exact stable panic at original region_constraint.rs:47 in
the actual all-edition/source-origin matrix; other4controls passed, tested
source/HEAD/binary guards match. V343 builds, final controls and unchanged helper
test terminal results are recorded separately below when observed. Fresh import
is byte-identical to the retained tree and inventory; a deliberately edited
projection still fails the audit after updating its inventory digest. This
checks the deterministic projection guard, not a hash-only edited-source waiver.

Pinned compiler rustc_data_structures/src/fx.rs:1-6 reexports rustc_hash::
FxHashSet/FxBuildHasher and aliases indexmap::IndexSet<V,FxBuildHasher>.
The shared data_structures::HashSet/IndexSet aliases are exactly those types;
this is namespace relocation, not a different graph representation or hasher.
The stable binding selects the same TransitiveRelation as nightly, and the
upstream tests' original `use super::*` imports it directly. Frozen's only text
change is its documentation example path. No HashSet iteration ordering or
closure algorithm is replaced. Original licenses remain in the retained root.

Actual compiler assumptions-map insertion keeps both no-escaping-bound-vars
assertions. Missing map entries keep upstream get().unwrap(); a missing universe
is an invariant failure, not successful empty assumptions. The host constructor
creates an actual empty map. Input facts/currentness do not become ready from
constructing a map or relation. Full inference snapshots/forall/solver response
contracts remain open, including upstream map rollback limitations.

Final V343 library74043, helper47213, CLI2624 and controls32307 all terminal exit0.18/18unchanged upstream relation tests and5/5source-context controls pass; source/HEAD/test/CLI guards match. Fresh-import/audit control48724 exits0. V342 library15301 exits0 and control86571 exits1 at the reproduced stable stub; all failed evidence retained separately. The reviewed final production source did not change after validation. No actual root invocation or new stock runtime pass follows. Full gates remain held. Exact commands, hashes, scope and terminal records: worktracks/evidence/2026-10-19-v342-v343-upstream-region-assumptions.json.
