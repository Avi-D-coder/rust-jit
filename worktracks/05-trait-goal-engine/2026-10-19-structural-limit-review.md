# Structural resource guard removal: separate source review

Baseline c3e9ed88a9aee95cd5200e0b8fd2a95f3268fc9a, final candidate V368
source fingerprint 0d2e0bc950dcce3500d04a936d1652bee9fe36c139db2928e74a7980641fb100.
Review scope is E-R5-006's connected legacy signature/substitution restriction;
this is not full M020 or E-R5-014 migration acceptance. See the bounded
[architecture record](2026-10-19-structural-limit-removal.md).

Source inspected: frontend/signature_subst.rs::SignatureSubstitution::new and
constrain_argument/result/equation, traits/unify.rs::{validate,resolve_head,
occurs,visible_substitution,admit_variables,Unifier::unify,fresh_universe},
traits/subst.rs::{Substitution,TransformContext,TransformTask,type_children,
transform,transformed_arguments,shift,
shifted,closed,bound,Instantiation::bound,instantiate_binder,
validate_binder_type}, traits/term.rs::{validate_opaque_shape,measure}.

MAX_DEPTH256 is deleted, not raised or recast as a compiler recursion limit.
The100000 projection work guard and the related signature/shape/measurement
admission passes are removed. Budget node/byte tracking and traversal-only
depth/steps disappear. Actual lift/cutoff/Bound.depth, typed namespace checks,
transaction/occurs/placeholder/universe checks and staged publication remain.
Checked u32 universe/binder-index overflow is a representational error, with
accurate generic-binding diagnostics, not a resource budget. Root universe
instantiation still fails for its semantic reason. No matching arm is added.

1. No assertion expression compiler/evaluator: original SyntaxProvider memo
   supplies the real id signature. Runtime source/expression dispatch unchanged.
2. No literal/break-only macro route: no macro expansion/index/verifier path
   changes. All source signatures already join before SignatureSubstitution.
3. No context-specific numeric/member selection: matching arms and namespace
   identities unchanged; only admission/accounting restrictions removed. The
   legacy unifier remains migration debt, not an upstream algorithm substitute.
4. No generated-source exclusion: no origin branch or source authority bypass.
   Signature/profile/declaration currentness is owned by the existing query.
5. No pattern duplication or eager cold arm checking: no pattern/body planning
   changes. Typed term traversal does not discover or execute guest bodies.
6. No repeated source parsing/planning during execution: no new callbacks,
   source walks or per-phase preparation. The shared transform now uses heap Visit/Assemble tasks and ordered
   child results; closed validation uses Visit/LeaveBinder with its typed scope
   stack. Old recursive transform_function/tuple/projection/opaque/array/slice/
   reference/nominal/arguments helpers are deleted. No special deep-reference
   route. Redundant measure passes are removed. Existing owned-term derived
   clone/equality/drop remain recursive; no unlimited OS-stack or performance
   claim. Work/temporary vectors grow with the metadata visited, without caps.
7. No full-world scan/unrelated invalidation: no namespace/cache dependency
   changes. Local transaction dictionaries retain their current ownership.
8. No immutable BodyIndex copies/whole-file node search: only owned symbolic
   type metadata is transformed; frame/index/source location logic unchanged.

V366 built, then the actual id<T> signature control failed at argument
constraint with LRR-E-METADATA-LIMIT/signature unification budget exceeded.
The control is byte-identical in V367. An initial wrong selector omitted the
crate_graph module; run_controls rejected it before executing any test. It was
corrected against the actual binary list. V366's real failure is retained.

Runtime and host terminals are recorded separately in the evidence record.
The new host depth300 boundary is supplemental, not an upstream main pass.
Imported deep-nesting runtime source is unchanged and licensed at the pinned
commit. It also exercises conditional traits/layout and may expose distinct
unsupported contracts. No assertion of a new stock gain without a measured
before/after stock comparison. Full origin/epoch/suspension/cleanup matrices,
upstream root/source fact/member/dictionary integration and obsolete route
removal remain unexecuted or incomplete. Existing public metadata measurement
APIs elsewhere remain broader accounting debt, outside this bounded deletion.

V367 builds passed; unchanged depth300 control then aborted with host stack
 overflow (exit-6), two existing generic source controls passed, and the
universal-region negative control failed (unwrap_err on successful admission).
No production frame source changes were made. V368 replaces the common
recursive substitution/closed traversal; it preserves ordering of projection
Self then trait/item type args, function parameters then result, mixed Type/
Region/Const namespaces and semantic lift/cutoff transitions. Each Assemble
consumes exactly its scheduled child results; leaf cloning contains no nested
Type children. This is traversal transport, not an independent upstream trait
solver. Full transaction/binder/opaque matrix remains unexecuted, not inferred
from the depth300 result. The existing source-selected runtime path still uses
the legacy Unifier, and full removal awaits actual upstream migration.

V367 raw runtime collector compared JSON byte arrays to text and incorrectly
recorded false assessments. Preserve those raw records; stock-assessment.json
explicitly compares expected UTF-8 byte arrays and terminal/capture/exit/guard
facts: all12rows/3distinct unchanged cases pass their selected runtime contracts
at V367, with observed outputs and cleanup. This is an assessment correction,
not a production change or an excuse to rerun a completed program. Final V368
production changes require their own runtime evidence. No compiler-native
execution counts, complete inventory total or measured new stock gain follows.

V369 final source fingerprint
895a972ae28cdfbe1d54d0e679489c9efa59a625179af4bc003b414518c58321.
Only the existing #[cfg(test)] vm/frame.rs universal-region control changes
from V368. Separate inspected production route: Frame::supported_source calls
frame_source_types.rs::structural_source_shape first; its Ref arm ignores region
as non-data. The prior negative test was stale under E006. Revised control
retains the actual source FnType placeholder through slot, parameter and binding
views, checks unchanged operation/handle/scope state and keeps the once-only
parameter-table guard. No production source/VM rules change; modes1-8 above
remain unchanged, not re-proven by matching final output. V369 both builds and
1/1affected control pass with source/HEAD/test/CLI guards. V368 three generic
controls and12runtime rows remain at their actual fingerprint; not an all4
final-source control run. V367/V368 failures remain in evidence. No full
regions fixture/borrow checking or solver-root acceptance is implied.
