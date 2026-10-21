# Actual union nested argument failure: read-only connected repair analysis

V533 source freeze, no source/Cargo/Git edits or execution. The user priority is
unchanged real rustc program completion through one interpreter. No new isolated
API/control is proposed as acceptance.

The actual unchanged witness is tests/ui/union/union-generic-rpass.rs, owning
edition 2015, SHA256 f59d90d6dad771e639a8ff498e0475e10e126c1b81342c3a14c08263458356b8.
Maintained V462 evidence stops at 18 VM operations with generic constructor
context missing. V470/V475/V477 evidence records 265 matched dispatches/outcomes
and 16 source calls, then LRR-E-UNSUPPORTED: "explicit Type argument with an
inference hole requires inference". This is historical progress, not a new pass.
V477 source fingerprint was f629719ee81890875beab182230d358878aca2f31f48d392ab6290a651bd923b.
V533 compilation failed before any program ran; it supplies no refreshed union
runtime result. Durable evidence: 2026-10-19-v462-v470-frame-source-answers.json,
2026-10-20-v471-v478-upstream-discriminant.json and
2026-10-20-v533-interpreter-runtime.json under worktracks/evidence.

Exact source chain for the stopping operation:
- main's reached record is MaybeItem::<std::slice::Iter<_>> with field
  ManuallyDrop::new(i.next().unwrap()). Earlier generic union transmute, Vec
  creation, iter() and first next() already ran in the maintained runtime.
- BoundRecordConstructor preparation calls prepared_constructors::prepare_arguments
  -> explicit_type_arguments::select_path_arguments(Constructor) -> select_argument
  with the actual indexed source Type occurrence.
- select_argument obtains AnnotationShape and shared let_annotation_thunk::select.
  The latter retains WrittenType::Generic(Iter, [Infer]); the normal
  WrittenType::source_with recursively returns None at WrittenType::Infer.
- explicit_type_arguments.rs:463 turns that None into the exact maintained fault.
  A standalone `_` is already retained as None; a nested `_` cannot become that
  same whole-argument None without losing the written Iter relationship.
- Even deleting that fault would not complete execution. Engine
  prepare_installed_record_template requires complete explicit arguments or a
  closed result context, and Frame::prepare_installed_nominal_constructor again
  requires descriptors for all explicit arguments. NominalMember specialization
  and source field binding must receive actual solved arguments, not holes
  mislabeled caller Params or a fallback Type.

The common execution order is already appropriate for ordinary effects:
syntax/thunks.rs constructor_thunk emits ConstructStart, normal operand entry,
ConstructField, then ConstructFinish. The engine currently prepares the bound
constructor before ConstructStart; source fields have not executed at the fault.
Frame/GuestStore have a PendingNominal guard and cleanup/ownership path, but
GuestStore::pending_nominal_args only calls each field's structural bind against
observed runtime descriptors. It is not an associated projection solver and must
not be extended into one. Similarly existing binding_constraint_iteration's
hole/fill helpers and associated_equations/associated_types normalization are
legacy custom semantic debt, not permission to solve this witness separately.

Smallest truthful connected production repair has two inseparable parts:
1. Extend the common indexed Type-application transport to retain the complete
   written application and a distinct source-owned identity for every actual `_`
   occurrence, plus existing named-leaf/currentness guards. It must serve call,
   owner and constructor arguments through the existing select_argument route;
   do not parse display text or erase nested structure. These holes become real
   variables under the same actual pinned SolverContext, not fake caller Params
   or the legacy global hole() sentinel.
2. At the reached constructor boundary, submit actual selected field/result
   inference to the same admitted upstream relation/fulfillment route. The real
   field equality is structurally
   ManuallyDrop<<Iter<'r, ?T> as Iterator>::Item> = ManuallyDrop<&'s u8>, using
   actual source identities, field mapping and caller ParamEnv. The real facade
   Iterator impl declares Item = &'a T. Pinned normalization/relation can infer
   T=u8 and preserve the actual region constraints. No Iter/MaybeItem names enter
   production policy; this follows any genuine nominal/projection field equation.
   Reuse PreparedAdtSourceFields::encode/materialize and its actual EarlyBinder
   instantiation with original constructor Args. Reuse pinned RootVariables,
   relation, fulfillment and owned response/constraint machinery.

This cannot truthfully be a one-line hole-admission change. Current
SourceRootCallInput carries a callee signature and call relations; there is no
actual constructor packet/admission subject. A nominal field is not a synthetic
Fn/callee/WellFormed root. Current SourceVariableFamily only covers Type/Integer/
Float; source Iter<'_,T> also needs genuine omitted-region evidence, never
invented static. Shared SourceTerms require exact generic kinds/counts, so a
runtime-erased WrittenType argument list cannot supply source lifetime Args.
These are concrete required joins to the lead's actual root owner/query closure,
not a request for another standalone inference helper.

Prefer using already available pure source-signature/result facts to settle at
the current ConstructStart boundary where they suffice; never execute field
thunks for type answers or whole-body-check unrelated code. Where actual reached
field values are necessary, the existing transactional construction guard must
retain actual source schema/partial Args and observe the real operand before its
ConstructField ownership transfer. Settle with upstream and publish descriptor/
source view only after successful owned fulfillment; preserve NeedInput, failure,
cleanup, once-only operands and earlier fields. That continuation needs explicit
common constructor admission, not a special union executor. A guessed structural
T from the result or taking u8 from Vec spelling would be wrong.

Required next validation is the unchanged program's actual main, with fresh
runtime receipts: completion and correct assertions, no repeated next()/field
effects, source calls and cleanup captured. Retain actual repr128 and known
passing main regressions. A higher opcode count, admitted hole transport,
metadata control, source review or compiling this prerequisite does not close
the failure. No acceptance run was performed by this agent.

Read source SHA256:
- frontend/explicit_type_arguments.rs ce2f693ebef5ac269bc14c8dec56a678387f7bc9ee5a748ae969198573f72d18
- frontend/explicit_path_arguments.rs 1854f46dd9605d05700cb0d91d09f31632f1908d839734ae34db77947efae942
- frontend/prepared_constructors.rs 6c715b4fde87ca0c17260850eda860bd18a59e5a5f8538f87f9b05d376e304f3
- engine/run_constructors.rs a1c2c41570003fa89a7e7b7212342f398d078ead03611a11162fdd4aa80e47bd
- vm/frame.rs 6e1637d5a3217577c78217fa719136b3c5eb4e96fa4847963246b7d8ab3cb997
- runtime/cell.rs cbe6a83aae11eb550dc230cf482c7f792c9d110da9570ad6855f7fec8177cd65
- ir/written_type.rs 210746512fe15991a59ae0ed9d942bc111745ba0847f8e93680531fa0fd5c7bd
- traits/upstream/root_input.rs 369072f7ad0ce552c15ed70e5904ac047a3b9f270f8b9fe9b9020d9b486b118b
- traits/upstream/source_adt_fields.rs 2b54ec89a46eee653cfaacb000669d96dc51096a56c78b74b3d8d3835bc5b8a7

All-eight review direction for the connected change: reuse select_argument and
ordinary constructor opcodes (1-2); exact source equations, no macro/name-based
numeric or member rule (3); retain original/generated source owners alike (4);
no unselected bodies/pattern/typechecker pass (5); retain the one indexed hole/
application plan across phases and suspension (6); prepare only actual field/
projection callback domains, no per-dispatch world scan (7); share original
BodyIndex/source/query handles (8). Actual changed-source review and execution
would still be required after implementation release; this record is diagnosis.

## V535 frozen-source successor analysis (read only)

The fresh V534 actual original-main baseline confirms this same blocker, rather
than relying on the historical V477 receipt: edition 2015, unchanged SHA above,
exit 2, 265 matched VM dispatches/outcomes and 16 actual source-call boundaries.
The executor reports the exact nested-hole diagnostic above at source bytes
518..597. Evidence is `worktracks/evidence/2026-10-20-v534-interpreter-runtime.json`.
V535 is currently active; its new terminal has not been observed by this worker.
Production/tools/fixtures/HEAD remain frozen; no execution was performed here.

Current production still has the exact chain described above. No nested-hole
transport or constructor root has landed, confirmed with the pinned-root owner.
`SourceRootCallInput` and `PreparedRootRelations::prepare` authenticate an actual
callable definition/signature; the active reached-call executor cannot admit this
record by manufacturing a function or its signature. Its ordinary relation and
fulfillment implementation is reusable, but its current call admission is not a
constructor certificate.

The minimum **connected** code boundary after the fresh terminal is:

1. In `explicit_type_arguments::select_argument`, retain the actual composite
   annotation together with distinct source sites for its `_` leaves, existing
   named-leaf guards and caller definition. `SelectedPathArguments` and
   `PreparedRecordConstructor` must transport this open application separately
   from `ExplicitType`'s complete descriptor contract. Do not turn `Iter<_>` into
   `None`, a caller `Param`, or `WrittenType::Infer` that binds any descriptor.
   The source site can be the exact indexed owning PC plus original type range;
   the current `AnnotationShape::Infer`/`WrittenType::Infer` alone loses the
   individual occurrence identity. Preserve current complete argument behavior
   for calls, owners, constructors and aliases through this one selector.
2. In `engine/run_constructors.rs::prepare_installed_record_template`, retain
   that actual open application with `PreparedRecordConstructorHandle`, its
   actual variant and `BoundRecordFields::written_to_declaration` mapping.
   Constructor admission must establish a real ADT subject in the same pinned
   owner/RootVariables/caller ParamEnv route. Reuse the shared source bundle and
   `PreparedAdtSourceFields::encode`/actual `AdtField` EarlyBinder instantiation.
   Generalize the existing relation preparation below call-specific admission;
   do not create a parallel unifier/root evaluator. The source relation for a
   record operand is directed actual-to-declared-field coercion, retaining the
   actual source field span, rather than blindly equating all record inputs.
3. Where actual source signatures cannot settle before `ConstructStart`, the
   existing constructor continuation must retain the open source state through
   ordinary operand execution. `run.rs` currently has coordinator preflights
   for ConstructStart and ConstructFinish only; the missing connected hook is
   a coordinator preflight of the installed actual `ConstructField` immediately
   before `vm::step` transfers that operand. Snapshot the actual source/bound
   view, descriptor and ownership cell under the same constructor attempt; use
   ordinary upstream relation/normalization/fulfillment, then recheck before
   committing bindings and performing the existing ownership transfer. NeedInput
   must resume this pending attempt without repeating `next`, `unwrap` or the
   field expression. The existing construction guard owns transferred fields
   and cleanup. No guessed field value or execution of a cold field/body is a
   type answer. A later field may constrain earlier holes, so the retained
   unresolved field relations must also survive until constructor completion.
4. `Frame::prepare_installed_nominal_constructor` currently rejects every open
   explicit argument and materializes all descriptors before start. Its genuine
   successor must consume the settled owned constructor response; no guard is
   simply deleted. `GuestStore::pending_nominal_args` remains a runtime schema
   binder, not a projection engine. Do not add Item/Iterator/Iter/ManuallyDrop
   recognition or extend `binding_constraint_nominal::constructed_inward`'s
   legacy semantic candidate/fallback route to solve this program.

Two exact prerequisites are still missing. The extracted immutable
`SourceRelationPlan` explicitly marks Record/Call/Method/WrittenType as typed
boundaries; it does not yet supply an unsolved record-field/call-result equation
closure. `BindingConstraints`' existing `record_inward` and
`constructed_inward` cannot be treated as pinned inference proof. Separately,
`RootVariables` only transports Type/Integer/Float and `source_terms::region`
holds `RegionTerm::Local`; actual omitted source lifetime slots in `Iter` must
be represented as constructor-owned region inputs under the same inference
owner. The pinned owner already has `next_region_var`; no static/erased region
may be substituted to make source encoding pass. If the reached operand lacks
a genuine source view, report that exact metadata hold rather than reconstructing
source lifetimes from its erased runtime descriptor.

This is a coupled ordinary constructor inference/continuation change, not a
one-line nested-hole exception. It should be assigned with its actual existing
entry/field/finish consumer modifications together and validated by the entire
unchanged original main plus existing real regressions. No new isolated helper
evaluation is an acceptance substitute. All eight architecture directions above
still apply; no current production change or new pass is claimed.

Inspected current SHA256 (V535 frozen candidate):
```
90543d1ec7bda5bc72f2eb8e32cf536547a1e08c4ee9c1eac08857d41c5e7004  frontend/explicit_type_arguments.rs
6c715b4fde87ca0c17260850eda860bd18a59e5a5f8538f87f9b05d376e304f3  frontend/prepared_constructors.rs
ccc8d27fcff62e67d94f1135f234ef2448dd80275ed35d05a6641048385b9ff1  frontend/binding_constraint_nominal.rs
a1c2c41570003fa89a7e7b7212342f398d078ead03611a11162fdd4aa80e47bd  engine/run_constructors.rs
6e1637d5a3217577c78217fa719136b3c5eb4e96fa4847963246b7d8ab3cb997  vm/frame.rs
fd32e07167d8978724a9acd80a873a7d3a8c21693b601f053d341e76455095f3  traits/upstream/root_input.rs
a167db0c8efe1f69b159fdadc8655ea8441a12602b4e9af310cf7a2c9344fd94  traits/upstream/root_relations.rs
2b54ec89a46eee653cfaacb000669d96dc51096a56c78b74b3d8d3835bc5b8a7  traits/upstream/source_adt_fields.rs
```
Paths in this list are relative to `crates/lrr/src/`. No Cargo/Git or production
edit was made; only this bounded existing record was appended.
