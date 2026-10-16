//! Real pre-route demands from unchanged core root. No SDK closure or runtime
//! execution is claimed; ordinary source grants remain unavailable to role files.
use super::*;
use crate::{
    db::{
        dependency::InputKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
    },
    expand::source_discovery::SourceDiscovery,
    source::manifest::Namespace,
};
fn root() -> (Database, Arc<SourceMacroExportFacet>, SourceDiscovery) {
    let (manifest, unit, inputs, selection, _) = role_tests::core_sources();
    let source = select(&inputs, &selection);
    let facet = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            manifest.graph.units[&unit].source_path.as_str(),
            source.share(),
        )
        .unwrap(),
    );
    let mut db = Database::default();
    role_tests::publish(&mut db, &manifest, unit, &[Arc::clone(&facet)]);
    let d = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Macro,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_macro_export_source_publications(&mut db)
    .unwrap();
    (db, facet, d)
}
#[test]
fn unchanged_core_prelude_yields_real_capture_demand_before_compiler_routes() {
    let (mut db, facet, mut d) = root();
    let declaration = d
        .unresolved_ordinary_module_sources()
        .find(|declaration| declaration.request().name == "prelude")
        .unwrap();
    let producer = declaration.producer();
    assert_eq!(declaration.scope().module_path, ["prelude"]);
    assert_eq!(
        d.original_module_declaration(producer)
            .unwrap()
            .fingerprint(),
        declaration.fingerprint()
    );
    assert!(
        d.original_module_parent_adoption(producer)
            .unwrap()
            .is_none()
    );
    let before = db.revision();
    let input = match d.prepare_original_module_source(
        &mut db,
        producer,
        &module_adoption_tests::authorities(&facet, facet.preparation()),
    ) {
        QueryPoll::Ready(Ok(input)) => input,
        other => {
            panic!("actual original prelude declaration did not yield capture demand: {other:?}")
        }
    };
    let InputRequest::CaptureModule {
        request,
        compile_environment,
    } = &input
    else {
        panic!("not shared module capture");
    };
    assert_eq!(request.producer(), producer);
    assert_eq!(*compile_environment, facet.environment());
    assert_eq!(request.declaration().name, "prelude");
    assert!(
        request
            .candidates()
            .iter()
            .any(|candidate| candidate.path().as_str().ends_with("src/prelude/mod.rs"))
    );
    assert_eq!(db.revision(), before);
    assert!(
        db.input(&InputKey::ModuleRecord(input.fingerprint()))
            .is_none()
    );
    assert!(
        db.input(&InputKey::SourceBlob(
            facet.source().selection().outline.parse.content
        ))
        .is_none()
    );
    match d.stage_ordinary_module_source(&mut db, &input) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-MACRO-EXPORT-PUBLICATION"),
        _ => panic!("macro-only capture demand acquired ordinary module syntax authority"),
    }
}
#[test]
fn original_role_module_demand_rejects_wrong_preparation_and_missing_role() {
    let (mut db, facet, d) = root();
    let producer = d
        .unresolved_ordinary_module_sources()
        .find(|row| row.request().name == "prelude")
        .unwrap()
        .producer();
    assert!(matches!(
        d.prepare_original_module_source(
            &mut db,
            producer,
            &module_adoption_tests::authorities(&facet, Digest([233; 32]))
        ),
        QueryPoll::Ready(Err(_))
    ));
    assert!(d.original_module_declaration(Digest([234; 32])).is_err());
    db.publish_macro_export_sources(&[]).unwrap();
    assert!(matches!(
        d.prepare_original_module_source(
            &mut db,
            producer,
            &module_adoption_tests::authorities(&facet, facet.preparation())
        ),
        QueryPoll::Ready(Err(_))
    ));
}

#[path = "compiler_module_preparation_tests.rs"]
mod compiler_module_preparation_tests;
