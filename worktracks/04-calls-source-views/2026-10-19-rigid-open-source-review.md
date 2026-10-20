# M018/T040/T053 frame-owned constraint answers — separate source review

Baseline8c3449eca662b6565727aa93ca0259e3abe566b1; solo LOCAL049 under the
existing LOCAL037/046/047 source reservations. MA002/MA003/MA006 are distinct
historical findings, not the recurring failure-mode numbers below. This is a
held component checkpoint; no complete feature/card/solver/milestone claim.

V466 fingerprint2f367454e9de6cc95b3dab62fea7d5788cf4234e197b928bed00481e5523933d:
inspected Component::{admitted_open_answer,static_candidate,constrain_term,
constrain_declared,decided}, closed_answer, prepare_expression;
explicit_type_arguments::prepare_application; written_terms::{written_template,
written_type}; WrittenType::{in_frame,source_with}; existing Component candidate/
parameter relations and GenericEnv/WrittenType frame materialization. A concrete
or actual same-body rigid term uses the existing source-application finalizer.
Its Param owner must match the retained SourceLocalPlan and complete written
round-trip; foreign/erased/inference fragments are not fabricated as ordinals.
The body's non-call declared result Param now reaches existing constrain_term
and project_term. Inferred argument/result trees retain the real body owner at
the root via WrittenType::Frame; immutable Arc trees remain shared.

V467 fingerprint72a1d68bf5f059eac33a7377a3859d04a2acc4b08bf203a345e52257cf19eeec:
inspected call_contracts::{prepare,closed_term}, PreparedReturnPlan observation/
currentness checks and both existing result-substitution blocks;
GenericEnv::{checked_bytes,bound_result_descriptor_find}; written_terms nominal
materialization; GuestStore::descriptor_source_term. Non-placeholder nominal
regions now follow Ref's existing metadata policy. This permits the actual
prepared Iter return to retain its substituted Type rather than discard it.
Runtime record/materializer omits nominal region slots by existing design;
V467 exposes their mismatched runtime comparison rather than a source proof.

All eight recurring failure modes, against these exact fingerprints:
1. No assertion evaluator or synthetic macro operand block is introduced. The
   same source constraints/written hints serve all admitted indexed expressions.
2. No literal/break/macro-output executor or opaque macro opcode is added.
3. No method/field/macro/fixture-name rule or new unification/trait-solving
   algorithm is introduced. Ordinary source relations preserve actual ParamKeys;
   inherited custom SignatureSubstitution/Component solving remains E014 debt.
4. Generated/handwritten syntax uses the same retained SourceLocalPlan/body owner
   and query guards; no original-only acceptance or source/epoch bypass is added.
5. No pattern engine or eager arm/body checking is added. Projected declared
   rigid Types use existing relations; source signatures do not enter cold bodies.
6. Preparation stays in the existing memoized source/binding queries. New owner
   admission fetches a retained SourceLocalPlan, and compound finalization can
   walk selected Type/schema/source handles; no parser or installed-op planning.
   Call substitution/closedness/return checks remain Type-tree walks at call
   boundaries. This is not a constant-time or measured performance claim.
7. No full-world namespace map/comparison or broad invalidation is added. Actual
   body/schema/source dependency guards remain attached to the retained plan.
8. No BodyIndex/parser deep clone or whole-file syntax search is added. Existing
   cold term/tree conversions remain; emitted written trees use retained Arc.

No new caps/accounting, sandbox, borrow/static/privacy checking, unsafe or
frontend/query/backend change. Upstream solver root/member/VM migration and
remaining legacy routes are still held; this does not implement a new solver.

Evidence: V463 four existing controls and2maintained originals pass, primary
original still18failed; V464/V465 are diagnostic CLI probes only, hooks removed
before V466. V466 both builds/four controls0, primary18->185failed,2maintained
originals pass, nesting fails22; V467 both builds/five controls0, primary185->207
failed,2maintained originals pass. All final input/binary guards0. Intermediate
execution is not a full original pass; no refreshed suite total or JIT admission.

V468 source6b5b163048748c312482718f66914c6e23c005d143d4cc64b89380c083a7cf0b:
inspected signature_subst::erase_regions and its existing call argument, frame
structural-source and Run::check_call_return consumers. Nominal Region slots
are omitted only in this existing runtime comparison view, matching the actual
GuestStore::descriptor_source_term and WrittenType generic descriptor shape.
Reference regions still normalize to Static, Types recurse, Const args retain
identity. No source term is overwritten, no source/binder/trait proof is minted,
and source goals still retain Region terms. The ordinary call-return guard
remains unchanged and actual Type/Param owners remain compared. This extends
an existing physical-family projection, not a new unifier/matcher or source
normalization route. All8mode conclusions above still apply; source/schema/
closedness/owner guards remain. Additional cold/hot cost is the same existing
Type-tree projection per relevant call-boundary comparison. No speed claim.
V468 acceptance pending asynchronous terminal collection at this record.

V469 sourcef89677519d608a40ebb52a2e1e7ee856e063918eafa31b14c8c4cb54b2f4d8ff:
Frame::{supported_structural_source,structural_source_shape,
require_structural_source_target,structural_source_target} remain the common
membership route used by normal call source publication, tuple projections,
pattern source checks and frame attachment. Region metadata is admitted in
nominal source trees without changing them. Descriptor checks filter only
Region slots; exact nominal identity, Type/Const slot order/arity, actual
recursive Type descriptors, reference mutability and source/runtime binding
checks remain. No new source authority or lookup is minted from a TypeId.
The LRR-only actual Iter-header corruption control covers4editions/4region forms,
wrong identity, changed/missing Type args, foreign Param, unsupported Const and
placeholder rejection. It is not an upstream execution or full source/hygiene
receipt. Remaining main source/runtime/origin acceptance is still required.
Eight-mode review: no new parser/evaluator/macro or method-name case (1-4), no
pattern/branch preparation change (5); existing retained source terms and
runtime descriptors are traversed only by the existing membership check (6).
Filter count plus existing zip adds at most one pass over nominal argument
slots, no allocation/map/namespace scan (7), no body clone or source search (8).
Only formatting of the new inferred-argument packet differs fromV468 in the
open constraint file. No new caps/static checks or solving algorithm. All
upstream solver/legacy-route and complete architecture holds remain.

V470 source1b25c372e2f1a2885b6368d4a22f5ea1de0d4bab385208393a6becaf89a39ead:
separate E006 removal review covers Frame::{metadata_bytes,start_call,
install_parameter_sources,set_source_view,preflight_source_view,
prepare_source_field,prepare_projection,preflight_return,cleanup_scope,leave},
frame_source_storage::{clone_source,preflight_call_source_views,
preflight_binding_source_view,set_binding_source_view,preflight_borrow_source_view,
clear_parameter_source_views,set_return_source_view}, frame_tuple_sources::
{prepare_tuple_source_operands,preflight_constructor_tuple_source} and
scope_cleanup_frame::clear_cleanup_slot. All source_extra_bytes/
retained_source_heap/source_heap/source_excess/source_storage_limit/
preflight_source_replacement references are gone from production and controls.
There is no new Region case in a byte walker; the walker is removed.

Source slot validity and supported_source stay in preflight_source_view; call
publication still validates every temporary slot and rejects duplicate slots
before ownership changes. Binding index validation remains before assignment.
Tuple source preflight still validates actual moved operands and each source
against its real completed descriptor; destination index is checked directly.
Provenance/cleanup/return source functions retain the same owned TyTerm and
exact clearing; only integer accounting is removed. Source-return proof and
actual return descriptor checks run as before. Underlying fallible allocation
and checked encodings/indices remain; no resource cap is raised or introduced.

Eight-mode review extension: no alternate evaluator/parser/macro route (1/2),
no context-specific numeric/method/type rule (3), no original/generated origin
branch (4), no pattern/cold arm preparation (5). Source cloning no longer
traverses the tree to measure it or recomputes global frame metadata first;
only the actual clone remains, and actual preflight membership retains its
existing Type-shape checks (6). No namespace maps/epoch broadening (7), no new
immutable body copies/search (8). Other inherited projection/frame metadata
accounting, custom structural inference and full source/solver/VM/architecture
matrices remain explicit debt. This is not full E006 or interpreter closure.
The updated old accounting controls now check actual view retention/transfer/
clearing and run-long returned source ownership rather than obsolete counters.
Final source/runtime acceptance is pending terminal collection here.

V470 final both builds/9controls0, all process terminals collected and closing
source/HEAD/test/CLI guard0. Primary reaches265dispatches/16calls and still fails
at nested explicit Type inference std::slice::Iter<_>;3maintained originals pass.
No source changes after validation. Full origin/runtime/solver/architecture/
interpreter/JIT gates remain held; see worktracks/evidence/2026-10-19-v462-v470-frame-source-answers.json.
