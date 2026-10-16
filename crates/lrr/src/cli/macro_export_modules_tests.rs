//! Source/adoption controls around unchanged official physical-module files.
//! Empty main and successful preparation do not establish macro guest execution.
use super::*;
use crate::{
    cli::macro_export_modules::{ConfiguredMacroExportSources, MacroExportModulePreparation},
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    },
    frontend::compile_input_catalog::CompileInputCatalog,
};
const ROOT_MODULES: &[u8] = include_bytes!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/modules/special_module_name_ignore.rs"
);
const CHILD_MODULE: &[u8] = include_bytes!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/modules/auxiliary/dummy_lib.rs"
);
fn configured(
    child: Option<Digest>,
) -> (
    Database,
    ToolManifest,
    ConfiguredMacroExportSources,
    Arc<CompileInputCatalog>,
) {
    let package = fixture_root().join("tests/ui/modules");
    let path = package.join("special_module_name_ignore.rs");
    let (mut manifest, unit) = manifest(&path, ROOT_MODULES, 2015, &package);
    if let Some(content) = child {
        manifest.inputs.source_files.insert(
            package
                .join("auxiliary/dummy_lib.rs")
                .to_str()
                .unwrap()
                .into(),
            Some(content),
        );
    }
    let (input, selection) = input(unit, "special_module_name_ignore.rs", ROOT_MODULES, 2015);
    let prepared = PreparedSourceInputs::refresh(None, vec![input], BTreeMap::new()).unwrap();
    let source = select(&prepared, &selection);
    let scope = source.outline().root_scope.clone();
    let facet = Arc::new(
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), source).unwrap(),
    );
    let mut db = Database::default();
    role_tests::publish(&mut db, &manifest, unit, &[Arc::clone(&facet)]);
    db.begin_expansion_epoch(1).unwrap();
    let state =
        ConfiguredMacroExportSources::new(&manifest, prepared, vec![facet], vec![], scope, 1)
            .unwrap();
    let compiler =
        Arc::new(CompileInputCatalog::standalone(&selection.outline.parse.profile).unwrap());
    (db, manifest, state, compiler)
}
#[test]
fn actual_module_candidates_capture_refresh_and_adopt_in_one_current_role_publication() {
    let (mut db, manifest, state, compiler) = configured(Some(Digest::of_bytes(CHILD_MODULE)));
    let (discovery, sources) = match state.prepare(&mut db, &manifest, compiler).unwrap() {
        MacroExportModulePreparation::Ready { discovery, sources } => (discovery, sources),
        _ => panic!("genuinely recorded original modules did not complete source preparation"),
    };
    assert_eq!(sources.facets().len(), 3);
    assert_eq!(sources.modules().len(), 2);
    assert_eq!(discovery.unresolved_ordinary_module_sources().count(), 0);
    let source = sources.facets()[0].source();
    for facet in sources.facets() {
        assert!(source.same_publication(facet.source()));
        assert!(
            db.input(&InputKey::CapturedSource(
                facet.source().selection().location_identity()
            ))
            .is_none()
        );
        assert!(
            db.input(&InputKey::SourceBlob(
                facet.source().selection().outline.parse.content
            ))
            .is_none()
        );
        assert!(matches!(
            db.input(&InputKey::MacroExportSource(
                facet.source().selection().location_identity()
            ))
            .as_deref(),
            Some(InputValue::MacroExportSource(_))
        ));
    }
    for module in sources.modules() {
        module.revalidate(&mut db).unwrap();
        let request = module.request();
        assert!(
            request
                .parent()
                .source()
                .same_publication(request.child().source())
        );
        assert_eq!(
            request
                .child()
                .source()
                .captured_macro_blob()
                .unwrap()
                .bytes(),
            CHILD_MODULE
        );
        assert!(
            sources
                .facets()
                .iter()
                .any(|facet| Arc::ptr_eq(facet, request.parent()))
        );
        assert!(
            sources
                .facets()
                .iter()
                .any(|facet| Arc::ptr_eq(facet, request.child()))
        );
    }
    assert!(sources.retained_admission_bytes().unwrap() > CHILD_MODULE.len());
}
#[test]
fn missing_prepared_inventory_yields_real_demand_without_adoption_or_manifest_mutation() {
    let (mut db, manifest, state, compiler) = configured(None);
    let revision = db.revision();
    let identity = manifest.fingerprint();
    let (sources, demand) = match state.prepare(&mut db, &manifest, compiler).unwrap() {
        MacroExportModulePreparation::NeedPreparation { sources, demand } => (sources, demand),
        _ => panic!(
            "missing configured inventory became trusted capture or completed source closure"
        ),
    };
    assert_eq!(demand.preparation(), identity);
    assert!(demand.metadata_bytes() > std::mem::size_of_val(&demand));
    assert_eq!(
        demand.missing_paths(),
        &[fixture_root().join("tests/ui/modules/auxiliary/dummy_lib.rs")]
    );
    assert_eq!(manifest.fingerprint(), identity);
    assert_eq!(db.revision(), revision);
    assert_eq!(sources.facets().len(), 1);
    assert!(sources.modules().is_empty());
    assert!(
        db.input(&InputKey::ModuleRecord(demand.input().fingerprint()))
            .is_none()
    );
}
#[test]
fn actual_candidate_bytes_must_match_prepared_inventory_before_record_adoption() {
    let (mut db, manifest, state, compiler) = configured(Some(Digest([244; 32])));
    let revision = db.revision();
    match state.prepare(&mut db, &manifest, compiler) {
        Err(error) => assert_eq!(error.code, "LRR-E-TOOL-PREPARE-STALE"),
        Ok(_) => panic!("changed candidate bytes acquired current configured module authority"),
    }
    assert_eq!(db.revision(), revision);
}
#[test]
fn configured_source_capture_is_rejected_before_io_during_guest_run() {
    let (mut db, manifest, state, compiler) = configured(Some(Digest::of_bytes(CHILD_MODULE)));
    let revision = db.revision();
    db.begin_run().unwrap();
    match state.prepare(&mut db, &manifest, compiler) {
        Err(error) => {
            assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
            assert_eq!(error.message, "source preparation during active run");
        }
        Ok(_) => panic!("configured source capture ran during guest execution"),
    }
    assert_eq!(db.revision(), revision);
    db.finish_run().unwrap();
}
