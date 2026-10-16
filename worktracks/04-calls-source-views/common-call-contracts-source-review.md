# Common call contract source review — LOCAL-024

Independent source-only review; no production edits, Cargo, compiler, source tests or guest execution. Root owns normal contracts/engine. This record distinguishes the initial blocker from the live successor reviewed while root implemented it. Source fingerprints below bind conclusions; later edits need rechecking.

## Prioritized findings

1. **Original blocker, addressed in inspected successor:** all nine official suffix functions have one named primitive parameter/result, no generics/predicates/receivers/references. FunctionSignature deliberately leaves these named Path types symbolic=None. Old ordinary-local admission has lexical_contract=None and therefore could not use mint_lexical/primitive_result; old fallback resolve_named/mint_primitive uses separate module/generated namespace transport and global source census. Current root prepare now selects PreparedReturnPlan46→DeclaredReturnGoal45→actual declaration_return_types/HeaderTypeSite→same common Type finalizer before either legacy branch for OrdinaryLocalAdmission. mint_declared checks exact wrapped FunctionSignature identity and private Ready source proof; DeclaredGoal origin keeps real shared guard, packet/source consistency and currentness without global publication epoch. All nine primitive results fit this normal existing VM deferred-primitive certificate. No numeric signature/profile extension needed.

2. **Preflight is already common; preserve it:** preflight_evaluated_call checks reached arity, bounded term metadata, unresolved transaction terms, descriptor presence and source primitive/descriptor consistency only. It has no LexicalAdmission or primitive_parameters dependency. It does not resolve declared parameter Types and must not be weakened to permit the new admission.

3. **Parameter constraints already join source service:** binding_constraints obtains actual callee Value39 declaration and DeclaredParameterGoal38 at argument ordinal. call_contracts::argument_hints uses the same source_parameter_goals service, and its local path calls plan.prepare_parameter_type with actual HeaderItem/HeaderField/generic/header-source authority. prepare overwrites instantiated parameter templates with these Ready source facts before SignatureSubstitution. The nine j bindings and nine direct numeric arguments should therefore use the same source-selected primitive Types; no use of legacy primitive_parameters is needed outside the held Lexical return-certificate branch. This is source reasoning, not a pass prediction.

4. **Repeated return guard capture, addressed in inspected successor:** an intermediate root branch captured prepare_input_guard(Q45) inside every call contract. Current PreparedReturnPlan46 memo captures Q45+guard once; contracts now clones plan.guard. Actual query-run/guard-capture reuse across all eighteen calls still requires validator evidence. Existing mutable argument substitution/descriptor checking remains per actual reached call, independent of source preparation.

5. **Remaining currentness ordering control:** prepare_source_call takes retained selected.callee and, for local owner, calls cheap preflight then selected_source_body/prepare_reached_local_source before call_contracts::prepare calls callee.revalidate. A stale Value guard after arguments/suspension should refuse before indexing the formerly selected body. Add/review normal callee.revalidate before that first body demand and an actual invalidation control; the local body metadata receipt intentionally does not redo name proof. No execution unsoundness or suffix terminal failure is asserted here.

6. **General return proof gap, not suffix blocker:** result_symbolic=Some generic/structural syntax bypasses the new Q45 header occurrence join; signature lower_type alone identifies generic spelling but is not the common header hygiene proof. Named generic return contexts need that genuine occurrence proof before a broad generated-generic acceptance claim. Nonprimitive deferred Ready results currently remain NeedType because existing VM normalized deferred-return envelope admits primitives only; nested structural/module explicit Types and local alias authority remain explicitly held. Do not extend primitive/numeric profile or synthesize empty generic schemas to hide these gaps.

## Concrete shared contracts and limits

Normal path: actual EnteredCall44/ordinary SourceCallable → next-written argument source_parameter_goals38 → evaluated-input preflight → reached source/body coordinator → prepared return46/declared return45 → SignatureSubstitution → PreparedCallContracts → GenericEnv::capture_current → normal VM call/return. GenericEnv validates exact declaration/arity, source terms/substitutions/descriptors and private normalized certificate, then captures current proofs. Existing return transfer compares actual descriptor and retained source identity. Same whole unchanged suffix has eighteen calls, two per declaration; main has source implicit Unit. None of this justifies lowering cold callee bodies to answer Type goals.

Publisher peer review is separately exact-source recorded at worktracks/evidence/2026-10-16-ordinary-local-publication-currentness-independent-review.json (SHA30ee130332e2dc9275aa4a39ec1fb61c6defa3160c4ac502a11394fba2e157c6). No concrete authority blocker found in dropping only global revision equality while retaining real Q39 guard and actual fixed rows; two cached distinct cold-body proofs/17 mutations/overflow controls are unexecuted.

QueryValue Clone source investigation found MemoEntry/Outcome retain Arc<QueryValue>; production memo clone sites clone that Arc, Provided has no Clone derive, and snapshot .as_ref().clone hits are InputValue tests. Root removed inner QueryValue Clone for candidate. An unpublished custom metadata-clone attempt was withdrawn on root STOP; live entered-call goal939907ec and ordinary admissione0dcacfd restored exactly, receipt /private/tmp/lrr-unpublished-cold-clone-attempt-held/receipt.json. No claim that compilation confirmed trait use.

## Eight-mode pass

1–3: inspected source services/certificates and normal contracts introduce no assertion evaluator, scalar lane or macro/name/numeric profile; legacy mint_lexical/mint_primitive/generated_result_namespace remain named held routes. 4: same admitted source/header pipeline; generated-generic and missing structural/module authority remain gaps. 5: next-written argument hints stay lazy; declaration goals require no child body or unselected executable shape; body-currentness ordering control remains. 6: memo38/44/45/46 immutable source plans, guard capture once in prepared46; no installed source parsing; actual reuse evidence pending. 7: DeclaredGoal uses relevant private guard; held legacy global census/revision checks not claimed removed globally. 8: retained actual BodyIndex/plan arcs and direct header AST location; explicit callable owner root, no immutable body clone/new registry in changed route. Existing bounded signature metadata copies are normal contract/environment ownership cost; no speed/scaling claim.

## Exact inspected source hashes

- crates/lrr/src/frontend/call_contracts.rs: 4d9243a43d4e070ac16cb5b263f4b1d6d998b402f5cc50c0c7b304de37fb9e26
- crates/lrr/src/frontend/source_parameter_goals.rs: e3d6526aaef9538020392281a9ccd47575168a00a66883e4d8670e8f2326c99e
- crates/lrr/src/frontend/declaration_field_types.rs: ba037c7a7d497d81f5de04fb34f83cb7906bd1c38abebe798e470526aa4439c6
- crates/lrr/src/frontend/declaration_return_types.rs: 230b2e4ed1337ead47c0efc70382104d14ca50ebe4838b9896363370c5e69aeb
- crates/lrr/src/frontend/source_return_goals.rs: 88374fc4607c17d791a43b1aea89a76e6ed5fed0cb1dd450d1da9343524fa946
- crates/lrr/src/frontend/binding_constraints.rs: 8dbeb584297b8c374a2af1b78ac626b2ea786ebc436c504641a437362e246e9a
- crates/lrr/src/engine/run.rs: 475cb20a03a4893da192c03e2d5bac757d8c22c58691158a9100f87906a4e560
- crates/lrr/src/vm/context.rs: 5f284b2d833940da0b7d2f42d34188451419c6489d034538fc773cdbbbb272c7
- crates/lrr/src/db/value.rs: 9e5cfcb6d1263d9fe61a2d3ada741e4812f46785750966dd94a814fb649a9620
