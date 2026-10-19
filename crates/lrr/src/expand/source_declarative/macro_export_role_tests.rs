//! Preparation/authority controls over unchanged official core source. These
//! tests grant no adopted SDK, completed export closure or guest execution.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    },
    expand::{builtins::FrozenCompileEnvironment, source_discovery::SourceDiscovery},
    frontend::compile_input_catalog::CompileInputCatalog,
    source::manifest::Namespace,
};
pub(super) const ROOT: &[u8] =
    include_bytes!("../../../../../fixtures/upstream/rustc-checkout/library/core/src/lib.rs");
pub(super) fn core_sources() -> (
    ToolManifest,
    Digest,
    PreparedSourceInputs,
    SourceSelection,
    SourceSelection,
) {
    let package = fixture_root().join("library/core");
    let root_path = package.join("src/lib.rs");
    let child_path = package.join("src/macros/mod.rs");
    let (mut manifest, unit) = manifest(&root_path, ROOT, 2024, &package);
    manifest.inputs.source_files.insert(
        child_path.to_str().unwrap().into(),
        Some(Digest::of_bytes(CORE)),
    );
    let (root, root_selection) = input(unit, "src/lib.rs", ROOT, 2024);
    let (mut child, mut child_selection) = input(unit, "src/macros/mod.rs", CORE, 2024);
    child.module_path = vec!["macros".into()];
    child_selection.outline.module_path = child.module_path.clone();
    let inputs = PreparedSourceInputs::refresh(None, vec![root, child], BTreeMap::new()).unwrap();
    (manifest, unit, inputs, root_selection, child_selection)
}
pub(super) fn publish(
    db: &mut Database,
    manifest: &ToolManifest,
    unit: Digest,
    facets: &[Arc<SourceMacroExportFacet>],
) {
    db.publish_inputs(BTreeMap::from([(
        InputKey::UnitCompileEnv(unit),
        InputValue::UnitCompileEnv(Arc::new(
            FrozenCompileEnvironment::new(unit, &manifest.compile_env[&unit]).unwrap(),
        )),
    )]))
    .unwrap();
    db.publish_macro_export_sources(facets).unwrap();
}
#[test]
fn no_core_suppresses_library_injection_but_preserves_explicit_extern_prelude() {
    let (manifest, unit, inputs, root_selection, child_selection) = core_sources();
    let root = select(&inputs, &root_selection);
    assert_eq!(root.captured_macro_blob().unwrap().bytes(), ROOT);
    let facts = root.macro_prelude_facts().unwrap();
    assert!(facts.no_core);
    assert!(!facts.standard);
    assert!(!facts.disabled.contains(&vec![]));
    let child = select(&inputs, &child_selection);
    let inherited = child.macro_prelude_policy().unwrap();
    assert!(inherited.no_core && !inherited.standard);
    assert!(!inherited.disabled.contains(&vec![]));
    let ordinary_policy = root.macro_prelude_policy().unwrap();
    assert!(ordinary_policy.no_core && !ordinary_policy.standard);
    assert!(!ordinary_policy.disabled.contains(&vec![]));
    let mut ordinary = SourceDeclarative::new(root.share()).unwrap();
    let compiler =
        Arc::new(CompileInputCatalog::standalone(&root_selection.outline.parse.profile).unwrap());
    ordinary.compiler = Some(Arc::clone(&compiler));
    assert!(compiler.injected_import(&root).unwrap().is_none());
    let (implicit, available) = ordinary.route_dependency_sets().unwrap();
    assert!(implicit.is_empty(), "no_core must not inject compiler crate dependencies");
    assert!(!available.is_empty(), "explicit extern crate routes remain available");
    let package = fixture_root().join("library/core");
    let facet = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            package.join("src/lib.rs").to_str().unwrap(),
            root.share(),
        )
        .unwrap(),
    );
    let mut db = Database::default();
    publish(&mut db, &manifest, unit, &[facet]);
    let observation = Arc::new(
        db.macro_export_source_observation(&root_selection, manifest.graph.fingerprint())
            .unwrap()
            .unwrap(),
    );
    assert!(
        observation
            .require_source(&root, Digest([251; 32]))
            .is_err()
    );
    let mut provider = SourceDeclarative::new(root.share()).unwrap();
    provider
        .bind_macro_export_source(Arc::clone(&observation), manifest.graph.fingerprint())
        .unwrap();
    provider.compiler = Some(compiler);
    let (dependencies, available) = provider.route_dependency_sets().unwrap();
    assert!(dependencies.is_empty());
    assert!(available.is_empty());
    let routes = provider.routes_in(&[]).unwrap();
    assert!(
        !routes[0]
            .implicit_disabled
            .contains(&root.outline().root_scope)
    );
    assert!(
        provider
            .compiler
            .as_ref()
            .unwrap()
            .attribute_catalog()
            .is_some()
    );
    assert!(
        provider
            .compiler
            .as_ref()
            .unwrap()
            .canonical_standalone_member(&root_selection.outline.parse.profile)
            .unwrap()
            .is_some()
    );
    let error = match provider.completed_runtime_index(&[], None, 64 * 1024 * 1024) {
        Err(error) => error,
        Ok(_) => panic!("macro source granted live runtime index"),
    };
    assert_eq!(error.code, "LRR-E-MACRO-EXPORT-PUBLICATION");
    assert!(
        ordinary
            .bind_macro_export_source(observation, manifest.graph.fingerprint())
            .is_err()
    );
    assert!(db.prepared_source(&root_selection).unwrap().is_none());
    assert!(
        db.input(&InputKey::SourceBlob(root_selection.outline.parse.content))
            .is_none()
    );
}
#[test]
fn configured_root_cannot_be_replaced_by_actual_macros_child() {
    let (manifest, unit, inputs, _, child_selection) = core_sources();
    let child = select(&inputs, &child_selection);
    let package = fixture_root().join("library/core");
    let path = package.join("src/macros/mod.rs");
    assert!(
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), child).is_ok()
    );
    // Same original bytes and genuine record, but an empty module path claims
    // root identity. The configured Cargo source root is actually lib.rs.
    let (pretend_root, selection) = input(unit, "src/macros/mod.rs", CORE, 2024);
    let inputs = PreparedSourceInputs::refresh(None, vec![pretend_root], BTreeMap::new()).unwrap();
    let error = match SourceMacroExportFacet::capture(
        &manifest,
        unit,
        path.to_str().unwrap(),
        select(&inputs, &selection),
    ) {
        Err(error) => error,
        Ok(_) => panic!("child source was admitted as configured root"),
    };
    assert_eq!(error.code, "LRR-E-MACRO-EXPORT-PREPARATION");
    assert_eq!(
        error.message,
        "macro-export crate root differs from configured Cargo source root"
    );
}
#[test]
fn same_unit_source_requires_its_own_observation_before_compiler_configuration() {
    let (manifest, unit, inputs, root_selection, child_selection) = core_sources();
    let root = select(&inputs, &root_selection);
    let child = select(&inputs, &child_selection);
    let package = fixture_root().join("library/core");
    let root_facet = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            package.join("src/lib.rs").to_str().unwrap(),
            root.share(),
        )
        .unwrap(),
    );
    let mut db = Database::default();
    publish(&mut db, &manifest, unit, &[root_facet]);
    let error = match SourceDiscovery::new(
        vec![root, child.share()],
        child.outline().root_scope.clone(),
        Namespace::Macro,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_macro_export_source_publications(&mut db)
    {
        Err(error) => error,
        Ok(_) => panic!("unit identity filled missing child source authority"),
    };
    assert_eq!(
        error.message,
        "selected macro-export unit source lacks its own role observation"
    );
}

#[test]
fn same_scope_recorded_candidate_cannot_replace_adopted_parent_selection() {
    let (mut manifest, unit, _, _, _) = core_sources();
    let package = fixture_root().join("library/core");
    let unrelated_path = package.join("src/not-the-declared-module.rs");
    manifest.inputs.source_files.insert(
        unrelated_path.to_str().unwrap().into(),
        Some(Digest::of_bytes(CORE)),
    );
    let (root_input, root_selection) = input(unit, "src/lib.rs", ROOT, 2024);
    let (mut child_input, mut child_selection) =
        input(unit, "src/not-the-declared-module.rs", CORE, 2024);
    child_input.module_path = vec!["macros".into()];
    child_selection.outline.module_path = child_input.module_path.clone();
    let inputs =
        PreparedSourceInputs::refresh(None, vec![root_input, child_input], BTreeMap::new())
            .unwrap();
    let root = select(&inputs, &root_selection);
    let child = select(&inputs, &child_selection);
    let root_facet = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            package.join("src/lib.rs").to_str().unwrap(),
            root.share(),
        )
        .unwrap(),
    );
    // A nonempty scope remains a raw candidate, including a recorded file with
    // actual licensed bytes. It does not prove the parent's physical selection.
    let child_facet = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            unrelated_path.to_str().unwrap(),
            child.share(),
        )
        .unwrap(),
    );
    let mut db = Database::default();
    publish(&mut db, &manifest, unit, &[root_facet, child_facet]);
    let compiler =
        Arc::new(CompileInputCatalog::standalone(&root_selection.outline.parse.profile).unwrap());
    let error = match SourceDiscovery::new(
        vec![root, child.share()],
        child.outline().root_scope.clone(),
        Namespace::Macro,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap()
    .with_macro_export_source_publications(&mut db)
    .unwrap()
    .with_declarative_compiler(
        Digest([252; 32]),
        crate::expand::source_declarative::engine_version(),
        compiler,
    ) {
        Err(error) => error,
        Ok(_) => panic!("module_path was promoted into file-selection authority"),
    };
    assert_eq!(error.code, "LRR-E-MACRO-EXPORT-PUBLICATION");
    assert_eq!(
        error.message,
        "macro-export child requires adopted source-parent selection"
    );
}
