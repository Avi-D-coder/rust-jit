//! Authority controls using the unchanged official exported-macro auxiliary.
//! Selected source receipts never substitute for the full discovery seal.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::{InputRequest, QueryPoll},
    },
    expand::{
        builtins::FrozenCompileEnvironment,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    source::manifest::Namespace,
};
#[test]
fn borrowed_facet_requires_completed_discovery_and_genuine_published_seal() {
    let package = fixture_root().join("tests/ui/macros/auxiliary");
    let path = package.join("foreign-crate-macro-pat.rs");
    let (manifest, unit) = manifest(&path, AUX, 2018, &package);
    let (source, selection) = input(unit, "foreign-crate-macro-pat.rs", AUX, 2018);
    let inputs = PreparedSourceInputs::refresh(None, vec![source], BTreeMap::new()).unwrap();
    let source = select(&inputs, &selection);
    let facet = Arc::new(
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), source.share())
            .unwrap(),
    );
    assert_eq!(facet.unit_graph(), manifest.graph.fingerprint());
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Macro,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative(
        Digest([194; 32]),
        crate::expand::source_declarative::engine_version(),
    )
    .unwrap();
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([(
        InputKey::UnitCompileEnv(unit),
        InputValue::UnitCompileEnv(Arc::new(
            FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit]).unwrap(),
        )),
    )]))
    .unwrap();
    db.publish_macro_export_sources(&[Arc::clone(&facet)])
        .unwrap();
    assert!(matches!(
        discovery.completed_macro_export_facet(&mut db, &facet),
        QueryPoll::Ready(Err(_))
    ));
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let revision = db.revision();
    match discovery.completed_macro_export_facet(&mut db, &facet) {
        QueryPoll::NeedInput(InputRequest::Discover { key }) => {
            assert_eq!(&key, discovery.key());
        }
        other => panic!("selected preparation replaced the genuine seal demand: {other:?}"),
    }
    assert_eq!(db.revision(), revision);
    assert!(
        db.input(&InputKey::CapturedSource(selection.location_identity()))
            .is_none()
    );
    assert!(
        db.input(&InputKey::SourceBlob(selection.outline.parse.content))
            .is_none()
    );
}

#[test]
fn genuine_role_source_seal_yields_borrowed_view_without_runtime_source_grants() {
    let package = fixture_root().join("tests/ui/macros/auxiliary");
    let path = package.join("foreign-crate-macro-pat.rs");
    let (manifest, unit) = manifest(&path, AUX, 2018, &package);
    let (source, selection) = input(unit, "foreign-crate-macro-pat.rs", AUX, 2018);
    let inputs = PreparedSourceInputs::refresh(None, vec![source], BTreeMap::new()).unwrap();
    let source = select(&inputs, &selection);
    let facet = Arc::new(
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), source.share())
            .unwrap(),
    );
    let mut db = Database::default();
    // Publish the genuine captured alias/root graph and environment only. The
    // macro source remains solely in the explicit facet/discovery owners.
    db.publish_inputs(BTreeMap::from([
        (
            InputKey::SourceDependencies,
            InputValue::SourceDependencies(source.source_dependencies().unwrap()),
        ),
        (
            InputKey::UnitCompileEnv(unit),
            InputValue::UnitCompileEnv(Arc::new(
                FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit]).unwrap(),
            )),
        ),
    ]))
    .unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.publish_macro_export_sources(&[Arc::clone(&facet)])
        .unwrap();
    let build = |graph| {
        SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Macro,
            graph,
            1,
        )
        .unwrap()
        .with_declarative(
            Digest([194; 32]),
            crate::expand::source_declarative::engine_version(),
        )
        .unwrap()
    };
    let mut absent_db = Database::default();
    let mut unchanged = build(manifest.graph.fingerprint())
        .with_macro_export_source_publications(&mut absent_db)
        .unwrap();
    let mut old_route = build(manifest.graph.fingerprint());
    let SourceDiscoveryStep::Sealed(unchanged_seal) = unchanged.poll().unwrap() else {
        panic!("ordinary source did not complete without a role observation");
    };
    let SourceDiscoveryStep::Sealed(old_seal) = old_route.poll().unwrap() else {
        panic!("original ordinary source route did not complete");
    };
    assert_eq!(unchanged_seal.fingerprint(), old_seal.fingerprint());
    assert!(
        build(Digest([195; 32]))
            .with_macro_export_source_publications(&mut db)
            .is_err()
    );
    let mut discovery = build(manifest.graph.fingerprint())
        .with_macro_export_source_publications(&mut db)
        .unwrap();
    let SourceDiscoveryStep::Sealed(seal) = discovery.poll().unwrap() else {
        panic!("unchanged exported macro discovery did not complete");
    };
    discovery.publish(&mut db).unwrap();
    let view = match discovery.completed_macro_export_facet(&mut db, &facet) {
        QueryPoll::Ready(Ok(view)) => view,
        other => panic!("genuine role source seal was unavailable: {other:?}"),
    };
    assert_eq!(view.provider().module_source().selection(), &selection);
    assert_eq!(view.facet().identity(), facet.identity());
    assert_eq!(view.seal().key(), seal.key());
    assert!(view.exclusive_metadata_bytes() < 1024 * 1024);
    for dependency in &view.dependencies()[..2] {
        let crate::db::dependency::DependencyStamp::Input { key, fingerprint } = dependency else {
            panic!("selected preparation dependency was not an input");
        };
        assert!(
            seal.input_dependencies()
                .binary_search(&(key.fingerprint(), *fingerprint))
                .is_ok()
        );
    }
    assert!(matches!(view.revalidate(&mut db), QueryPoll::Ready(Ok(()))));
    assert!(
        db.input(&InputKey::CapturedSource(selection.location_identity()))
            .is_none()
    );
    assert!(
        db.input(&InputKey::SourceBlob(selection.outline.parse.content))
            .is_none()
    );
    assert!(db.prepared_source(&selection).unwrap().is_none());
    match db.fetch(
        &crate::db::provider::SyntaxProvider,
        crate::db::key::QueryKey::Outline(selection.outline.clone()),
    ) {
        QueryPoll::Ready(Err(error)) => {
            assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
            assert_eq!(error.message, "parse source absent");
        }
        other => panic!("ordinary outline did not refuse its absent runtime source: {other:?}"),
    }
    db.publish_macro_export_sources(&[]).unwrap();
    assert!(matches!(view.revalidate(&mut db), QueryPoll::Ready(Err(_))));
    drop(view);
    assert!(
        discovery
            .with_macro_export_source_publications(&mut db)
            .is_err()
    );
}

/// Required source-publication frontier, not an expected-refusal compatibility
/// oracle. If the unchanged complete source cannot seal, this test must fail
/// and retain its actual unresolved producer/namespace diagnostic.
#[test]
fn unchanged_full_core_macro_source_requires_real_annotation_closure_and_publication() {
    let package = fixture_root().join("library/core");
    let path = package.join("src/macros/mod.rs");
    let (mut manifest, unit) = manifest(&path, CORE, 2024, &package);
    let root = package.join("src/lib.rs").to_str().unwrap().to_owned();
    manifest.inputs.source_files.insert(
        root.clone(),
        Some(Digest::of_bytes(include_bytes!(
            "../../../../../fixtures/upstream/rustc-checkout/library/core/src/lib.rs"
        ))),
    );
    manifest.graph.units.get_mut(&unit).unwrap().source_path = root;
    let (mut source, mut selection) = input(unit, "src/macros/mod.rs", CORE, 2024);
    source.module_path = vec!["macros".into()];
    selection.outline.module_path = source.module_path.clone();
    let inputs = PreparedSourceInputs::refresh(None, vec![source], BTreeMap::new()).unwrap();
    let source = select(&inputs, &selection);
    let facet = Arc::new(
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), source.share())
            .unwrap(),
    );
    assert_eq!(facet.source().captured_macro_blob().unwrap().bytes(), CORE);
    let environment =
        Arc::new(FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit]).unwrap());
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([
        (
            InputKey::SourceDependencies,
            InputValue::SourceDependencies(source.source_dependencies().unwrap()),
        ),
        (
            InputKey::UnitCompileEnv(unit),
            InputValue::UnitCompileEnv(Arc::clone(&environment)),
        ),
    ]))
    .unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.publish_macro_export_sources(&[Arc::clone(&facet)])
        .unwrap();
    let compiler = Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &source.selection().outline.parse.profile,
        )
        .unwrap(),
    );
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Macro,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        environment.fingerprint(),
        crate::expand::source_declarative::engine_version(),
        compiler,
    )
    .unwrap()
    .with_macro_export_source_publications(&mut db)
    .unwrap();
    match discovery.poll().unwrap() {
        SourceDiscoveryStep::Sealed(_) => {}
        SourceDiscoveryStep::Incomplete {
            diagnostic,
            blocked,
        } => panic!(
            "unchanged complete core source has a real source-publication frontier: {}: {}; {} blocked producers",
            diagnostic.code,
            diagnostic.message,
            blocked.len(),
        ),
        _ => panic!("unchanged complete core source requires additional genuine producer inputs"),
    }
    discovery.publish(&mut db).unwrap();
    let view = match discovery.completed_macro_export_facet(&mut db, &facet) {
        QueryPoll::Ready(Ok(view)) => view,
        other => panic!("full core source closure lacks its genuine published seal: {other:?}"),
    };
    let equality = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "assert_eq")
        .unwrap();
    assert!(
        view.provider()
            .macro_index()
            .definition(equality.def_key.fingerprint())
            .is_some()
    );
    assert!(matches!(view.revalidate(&mut db), QueryPoll::Ready(Ok(()))));
    assert!(
        db.input(&InputKey::CapturedSource(selection.location_identity()))
            .is_none()
    );
    assert!(
        db.input(&InputKey::SourceBlob(selection.outline.parse.content))
            .is_none()
    );
    assert!(db.prepared_source(&selection).unwrap().is_none());
}
