//! The actual cross-crate check-pass fixture exercises source preparation only.
//! It is neither an upstream runtime contract nor a whole-fixture pass.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
    },
    expand::{
        builtins::FrozenCompileEnvironment,
        source_declarative::{
            CapturedMacroContext, DeclarativeBody, SourceDeclarativeDemand,
            assembly::SyntaxRequestReceipt, engine_version,
        },
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{impl_index::DeclarationMacroIndex, source_macro_routes::SourceMacroRoutes},
    project::cargo_units::DependencyEdge,
    source::manifest::{ItemKind, Namespace},
    syntax::token_wire::{self as wire, Key, TokenForest, TokenTree},
};
const CROSS: &[u8] = include_bytes!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/cross-crate-pat-span.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual cross-crate source preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual cross-crate input frontier: {input:?}"),
    }
}
fn token_span<'a>(forest: &'a TokenForest, name: &str) -> &'a wire::SpanRecord {
    let mut pending: Vec<_> = forest.roots().iter().collect();
    while let Some(token) = pending.pop() {
        let key = match token {
            TokenTree::Group { children, .. } => {
                pending.extend(children);
                continue;
            }
            TokenTree::Ident { text, span, .. } if text == name => *span,
            TokenTree::Literal { token_text, span } if token_text == name => *span,
            _ => continue,
        };
        return forest
            .spans()
            .iter()
            .find(|span| span.key() == key)
            .unwrap();
    }
    panic!("actual pinned output token absent: {name}")
}
#[test]
fn unchanged_cross_crate_call_borrows_sealed_aux_definition_preserving_actual_editions() {
    let package = fixture_root().join("tests/ui/macros");
    let aux_path = package.join("auxiliary/foreign-crate-macro-pat.rs");
    let caller_path = package.join("cross-crate-pat-span.rs");
    let (mut manifest, aux_unit) = manifest(&aux_path, AUX, 2018, aux_path.parent().unwrap());
    // Same schema4 edge encoding as the existing two-unit role publication
    // controls; actual identifiers are fingerprints of coherent configured keys.
    let mut root = manifest.graph.units[&aux_unit].clone();
    root.key.role = UnitRole::Live;
    root.key.target_kind = TargetKind::Binary;
    root.key.target_name = "cross_crate_pat_span".into();
    root.key.edition = 2021;
    root.source_path = caller_path.to_str().unwrap().into();
    root.dependencies = vec![DependencyEdge {
        alias: "foreign_crate_macro_pat".into(),
        target: aux_unit,
        public: false,
        no_prelude: false,
        no_unused: false,
    }];
    let mut edges = StableEncoder::new("lrr/v2/cargo-unit-edges");
    edges.sequence(&root.dependencies, |e, edge| {
        e.string(&edge.alias);
        e.digest(edge.target);
        e.bool(edge.public);
        e.bool(edge.no_prelude);
        e.bool(edge.no_unused);
    });
    root.key.dependency_edges = edges.finish();
    let caller_unit = root.key.fingerprint();
    manifest.graph.units.insert(caller_unit, root);
    manifest.graph.roots = BTreeSet::from([caller_unit]);
    manifest.inputs.source_files.insert(
        caller_path.to_str().unwrap().into(),
        Some(Digest::of_bytes(CROSS)),
    );
    let mut env = manifest.compile_env[&aux_unit].clone();
    env.unit = caller_unit;
    env.cargo.insert(
        "CARGO_MANIFEST_DIR".into(),
        package.to_str().unwrap().into(),
    );
    manifest.compile_env.insert(caller_unit, env);
    manifest.validate().unwrap();
    let (aux, aux_selection) = input(aux_unit, "foreign-crate-macro-pat.rs", AUX, 2018);
    let (caller, caller_selection) = input(caller_unit, "cross-crate-pat-span.rs", CROSS, 2021);
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        vec![aux, caller],
        BTreeMap::new(),
        vec![SourceDependency {
            consumer: caller_unit,
            alias: "foreign_crate_macro_pat".into(),
            provider: aux_unit,
        }],
    )
    .unwrap();
    let aux = select(&inputs, &aux_selection);
    let caller = select(&inputs, &caller_selection);
    assert_eq!(aux.captured_macro_blob().unwrap().bytes(), AUX);
    assert_eq!(caller.captured_macro_blob().unwrap().bytes(), CROSS);
    let facet = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            aux_unit,
            aux_path.to_str().unwrap(),
            aux.share(),
        )
        .unwrap(),
    );
    let mut db = Database::default();
    let mut base = BTreeMap::from([(
        InputKey::SourceDependencies,
        InputValue::SourceDependencies(caller.source_dependencies().unwrap()),
    )]);
    for (unit, env) in &manifest.compile_env {
        base.insert(
            InputKey::UnitCompileEnv(*unit),
            InputValue::UnitCompileEnv(Arc::new(
                FrozenCompileEnvironment::new(*unit, env).unwrap(),
            )),
        );
    }
    let caller_environment =
        FrozenCompileEnvironment::new(caller_unit, &manifest.compile_env[&caller_unit])
            .unwrap()
            .fingerprint();
    db.publish_inputs(base).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.publish_macro_export_sources(&[Arc::clone(&facet)])
        .unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![aux.share()],
        aux.outline().root_scope.clone(),
        Namespace::Macro,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_macro_export_source_publications(&mut db)
    .unwrap()
    .with_declarative(facet.environment(), engine_version())
    .unwrap();
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    discovery.publish(&mut db).unwrap();
    let owned =
        ready(discovery.take_completed_macro_export_providers(&mut db, &[Arc::clone(&facet)]));
    drop(discovery);
    // Full sealed guard is a COLD coordinator check, never a runtime recipe.
    owned.require_current(&db).unwrap();
    let callers = [SourceDeclarative::new(caller.share()).unwrap()];
    let consumer = &callers[0];
    let context = CapturedMacroContext {
        provider: consumer,
        owners: &callers,
        before: 0,
        inherited: None,
        exports: Some(&owned),
    };
    let mut routes = consumer.routes_in_context(context).unwrap();
    routes.extend(owned.routes().iter().cloned());
    // Actual existing source/prelude/module route worklist, not a test resolver
    // or fabricated NameDefinition. It does not certify runtime dependencies.
    let names = SourceMacroRoutes::discovery_graph(&routes).unwrap();
    let scan = caller.declaration_inventory(Default::default()).unwrap();
    let index = DeclarationMacroIndex::new(Arc::clone(&scan)).unwrap();
    let producers = scan
        .producers()
        .iter()
        .filter(|producer| {
            producer.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
        })
        .collect::<Vec<_>>();
    assert_eq!(producers.len(), 1);
    let actual = producers[0];
    let actual_text = std::str::from_utf8(CROSS).unwrap();
    assert!(
        actual_text[actual.range.start as usize..actual.range.end as usize]
            .contains("custom_matches!")
    );
    db.trace.detailed = true;
    let trace = db.trace.events.len();
    let SourceDeclarativeDemand::Ready(request) = consumer
        .declaration_import_request_in_context(
            &index,
            actual.identity,
            &names,
            context,
            caller_environment,
            engine_version(),
        )
        .unwrap()
    else {
        panic!("actual cross-crate declaration request unresolved")
    };
    assert_eq!(
        db.trace.events.len(),
        trace,
        "pure request must not fetch providers"
    );
    assert_eq!(request.key().definition_edition, 2018);
    assert_eq!(request.key().call_edition, 2021);
    assert_eq!(request.key().invocation, actual.identity);
    assert_eq!(request.key().grammar, actual.grammar);
    assert!(!request.is_compiler());
    assert!(request.procedural_request().is_none());
    let definition = aux
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.item_kind == ItemKind::Macro)
        .unwrap();
    let selected = crate::expand::scopes::MacroDefinition {
        identity: definition.def_key.fingerprint(),
        defining_unit: aux_unit,
        definition_edition: 2018,
    };
    let owner = context.definition_owner(&selected).unwrap();
    assert!(std::ptr::eq(owner, &owned.providers()[0]));
    assert!(!std::ptr::eq(owner, consumer));
    assert_eq!(owner.module_source().selection(), &aux_selection);
    assert_eq!(
        context.provider.module_source().selection(),
        &caller_selection
    );
    let receipt = SyntaxRequestReceipt::capture(&request).unwrap();
    assert_eq!(receipt.key(), request.key());
    assert_eq!(
        receipt.key().macro_identity,
        crate::expand::identity::MacroIdentity::Declaration {
            definition: selected.identity
        },
    );
    assert_eq!(request.owner, aux_unit);
    let base = request.source_base().unwrap();
    assert_eq!(base.source.unit, Key(caller_unit.0));
    assert_eq!(
        base.source.content,
        Key(caller_selection.outline.parse.content.0)
    );
    assert_eq!(base.edition, wire::Edition::E2021);
    assert!(actual.range.start <= base.start && base.end <= actual.range.end);
    let origin = request.origin.as_ref().unwrap();
    assert_eq!(
        origin.invocation.as_ref().unwrap().source.unit,
        Key(caller_unit.0)
    );
    assert_eq!(
        origin.definition.as_ref().unwrap().location.source.unit,
        Key(aux_unit.0)
    );
    assert_eq!(
        origin.definition.as_ref().unwrap().location.range,
        aux.macro_definition_header(selected.identity).unwrap().0
    );
    assert!(matches!(&request.body, DeclarativeBody::Source(_)));
    let output = request.expand().unwrap();
    assert_eq!(output.key(), request.key());
    let tokens = output.tokens().unwrap();
    for spelling in ["match", "true", "false"] {
        let span = token_span(&tokens, spelling);
        assert_eq!(span.source.unit, Key(aux_unit.0));
        assert_eq!(span.edition, wire::Edition::E2018);
    }
    let caller_tokens = TokenForest::decode(&request.input.encode().unwrap()).unwrap();
    for spelling in ["b'3'", "b'0'", "b'9'"] {
        let before = token_span(&caller_tokens, spelling);
        let after = token_span(&tokens, spelling);
        assert_eq!(before.source, after.source);
        assert_eq!((before.start, before.end), (after.start, after.end));
        assert_eq!(after.source.unit, Key(caller_unit.0));
        assert_eq!(after.edition, wire::Edition::E2021);
    }
    assert!(matches!(consumer.declaration_import_request_in_context(
        &index, actual.identity, &names, CapturedMacroContext { exports: None, ..context },
        caller_environment, engine_version(),
    ), Err(error) if error.message == "selected captured macro owner absent"));
    assert!(matches!(consumer.declaration_import_request_in_context(
        &index, actual.identity, &names, CapturedMacroContext { provider: owner, ..context },
        caller_environment, engine_version(),
    ), Err(error) if error.message == "request caller provider/context differs"));
    for key in [
        InputKey::CapturedSource(aux_selection.location_identity()),
        InputKey::SourceBlob(aux_selection.outline.parse.content),
        InputKey::DefinitionHeader(selected.identity),
        InputKey::DefinitionBody(selected.identity),
    ] {
        assert!(db.input(&key).is_none());
    }
    assert!(db.prepared_source(&aux_selection).unwrap().is_none());
    assert_eq!(db.trace.truncated, 0);
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
    owned.require_current(&db).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert!(owned.require_current(&db).is_err());
}
