use lrr::{core::{digest::{Digest, StableEncode, StableEncoder},profile::GUEST_TARGET},project::{cargo_units::{CargoUnitKey,DependencyEdge,NormalizedUnit,TargetKind,UnitGraph,UnitMode,UnitRole},tool_manifest::{BuildOutput,CompileEnv,InputInventory,ToolBinary,ToolManifest,ToolchainRecord}},source::{generated::GeneratedRoot,snapshot::CfgSet}};
use std::{collections::{BTreeMap,BTreeSet},fs,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) manifest: ToolManifest,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
pub(super) fn edges(values: &[DependencyEdge]) -> Digest {
    let mut e = StableEncoder::new("lrr/v2/cargo-unit-edges");
    e.sequence(values, |e, d| {
        e.string(&d.alias);
        e.digest(d.target);
        e.bool(d.public);
        e.bool(d.no_prelude);
        e.bool(d.no_unused);
    });
    e.finish()
}
fn key(role: UnitRole, kind: TargetKind, dependencies: Digest, compiler: Digest) -> CargoUnitKey {
    CargoUnitKey {
        package: "fixture@0.1.0".into(),
        target_name: if kind == TargetKind::BuildScript {
            "build-script-build"
        } else {
            "fixture"
        }
        .into(),
        target_kind: kind,
        role,
        compile_target: GUEST_TARGET.into(),
        test_intent: false,
        edition: 2024,
        features: BTreeSet::from(["feature".into()]),
        profile_fingerprint: Digest::of_bytes(b"profile"),
        cfg_fingerprint: CfgSet::guest_default().fingerprint(),
        dependency_edges: dependencies,
        rustc_fingerprint: compiler,
    }
}
impl Fixture {
    pub(super) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let counter = NEXT
            .try_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_add(1),
            )
            .unwrap();
        let root = std::env::temp_dir().join(format!(
            "lrr-tool-manifest-{}-{}-{counter}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let out = root.join("out");
        fs::create_dir(&out).unwrap();
        fs::write(out.join("generated.rs"), "31_i64\n").unwrap();
        let store = root.join("store");
        fs::create_dir(&store).unwrap();
        let tools = root.join("tools");
        fs::create_dir_all(tools.join("sysroot/libexec")).unwrap();
        fs::create_dir_all(tools.join("sysroot/lib/rustlib")).unwrap();
        fs::write(tools.join("rustc"), b"fixture compiler").unwrap();
        fs::write(tools.join("cargo"), b"fixture planner").unwrap();
        fs::write(
            tools.join("sysroot/libexec/rust-analyzer-proc-macro-srv"),
            b"fixture server",
        )
        .unwrap();
        fs::write(
            tools.join("sysroot/lib/rustlib/manifest-rustc"),
            b"fixture component",
        )
        .unwrap();
        let compiler = ToolBinary {
            path: tools.join("rustc").to_str().unwrap().into(),
            content: Digest::of_bytes(b"fixture compiler"),
            selection: "1.99.0".into(),
            version_verbose: format!(
                "rustc 1.99.0\nrelease: 1.99.0\ncommit-hash: {}\nhost: {GUEST_TARGET}\n",
                "a".repeat(40)
            ),
        };
        let build = key(
            UnitRole::BuildHost,
            TargetKind::BuildScript,
            edges(&[]),
            compiler.content,
        );
        let build_id = build.fingerprint();
        let dependencies = vec![DependencyEdge {
            alias: "build_script_build".into(),
            target: build_id,
            public: false,
            no_prelude: false,
            no_unused: false,
        }];
        let live = key(
            UnitRole::Live,
            TargetKind::Library,
            edges(&dependencies),
            compiler.content,
        );
        let live_id = live.fingerprint();
        let build_path = root.join("build.rs").to_str().unwrap().to_owned();
        let source_path = root.join("lib.rs").to_str().unwrap().to_owned();
        fs::write(&build_path, "fn main() {}\n").unwrap();
        fs::write(&source_path, "fn cold() { absent(); }\n").unwrap();
        let generated = GeneratedRoot::open(&build, &out)
            .unwrap()
            .capture()
            .unwrap();
        let manifest = ToolManifest {
            schema_version: 4,
            semantics_profile: "lrr-2024-v2".into(),
            toolchain: ToolchainRecord {
                planner: ToolBinary {
                    path: tools.join("cargo").to_str().unwrap().into(),
                    content: Digest::of_bytes(b"fixture planner"),
                    selection: "nightly-2026-10-01".into(),
                    version_verbose: "cargo fixture-nightly".into(),
                },
                compiler,
                compiler_commit: "a".repeat(40),
                host: GUEST_TARGET.into(),
                sysroot: tools.join("sysroot").to_str().unwrap().into(),
                macro_server: ToolBinary {
                    path: tools
                        .join("sysroot/libexec/rust-analyzer-proc-macro-srv")
                        .to_str()
                        .unwrap()
                        .into(),
                    content: Digest::of_bytes(b"fixture server"),
                    selection: "1.99.0".into(),
                    version_verbose: "fixture server/compiler tuple".into(),
                },
                macro_protocol: 6,
                component_manifest: tools
                    .join("sysroot/lib/rustlib/manifest-rustc")
                    .to_str()
                    .unwrap()
                    .into(),
                component_manifest_content: Digest::of_bytes(b"fixture component"),
                prepared_macro_server: None,
            },
            graph: UnitGraph {
                schema: 1,
                roots: BTreeSet::from([live_id]),
                units: BTreeMap::from([
                    (
                        build_id,
                        NormalizedUnit {
                            key: build,
                            mode: UnitMode::RunBuildScript,
                            source_path: build_path.clone(),
                            dependencies: vec![],
                        },
                    ),
                    (
                        live_id,
                        NormalizedUnit {
                            key: live,
                            mode: UnitMode::Check,
                            source_path: source_path.clone(),
                            dependencies,
                        },
                    ),
                ]),
            },
            inputs: InputInventory {
                source_files: BTreeMap::from([
                    (
                        build_path.clone(),
                        Some(Digest::of_bytes(&fs::read(build_path).unwrap())),
                    ),
                    (
                        source_path.clone(),
                        Some(Digest::of_bytes(&fs::read(source_path).unwrap())),
                    ),
                ]),
                environment: BTreeMap::from([("ABSENT".into(), None)]),
                ..InputInventory::default()
            },
            artifacts: BTreeMap::new(),
            compile_env: BTreeMap::from([(
                live_id,
                CompileEnv {
                    unit: live_id,
                    host: GUEST_TARGET.into(),
                    target: GUEST_TARGET.into(),
                    cfg: CfgSet::guest_default(),
                    cargo: BTreeMap::from([("OUT_DIR".into(), out.to_str().unwrap().into())]),
                    rustc_env: BTreeMap::from([("FROM_BUILD".into(), "captured".into())]),
                    declared: BTreeMap::from([("ABSENT".into(), None)]),
                    out_dir: Some(out.to_str().unwrap().into()),
                },
            )]),
            build_outputs: BTreeMap::from([(
                build_id,
                BuildOutput {
                    producer: build_id,
                    consumers: BTreeSet::from([live_id]),
                    tree: generated,
                    cfgs: BTreeSet::from(["generated_cfg".into()]),
                    rustc_env: BTreeMap::from([("FROM_BUILD".into(), "captured".into())]),
                    linked_libs: vec![],
                    linked_paths: vec![],
                    dependency_metadata: BTreeMap::new(),
                },
            )]),
        };
        Self { root, manifest }
    }
}
