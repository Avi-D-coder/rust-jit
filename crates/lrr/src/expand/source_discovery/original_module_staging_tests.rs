//! Source-only authority route controls using unchanged official module files.
//! No guest execution or full upstream fixture result is claimed.
use super::*;
use crate::{
    expand::{builtins::FrozenCompileEnvironment, source_declarative},
    frontend::compile_input_catalog::CompileInputCatalog,
    project::tool_manifest::CompileEnv,
    service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    },
    source::{
        include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
        manifest::BodyKey,
    },
};
const MODULE_ROOT: &[u8] = include_bytes!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/modules/special_module_name_ignore.rs"
);
fn authority(directory: &std::path::Path, unit: Digest, generation: Digest) -> IncludeAuthorities {
    IncludeAuthorities::new(
        unit,
        generation,
        [IncludeRoot {
            identity: Digest([236; 32]),
            path: directory.to_path_buf(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap()
}
#[test]
fn unchanged_original_live_modules_capture_stage_complete_and_publish_with_actual_parent() {
    use crate::db::poll::{InputRequest, QueryPoll};
    for edition in [2015, 2018, 2021, 2024] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/upstream/rustc-checkout/tests/ui/modules")
            .canonicalize()
            .unwrap();
        let unit = Digest([235; 32]);
        let generation = Digest([237; 32]);
        let cfg = CfgSet::guest_default();
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let blob = SourceBlob::new(MODULE_ROOT.to_vec()).unwrap();
        let path = SourcePath::new(
            directory
                .join("special_module_name_ignore.rs")
                .to_str()
                .unwrap(),
        )
        .unwrap();
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
        let environment = Arc::new(
            FrozenCompileEnvironment::new(
                unit,
                &CompileEnv {
                    unit,
                    host: "aarch64-apple-darwin".into(),
                    target: "aarch64-apple-darwin".into(),
                    cfg: cfg.clone(),
                    cargo: BTreeMap::new(),
                    rustc_env: BTreeMap::new(),
                    declared: BTreeMap::new(),
                    out_dir: None,
                },
            )
            .unwrap(),
        );
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            BTreeMap::new(),
        )
        .unwrap()
        .with_compile_environment(Arc::clone(&environment))
        .unwrap();
        let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
            panic!("actual source missing");
        };
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        let mut d = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Macro,
            Digest([238; 32]),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            environment.fingerprint(),
            source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&selection.outline.parse.profile).unwrap()),
        )
        .unwrap();
        let producers: Vec<_> = d
            .unresolved_ordinary_module_sources()
            .map(|row| row.producer())
            .collect();
        assert_eq!(producers.len(), 2);
        let mut captured = vec![];
        for (ordinal, producer) in producers.into_iter().enumerate() {
            let input = match d.prepare_original_module_source(
                &mut db,
                producer,
                &authority(&directory, unit, generation),
            ) {
                QueryPoll::Ready(Ok(input)) => input,
                other => panic!("actual original module capture demand unavailable: {other:?}"),
            };
            let InputRequest::CaptureModule {
                request,
                compile_environment,
            } = &input
            else {
                panic!("module input missing");
            };
            let capture =
                SourceCapture::open(authority(&directory, unit, generation), BTreeMap::new())
                    .unwrap();
            let record = capture.capture_module(request).unwrap();
            let context = DemandContext {
                request: input.fingerprint(),
                session: Digest([239; 32]),
                unit,
                revision: db.source_revision(),
                epoch: 1,
                source_generation: generation,
                native_generation: None,
                compile_environment: *compile_environment,
            };
            let mut coordinator = ExpansionCoordinator::new(context.clone());
            let token = coordinator
                .issue(
                    crate::db::key::QueryKey::Outline(selection.outline.clone()),
                    PhaseIdentity {
                        frame: generation,
                        body: BodyKey {
                            definition: producer,
                            exact_body_bytes: input.fingerprint(),
                        },
                        index: u32::try_from(ordinal).unwrap(),
                    },
                    input.clone(),
                )
                .unwrap();
            coordinator.start(&context, &token).unwrap();
            coordinator
                .accept_module(&context, &token, &record)
                .unwrap();
            coordinator
                .adopt_module(&context, &token, &mut db, record)
                .unwrap();
            let child = match d.stage_ordinary_module_source(&mut db, &input) {
                QueryPoll::Ready(Ok(child)) => child,
                QueryPoll::Ready(Err(error)) => panic!(
                    "actual root module stage refused {}: {}",
                    error.code, error.message
                ),
                QueryPoll::NeedInput(_) => panic!("actual adopted record disappeared"),
            };
            assert_eq!(
                child.origin().declaration().source().selection(),
                &selection
            );
            assert_eq!(child.origin().declaration().producer(), producer);
            captured.push(input);
        }
        // Poll consumes actual child parse/import work; completion then retires
        // each actual parent producer, after which its ordinary work can seal.
        assert!(matches!(
            d.poll().unwrap(),
            SourceDiscoveryStep::Incomplete { .. }
        ));
        for input in &captured {
            match d.complete_ordinary_module_namespace(&mut db, input) {
                QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(_))) => {}
                QueryPoll::Ready(Err(error)) => panic!(
                    "actual root child closure refused {}: {}",
                    error.code, error.message
                ),
                _ => panic!("actual original child namespace was not complete"),
            }
        }
        assert!(matches!(d.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
        assert_eq!(d.unresolved_ordinary_module_sources().count(), 0);
        d.publish(&mut db).unwrap();
    }
}
