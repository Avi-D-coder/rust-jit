use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        names::NameNamespaces,
        type_aliases::{AliasNormalizer, AliasTarget},
    },
    ir::op::PrimitiveType,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
    traits::term::TyTerm,
};
fn source(text: &str) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("src/main.rs").unwrap();
    let crate_key = Digest([152; 32]);
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
fn ready(poll: QueryPoll<AliasTarget>) -> AliasTarget {
    match poll {
        QueryPoll::Ready(Ok(target)) => target,
        QueryPoll::Ready(Err(error)) => panic!("{error}"),
        QueryPoll::NeedInput(input) => panic!("unexpected input {input:?}"),
    }
}
#[test]
fn reached_alias_chains_use_definition_scope_and_preserve_nominal_identity() {
    let (inputs, view) = source(
        "type A=B;type B=i64; struct Word;mod defs{pub struct Word;pub type Named=Word;} fn cold(x:Missing){absent();}",
    );
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let sources = [&view];
    let normalizer = AliasNormalizer::new(&graph, &sources).unwrap();
    assert_eq!(
        ready(normalizer.normalize(&mut db, id(&view, "A"))),
        AliasTarget::Type(TyTerm::Primitive(PrimitiveType::I64))
    );
    let named = id(&view, "Named");
    let expected = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "Word" && i.scope.module_path == ["defs"])
        .unwrap()
        .def_key
        .fingerprint();
    assert_eq!(
        ready(normalizer.normalize(&mut db, named)),
        AliasTarget::Nominal(expected)
    );
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
    assert!(db.trace.events.iter().all(|event|!matches!(event.key,QueryKey::FunctionSignature(def)|QueryKey::Signature(def) if def==id(&view,"cold"))));
}
#[test]
fn alias_cycles_fault_only_on_demand_and_repeated_capture_uses_owned_memos() {
    let (inputs, view) = source("type A=B;type B=A;type Good=u64;fn cold(x:A){missing();}");
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let sources = [&view];
    let normalizer = AliasNormalizer::new(&graph, &sources).unwrap();
    let good = id(&view, "Good");
    assert_eq!(
        ready(normalizer.normalize(&mut db, good)),
        AliasTarget::Type(TyTerm::Primitive(PrimitiveType::U64))
    );
    assert!(db.trace.events.iter().all(|event|!matches!(event.key,QueryKey::TypeAlias(def)if def==id(&view,"A")||def==id(&view,"B"))));
    assert!(
        matches!(normalizer.normalize(&mut db,id(&view,"A")),QueryPoll::Ready(Err(error))if error.code=="LRR-E-TYPE-ALIAS-CYCLE"&&error.message.contains(&id(&view,"A").hex())&&error.message.contains(&id(&view,"B").hex()))
    );
    let QueryPoll::Ready(Ok(first)) = db.fetch(&SyntaxProvider, QueryKey::TypeAlias(good)) else {
        panic!("missing alias")
    };
    let QueryPoll::Ready(Ok(second)) = db.fetch(&SyntaxProvider, QueryKey::TypeAlias(good)) else {
        panic!("missing alias")
    };
    assert!(std::sync::Arc::ptr_eq(&first.value, &second.value));
    assert!(matches!(first.value.as_ref(),QueryValue::TypeAlias(recipe)if recipe.definition==good));
}
#[test]
fn alias_pending_generic_and_stale_contexts_remain_explicit() {
    for (text, pending) in [
        ("make!();type A=i64;", true),
        ("type A<T: Missing>=T;", false),
    ] {
        let (inputs, view) = source(text);
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        let sources = [&view];
        let normalizer = AliasNormalizer::new(&graph, &sources).unwrap();
        let target = ready(normalizer.normalize(&mut db, id(&view, "A")));
        if pending {
            assert!(matches!(target, AliasTarget::NeedProducer(_)))
        } else {
            assert!(
                matches!(target,AliasTarget::Deferred{definition,..}if definition==id(&view,"A"))
            )
        }
    }
    let (old_inputs, old_view) = source("type A=i64;");
    let (_, new_view) = source("type A=i64;fn other(){}");
    let graph = NameNamespaces::from_outlines(&[new_view.outline()]).unwrap();
    let sources = [&new_view];
    let normalizer = AliasNormalizer::new(&graph, &sources).unwrap();
    let mut db = Database::default();
    old_inputs.publish(&mut db).unwrap();
    assert_eq!(id(&old_view, "A"), id(&new_view, "A"));
    assert!(
        matches!(normalizer.normalize(&mut db,id(&new_view,"A")),QueryPoll::Ready(Err(error))if error.code=="LRR-E-TYPE-CONTEXT")
    );
}

#[test]
fn alias_depth_budget_rejects_before_fetching_the_unprocessed_declaration() {
    let mut text = String::new();
    for i in 0..256 {
        text.push_str(&format!("type A{i}=A{};", i + 1));
    }
    text.push_str("type A256=i64;");
    let (inputs, view) = source(&text);
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let sources = [&view];
    let normalizer = AliasNormalizer::new(&graph, &sources).unwrap();
    assert!(
        matches!(normalizer.normalize(&mut db,id(&view,"A0")),QueryPoll::Ready(Err(error))if error.code=="LRR-E-METADATA-LIMIT")
    );
    let unprocessed = id(&view, "A256");
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key,QueryKey::TypeAlias(def)if def==unprocessed))
    );
}

#[test]
fn rustc_original_higher_ranked_alias_keeps_rigid_parameter_and_bound_region() {
    use lrr::traits::{
        subst::{Substitution, validate_closed},
        term::{BoundKind, GenericArg, ParamKey, RegionTerm},
    };
    const ORIGINAL: &str = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-outlives-projection-hrtype.rs"
    );
    let (inputs, view) = source(ORIGINAL);
    let definition = id(&view, "FnType");
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(value)) = db.fetch(&SyntaxProvider, QueryKey::TypeAlias(definition))
    else {
        panic!("original alias header not captured")
    };
    let QueryValue::TypeAlias(recipe) = value.value.as_ref() else {
        panic!("alias query lost its record")
    };
    assert_eq!(recipe.definition, definition);
    assert_eq!(recipe.generics.len(), 1);
    let parameter = ParamKey {
        definition,
        ordinal: recipe.generics[0].ordinal,
    };
    let symbolic = recipe
        .symbolic
        .as_ref()
        .expect("original higher-ranked target missing");
    let TyTerm::Function(function) = symbolic else {
        panic!("original function pointer lost")
    };
    assert_eq!(function.variables, vec![BoundKind::Region]);
    assert_eq!(
        function.value.parameters,
        vec![TyTerm::Ref(
            RegionTerm::Bound { depth: 0, index: 0 },
            false,
            Box::new(TyTerm::Param(parameter))
        )]
    );
    assert_eq!(
        *function.value.result,
        TyTerm::Primitive(PrimitiveType::Unit)
    );
    validate_closed(symbolic).unwrap();
    let mut subst = Substitution::default();
    subst
        .insert(
            parameter,
            GenericArg::Type(TyTerm::Primitive(PrimitiveType::Unit)),
        )
        .unwrap();
    let instantiated = subst.apply(symbolic).unwrap();
    validate_closed(&instantiated).unwrap();
    let TyTerm::Function(function) = instantiated else {
        panic!("substitution lost function")
    };
    assert_eq!(
        function.value.parameters,
        vec![TyTerm::Ref(
            RegionTerm::Bound { depth: 0, index: 0 },
            false,
            Box::new(TyTerm::Primitive(PrimitiveType::Unit))
        )]
    );
    // This reads the original alias through the production immutable-source
    // query. It does not establish its projection/outlives goal or run main.
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::FunctionSignature(_)
    )));
}
