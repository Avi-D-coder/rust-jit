//! Typed preparation follows the SAME original source/compiler route worklist.
//! These source controls do not claim complete SDK or guest macro execution.
use super::*;
use crate::{
    expand::source_discovery::{SourceCompilerPreparation, SourceDiscoveryStep},
    frontend::compile_input_catalog::CompileInputCatalog,
};
#[test]
fn actual_core_missing_module_is_a_repeatable_declaration_demand_before_installation() {
    let (mut db, facet, mut discovery) = root();
    let compiler = Arc::new(
        CompileInputCatalog::standalone(&facet.source().selection().outline.parse.profile).unwrap(),
    );
    let before = db.revision();
    let mut previous = None;
    let mut strict_expected = None;
    for _ in 0..2 {
        let (next, demand) = match discovery
            .prepare_with_declarative_compiler(
                facet.environment(),
                crate::expand::source_declarative::engine_version(),
                Arc::clone(&compiler),
            )
            .unwrap()
        {
            SourceCompilerPreparation::NeedModule { discovery, demand } => (discovery, demand),
            SourceCompilerPreparation::Ready(_) => {
                panic!("actual missing core modules became a completed namespace")
            }
        };
        let declaration = demand
            .declaration(next.unresolved_ordinary_module_sources())
            .unwrap()
            .expect("actual unresolved target has a retained original declaration");
        let producer = declaration.producer();
        assert_eq!(declaration.source().selection(), facet.source().selection());
        assert_eq!(
            next.original_module_declaration(producer)
                .unwrap()
                .fingerprint(),
            declaration.fingerprint()
        );
        let selected = (demand.import.identity, demand.missing.clone(), producer);
        if let Some(old) = previous.replace(selected.clone()) {
            assert_eq!(old, selected);
        }
        strict_expected = Some(demand.diagnostic());
        assert!(matches!(
            next.prepare_original_module_source(
                &mut db,
                producer,
                &module_adoption_tests::authorities(&facet, facet.preparation()),
            ),
            QueryPoll::Ready(Ok(InputRequest::CaptureModule { .. }))
        ));
        assert_eq!(db.revision(), before);
        discovery = next;
    }
    let expected = strict_expected.unwrap();
    match discovery.with_declarative_compiler(
        facet.environment(),
        crate::expand::source_declarative::engine_version(),
        compiler,
    ) {
        Err(error) => {
            assert_eq!(error.code, expected.code);
            assert_eq!(error.message, expected.message);
        }
        Ok(_) => panic!("strict installer admitted a missing-module graph"),
    }
}
#[test]
fn unchanged_complete_auxiliary_uses_same_ready_graph_as_strict_constructor() {
    let package = fixture_root().join("tests/ui/macros/auxiliary");
    let path = package.join("foreign-crate-macro-pat.rs");
    let (manifest, unit) = manifest(&path, AUX, 2018, &package);
    let (input, selection) = input(unit, "foreign-crate-macro-pat.rs", AUX, 2018);
    let inputs = PreparedSourceInputs::refresh(None, vec![input], BTreeMap::new()).unwrap();
    let source = select(&inputs, &selection);
    let facet = Arc::new(
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), source.share())
            .unwrap(),
    );
    let mut db = Database::default();
    role_tests::publish(&mut db, &manifest, unit, &[Arc::clone(&facet)]);
    let build = |db: &mut Database| {
        SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Macro,
            manifest.graph.fingerprint(),
            1,
        )
        .unwrap()
        .with_macro_export_source_publications(db)
        .unwrap()
    };
    let compiler =
        Arc::new(CompileInputCatalog::standalone(&selection.outline.parse.profile).unwrap());
    let mut ready = match build(&mut db)
        .prepare_with_declarative_compiler(
            facet.environment(),
            crate::expand::source_declarative::engine_version(),
            Arc::clone(&compiler),
        )
        .unwrap()
    {
        SourceCompilerPreparation::Ready(discovery) => discovery,
        SourceCompilerPreparation::NeedModule { .. } => {
            panic!("complete actual auxiliary unexpectedly demanded a physical child")
        }
    };
    let mut strict = build(&mut db)
        .with_declarative_compiler(
            facet.environment(),
            crate::expand::source_declarative::engine_version(),
            compiler,
        )
        .unwrap();
    let ready_seal = match ready.poll().unwrap() {
        SourceDiscoveryStep::Sealed(seal) => seal,
        _ => panic!("actual Ready graph did not close the unchanged auxiliary"),
    };
    let strict_seal = match strict.poll().unwrap() {
        SourceDiscoveryStep::Sealed(seal) => seal,
        _ => panic!("strict actual graph did not close the unchanged auxiliary"),
    };
    assert_eq!(ready_seal, strict_seal);
    assert!(
        db.input(&InputKey::CapturedSource(selection.location_identity()))
            .is_none()
    );
}
