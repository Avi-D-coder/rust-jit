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
