use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    frontend::{
        call_contracts::{
            ArgumentType, ContractPreparation, PreparedCallContracts, prepare_contracts,
        },
        callables::{CallResolver, CallSelection},
        names::NameNamespaces,
    },
    ir::op::PrimitiveType as P,
    runtime::shape::TypeId,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
    traits::term::{ParamKey, TyTerm},
};
fn source(text: &str) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("src/main.rs").unwrap();
    let crate_key = Digest([151; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source missing")
    };
    (inputs, view)
}

fn id(view: &PreparedSourceView, name: &str) -> Digest {
    view.outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap()
        .def_key
        .fingerprint()
}
fn arg(p: P, id: u32) -> ArgumentType {
    ArgumentType {
        source: TyTerm::Primitive(p),
        descriptor: TypeId(id),
    }
}
fn ready(poll: QueryPoll<ContractPreparation>) -> PreparedCallContracts {
    match poll {
        QueryPoll::Ready(Ok(ContractPreparation::Ready(c))) => c,
        QueryPoll::Ready(Err(error)) => panic!("{error}"),
        _ => panic!("contract deferred"),
    }
}
#[test]
fn reached_generic_call_contracts_keep_rigid_views_and_separate_runtime_bindings() {
    let (inputs, view) = source("fn caller(){}fn chosen<T: Missing,U: Other>(x:T)->T{missing();}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "chosen")
    else {
        panic!("callee missing")
    };
    let actual = arg(P::I64, 7);
    let contracts = ready(prepare_contracts(
        &mut db,
        &callee,
        &graph,
        &sources,
        &[],
        None,
        std::slice::from_ref(&actual),
    ));
    let key = ParamKey {
        definition: callee.signature().definition,
        ordinal: 0,
    };
    assert_eq!(contracts.source_parameters, vec![TyTerm::Param(key)]);
    assert_eq!(contracts.source_result, Some(TyTerm::Param(key)));
    assert_eq!(contracts.runtime_substitution.get(&key), Some(&TypeId(7)));
    assert_eq!(
        contracts.source_substitution.get(&key),
        Some(&TyTerm::Primitive(P::I64))
    );
    assert_eq!(contracts.runtime_substitution.len(), 1);
    assert_eq!(contracts.result_descriptor, Some(TypeId(7)));
    contracts.check_return_descriptor(TypeId(7)).unwrap();
    assert_eq!(
        contracts
            .check_return_descriptor(TypeId(12))
            .unwrap_err()
            .code,
        "LRR-E-CALL-RETURN-TYPE"
    );
    assert!(db.trace.events.iter().all(|e| !matches!(
        e.key,
        lrr::db::key::QueryKey::BodyStructure(_)
            | lrr::db::key::QueryKey::Thunk(_)
            | lrr::db::key::QueryKey::Constant(_)
    )));
}
#[test]
fn explicit_result_and_argument_constraints_disagree_without_widening_or_defaults() {
    let (inputs, view) = source("fn caller(){}fn chosen<T: Missing>(x:T,y:T)->T{missing();}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "chosen")
    else {
        panic!("callee missing")
    };
    let i = arg(P::I64, 7);
    let u = arg(P::U64, 12);
    for (explicit, hint, args, code) in [
        (
            vec![(0, u.clone())],
            None,
            vec![i.clone(), i.clone()],
            "LRR-E-GENERIC-BINDING",
        ),
        (
            vec![],
            Some(u.clone()),
            vec![i.clone(), i.clone()],
            "LRR-E-GENERIC-BINDING",
        ),
        (
            vec![],
            None,
            vec![i.clone(), u.clone()],
            "LRR-E-GENERIC-BINDING",
        ),
        (vec![], None, vec![i.clone()], "LRR-E-CALL-ARITY"),
    ] {
        assert!(
            matches!(prepare_contracts(&mut db,&callee,&graph,&sources,&explicit,hint.as_ref(),&args),QueryPoll::Ready(Err(error))if error.code==code)
        );
    }
    let unresolved = ArgumentType {
        source: TyTerm::Infer(lrr::traits::term::InferVarKey {
            transaction: Digest([211; 32]),
            ordinal: 0,
        }),
        descriptor: TypeId(7),
    };
    assert!(
        matches!(prepare_contracts(&mut db,&callee,&graph,&sources,&[],None,&[unresolved,i.clone()]),QueryPoll::Ready(Err(error))if error.code=="LRR-E-TYPE-CONTEXT")
    );
    let bad = ArgumentType {
        source: TyTerm::Primitive(P::I64),
        descriptor: TypeId(12),
    };
    assert!(
        matches!(prepare_contracts(&mut db,&callee,&graph,&sources,&[],None,&[bad,i.clone()]),QueryPoll::Ready(Err(error))if error.code=="LRR-E-CALL-CONTEXT")
    );
    assert_eq!(
        ready(prepare_contracts(
            &mut db,
            &callee,
            &graph,
            &sources,
            &[],
            None,
            &[i.clone(), i]
        ))
        .parameter_descriptors,
        vec![TypeId(7), TypeId(7)]
    );
}
#[test]
fn concrete_alias_parameter_contracts_resolve_before_runtime_descriptor_checks() {
    let (inputs, view) =
        source("type Word=i64;fn caller(){}fn chosen(x:Word){}fn unknown(x:Missing){}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let caller = id(&view, "caller");
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, caller, "chosen")
    else {
        panic!("callee missing")
    };
    assert_eq!(
        ready(prepare_contracts(
            &mut db,
            &callee,
            &graph,
            &sources,
            &[],
            None,
            &[arg(P::I64, 7)]
        ))
        .parameter_descriptors,
        vec![TypeId(7)]
    );
    assert!(
        matches!(prepare_contracts(&mut db,&callee,&graph,&sources,&[],None,&[arg(P::U64,12)]),QueryPoll::Ready(Err(error))if error.code=="LRR-E-CALL-ARG-TYPE")
    );
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, caller, "unknown")
    else {
        panic!("callee missing")
    };
    assert!(matches!(
        prepare_contracts(
            &mut db,
            &callee,
            &graph,
            &sources,
            &[],
            None,
            &[arg(P::I64, 7)]
        ),
        QueryPoll::Ready(Ok(ContractPreparation::NeedName(_)))
    ));
}

fn hints_ready(
    poll: QueryPoll<lrr::frontend::call_contracts::ArgumentHintPreparation>,
) -> Vec<lrr::frontend::call_contracts::ArgumentHint> {
    match poll {
        QueryPoll::Ready(Ok(lrr::frontend::call_contracts::ArgumentHintPreparation::Ready(
            hints,
        ))) => hints,
        QueryPoll::Ready(Err(error)) => panic!("{error}"),
        _ => panic!("argument hint preparation deferred"),
    }
}
#[test]
fn argument_hints_prepare_concrete_contracts_without_binding_generic_parameters_or_bounds() {
    use lrr::frontend::call_contracts::prepare_argument_hints;
    let (inputs, view) = source(
        "fn caller(){}fn chosen<T:Missing,U:Other>(x:i64,y:T,z:bool){missing();}fn cold(x:Undefined){absent();}",
    );
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "chosen")
    else {
        panic!("callee absent");
    };
    let hints = hints_ready(prepare_argument_hints(&mut db, &callee, &graph, &sources));
    assert_eq!(
        hints.iter().map(|h| h.descriptor).collect::<Vec<_>>(),
        vec![Some(TypeId(7)), None, Some(TypeId(2))]
    );
    assert_eq!(
        hints[1].source,
        TyTerm::Param(ParamKey {
            definition: callee.signature().definition,
            ordinal: 0
        })
    );
    assert_eq!(hints[0].source, TyTerm::Primitive(P::I64));
    assert_eq!(hints[2].source, TyTerm::Primitive(P::Bool));
    assert!(db.trace.events.iter().all(|e| !matches!(
        e.key,
        lrr::db::key::QueryKey::BodyStructure(_)
            | lrr::db::key::QueryKey::Thunk(_)
            | lrr::db::key::QueryKey::Constant(_)
    )));
}
#[test]
fn argument_hints_resolve_aliases_and_source_names_before_primitive_fallback() {
    use lrr::frontend::call_contracts::{ArgumentHintPreparation, prepare_argument_hints};
    let (inputs, view) = source(
        "type i64=bool;type Word=i64;fn caller(){}fn chosen(x:Word){}fn unresolved(x:Missing){}",
    );
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "chosen")
    else {
        panic!("callee absent");
    };
    let hints = hints_ready(prepare_argument_hints(&mut db, &callee, &graph, &sources));
    assert_eq!(hints[0].source, TyTerm::Primitive(P::Bool));
    assert_eq!(hints[0].descriptor, Some(TypeId(2)));
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "unresolved")
    else {
        panic!("callee absent");
    };
    assert!(matches!(
        prepare_argument_hints(&mut db, &callee, &graph, &sources),
        QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedName(_)))
    ));
}
#[test]
fn argument_hints_reject_stale_callable_publications_before_resolving_templates() {
    use lrr::frontend::call_contracts::prepare_argument_hints;
    let (inputs, view) = source("fn caller(){}fn chosen(x:i64){}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "chosen")
    else {
        panic!("callee absent");
    };
    let (changed, _new_view) = source("fn caller(){}fn chosen(x:u64){}");
    changed.publish(&mut db).unwrap();
    assert!(
        matches!(prepare_argument_hints(&mut db,&callee,&graph,&sources),QueryPoll::Ready(Err(e)) if e.code=="LRR-E-CALL-CONTEXT")
    );
}

#[test]
fn return_only_binding_preserves_rigid_source_result_and_rejects_later_mismatch() {
    let (inputs, view) = source("fn caller(){}fn make<T:Missing>()->T{missing();}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "make")
    else {
        panic!("callee absent");
    };
    let mut contracts = ready(prepare_contracts(
        &mut db,
        &callee,
        &graph,
        &sources,
        &[],
        None,
        &[],
    ));
    let key = ParamKey {
        definition: callee.signature().definition,
        ordinal: 0,
    };
    assert_eq!(contracts.result_descriptor, None);
    contracts.bind_return_parameter(&arg(P::I64, 7)).unwrap();
    assert_eq!(contracts.source_result, Some(TyTerm::Param(key)));
    assert_eq!(contracts.result_descriptor, Some(TypeId(7)));
    assert_eq!(
        contracts
            .bind_return_parameter(&arg(P::U64, 7))
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );

    assert_eq!(
        contracts.source_substitution.get(&key),
        Some(&TyTerm::Primitive(P::I64))
    );
    assert_eq!(
        contracts
            .bind_return_parameter(&arg(P::U64, 12))
            .unwrap_err()
            .code,
        "LRR-E-CALL-RETURN-TYPE"
    );
    assert_eq!(contracts.result_descriptor, Some(TypeId(7)));
}

fn environment_fixture() -> (
    Box<lrr::frontend::callables::SourceCallable>,
    PreparedCallContracts,
    ParamKey,
    ParamKey,
    Database,
) {
    let (inputs, view) = source(
        "fn caller<T>(){}fn chosen<'a,T: Missing,U: Other>(x:T)->T where T: Another {missing();}",
    );
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, id(&view, "caller"), "chosen")
    else {
        panic!("callee missing")
    };
    let caller = ParamKey {
        definition: id(&view, "caller"),
        ordinal: 0,
    };
    let actual = ArgumentType {
        source: TyTerm::Param(caller),
        descriptor: TypeId(7),
    };
    let contracts = ready(prepare_contracts(
        &mut db,
        &callee,
        &graph,
        &sources,
        &[],
        None,
        &[actual],
    ));
    let parameter = ParamKey {
        definition: callee.signature().definition,
        ordinal: 1,
    };
    (callee, contracts, parameter, caller, db)
}
#[test]
fn frame_definition_environment_keeps_unforced_bounds_and_distinct_substitutions() {
    use lrr::vm::context::GenericEnv;
    let (callee, mut contracts, parameter, caller, db) = environment_fixture();
    let events = db.trace.events.len();
    let env = GenericEnv::capture(callee.signature(), &contracts).unwrap();
    assert_eq!(env.definition(), callee.signature());
    assert_eq!(env.signature_key(), callee.signature().fingerprint());
    assert_eq!(
        env.symbolic_binding(parameter),
        Some(&TyTerm::Param(caller))
    );
    assert_eq!(env.runtime_binding(parameter), Some(TypeId(7)));
    assert_eq!(
        env.definition().parameters[0].symbolic,
        Some(TyTerm::Param(parameter))
    );
    assert!(
        env.definition().generics[1]
            .bounds
            .iter()
            .any(|token| token.text == "Missing")
    );
    assert!(
        env.definition()
            .predicates
            .iter()
            .any(|token| token.text == "Another")
    );
    let unused = ParamKey {
        definition: parameter.definition,
        ordinal: 2,
    };
    assert!(env.symbolic_binding(unused).is_none());
    assert!(env.runtime_binding(unused).is_none());
    contracts.runtime_substitution.insert(parameter, TypeId(12));
    assert_eq!(env.runtime_binding(parameter), Some(TypeId(7)));
    assert_eq!(db.trace.events.len(), events);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        lrr::db::key::QueryKey::BodyStructure(_)
            | lrr::db::key::QueryKey::Thunk(_)
            | lrr::db::key::QueryKey::Constant(_)
    )));
}
#[test]
fn frame_definition_environment_rejects_foreign_missing_and_inconsistent_bindings() {
    use lrr::vm::context::GenericEnv;
    let (callee, mut contracts, parameter, caller, _) = environment_fixture();
    contracts.parameter_descriptors[0] = TypeId(0);
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    contracts.parameter_descriptors[0] = TypeId(7);
    let result = contracts.source_result.take();
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    contracts.source_result = result;
    contracts.definition = caller.definition;
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    contracts.definition = parameter.definition;
    contracts.source_substitution.remove(&parameter);
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    contracts
        .source_substitution
        .insert(parameter, TyTerm::Primitive(P::Bool));
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    contracts
        .source_substitution
        .insert(parameter, TyTerm::Primitive(P::I64));
    contracts.runtime_substitution.insert(caller, TypeId(7));
    contracts
        .source_substitution
        .insert(caller, TyTerm::Primitive(P::I64));
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    contracts.runtime_substitution.remove(&caller);
    contracts.source_substitution.remove(&caller);
    let lifetime = ParamKey {
        definition: parameter.definition,
        ordinal: 0,
    };
    contracts.runtime_substitution.insert(lifetime, TypeId(7));
    contracts
        .source_substitution
        .insert(lifetime, TyTerm::Primitive(P::I64));
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
}
#[test]
fn frame_definition_environment_checks_nested_inference_and_term_limits_before_copying() {
    use lrr::{traits::term::InferVarKey, vm::context::GenericEnv};
    let (callee, mut contracts, parameter, _, _) = environment_fixture();
    contracts.source_substitution.insert(
        parameter,
        TyTerm::Tuple(vec![TyTerm::Infer(InferVarKey {
            transaction: Digest([3; 32]),
            ordinal: 0,
        })]),
    );
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    use lrr::traits::term::{GenericArg, ProjectionKey, TraitRef};
    let infer = TyTerm::Infer(InferVarKey {
        transaction: Digest([3; 32]),
        ordinal: 0,
    });
    for position in 0..3 {
        let mut key = ProjectionKey {
            trait_ref: TraitRef {
                trait_key: Digest([9; 32]),
                self_ty: TyTerm::Primitive(P::I64),
                args: vec![],
            },
            item: Digest([10; 32]),
            item_args: vec![],
        };
        match position {
            0 => key.trait_ref.self_ty = infer.clone(),
            1 => key.trait_ref.args.push(GenericArg::Type(infer.clone())),
            2 => key.item_args.push(GenericArg::Type(infer.clone())),
            _ => unreachable!(),
        }
        contracts
            .source_substitution
            .insert(parameter, TyTerm::Projection(Box::new(key)));
        assert_eq!(
            GenericEnv::capture(callee.signature(), &contracts)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
    }
    let mut term = TyTerm::Primitive(P::I64);
    for _ in 0..300 {
        term = TyTerm::Slice(Box::new(term));
    }
    contracts.source_substitution.insert(parameter, term);
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
}

#[test]
fn frame_definition_environment_rejects_transaction_placeholders_in_all_namespaces() {
    use lrr::{
        traits::{
            subst::instantiate_binder,
            term::{ConstTerm, GenericArg, PlaceholderKey, ProjectionKey, RegionTerm, TraitRef},
        },
        vm::context::GenericEnv,
    };
    const ORIGINAL: &str = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/where-clauses/higher-ranked-fn-type.rs"
    );
    use ra_ap_syntax::{AstNode, ast};
    let parsed = ast::SourceFile::parse(ORIGINAL, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let predicate = parsed
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::WherePred::cast)
        .unwrap();
    let captured = lrr::syntax::symbolic_headers::function_predicate(&predicate).unwrap();
    let transaction = Digest::of_bytes(ORIGINAL.as_bytes());
    let TyTerm::Function(function) = instantiate_binder(&captured, transaction, 1).unwrap() else {
        panic!("original universal function missing");
    };
    let reference = function.value.parameters[0].clone();
    let TyTerm::Ref(RegionTerm::Placeholder(key), _, _) = &reference else {
        panic!("universal placeholder absent")
    };
    let key: PlaceholderKey = *key;
    let (callee, mut contracts, parameter, caller, _) = environment_fixture();
    let mut terms = vec![
        TyTerm::Placeholder(key),
        reference,
        TyTerm::Array(
            Box::new(TyTerm::Primitive(P::I64)),
            ConstTerm::Placeholder(key),
        ),
    ];
    for argument in [
        GenericArg::Type(TyTerm::Placeholder(key)),
        GenericArg::Region(RegionTerm::Placeholder(key)),
        GenericArg::Const(ConstTerm::Placeholder(key)),
    ] {
        terms.push(TyTerm::Nominal(Digest([12; 32]), vec![argument.clone()]));
        terms.push(TyTerm::Projection(Box::new(ProjectionKey {
            trait_ref: TraitRef {
                trait_key: Digest([13; 32]),
                self_ty: TyTerm::Param(caller),
                args: vec![argument.clone()],
            },
            item: Digest([14; 32]),
            item_args: vec![],
        })));
        terms.push(TyTerm::Projection(Box::new(ProjectionKey {
            trait_ref: TraitRef {
                trait_key: Digest([13; 32]),
                self_ty: TyTerm::Param(caller),
                args: vec![],
            },
            item: Digest([14; 32]),
            item_args: vec![argument],
        })));
    }
    for term in &terms {
        contracts
            .source_substitution
            .insert(parameter, term.clone());
        assert_eq!(
            GenericEnv::capture(callee.signature(), &contracts)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
    }
    contracts
        .source_substitution
        .insert(parameter, TyTerm::Param(caller));
    // The owning declaration and resolved parameter templates are retained
    // separately from substitutions. Poison each retained view independently.
    for term in terms {
        let mut declaration = callee.signature().clone();
        declaration.parameters[0].symbolic = Some(term.clone());
        assert_eq!(
            GenericEnv::capture(&declaration, &contracts)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        declaration = callee.signature().clone();
        declaration.result_symbolic = Some(term.clone());
        let original_result = contracts.source_result.clone();
        contracts.source_result = Some(term.clone());
        assert_eq!(
            GenericEnv::capture(&declaration, &contracts)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        contracts.source_result = original_result;
        let original_parameter = contracts.source_parameters[0].clone();
        contracts.source_parameters[0] = term;
        assert_eq!(
            GenericEnv::capture(callee.signature(), &contracts)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        contracts.source_parameters[0] = original_parameter;
    }
    assert_eq!(
        GenericEnv::capture(callee.signature(), &contracts)
            .unwrap()
            .symbolic_binding(parameter),
        Some(&TyTerm::Param(caller))
    );
}
