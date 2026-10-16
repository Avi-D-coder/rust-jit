//! LRR authority/route controls around unchanged official source, not a
//! substitute language suite. Constructed schema4 records exercise admission;
//! they do not claim a physically adopted SDK/tool preparation or guest run.
use super::*;
#[path = "compiler_attribute_preparation_tests.rs"]
mod compiler_attribute_preparation_tests;
#[path = "macro_export_module_tests.rs"]
mod module_adoption_tests;
#[path = "macro_export_consumption_tests.rs"]
mod module_consumption_tests;
#[path = "original_annotations_tests.rs"]
mod original_annotation_tests;
#[path = "original_module_discovery_tests.rs"]
mod original_module_discovery_tests;
#[path = "../../db/macro_export_inputs_tests.rs"]
mod publication_tests;
#[path = "macro_export_role_tests.rs"]
mod role_tests;
#[path = "sealed_macro_export_view_tests.rs"]
mod sealed_view_tests;
use crate::{
    core::profile::{GUEST_TARGET, ProfileIdentity},
    db::key::OutlineKey,
    project::{
        cargo_units::{CargoUnitKey, NormalizedUnit, TargetKind, UnitGraph, UnitMode},
        tool_manifest::{CompileEnv, InputInventory, ToolBinary, ToolchainRecord},
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::collections::{BTreeMap, BTreeSet};
const CORE: &[u8] = include_bytes!(
    "../../../../../fixtures/upstream/rustc-checkout/library/core/src/macros/mod.rs"
);
const AUX: &[u8] = include_bytes!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/auxiliary/foreign-crate-macro-pat.rs"
);
fn fixture_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout")
        .canonicalize()
        .unwrap()
}
fn manifest(
    path: &std::path::Path,
    bytes: &[u8],
    edition: u16,
    package: &std::path::Path,
) -> (ToolManifest, Digest) {
    let cfg = CfgSet::guest_default();
    let compiler = Digest([121; 32]);
    let mut edges = StableEncoder::new("lrr/v2/cargo-unit-edges");
    edges.u64(0);
    let key = CargoUnitKey {
        package: "macro-export-authority-control".into(),
        target_name: "fixture".into(),
        target_kind: TargetKind::Library,
        role: UnitRole::MacroExport,
        compile_target: GUEST_TARGET.into(),
        test_intent: false,
        edition,
        features: BTreeSet::new(),
        profile_fingerprint: Digest([122; 32]),
        cfg_fingerprint: cfg.fingerprint(),
        dependency_edges: edges.finish(),
        rustc_fingerprint: compiler,
    };
    let unit = key.fingerprint();
    let tool = |name: &str, selection: &str| ToolBinary {
        path: format!("/prepared/sysroot/{name}"),
        content: compiler,
        selection: selection.into(),
        version_verbose: "constructed-authority-control".into(),
    };
    let path = path.to_str().unwrap().to_owned();
    let manifest = ToolManifest {
        schema_version: 4,
        semantics_profile: crate::core::profile::SEMANTICS_PROFILE.into(),
        toolchain: ToolchainRecord {
            planner: tool("cargo", "nightly-2026-10-01"),
            compiler: tool("rustc", "1.99.0"),
            compiler_commit: "ae0fe511b15ab760d7e963b13389d1bc2ab42b18".into(),
            host: GUEST_TARGET.into(),
            sysroot: "/prepared/sysroot".into(),
            macro_server: tool("server", "1.99.0"),
            macro_protocol: 6,
            component_manifest: "/prepared/sysroot/component".into(),
            component_manifest_content: compiler,
            prepared_macro_server: None,
        },
        graph: UnitGraph {
            schema: 1,
            roots: BTreeSet::from([unit]),
            units: BTreeMap::from([(
                unit,
                NormalizedUnit {
                    key,
                    mode: UnitMode::Build,
                    source_path: path.clone(),
                    dependencies: vec![],
                },
            )]),
        },
        inputs: InputInventory {
            source_files: BTreeMap::from([(path, Some(Digest::of_bytes(bytes)))]),
            ..InputInventory::default()
        },
        artifacts: BTreeMap::new(),
        build_outputs: BTreeMap::new(),
        compile_env: BTreeMap::from([(
            unit,
            CompileEnv {
                unit,
                host: GUEST_TARGET.into(),
                target: GUEST_TARGET.into(),
                cfg,
                cargo: BTreeMap::from([(
                    "CARGO_MANIFEST_DIR".into(),
                    package.to_str().unwrap().into(),
                )]),
                rustc_env: BTreeMap::new(),
                declared: BTreeMap::new(),
                out_dir: None,
            },
        )]),
    };
    manifest.validate().unwrap();
    (manifest, unit)
}
fn input(
    unit: Digest,
    path: &str,
    bytes: &[u8],
    edition: u16,
) -> (SourceUnitInput, SourceSelection) {
    let blob = SourceBlob::new(bytes.to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new(path).unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    (
        SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        },
        selection,
    )
}
fn select(inputs: &PreparedSourceInputs, selection: &SourceSelection) -> PreparedSourceView {
    let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
        panic!("fixture source not selected")
    };
    view
}
#[test]
fn configured_original_export_uses_ordinary_matcher_and_definition_edition() {
    let package = fixture_root().join("tests/ui/macros/auxiliary");
    let path = package.join("foreign-crate-macro-pat.rs");
    let (manifest, unit) = manifest(&path, AUX, 2018, &package);
    for edition in [2015, 2018, 2021, 2024] {
        let (definition, definition_selection) =
            input(unit, "foreign-crate-macro-pat.rs", AUX, 2018);
        let (caller, caller_selection) = input(
            Digest([123; 32]),
            "caller.rs",
            b"extern crate renamed; renamed::custom_matches!(true, true);",
            edition,
        );
        let inputs = PreparedSourceInputs::refresh_with_dependencies(
            None,
            vec![definition, caller],
            BTreeMap::new(),
            vec![SourceDependency {
                consumer: Digest([123; 32]),
                alias: "renamed".into(),
                provider: unit,
            }],
        )
        .unwrap();
        let facet = crate::cli::capture_macro_export_facet(
            &manifest,
            unit,
            &path,
            select(&inputs, &definition_selection),
        )
        .unwrap();
        assert_eq!(facet.source().captured_macro_blob().unwrap().bytes(), AUX);
        let consumer = SourceDeclarative::new(select(&inputs, &caller_selection)).unwrap();
        let invocation = consumer.module_source().outline().item_invocations[0].item_input;
        let super::super::SourceDeclarativeDemand::Ready(request) = facet
            .declaration_demand(
                &consumer,
                invocation,
                Digest([124; 32]),
                super::super::engine_version(),
            )
            .unwrap()
        else {
            panic!("ordinary exported macro unresolved")
        };
        assert_eq!(request.key().definition_edition, 2018);
        assert_eq!(request.key().call_edition, edition);
        let crate::expand::identity::MacroIdentity::Declaration { definition } =
            &request.key().macro_identity
        else {
            panic!("not a source declaration")
        };
        assert!(
            facet
                .routes()
                .definitions()
                .iter()
                .any(|route| route.definition.identity == *definition
                    && route.definition.defining_unit == unit)
        );
        let output = request.expand().unwrap();
        assert!(output.token_count() > 0);
        assert_eq!(output.key(), request.key());
        assert!(
            output
                .tokens()
                .unwrap()
                .contexts()
                .iter()
                .any(|context| context.defining_unit == crate::syntax::token_wire::Key(unit.0))
        );
        assert!(facet.metadata_bytes() < 64 * 1024 * 1024);
    }
}
#[test]
fn actual_core_macro_source_keeps_owning_edition_and_builtin_obligation() {
    let package = fixture_root().join("library/core");
    let path = package.join("src/macros/mod.rs");
    // The configured crate root is lib.rs; the facet source remains the actual
    // macros module, not a synthetic crate body assembled from selected rules.
    let (mut manifest, _) = manifest(&path, CORE, 2024, &package);
    let (_, old) = manifest.graph.units.pop_first().unwrap();
    let unit = old.key.fingerprint();
    let root_path = package.join("src/lib.rs").to_str().unwrap().to_owned();
    manifest.inputs.source_files.insert(
        root_path.clone(),
        Some(Digest::of_bytes(include_bytes!(
            "../../../../../fixtures/upstream/rustc-checkout/library/core/src/lib.rs"
        ))),
    );
    manifest.graph.units.insert(
        unit,
        NormalizedUnit {
            source_path: root_path,
            ..old
        },
    );
    let (mut source, mut selection) = input(unit, "src/macros/mod.rs", CORE, 2024);
    source.module_path = vec!["macros".into()];
    selection.outline.module_path = source.module_path.clone();
    let inputs = PreparedSourceInputs::refresh(None, vec![source], BTreeMap::new()).unwrap();
    let facet = SourceMacroExportFacet::capture(
        &manifest,
        unit,
        path.to_str().unwrap(),
        select(&inputs, &selection),
    )
    .unwrap();
    let outline = facet.source().outline();
    let equality = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "assert_eq")
        .unwrap();
    let panic = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "panic")
        .unwrap();
    assert_eq!(facet.source().captured_macro_blob().unwrap().bytes(), CORE);
    assert_eq!(outline.profile.edition, 2024);
    assert!(!facet.requires_compiler_hook(equality.def_key.fingerprint()));
    assert!(facet.requires_compiler_hook(panic.def_key.fingerprint()));
    // These compiler attributes are still producers in the common ordinary
    // outline; source capture must not silently treat them as completed exports.
    assert!(
        facet
            .provider()
            .macro_index()
            .definition(equality.def_key.fingerprint())
            .is_none()
    );
    assert!(
        !facet
            .routes()
            .definitions()
            .iter()
            .any(|route| route.definition.identity == equality.def_key.fingerprint())
    );
    assert!(
        facet.routes().export_pending.len() > 0,
        "compiler attributes/producer closure are still genuine pending obligations"
    );
}
#[test]
fn wrong_unit_source_configuration_and_unpublished_facet_remain_typed() {
    let package = fixture_root().join("tests/ui/macros/auxiliary");
    let path = package.join("foreign-crate-macro-pat.rs");
    let (manifest, unit) = manifest(&path, AUX, 2018, &package);
    let (source, selection) = input(unit, "foreign-crate-macro-pat.rs", AUX, 2018);
    let inputs = PreparedSourceInputs::refresh(None, vec![source], BTreeMap::new()).unwrap();
    let view = select(&inputs, &selection);
    assert!(
        SourceMacroExportFacet::capture(
            &manifest,
            Digest([255; 32]),
            path.to_str().unwrap(),
            view.share()
        )
        .is_err()
    );
    let mut changed = manifest.clone();
    changed
        .inputs
        .source_files
        .insert(path.to_str().unwrap().into(), None);
    assert!(
        SourceMacroExportFacet::capture(&changed, unit, path.to_str().unwrap(), view.share())
            .is_err()
    );
    let mut changed = manifest.clone();
    changed
        .compile_env
        .get_mut(&unit)
        .unwrap()
        .cfg
        .atoms
        .insert("other_cfg".into());
    assert!(
        SourceMacroExportFacet::capture(&changed, unit, path.to_str().unwrap(), view.share())
            .is_err()
    );
    assert!(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            package.join("other.rs").to_str().unwrap(),
            view.share()
        )
        .is_err()
    );
    let facet = Arc::new(
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), view.share())
            .unwrap(),
    );
    let catalog = crate::frontend::compile_input_catalog::CompileInputCatalog::new(
        Digest([126; 32]),
        2018,
        [crate::expand::compile_input_identity::descriptor("concat")
            .unwrap()
            .unwrap()],
    )
    .unwrap()
    .with_macro_export_facets([Arc::clone(&facet)])
    .unwrap();
    assert_eq!(
        catalog.macro_export_facets().next().unwrap().identity(),
        facet.identity()
    );
    let discovery = crate::expand::source_discovery::SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        crate::source::manifest::Namespace::Macro,
        manifest.graph.fingerprint(),
        1,
    )
    .unwrap();
    let error = discovery
        .with_declarative_compiler(
            Digest([124; 32]),
            super::super::engine_version(),
            Arc::new(catalog),
        )
        .err()
        .unwrap();
    assert_eq!(error.code, "LRR-E-MACRO-EXPORT-PUBLICATION");
}

#[path = "../../cli/macro_export_modules_tests.rs"]
mod configured_module_preparation_tests;
