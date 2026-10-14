use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{DependencyStamp, InputKey},
        key::QueryKey,
        memo::{Database, Outcome},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::{
        op::Op,
        pc::{NodeId, Phase},
        recipe::RecipeExit,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        outline::BodyHandle,
    },
};
fn prepare(
    previous: Option<&PreparedSourceInputs>,
    source: &str,
) -> (PreparedSourceInputs, Vec<(String, BodyHandle)>) {
    prepare_edition(previous, source, 2024)
}
fn prepare_edition(
    previous: Option<&PreparedSourceInputs>,
    source: &str,
    edition: u16,
) -> (PreparedSourceInputs, Vec<(String, BodyHandle)>) {
    let input = SourceUnitInput {
        path: SourcePath::new("src/main.rs").unwrap(),
        crate_key: Digest([81; 32]),
        module_path: vec![],
        profile: ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    let selection = SourceSelection {
        path: input.path.clone(),
        outline: lrr::db::key::OutlineKey {
            parse: lrr::syntax::parse::ParseKey {
                content: input.blob.content(),
                profile: input.profile.clone(),
            },
            crate_key: input.crate_key,
            module_path: vec![],
            cfg: input.cfg.fingerprint(),
        },
    };
    let prepared =
        PreparedSourceInputs::refresh(previous, vec![input], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("source publication missing")
    };
    let bodies = view
        .outline()
        .items
        .iter()
        .filter_map(|i| i.body.clone().map(|b| (i.def_key.declared_name.clone(), b)))
        .collect();
    (prepared, bodies)
}
#[test]
fn stock_unrelated_body_import_preserves_other_callee_and_same_name_demand() {
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/resolve/blind-item-mixed-use-item.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (prepared, bodies) = prepare_edition(None, source, edition);
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        let body = &bodies.iter().find(|(name, _)| name == "main").unwrap().1;
        let structure = fetch(&mut db, QueryKey::BodyStructure(body.key()));
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!("missing original main structure")
        };
        let calls: Vec<_> = index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
            .collect();
        assert_eq!(calls.len(), 2);
        let other = fetch(
            &mut db,
            QueryKey::Thunk(index.pc(calls[0].id, Phase::Entry).unwrap()),
        );
        let QueryValue::Thunk(other) = other.value.as_ref() else {
            panic!("missing original f call thunk")
        };
        assert!(
            other
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::CallPrepare { name, .. } if name == "f")),
            "unrelated local g import must not hide module f: {:?}",
            other.recipe()
        );
        let same = fetch(
            &mut db,
            QueryKey::Thunk(index.pc(calls[1].id, Phase::Entry).unwrap()),
        );
        let QueryValue::Thunk(same) = same.value.as_ref() else {
            panic!("missing original g call thunk")
        };
        assert!(
            same.recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Unsupported(_))),
            "body-local g must retain lexical namespace demand"
        );
    }
}
#[test]
fn stock_body_import_adaptations_keep_alias_group_self_and_glob_frontiers() {
    let original = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/resolve/blind-item-mixed-use-item.rs"
    );
    // Disclosed LRR adaptations of the original import only. The imported
    // fixture remains unchanged; no adapted program is a stock fixture pass.
    for (import, blocked) in [
        ("use m::{g};", false),
        ("use m::{g as f};", true),
        ("use m::{g as r#f};", true),
        ("use m::{self as f, g};", true),
        ("use m::g as _;", false),
        ("use m::*;", true),
    ] {
        let source = original.replace("use m::g;", import);
        let (prepared, bodies) = prepare(None, &source);
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        let body = &bodies.iter().find(|(name, _)| name == "main").unwrap().1;
        let structure = fetch(&mut db, QueryKey::BodyStructure(body.key()));
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!("missing adapted main structure")
        };
        let call = index
            .nodes()
            .iter()
            .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
            .unwrap();
        let thunk = fetch(
            &mut db,
            QueryKey::Thunk(index.pc(call.id, Phase::Entry).unwrap()),
        );
        let QueryValue::Thunk(thunk) = thunk.value.as_ref() else {
            panic!("missing adapted f call thunk")
        };
        assert_eq!(
            thunk
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Unsupported(_))),
            blocked,
            "import {import}"
        );
        if !blocked {
            assert!(
                thunk
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::CallPrepare { name, .. } if name == "f"))
            );
        }
    }
}
fn fetch(db: &mut Database, key: QueryKey) -> Outcome {
    match db.fetch(&SyntaxProvider, key) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("{error}"),
        QueryPoll::NeedInput(request) => panic!("unexpected input demand {request:?}"),
    }
}
#[test]
fn body_and_literal_queries_record_only_entered_header_without_successor_fetch() {
    let (prepared, bodies) = prepare(
        None,
        "fn live(){0x2a_i64; missing();} fn cold<T: Unknown>() {absent();}",
    );
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    let live = &bodies[0].1;
    let cold = &bodies[1].1;
    let structure = fetch(&mut db, QueryKey::BodyStructure(live.key()));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("missing actual body index")
    };
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
        .unwrap();
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let thunk = fetch(&mut db, QueryKey::Thunk(pc.clone()));
    let QueryValue::Thunk(recipe) = thunk.value.as_ref() else {
        panic!("missing actual thunk")
    };
    let [
        Op::Literal {
            literal,
            destination,
            ..
        },
    ] = &*recipe.recipe().operations
    else {
        panic!("source was not lowered to literal opcode")
    };
    assert_eq!(literal.spelling, "0x2a_i64");
    assert_eq!(*destination, node.value.unwrap());
    let RecipeExit::Next(next) = &recipe.recipe().exit else {
        panic!("missing symbolic continuation")
    };
    assert!(db.memo(&QueryKey::Thunk(next.clone())).is_none());
    assert!(
        db.memo(&QueryKey::Thunk(index.pc(NodeId(0), Phase::Entry).unwrap()))
            .is_none()
    );
    let dependencies = &db
        .memo(&QueryKey::BodyStructure(live.key()))
        .unwrap()
        .dependencies;
    assert!(dependencies.iter().any(|d|matches!(d,DependencyStamp::Query{key:QueryKey::Signature(def),..} if *def==live.def_key)));
    assert!(dependencies.iter().any(|d|matches!(d,DependencyStamp::Input{key:InputKey::DefinitionBody(def),..} if *def==live.def_key)));
    assert!(dependencies.iter().any(|d|matches!(d,DependencyStamp::Input{key:InputKey::DefinitionSource(def),..} if *def==live.def_key)));
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::Signature(def) => *def != cold.def_key,
        QueryKey::BodyStructure(body) => body.definition != cold.def_key,
        QueryKey::Thunk(pc) => pc.body.definition != cold.def_key,
        _ => true,
    }));
    assert_eq!(
        fetch(&mut db, QueryKey::Thunk(pc)).fingerprint,
        thunk.fingerprint
    );
}
#[test]
fn entered_call_syntax_is_a_verified_recipe_without_eager_name_resolution() {
    let (prepared, bodies) = prepare(
        None,
        "fn live(){missing::<Absent>();} fn cold(){invalid_call();}",
    );
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let structure = fetch(&mut db, QueryKey::BodyStructure(bodies[0].1.key()));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("body structure missing")
    };
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Call))
        .unwrap();
    let thunk = fetch(
        &mut db,
        QueryKey::Thunk(index.pc(node.id, Phase::Entry).unwrap()),
    );
    let QueryValue::Thunk(recipe) = thunk.value.as_ref() else {
        panic!("entered call syntax must produce an owned recipe")
    };
    assert!(matches!(
        &recipe.recipe().operations[..],
        [Op::CallSeal { name, explicit, arguments, .. }] if name == "missing" && explicit == &["Absent"] && arguments.is_empty()
    ));
    assert_eq!(db.trace.counts.get("ScopeBucket:ProviderRun"), None);
    assert_eq!(db.trace.counts.get("Constant:ProviderRun"), None);
}
#[test]
fn body_source_refresh_preserves_relative_recipe_identity_and_rejects_stale_body() {
    let (first, bodies) = prepare(None, "fn live(){40_i64;} fn cold(){missing();}");
    let mut db = Database::default();
    first.publish(&mut db).unwrap();
    let old = bodies[0].1.key();
    let structure = fetch(&mut db, QueryKey::BodyStructure(old.clone()));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("body structure missing")
    };
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
        .unwrap();
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let before = fetch(&mut db, QueryKey::Thunk(pc.clone()));
    let (moved, new_bodies) = prepare(
        Some(&first),
        "// moved source\nfn live(){40_i64;} fn cold(){other::<Missing>();}",
    );
    moved.publish(&mut db).unwrap();
    assert_eq!(new_bodies[0].1.key(), old);
    assert_eq!(
        fetch(&mut db, QueryKey::BodyStructure(old.clone())).fingerprint,
        structure.fingerprint
    );
    assert_eq!(
        fetch(&mut db, QueryKey::Thunk(pc.clone())).fingerprint,
        before.fingerprint
    );
    let (edited, current) = prepare(
        Some(&moved),
        "fn live(){41_i64;} fn cold(){other::<Missing>();}",
    );
    edited.publish(&mut db).unwrap();
    assert_ne!(current[0].1.key(), old);
    assert!(
        matches!(db.fetch(&SyntaxProvider,QueryKey::BodyStructure(old)),QueryPoll::Ready(Err(error)) if error.code=="LRR-E-INTERNAL-SNAPSHOT")
    );
    assert!(
        matches!(db.fetch(&SyntaxProvider,QueryKey::Thunk(pc)),QueryPoll::Ready(Err(error)) if error.code=="LRR-E-INTERNAL-SNAPSHOT")
    );
}
#[test]
fn body_and_thunk_keys_and_result_variants_cannot_be_interchanged() {
    let (prepared, bodies) = prepare(None, "fn live(){40_i64;}");
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let key = bodies[0].1.key();
    let structure = fetch(&mut db, QueryKey::BodyStructure(key.clone()));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("missing body structure")
    };
    let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
    let thunk = fetch(&mut db, QueryKey::Thunk(pc.clone()));
    assert!(!structure.value.matches(&QueryKey::Thunk(pc.clone())));
    assert!(!thunk.value.matches(&QueryKey::BodyStructure(key.clone())));
    assert_ne!(
        QueryKey::BodyStructure(key).fingerprint(),
        QueryKey::Thunk(pc.clone()).fingerprint()
    );
    let mut other = pc;
    other.phase = Phase::Exit.code().unwrap();
    assert!(!thunk.value.matches(&QueryKey::Thunk(other)));
    assert!(structure.value.estimated_bytes() > index.estimated_bytes());
}

#[test]
fn installed_pc_hits_do_not_reenter_queries_and_new_publications_are_rejected() {
    let (prepared, bodies) = prepare(None, "fn live(){40_i64;} fn cold(){missing();}");
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let structure = fetch(&mut db, QueryKey::BodyStructure(bodies[0].1.key()));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("missing index")
    };
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
        .unwrap();
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let mut installed =
        lrr::engine::install::InstalledRecipes::new(db.source_revision(), 64 * 1024 * 1024)
            .unwrap();
    let QueryPoll::Ready(Ok(id)) = installed.resolve(&mut db, pc.clone()) else {
        panic!("missing actual installed recipe")
    };
    let counts = db.trace.counts.clone();
    assert!(
        matches!(installed.resolve(&mut db,pc.clone()),QueryPoll::Ready(Ok(again)) if again==id)
    );
    assert_eq!(db.trace.counts, counts);
    assert_eq!(installed.len(), 1);
    assert_eq!(installed.get(id).unwrap().recipe().key, pc);
    let (edited, _) = prepare(Some(&prepared), "fn live(){41_i64;} fn cold(){missing();}");
    edited.publish(&mut db).unwrap();
    assert!(
        matches!(installed.resolve(&mut db,pc),QueryPoll::Ready(Err(error)) if error.code=="LRR-E-INTERNAL-SNAPSHOT")
    );
}
#[test]
fn installed_recipe_budget_rejection_keeps_the_table_empty() {
    let (prepared, bodies) = prepare(None, "fn live(){40_i64;}");
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let structure = fetch(&mut db, QueryKey::BodyStructure(bodies[0].1.key()));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("missing index")
    };
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
        .unwrap();
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let mut installed =
        lrr::engine::install::InstalledRecipes::new(db.source_revision(), 1).unwrap();
    assert!(
        matches!(installed.resolve(&mut db,pc),QueryPoll::Ready(Err(error)) if error.code=="LRR-E-METADATA-LIMIT")
    );
    assert!(installed.is_empty());
    assert_eq!(installed.metadata_bytes(), 0);
    assert!(
        lrr::engine::install::InstalledRecipes::new(db.source_revision(), 64 * 1024 * 1024 + 1)
            .is_err()
    );
}

#[test]
fn source_literal_executes_through_installed_recipe_without_fetching_its_return_pc() {
    for (spelling, expected) in [("0x2a_i64", 42), ("43_i64", 43)] {
        let (prepared, bodies) = prepare(
            None,
            &format!("fn live(){{{spelling};}} fn cold(){{missing::<Absent>();}}"),
        );
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let structure = fetch(&mut db, QueryKey::BodyStructure(bodies[0].1.key()));
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!("missing index")
        };
        let node = index
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Literal))
            .unwrap();
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let mut store = lrr::runtime::cell::GuestStore::new(1, 1000, 1 << 20).unwrap();
        let mut frame = lrr::vm::frame::Frame::new(
            index.as_ref().clone(),
            index.pc(NodeId(0), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        // This kernel integration supplies the scope entry explicitly. Full source
        // block/let/read phase lowering remains required for T024 acceptance.
        let enter = lrr::ir::validate::verify(
            lrr::ir::recipe::RecipeDraft {
                key: frame.pc().clone(),
                origin: lrr::ir::recipe::Origin {
                    span: index.span(NodeId(0)).unwrap(),
                    expansion_ancestry: vec![],
                },
                operations: vec![
                    Op::ScopeEnter {
                        scope: lrr::syntax::body_index::ScopeId(0),
                    },
                    Op::ScopeEnter { scope: node.scope },
                ],
                exits: vec![RecipeExit::Next(pc.clone())],
                dependencies: vec![],
            },
            index,
        )
        .unwrap();
        lrr::vm::step::step(&mut frame, &mut store, &enter).unwrap();
        let mut installed =
            lrr::engine::install::InstalledRecipes::new(db.source_revision(), 64 * 1024 * 1024)
                .unwrap();
        let QueryPoll::Ready(Ok(id)) = installed.resolve(&mut db, pc) else {
            panic!("actual source recipe missing")
        };
        let lrr::vm::step::StepOutcome::Next(next) =
            lrr::vm::step::step(&mut frame, &mut store, installed.get(id).unwrap()).unwrap()
        else {
            panic!("literal did not select a return PC")
        };
        let value = store
            .read_scalar(frame.cell(node.value.unwrap()).unwrap())
            .unwrap();
        assert_eq!(value.payload, expected);
        assert_eq!(value.type_id, 7);
        assert_eq!(frame.counters().operations_completed, 3);
        let mut expected = [0; lrr::ir::op::OPCODE_COUNT];
        expected[0] = 1;
        expected[4] = 2;
        assert_eq!(frame.counters().started_by_opcode, expected);
        assert!(db.memo(&QueryKey::Thunk(next)).is_none());
        db.finish_run().unwrap();
    }
}

#[test]
fn reached_function_signature_preserves_symbolic_parameters_without_body_or_bound_queries() {
    use lrr::syntax::{ast_adapter::token_text, signatures::GenericKind};
    let (prepared, bodies) = prepare(
        None,
        "fn live<'a,T: Missing,const N:usize=3>(x:&'a [T;N])->T where T: for<'b> Other<&'b T> {missing();} fn cold(x: Unknown)->Unknown{absent();}",
    );
    let live = &bodies[0].1;
    let cold = &bodies[1].1;
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    let value = fetch(&mut db, QueryKey::FunctionSignature(live.def_key));
    let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
        panic!("missing owned function signature")
    };
    assert_eq!(signature.definition, live.def_key);
    assert_eq!(
        signature
            .generics
            .iter()
            .map(|p| p.kind)
            .collect::<Vec<_>>(),
        vec![GenericKind::Lifetime, GenericKind::Type, GenericKind::Const]
    );
    assert_eq!(signature.generics[1].name, "T");
    assert_eq!(signature.generics[1].ordinal, 1);
    assert_eq!(token_text(&signature.generics[1].bounds), "Missing");
    assert_eq!(token_text(&signature.generics[2].const_type), "usize");
    assert_eq!(token_text(&signature.generics[2].default), "3");
    assert_eq!(token_text(&signature.parameters[0].ty), "& 'a [ T ; N ]");
    assert_eq!(token_text(&signature.result), "T");
    use lrr::traits::term::{ConstTerm, ParamKey, RegionTerm, TyTerm};
    let key = |ordinal| ParamKey {
        definition: signature.definition,
        ordinal,
    };
    assert_eq!(signature.result_symbolic, Some(TyTerm::Param(key(1))));
    assert_eq!(
        signature.parameters[0].symbolic,
        Some(TyTerm::Ref(
            RegionTerm::Param(key(0)),
            false,
            Box::new(TyTerm::Array(
                Box::new(TyTerm::Param(key(1))),
                ConstTerm::Param(key(2))
            ))
        ))
    );

    assert!(token_text(&signature.predicates).contains("Other"));
    assert!(signature.estimated_bytes() > std::mem::size_of_val(signature.as_ref()));
    assert!(db.trace.events.iter().all(|event| !matches!(
        &event.key,
        QueryKey::BodyStructure(_)
            | QueryKey::Thunk(_)
            | QueryKey::ScopeBucket(_)
            | QueryKey::Constant(_)
    )));
    assert!(db.trace.events.iter().all(|event| !matches!(&event.key,
        QueryKey::Signature(def) | QueryKey::FunctionSignature(def) if *def == cold.def_key)));
}

#[test]
fn function_signature_identity_ignores_cold_body_edits_but_tracks_declared_contracts() {
    let (first, bodies) = prepare(None, "fn live<T: Missing>(x:T)->T {missing();}");
    let definition = bodies[0].1.def_key;
    let mut db = Database::default();
    first.publish(&mut db).unwrap();
    let before = fetch(&mut db, QueryKey::FunctionSignature(definition))
        .value
        .fingerprint();
    let (edited, bodies) = prepare(Some(&first), "fn live<T: Missing>(x:T)->T {other();}");
    assert_eq!(bodies[0].1.def_key, definition);
    edited.publish(&mut db).unwrap();
    assert_eq!(
        fetch(&mut db, QueryKey::FunctionSignature(definition))
            .value
            .fingerprint(),
        before
    );
    let (changed, bodies) = prepare(Some(&edited), "fn live<T: Different>(x:T)->T {other();}");
    changed.publish(&mut db).unwrap();
    assert_ne!(
        fetch(&mut db, QueryKey::FunctionSignature(bodies[0].1.def_key))
            .value
            .fingerprint(),
        before
    );
}

#[test]
fn function_signature_rejects_duplicate_parameter_identity_without_entering_body() {
    let (prepared, bodies) = prepare(None, "fn live<T,T>(x:T){missing();}");
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    assert!(
        matches!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(bodies[0].1.def_key)), QueryPoll::Ready(Err(error)) if error.code == "LRR-E-CALL-SIGNATURE")
    );
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
}

#[test]
fn signature_substitution_keeps_source_params_rigid_and_does_not_force_unused_bounds() {
    use lrr::{
        frontend::signature_subst::SignatureSubstitution,
        ir::op::PrimitiveType,
        traits::term::{ParamKey, TyTerm},
    };
    let (prepared, bodies) = prepare(None, "fn live<T: Missing,U: Other>(x:T)->T{missing();}");
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    let value = fetch(&mut db, QueryKey::FunctionSignature(bodies[0].1.def_key));
    let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
        panic!("missing signature")
    };
    let source = TyTerm::Param(ParamKey {
        definition: signature.definition,
        ordinal: 0,
    });
    let mut substitution = SignatureSubstitution::new(signature, Digest([141; 32])).unwrap();
    assert_eq!(substitution.source_parameter(0), Some(&source));
    assert_eq!(substitution.source_result(), Some(&source));
    assert!(substitution.binding(0).is_none());
    assert!(substitution.binding(1).is_none());
    let actual = TyTerm::Primitive(PrimitiveType::I64);
    substitution.constrain_argument(0, &actual).unwrap();
    substitution.constrain_result(&actual).unwrap();
    assert_eq!(substitution.binding(0), Some(&actual));
    assert!(substitution.binding(1).is_none());
    assert_eq!(substitution.source_parameter(0), Some(&source));
    assert_eq!(
        substitution
            .constrain_result(&TyTerm::Primitive(PrimitiveType::U64))
            .unwrap_err()
            .code,
        "LRR-E-GENERIC-BINDING"
    );
    assert_eq!(substitution.binding(0), Some(&actual));
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::BodyStructure(_)
            | QueryKey::Thunk(_)
            | QueryKey::ScopeBucket(_)
            | QueryKey::Constant(_)
    )));
}

#[test]
fn signature_type_paths_remain_deferred_until_source_identity_resolution() {
    let (prepared, bodies) = prepare(None, "struct i64; fn live(x:i64)->i64{missing();}");
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let value = fetch(&mut db, QueryKey::FunctionSignature(bodies[0].1.def_key));
    let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
        panic!("missing signature")
    };
    assert!(signature.parameters[0].symbolic.is_none());
    assert!(signature.result_symbolic.is_none());
    assert_eq!(
        lrr::syntax::ast_adapter::token_text(&signature.parameters[0].ty),
        "i64"
    );
}

fn parameter_source_frame(
    source: &str,
    value: i64,
    max_handles: u32,
) -> (
    PreparedSourceInputs,
    Database,
    lrr::vm::frame::Frame,
    lrr::runtime::cell::GuestStore,
    lrr::vm::call::ArgumentPacket,
) {
    use lrr::runtime::{
        cell::{CellOwner, GuestStore, StorageContract},
        shape::TypeId,
        value::ValueRepr,
    };
    let (prepared, bodies) = prepare(None, source);
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let outcome = fetch(&mut db, QueryKey::BodyStructure(bodies[0].1.key()));
    let QueryValue::BodyStructure(body) = outcome.value.as_ref() else {
        panic!("body absent");
    };
    let mut store = GuestStore::new(1, max_handles, 1024 * 1024).unwrap();
    let frame = lrr::vm::frame::Frame::new(
        body.as_ref().clone(),
        body.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    let owner = store.new_frame_anchor().unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(TypeId(7)),
            false,
            CellOwner::FrameSlot {
                frame: owner,
                slot: 0,
            },
        )
        .unwrap();
    let ticket = store
        .ticket(ValueRepr::signed(TypeId(7), value).unwrap())
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    let packet = lrr::vm::call::ArgumentPacket::seal(&mut store, &[cell], &[TypeId(7)]).unwrap();
    (prepared, db, frame, store, packet)
}
#[test]
fn sealed_parameter_batch_executes_source_parameter_reads_and_local_shadowing() {
    use lrr::{
        runtime::shape::TypeId,
        vm::step::{StepOutcome, step},
    };
    for (source, expected, reads) in [
        ("fn live(x:i64)->i64{x+x}", 62, 2),
        ("fn live(x:i64)->i64{let x=9i64;x}", 9, 0),
    ] {
        let (_prepared, mut db, mut frame, mut store, packet) =
            parameter_source_frame(source, 31, 1000);
        frame
            .initialize_parameters(&packet, &[(TypeId(7), false)], &mut store)
            .unwrap();
        assert!(
            frame
                .initialize_parameters(&packet, &[(TypeId(7), false)], &mut store)
                .is_err()
        );
        loop {
            let outcome = fetch(&mut db, QueryKey::Thunk(frame.pc().clone()));
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("thunk absent");
            };
            match step(&mut frame, &mut store, recipe).unwrap() {
                StepOutcome::Next(_) => {}
                StepOutcome::Return(ticket) => {
                    assert_eq!(store.peek_ticket(ticket).unwrap().payload as i64, expected);
                    frame.finish_return(&mut store).unwrap();
                    store.discard(ticket).unwrap();
                    break;
                }
                _ => panic!("unexpected parameter body control"),
            }
        }
        assert_eq!(frame.counters().started_by_opcode[20], 1);
        assert_eq!(frame.counters().started_by_opcode[21], reads);
        assert_eq!(frame.counters().return_scopes_cleaned, 1);
    }
}
#[test]
fn parameter_initialization_descriptor_and_quota_failures_preserve_the_sealed_packet() {
    use lrr::runtime::shape::TypeId;
    for limit in [6, 1000] {
        let (_prepared, _db, mut frame, mut store, packet) =
            parameter_source_frame("fn live(x:i64){x;}", 31, limit);
        let before = (store.budget().handles(), store.budget().bytes());
        assert_eq!(
            frame
                .initialize_parameters(&packet, &[], &mut store)
                .unwrap_err()
                .code,
            "LRR-E-CALL-ARITY"
        );
        assert_eq!((store.budget().handles(), store.budget().bytes()), before);

        assert_eq!(
            frame
                .initialize_parameters(&packet, &[(TypeId(12), false)], &mut store)
                .unwrap_err()
                .code,
            "LRR-E-CALL-ARG-TYPE"
        );
        assert_eq!((store.budget().handles(), store.budget().bytes()), before);
        if limit == 6 {
            assert_eq!(
                frame
                    .initialize_parameters(&packet, &[(TypeId(7), false)], &mut store)
                    .unwrap_err()
                    .code,
                "LRR-E-RUNTIME-RESOURCE"
            );
            assert_eq!((store.budget().handles(), store.budget().bytes()), before);
            packet.discard(&mut store).unwrap();
        } else {
            frame
                .initialize_parameters(&packet, &[(TypeId(7), false)], &mut store)
                .unwrap();
            assert!(packet.discard(&mut store).is_err());
        }
        assert_eq!(frame.fuel_snapshot().used, 0);
    }
}

#[test]
fn qualified_call_thunks_capture_owned_paths_without_namespace_queries() {
    let (prepared, bodies) = prepare(
        None,
        "fn live(){crate::missing::answer::<Absent>();}fn cold(){invalid();}",
    );
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let structure = fetch(&mut db, QueryKey::BodyStructure(bodies[0].1.key()));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("body missing")
    };
    let node = index
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .unwrap();
    let thunk = fetch(
        &mut db,
        QueryKey::Thunk(index.pc(node.id, Phase::Entry).unwrap()),
    );
    let QueryValue::Thunk(recipe) = thunk.value.as_ref() else {
        panic!("recipe missing")
    };
    assert!(
        matches!(&recipe.recipe().operations[..],[Op::CallSeal{name,explicit,arguments,..}] if name=="crate::missing::answer" && explicit==&["Absent"] && arguments.is_empty())
    );
    assert_eq!(db.trace.counts.get("ScopeBucket:ProviderRun"), None);
    assert_eq!(db.trace.counts.get("FunctionSignature:ProviderRun"), None);
    assert_eq!(db.trace.counts.get("Constant:ProviderRun"), None);
}

#[test]
fn stock_cfg_disabled_statement_keeps_initializer_and_later_assertions_cold() {
    let original =
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cfg/cfg_stmt_expr.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (prepared, bodies) = prepare_edition(None, original, edition);
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let body = &bodies.iter().find(|(name, _)| name == "main").unwrap().1;
        let structure = fetch(&mut db, QueryKey::BodyStructure(body.key()));
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!("missing original cfg body")
        };
        let statement = index
            .nodes()
            .iter()
            .find(|node| {
                node.role == NodeRole::Statement
                    && original[(body.range.start + node.range.start) as usize
                        ..(body.range.start + node.range.end) as usize]
                        .contains("let a = ()")
            })
            .unwrap();
        let key = index.pc(statement.id, Phase::Entry).unwrap();
        let outcome = fetch(&mut db, QueryKey::Thunk(key.clone()));
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("missing disabled statement recipe")
        };
        assert!(
            recipe.recipe().operations.iter().all(|op| matches!(
                op,
                Op::ScopeEnter { .. } | Op::ScopeLeave { .. } | Op::NumericHint { .. }
            )),
            "disabled original statement must retire with only structural scope operations: {:?}",
            recipe.recipe()
        );
        assert!(!recipe.recipe().operations.iter().any(|op| matches!(
            op,
            Op::Unsupported(_) | Op::BindLocal { .. } | Op::SetHint { .. }
        )));
        let RecipeExit::Next(next) = &recipe.recipe().exit else {
            panic!("disabled statement must advance structurally")
        };
        assert!(db.memo(&QueryKey::Thunk(next.clone())).is_none());
        let forced: Vec<_> = db
            .trace
            .events
            .iter()
            .filter_map(|event| match &event.key {
                QueryKey::Thunk(pc) => Some(pc),
                _ => None,
            })
            .collect();
        assert!(forced.iter().all(|pc| **pc == key));
    }
}

#[test]
fn stock_cfg_false_use_item_runs_unchanged_in_every_official_edition() {
    use lrr::engine::run::{RunMachine, RunPoll, ValueSummary};
    let original =
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cfg/cfg-false-use-item.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (prepared, bodies) = prepare_edition(None, original, edition);
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let body = &bodies.iter().find(|(name, _)| name == "main").unwrap().1;
        db.begin_run().unwrap();
        let mut machine = match RunMachine::start(&mut db, body.key(), 1_000) {
            QueryPoll::Ready(Ok(machine)) => machine,
            QueryPoll::Ready(Err(error)) => panic!("original cfg startup: {error}"),
            QueryPoll::NeedInput(input) => panic!("original cfg startup demand: {input:?}"),
        };
        let report = match machine.poll(&mut db) {
            RunPoll::Completed(report) => report,
            RunPoll::Fault(error) => panic!("original cfg program: {error}"),
            RunPoll::Panicked(report) => panic!("original cfg panic: {:?}", report.panic.kind),
            RunPoll::NeedInput(input) => panic!("original cfg runtime demand: {input:?}"),
        };
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.live_edition, edition);
        assert_eq!(db.trace.counts.get("ScopeBucket:ProviderRun"), None);
        assert_eq!(db.trace.counts.get("LookupPath:ProviderRun"), None);
    }
}

#[test]
fn stock_cfg_false_use_adapter_keeps_disabled_alias_out_of_callee_guard() {
    let original =
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cfg/cfg-false-use-item.rs");
    // The upstream original is unchanged. This external LRR adaptation makes
    // the inactive import share a name with a reached source function call.
    for predicate in ["false", "any()"] {
        let source = original.replace(
            "#[cfg(false)] use bar as foo;",
            &format!("#[cfg({predicate})] use bar as foo;\nfoo();"),
        ) + "\nfn foo() {}\n";
        for edition in [2015, 2018, 2021, 2024] {
            let (prepared, bodies) = prepare_edition(None, &source, edition);
            let mut db = Database::default();
            prepared.publish(&mut db).unwrap();
            let body = &bodies.iter().find(|(name, _)| name == "main").unwrap().1;
            let outcome = fetch(&mut db, QueryKey::BodyStructure(body.key()));
            let QueryValue::BodyStructure(index) = outcome.value.as_ref() else {
                panic!("cfg adaptation body absent")
            };
            let call = index
                .nodes()
                .iter()
                .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
                .unwrap();
            let outcome = fetch(
                &mut db,
                QueryKey::Thunk(index.pc(call.id, Phase::Entry).unwrap()),
            );
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("cfg adaptation call absent")
            };
            assert!(
                recipe
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::CallSeal { name, explicit, arguments, .. } if name == "foo" && explicit.is_empty() && arguments.is_empty())),
                "disabled imported alias must not shadow active callee: {:?}",
                recipe.recipe()
            );
            assert_eq!(db.trace.counts.get("ScopeBucket:ProviderRun"), None);
        }
    }
}
