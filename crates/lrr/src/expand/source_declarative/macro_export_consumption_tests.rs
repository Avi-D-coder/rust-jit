//! Actual configured role-child attachment, annotation closure and full seal
//! controls over unchanged official module sources; no guest execution claim.
use super::*;
use crate::{
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        memo::Database,
        poll::QueryPoll,
        seals::DiscoveryInput,
    },
    expand::{
        builtins::FrozenCompileEnvironment,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{
        compile_input_catalog::CompileInputCatalog,
        source_module_declarations::SourceModuleDeclaration,
    },
    source::{
        compiler_replay::CompilerReplaySource,
        macro_export_module::{MacroExportModuleAdoption, MacroExportModuleRequest},
        manifest::Namespace,
    },
};
const MODULE_ROOT: &[u8] = include_bytes!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/modules/special_module_name_ignore.rs"
);
const MODULE_CHILD: &[u8] = include_bytes!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/modules/auxiliary/dummy_lib.rs"
);
pub(super) fn prepared(
    edition: u16,
) -> (
    Database,
    Vec<Arc<SourceMacroExportFacet>>,
    Vec<Arc<MacroExportModuleAdoption>>,
) {
    let package = fixture_root().join("tests/ui/modules");
    let root_path = package.join("special_module_name_ignore.rs");
    let child_path = package.join("auxiliary/dummy_lib.rs");
    let (mut manifest, unit) = manifest(&root_path, MODULE_ROOT, edition, &package);
    manifest.inputs.source_files.insert(
        child_path.to_str().unwrap().into(),
        Some(Digest::of_bytes(MODULE_CHILD)),
    );
    let (root, root_selection) = input(unit, "special_module_name_ignore.rs", MODULE_ROOT, edition);
    let mut units = vec![root];
    let mut selections = vec![root_selection];
    for name in ["lib", "main"] {
        let (mut child, mut selection) =
            input(unit, "auxiliary/dummy_lib.rs", MODULE_CHILD, edition);
        child.module_path = vec![name.into()];
        selection.outline.module_path = child.module_path.clone();
        units.push(child);
        selections.push(selection);
    }
    let inputs = PreparedSourceInputs::refresh(None, units, BTreeMap::new()).unwrap();
    let facets = selections
        .iter()
        .enumerate()
        .map(|(ordinal, selection)| {
            Arc::new(
                SourceMacroExportFacet::capture(
                    &manifest,
                    unit,
                    if ordinal == 0 {
                        root_path.to_str().unwrap()
                    } else {
                        child_path.to_str().unwrap()
                    },
                    select(&inputs, selection),
                )
                .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let environment =
        Arc::new(FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit]).unwrap());
    let mut db = Database::default();
    db.publish_inputs(BTreeMap::from([
        (
            InputKey::UnitCompileEnv(unit),
            InputValue::UnitCompileEnv(environment),
        ),
        (
            InputKey::SourceDependencies,
            InputValue::SourceDependencies(facets[0].source().source_dependencies().unwrap()),
        ),
    ]))
    .unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.publish_macro_export_sources(&facets).unwrap();
    let declarations = SourceModuleDeclaration::from_captured(facets[0].source()).unwrap();
    let mut modules = vec![];
    for child in &facets[1..] {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.scope() == &child.source().outline().root_scope)
            .unwrap()
            .share();
        let request = MacroExportModuleRequest::prepare(
            &mut db,
            Arc::clone(&facets[0]),
            None,
            Arc::clone(child),
            declaration,
            &super::module_adoption_tests::authorities(&facets[0], facets[0].preparation()),
        )
        .unwrap();
        modules.push(super::module_adoption_tests::adopt(&mut db, &request).unwrap());
    }
    (db, facets, modules)
}
fn selected(db: &mut Database, facets: &[Arc<SourceMacroExportFacet>]) -> SourceDiscovery {
    SourceDiscovery::new(
        facets.iter().map(|facet| facet.source().share()).collect(),
        facets[1].source().outline().root_scope.clone(),
        Namespace::Macro,
        facets[0].unit_graph(),
        1,
    )
    .unwrap()
    .with_macro_export_source_publications(db)
    .unwrap()
}
fn configure(
    discovery: SourceDiscovery,
    root: &SourceMacroExportFacet,
) -> Result<SourceDiscovery, crate::core::diagnostic::Diagnostic> {
    discovery.with_declarative_compiler(
        root.environment(),
        crate::expand::source_declarative::engine_version(),
        Arc::new(
            CompileInputCatalog::standalone(&root.source().selection().outline.parse.profile)
                .unwrap(),
        ),
    )
}
pub(super) fn completed(
    edition: u16,
) -> (
    Database,
    SourceDiscovery,
    Vec<Arc<SourceMacroExportFacet>>,
    Vec<Arc<MacroExportModuleAdoption>>,
) {
    let (mut db, facets, modules) = prepared(edition);
    let selected = selected(&mut db, &facets)
        .with_macro_export_module_adoptions(&mut db, &modules)
        .unwrap();
    assert_eq!(selected.macro_export_module_roots().count(), 2);
    assert!(selected.macro_export_module_admission_bytes() <= 320 * 1024 * 1024);
    let mut discovery = configure(selected, &facets[0]).unwrap();
    match discovery.poll().unwrap() {
        SourceDiscoveryStep::Sealed(_) => {}
        SourceDiscoveryStep::Incomplete {
            diagnostic,
            blocked,
        } => panic!(
            "unchanged official module source has real closure frontier {}: {}; {} blocked producers",
            diagnostic.code,
            diagnostic.message,
            blocked.len()
        ),
        _ => panic!("unchanged official module source requires additional genuine producer input"),
    }
    discovery.publish(&mut db).unwrap();
    (db, discovery, facets, modules)
}
fn published(db: &mut Database, discovery: &SourceDiscovery) -> DiscoveryInput {
    let value = db
        .input(&InputKey::DiscoverySeal(discovery.key().clone()))
        .unwrap();
    let InputValue::DiscoverySeal(input) = value.as_ref() else {
        panic!("genuine published discovery missing")
    };
    input.as_ref().clone()
}
#[test]
fn unchanged_module_sources_attach_actual_adoptions_publish_seal_and_deny_runtime_grants() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, discovery, facets, modules) = completed(edition);
        let input = published(&mut db, &discovery);
        for module in &modules {
            module.validate_discovery(&mut db, &input).unwrap();
            for dependency in module
                .request()
                .role_dependencies()
                .chain(std::iter::once(module.module_dependency()))
            {
                let DependencyStamp::Input { key, fingerprint } = dependency else {
                    panic!("wrong dependency kind")
                };
                assert_eq!(input.compiler_inputs.get(key), Some(fingerprint));
                assert!(
                    input
                        .seal
                        .input_dependencies()
                        .binary_search(&(key.fingerprint(), *fingerprint))
                        .is_ok()
                );
            }
        }
        let view = match discovery.completed_macro_export_facet(&mut db, &facets[1]) {
            QueryPoll::Ready(Ok(view)) => view,
            other => panic!("actual child fullseal view missing: {other:?}"),
        };
        assert_eq!(
            view.provider()
                .macro_export_module_adoption()
                .unwrap()
                .identity(),
            modules[0].identity()
        );
        assert!(matches!(view.revalidate(&mut db), QueryPoll::Ready(Ok(()))));
        assert!(discovery.completed_runtime_macro_indexes().is_err());
        assert!(discovery.completed_runtime_macro_namespace().is_err());
        for facet in &facets {
            let selection = facet.source().selection();
            assert!(
                db.input(&InputKey::SourceBlob(selection.outline.parse.content))
                    .is_none()
            );
            assert!(
                db.input(&InputKey::CapturedSource(selection.location_identity()))
                    .is_none()
            );
            assert!(db.prepared_source(selection).unwrap().is_none());
            for item in &facet.source().outline().items {
                assert!(
                    db.input(&InputKey::DefinitionHeader(item.def_key.fingerprint()))
                        .is_none()
                );
            }
        }
    }
}
#[test]
fn production_builder_requires_complete_unique_current_adopted_sources() {
    let (mut db, facets, modules) = prepared(2015);
    assert!(
        selected(&mut db, &facets)
            .with_macro_export_module_adoptions(&mut db, &modules[..1])
            .is_err()
    );
    assert!(
        selected(&mut db, &facets)
            .with_macro_export_module_adoptions(
                &mut db,
                &[Arc::clone(&modules[0]), Arc::clone(&modules[0])]
            )
            .is_err()
    );
    assert!(
        selected(&mut db, &facets[..2])
            .with_macro_export_module_adoptions(&mut db, &modules)
            .is_err()
    );
    let unbound = selected(&mut db, &facets);
    assert!(configure(unbound, &facets[0]).is_err());
    let bound = selected(&mut db, &facets)
        .with_macro_export_module_adoptions(&mut db, &modules)
        .unwrap();
    let bound = bound
        .with_macro_export_module_adoptions(&mut db, &modules)
        .unwrap();
    assert_eq!(bound.macro_export_module_roots().count(), 2);
    assert!(
        configure(bound, &facets[0])
            .unwrap()
            .with_macro_export_module_adoptions(&mut db, &modules)
            .is_err()
    );
    db.publish_macro_export_sources(&[]).unwrap();
    assert!(
        selected(&mut db, &facets)
            .with_macro_export_module_adoptions(&mut db, &modules)
            .is_err()
    );
}
#[test]
fn actual_publication_rejects_missing_duplicate_unselected_or_unbound_module_dependencies() {
    let (mut db, discovery, facets, modules) = completed(2015);
    let input = published(&mut db, &discovery);
    let scans = facets
        .iter()
        .map(|facet| {
            facet
                .source()
                .declaration_inventory(Default::default())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let replay = facets
        .iter()
        .zip(&scans)
        .map(|(facet, scan)| CompilerReplaySource::prepare(facet.source(), scan).unwrap())
        .collect::<Vec<_>>();
    let mut records = discovery
        .sealed_expansions()
        .unwrap()
        .cloned()
        .collect::<Vec<_>>();
    records.extend(discovery.sealed_declaration_expansions().unwrap());
    let revision = db.revision();
    assert!(
        db.publish_discovery(
            input.clone(),
            &records,
            &[],
            &[],
            &replay,
            &modules[..1],
            BTreeMap::new()
        )
        .is_err()
    );
    assert_eq!(db.revision(), revision);
    assert!(
        db.publish_discovery(
            input.clone(),
            &records,
            &[],
            &[],
            &replay,
            &[
                Arc::clone(&modules[0]),
                Arc::clone(&modules[0]),
                Arc::clone(&modules[1])
            ],
            BTreeMap::new()
        )
        .is_err()
    );
    assert_eq!(db.revision(), revision);
    let mut missing_source = input.clone();
    missing_source
        .sources
        .retain(|source| source != facets[2].source().selection());
    assert!(
        db.publish_discovery(
            missing_source,
            &records,
            &[],
            &[],
            &replay,
            &modules,
            BTreeMap::new()
        )
        .is_err()
    );
    let mut missing_stamp = input.clone();
    let DependencyStamp::Input { key, .. } = modules[0].module_dependency() else {
        panic!("wrong dependency kind")
    };
    missing_stamp.compiler_inputs.remove(key);
    assert!(
        modules[0]
            .validate_discovery(&mut db, &missing_stamp)
            .is_err()
    );
    assert!(
        db.publish_discovery(
            missing_stamp,
            &records,
            &[],
            &[],
            &replay,
            &modules,
            BTreeMap::new()
        )
        .is_err()
    );
    assert_eq!(db.revision(), revision);
    db.publish_macro_export_sources(&[]).unwrap();
    assert!(modules[0].validate_discovery(&mut db, &input).is_err());
    // Restore exactly the current source/env roles through the coordinator,
    // while the genuine base-publication reset removes adopted ModuleRecords.
    let environment = db
        .input(&InputKey::UnitCompileEnv(facets[0].unit()))
        .unwrap();
    db.publish_inputs(BTreeMap::from([
        (
            InputKey::UnitCompileEnv(facets[0].unit()),
            environment.as_ref().clone(),
        ),
        (
            InputKey::SourceDependencies,
            InputValue::SourceDependencies(facets[0].source().source_dependencies().unwrap()),
        ),
    ]))
    .unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.publish_macro_export_sources(&facets).unwrap();
    let error = modules[0].revalidate(&mut db).unwrap_err();
    assert_eq!(
        error.message,
        "macro-export module adoption is no longer published"
    );
    assert!(modules[0].validate_discovery(&mut db, &input).is_err());
}
