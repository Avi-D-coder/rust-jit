# M020 bounded pinned WF value helpers

Owner `/root/wf_value_helpers`, delegated by implementing lead `/root` under the
existing WT05 reservation, branch `codex/lrr-implementation`. Scope is
`crates/lrr/src/traits/upstream/wf_values.rs`,
`tools/upstream_wf_values.py`, this record and the two newly retained compiler
originals `ty/consts.rs` and `ty/consts/int.rs`. No Cargo target, source-producer,
root-ready, commit or manifest ownership is assumed.

Invariant: the demanded new-solver `infer/well_formed.rs::visit_const` must use
the exact pinned compiler's `Value::to_branch` and `destructure_adt_const` rules,
including the enum-head u32 scalar size check and impossible-shape panics. The
unchanged `repr128.rs` is the parent worktrack's runtime witness; this helper
slice does not execute it or establish any runtime gain. Pin is
`ae0fe511b15ab760d7e963b13389d1bc2ab42b18` (MIT OR Apache-2.0).

Common route: existing source/header identity and guarded immutable metadata ->
TypeIR `ConstKind::Value(ValueConst)` -> pinned unnormalized WF visitor -> same
upstream Goal predicates. Handwritten and generated source already share that
TypeIR interface; helpers add no syntax/parser/evaluator/VM or continuation
route. The ordinary VM remains responsible for reached const/static execution.
No alternate compiler semantic route is introduced or extended. The old
function-pointer WF client is owned by the lead's migration, not removed here.

The helper reads only its existing valtree and, for ADTs, actual prepared
`AdtDefinition::variants/is_enum`; the ADT worker owns real variant/field facts.
Missing metadata may not become an empty variant list. Preparation-time
construction, receipt/currentness and readiness are dependencies outside this
slice. `ValueConst::to_branch/to_leaf` and scalar conversion are O(1), shared
slices are borrowed, and no parse, namespace scan, macro expansion, IO or guest
effect occurs per access. Scalar `Size`, `to_bits/check_data/to_uint/to_u32`
preserve the compiler implementation after field spelling adaptation.

Closure evidence for this slice: unchanged retained file hashes checked against
Git blobs at the pin; registered generator transformations match generated Rust;
lead-owned frozen macOS1.99 build and relevant source controls. Full source
producer/root/member/VM, fixture/origin/currentness/cleanup matrices and M020,
interpreter and JIT gates remain held. No build/runtime test is claimed here.

Separate architecture review will record the final source fingerprints. The
eight recurring failure modes are reviewed at these changed helpers only:
1-4 no expression evaluator, macro-only payload path, inference/member special
case or generated-source exclusion; 5 no pattern engine or cold body traversal;
6 borrowed immutable tree access without planning; 7 no namespace operation;
8 no BodyIndex or parsed-file access and no immutable tree clone. This review
cannot establish the surrounding frontend/root/runtime routes and leaves their
acceptance with the lead.

## Source-only review and handoff

Separate review after generation, before lead integration: generated Rust SHA256
`c902195eb79f982ee618d6fb3d2c2ee2d07d89cc66713920a4f8934e26b3a523`;
generator SHA256
`7972da4ff8c55e5d05e767680204555b912fe91d79289eefac5023af7fbf2708`.
`python3 tools/upstream_wf_values.py` terminated exit0 with matching registered
transformations. A separate Python check ran `git -C rustc/rust show
<exact-pin>:<path>` for all four dependencies and verified both expected SHA256
and byte equality with the retained vendor originals; exit0. These commands
are provenance/source evidence, not execution of helper semantics.

Retained originals and SHA256:

- `compiler/rustc_middle/src/ty/consts/valtree.rs`:
  `1ab0c5918c3a8b8ff86a3eef72bb726e559ba43d9503433b7ab2c153d7368b81`.
- `compiler/rustc_middle/src/ty/consts.rs` (4687bytes):
  `6337715239e9bf252c749c85d1c19fb2f4b4c5fad2430046f1bcc106ceb2c52c`.
- `compiler/rustc_middle/src/ty/consts/int.rs` (22026bytes):
  `adc87f2118e4023afcd21b22dfc94553471eca2458d73515336d02808c1da1b9`.
- `compiler/rustc_middle/src/ty/mod.rs`:
  `bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7`.

Reviewed symbols: `ConstValueExt::{to_value,to_leaf}`,
`ValueConst::{to_leaf,to_branch,destructure_adt_const}`,
`ScalarInt::{size,check_data,to_bits,to_uint,to_u32}` and
`DestructuredAdtConst`. Modes1/2: these are read-only destructuring/conversion,
not expression compilation or macro payload execution. Mode3: only TypeIR
`ConstKind::Value`, `ValTreeKind` and `TyKind::Adt` select behavior; no method
name or macro-specific constraint. Mode4: no generated/original test. Mode5:
only actual value branches/ADT variants are inspected; no pattern machinery or
body/arm traversal. Mode6: `tree.kind`, `branch.items` and scalar conversion
access retained values directly; no source preparation. Mode7: ADT variants
read the owning prepared metadata; no namespace iteration/invalidation added.
Mode8: borrowed field slices and copied small value/index handles; no body
index, source tree or whole-file search. No legacy route is removed by this
helper-only slice. Surrounding source/root/runtime acceptance remains
unreviewed/unexecuted by this worker.

Lead must register `mod wf_values`, add direct `rustc_abi` dependency for
`Size/VariantIdx/FIRST_VARIANT`, register two new original manifest rows, and
collect its frozen macOS build/source-control terminals. ADT worker supplies
`AdtDefinition::variants()->&[AdtVariant]` and
`variant(rustc_abi::VariantIdx)`. Model helper imports are inherent; no extra
`well_formed.rs` import is needed. These API assumptions are integration
requirements, not fabricated source facts. No Cargo command, original fixture,
full-runtime route or root-ready grant was executed here.

## Read-only constructor invariant follow-up

Lead requested this source-only review while its build runs; helper/generator
sources remain frozen. Reviewed source fingerprints:

- `model.rs`: `2633afa2354f7f2290758df11a2cf70523264b462933a599343f215c1a3bc799`.
- `context.rs`: `b2c9148b572854cb23187f9ab6aecdb9a554a59f63e11d28ef86081dd5dac4fd`.
- `source_terms.rs`: `d09be5a84b13ce31e1dba30cb2933d18f45ffb4c065c6f6ddb794eb8b127f1bd`.
- `infer/well_formed.rs`: `3152452867457007e9aafa50592aa4391db3a7049336ab982f86530929f7c150`.
- `prepared.rs`: `97753a581968951e4d2f2a86fae8580f9d2778d7c720382520fb93aaa92024c7`.

Read-only compiler reference
`rustc/rust/compiler/rustc_const_eval/src/const_eval/valtrees.rs` SHA256
`f1258054c6b8dbc17379538685e94c39f0bf87268d990aa3266fecb66e02183b`
is byte-equal to its exact pinned Git blob. It was inspected, not imported,
ported or executed. Its `branches`, `slice_branches` and
`const_to_valtree_inner` explain the producer preconditions assumed by the
ported compiler helpers. Its node accounting is not a proposed LRR resource
limit or a change to E-R5-006.

Concrete current gaps:

1. `model::ScalarInt` has crate-writable `bits:u128, bytes:u8`; unlike the
   compiler's private `data` and `NonZero<u8> size`, the model permits zero,
   widths above16, or nonzero high padding bits. `Context::valtree` merely
   structurally interns a `ValTreeKind`; `mk_ct_from_kind` adds cached flags
   without establishing typed-value validity. Existing `to_u32` preserves an
   unconditional four-byte size check and checked numeric conversion, but
   `check_data` is only a debug assertion, as upstream. No caller may treat it
   as a release-mode general constructor validator. The source producer must
   establish widths1..16 and zero high padding bits with widths matching the
   actual scalar Type; representation must be canonical for value equality.
2. `ValueConst {ty,tree}` is crate-writable and does not prove that an aggregate
   Type has a Branch, or that its children have the right number/order/types.
   The helper correctly panics on Leaf-as-Branch, Branch-as-Leaf, a non-Value
   enum head, a zero-variant ADT or a non-ADT destructure request. These are
   compiler invariant failures, not source Type checking or guest refusals.
3. For enums, the producer must use the actual selected variant *ordinal*,
   encoded as a typed `u32` `ConstKind::Value` containing a four-byte Leaf,
   followed by exactly that variant's fields. This header is independent of
   the enum's repr and runtime discriminant value (including repr128). The
   helper does not check the head's Type and only decodes its bits/size;
   `AdtDefinition::variant` indexes the real variant slice and will panic on
   an out-of-range ordinal. No enum head may be guessed from runtime repr.
4. For structs and tuples, children must exactly match declaration/tuple order
   and count; for arrays, count must equal the actual length and all child
   Types must equal the element Type. Every child is a fully evaluated,
   correctly typed value, as pinned `branches`/`slice_branches` produce.
   `visit_const` zips ADT field definitions or tuple Types with child values;
   missing/extra children silently truncate the resulting obligations. That
   behavior is intentionally preserved from the pinned consumer, so the
   future producer must establish exact cardinality before admission.
5. Children, Type and ADT metadata must refer to one coherent context and
   actual current source/instantiated generic arguments. The interner's
   structural storage and borrowed handles do not independently prove such
   provenance. `PreparedAdtVariants::require_shape` checks schema/variant/
   field counts, names/order and agreement with flattened Types; it does not
   construct or validate a value tree, nor compare a typed field against its
   source AST Type. Those remain source-producer obligations. Required
   source/target/layout/constant dependencies must be observed before the
   immutable TypeIR value enters the solver.

Observed current production subset: the only production `ValueConst` literal
found under `traits/upstream` is `source_terms::Encoder::constant` for
`ConstTerm::Value(u64)`. It requires `expected`, requires the configured pointer
width and currently admits only eight bytes. Its only `Some(expected)` caller
is the array-Type branch supplying `usize`. Thus the inspected production
subset has valid width/padding; no current production aggregate construction or
malformed aggregate witness is claimed. Const generic arguments call it with
`None` and remain `ConstTypeMetadata` pending. `ConstTerm::Value(u64)` cannot
represent arbitrary signed/u128/floating/aggregate constants, and no production
`ValTreeKind::Branch` construction was found. These are explicit unimplemented
producer dependencies, not reasons to expand the helper into an evaluator or
mint readiness. Host tests construct scalars directly for specific inference
controls; those are not evidence of a complete compiler-compatible producer.

No source edits, feature/tests or executable experiments accompanied this
follow-up. The helper source fingerprints from the prior handoff are unchanged.
