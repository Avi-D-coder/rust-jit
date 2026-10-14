use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
    },
    expand::{
        builtins::FrozenCompileEnvironment,
        compile_input_identity::descriptor,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    project::tool_manifest::CompileEnv,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::{collections::BTreeMap, sync::Arc};
const UNIT: Digest = Digest([231; 32]);
fn fixture(
    source: &str,
) -> (
    SourceDiscovery,
    Database,
    FrozenCompileEnvironment,
    IncludeAuthorities,
    std::path::PathBuf,
) {
    fixture_with_environment(source, None)
}
fn fixture_with_environment(
    source: &str,
    value: Option<&str>,
) -> (
    SourceDiscovery,
    Database,
    FrozenCompileEnvironment,
    IncludeAuthorities,
    std::path::PathBuf,
) {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE
        .try_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m013-discovery-include-{}-{}-{}",
            std::process::id(),
            sequence,
            Digest::of_bytes(source.as_bytes()).hex()
        ));
    std::fs::create_dir_all(&path).unwrap();
    let path = std::fs::canonicalize(path).unwrap();
    std::fs::write(path.join("part.rs"), "pub fn made()->i64{691_i64}").unwrap();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let cfg = CfgSet::guest_default();
    let profile = ProfileIdentity::current();
    let source_path = SourcePath::new("main.rs").unwrap();
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path: source_path.clone(),
            crate_key: UNIT,
            module_path: vec![],
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob: blob.clone(),
        }],
        Default::default(),
    )
    .unwrap();
    let selection = SourceSelection {
        path: source_path,
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile,
            },
            crate_key: UNIT,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let env = FrozenCompileEnvironment::new(
        UNIT,
        &CompileEnv {
            unit: UNIT,
            host: "aarch64-apple-darwin".into(),
            target: "aarch64-apple-darwin".into(),
            cfg: cfg.clone(),
            cargo: BTreeMap::new(),
            rustc_env: value
                .map(|v| BTreeMap::from([("CONTROL".into(), v.into())]))
                .unwrap_or_default(),
            declared: BTreeMap::new(),
            out_dir: None,
        },
    )
    .unwrap();
    let inputs = inputs
        .with_compile_environment(Arc::new(env.clone()))
        .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let catalog = Arc::new(
        CompileInputCatalog::new(
            Digest([232; 32]),
            2024,
            [
                descriptor("include").unwrap().unwrap(),
                descriptor("concat").unwrap().unwrap(),
                descriptor("env").unwrap().unwrap(),
            ],
        )
        .unwrap(),
    );
    let work = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([233; 32]),
        1,
    )
    .unwrap()
    .with_declarative(env.fingerprint(), Digest([234; 32]))
    .unwrap()
    .with_compiler(catalog)
    .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let authorities = IncludeAuthorities::new(
        UNIT,
        Digest([235; 32]),
        [IncludeRoot {
            identity: Digest([236; 32]),
            path: path.clone(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    (work, db, env, authorities, path)
}
fn complete(
    work: &mut SourceDiscovery,
    db: &mut Database,
    env: &FrozenCompileEnvironment,
    authorities: &IncludeAuthorities,
) -> usize {
    complete_verifying(work, db, env, authorities, false)
}
fn complete_verifying(
    work: &mut SourceDiscovery,
    db: &mut Database,
    env: &FrozenCompileEnvironment,
    authorities: &IncludeAuthorities,
    reject_forged: bool,
) -> usize {
    try_complete_verifying(work, db, env, authorities, reject_forged).unwrap()
}
fn try_complete_verifying(
    work: &mut SourceDiscovery,
    db: &mut Database,
    env: &FrozenCompileEnvironment,
    authorities: &IncludeAuthorities,
    reject_forged: bool,
) -> Result<usize, lrr::core::diagnostic::Diagnostic> {
    use lrr::service::{
        eager_compile_inputs::EagerCompileInputPoll,
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        source_capture::SourceCapture,
    };
    let context = DemandContext {
        request: Digest([237; 32]),
        session: Digest([238; 32]),
        unit: UNIT,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([235; 32]),
        native_generation: None,
        compile_environment: env.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let query = QueryKey::Constant(fixture_key(
        Digest([239; 32]),
        vec![],
        Digest::default(),
        ProfileIdentity::current(),
    ));
    let mut captured = 0;
    for index in 0..32 {
        let phase = PhaseIdentity {
            frame: Digest([240; 32]),
            body: lrr::source::manifest::BodyKey {
                definition: Digest([241; 32]),
                exact_body_bytes: Digest([242; 32]),
            },
            index,
        };
        let step = work.poll()?;
        let compiler = match &step {
            SourceDiscoveryStep::NeedExpansion { request } => request.is_compiler(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => request.is_compiler(),
            _ => false,
        };
        if compiler {
            match work.poll_compiler(db, env, authorities)? {
                EagerCompileInputPoll::NeedInput(input @ InputRequest::CaptureInclude { .. }) => {
                    let InputRequest::CaptureInclude { request, .. } = &input else {
                        unreachable!()
                    };
                    let capture = SourceCapture::open(
                        IncludeAuthorities::new(
                            UNIT,
                            Digest([235; 32]),
                            [authorities.root(Digest([236; 32])).unwrap().clone()],
                        )
                        .unwrap(),
                        BTreeMap::new(),
                    )
                    .unwrap()
                    .capture(request)
                    .unwrap();
                    let token = coordinator.issue(query.clone(), phase, input).unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept_include(&context, &token, &capture)
                        .unwrap();
                    coordinator
                        .adopt_include(&context, &token, db, capture)
                        .unwrap();
                    captured += 1;
                }
                EagerCompileInputPoll::Produced(record) => {
                    let input = InputRequest::Expand(Box::new(lrr::db::poll::ExpansionRequest {
                        key: record.key().clone(),
                    }));
                    let token = coordinator.issue(query.clone(), phase, input).unwrap();
                    coordinator.start(&context, &token).unwrap();
                    coordinator
                        .accept(&context, &token, Reply::success(&token, record.output()))
                        .unwrap();
                    coordinator.adopt(&context, &token, db, record).unwrap();
                }
                EagerCompileInputPoll::NeedExternal(request) => {
                    let token = coordinator
                        .issue(query.clone(), phase, request.input())
                        .unwrap();
                    coordinator.start(&context, &token).unwrap();
                    let record =
                        work.expand_compiler_string_operand(db, env, authorities, &request)?;
                    assert_eq!(record.key(), request.key());
                    assert!(
                        db.input(&lrr::db::dependency::InputKey::ExpansionRecord(
                            record.key().fingerprint()
                        ))
                        .is_none()
                    );
                    coordinator
                        .accept(&context, &token, Reply::success(&token, record.output()))
                        .unwrap();
                    coordinator.adopt(&context, &token, db, record).unwrap();
                }
                EagerCompileInputPoll::Complete(record) => {
                    if reject_forged {
                        assert!(
                            work.admit_expansion(Arc::new(record.clone()))
                                .unwrap_err()
                                .message
                                .contains("explicit adopted")
                        );
                        let original = record.tokens().unwrap();
                        let forged = lrr_expand_adapter::token_wire::parse_source_in_context(
                            "fn spoof()->i64{999_i64}",
                            lrr_expand_adapter::token_wire::SourceIdentity {
                                unit: lrr_expand_adapter::token_wire::Key(UNIT.0),
                                content: lrr_expand_adapter::token_wire::Key([243; 32]),
                                path: "spoof.rs".into(),
                            },
                            &lrr_expand_adapter::token_wire::TokenForest::decode(
                                &original.encode().unwrap(),
                            )
                            .unwrap(),
                            lrr_expand_adapter::token_wire::Key(original.spans()[0].key().0),
                            lrr_expand_adapter::token_wire::Key([244; 32]),
                        )
                        .unwrap();
                        let forged = Arc::new(
                            lrr::expand::record::ExpansionRecord::from_wire(
                                record.key().clone(),
                                forged.encode().unwrap(),
                            )
                            .unwrap(),
                        );
                        assert!(
                            work.admit_compiler_expansion(db, env, authorities, forged)
                                .unwrap_err()
                                .message
                                .contains("differs from adopted input reproduction")
                        );
                    }
                    work.admit_compiler_expansion(db, env, authorities, Arc::new(record))?;
                }
                _ => panic!("unexpected compiler suspension"),
            }
            continue;
        }
        match step {
            SourceDiscoveryStep::NeedExpansion { request } => {
                work.admit_expansion(Arc::new(request.expand().unwrap()))?
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                work.admit_expansion(Arc::new(request.expand().unwrap()))?
            }
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. }
            | SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => return Ok(captured),
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => return Err(diagnostic),
        }
    }
    panic!("include discovery did not terminate")
}
#[test]
fn captured_include_contributes_a_generated_header_before_discovery_seals() {
    let (mut work, mut db, env, authority, _) =
        fixture("include!(\"part.rs\");fn main()->i64{made()}");
    assert_eq!(complete(&mut work, &mut db, &env, &authority), 1);
    assert!(matches!(
        work.lookup("made").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
}
#[test]
fn compiler_operand_and_final_output_share_discovery_token_budget() {
    let text = "include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}";
    let (mut preview, mut preview_db, env, authorities, _) = fixture(text);
    complete(&mut preview, &mut preview_db, &env, &authorities);
    let integrated_tokens = preview
        .sealed_expansions()
        .unwrap()
        .map(|record| record.token_count())
        .sum();
    let (work, mut db, env, authorities, _) = fixture(text);
    let mut work = work.with_generated_token_limit(integrated_tokens).unwrap();
    let error = try_complete_verifying(&mut work, &mut db, &env, &authorities, false).unwrap_err();
    assert_eq!(error.code, "LRR-E-DISCOVERY-LIMIT");
    assert_eq!(error.message, "cumulative generated token limit");
    assert!(work.generated_token_count().unwrap() <= integrated_tokens);
    assert!(work.sealed_expansions().is_err());
    let revision = db.revision();
    let Err(error) = work.poll_compiler(&mut db, &env, &authorities) else {
        panic!("repeated compiler polling bypassed quota");
    };
    assert_eq!(error.code, "LRR-E-DISCOVERY-LIMIT");
    assert_eq!(db.revision(), revision);
}
#[test]
fn repeated_compiler_observation_charges_each_actual_output_once() {
    let (mut work, mut db, env, authorities, _) =
        fixture("include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}");
    complete(&mut work, &mut db, &env, &authorities);
    let integrated_tokens: usize = work
        .sealed_expansions()
        .unwrap()
        .map(|record| record.token_count())
        .sum();
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
    assert_eq!(work.generated_token_count().unwrap(), integrated_tokens + 2);
    work.publish(&mut db).unwrap();
    let tokens = work.generated_token_count().unwrap();
    work.publish(&mut db).unwrap();
    assert_eq!(work.generated_token_count().unwrap(), tokens);
}
#[test]
fn compiler_accounting_does_not_charge_unobserved_snapshot_records() {
    use lrr::service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply};
    let text = "include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}";
    let (mut preview, mut preview_db, env, authorities, _) = fixture(text);
    complete(&mut preview, &mut preview_db, &env, &authorities);
    let needed = preview.generated_token_count().unwrap();
    let (work, mut db, env, authorities, _) = fixture(text);
    let mut work = work.with_generated_token_limit(needed).unwrap();
    let (mut unrelated, _, _, _, _) =
        fixture("macro_rules! unused{()=>{fn first(){}fn second(){}fn third(){}}}unused!();");
    let SourceDiscoveryStep::NeedExpansion { request } = unrelated.poll().unwrap() else {
        panic!("unrelated captured demand absent")
    };
    let record = request.expand().unwrap();
    assert!(record.token_count() > needed);
    let context = DemandContext {
        request: Digest([237; 32]),
        session: Digest([238; 32]),
        unit: UNIT,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([235; 32]),
        native_generation: None,
        compile_environment: env.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let input = InputRequest::Expand(Box::new(lrr::db::poll::ExpansionRequest {
        key: record.key().clone(),
    }));
    let token = coordinator
        .issue(
            QueryKey::CurrentExpansionInput(Box::new(record.key().clone())),
            PhaseIdentity {
                frame: Digest([240; 32]),
                body: lrr::source::manifest::BodyKey {
                    definition: Digest([241; 32]),
                    exact_body_bytes: Digest([242; 32]),
                },
                index: 0,
            },
            input,
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept(&context, &token, Reply::success(&token, record.output()))
        .unwrap();
    coordinator
        .adopt(&context, &token, &mut db, record)
        .unwrap();
    complete(&mut work, &mut db, &env, &authorities);
    assert_eq!(work.generated_token_count().unwrap(), needed);
    assert_eq!(work.accounted_expansion_count().unwrap(), 2);
}
#[test]
fn nested_concat_include_suspends_and_adopts_before_discovery_seals() {
    let (mut work, mut db, env, authority, _) =
        fixture("include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}");
    assert_eq!(complete(&mut work, &mut db, &env, &authority), 1);
    work.publish(&mut db).unwrap();
    assert!(matches!(
        work.lookup("made").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
}

#[test]
fn compiler_discovery_requires_captured_environment_and_does_not_expand_on_ordinary_path() {
    let (mut work, mut db, env, authority, _) =
        fixture("include!(\"part.rs\");fn main()->i64{made()}");
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("compiler request absent")
    };
    assert!(request.is_compiler());
    assert_eq!(request.expand().err().unwrap().code, "LRR-E-COMPILE-INPUT");
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(
        matches!(work.poll_compiler(&mut db,&env,&authority).unwrap(),lrr::service::eager_compile_inputs::EagerCompileInputPoll::NeedInput(InputRequest::CaptureCompileEnvironment{unit}) if unit==UNIT)
    );
}
#[test]
fn compiler_discovery_rejects_ordinary_and_premature_output_admission() {
    let (mut work, mut db, env, authority, _) =
        fixture("include!(\"part.rs\");fn main()->i64{made()}");
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("compiler request absent")
    };
    let forest = lrr_expand_adapter::token_wire::parse_source(
        "fn spoof()->i64{999_i64}",
        lrr_expand_adapter::token_wire::SourceIdentity {
            unit: lrr_expand_adapter::token_wire::Key(UNIT.0),
            content: lrr_expand_adapter::token_wire::Key([243; 32]),
            path: "fake.rs".into(),
        },
        lrr_expand_adapter::token_wire::ContextRecord {
            parent: None,
            expansion: None,
            transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
            defining_unit: lrr_expand_adapter::token_wire::Key(UNIT.0),
            edition: lrr_expand_adapter::token_wire::Edition::E2024,
        },
        lrr_expand_adapter::token_wire::Key([244; 32]),
    )
    .unwrap();
    let record = Arc::new(
        lrr::expand::record::ExpansionRecord::from_wire(
            request.key().clone(),
            forest.encode().unwrap(),
        )
        .unwrap(),
    );
    assert!(
        work.admit_expansion(record.clone())
            .unwrap_err()
            .message
            .contains("explicit adopted")
    );
    assert!(
        work.admit_compiler_expansion(&mut db, &env, &authority, record)
            .unwrap_err()
            .message
            .contains("before input adoption")
    );
    assert_eq!(complete(&mut work, &mut db, &env, &authority), 1);
    assert!(matches!(
        work.lookup("spoof").unwrap(),
        lrr::frontend::names::NameLookup::Absent
    ));
}
#[test]
fn discovery_compiler_seal_refuses_changed_environment_with_unchanged_source() {
    let source = "include!(\"part.rs\");fn main()->i64{made()}";
    let (mut work, mut db, env, authority, _) = fixture(source);
    complete(&mut work, &mut db, &env, &authority);
    work.publish(&mut db).unwrap();
    let (_, mut changed, other, _, _) = fixture_with_environment(source, Some("changed"));
    assert_ne!(env.fingerprint(), other.fingerprint());
    assert!(
        work.publish(&mut changed)
            .unwrap_err()
            .message
            .contains("compiler input is stale")
    );
    work.publish(&mut db).unwrap();
}
#[test]
fn included_function_executes_in_guest_and_matches_native_macos() {
    use lrr::{
        db::value::QueryValue,
        engine::run::{RunMachine, RunPoll, ValueSummary},
    };
    let source = "include!(concat!(\"part\",\".rs\"));fn main()->i64{made()}";
    let (mut work, mut db, env, authority, path) = fixture(source);
    complete(&mut work, &mut db, &env, &authority);
    let QueryPoll::Ready(Ok(value)) = db.fetch(
        &SyntaxProvider,
        QueryKey::Outline(OutlineKey {
            parse: ParseKey {
                content: Digest::of_bytes(source.as_bytes()),
                profile: ProfileIdentity::current(),
            },
            crate_key: UNIT,
            module_path: vec![],
            cfg: CfgSet::guest_default().fingerprint(),
        }),
    ) else {
        panic!("entry outline absent")
    };
    let QueryValue::Outline(outline) = value.value.as_ref() else {
        panic!("entry outline variant")
    };
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_discovery(&mut db, body, 10_000, work)
    else {
        panic!("guest entry absent")
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("guest did not finish")
    };
    let ValueSummary::Signed(actual) = report.value else {
        panic!("noninteger guest result")
    };
    std::fs::write(path.join("vm.stdout"), &actual).unwrap();
    let version = std::process::Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(
        version_text.contains("release: 1.99.0")
            && version_text.contains("host: aarch64-apple-darwin")
    );
    std::fs::write(path.join("rustc.stdout"), version.stdout).unwrap();
    std::fs::write(
        path.join("oracle.rs"),
        source.replace("fn main()->i64", "fn case()->i64")
            + "\nfn main(){::std::println!(\"{}\",case());}\n",
    )
    .unwrap();
    let built = std::process::Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
        .arg(path.join("oracle.rs"))
        .arg("-o")
        .arg(path.join("oracle"))
        .output()
        .unwrap();
    std::fs::write(path.join("compile.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = std::process::Command::new(path.join("oracle"))
        .output()
        .unwrap();
    std::fs::write(path.join("native.stdout"), &native.stdout).unwrap();
    assert!(native.status.success());
    assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
}

#[test]
fn included_macro_definition_resolves_a_later_original_producer() {
    let (mut work, mut db, env, authority, path) =
        fixture("include!(\"part.rs\");inner!();fn main()->i64{made()}");
    std::fs::write(
        path.join("part.rs"),
        "macro_rules! inner{()=>{pub fn made()->i64{709_i64}}}",
    )
    .unwrap();
    assert_eq!(complete(&mut work, &mut db, &env, &authority), 1);
    assert!(matches!(
        work.lookup("made").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    work.publish(&mut db).unwrap();
}
#[test]
fn source_macro_can_shadow_compiler_include_without_requesting_capture() {
    let (mut work, mut db, env, authority, _) = fixture(
        "macro_rules! include{($p:expr)=>{pub fn made()->i64{701_i64}}}include!(\"unread.rs\");fn main()->i64{made()}",
    );
    assert_eq!(complete(&mut work, &mut db, &env, &authority), 0);
    work.publish(&mut db).unwrap();
}

#[test]
fn compiler_discovery_reproduces_output_from_adopted_capture_before_admission() {
    let (mut work, mut db, env, authority, _) =
        fixture("include!(\"part.rs\");fn main()->i64{made()}");
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    assert!(matches!(
        work.lookup("spoof").unwrap(),
        lrr::frontend::names::NameLookup::Absent
    ));
    work.publish(&mut db).unwrap();
}

#[test]
fn generated_compiler_include_closes_with_exact_adopted_source_dependencies() {
    let (mut work, mut db, env, authority, _) =
        fixture("macro_rules! emit{()=>{include!(\"part.rs\");}}emit!();fn main()->i64{made()}");
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    work.publish(&mut db).unwrap();
    assert_eq!(work.sealed_expansions().unwrap().len(), 2);
    assert!(matches!(
        work.lookup("made").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
}
#[test]
fn nested_included_compiler_inputs_all_join_the_discovery_seal() {
    let (mut work, mut db, env, authority, path) =
        fixture("include!(\"part.rs\");fn main()->i64{made()}");
    std::fs::create_dir_all(path.join("nested")).unwrap();
    std::fs::write(
        path.join("part.rs"),
        "include!(concat!(\"nested/second\",\".rs\"));",
    )
    .unwrap();
    std::fs::write(path.join("nested/second.rs"), "fn made()->i64{1193_i64}").unwrap();
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        2
    );
    assert_eq!(work.sealed_expansions().unwrap().len(), 2);
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn included_macro_invocation_file_base_is_bound_to_adopted_compiler_inputs() {
    let (mut work, mut db, env, authority, path) =
        fixture("include!(\"definitions/macros.rs\");emit!();fn main()->i64{made()}");
    std::fs::create_dir_all(path.join("definitions")).unwrap();
    std::fs::write(
        path.join("definitions/macros.rs"),
        "macro_rules! emit{()=>{include!(\"part.rs\");}}",
    )
    .unwrap();
    std::fs::write(
        path.join("definitions/part.rs"),
        "fn wrong()->i64{1319_i64}",
    )
    .unwrap();
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        2
    );
    assert_eq!(work.sealed_expansions().unwrap().len(), 3);
    assert!(matches!(
        work.lookup("made").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    assert!(matches!(
        work.lookup("wrong").unwrap(),
        lrr::frontend::names::NameLookup::Absent
    ));
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
}
#[test]
fn included_macro_call_in_another_source_keeps_seal_revalidation() {
    let source =
        "include!(\"definitions/macros.rs\");include!(\"calls/use.rs\");fn main()->i64{made()}";
    let (mut work, mut db, env, authority, path) = fixture(source);
    std::fs::create_dir_all(path.join("definitions")).unwrap();
    std::fs::create_dir_all(path.join("calls")).unwrap();
    std::fs::write(
        path.join("definitions/macros.rs"),
        "macro_rules! emit{()=>{include!(concat!(\"part\",\".rs\"));}}",
    )
    .unwrap();
    std::fs::write(path.join("calls/use.rs"), "emit!();").unwrap();
    std::fs::write(path.join("calls/part.rs"), "fn made()->i64{1321_i64}").unwrap();
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        3
    );
    assert_eq!(work.sealed_expansions().unwrap().len(), 4);
    work.publish(&mut db).unwrap();
    let (_, mut changed, _, _, _) = fixture_with_environment(source, Some("changed"));
    assert!(
        work.publish(&mut changed)
            .unwrap_err()
            .message
            .contains("compiler input is stale")
    );
    work.publish(&mut db).unwrap();
}

#[test]
fn user_string_operand_requires_explicit_adoption_before_include_capture() {
    let (mut work, mut db, env, authority, _) = fixture(
        "macro_rules! stem{()=>{\"part\"}}include!(concat!(stem!(),\".rs\"));fn main()->i64{made()}",
    );
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
}
#[test]
fn generated_local_string_operand_joins_compiler_seal_dependencies() {
    let (mut work, mut db, env, authority, _) = fixture(
        "macro_rules! emit{()=>{macro_rules! path{()=>{\"part.rs\"}}include!(path!());}}emit!();fn main()->i64{made()}",
    );
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    assert_eq!(work.sealed_expansions().unwrap().len(), 2);
    work.publish(&mut db).unwrap();
    assert!(matches!(
        work.lookup("made").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
}

#[test]
fn external_string_operand_rejects_a_different_demand_and_never_adopts_itself() {
    use lrr::service::eager_compile_inputs::EagerCompileInputPoll;
    let (mut work, mut db, env, authority, _) =
        fixture("macro_rules! stem{()=>{\"part.rs\"}}include!(stem!());fn main()->i64{made()}");
    let (mut other, mut other_db, other_env, other_authority, _) =
        fixture("macro_rules! other{()=>{\"part.rs\"}}include!(other!());fn main()->i64{made()}");
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::NeedExpansion { .. }
    ));
    assert!(matches!(
        other.poll().unwrap(),
        SourceDiscoveryStep::NeedExpansion { .. }
    ));
    let EagerCompileInputPoll::NeedExternal(request) =
        work.poll_compiler(&mut db, &env, &authority).unwrap()
    else {
        panic!("external operand absent")
    };
    let EagerCompileInputPoll::NeedExternal(wrong) = other
        .poll_compiler(&mut other_db, &other_env, &other_authority)
        .unwrap()
    else {
        panic!("other operand absent")
    };
    assert!(
        work.expand_compiler_string_operand(&mut db, &env, &authority, &wrong)
            .unwrap_err()
            .message
            .contains("differs from current demand")
    );
    let record = work
        .expand_compiler_string_operand(&mut db, &env, &authority, &request)
        .unwrap();
    assert_eq!(record.key(), request.key());
    assert!(
        db.input(&lrr::db::dependency::InputKey::ExpansionRecord(
            record.key().fingerprint()
        ))
        .is_none()
    );
    assert!(
        work.admit_compiler_expansion(&mut db, &env, &authority, Arc::new(record))
            .unwrap_err()
            .message
            .contains("before input adoption")
    );
    assert_eq!(complete(&mut work, &mut db, &env, &authority), 1);
    work.publish(&mut db).unwrap();
}

#[test]
fn recursive_string_chain_requires_each_adopted_producer_before_capture() {
    let (mut work, mut db, env, authority, _) = fixture(
        "macro_rules! leaf{()=>{concat!(\"part\",\".rs\")}}macro_rules! path{()=>{leaf!()}}include!(path!());fn main()->i64{made()}",
    );
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
    let (_, mut changed_db, _, _, _) = fixture_with_environment(
        "macro_rules! leaf{()=>{concat!(\"part\",\".rs\")}}macro_rules! path{()=>{leaf!()}}include!(path!());fn main()->i64{made()}",
        Some("changed"),
    );
    assert!(
        work.publish(&mut changed_db)
            .unwrap_err()
            .message
            .contains("compiler input is stale")
    );
    work.publish(&mut db).unwrap();
}
#[test]
fn recursive_generated_string_chain_joins_the_discovery_seal() {
    let (mut work, mut db, env, authority, _) = fixture(
        "macro_rules! emit{()=>{macro_rules! leaf{()=>{concat!(\"part\",\".rs\")}}macro_rules! path{()=>{leaf!()}}include!(path!());}}emit!();fn main()->i64{made()}",
    );
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    work.publish(&mut db).unwrap();
    assert!(matches!(
        work.lookup("made").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
}

#[test]
fn recursive_environment_name_operand_requires_all_adopted_producers() {
    let source = "macro_rules! leaf{()=>{\"CONTROL\"}}macro_rules! name{()=>{leaf!()}}include!(env!(name!()));fn main()->i64{made()}";
    let (mut work, mut db, env, authority, _) = fixture_with_environment(source, Some("part.rs"));
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(work.key().clone())),
        QueryPoll::Ready(Ok(_))
    ));
    let (_, mut changed, _, _, _) = fixture_with_environment(source, Some("other.rs"));
    assert!(
        work.publish(&mut changed)
            .unwrap_err()
            .message
            .contains("compiler input is stale")
    );
}
#[test]
fn recursive_environment_and_concat_operand_chains_match_native_macos() {
    use lrr::{
        db::value::QueryValue,
        engine::run::{RunMachine, RunPoll, ValueSummary},
    };
    let source = "macro_rules! leaf{()=>{\"CON\"}}macro_rules! name{()=>{leaf!()}}macro_rules! emit{()=>{include!(env!(concat!(name!(),\"TROL\")));}}emit!();fn main()->i64{made()}";
    let (mut work, mut db, env, authority, path) =
        fixture_with_environment(source, Some("part.rs"));
    assert_eq!(
        complete_verifying(&mut work, &mut db, &env, &authority, true),
        1
    );
    let QueryPoll::Ready(Ok(value)) = db.fetch(
        &SyntaxProvider,
        QueryKey::Outline(OutlineKey {
            parse: ParseKey {
                content: Digest::of_bytes(source.as_bytes()),
                profile: ProfileIdentity::current(),
            },
            crate_key: UNIT,
            module_path: vec![],
            cfg: CfgSet::guest_default().fingerprint(),
        }),
    ) else {
        panic!("entry outline absent")
    };
    let QueryValue::Outline(outline) = value.value.as_ref() else {
        panic!("entry outline variant")
    };
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_discovery(&mut db, body, 10_000, work)
    else {
        panic!("guest entry absent")
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("guest did not finish")
    };
    let ValueSummary::Signed(actual) = report.value else {
        panic!("noninteger guest result")
    };
    std::fs::write(path.join("vm.stdout"), &actual).unwrap();
    let version = std::process::Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(
        version_text.contains("release: 1.99.0")
            && version_text.contains("host: aarch64-apple-darwin")
    );
    std::fs::write(path.join("rustc.stdout"), version.stdout).unwrap();
    std::fs::write(
        path.join("oracle.rs"),
        source.replace("fn main()->i64", "fn case()->i64")
            + "\nfn main(){::std::println!(\"{}\",case());}\n",
    )
    .unwrap();
    let built = std::process::Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
        .env("CONTROL", "part.rs")
        .arg(path.join("oracle.rs"))
        .arg("-o")
        .arg(path.join("oracle"))
        .output()
        .unwrap();
    std::fs::write(path.join("compile.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = std::process::Command::new(path.join("oracle"))
        .output()
        .unwrap();
    std::fs::write(path.join("native.stdout"), &native.stdout).unwrap();
    assert!(native.status.success());
    assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
}

#[test]
fn generated_module_source_demand_is_retained_without_capture_or_sealing() {
    let (mut work, _, _, _, _) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let demands = work.unresolved_module_sources().collect::<Vec<_>>();
    assert_eq!(demands.len(), 1);
    let demand = demands[0];
    assert_eq!(demand.scope().module_path, vec!["child"]);
    assert_eq!(demand.request().name, "child");
    assert_eq!(demand.header().expansion, request.key().fingerprint());
    let record = work.expansion_record(request.key()).unwrap();
    assert!(
        record
            .tokens()
            .unwrap()
            .spans()
            .iter()
            .any(|span| span.key() == demand.site())
    );
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn generated_module_source_view_requires_adoption_and_keeps_the_frontier_open() {
    use lrr::service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    };
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "pub fn made()->i64{1667_i64}fn cold(){let _:Missing=missing();}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module declaration producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    assert!(matches!(
        work.module_source_view(&mut db, &input),
        QueryPoll::NeedInput(_)
    ));
    let InputRequest::CaptureModule { request, .. } = &input else {
        panic!("module capture input absent");
    };
    let capture = SourceCapture::open(
        IncludeAuthorities::new(
            UNIT,
            Digest([235; 32]),
            [authority.root(Digest([236; 32])).unwrap().clone()],
        )
        .unwrap(),
        BTreeMap::new(),
    )
    .unwrap();
    let record = capture.capture_module(request).unwrap();
    let context = DemandContext {
        request: Digest([249; 32]),
        session: Digest([250; 32]),
        unit: UNIT,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([235; 32]),
        native_generation: None,
        compile_environment: env.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            QueryKey::Constant(fixture_key(
                Digest([251; 32]),
                vec![],
                Digest::default(),
                ProfileIdentity::current(),
            )),
            PhaseIdentity {
                frame: Digest([252; 32]),
                body: lrr::source::manifest::BodyKey {
                    definition: Digest([253; 32]),
                    exact_body_bytes: Digest([254; 32]),
                },
                index: 0,
            },
            input.clone(),
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept_module(&context, &token, &record)
        .unwrap();
    assert!(matches!(
        work.module_source_view(&mut db, &input),
        QueryPoll::NeedInput(_)
    ));
    coordinator
        .adopt_module(&context, &token, &mut db, record)
        .unwrap();
    let QueryPoll::Ready(Ok(view)) = work.module_source_view(&mut db, &input) else {
        panic!("adopted child view absent");
    };
    assert_eq!(view.outline().root_scope.module_path, vec!["child"]);
    assert_eq!(view.outline().items.len(), 2);
    assert!(
        view.outline()
            .items
            .iter()
            .all(|item| item.scope.module_path == vec!["child"])
    );
    assert_eq!(
        view.parent_header(),
        work.unresolved_module_sources().next().unwrap().header()
    );
    assert_eq!(view.producer(), producer);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_expansions().is_err());
    assert_eq!(view.invocation_base().source.path, "main.rs");
    assert_eq!(view.caller().selection().path.as_str(), "main.rs");
    let old_content = view.capture().record().selected().unwrap().blob().content();
    std::fs::remove_file(path.join("child.rs")).unwrap();
    let QueryPoll::Ready(Ok(retained)) = work.module_source_view(&mut db, &input) else {
        panic!("pure view re-read deleted source");
    };
    assert_eq!(
        retained
            .capture()
            .record()
            .selected()
            .unwrap()
            .blob()
            .content(),
        old_content
    );
    let InputRequest::CaptureModule {
        request: original,
        compile_environment,
    } = &input
    else {
        unreachable!()
    };
    let mut changed_declaration = original.declaration().clone();
    changed_declaration.module_path = vec!["different_child".into()];
    let altered = authority
        .module_request(
            original.candidates()[0].from(),
            std::path::Path::new(""),
            changed_declaration,
            producer,
            work.unresolved_module_sources().next().unwrap().site(),
        )
        .unwrap();
    assert_eq!(altered.candidates(), original.candidates());
    assert_ne!(altered.identity(), original.identity());
    assert!(matches!(
        work.module_source_view(
            &mut db,
            &InputRequest::CaptureModule {
                request: Box::new(altered),
                compile_environment: *compile_environment,
            }
        ),
        QueryPoll::Ready(Err(_))
    ));
    let mut other = input.clone();
    let InputRequest::CaptureModule {
        compile_environment,
        ..
    } = &mut other
    else {
        unreachable!();
    };
    *compile_environment = Digest([255; 32]);
    assert!(matches!(
        work.module_source_view(&mut db, &other),
        QueryPoll::Ready(Err(_))
    ));
}

#[test]
fn generated_module_view_reobserves_caller_and_compile_environment_before_suspending() {
    use lrr::db::dependency::{InputKey, InputValue};
    for changed_source in [false, true] {
        let (mut work, mut db, _, authority, _) =
            fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("producer absent");
        };
        work.admit_expansion(Arc::new(request.expand().unwrap()))
            .unwrap();
        let producer = work.unresolved_module_sources().next().unwrap().producer();
        let input = work
            .prepare_module_source(producer, &authority, std::path::Path::new(""))
            .unwrap();
        let location = SourceSelection {
            path: SourcePath::new("main.rs").unwrap(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: Digest::default(),
                    profile: ProfileIdentity::current(),
                },
                crate_key: UNIT,
                module_path: vec![],
                cfg: Digest::default(),
            },
        }
        .location_identity();
        let source_key = InputKey::CapturedSource(location);
        let mut inputs = BTreeMap::from([
            (
                source_key.clone(),
                db.input(&source_key).unwrap().as_ref().clone(),
            ),
            (
                InputKey::CrateCfg(UNIT),
                db.input(&InputKey::CrateCfg(UNIT))
                    .unwrap()
                    .as_ref()
                    .clone(),
            ),
            (
                InputKey::UnitCompileEnv(UNIT),
                db.input(&InputKey::UnitCompileEnv(UNIT))
                    .unwrap()
                    .as_ref()
                    .clone(),
            ),
        ]);
        if changed_source {
            let InputValue::CapturedSource(original) = inputs.get_mut(&source_key).unwrap() else {
                unreachable!()
            };
            original.outline.parse.content = Digest([255; 32]);
        } else {
            let different = FrozenCompileEnvironment::new(
                UNIT,
                &CompileEnv {
                    unit: UNIT,
                    host: "aarch64-apple-darwin".into(),
                    target: "aarch64-apple-darwin".into(),
                    cfg: CfgSet::guest_default(),
                    cargo: BTreeMap::new(),
                    rustc_env: BTreeMap::from([("CHANGED".into(), "yes".into())]),
                    declared: BTreeMap::new(),
                    out_dir: None,
                },
            )
            .unwrap();
            inputs.insert(
                InputKey::UnitCompileEnv(UNIT),
                InputValue::UnitCompileEnv(Arc::new(different)),
            );
        }
        db.publish_inputs(inputs).unwrap();
        let QueryPoll::Ready(Err(error)) = work.module_source_view(&mut db, &input) else {
            panic!("changed context resumed");
        };
        assert_eq!(
            error.message,
            if changed_source {
                "module caller publication changed"
            } else {
                "module compile environment publication changed"
            }
        );
        assert!(work.sealed_expansions().is_err());
    }
}

#[test]
fn generated_module_view_enters_nested_scope_and_preserves_missing_or_ambiguous_evidence() {
    use lrr::service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    };
    for state in ["present", "absent", "ambiguous"] {
        let (mut work, mut db, env, authority, path) =
            fixture("macro_rules! emit{()=>{mod outer{mod child;}}}emit!();fn main()->i64{0_i64}");
        std::fs::create_dir_all(path.join("outer/child")).unwrap();
        if state != "absent" {
            std::fs::write(path.join("outer/child.rs"), "pub fn made()->i64{1693_i64}").unwrap();
        }
        if state == "ambiguous" {
            std::fs::write(
                path.join("outer/child/mod.rs"),
                "pub fn alternate()->i64{1699_i64}",
            )
            .unwrap();
        }
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("nested module producer absent");
        };
        work.admit_expansion(Arc::new(request.expand().unwrap()))
            .unwrap();
        let producer = work.unresolved_module_sources().next().unwrap().producer();
        let input = work
            .prepare_module_source(producer, &authority, std::path::Path::new(""))
            .unwrap();
        let InputRequest::CaptureModule { request, .. } = &input else {
            unreachable!()
        };
        let capture = SourceCapture::open(authority, BTreeMap::new()).unwrap();
        let record = capture.capture_module(request).unwrap();
        let context = DemandContext {
            request: Digest([249; 32]),
            session: Digest([250; 32]),
            unit: UNIT,
            revision: db.source_revision(),
            epoch: 1,
            source_generation: Digest([235; 32]),
            native_generation: None,
            compile_environment: env.fingerprint(),
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let token = coordinator
            .issue(
                QueryKey::Constant(fixture_key(
                    Digest([251; 32]),
                    vec![],
                    Digest::default(),
                    ProfileIdentity::current(),
                )),
                PhaseIdentity {
                    frame: Digest([252; 32]),
                    body: lrr::source::manifest::BodyKey {
                        definition: Digest([253; 32]),
                        exact_body_bytes: Digest([254; 32]),
                    },
                    index: 0,
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
        match (state, work.module_source_view(&mut db, &input)) {
            ("present", QueryPoll::Ready(Ok(view))) => {
                assert_eq!(view.outline().root_scope.module_path, ["outer", "child"]);
                assert_eq!(view.outline().items.len(), 1);
                assert_eq!(
                    view.capture()
                        .record()
                        .selected()
                        .unwrap()
                        .location()
                        .path
                        .as_str(),
                    "outer/child.rs"
                );
            }
            ("absent", QueryPoll::Ready(Err(error))) => {
                assert_eq!(error.code, "LRR-E-SOURCE-MISSING")
            }
            ("ambiguous", QueryPoll::Ready(Err(error))) => {
                assert_eq!(error.code, "LRR-E-MODULE-AMBIGUOUS")
            }
            _ => panic!("module evidence did not preserve {state}"),
        }
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Incomplete { .. }
        ));
        assert!(work.sealed_expansions().is_err());
    }
}

#[test]
fn adopted_module_parent_lookup_joins_generated_cutoff_and_original_source_context() {
    use lrr::{
        frontend::{macro_origins::MacroSelection, macro_scopes::HygienicLookup},
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            source_capture::SourceCapture,
        },
    };
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! inherited{()=>{pub fn inherited(){1753_i64;}}}macro_rules! emit{()=>{macro_rules! helper{()=>{pub fn earlier(){1759_i64;}}}mod child;macro_rules! helper{()=>{pub fn later(){1777_i64;}}}}}emit!();macro_rules! late{()=>{}}fn main()->i64{0_i64}",
    );
    std::fs::write(path.join("child.rs"), "helper!();inherited!();").unwrap();
    let request = match work.poll().unwrap() {
        SourceDiscoveryStep::NeedExpansion { request } => request,
        SourceDiscoveryStep::Incomplete {
            diagnostic,
            blocked,
        } => panic!("module producer incomplete: {diagnostic:?} {blocked:?}"),
        _ => panic!("module producer absent"),
    };
    let parent = Arc::new(request.expand().unwrap());
    work.admit_expansion(parent).unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    assert!(matches!(
        work.module_parent_macro_lookup(&mut db, &input, "helper"),
        QueryPoll::NeedInput(_)
    ));
    let InputRequest::CaptureModule { request, .. } = &input else {
        unreachable!()
    };
    let capture = SourceCapture::open(authority, BTreeMap::new()).unwrap();
    let record = capture.capture_module(request).unwrap();
    let context = DemandContext {
        request: Digest([249; 32]),
        session: Digest([250; 32]),
        unit: UNIT,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([235; 32]),
        native_generation: None,
        compile_environment: env.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            QueryKey::Constant(fixture_key(
                Digest([251; 32]),
                vec![],
                Digest::default(),
                ProfileIdentity::current(),
            )),
            PhaseIdentity {
                frame: Digest([252; 32]),
                body: lrr::source::manifest::BodyKey {
                    definition: Digest([253; 32]),
                    exact_body_bytes: Digest([254; 32]),
                },
                index: 0,
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
    let QueryPoll::Ready(Ok(view)) = work.module_source_view(&mut db, &input) else {
        panic!("child view absent");
    };
    let helper = view
        .parent()
        .items()
        .iter()
        .find(|h| h.name.as_ref().is_some_and(|n| n.text == "helper"))
        .unwrap();
    let QueryPoll::Ready(Ok(HygienicLookup::Selection(MacroSelection::Defined(selected)))) =
        work.module_parent_macro_lookup(&mut db, &input, "helper")
    else {
        panic!("generated parent helper absent");
    };
    assert_eq!(selected.identity, helper.key.fingerprint());
    assert!(matches!(
        work.module_parent_macro_lookup(&mut db, &input, "inherited"),
        QueryPoll::Ready(Ok(HygienicLookup::Selection(MacroSelection::Defined(_))))
    ));
    assert!(matches!(
        work.module_parent_macro_lookup(&mut db, &input, "late"),
        QueryPoll::Ready(Ok(HygienicLookup::Selection(MacroSelection::Absent)))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn recursive_generated_module_demand_retains_records_and_adopted_ancestor_context() {
    use lrr::{
        frontend::{macro_origins::MacroSelection, macro_scopes::HygienicLookup},
        service::{
            expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
            source_capture::SourceCapture,
        },
    };
    let source = concat!(
        "macro_rules! inherited{()=>{}}macro_rules! emit{()=>{",
        "macro_rules! ancestor{()=>{pub fn original_ancestor(){1783_i64;}}}",
        "macro_rules! first{()=>{",
        "macro_rules! helper{()=>{pub fn earlier_helper(){1787_i64;}}}",
        "macro_rules! second{()=>{mod child;}}second!();",
        "macro_rules! helper{()=>{pub fn later_helper(){1789_i64;}}}",
        "}}first!();",
        "macro_rules! ancestor{()=>{pub fn later_ancestor(){1801_i64;}}}",
        "}}emit!();fn main()->i64{0_i64}",
    );
    let (mut work, mut db, env, authority, path) = fixture(source);
    std::fs::write(
        path.join("child.rs"),
        "helper!();ancestor!();inherited!();fn cold(){let _:Missing=missing();}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("original producer absent");
    };
    let root_record = Arc::new(request.expand().unwrap());
    let root_key = root_record.key().clone();
    work.admit_expansion(root_record).unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("first generated producer absent");
    };
    let middle_record = Arc::new(request.expand().unwrap());
    let middle_key = middle_record.key().clone();
    work.admit_expansion(middle_record).unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("second generated producer absent");
    };
    let module_record = Arc::new(request.expand().unwrap());
    let module_key = module_record.key().clone();
    work.admit_expansion(module_record).unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    assert_eq!(
        work.unresolved_module_sources()
            .next()
            .unwrap()
            .header()
            .expansion,
        module_key.fingerprint()
    );
    assert!(work.expansion_record(&root_key).is_some());
    assert!(work.expansion_record(&middle_key).is_some());
    assert!(work.expansion_record(&module_key).is_some());
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    assert!(matches!(
        work.module_parent_macro_lookup(&mut db, &input, "helper"),
        QueryPoll::NeedInput(_)
    ));
    let InputRequest::CaptureModule { request, .. } = &input else {
        unreachable!()
    };
    let capture = SourceCapture::open(authority, BTreeMap::new()).unwrap();
    let record = capture.capture_module(request).unwrap();
    let context = DemandContext {
        request: Digest([249; 32]),
        session: Digest([250; 32]),
        unit: UNIT,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([235; 32]),
        native_generation: None,
        compile_environment: env.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            QueryKey::Constant(fixture_key(
                Digest([251; 32]),
                vec![],
                Digest::default(),
                ProfileIdentity::current(),
            )),
            PhaseIdentity {
                frame: Digest([252; 32]),
                body: lrr::source::manifest::BodyKey {
                    definition: Digest([253; 32]),
                    exact_body_bytes: Digest([254; 32]),
                },
                index: 0,
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
    let QueryPoll::Ready(Ok(view)) = work.module_source_view(&mut db, &input) else {
        panic!("nested child view absent");
    };
    assert_eq!(view.outline().root_scope.module_path, ["child"]);
    for (name, key) in [("helper", &middle_key), ("ancestor", &root_key)] {
        let record = work.expansion_record(key).unwrap();
        let items =
            lrr::expand::generated_items::GeneratedItems::admit(key, record.clone(), 1).unwrap();
        let expected = items
            .items()
            .iter()
            .find(|h| h.name.as_ref().is_some_and(|n| n.text == name))
            .unwrap();
        let QueryPoll::Ready(Ok(HygienicLookup::Selection(MacroSelection::Defined(selected)))) =
            work.module_parent_macro_lookup(&mut db, &input, name)
        else {
            panic!("{name} ancestor context absent");
        };
        assert_eq!(selected.identity, expected.key.fingerprint());
    }
    assert!(matches!(
        work.module_parent_macro_lookup(&mut db, &input, "inherited"),
        QueryPoll::Ready(Ok(HygienicLookup::Selection(MacroSelection::Defined(_))))
    ));
    let SourceDiscoveryStep::Incomplete { blocked, .. } = work.poll().unwrap() else {
        panic!("recursive module sealed prematurely");
    };
    assert!(blocked.iter().any(|(_, waits)| waits.contains(&producer)));
    assert!(work.sealed_expansions().is_err());
}

fn adopt_fixture_module(
    db: &mut Database,
    env: &FrozenCompileEnvironment,
    authority: IncludeAuthorities,
    input: &InputRequest,
) {
    use lrr::service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    };
    let InputRequest::CaptureModule { request, .. } = input else {
        panic!("module input absent");
    };
    let capture = SourceCapture::open(authority, BTreeMap::new()).unwrap();
    let record = capture.capture_module(request).unwrap();
    let context = DemandContext {
        request: input.fingerprint(),
        session: Digest([250; 32]),
        unit: UNIT,
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([235; 32]),
        native_generation: None,
        compile_environment: env.fingerprint(),
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            QueryKey::Constant(fixture_key(
                Digest([251; 32]),
                vec![],
                Digest::default(),
                ProfileIdentity::current(),
            )),
            PhaseIdentity {
                frame: Digest([252; 32]),
                body: lrr::source::manifest::BodyKey {
                    definition: Digest([253; 32]),
                    exact_body_bytes: Digest([254; 32]),
                },
                index: 0,
            },
            input.clone(),
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept_module(&context, &token, &record)
        .unwrap();
    coordinator
        .adopt_module(&context, &token, db, record)
        .unwrap();
}

#[test]
fn recursive_sibling_module_demands_keep_distinct_scopes_and_parent_cutoffs() {
    use lrr::frontend::{macro_origins::MacroSelection, macro_scopes::HygienicLookup};
    let (mut work, mut db, env, authority, path) = fixture(concat!(
        "macro_rules! emit{()=>{macro_rules! first{()=>{mod child;}}",
        "macro_rules! helper{()=>{pub fn before(){1811_i64;}}}",
        "mod left{first!();}",
        "macro_rules! helper{()=>{pub fn between(){1823_i64;}}}",
        "mod right{first!();}}}emit!();fn main()->i64{0_i64}",
    ));
    for name in ["left", "right"] {
        std::fs::create_dir_all(path.join(name)).unwrap();
        std::fs::write(path.join(name).join("child.rs"), "helper!();").unwrap();
    }
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("root producer absent");
    };
    let root_key = request.key().clone();
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    for _ in 0..2 {
        let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
            panic!("sibling producer absent");
        };
        work.admit_expansion(Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    let demands = work
        .unresolved_module_sources()
        .map(|d| (d.producer(), d.scope().module_path.clone()))
        .collect::<Vec<_>>();
    assert_eq!(demands.len(), 2);
    assert_ne!(demands[0].0, demands[1].0);
    let root_items = lrr::expand::generated_items::GeneratedItems::admit(
        &root_key,
        work.expansion_record(&root_key).unwrap().clone(),
        1,
    )
    .unwrap();
    let helpers = root_items
        .items()
        .iter()
        .filter(|h| h.name.as_ref().is_some_and(|n| n.text == "helper"))
        .collect::<Vec<_>>();
    assert_eq!(helpers.len(), 2);
    for (producer, module) in demands {
        let name = module[0].as_str();
        let input = work
            .prepare_module_source(producer, &authority, std::path::Path::new(name))
            .unwrap();
        let duplicate_authority = IncludeAuthorities::new(
            UNIT,
            Digest([235; 32]),
            [authority.root(Digest([236; 32])).unwrap().clone()],
        )
        .unwrap();
        adopt_fixture_module(&mut db, &env, duplicate_authority, &input);
        let QueryPoll::Ready(Ok(view)) = work.module_source_view(&mut db, &input) else {
            panic!("sibling child absent");
        };
        assert_eq!(view.outline().root_scope.module_path, module);
        assert_eq!(
            view.capture()
                .record()
                .selected()
                .unwrap()
                .location()
                .path
                .as_str(),
            format!("{name}/child.rs")
        );
        let QueryPoll::Ready(Ok(HygienicLookup::Selection(MacroSelection::Defined(selected)))) =
            work.module_parent_macro_lookup(&mut db, &input, "helper")
        else {
            panic!("sibling helper absent");
        };
        let index = match name {
            "left" => 0,
            "right" => 1,
            _ => panic!("unexpected module"),
        };
        assert_eq!(selected.identity, helpers[index].key.fingerprint());
    }
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(work.unresolved_module_sources().count(), 2);
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn recursive_module_unknown_attribute_retains_tokens_and_blocks_discovery() {
    let (mut work, _, _, _, _) = fixture(
        "macro_rules! emit{()=>{macro_rules! nested{()=>{#[opaque]mod child;}}nested!();}}emit!();fn main()->i64{0_i64}",
    );
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("root producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("nested producer absent");
    };
    let key = request.key().clone();
    let record = Arc::new(request.expand().unwrap());
    work.admit_expansion(record).unwrap();
    assert_eq!(work.unresolved_module_sources().count(), 0);
    assert!(work.expansion_record(&key).is_some());
    let SourceDiscoveryStep::Incomplete { blocked, .. } = work.poll().unwrap() else {
        panic!("unknown generated attribute completed");
    };
    assert!(!blocked.is_empty());
    let SourceDiscoveryStep::Incomplete {
        blocked: replay, ..
    } = work.poll().unwrap()
    else {
        panic!("pending attribute producer was discarded");
    };
    assert_eq!(blocked, replay);
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn adopted_module_syntax_projects_exact_declarations_without_publication_or_body_lowering() {
    use lrr::db::dependency::{InputKey, InputValue};
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "pub fn made()->i64{1831_i64}fn cold(){let _:Missing=missing();}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    assert!(matches!(
        work.module_syntax_input(&mut db, &input),
        QueryPoll::NeedInput(_)
    ));
    adopt_fixture_module(&mut db, &env, authority, &input);
    let revision = db.source_revision();
    let QueryPoll::Ready(Ok(syntax)) = work.module_syntax_input(&mut db, &input) else {
        panic!("adopted syntax absent");
    };
    let prepared = syntax.prepared_source();
    assert_eq!(prepared.selection().path.as_str(), "child.rs");
    assert_eq!(prepared.outline().root_scope.module_path, ["child"]);
    assert!(std::ptr::eq(prepared.outline(), syntax.origin().outline()));
    assert_eq!(prepared.outline().items.len(), 2);
    for item in &prepared.outline().items {
        let id = item.def_key.fingerprint();
        assert!(
            matches!(syntax.inputs().get(&InputKey::DefinitionHeader(id)), Some(InputValue::DefinitionHeader(header)) if header == &item.header)
        );
        assert!(
            matches!(syntax.inputs().get(&InputKey::DefinitionSource(id)), Some(InputValue::DefinitionSource(key)) if key == &prepared.selection().outline.parse)
        );
        assert!(
            matches!(syntax.inputs().get(&InputKey::DeclarationContext(id)), Some(InputValue::DeclarationContext(context)) if context.scope == item.scope && context.range == item.declaration_span)
        );
        assert!(
            matches!(syntax.inputs().get(&InputKey::DefinitionBody(id)), Some(InputValue::DefinitionBody(body)) if body == item.body.as_ref().unwrap())
        );
        assert!(db.input(&InputKey::DefinitionHeader(id)).is_none());
    }
    let made = prepared
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "made")
        .unwrap();
    prepared
        .index_body(
            prepared.selection(),
            &made.body.as_ref().unwrap().key(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(db.source_revision(), revision);
    assert!(work.publish(&mut db).is_err());
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
}

#[test]
fn module_syntax_identity_binds_provenance_and_logical_scope_for_identical_bytes() {
    let mut results = Vec::new();
    for source in [
        "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}",
        "macro_rules! renamed{()=>{mod child;}}renamed!();fn main()->i64{0_i64}",
        "macro_rules! emit{()=>{#[path=\"child.rs\"]mod alternate;}}emit!();fn main()->i64{0_i64}",
    ] {
        let (mut work, mut db, env, authority, path) = fixture(source);
        std::fs::write(path.join("child.rs"), "pub fn made()->i64{1831_i64}").unwrap();
        let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
            panic!("module producer absent");
        };
        work.admit_expansion(Arc::new(request.expand().unwrap()))
            .unwrap();
        let producer = work.unresolved_module_sources().next().unwrap().producer();
        let input = work
            .prepare_module_source(producer, &authority, std::path::Path::new(""))
            .unwrap();
        adopt_fixture_module(&mut db, &env, authority, &input);
        let QueryPoll::Ready(Ok(syntax)) = work.module_syntax_input(&mut db, &input) else {
            panic!("syntax projection absent");
        };
        let QueryPoll::Ready(Ok(replay)) = work.module_syntax_input(&mut db, &input) else {
            panic!("syntax replay absent");
        };
        assert_eq!(syntax.identity(), replay.identity());
        results.push((
            syntax.identity(),
            syntax.prepared_source().selection().clone(),
            syntax.prepared_source().outline().items[0]
                .def_key
                .fingerprint(),
        ));
    }
    assert_ne!(results[0].0, results[1].0);
    assert_ne!(results[0].0, results[2].0);
    assert_eq!(results[0].1.outline.parse, results[1].1.outline.parse);
    assert_eq!(results[0].1.outline.parse, results[2].1.outline.parse);
    assert_eq!(results[0].2, results[1].2);
    assert_ne!(results[0].2, results[2].2);
}

#[test]
fn module_syntax_projection_refuses_unreliable_declarations_after_adoption() {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(path.join("child.rs"), "pub fn made()->i64{1831_i64} ?").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    assert!(matches!(
        work.module_source_view(&mut db, &input),
        QueryPoll::Ready(Ok(_))
    ));
    let QueryPoll::Ready(Err(error)) = work.module_syntax_input(&mut db, &input) else {
        panic!("unreliable declarations admitted");
    };
    assert_eq!(error.code, "LRR-E-PARSE-BLOCKS-ENTRY");
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn ordinary_source_discovery_refuses_parse_errors_outside_indexed_items() {
    let source = "fn main()->i64{1831_i64} ?";
    let cfg = CfgSet::guest_default();
    let profile = ProfileIdentity::current();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: UNIT,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: UNIT,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent");
    };
    let scope = view.outline().root_scope.clone();
    let error = match SourceDiscovery::new(
        vec![view],
        scope,
        lrr::source::manifest::Namespace::Value,
        Digest([233; 32]),
        1,
    ) {
        Err(error) => error,
        Ok(_) => panic!("unreliable root declarations admitted"),
    };
    assert_eq!(error.code, "LRR-E-PARSE-BLOCKS-ENTRY");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m013-module-syntax-native-{}",
            std::process::id()
        ));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("oracle.rs"), source).unwrap();
    let native = std::process::Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=lib",
            "--crate-name=oracle",
        ])
        .arg(path.join("oracle.rs"))
        .arg("-o")
        .arg(path.join("oracle.rlib"))
        .output()
        .unwrap();
    std::fs::write(path.join("compile.stderr"), &native.stderr).unwrap();
    assert!(!native.status.success());
    let stderr = String::from_utf8(native.stderr).unwrap();
    assert!(
        stderr.contains("expected item") && stderr.contains('?'),
        "{stderr}"
    );
    std::fs::write(
        path.join("comparison.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "native_exit_code": native.status.code(), "native_syntax_rejected": true,
            "lrr_diagnostic": error.code, "comparison_passed": true,
            "scope": "Root declaration syntax rejection only; no child LRR execution claim",
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn module_syntax_projection_keeps_parse_errors_inside_cold_bodies_lazy() {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "pub fn made()->i64{1831_i64}fn cold(){let broken = ;}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.module_syntax_input(&mut db, &input) else {
        panic!("cold body error blocked declarations");
    };
    let cold = syntax
        .prepared_source()
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "cold")
        .unwrap();
    let range = cold.body.as_ref().unwrap().range;
    assert!(!syntax.origin().parsed().errors.is_empty());
    assert!(
        syntax
            .origin()
            .parsed()
            .errors
            .iter()
            .all(|error| error.range.start >= range.start && error.range.end <= range.end)
    );
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn shared_module_prepared_source_keeps_adopted_and_caller_bytes_accounted() {
    let source = "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}";
    let child = "pub fn made()->i64{1831_i64}";
    let (mut work, mut db, env, authority, path) = fixture(source);
    std::fs::write(path.join("child.rs"), child).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.module_syntax_input(&mut db, &input) else {
        panic!("module syntax absent");
    };
    let shared = syntax.prepared_source().share();
    drop(syntax);
    drop(work);
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert_eq!(
        db.cache_usage().unwrap().source_input_bytes,
        source.len() + child.len()
    );
    assert_eq!(shared.outline().root_scope.module_path, ["child"]);
    drop(shared);
    assert_eq!(db.cache_usage().unwrap().source_input_bytes, 0);
}

#[test]
fn adopted_module_staging_registers_real_child_jobs_without_sealing_or_publication() {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "use crate::main as imported;pub fn made()->i64{1871_i64}unknown!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    assert!(matches!(
        work.stage_module_source(&mut db, &input),
        QueryPoll::NeedInput(_)
    ));
    assert!(work.module_source_progress(producer).is_none());
    adopt_fixture_module(&mut db, &env, authority, &input);
    let revision = db.source_revision();
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent");
    };
    let before = work.module_source_progress(producer).unwrap();
    assert!(!before.parsed);
    assert_eq!(before.scheduled_imports, 1);
    assert_eq!(before.scheduled_producers, 1);
    let call = syntax.prepared_source().outline().item_invocations[0].item_input;
    let SourceDiscoveryStep::Incomplete { blocked, .. } = work.poll().unwrap() else {
        panic!("unfinished module sealed");
    };
    assert!(blocked.iter().any(|(_, waiting)| waiting.contains(&call)));
    assert!(work.module_source_progress(producer).unwrap().parsed);
    assert_eq!(
        work.module_source_progress(producer).unwrap().declarations,
        syntax.prepared_source().outline().items.len()
    );
    let QueryPoll::Ready(Ok(replay)) = work.stage_module_source(&mut db, &input) else {
        panic!("staging replay refused");
    };
    assert!(Arc::ptr_eq(&syntax, &replay));
    assert_eq!(db.source_revision(), revision);
    for item in &syntax.prepared_source().outline().items {
        assert!(
            db.input(&lrr::db::dependency::InputKey::DefinitionHeader(
                item.def_key.fingerprint()
            ))
            .is_none()
        );
    }
    assert!(work.sealed_expansions().is_err());
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn module_staging_revalidates_stale_environment_without_installing_a_child() {
    use lrr::db::dependency::{InputKey, InputValue};
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(path.join("child.rs"), "pub fn made()->i64{1871_i64}").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let location = SourceSelection {
        path: SourcePath::new("main.rs").unwrap(),
        outline: OutlineKey {
            parse: ParseKey {
                content: Digest::default(),
                profile: ProfileIdentity::current(),
            },
            crate_key: UNIT,
            module_path: vec![],
            cfg: Digest::default(),
        },
    }
    .location_identity();
    let source_key = InputKey::CapturedSource(location);
    let different = FrozenCompileEnvironment::new(
        UNIT,
        &CompileEnv {
            unit: UNIT,
            host: "aarch64-apple-darwin".into(),
            target: "aarch64-apple-darwin".into(),
            cfg: CfgSet::guest_default(),
            cargo: BTreeMap::new(),
            rustc_env: BTreeMap::from([("CHANGED".into(), "yes".into())]),
            declared: BTreeMap::new(),
            out_dir: None,
        },
    )
    .unwrap();
    let replacement = BTreeMap::from([
        (
            source_key.clone(),
            db.input(&source_key).unwrap().as_ref().clone(),
        ),
        (
            InputKey::CrateCfg(UNIT),
            db.input(&InputKey::CrateCfg(UNIT))
                .unwrap()
                .as_ref()
                .clone(),
        ),
        (
            InputKey::UnitCompileEnv(UNIT),
            InputValue::UnitCompileEnv(Arc::new(different)),
        ),
    ]);
    db.publish_inputs(replacement).unwrap();
    let QueryPoll::Ready(Err(error)) = work.stage_module_source(&mut db, &input) else {
        panic!("stale module staged");
    };
    assert_eq!(
        error.message,
        "module compile environment publication changed"
    );
    assert!(work.module_source_progress(producer).is_none());
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn staged_child_import_preserves_actual_generated_parent_namespace_entries() {
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{pub fn generated()->i64{1889_i64}mod child;}}emit!();fn main()->i64{0_i64}",
    );
    std::fs::write(
        path.join("child.rs"),
        "use crate::generated as provided;pub fn made()->i64{provided()}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("module producer absent");
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    assert!(matches!(
        work.stage_module_source(&mut db, &input),
        QueryPoll::Ready(Ok(_))
    ));
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_import_facets,
        0
    );
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let progress = work.module_source_progress(producer).unwrap();
    assert!(progress.parsed);
    assert_eq!(progress.scheduled_imports, 1);
    assert_eq!(progress.completed_import_facets, 1);
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn staged_module_macro_request_preserves_local_tokens_and_cold_bodies() {
    use lrr::expand::source_declarative::SourceDeclarativeDemand;
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{macro_rules! helper{($n:ident,$v:literal)=>{pub fn wrong(){0_i64;}}}mod child;}}emit!();fn main()->i64{0_i64}",
    );
    std::fs::write(path.join("child.rs"), "macro_rules! helper{($n:ident,$v:literal)=>{pub fn $n(){ $v; }}}helper!(selected,1901_i64);fn cold(){let _:Missing=missing();}").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let call = syntax.prepared_source().outline().item_invocations[0].item_input;
    let revision = db.source_revision();
    let QueryPoll::Ready(Ok(SourceDeclarativeDemand::Ready(request))) =
        work.module_macro_demand(&mut db, &input, call)
    else {
        panic!("local request absent")
    };
    assert_eq!(request.key().invocation, call);
    assert_eq!(request.key().consumer_compile_env, env.fingerprint());
    assert_eq!(request.key().call_edition, 2024);
    let record = request.expand().unwrap();
    let tokens = record.tokens().unwrap();
    let rendered = module_request_words(&tokens);
    assert!(rendered.contains(&"selected".into()), "{rendered:?}");
    assert!(rendered.contains(&"1901_i64".into()), "{rendered:?}");
    assert!(!rendered.contains(&"wrong".into()), "{rendered:?}");
    assert_eq!(db.source_revision(), revision);
    assert!(work.sealed_expansions().is_err());
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn staged_module_macro_request_uses_parent_cutoff_and_suspends_later_calls() {
    use lrr::expand::source_declarative::SourceDeclarativeDemand;
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{macro_rules! helper{($v:literal)=>{pub fn earlier(){ $v; }}}mod child;macro_rules! helper{($v:literal)=>{pub fn later(){ $v; }}}}}emit!();fn main()->i64{0_i64}",
    );
    std::fs::write(
        path.join("child.rs"),
        "helper!(1907_i64);helper!(1913_i64);",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let calls = &syntax.prepared_source().outline().item_invocations;
    let QueryPoll::Ready(Ok(SourceDeclarativeDemand::Ready(request))) =
        work.module_macro_demand(&mut db, &input, calls[0].item_input)
    else {
        panic!("parent request absent")
    };
    let record = request.expand().unwrap();
    let rendered = module_request_words(&record.tokens().unwrap());
    assert!(rendered.contains(&"earlier".into()), "{rendered:?}");
    assert!(rendered.contains(&"1907_i64".into()), "{rendered:?}");
    assert!(!rendered.contains(&"later".into()), "{rendered:?}");
    assert!(
        matches!(work.module_macro_demand(&mut db, &input, calls[1].item_input),
        QueryPoll::Ready(Ok(SourceDeclarativeDemand::NeedInput(id))) if id == calls[0].item_input)
    );
    assert!(matches!(
        work.module_macro_demand(&mut db, &input, Digest([42; 32])),
        QueryPoll::Ready(Err(_))
    ));
}

fn module_request_words(tokens: &lrr::syntax::token_wire::TokenForest) -> Vec<String> {
    use lrr::syntax::token_wire::TokenTree;
    let mut stack = tokens.roots().iter().collect::<Vec<_>>();
    let mut words = Vec::new();
    while let Some(token) = stack.pop() {
        match token {
            TokenTree::Ident { text, .. } => words.push(text.clone()),
            TokenTree::Literal { token_text, .. } => words.push(token_text.clone()),
            TokenTree::Group { children, .. } => stack.extend(children),
            _ => {}
        }
    }
    words
}

#[test]
fn staged_module_macro_request_owns_original_parent_tokens_after_input_replacement() {
    use lrr::expand::source_declarative::SourceDeclarativeDemand;
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! inherited{($n:ident)=>{pub fn $n(){1919_i64;}}}macro_rules! emit{()=>{mod child;}}emit!();macro_rules! late{()=>{}}fn main()->i64{0_i64}",
    );
    std::fs::write(path.join("child.rs"), "inherited!(from_original);").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let call = syntax.prepared_source().outline().item_invocations[0].item_input;
    let QueryPoll::Ready(Ok(SourceDeclarativeDemand::Ready(request))) =
        work.module_macro_demand(&mut db, &input, call)
    else {
        panic!("inherited request absent")
    };
    drop(syntax);
    drop(work);
    db.publish_inputs(BTreeMap::new()).unwrap();
    let words = module_request_words(&request.expand().unwrap().tokens().unwrap());
    assert!(words.contains(&"from_original".into()));
    assert!(words.contains(&"1919_i64".into()));
}

#[test]
fn staged_module_macro_request_keeps_annotated_producer_pending() {
    use lrr::expand::source_declarative::SourceDeclarativeDemand;
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! helper{()=>{pub fn made(){}}}#[unknown]helper!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let call = &syntax.prepared_source().outline().item_invocations[0];
    let expected = call.attribute_input.expect("attribute producer absent");
    assert!(
        matches!(work.module_macro_demand(&mut db, &input, call.item_input),
        QueryPoll::Ready(Ok(SourceDeclarativeDemand::NeedInput(id))) if id == expected)
    );
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn staged_module_worklist_admits_exact_reply_and_advances_later_calls() {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(path.join("child.rs"), "macro_rules! helper{($n:ident,$v:literal)=>{pub fn $n(){ $v; }}}helper!(first,1931_i64);helper!(second,1933_i64);").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let calls = &syntax.prepared_source().outline().item_invocations;
    let revision = db.source_revision();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("child request absent")
    };
    assert_eq!(request.key().invocation, calls[0].item_input);
    let SourceDiscoveryStep::NeedExpansion { request: replay } = work.poll().unwrap() else {
        panic!("child replay absent")
    };
    assert!(Arc::ptr_eq(&request, &replay));
    let record = Arc::new(request.expand().unwrap());
    let mut wrong_key = record.key().clone();
    wrong_key.input_tokens = Digest([37; 32]);
    let wrong = Arc::new(
        lrr::expand::record::ExpansionRecord::from_tokens(
            wrong_key.clone(),
            &record.tokens().unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        work.admit_expansion(wrong).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    assert!(work.expansion_record(&wrong_key).is_none());
    work.admit_expansion(record.clone()).unwrap();
    assert!(Arc::ptr_eq(
        &record,
        &work.expansion_record(record.key()).unwrap()
    ));
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        1
    );
    assert!(work.admit_expansion(record).is_err());
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("later child request absent")
    };
    assert_eq!(request.key().invocation, calls[1].item_input);
    let words = module_request_words(&request.expand().unwrap().tokens().unwrap());
    assert!(words.contains(&"second".into()));
    assert!(words.contains(&"1933_i64".into()));
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        2
    );
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_expansions().is_err());
    assert!(work.publish(&mut db).is_err());
    assert_eq!(db.source_revision(), revision);
    assert!(
        work.unresolved_module_sources()
            .any(|d| d.producer() == producer)
    );
}

#[test]
fn staged_module_admitted_macro_definition_resolves_next_physical_call() {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(path.join("child.rs"), "macro_rules! define{()=>{macro_rules! made{()=>{pub fn generated(){1949_i64;}}}}}define!();made!();").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("define request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("generated definition request absent")
    };
    assert_eq!(
        request.key().invocation,
        syntax.prepared_source().outline().item_invocations[1].item_input
    );
    let record = Arc::new(request.expand().unwrap());
    let words = module_request_words(&record.tokens().unwrap());
    assert!(words.contains(&"generated".into()));
    assert!(words.contains(&"1949_i64".into()));
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn staged_module_admission_keeps_generated_unknown_and_module_frontiers() {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! produce{()=>{pub fn known(){1951_i64;}unknown!();}}produce!();produce!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("child request absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let key = record.key().clone();
    work.admit_expansion(record).unwrap();
    assert!(work.expansion_record(&key).is_some());
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        0
    );
    let SourceDiscoveryStep::Incomplete { blocked, .. } = work.poll().unwrap() else {
        panic!("unresolved child work completed")
    };
    assert!(blocked.iter().any(|(_, waiting)| {
        waiting.contains(&syntax.prepared_source().outline().item_invocations[0].item_input)
    }));
    assert!(work.publish(&mut db).is_err());
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn staged_module_unknown_attribute_retains_tokens_without_sealing_or_publication() {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! produce{()=>{#[unknown]pub fn pending(){1957_i64;}}}produce!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(_)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("child request absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let key = record.key().clone();
    work.admit_expansion(record).unwrap();
    assert!(work.expansion_record(&key).is_some());
    let SourceDiscoveryStep::Incomplete { blocked, .. } = work.poll().unwrap() else {
        panic!("unknown staged attribute completed")
    };
    assert!(!blocked.is_empty());
    let SourceDiscoveryStep::Incomplete {
        blocked: replay, ..
    } = work.poll().unwrap()
    else {
        panic!("pending staged attribute producer was discarded")
    };
    assert_eq!(blocked, replay);
    assert!(work.sealed_expansions().is_err());
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        0
    );
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn physical_module_recursive_generated_calls_keep_local_and_original_owners() {
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! inherited{()=>{pub fn inherited_result(){1973_i64;}}}macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}",
    );
    std::fs::write(path.join("child.rs"), "macro_rules! local{()=>{pub fn local_result(){1979_i64;}}}macro_rules! produce{()=>{macro_rules! nested{()=>{local!();inherited!();}}nested!();}}produce!();local!();").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("physical producer request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let mut words = Vec::new();
    for _ in 0..3 {
        let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
            panic!("recursive generated demand absent")
        };
        let record = Arc::new(request.expand().unwrap());
        words.extend(module_request_words(&record.tokens().unwrap()));
        work.admit_expansion(record).unwrap();
    }
    assert!(words.contains(&"local_result".into()));
    assert!(words.contains(&"1979_i64".into()));
    assert!(words.contains(&"inherited_result".into()));
    assert!(words.contains(&"1973_i64".into()));
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("later physical call did not resume")
    };
    assert_eq!(
        request.key().invocation,
        syntax.prepared_source().outline().item_invocations[1].item_input
    );
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        1
    );
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        2
    );
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn physical_module_generated_call_uses_generated_parent_cutoff_and_exact_reply() {
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{macro_rules! helper{()=>{pub fn earlier_result(){1987_i64;}}}mod child;macro_rules! helper{()=>{pub fn later_result(){1993_i64;}}}}}emit!();fn main()->i64{0_i64}",
    );
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! produce{()=>{helper!();}}produce!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(_)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("physical request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("inherited generated demand absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let words = module_request_words(&record.tokens().unwrap());
    assert!(words.contains(&"earlier_result".into()));
    assert!(words.contains(&"1987_i64".into()));
    assert!(!words.contains(&"later_result".into()));
    let mut wrong_key = record.key().clone();
    wrong_key.consumer_compile_env = Digest([91; 32]);
    let wrong = Arc::new(
        lrr::expand::record::ExpansionRecord::from_tokens(
            wrong_key.clone(),
            &record.tokens().unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        work.admit_expansion(wrong).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    assert!(work.expansion_record(&wrong_key).is_none());
    let SourceDiscoveryStep::NeedGeneratedExpansion { request: replay } = work.poll().unwrap()
    else {
        panic!("generated replay absent")
    };
    assert!(Arc::ptr_eq(&request, &replay));
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        1
    );
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn physical_module_generated_call_retains_generated_ancestor_position() {
    let (mut work, mut db, env, authority, path) = fixture(concat!(
        "macro_rules! emit{()=>{",
        "macro_rules! ancestor{()=>{pub fn before_nested_module(){1997_i64;}}}",
        "macro_rules! level{()=>{mod child;}}level!();",
        "macro_rules! ancestor{()=>{pub fn after_nested_module(){1999_i64;}}}",
        "}}emit!();fn main()->i64{0_i64}",
    ));
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! produce{()=>{ancestor!();}}produce!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("module ancestor request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    assert!(matches!(work.stage_module_source(&mut db, &input),
        QueryPoll::Ready(Err(error)) if error.code == "LRR-E-DISCOVERY-TICKET"));
    assert!(work.module_source_progress(producer).is_none());
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    match work.stage_module_source(&mut db, &input) {
        QueryPoll::Ready(Ok(_)) => {}
        QueryPoll::Ready(Err(error)) => panic!("module staging error: {error:?}"),
        QueryPoll::NeedInput(_) => panic!("module staging requires unadopted input"),
    }
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("physical producer request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("ancestor generated request absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let words = module_request_words(&record.tokens().unwrap());
    assert!(words.contains(&"before_nested_module".into()));
    assert!(words.contains(&"1997_i64".into()));
    assert!(!words.contains(&"after_nested_module".into()));
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_invocations,
        1
    );
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn module_source_readiness_waits_for_parse_then_retains_cold_source_without_sealing() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "pub fn cold(){let _:Missing=missing();}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let parse =
        QueryKey::Outline(syntax.prepared_source().selection().outline.clone()).fingerprint();
    assert!(matches!(work.module_source_readiness(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Pending { waiting })) if waiting == [parse]));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let revision = db.source_revision();
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(ready))) =
        work.module_source_readiness(&mut db, &input)
    else {
        panic!("source readiness absent")
    };
    assert_eq!(ready.syntax().identity(), syntax.identity());
    assert_eq!(
        ready.macros().source(),
        syntax.prepared_source().selection()
    );
    assert_eq!(ready.namespace(), lrr::source::manifest::Namespace::Value);
    assert!(ready.records().is_empty());
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(replay))) =
        work.module_source_readiness(&mut db, &input)
    else {
        panic!("readiness replay absent")
    };
    assert_eq!(ready.identity(), replay.identity());
    assert_eq!(db.source_revision(), revision);
    assert!(
        work.unresolved_module_sources()
            .any(|d| d.producer() == producer)
    );
    assert!(work.publish(&mut db).is_err());
    drop(replay);
    drop(syntax);
    drop(work);
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(db.cache_usage().unwrap().source_input_bytes > 0);
    assert_eq!(
        ready
            .syntax()
            .prepared_source()
            .outline()
            .root_scope
            .module_path,
        ["child"]
    );
    drop(ready);
    assert_eq!(db.cache_usage().unwrap().source_input_bytes, 0);
}

#[test]
fn module_source_readiness_revalidates_real_recursive_records_and_exact_pending_inputs() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{macro_rules! helper{()=>{pub fn made(){2003_i64;}}}mod child;}}emit!();fn main()->i64{0_i64}",
    );
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! produce{()=>{helper!();}}produce!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    let call = syntax.prepared_source().outline().item_invocations[0].item_input;
    assert!(matches!(work.module_source_readiness(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Pending { waiting })) if waiting.contains(&call)));
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("physical request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("recursive request absent")
    };
    assert!(matches!(work.module_source_readiness(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Pending { waiting })) if waiting.contains(&call)));
    let record = Arc::new(request.expand().unwrap());
    let descriptor = record.descriptor().fingerprint();
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(ready))) =
        work.module_source_readiness(&mut db, &input)
    else {
        panic!("completed recursive source not ready")
    };
    assert_eq!(ready.records().len(), 2);
    assert!(
        ready
            .records()
            .iter()
            .any(|record| record.fingerprint() == descriptor)
    );
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn module_source_readiness_preserves_unknown_producer_and_import_obligations() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(path.join("child.rs"), "use crate::missing;unknown!();").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let call = syntax.prepared_source().outline().item_invocations[0].item_input;
    let import = lrr::frontend::names::NameNamespaces::import_identity(
        &syntax.prepared_source().outline().imports[0],
    );
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Pending { waiting })) =
        work.module_source_readiness(&mut db, &input)
    else {
        panic!("unresolved source considered ready")
    };
    assert!(waiting.contains(&call));
    assert!(waiting.contains(&import));
    assert!(waiting.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(work.publish(&mut db).is_err());
    db.publish_inputs(BTreeMap::new()).unwrap();
    match work.module_source_readiness(&mut db, &input) {
        QueryPoll::Ready(Err(error)) => {
            assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
            assert_eq!(error.message, "module caller publication absent");
        }
        _ => panic!("withdrawn input outcome differs"),
    }
}

#[test]
fn module_source_readiness_rejects_environment_change_after_positive_readiness() {
    use lrr::{
        db::dependency::{InputKey, InputValue},
        expand::source_discovery::ModuleSourceReadiness,
    };
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "use crate::main as imported;pub fn made(){2009_i64;}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(ready))) =
        work.module_source_readiness(&mut db, &input)
    else {
        panic!("known value import not ready")
    };
    assert_eq!(
        work.module_source_progress(producer)
            .unwrap()
            .completed_import_facets,
        1
    );
    assert_eq!(ready.namespace(), lrr::source::manifest::Namespace::Value);
    let location = syntax.origin().caller().selection().location_identity();
    let source = InputKey::CapturedSource(location);
    let changed = FrozenCompileEnvironment::new(
        UNIT,
        &CompileEnv {
            unit: UNIT,
            host: "aarch64-apple-darwin".into(),
            target: "aarch64-apple-darwin".into(),
            cfg: CfgSet::guest_default(),
            cargo: BTreeMap::new(),
            rustc_env: BTreeMap::from([("CHANGED".into(), "yes".into())]),
            declared: BTreeMap::new(),
            out_dir: None,
        },
    )
    .unwrap();
    let replacement = BTreeMap::from([
        (source.clone(), db.input(&source).unwrap().as_ref().clone()),
        (
            InputKey::CrateCfg(UNIT),
            db.input(&InputKey::CrateCfg(UNIT))
                .unwrap()
                .as_ref()
                .clone(),
        ),
        (
            InputKey::UnitCompileEnv(UNIT),
            InputValue::UnitCompileEnv(Arc::new(changed)),
        ),
    ]);
    db.publish_inputs(replacement).unwrap();
    let QueryPoll::Ready(Err(error)) = work.module_source_readiness(&mut db, &input) else {
        panic!("stale readiness replayed")
    };
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(
        error.message,
        "module compile environment publication changed"
    );
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn module_catalogue_macro_use_expands_actual_physical_body_in_generated_parent() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{#[macro_use]mod child;physical!(from_parent,2011_i64);}}emit!();fn main()->i64{0_i64}",
    );
    std::fs::write(path.join("child.rs"), "macro_rules! physical{($n:ident,$v:literal)=>{pub fn $n(){ $v; }}}fn cold(){let _:Missing=missing();}").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(_)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let revision = db.source_revision();
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(ready))) =
        work.admit_module_macro_catalogue(&mut db, &input)
    else {
        panic!("module catalogue refused")
    };
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("macro-use generated call remained blocked")
    };
    let record = Arc::new(request.expand().unwrap());
    let words = module_request_words(&record.tokens().unwrap());
    assert!(words.contains(&"from_parent".into()));
    assert!(words.contains(&"2011_i64".into()));
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(replay))) =
        work.admit_module_macro_catalogue(&mut db, &input)
    else {
        panic!("completed catalogue replay refused")
    };
    assert_eq!(ready.identity(), replay.identity());
    assert_eq!(db.source_revision(), revision);
    assert!(work.publish(&mut db).is_err());
    assert!(
        work.unresolved_module_sources()
            .any(|d| d.producer() == producer)
    );
}

#[test]
fn completed_module_catalogue_macro_use_reaches_later_original_call() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{#[macro_use]mod child;}}emit!();physical!(later_original,2017_i64);fn main()->i64{0_i64}",
    );
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! physical{($n:ident,$v:literal)=>{pub fn $n(){ $v; }}}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(_)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.admit_module_macro_catalogue(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("later original macro-use call absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let words = module_request_words(&record.tokens().unwrap());
    assert!(words.contains(&"later_original".into()));
    assert!(words.contains(&"2017_i64".into()));
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.module_source_readiness(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn module_catalogue_keeps_unknown_source_pending() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();private!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! private{()=>{pub fn hidden(){2027_i64;}}}unknown!();",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let call = syntax.prepared_source().outline().item_invocations[0].item_input;
    assert!(matches!(work.admit_module_macro_catalogue(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Pending { waiting })) if waiting.contains(&call)));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn completed_plain_module_catalogue_does_not_export_child_textual_macros() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();private!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "macro_rules! private{()=>{pub fn hidden(){2029_i64;}}}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(_)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.admit_module_macro_catalogue(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.module_parent_macro_lookup(&mut db, &input, "private"),
        QueryPoll::Ready(Ok(lrr::frontend::macro_scopes::HygienicLookup::Selection(
            lrr::frontend::macro_origins::MacroSelection::Absent
        )))
    ));
    assert!(matches!(
        work.module_source_readiness(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn module_namespace_completion_retires_proven_frontier_and_publishes_actual_child_inputs() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(
        path.join("child.rs"),
        "pub fn made()->i64{2039_i64}fn cold(){let _:Missing=missing();}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let revision = db.source_revision();
    assert!(matches!(
        work.complete_module_namespace(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    let SourceDiscoveryStep::Sealed(seal) = work.poll().unwrap() else {
        panic!("proven module namespace did not seal")
    };
    assert_eq!(seal.key(), *work.key());
    assert_eq!(work.sealed_expansions().unwrap().len(), 1);
    work.publish(&mut db).unwrap();
    for (key, expected) in syntax.inputs() {
        assert_eq!(db.input(key).unwrap().fingerprint(), expected.fingerprint());
    }
    assert_eq!(db.source_revision(), revision);
    let published = db.revision();
    work.publish(&mut db).unwrap();
    assert_eq!(db.revision(), published);
}

#[test]
fn completed_module_macro_export_route_resolves_later_original_invocation() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) = fixture(
        "macro_rules! emit{()=>{mod child;}}emit!();exported!(after_export,2053_i64);fn main()->i64{0_i64}",
    );
    std::fs::write(
        path.join("child.rs"),
        "#[macro_export]macro_rules! exported{($n:ident,$v:literal)=>{pub fn $n(){ $v; }}}",
    )
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(_)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.complete_module_namespace(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("actual module export route absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let words = module_request_words(&record.tokens().unwrap());
    assert!(words.contains(&"after_export".into()));
    assert!(words.contains(&"2053_i64".into()));
    work.admit_expansion(record).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(work.sealed_expansions().unwrap().len(), 2);
    work.publish(&mut db).unwrap();
}

#[test]
fn module_namespace_completion_preserves_missing_import_and_unknown_source_inputs() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(path.join("child.rs"), "use crate::missing;unknown!();").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let import = lrr::frontend::names::NameNamespaces::import_identity(
        &syntax.prepared_source().outline().imports[0],
    );
    let call = syntax.prepared_source().outline().item_invocations[0].item_input;
    let QueryPoll::Ready(Ok(ModuleSourceReadiness::Pending { waiting })) =
        work.complete_module_namespace(&mut db, &input)
    else {
        panic!("unfinished namespace closed")
    };
    assert!(waiting.contains(&call));
    assert!(waiting.contains(&import));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn generated_external_module_function_executes_in_guest_and_matches_native_macos() {
    use lrr::{
        db::value::QueryValue,
        engine::run::{RunMachine, RunPoll, ValueSummary},
        expand::source_discovery::ModuleSourceReadiness,
    };
    let source = "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{child::made()}";
    let child = "pub fn made()->i64{1583_i64}";
    let (mut work, mut db, env, authority, path) = fixture(source);
    std::fs::write(path.join("child.rs"), child).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(_)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.complete_module_namespace(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let QueryPoll::Ready(Ok(value)) = db.fetch(
        &SyntaxProvider,
        QueryKey::Outline(OutlineKey {
            parse: ParseKey {
                content: Digest::of_bytes(source.as_bytes()),
                profile: ProfileIdentity::current(),
            },
            crate_key: UNIT,
            module_path: vec![],
            cfg: CfgSet::guest_default().fingerprint(),
        }),
    ) else {
        panic!("entry outline absent")
    };
    let QueryValue::Outline(outline) = value.value.as_ref() else {
        panic!("entry outline variant")
    };
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let started = RunMachine::start_with_discovery(&mut db, body, 10_000, work);
    let mut run = match started {
        QueryPoll::Ready(Ok(run)) => run,
        QueryPoll::Ready(Err(error)) => panic!("guest start: {error:?}"),
        QueryPoll::NeedInput(request) => panic!("guest start needs: {request:?}"),
    };
    let report = match run.poll(&mut db) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(diagnostic) => panic!("guest module run: {diagnostic:?}"),
        RunPoll::NeedInput(request) => panic!("guest module needs: {request:?}"),
        RunPoll::Panicked(_) => panic!("guest module panicked"),
    };
    let ValueSummary::Signed(actual) = report.value else {
        panic!("noninteger child result")
    };
    let directory = path.parent().unwrap().join(format!(
        "m013-module-publication-native-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("child.rs"), child).unwrap();
    std::fs::write(
        directory.join("oracle.rs"),
        source.replace("fn main()->i64", "fn case()->i64")
            + "\nfn main(){::std::println!(\"{}\",case());}\n",
    )
    .unwrap();
    std::fs::write(directory.join("vm.stdout"), &actual).unwrap();
    let version = std::process::Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(
        version_text.contains("release: 1.99.0")
            && version_text.contains("host: aarch64-apple-darwin")
    );
    std::fs::write(directory.join("rustc.stdout"), &version.stdout).unwrap();
    let compiled = std::process::Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
        .arg(directory.join("oracle.rs"))
        .arg("-o")
        .arg(directory.join("oracle"))
        .output()
        .unwrap();
    std::fs::write(directory.join("compile.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let native = std::process::Command::new(directory.join("oracle"))
        .output()
        .unwrap();
    assert!(native.status.success());
    std::fs::write(directory.join("native.stdout"), &native.stdout).unwrap();
    let observed = String::from_utf8(native.stdout).unwrap();
    assert_eq!(observed.trim(), actual);
    std::fs::write(directory.join("comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "comparison_passed": true, "platform": "aarch64-apple-darwin", "rust_version": "1.99.0",
        "native_stdout": observed.trim(), "vm_stdout": actual,
        "scope": "Actual generated external module function in the guest versus a standalone native oracle copy; no full module/JIT/release acceptance claim"
    })).unwrap()).unwrap();
}

#[test]
fn module_publication_refuses_stale_inputs_without_partial_child_rows() {
    use lrr::expand::source_discovery::ModuleSourceReadiness;
    let source = "macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}";
    let (mut work, mut db, env, authority, path) = fixture(source);
    std::fs::write(path.join("child.rs"), "pub fn made()->i64{2063_i64}").unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent request absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(&mut db, &env, authority, &input);
    let QueryPoll::Ready(Ok(syntax)) = work.stage_module_source(&mut db, &input) else {
        panic!("module staging absent")
    };
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.complete_module_namespace(&mut db, &input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let (_, mut changed, different, _, _) = fixture_with_environment(source, Some("changed"));
    assert_ne!(env.fingerprint(), different.fingerprint());
    let revision = changed.revision();
    assert_eq!(
        work.publish(&mut changed).unwrap_err().message,
        "discovery compiler input is stale or absent"
    );
    assert_eq!(changed.revision(), revision);
    for item in &syntax.prepared_source().outline().items {
        assert!(
            changed
                .input(&lrr::db::dependency::InputKey::DefinitionHeader(
                    item.def_key.fingerprint()
                ))
                .is_none()
        );
    }
    assert!(
        changed
            .input(&lrr::db::dependency::InputKey::DiscoverySeal(
                work.key().clone()
            ))
            .is_none()
    );
    work.publish(&mut db).unwrap();
}

fn ordinary_staging_fixture(
    payload: &str,
) -> (
    SourceDiscovery,
    Database,
    FrozenCompileEnvironment,
    IncludeAuthorities,
    InputRequest,
    Digest,
) {
    let (mut work, mut db, env, authority, path) =
        fixture("macro_rules! emit{()=>{mod child;}}emit!();fn main()->i64{0_i64}");
    std::fs::write(path.join("child.rs"),"macro_rules! before{()=>{pub fn earlier(){}}}mod grandchild;macro_rules! after{()=>{pub fn later(){}}}").unwrap();
    std::fs::create_dir_all(path.join("child")).unwrap();
    std::fs::write(path.join("child/grandchild.rs"), payload).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("root macro absent")
    };
    work.admit_expansion(Arc::new(request.expand().unwrap()))
        .unwrap();
    let producer = work.unresolved_module_sources().next().unwrap().producer();
    let input = work
        .prepare_module_source(producer, &authority, std::path::Path::new(""))
        .unwrap();
    adopt_fixture_module(
        &mut db,
        &env,
        IncludeAuthorities::new(
            UNIT,
            Digest([235; 32]),
            [IncludeRoot {
                identity: Digest([236; 32]),
                path: path.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap(),
        &input,
    );
    assert!(matches!(
        work.stage_module_source(&mut db, &input),
        QueryPoll::Ready(Ok(_))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let ordinary = work
        .unresolved_ordinary_module_sources()
        .next()
        .unwrap()
        .producer();
    let input = work
        .prepare_ordinary_module_source(ordinary, &authority)
        .unwrap();
    adopt_fixture_module(
        &mut db,
        &env,
        IncludeAuthorities::new(
            UNIT,
            Digest([235; 32]),
            [IncludeRoot {
                identity: Digest([236; 32]),
                path: path.clone(),
                kind: IncludeRootKind::Source,
            }],
        )
        .unwrap(),
        &input,
    );
    (work, db, env, authority, input, ordinary)
}
fn stage_ordinary_required(
    work: &mut SourceDiscovery,
    db: &mut Database,
    input: &InputRequest,
) -> Arc<lrr::source::ordinary_module::OrdinaryModuleSyntaxInput> {
    match work.stage_ordinary_module_source(db, input) {
        QueryPoll::Ready(Ok(syntax)) => syntax,
        QueryPoll::Ready(Err(error)) => panic!("ordinary staging error: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("ordinary staging additional input: {input:?}"),
    }
}
#[test]
fn ordinary_module_staging_registers_real_source_jobs_without_publishing_or_sealing() {
    let (mut work, mut db, _, _, input, producer) =
        ordinary_staging_fixture("pub fn made()->i64{2053_i64}fn cold<T:Missing>(){unknown();}");
    let revision = db.revision();
    let syntax = stage_ordinary_required(&mut work, &mut db, &input);
    assert_eq!(
        syntax.prepared_source().outline().root_scope.module_path,
        ["child", "grandchild"]
    );
    assert_eq!(db.revision(), revision);
    let progress = work.module_source_progress(producer).unwrap();
    assert!(!progress.parsed);
    assert_eq!(progress.declarations, 0);
    assert!(work.sealed_expansions().is_err());
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let progress = work.module_source_progress(producer).unwrap();
    assert!(progress.parsed);
    assert_eq!(progress.declarations, 2);
    assert!(work.sealed_expansions().is_err());
    assert!(work.publish(&mut db).is_err());
    assert_eq!(db.revision(), revision);
}
#[test]
fn ordinary_module_staging_replay_keeps_projection_identity_and_parent_frontier() {
    let (mut work, mut db, _, _, input, producer) =
        ordinary_staging_fixture("pub fn made()->i64{2053_i64}");
    let first = stage_ordinary_required(&mut work, &mut db, &input);
    let second = stage_ordinary_required(&mut work, &mut db, &input);
    assert_eq!(first.identity(), second.identity());
    assert!(Arc::ptr_eq(&first, &second));
    assert!(!work.module_source_progress(producer).unwrap().parsed);
    assert!(
        work.unresolved_ordinary_module_sources()
            .any(|fact| fact.producer() == producer)
    );
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn ordinary_module_staging_preserves_nested_source_producer_obligations() {
    let (mut work, mut db, _, authority, input, producer) =
        ordinary_staging_fixture("mod leaf;pub fn made()->i64{leaf::made()}");
    stage_ordinary_required(&mut work, &mut db, &input);
    let facts = work
        .unresolved_ordinary_module_sources()
        .map(|fact| (fact.producer(), fact.scope().module_path.clone()))
        .collect::<Vec<_>>();
    assert!(
        facts
            .iter()
            .any(|(id, scope)| *id != producer && scope == &["child", "grandchild", "leaf"])
    );
    let leaf = facts
        .iter()
        .find(|(_, scope)| scope == &["child", "grandchild", "leaf"])
        .unwrap()
        .0;
    let request = work
        .prepare_ordinary_module_source(leaf, &authority)
        .unwrap();
    let InputRequest::CaptureModule { request, .. } = request else {
        panic!("ordinary capture absent")
    };
    assert!(request.candidates().iter().all(|candidate| {
        candidate
            .from()
            .path
            .as_str()
            .ends_with("child/grandchild.rs")
    }));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn ordinary_module_readiness_waits_for_parse_and_completion_retires_parent_source_frontier() {
    use lrr::expand::source_discovery::OrdinaryModuleReadiness;
    let (mut work, mut db, _, _, input, producer) =
        ordinary_staging_fixture("pub fn made()->i64{2053_i64}fn cold<T:Missing>(){unknown();}");
    let syntax = stage_ordinary_required(&mut work, &mut db, &input);
    assert!(matches!(
        work.ordinary_module_readiness(&mut db, &input),
        QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Pending { .. }))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(ready))) =
        work.ordinary_module_readiness(&mut db, &input)
    else {
        panic!("ordinary leaf not ready")
    };
    assert_eq!(ready.syntax().identity(), syntax.identity());
    assert!(ready.records().is_empty());
    assert!(
        work.unresolved_ordinary_module_sources()
            .any(|fact| fact.producer() == producer)
    );
    assert!(matches!(
        work.complete_ordinary_module_namespace(&mut db, &input),
        QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(_)))
    ));
    assert!(
        !work
            .unresolved_ordinary_module_sources()
            .any(|fact| fact.producer() == producer)
    );
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn ordinary_module_readiness_keeps_nested_source_and_missing_import_demands_pending() {
    use lrr::expand::source_discovery::OrdinaryModuleReadiness;
    for payload in [
        "mod leaf;pub fn made(){}",
        "use absent::item;pub fn made(){}",
    ] {
        let (mut work, mut db, _, _, input, producer) = ordinary_staging_fixture(payload);
        stage_ordinary_required(&mut work, &mut db, &input);
        assert!(matches!(
            work.poll().unwrap(),
            SourceDiscoveryStep::Incomplete { .. }
        ));
        let QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Pending { waiting })) =
            work.complete_ordinary_module_namespace(&mut db, &input)
        else {
            panic!("incomplete ordinary source marked ready")
        };
        assert!(!waiting.is_empty());
        assert!(
            work.unresolved_ordinary_module_sources()
                .any(|fact| fact.producer() == producer)
        );
        assert!(work.sealed_expansions().is_err());
    }
}
#[test]
fn ordinary_module_completion_replay_revalidates_inputs_without_republishing() {
    use lrr::expand::source_discovery::OrdinaryModuleReadiness;
    let (mut work, mut db, _, _, input, producer) =
        ordinary_staging_fixture("pub fn made()->i64{2053_i64}");
    stage_ordinary_required(&mut work, &mut db, &input);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(first))) =
        work.complete_ordinary_module_namespace(&mut db, &input)
    else {
        panic!("ordinary leaf completion absent")
    };
    let revision = db.revision();
    let QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(replay))) =
        work.complete_ordinary_module_namespace(&mut db, &input)
    else {
        panic!("ordinary leaf replay absent")
    };
    assert_eq!(first.identity(), replay.identity());
    assert_eq!(first.syntax().identity(), replay.syntax().identity());
    assert_eq!(db.revision(), revision);
    assert!(
        !work
            .unresolved_ordinary_module_sources()
            .any(|fact| fact.producer() == producer)
    );
    let InputRequest::CaptureModule { request, .. } = &input else {
        panic!("capture absent")
    };
    let changed = InputRequest::CaptureModule {
        request: request.clone(),
        compile_environment: Digest([19; 32]),
    };
    assert!(matches!(
        work.complete_ordinary_module_namespace(&mut db, &changed),
        QueryPoll::Ready(Err(_))
    ));
    assert_eq!(db.revision(), revision);
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn ordinary_module_publication_binds_actual_child_projection_atomically() {
    use lrr::expand::source_discovery::{ModuleSourceReadiness, OrdinaryModuleReadiness};
    let (mut work, mut db, _, authority, input, _) =
        ordinary_staging_fixture("pub fn made()->i64{2053_i64}fn cold<T:Missing>(){unknown();}");
    let syntax = stage_ordinary_required(&mut work, &mut db, &input);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.complete_ordinary_module_namespace(&mut db, &input),
        QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(_)))
    ));
    let parent = work.unresolved_module_sources().next().unwrap().producer();
    let parent_input = work
        .prepare_module_source(parent, &authority, std::path::Path::new(""))
        .unwrap();
    assert!(matches!(
        work.complete_module_namespace(&mut db, &parent_input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let source_revision = db.source_revision();
    work.publish(&mut db).unwrap();
    for (key, value) in syntax.inputs() {
        assert_eq!(db.input(key).unwrap().fingerprint(), value.fingerprint());
    }
    assert_eq!(db.source_revision(), source_revision);
    let revision = db.revision();
    work.publish(&mut db).unwrap();
    assert_eq!(db.revision(), revision);
}
#[test]
fn ordinary_module_publication_rejects_stale_capture_without_partial_child_rows() {
    use lrr::expand::source_discovery::{ModuleSourceReadiness, OrdinaryModuleReadiness};
    let (mut work, mut db, _, authority, input, _) =
        ordinary_staging_fixture("pub fn made()->i64{2053_i64}");
    let syntax = stage_ordinary_required(&mut work, &mut db, &input);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.complete_ordinary_module_namespace(&mut db, &input),
        QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(_)))
    ));
    let parent = work.unresolved_module_sources().next().unwrap().producer();
    let parent_input = work
        .prepare_module_source(parent, &authority, std::path::Path::new(""))
        .unwrap();
    assert!(matches!(
        work.complete_module_namespace(&mut db, &parent_input),
        QueryPoll::Ready(Ok(ModuleSourceReadiness::Ready(_)))
    ));
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    db.publish_inputs(BTreeMap::new()).unwrap();
    let revision = db.revision();
    let error = work.publish(&mut db).unwrap_err();
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(db.revision(), revision);
    for key in syntax.inputs().keys() {
        assert!(db.input(key).is_none());
    }
}
#[test]
fn ordinary_module_readiness_rejects_changed_environment_after_positive_result() {
    use lrr::expand::source_discovery::OrdinaryModuleReadiness;
    let (mut work, mut db, _, _, input, _) =
        ordinary_staging_fixture("pub fn made()->i64{2053_i64}");
    stage_ordinary_required(&mut work, &mut db, &input);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(matches!(
        work.ordinary_module_readiness(&mut db, &input),
        QueryPoll::Ready(Ok(OrdinaryModuleReadiness::Ready(_)))
    ));
    let InputRequest::CaptureModule { request, .. } = &input else {
        panic!("ordinary capture absent")
    };
    let changed = InputRequest::CaptureModule {
        request: request.clone(),
        compile_environment: Digest([19; 32]),
    };
    let revision = db.revision();
    let QueryPoll::Ready(Err(error)) = work.complete_ordinary_module_namespace(&mut db, &changed)
    else {
        panic!("foreign environment reused ready result")
    };
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(error.message, "ordinary module compile environment changed");
    assert_eq!(db.revision(), revision);
    assert!(work.sealed_expansions().is_err());
}

include!("support/constant_key.rs");
