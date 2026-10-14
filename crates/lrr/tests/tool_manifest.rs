use lrr::{
    core::digest::{Digest, StableEncode},
    project::{
        cargo_units::{DependencyEdge, UnitMode},
        tool_manifest::{
            BuildScriptMessage, InputKind, InputPlan, InputRoot, PreparedFiles, ToolManifest,
            ToolStore, associate_build_outputs, verify_adoption,
        },
    },
    source::{generated::GeneratedRoot, snapshot::SourcePath},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
mod fixture {
    include!("support/tool_manifest_fixture.rs");
}
use fixture::{Fixture, edges};
#[test]
fn schema4_tool_manifest_round_trip_preserves_identity_and_absent_inputs() {
    let fixture = Fixture::new();
    let bytes = fixture.manifest.encode().unwrap();
    let decoded = ToolManifest::decode(&bytes).unwrap();
    assert_eq!(fixture.manifest.fingerprint(), decoded.fingerprint());
    assert_eq!(decoded.inputs.environment["ABSENT"], None);
    let mut changed = decoded.clone();
    changed
        .inputs
        .environment
        .insert("ABSENT".into(), Some("".into()));
    assert_ne!(decoded.fingerprint(), changed.fingerprint());
}
#[test]
fn schema4_rejects_duplicate_keys_sets_noncanonical_paths_and_stale_schema() {
    let fixture = Fixture::new();
    let bytes = String::from_utf8(fixture.manifest.encode().unwrap()).unwrap();
    let duplicate = bytes.replacen(
        "\"schema_version\":4",
        "\"schema_version\":4,\"schema_version\":4",
        1,
    );
    assert!(
        ToolManifest::decode(duplicate.as_bytes())
            .unwrap_err()
            .message
            .contains("duplicate")
    );
    let set = bytes.replacen(
        "\"features\":[\"feature\"]",
        "\"features\":[\"feature\",\"feature\"]",
        1,
    );
    assert!(
        ToolManifest::decode(set.as_bytes())
            .unwrap_err()
            .message
            .contains("duplicate")
    );
    let stale = bytes.replacen("\"schema_version\":4", "\"schema_version\":3", 1);
    assert!(
        ToolManifest::decode(stale.as_bytes())
            .unwrap_err()
            .message
            .contains("schema4")
    );
    let path = bytes.replacen("\"generated.rs\":", "\"nested/../generated.rs\":", 1);
    assert!(ToolManifest::decode(path.as_bytes()).is_err());
    let mut unknown: serde_json::Value = serde_json::from_str(&bytes).unwrap();
    unknown["toolchain"]["surprise"] = serde_json::json!(true);
    assert!(ToolManifest::decode(&serde_json::to_vec(&unknown).unwrap()).is_err());
}
#[test]
fn schema4_rejects_wrong_unit_output_and_live_compiled_artifacts() {
    let fixture = Fixture::new();
    let live = *fixture.manifest.graph.roots.first().unwrap();
    let producer = *fixture.manifest.build_outputs.keys().next().unwrap();
    let mut wrong = fixture.manifest.clone();
    wrong.build_outputs.get_mut(&producer).unwrap().producer = live;
    assert!(wrong.validate().is_err());
    let mut wrong = fixture.manifest.clone();
    wrong.compile_env.get_mut(&live).unwrap().out_dir = Some("/wrong/unit/out".into());
    assert!(wrong.validate().is_err());
    let mut wrong = fixture.manifest.clone();
    wrong.artifacts.insert(
        live,
        lrr::project::tool_manifest::ToolArtifact {
            auxiliary: BTreeMap::new(),
            path: "/tools/not-a-tool.rlib".into(),
            content: Digest::default(),
            kind: lrr::project::tool_manifest::ArtifactKind::HostLibrary,
        },
    );
    assert!(wrong.validate().is_err());
    let mut wrong = fixture.manifest.clone();
    wrong.graph.units.get_mut(&live).unwrap().dependencies[0].target = live;
    assert!(wrong.validate().is_err());
}
#[test]
fn immutable_tool_store_publication_is_atomic_idempotent_and_stale_safe() {
    let fixture = Fixture::new();
    let store = ToolStore::open(&fixture.root.join("store")).unwrap();
    let id = store
        .publish(&fixture.manifest, &fixture.manifest.inputs)
        .unwrap();
    assert_eq!(store.load(id).unwrap().fingerprint(), id);
    assert_eq!(
        store
            .publish(&fixture.manifest, &fixture.manifest.inputs)
            .unwrap(),
        id
    );
    let before = fs::read_dir(fixture.root.join("store")).unwrap().count();
    let mut changed = fixture.manifest.inputs.clone();
    changed
        .environment
        .insert("ABSENT".into(), Some("new".into()));
    assert_eq!(
        store.publish(&fixture.manifest, &changed).unwrap_err().code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    let mut malformed = fixture.manifest.clone();
    malformed.schema_version = 3;
    assert!(store.publish(&malformed, &malformed.inputs).is_err());
    assert_eq!(
        fs::read_dir(fixture.root.join("store")).unwrap().count(),
        before
    );
    assert_eq!(store.load(id).unwrap().fingerprint(), id);
}
#[test]
fn concurrent_tool_publication_and_tampered_store_are_checked() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest.clone();
    let root = fixture.root.join("store");
    let second_root = root.clone();
    let worker = std::thread::spawn(move || {
        ToolStore::open(&second_root)
            .unwrap()
            .publish(&manifest, &manifest.inputs)
            .unwrap()
    });
    let store = ToolStore::open(&root).unwrap();
    let id = store
        .publish(&fixture.manifest, &fixture.manifest.inputs)
        .unwrap();
    assert_eq!(worker.join().unwrap(), id);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::write(root.join(format!("{}.json", id.hex())), b"{}").unwrap();
    assert!(store.load(id).is_err());
    assert!(
        store
            .publish(&fixture.manifest, &fixture.manifest.inputs)
            .is_err()
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}
#[test]
fn normalized_source_path_wire_cannot_hide_duplicate_membership() {
    assert!(serde_json::from_str::<SourcePath>("\"a/../b\"").is_err());
    assert_eq!(SourcePath::new("a/../b").unwrap().as_str(), "b");
    assert!(SourcePath::new("a\0b").is_err());
}

#[test]
fn schema4_wire_size_depth_and_environment_names_fail_boundedly() {
    assert!(
        ToolManifest::decode(&vec![b' '; 64 * 1024 * 1024 + 1])
            .unwrap_err()
            .message
            .contains("64 MiB")
    );
    let nested = format!("{}0{}", "[".repeat(256), "]".repeat(256));
    assert!(ToolManifest::decode(nested.as_bytes()).is_err());
    let mut fixture = Fixture::new();
    let unit = *fixture.manifest.compile_env.keys().next().unwrap();
    fixture
        .manifest
        .compile_env
        .get_mut(&unit)
        .unwrap()
        .cargo
        .insert("BAD=NAME".into(), "x".into());
    assert!(fixture.manifest.encode().is_err());
    fixture
        .manifest
        .compile_env
        .get_mut(&unit)
        .unwrap()
        .cargo
        .remove("BAD=NAME");
    fixture
        .manifest
        .compile_env
        .get_mut(&unit)
        .unwrap()
        .cargo
        .insert("LARGE".into(), "x".repeat(65537));
    assert!(fixture.manifest.encode().is_err());
    fixture
        .manifest
        .compile_env
        .get_mut(&unit)
        .unwrap()
        .cargo
        .remove("LARGE");
    fixture.manifest.toolchain.macro_server.path = "/tools/sysroot/../outside/server".into();
    assert!(fixture.manifest.encode().is_err());
}

#[test]
fn preparation_schema_contract_matches_serialized_record_fields() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../compat/preparation-v4.json")).unwrap();
    let mut fixture = Fixture::new();
    add_compiled_tool(&mut fixture);
    let wire = serde_json::to_value(&fixture.manifest).unwrap();
    fn check_fields(schema: &serde_json::Value, wire: &serde_json::Value) {
        let expected: BTreeSet<_> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let actual: BTreeSet<_> = wire
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(expected, actual);
        assert_eq!(schema["additionalProperties"], false);
    }
    check_fields(&schema, &wire);
    for (definition, record) in [
        ("ToolchainRecord", &wire["toolchain"]),
        ("ToolBinary", &wire["toolchain"]["compiler"]),
        ("InputInventory", &wire["inputs"]),
        (
            "ToolArtifact",
            wire["artifacts"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .unwrap(),
        ),
        ("UnitGraph", &wire["graph"]),
        (
            "CompileEnv",
            wire["compile_env"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .unwrap(),
        ),
        (
            "BuildOutput",
            wire["build_outputs"]
                .as_object()
                .unwrap()
                .values()
                .next()
                .unwrap(),
        ),
    ] {
        check_fields(&schema["$defs"][definition], record);
    }
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        wire["schema_version"]
    );
    assert_eq!(
        schema["properties"]["semantics_profile"]["const"],
        wire["semantics_profile"]
    );
}

#[test]
fn captured_input_publication_preserves_previous_record_on_changed_or_restored_inputs() {
    let mut fixture = Fixture::new();
    let selected = BTreeSet::from([
        SourcePath::new(std::path::Path::new("lib.rs")).unwrap(),
        SourcePath::new(std::path::Path::new("build.rs")).unwrap(),
    ]);
    let plan = InputPlan::new(
        vec![
            InputRoot::open(
                InputKind::Source,
                &fixture.root,
                selected,
                BTreeSet::new(),
                false,
                BTreeSet::new(),
            )
            .unwrap(),
        ],
        BTreeSet::new(),
    )
    .unwrap();
    let before = plan.capture().unwrap();
    fixture.manifest.inputs = before.inventory().clone();
    for env in fixture.manifest.compile_env.values_mut() {
        env.declared.clear();
    }
    let store = ToolStore::open(&fixture.root.join("store")).unwrap();
    let id = store
        .publish_captured(&fixture.manifest, &before, &plan.capture().unwrap())
        .unwrap();
    assert_eq!(
        store.load(id).unwrap().fingerprint(),
        fixture.manifest.fingerprint()
    );
    let original = fs::read(fixture.root.join("lib.rs")).unwrap();
    fs::write(fixture.root.join("lib.rs"), b"fn changed() {}\n").unwrap();
    let changed = plan.capture().unwrap();
    assert_eq!(
        store
            .publish_captured(&fixture.manifest, &before, &changed)
            .unwrap_err()
            .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    fs::write(fixture.root.join("replacement.rs"), original).unwrap();
    fs::rename(
        fixture.root.join("replacement.rs"),
        fixture.root.join("lib.rs"),
    )
    .unwrap();
    let restored = plan.capture().unwrap();
    assert_eq!(before.inventory(), restored.inventory());
    assert_eq!(
        store
            .publish_captured(&fixture.manifest, &before, &restored)
            .unwrap_err()
            .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    assert_eq!(store.load(id).unwrap().fingerprint(), id);
    assert_eq!(fs::read_dir(fixture.root.join("store")).unwrap().count(), 1);
}
#[test]
fn adoption_compares_current_records_without_tool_execution() {
    let mut fixture = Fixture::new();
    let plan = InputPlan::new(
        vec![
            InputRoot::open(
                InputKind::Source,
                &fixture.root,
                BTreeSet::from([
                    SourcePath::new(std::path::Path::new("lib.rs")).unwrap(),
                    SourcePath::new(std::path::Path::new("build.rs")).unwrap(),
                ]),
                BTreeSet::new(),
                false,
                BTreeSet::new(),
            )
            .unwrap(),
        ],
        BTreeSet::new(),
    )
    .unwrap();
    let capture = plan.capture().unwrap();
    fixture.manifest.inputs = capture.inventory().clone();
    for env in fixture.manifest.compile_env.values_mut() {
        env.declared.clear();
    }
    let files = PreparedFiles::capture(&fixture.manifest).unwrap();
    verify_adoption(
        &fixture.manifest,
        &fixture.manifest.toolchain,
        &capture,
        &files,
    )
    .unwrap();
    let mut changed_toolchain = fixture.manifest.toolchain.clone();
    changed_toolchain.macro_server.content = Digest::of_bytes(b"different server");
    assert_eq!(
        verify_adoption(&fixture.manifest, &changed_toolchain, &capture, &files)
            .unwrap_err()
            .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    fs::remove_file(fixture.root.join("lib.rs")).unwrap();
    assert_eq!(
        verify_adoption(
            &fixture.manifest,
            &fixture.manifest.toolchain,
            &plan.capture().unwrap(),
            &files,
        )
        .unwrap_err()
        .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
}

fn cargo_build_record(fixture: &Fixture) -> serde_json::Value {
    let output = fixture.manifest.build_outputs.values().next().unwrap();
    serde_json::json!({"reason":"build-script-executed", "package_id":"fixture@0.1.0", "linked_libs":[], "linked_paths":[], "cfgs":["generated_cfg"], "env":[["FROM_BUILD","captured"]], "out_dir":output.tree.out_dir()})
}
#[test]
fn cargo_build_record_decode_rejects_duplicates_reserved_env_and_unknown_schema() {
    let fixture = Fixture::new();
    let original = cargo_build_record(&fixture);
    BuildScriptMessage::decode(&serde_json::to_vec(&original).unwrap()).unwrap();
    for (field, replacement) in [
        (
            "env",
            serde_json::json!([["KEY", "first"], ["KEY", "last"]]),
        ),
        ("env", serde_json::json!([["OUT_DIR", "override"]])),
        ("cfgs", serde_json::json!(["same", "same"])),
        ("reason", serde_json::json!("compiler-artifact")),
        ("out_dir", serde_json::json!("relative")),
        ("package_id", serde_json::json!("")),
    ] {
        let mut malformed = original.clone();
        malformed[field] = replacement;
        assert!(
            BuildScriptMessage::decode(&serde_json::to_vec(&malformed).unwrap()).is_err(),
            "{malformed}"
        );
    }
    let mut unknown = original.clone();
    unknown["unknown"] = serde_json::json!(true);
    assert!(BuildScriptMessage::decode(&serde_json::to_vec(&unknown).unwrap()).is_err());
    let bytes = serde_json::to_string(&original).unwrap();
    let duplicate = bytes.replacen(
        "\"package_id\":",
        "\"package_id\":\"duplicate\",\"package_id\":",
        1,
    );
    assert!(
        BuildScriptMessage::decode(duplicate.as_bytes())
            .unwrap_err()
            .message
            .contains("duplicate")
    );
    assert!(BuildScriptMessage::decode(&vec![b' '; 1024 * 1024 + 1]).is_err());
}
#[test]
fn cargo_output_association_rejects_ambiguity_duplicate_reports_and_changed_graph() {
    let fixture = Fixture::new();
    let bytes = serde_json::to_vec(&cargo_build_record(&fixture)).unwrap();
    let message = BuildScriptMessage::decode(&bytes).unwrap();
    let associations =
        associate_build_outputs(&fixture.manifest.graph, vec![message.clone()]).unwrap();
    let producer = *associations.keys().next().unwrap();
    let captured = associations[&producer]
        .capture(&fixture.manifest.graph)
        .unwrap();
    assert_eq!(
        captured.fingerprint(),
        fixture.manifest.build_outputs[&producer].fingerprint()
    );
    assert!(
        associate_build_outputs(
            &fixture.manifest.graph,
            vec![message.clone(), message.clone()]
        )
        .is_err()
    );
    let mut ambiguous = fixture.manifest.graph.clone();
    let mut second = ambiguous.units[&producer].clone();
    second.key.features.insert("different-context".into());
    ambiguous.units.insert(second.key.fingerprint(), second);
    assert!(
        associate_build_outputs(&ambiguous, vec![message.clone()])
            .unwrap_err()
            .message
            .contains("one selected unit context")
    );
    let mut removed = fixture.manifest.graph.clone();
    removed.units.remove(&producer);
    assert!(associations[&producer].capture(&removed).is_err());
    let mut changed = fixture.manifest.graph.clone();
    let mut replacement = changed.units.remove(&producer).unwrap();
    replacement.key.features.insert("changed".into());
    let new_id = replacement.key.fingerprint();
    changed.units.insert(new_id, replacement);
    let consumer = *fixture.manifest.graph.roots.iter().next().unwrap();
    let unit = changed.units.get_mut(&consumer).unwrap();
    unit.dependencies[0].target = new_id;
    unit.key.dependency_edges = edges(&unit.dependencies);
    let new_consumer = unit.key.fingerprint();
    let remapped = changed.units.remove(&consumer).unwrap();
    changed.units.insert(new_consumer, remapped);
    changed.roots = BTreeSet::from([new_consumer]);
    assert!(
        associations[&producer]
            .capture(&changed)
            .unwrap_err()
            .message
            .contains("producer changed")
    );
    let mut oversized = fixture.manifest.graph.clone();
    let consumer = *oversized.roots.iter().next().unwrap();
    let dependencies = &mut oversized.units.get_mut(&consumer).unwrap().dependencies;
    dependencies.resize(100_001, dependencies[0].clone());
    assert!(
        associate_build_outputs(&oversized, vec![message.clone()])
            .unwrap_err()
            .message
            .contains("edge limit")
    );
    let mut wrong = fixture.manifest.graph.clone();
    wrong.units.get_mut(&consumer).unwrap().key.package = "another-package".into();
    assert!(
        associate_build_outputs(&wrong, vec![message])
            .unwrap_err()
            .message
            .contains("consumer edge")
    );
}

fn source_capture(fixture: &mut Fixture) -> lrr::project::tool_manifest::InputCapture {
    let plan = InputPlan::new(
        vec![
            InputRoot::open(
                InputKind::Source,
                &fixture.root,
                BTreeSet::from([
                    SourcePath::new(std::path::Path::new("lib.rs")).unwrap(),
                    SourcePath::new(std::path::Path::new("build.rs")).unwrap(),
                ]),
                BTreeSet::new(),
                false,
                BTreeSet::new(),
            )
            .unwrap(),
        ],
        BTreeSet::new(),
    )
    .unwrap();
    let capture = plan.capture().unwrap();
    fixture.manifest.inputs = capture.inventory().clone();
    for env in fixture.manifest.compile_env.values_mut() {
        env.declared.clear();
    }
    capture
}
fn add_compiled_tool(fixture: &mut Fixture) -> Digest {
    use lrr::project::tool_manifest::{ArtifactKind, ToolArtifact};
    let old_live = *fixture.manifest.graph.roots.iter().next().unwrap();
    let old_run = *fixture.manifest.build_outputs.keys().next().unwrap();
    let mut run = fixture.manifest.graph.units.remove(&old_run).unwrap();
    let mut compiled = run.clone();
    compiled.mode = UnitMode::Build;
    let compiled_id = compiled.key.fingerprint();
    run.dependencies = vec![DependencyEdge {
        alias: "build-script-executable".into(),
        target: compiled_id,
        public: false,
        no_prelude: false,
        no_unused: false,
    }];
    run.key.dependency_edges = edges(&run.dependencies);
    let run_id = run.key.fingerprint();
    let mut live = fixture.manifest.graph.units.remove(&old_live).unwrap();
    live.dependencies[0].target = run_id;
    live.key.dependency_edges = edges(&live.dependencies);
    let live_id = live.key.fingerprint();
    let mut output = fixture.manifest.build_outputs.remove(&old_run).unwrap();
    output.tree = GeneratedRoot::open(&run.key, std::path::Path::new(output.tree.out_dir()))
        .unwrap()
        .capture()
        .unwrap();
    output.producer = run_id;
    output.consumers = BTreeSet::from([live_id]);
    fixture.manifest.build_outputs.insert(run_id, output);
    let mut env = fixture.manifest.compile_env.remove(&old_live).unwrap();
    env.unit = live_id;
    fixture.manifest.compile_env.insert(live_id, env);
    fixture.manifest.graph.units.insert(compiled_id, compiled);
    fixture.manifest.graph.units.insert(run_id, run);
    fixture.manifest.graph.units.insert(live_id, live);
    fixture.manifest.graph.roots = BTreeSet::from([live_id]);
    let path = fixture.root.join("tools/prepared-build-script");
    fs::write(&path, b"compiled fixture tool bytes").unwrap();
    fixture.manifest.artifacts.insert(
        compiled_id,
        ToolArtifact {
            auxiliary: BTreeMap::from([(
                fixture
                    .root
                    .join("tools/prepared-metadata.rmeta")
                    .to_str()
                    .unwrap()
                    .into(),
                Digest::of_bytes(b"fixture metadata"),
            )]),
            path: path.to_str().unwrap().into(),
            content: Digest::of_bytes(b"compiled fixture tool bytes"),
            kind: ArtifactKind::BuildProgram,
        },
    );
    fs::write(
        fixture.root.join("tools/prepared-metadata.rmeta"),
        b"fixture metadata",
    )
    .unwrap();
    fixture.manifest.validate().unwrap();
    compiled_id
}
#[test]
fn prepared_file_capture_refuses_tampered_missing_and_escaping_files() {
    let mut fixture = Fixture::new();
    let unit = add_compiled_tool(&mut fixture);
    let sources = source_capture(&mut fixture);
    for path in [
        fixture.manifest.toolchain.planner.path.clone(),
        fixture.manifest.toolchain.compiler.path.clone(),
        fixture.manifest.toolchain.macro_server.path.clone(),
        fixture.manifest.toolchain.component_manifest.clone(),
        fixture
            .root
            .join("tools/prepared-metadata.rmeta")
            .to_str()
            .unwrap()
            .into(),
    ] {
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"changed toolchain file").unwrap();
        assert_eq!(
            PreparedFiles::capture(&fixture.manifest).unwrap_err().code,
            "LRR-E-TOOL-PREPARE-STALE"
        );
        fs::write(&path, original).unwrap();
    }
    let baseline = PreparedFiles::capture(&fixture.manifest).unwrap();
    verify_adoption(
        &fixture.manifest,
        &fixture.manifest.toolchain,
        &sources,
        &baseline,
    )
    .unwrap();
    let artifact = std::path::Path::new(&fixture.manifest.artifacts[&unit].path);
    fs::write(artifact, b"tampered").unwrap();
    assert_eq!(
        PreparedFiles::capture(&fixture.manifest).unwrap_err().code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    fs::remove_file(artifact).unwrap();
    assert_eq!(
        PreparedFiles::capture(&fixture.manifest).unwrap_err().code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    let outside = fixture.root.join("outside-tool");
    fs::write(&outside, b"compiled fixture tool bytes").unwrap();
    std::os::unix::fs::symlink(&outside, artifact).unwrap();
    assert_eq!(
        PreparedFiles::capture(&fixture.manifest).unwrap_err().code,
        "LRR-E-TOOLCHAIN-CAPABILITY"
    );
    fs::remove_file(artifact).unwrap();
    fs::write(artifact, b"compiled fixture tool bytes").unwrap();
    let restored = PreparedFiles::capture(&fixture.manifest).unwrap();
    assert_eq!(
        baseline.verify_unchanged(&restored).unwrap_err().code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    // A witness cannot be reused for a different immutable manifest, even if paths are unchanged.
    let mut other = fixture.manifest.clone();
    other.toolchain.planner.version_verbose.push_str(" changed");
    assert_eq!(
        verify_adoption(&other, &other.toolchain, &sources, &restored)
            .unwrap_err()
            .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
}
#[test]
fn compiled_file_changes_refuse_publication_and_preserve_previous_generation() {
    let mut fixture = Fixture::new();
    let unit = add_compiled_tool(&mut fixture);
    let sources = source_capture(&mut fixture);
    let files = PreparedFiles::capture(&fixture.manifest).unwrap();
    let store = ToolStore::open(&fixture.root.join("store")).unwrap();
    let id = store
        .publish_verified(
            &fixture.manifest,
            &sources,
            &sources,
            &files,
            &PreparedFiles::capture(&fixture.manifest).unwrap(),
        )
        .unwrap();
    let path = std::path::Path::new(&fixture.manifest.artifacts[&unit].path);
    let replacement = fixture.root.join("replacement-tool");
    fs::write(&replacement, b"compiled fixture tool bytes").unwrap();
    fs::rename(&replacement, path).unwrap();
    let after = PreparedFiles::capture(&fixture.manifest).unwrap();
    assert_eq!(
        store
            .publish_verified(&fixture.manifest, &sources, &sources, &files, &after)
            .unwrap_err()
            .code,
        "LRR-E-TOOL-PREPARE-STALE"
    );
    assert_eq!(store.load(id).unwrap().fingerprint(), id);
    assert_eq!(fs::read_dir(fixture.root.join("store")).unwrap().count(), 1);
    let mut live = fixture.manifest.clone();
    let artifact = live.artifacts.remove(&unit).unwrap();
    live.artifacts
        .insert(*live.graph.roots.iter().next().unwrap(), artifact);
    live.toolchain.compiler.path = "/missing/compiler".into();
    assert!(
        PreparedFiles::capture(&live)
            .unwrap_err()
            .message
            .contains("live/native unit")
    );
    let mut invalid = fixture.manifest.clone();
    invalid.schema_version = 3;
    invalid.toolchain.compiler.path = "/missing/tool".into();
    assert!(
        PreparedFiles::capture(&invalid)
            .unwrap_err()
            .message
            .contains("requires preparation schema4")
    );
}

#[test]
fn generated_lookup_uses_immutable_unit_context_and_explicit_absence() {
    use lrr::project::generated_sources::GeneratedSources;
    let fixture = Fixture::new();
    let unit = *fixture.manifest.graph.roots.iter().next().unwrap();
    let out = fixture
        .manifest
        .build_outputs
        .values()
        .next()
        .unwrap()
        .tree
        .out_dir()
        .to_owned();
    let index = GeneratedSources::new(std::sync::Arc::new(fixture.manifest.clone())).unwrap();
    let path = std::path::Path::new(&out).join("generated.rs");
    let before = index.lookup_original(unit, &path).unwrap();
    assert_eq!(
        before.source().unwrap().unwrap().text().unwrap(),
        "31_i64\n"
    );
    assert_eq!(before.original_path(), path.to_str().unwrap());
    let missing = SourcePath::new(std::path::Path::new("missing.rs")).unwrap();
    let absent = index.lookup(unit, missing.clone()).unwrap();
    assert_eq!(absent.bytes(), None);
    let absent_id = absent.fingerprint();
    let membership = absent.membership();
    fs::write(&path, b"changed live disk bytes").unwrap();
    fs::write(std::path::Path::new(&out).join("missing.rs"), b"new source").unwrap();
    assert_eq!(
        index
            .lookup_original(unit, &path)
            .unwrap()
            .source()
            .unwrap()
            .unwrap()
            .text()
            .unwrap(),
        "31_i64\n"
    );
    assert_eq!(
        index.lookup(unit, missing.clone()).unwrap().fingerprint(),
        absent_id
    );
    let mut updated = fixture.manifest.clone();
    let producer = *updated.build_outputs.keys().next().unwrap();
    updated.build_outputs.get_mut(&producer).unwrap().tree = GeneratedRoot::open(
        &updated.graph.units[&producer].key,
        std::path::Path::new(&out),
    )
    .unwrap()
    .capture()
    .unwrap();
    let new_index = GeneratedSources::new(std::sync::Arc::new(updated)).unwrap();
    let added = new_index.lookup(unit, missing).unwrap();
    assert_eq!(added.bytes(), Some(b"new source".as_slice()));
    assert_ne!(added.fingerprint(), absent_id);
    assert_ne!(added.membership(), membership);
    assert!(
        index
            .lookup_original(unit, &fixture.root.join("other/generated.rs"))
            .is_err()
    );
    assert!(
        index
            .lookup(
                Default::default(),
                SourcePath::new(std::path::Path::new("generated.rs")).unwrap()
            )
            .is_err()
    );
    fs::remove_dir_all(&out).unwrap();
    assert_eq!(
        index.lookup_original(unit, &path).unwrap().bytes(),
        Some(b"31_i64\n".as_slice())
    );
}
#[test]
fn generated_membership_is_independent_of_bytes_and_binary_text_is_demand_checked() {
    use lrr::project::generated_sources::GeneratedSources;
    let mut fixture = Fixture::new();
    let unit = *fixture.manifest.graph.roots.iter().next().unwrap();
    let producer = *fixture.manifest.build_outputs.keys().next().unwrap();
    let root = fixture.manifest.build_outputs[&producer]
        .tree
        .out_dir()
        .to_owned();
    fs::write(std::path::Path::new(&root).join("data.bin"), [0xff, 0xfe]).unwrap();
    let tree = GeneratedRoot::open(
        &fixture.manifest.graph.units[&producer].key,
        std::path::Path::new(&root),
    )
    .unwrap()
    .capture()
    .unwrap();
    let membership = tree.membership_fingerprint();
    fixture
        .manifest
        .build_outputs
        .get_mut(&producer)
        .unwrap()
        .tree = tree;
    let index = GeneratedSources::new(std::sync::Arc::new(fixture.manifest.clone())).unwrap();
    let lookup = index
        .lookup(
            unit,
            SourcePath::new(std::path::Path::new("data.bin")).unwrap(),
        )
        .unwrap();
    assert_eq!(lookup.bytes(), Some([0xff, 0xfe].as_slice()));
    assert_eq!(lookup.source().unwrap_err().code, "LRR-E-SOURCE-UTF8");
    fs::write(
        std::path::Path::new(&root).join("generated.rs"),
        b"different bytes",
    )
    .unwrap();
    let refreshed = GeneratedRoot::open(
        &fixture.manifest.graph.units[&producer].key,
        std::path::Path::new(&root),
    )
    .unwrap()
    .capture()
    .unwrap();
    assert_eq!(refreshed.membership_fingerprint(), membership);
}
