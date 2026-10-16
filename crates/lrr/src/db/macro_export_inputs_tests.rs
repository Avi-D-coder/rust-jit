//! Role/currentness controls using the existing unchanged upstream macro input.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    },
    expand::builtins::FrozenCompileEnvironment,
};

fn captured(manifest: &ToolManifest, unit: Digest) -> Arc<SourceMacroExportFacet> {
    let path = fixture_root().join("tests/ui/macros/auxiliary/foreign-crate-macro-pat.rs");
    let (unit_source, selection) = input(unit, "foreign-crate-macro-pat.rs", AUX, 2018);
    let sources = PreparedSourceInputs::refresh_with_dependencies(
        None,
        vec![unit_source],
        BTreeMap::new(),
        vec![],
    )
    .unwrap();
    Arc::new(
        SourceMacroExportFacet::capture(
            manifest,
            unit,
            path.to_str().unwrap(),
            select(&sources, &selection),
        )
        .unwrap(),
    )
}
fn preparation_with_pair() -> (
    ToolManifest,
    Arc<SourceMacroExportFacet>,
    Arc<SourceMacroExportFacet>,
) {
    let package = fixture_root().join("tests/ui/macros/auxiliary");
    let path = package.join("foreign-crate-macro-pat.rs");
    let (mut manifest, one) = manifest(&path, AUX, 2018, &package);
    // Two configured targets may capture the same unchanged source. These are
    // constructed authority records, not evidence of an adopted SDK/Cargo run.
    let mut other = manifest.graph.units[&one].clone();
    other.key.target_name = "other_macro_target".into();
    let two = other.key.fingerprint();
    manifest.graph.units.insert(two, other);
    let mut environment = manifest.compile_env[&one].clone();
    environment.unit = two;
    manifest.compile_env.insert(two, environment);
    // The persisted planner contract selects one root. Both independent macro
    // facets are reachable dependencies of that selected configured target.
    let mut root = manifest.graph.units[&one].clone();
    root.key.role = UnitRole::Live;
    root.key.target_kind = TargetKind::Binary;
    root.key.target_name = "authority_control_root".into();
    root.dependencies = vec![
        crate::project::cargo_units::DependencyEdge {
            alias: "first".into(),
            target: one,
            public: false,
            no_prelude: false,
            no_unused: false,
        },
        crate::project::cargo_units::DependencyEdge {
            alias: "second".into(),
            target: two,
            public: false,
            no_prelude: false,
            no_unused: false,
        },
    ];
    let mut edges = StableEncoder::new("lrr/v2/cargo-unit-edges");
    edges.sequence(&root.dependencies, |encoder, edge| {
        encoder.string(&edge.alias);
        encoder.digest(edge.target);
        encoder.bool(edge.public);
        encoder.bool(edge.no_prelude);
        encoder.bool(edge.no_unused);
    });
    root.key.dependency_edges = edges.finish();
    let selected = root.key.fingerprint();
    manifest.graph.units.insert(selected, root);
    manifest.graph.roots = BTreeSet::from([selected]);
    let mut environment = manifest.compile_env[&one].clone();
    environment.unit = selected;
    manifest.compile_env.insert(selected, environment);
    manifest.validate().unwrap();
    let first = captured(&manifest, one);
    let second = captured(&manifest, two);
    (manifest, first, second)
}
fn environment_inputs(manifest: &ToolManifest) -> BTreeMap<InputKey, InputValue> {
    manifest
        .compile_env
        .iter()
        .map(|(unit, environment)| {
            (
                InputKey::UnitCompileEnv(*unit),
                InputValue::UnitCompileEnv(Arc::new(
                    FrozenCompileEnvironment::new(*unit, environment).unwrap(),
                )),
            )
        })
        .collect()
}
fn database(manifest: &ToolManifest) -> Database {
    let mut db = Database::default();
    db.publish_inputs(environment_inputs(manifest)).unwrap();
    db
}

#[test]
fn macro_export_receipts_publish_no_runtime_source_or_retained_graph() {
    let (manifest, one, two) = preparation_with_pair();
    drop(two);
    let mut db = database(&manifest);
    let source = one.source().selection().clone();
    let owners = one.source().retained_state_weak().strong_count();
    db.publish_macro_export_sources(std::slice::from_ref(&one))
        .unwrap();
    assert_eq!(one.source().retained_state_weak().strong_count(), owners);
    let dependencies = db.macro_export_source_dependencies(&one).unwrap();
    assert!(
        matches!(&dependencies[0], crate::db::dependency::DependencyStamp::Input {
        key: InputKey::MacroExportSource(location), ..
    } if *location == source.location_identity())
    );
    assert!(
        db.published_input(&InputKey::SourceBlob(source.outline.parse.content))
            .is_none()
    );
    assert!(
        db.published_input(&InputKey::CapturedSource(source.location_identity()))
            .is_none()
    );
    assert!(
        db.published_input(&InputKey::CrateCfg(one.unit()))
            .is_none()
    );
    for item in &one.source().outline().items {
        let definition = item.def_key.fingerprint();
        assert!(
            db.published_input(&InputKey::DefinitionHeader(definition))
                .is_none()
        );
        assert!(
            db.published_input(&InputKey::DefinitionSource(definition))
                .is_none()
        );
        assert!(
            db.published_input(&InputKey::DefinitionBody(definition))
                .is_none()
        );
    }
    assert!(db.prepared_source(&source).unwrap().is_none());
    assert!(db.prepared_parse(&source.outline.parse).is_none());
    assert!(db.prepared_outline(&source.outline).is_none());
    let usage = db.cache_usage().unwrap();
    assert_eq!(usage.source_input_bytes, 0);
    assert_eq!(usage.parse_bytes, 0);
    let weak = Arc::downgrade(&one);
    drop(one);
    assert!(
        weak.upgrade().is_none(),
        "scalar receipt retained a facet graph"
    );
    assert!(
        db.published_input(&InputKey::MacroExportSource(source.location_identity()))
            .is_some()
    );
}

#[test]
fn macro_export_publication_is_atomic_idempotent_and_forbidden_in_runs() {
    let (manifest, one, _) = preparation_with_pair();
    let mut db = database(&manifest);
    db.publish_macro_export_sources(std::slice::from_ref(&one))
        .unwrap();
    let revision = db.revision();
    let stamps = db.macro_export_source_dependencies(&one).unwrap();
    db.publish_macro_export_sources(std::slice::from_ref(&one))
        .unwrap();
    assert_eq!(db.revision(), revision);
    assert!(
        db.publish_macro_export_sources(&[Arc::clone(&one), Arc::clone(&one)])
            .is_err()
    );
    assert_eq!(db.revision(), revision);
    assert_eq!(db.macro_export_source_dependencies(&one).unwrap(), stamps);
    db.begin_run().unwrap();
    assert_eq!(
        db.publish_macro_export_sources(&[]).unwrap_err().code,
        "LRR-E-MACRO-EXPORT-PUBLICATION"
    );
    assert_eq!(db.revision(), revision);
    db.finish_run().unwrap();
    db.publish_macro_export_sources(&[]).unwrap();
    assert!(db.macro_export_source_dependencies(&one).is_err());
}

#[test]
fn macro_export_unrelated_receipt_does_not_change_selected_dependency_stamps() {
    let (manifest, one, two) = preparation_with_pair();
    let mut db = database(&manifest);
    db.publish_macro_export_sources(std::slice::from_ref(&one))
        .unwrap();
    let before = db.macro_export_source_dependencies(&one).unwrap();
    db.publish_macro_export_sources(&[Arc::clone(&one), two])
        .unwrap();
    assert_eq!(db.macro_export_source_dependencies(&one).unwrap(), before);
    // Selected facets explicitly add their stamps. The old global compiler
    // input enumerator must not turn every facet into every query's dependency.
    assert!(
        !db.compiler_input_stamps()
            .unwrap()
            .keys()
            .any(|key| matches!(key, InputKey::MacroExportSource(_)))
    );
}

#[test]
fn macro_export_environment_or_preparation_replacement_invalidates_old_receipt() {
    let (mut manifest, one, two) = preparation_with_pair();
    let mut db = database(&manifest);
    db.publish_macro_export_sources(std::slice::from_ref(&one))
        .unwrap();
    let old_revision = db.revision();
    manifest
        .compile_env
        .get_mut(&one.unit())
        .unwrap()
        .declared
        .insert("LRR_AUTHORITY_CONTROL".into(), Some("changed".into()));
    manifest
        .inputs
        .environment
        .insert("LRR_AUTHORITY_CONTROL".into(), Some("changed".into()));
    let replacement = captured(&manifest, one.unit());
    // Source bytes/edition/unit are identical, but actual compile environment
    // and immutable preparation identity differ. No source-only rebinding.
    assert_eq!(replacement.source().selection(), one.source().selection());
    assert!(
        db.publish_macro_export_sources(std::slice::from_ref(&replacement))
            .is_err()
    );
    assert_eq!(db.revision(), old_revision);
    assert!(db.macro_export_source_dependencies(&one).is_ok());
    db.publish_inputs(environment_inputs(&manifest)).unwrap();
    assert!(db.macro_export_source_dependencies(&one).is_err());
    assert!(
        db.publish_macro_export_sources(&[replacement.clone(), two])
            .is_err()
    );
    db.publish_macro_export_sources(std::slice::from_ref(&replacement))
        .unwrap();
    assert!(db.macro_export_source_dependencies(&one).is_err());
    assert!(db.macro_export_source_dependencies(&replacement).is_ok());
    // Generic source publication cannot inject the derived opaque receipt.
    let key = InputKey::MacroExportSource(replacement.source().selection().location_identity());
    let value = db.input(&key).unwrap().as_ref().clone();
    let revision = db.revision();
    assert!(db.publish_inputs(BTreeMap::from([(key, value)])).is_err());
    assert_eq!(db.revision(), revision);
    assert!(db.macro_export_source_dependencies(&replacement).is_ok());
}
