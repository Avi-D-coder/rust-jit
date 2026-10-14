use lrr::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        dependencies::SourceDependency,
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const APP: Digest = Digest([201; 32]);
const LIB: Digest = Digest([202; 32]);
fn edge(alias: &str) -> SourceDependency {
    SourceDependency {
        consumer: APP,
        alias: alias.into(),
        provider: LIB,
    }
}
fn capture(
    main: &str,
    lib: &str,
    rows: Vec<SourceDependency>,
) -> Result<(Vec<PreparedSourceView>, PreparedSourceInputs), Diagnostic> {
    capture_using(None, main, lib, Some(rows))
}
fn capture_using(
    previous: Option<&PreparedSourceInputs>,
    main: &str,
    lib: &str,
    rows: Option<Vec<SourceDependency>>,
) -> Result<(Vec<PreparedSourceView>, PreparedSourceInputs), Diagnostic> {
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let mut units = Vec::new();
    let mut keys = Vec::new();
    for (path, unit, text) in [("app/main.rs", APP, main), ("dependency/lib.rs", LIB, lib)] {
        let path = SourcePath::new(path)?;
        let blob = SourceBlob::new(text.as_bytes().to_vec())?;
        keys.push(SourceSelection {
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
        });
        units.push(SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = match rows {
        Some(rows) => PreparedSourceInputs::refresh_with_dependencies(
            previous,
            units,
            Default::default(),
            rows,
        )?,
        None => PreparedSourceInputs::refresh(previous, units, Default::default())?,
    };
    let views = keys
        .iter()
        .map(|key| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(key)? else {
                panic!("captured unit absent")
            };
            Ok(view)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok((views, inputs))
}
fn discovery(views: &[PreparedSourceView]) -> SourceDiscovery {
    SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([222; 32]), Digest([223; 32]))
    .unwrap()
}
fn finish(work: &mut SourceDiscovery) {
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => work
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                let record = request.expand().unwrap();
                work.admit_expansion(std::sync::Arc::new(record)).unwrap();
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
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("{diagnostic:?} {blocked:?}"),
        }
    }
}
fn run(main: &str, lib: &str) -> String {
    let (views, inputs) = capture(main, lib, vec![edge("renamed")]).unwrap();
    run_captured(views, inputs)
}
fn run_captured(views: Vec<PreparedSourceView>, inputs: PreparedSourceInputs) -> String {
    let body = views[0]
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut work = discovery(&views);
    finish(&mut work);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_discovery(&mut db, body, 10_000, work)
    else {
        panic!("dependency entry absent")
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("dependency runtime did not complete")
    };
    let ValueSummary::Signed(value) = report.value else {
        panic!("noninteger dependency result")
    };
    value
}
const SEED: &str = "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{137_i64}}}";
#[test]
fn captured_dependency_alias_selects_the_actual_source_macro_body() {
    assert_eq!(run("renamed::seed!();fn main()->i64{made()}", SEED), "137");
}
#[test]
fn captured_dependency_macro_import_retains_its_definition_identity() {
    assert_eq!(
        run(
            "use renamed::seed as picked;picked!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn generated_dependency_macro_import_uses_the_captured_alias_graph() {
    assert_eq!(
        run(
            "macro_rules! outer{()=>{use renamed::seed as picked;picked!();}}outer!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn dependency_dollar_crate_call_retains_the_emitting_definition_unit() {
    assert_eq!(
        run(
            "renamed::outer!();fn main()->i64{made()}",
            "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{139_i64}}}#[macro_export]macro_rules! outer{()=>{$crate::seed!();}}"
        ),
        "139"
    );
}
#[test]
fn dependency_preparation_refuses_uncaptured_providers() {
    let mut row = edge("renamed");
    row.provider = Digest([203; 32]);
    assert_eq!(
        capture("", SEED, vec![row]).err().unwrap().code,
        "LRR-E-UNIT-GRAPH"
    );
}
#[test]
fn dependency_preparation_refuses_conflicting_canonical_aliases() {
    let mut row = edge("renamed");
    row.provider = APP;
    assert_eq!(
        capture("", SEED, vec![edge("renamed"), row])
            .err()
            .unwrap()
            .code,
        "LRR-E-UNIT-GRAPH"
    );
}
#[test]
fn dependency_row_order_does_not_change_the_published_input_fingerprint() {
    use lrr::db::dependency::InputKey;
    let (_, a) = capture("", SEED, vec![edge("renamed"), edge("second")]).unwrap();
    let (_, b) = capture("", SEED, vec![edge("second"), edge("renamed")]).unwrap();
    let mut db = Database::default();
    a.publish(&mut db).unwrap();
    let first = db
        .input(&InputKey::SourceDependencies)
        .unwrap()
        .fingerprint();
    b.publish(&mut db).unwrap();
    assert_eq!(
        db.input(&InputKey::SourceDependencies)
            .unwrap()
            .fingerprint(),
        first
    );
}
#[test]
fn dependency_alias_edit_invalidates_old_namespace_certificates() {
    use lrr::db::{
        dependency::{DependencyStamp, InputKey},
        key::{BucketKey, QueryKey, SealedNameKey},
        provider::SyntaxProvider,
    };
    let (views, inputs) = capture(
        "renamed::seed!();fn main()->i64{made()}",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let mut work = discovery(&views);
    finish(&mut work);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = QueryKey::SealedName(Box::new(SealedNameKey {
        seal: work.key().clone(),
        bucket: BucketKey {
            scope: views[0].outline().root_scope.clone(),
            namespace: Namespace::Value,
            name: "missing".into(),
        },
    }));
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    let seal = QueryKey::DiscoverySeal(work.key().clone());
    assert!(
        db.memo(&seal)
            .unwrap()
            .dependencies
            .iter()
            .any(|d| matches!(
                d,
                DependencyStamp::Input {
                    key: InputKey::SourceDependencies,
                    ..
                }
            ))
    );
    let (_, changed) = capture(
        "renamed::seed!();fn main()->i64{made()}",
        SEED,
        vec![edge("other")],
    )
    .unwrap();
    changed.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, key),
        QueryPoll::NeedInput(_)
    ));
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn ordinary_source_refresh_preserves_captured_dependency_aliases() {
    let main = "renamed::seed!();fn main()->i64{made()}";
    let (_, prior) = capture(main, SEED, vec![edge("renamed")]).unwrap();
    let (views, next) = capture_using(
        Some(&prior),
        main,
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{141_i64}}}",
        None,
    )
    .unwrap();
    assert_eq!(run_captured(views, next), "141");
}
const LOCAL_SHADOW: &str =
    "mod renamed{macro_rules! seed{()=>{pub fn made()->i64{149_i64}}}pub(crate) use seed;}";
#[test]
fn local_module_macro_path_shadows_a_dependency_alias() {
    assert_eq!(
        run(
            &format!("{LOCAL_SHADOW}renamed::seed!();fn main()->i64{{made()}}"),
            SEED
        ),
        "149"
    );
}
#[test]
fn absolute_macro_path_retains_the_captured_dependency_alias() {
    assert_eq!(
        run(
            &format!("{LOCAL_SHADOW}::renamed::seed!();fn main()->i64{{made()}}"),
            SEED
        ),
        "137"
    );
}
#[test]
fn local_module_macro_import_shadows_a_dependency_alias() {
    assert_eq!(
        run(
            &format!(
                "{LOCAL_SHADOW}use renamed::seed as picked;picked!();fn main()->i64{{made()}}"
            ),
            SEED
        ),
        "149"
    );
}
#[test]
fn generated_macro_import_keeps_local_module_precedence_over_a_dependency() {
    assert_eq!(
        run(
            &format!(
                "{LOCAL_SHADOW}macro_rules! outer{{()=>{{use renamed::seed as picked;picked!();}}}}outer!();fn main()->i64{{made()}}"
            ),
            SEED
        ),
        "149"
    );
}

#[test]
fn absolute_macro_import_bypasses_the_same_named_local_module() {
    assert_eq!(
        run(
            &format!(
                "{LOCAL_SHADOW}use ::renamed::seed as picked;picked!();fn main()->i64{{made()}}"
            ),
            SEED
        ),
        "137"
    );
}

#[test]
fn explicit_empty_dependency_refresh_removes_prior_aliases() {
    let main = "renamed::seed!();fn main()->i64{made()}";
    let (_, prior) = capture(main, SEED, vec![edge("renamed")]).unwrap();
    let (views, _) = capture_using(Some(&prior), main, SEED, Some(Vec::new())).unwrap();
    let mut work = discovery(&views);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("removed dependency remained visible")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn explicit_extern_crate_alias_resolves_captured_macro_definition() {
    assert_eq!(
        run(
            "extern crate renamed as tools;tools::seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn nested_extern_crate_alias_resolves_in_its_module() {
    assert_eq!(
        run(
            "mod inner{extern crate renamed as tools;tools::seed!();}fn main()->i64{inner::made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn generated_extern_crate_alias_resolves_captured_macro_definition() {
    assert_eq!(
        run(
            "macro_rules! outer{()=>{extern crate renamed as tools;tools::seed!();}}outer!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}

#[test]
fn public_extern_crate_alias_supports_macro_reexport() {
    assert_eq!(
        run(
            "mod inner{pub extern crate renamed as tools;}use inner::tools::seed as picked;picked!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn inactive_extern_crate_alias_never_demands_an_uncaptured_unit() {
    assert_eq!(
        run(
            "#[cfg(any())]extern crate missing as tools;renamed::seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn explicit_extern_crate_alias_survives_no_implicit_prelude() {
    assert_eq!(
        run(
            "#![no_implicit_prelude]extern crate renamed as tools;tools::seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn uncaptured_extern_crate_alias_is_refused_before_discovery() {
    let (views, _) = capture(
        "extern crate absent as tools;tools::seed!();",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let result = SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([222; 32]), Digest([223; 32]));
    let diagnostic = result.err().expect("uncaptured alias accepted");
    assert_eq!(diagnostic.code, "LRR-E-MACRO-SCOPE");
    assert!(diagnostic.message.contains("not captured"));
}
#[test]
fn private_nested_extern_crate_alias_does_not_escape_visibility() {
    let (views, _) = capture(
        "mod inner{extern crate renamed as tools;}inner::tools::seed!();",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let mut work = discovery(&views);
    let error = work.poll().err().expect("private alias accepted");
    assert_eq!(error.code, "LRR-E-MACRO-INACCESSIBLE");
}

#[test]
fn explicit_extern_crate_alias_examples_match_native_macos() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let evidence = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../artifacts/macos-first/m013-extern-alias-oracles/run-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&evidence).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(version_text.contains("release: 1.99.0"));
    assert!(version_text.contains("host: aarch64-apple-darwin"));
    fs::write(evidence.join("rustc.stdout"), version.stdout).unwrap();
    let lib = evidence.join("lib.rs");
    fs::write(&lib, SEED).unwrap();
    let rlib = evidence.join("librenamed.rlib");
    let compiled = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name=renamed",
            "--crate-type=rlib",
        ])
        .arg(&lib)
        .arg("-o")
        .arg(&rlib)
        .output()
        .unwrap();
    fs::write(evidence.join("library.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let examples = [
        "extern crate renamed as tools;tools::seed!();fn main()->i64{made()}",
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{137_i64}}}extern crate self as tools;tools::seed!();fn main()->i64{made()}",
        "mod inner{extern crate renamed as tools;tools::seed!();}fn main()->i64{inner::made()}",
        "macro_rules! outer{()=>{extern crate renamed as tools;tools::seed!();}}outer!();fn main()->i64{made()}",
        "mod inner{pub extern crate renamed as tools;}use inner::tools::seed as picked;picked!();fn main()->i64{made()}",
        "#[cfg(any())]extern crate missing as tools;renamed::seed!();fn main()->i64{made()}",
        "#![no_implicit_prelude]extern crate renamed as tools;tools::seed!();fn main()->i64{made()}",
    ];
    for (ordinal, source) in examples.into_iter().enumerate() {
        let actual = run(source, SEED);
        let directory = evidence.join(ordinal.to_string());
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("live.rs"), source).unwrap();
        fs::write(directory.join("vm.stdout"), &actual).unwrap();
        let oracle = directory.join("oracle.rs");
        fs::write(
            &oracle,
            source.replace("fn main()->i64", "fn case()->i64")
                + "\nfn main(){::std::println!(\"{}\",case());}\n",
        )
        .unwrap();
        let executable = directory.join("oracle");
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=oracle",
                "--extern",
            ])
            .arg(format!("renamed={}", rlib.display()))
            .arg(&oracle)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        fs::write(directory.join("compile.stderr"), &compiled.stderr).unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(executable).output().unwrap();
        fs::write(directory.join("native.stdout"), &output.stdout).unwrap();
        fs::write(directory.join("native.stderr"), &output.stderr).unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), actual);
    }
    for (name, source, code) in [
        (
            "private",
            "mod inner{extern crate renamed as tools;}inner::tools::seed!();fn main(){}",
            "E0603",
        ),
        (
            "duplicate",
            "extern crate renamed as tools;extern crate renamed as tools;fn main(){}",
            "E0259",
        ),
        (
            "collision",
            "mod tools{}extern crate renamed as tools;fn main(){}",
            "E0260",
        ),
        (
            "anonymous",
            "#![no_implicit_prelude]extern crate renamed as _;renamed::seed!();fn main(){}",
            "E0433",
        ),
    ] {
        let denied = evidence.join(format!("{name}.rs"));
        fs::write(&denied, source).unwrap();
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=denied",
                "--extern",
            ])
            .arg(format!("renamed={}", rlib.display()))
            .arg(denied)
            .arg("-o")
            .arg(evidence.join(name))
            .output()
            .unwrap();
        fs::write(evidence.join(format!("{name}.stderr")), &compiled.stderr).unwrap();
        assert!(!compiled.status.success());
        assert!(String::from_utf8(compiled.stderr).unwrap().contains(code));
    }
}

#[test]
fn explicit_self_crate_alias_retains_local_definition_identity() {
    assert_eq!(
        run(
            &format!("{SEED}extern crate self as tools;tools::seed!();fn main()->i64{{made()}}"),
            ""
        ),
        "137"
    );
}
#[test]
fn duplicate_extern_crate_alias_is_refused() {
    let (views, _) = capture(
        "extern crate renamed as tools;extern crate renamed as tools;tools::seed!();",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let error = SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([222; 32]), Digest([223; 32]))
    .err()
    .expect("duplicate alias accepted");
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(error.message.contains("duplicate"));
}
#[test]
fn extern_crate_alias_collision_with_local_module_is_refused() {
    let (views, _) = capture(
        "mod tools{}extern crate renamed as tools;tools::seed!();",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let error = SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([222; 32]), Digest([223; 32]))
    .err()
    .expect("module collision accepted");
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(error.message.contains("module"));
}

#[test]
fn unresolved_attribute_cannot_publish_extern_crate_alias() {
    let (views, _) = capture(
        "#[unknown]extern crate renamed as tools;tools::seed!();",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let mut work = discovery(&views);
    assert!(
        matches!(work.poll().unwrap(), SourceDiscoveryStep::Incomplete { .. }),
        "attribute-pending alias supplied a macro expansion"
    );
}

#[test]
fn anonymous_extern_crate_cannot_fabricate_a_named_alias() {
    let (views, _) = capture(
        "#![no_implicit_prelude]extern crate renamed as _;renamed::seed!();",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let mut work = discovery(&views);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(
        views[0]
            .outline()
            .extern_crates
            .iter()
            .all(|row| row.alias.is_none())
    );
}

fn refuse_alias_fragment(source: &str) -> Diagnostic {
    let (views, _) = capture(source, SEED, vec![edge("renamed")]).unwrap();
    let work = match SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .and_then(|work| work.with_declarative(Digest([222; 32]), Digest([223; 32])))
    {
        Ok(work) => work,
        Err(error) => return error,
    };
    drive_alias_refusal(work)
}
fn drive_alias_refusal(mut work: SourceDiscovery) -> Diagnostic {
    for _ in 0..32 {
        let step = match work.poll() {
            Ok(step) => step,
            Err(error) => {
                let repeated = work
                    .poll()
                    .err()
                    .expect("alias failure did not poison discovery");
                assert_eq!(repeated.code, error.code);
                assert_eq!(repeated.message, error.message);
                return error;
            }
        };
        let admitted = match step {
            SourceDiscoveryStep::NeedExpansion { request } => {
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
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
            SourceDiscoveryStep::Sealed(_) => panic!("conflicting aliases were sealed"),
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => {
                panic!("expected a checked alias conflict, got {diagnostic:?}")
            }
        };
        if let Err(error) = admitted {
            let repeated = work
                .poll()
                .err()
                .expect("alias admission failure did not poison discovery");
            assert_eq!(repeated.code, error.code);
            assert_eq!(repeated.message, error.message);
            return error;
        }
    }
    panic!("alias conflict exceeded bounded discovery controls")
}
fn assert_alias_conflict(source: &str) {
    let error = refuse_alias_fragment(source);
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(
        error.message.contains("duplicate") || error.message.contains("conflict"),
        "{error:?}"
    );
}
#[test]
fn original_and_generated_extern_aliases_keep_distinct_declarations() {
    assert_alias_conflict(
        "extern crate renamed as tools;macro_rules! emit{()=>{extern crate renamed as tools;}}emit!();fn main()->i64{0_i64}",
    );
}
#[test]
fn sibling_generated_extern_aliases_keep_distinct_declarations() {
    assert_alias_conflict(
        "macro_rules! a{()=>{extern crate renamed as tools;}}macro_rules! b{()=>{extern crate renamed as tools;}}a!();b!();fn main()->i64{0_i64}",
    );
}
#[test]
fn repeated_generated_extern_aliases_keep_distinct_declarations() {
    assert_alias_conflict(
        "macro_rules! emit{()=>{extern crate renamed as tools;}}emit!();emit!();fn main()->i64{0_i64}",
    );
}
#[test]
fn nested_generated_extern_aliases_keep_distinct_declarations() {
    assert_alias_conflict(
        "macro_rules! inner{()=>{extern crate renamed as tools;}}macro_rules! outer{()=>{extern crate renamed as tools;inner!();}}outer!();fn main()->i64{0_i64}",
    );
}
#[test]
fn generated_extern_aliases_in_distinct_modules_remain_independent() {
    assert_eq!(
        run(
            "macro_rules! emit{()=>{extern crate renamed as tools;tools::seed!();}}mod left{emit!();}mod right{emit!();}fn main()->i64{left::made()+right::made()}",
            SEED
        ),
        "274"
    );
}

#[test]
fn generated_extern_alias_conflicts_match_native_macos() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let evidence = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../artifacts/macos-first/m013-extern-alias-conflicts-oracles/run-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&evidence).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(version_text.contains("release: 1.99.0"));
    assert!(version_text.contains("host: aarch64-apple-darwin"));
    fs::write(evidence.join("rustc.stdout"), version.stdout).unwrap();
    let lib = evidence.join("lib.rs");
    fs::write(&lib, SEED).unwrap();
    let rlib = evidence.join("librenamed.rlib");
    let compiled = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name=renamed",
            "--crate-type=rlib",
        ])
        .arg(&lib)
        .arg("-o")
        .arg(&rlib)
        .output()
        .unwrap();
    fs::write(evidence.join("library.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    for (name, source, code) in [
        (
            "original_generated",
            "extern crate renamed as tools;macro_rules! emit{()=>{extern crate renamed as tools;}}emit!();fn main(){}",
            "E0259",
        ),
        (
            "siblings",
            "macro_rules! a{()=>{extern crate renamed as tools;}}macro_rules! b{()=>{extern crate renamed as tools;}}a!();b!();fn main(){}",
            "E0259",
        ),
        (
            "repeated",
            "macro_rules! emit{()=>{extern crate renamed as tools;}}emit!();emit!();fn main(){}",
            "E0259",
        ),
        (
            "nested",
            "macro_rules! inner{()=>{extern crate renamed as tools;}}macro_rules! outer{()=>{extern crate renamed as tools;inner!();}}outer!();fn main(){}",
            "E0259",
        ),
        (
            "generated_import",
            "extern crate renamed as tools;macro_rules! emit{()=>{use renamed as tools;}}emit!();fn main(){}",
            "E0254",
        ),
        (
            "generated_extern",
            "use renamed as tools;macro_rules! emit{()=>{extern crate renamed as tools;}}emit!();fn main(){}",
            "E0254",
        ),
    ] {
        let denied = evidence.join(format!("{name}.rs"));
        fs::write(&denied, source).unwrap();
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=denied",
                "--extern",
            ])
            .arg(format!("renamed={}", rlib.display()))
            .arg(denied)
            .arg("-o")
            .arg(evidence.join(name))
            .output()
            .unwrap();
        fs::write(evidence.join(format!("{name}.stderr")), &compiled.stderr).unwrap();
        assert!(!compiled.status.success());
        assert!(String::from_utf8(compiled.stderr).unwrap().contains(code));
    }
    let examples = [
        "macro_rules! emit{()=>{extern crate renamed as tools;tools::seed!();}}mod left{emit!();}mod right{emit!();}fn main()->i64{left::made()+right::made()}",
        "macro_rules! emit{()=>{extern crate renamed as tools;use tools::seed as picked;picked!();}}emit!();fn main()->i64{made()}",
        "extern crate renamed as tools;use renamed::seed as tools;tools!();fn main()->i64{made()}",
        "macro_rules! emit{()=>{pub extern crate renamed;use renamed::seed as picked;picked!();}}emit!();fn main()->i64{made()}",
    ];
    for (ordinal, source) in examples.into_iter().enumerate() {
        let actual = run(source, SEED);
        let directory = evidence.join(ordinal.to_string());
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("live.rs"), source).unwrap();
        fs::write(directory.join("vm.stdout"), &actual).unwrap();
        let oracle = directory.join("oracle.rs");
        fs::write(
            &oracle,
            source.replace("fn main()->i64", "fn case()->i64")
                + "\nfn main(){::std::println!(\"{}\",case());}\n",
        )
        .unwrap();
        let executable = directory.join("oracle");
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=oracle",
                "--extern",
            ])
            .arg(format!("renamed={}", rlib.display()))
            .arg(&oracle)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        fs::write(directory.join("compile.stderr"), &compiled.stderr).unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(executable).output().unwrap();
        fs::write(directory.join("native.stdout"), &output.stdout).unwrap();
        fs::write(directory.join("native.stderr"), &output.stderr).unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), actual);
    }
}

#[test]
fn generated_extern_alias_macro_import_facet_is_retained() {
    assert_eq!(
        run(
            "macro_rules! emit{()=>{extern crate renamed as tools;use tools::seed as picked;picked!();}}emit!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}

#[test]
fn generated_module_import_cannot_merge_with_extern_alias() {
    assert_alias_conflict(
        "extern crate renamed as tools;macro_rules! emit{()=>{use renamed as tools;}}emit!();fn main()->i64{0_i64}",
    );
}
#[test]
fn generated_extern_alias_cannot_merge_with_module_import() {
    assert_alias_conflict(
        "use renamed as tools;macro_rules! emit{()=>{extern crate renamed as tools;}}emit!();fn main()->i64{0_i64}",
    );
}
#[test]
fn extern_alias_and_macro_import_share_spelling_in_distinct_namespaces() {
    assert_eq!(
        run(
            "extern crate renamed as tools;use renamed::seed as tools;tools!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn generated_public_extern_alias_can_set_recorded_prelude_visibility() {
    assert_eq!(
        run(
            "macro_rules! emit{()=>{pub extern crate renamed;use renamed::seed as picked;picked!();}}emit!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn extern_alias_edit_invalidates_seal_before_conflict_revalidation() {
    use lrr::db::{
        key::{BucketKey, QueryKey, SealedNameKey},
        provider::SyntaxProvider,
    };
    let main = "extern crate renamed as tools;macro_rules! emit{()=>{extern crate renamed as other;}}emit!();other::seed!();fn main()->i64{made()}";
    let (views, inputs) = capture(main, SEED, vec![edge("renamed")]).unwrap();
    let mut work = discovery(&views);
    finish(&mut work);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = QueryKey::SealedName(Box::new(SealedNameKey {
        seal: work.key().clone(),
        bucket: BucketKey {
            scope: views[0].outline().root_scope.clone(),
            namespace: Namespace::Value,
            name: "made".into(),
        },
    }));
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    let changed = main.replace("as other", "as tools");
    let (next_views, next) = capture_using(Some(&inputs), &changed, SEED, None).unwrap();
    next.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, key),
        QueryPoll::NeedInput(_)
    ));
    let revision = db.revision();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.revision(), revision);
    let error = drive_alias_refusal(discovery(&next_views));
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(error.message.contains("duplicate"));
    assert_eq!(db.revision(), revision);
}

#[test]
fn extern_macro_use_resolves_exported_dependency_definition() {
    assert_eq!(
        run(
            "#[macro_use]extern crate renamed;seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn extern_macro_use_is_available_before_its_declaration() {
    assert_eq!(
        run(
            "seed!();#[macro_use]extern crate renamed;fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn extern_macro_use_is_inherited_by_nested_modules() {
    assert_eq!(
        run(
            "#[macro_use]extern crate renamed;mod inner{seed!();}fn main()->i64{inner::made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn generated_extern_macro_use_retains_dependency_definition_owner() {
    assert_eq!(
        run(
            "macro_rules! emit{()=>{#[macro_use]extern crate renamed;seed!();}}emit!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn selective_extern_macro_use_resolves_only_named_exports() {
    assert_eq!(
        run(
            "#[macro_use(seed)]extern crate renamed;seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn textual_macro_shadows_extern_macro_use_prelude() {
    assert_eq!(
        run(
            "#[macro_use]extern crate renamed;macro_rules! seed{()=>{fn made()->i64{619_i64}}}seed!();fn main()->i64{made()}",
            SEED
        ),
        "619"
    );
}

fn incomplete_macro_use(source: &str, library: &str) {
    let (views, _) = capture(source, library, vec![edge("renamed")]).unwrap();
    let mut work = discovery(&views);
    assert!(
        matches!(work.poll().unwrap(), SourceDiscoveryStep::Incomplete { .. }),
        "unexpected expansion or seal"
    );
}
#[test]
fn selective_macro_use_does_not_import_other_exports() {
    incomplete_macro_use(
        "#[macro_use(outer)]extern crate renamed;seed!();",
        &format!("{SEED}#[macro_export]macro_rules! outer{{()=>{{}}}}"),
    );
}
#[test]
fn macro_use_prelude_does_not_create_qualified_consumer_exports() {
    incomplete_macro_use("#[macro_use]extern crate renamed;crate::seed!();", SEED);
}
#[test]
fn no_implicit_prelude_disables_extern_macro_use_invocations() {
    incomplete_macro_use(
        "#![no_implicit_prelude]#[macro_use]extern crate renamed;seed!();",
        SEED,
    );
}
#[test]
fn unknown_provider_export_producer_prevents_macro_use_seal() {
    incomplete_macro_use("#[macro_use]extern crate renamed;seed!();", "missing!();");
}
#[test]
fn selective_macro_use_waits_for_a_generated_export() {
    assert_eq!(
        run(
            "#[macro_use(seed)]extern crate renamed;seed!();fn main()->i64{made()}",
            "macro_rules! emit{()=>{#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{631_i64}}}}}emit!();"
        ),
        "631"
    );
}
#[test]
fn extern_macro_use_dollar_crate_retains_actual_provider() {
    assert_eq!(
        run(
            "#[macro_use]extern crate renamed;macro_rules! seed{()=>{fn made()->i64{619_i64}}}outer!();fn main()->i64{made()}",
            &format!("{SEED}#[macro_export]macro_rules! outer{{()=>{{$crate::seed!();}}}}")
        ),
        "137"
    );
}
#[test]
fn explicit_macro_import_shadows_macro_use_prelude() {
    assert_eq!(
        run(
            "#[macro_use]extern crate renamed;use renamed::alternate as seed;seed!();fn main()->i64{made()}",
            &format!(
                "{SEED}#[macro_export]macro_rules! alternate{{()=>{{fn made()->i64{{641_i64}}}}}}"
            )
        ),
        "641"
    );
}
#[test]
fn empty_selective_macro_use_preserves_explicit_dependency_path() {
    assert_eq!(
        run(
            "#[macro_use()]extern crate renamed;renamed::seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn selective_macro_use_refuses_unexported_private_definition() {
    let (views, _) = capture(
        "#[macro_use(seed)]extern crate renamed;fn main()->i64{0_i64}",
        "macro_rules! seed{()=>{}}",
        vec![edge("renamed")],
    )
    .unwrap();
    let error = SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([222; 32]), Digest([223; 32]))
    .err()
    .expect("unexported selector accepted");
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(error.message.contains("unexported"));
}

#[test]
fn nested_generated_extern_macro_use_retains_source_order_path() {
    assert_eq!(
        run(
            "macro_rules! inner{()=>{#[macro_use]extern crate renamed;seed!();}}macro_rules! outer{()=>{inner!();}}outer!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn extern_macro_use_outside_crate_root_is_refused() {
    let (views, _) = capture(
        "mod inner{#[macro_use]extern crate renamed;seed!();}",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let error = SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([222; 32]), Digest([223; 32]))
    .err()
    .expect("nested extern macro-use accepted");
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(error.message.contains("crate root"));
}
#[test]
fn extern_macro_use_examples_match_native_macos() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../artifacts/macos-first/m013-extern-macro-use-oracles/run-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0"));
    assert!(text.contains("host: aarch64-apple-darwin"));
    fs::write(root.join("rustc.stdout"), version.stdout).unwrap();
    let cases=[
        ("all","#[macro_use]extern crate renamed;seed!();fn main()->i64{made()}",SEED.to_owned(),None),
        ("before","seed!();#[macro_use]extern crate renamed;fn main()->i64{made()}",SEED.to_owned(),None),
        ("nested","#[macro_use]extern crate renamed;mod inner{seed!();}fn main()->i64{inner::made()}",SEED.to_owned(),None),
        ("generated","macro_rules! emit{()=>{#[macro_use]extern crate renamed;seed!();}}emit!();fn main()->i64{made()}",SEED.to_owned(),None),
        ("nested_generated","macro_rules! inner{()=>{#[macro_use]extern crate renamed;seed!();}}macro_rules! outer{()=>{inner!();}}outer!();fn main()->i64{made()}",SEED.to_owned(),None),
        ("selective","#[macro_use(seed)]extern crate renamed;seed!();fn main()->i64{made()}",SEED.to_owned(),None),
        ("generated_export","#[macro_use(seed)]extern crate renamed;seed!();fn main()->i64{made()}","macro_rules! emit{()=>{#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{631_i64}}}}}emit!();".to_owned(),None),
        ("textual","#[macro_use]extern crate renamed;macro_rules! seed{()=>{fn made()->i64{619_i64}}}seed!();fn main()->i64{made()}",SEED.to_owned(),None),
        ("named","#[macro_use]extern crate renamed;use renamed::alternate as seed;seed!();fn main()->i64{made()}",format!("{SEED}#[macro_export]macro_rules! alternate{{()=>{{fn made()->i64{{641_i64}}}}}}"),None),
        ("dollar_crate","#[macro_use]extern crate renamed;macro_rules! seed{()=>{fn made()->i64{619_i64}}}outer!();fn main()->i64{made()}",format!("{SEED}#[macro_export]macro_rules! outer{{()=>{{$crate::seed!();}}}}"),None),
        ("empty","#[macro_use()]extern crate renamed;renamed::seed!();fn main()->i64{made()}",SEED.to_owned(),None),
        ("unselected","#[macro_use(outer)]extern crate renamed;seed!();fn main()->i64{0_i64}",format!("{SEED}#[macro_export]macro_rules! outer{{()=>{{}}}}"),Some("cannot find macro")),
        ("private","#[macro_use(seed)]extern crate renamed;fn main()->i64{0_i64}","macro_rules! seed{()=>{}}".to_owned(),Some("E0469")),
        ("disabled","#![no_implicit_prelude]#[macro_use]extern crate renamed;seed!();fn main()->i64{0_i64}",SEED.to_owned(),Some("cannot find macro")),
        ("qualified","#[macro_use]extern crate renamed;crate::seed!();fn main()->i64{0_i64}",SEED.to_owned(),Some("E0433")),
        ("generated_textual","macro_rules! seed{()=>{}}macro_rules! emit{()=>{#[macro_use]extern crate renamed;}}emit!();fn main()->i64{0_i64}",SEED.to_owned(),None),
        ("wrong_scope","mod inner{#[macro_use]extern crate renamed;}fn main()->i64{0_i64}",SEED.to_owned(),Some("E0468")),
    ];
    for (name, source, library, expected_error) in cases {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("live.rs"), source).unwrap();
        fs::write(directory.join("lib.rs"), library.as_bytes()).unwrap();
        let rlib = directory.join("librenamed.rlib");
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=renamed",
                "--crate-type=rlib",
            ])
            .arg(directory.join("lib.rs"))
            .arg("-o")
            .arg(&rlib)
            .output()
            .unwrap();
        fs::write(directory.join("library.stderr"), &compiled.stderr).unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let oracle = directory.join("oracle.rs");
        fs::write(
            &oracle,
            source.replace("fn main()->i64", "fn case()->i64")
                + "\nfn main(){::std::println!(\"{}\",case());}\n",
        )
        .unwrap();
        let executable = directory.join("oracle");
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=oracle",
                "--extern",
            ])
            .arg(format!("renamed={}", rlib.display()))
            .arg(&oracle)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        fs::write(directory.join("compile.stderr"), &compiled.stderr).unwrap();
        if let Some(error) = expected_error {
            assert!(
                !compiled.status.success(),
                "{name}: native refusal expected"
            );
            assert!(
                String::from_utf8_lossy(&compiled.stderr).contains(error),
                "{name}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        } else {
            assert!(
                compiled.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let output = Command::new(executable).output().unwrap();
            fs::write(directory.join("native.stdout"), &output.stdout).unwrap();
            fs::write(directory.join("native.stderr"), &output.stderr).unwrap();
            assert!(output.status.success());
            let actual = run(source, &library);
            fs::write(directory.join("vm.stdout"), &actual).unwrap();
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                actual,
                "{name}"
            );
        }
    }
}

fn capture_macro_use_pair(main: &str) -> (Vec<PreparedSourceView>, PreparedSourceInputs) {
    let second = Digest([203; 32]);
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let mut keys = Vec::new();
    let mut units = Vec::new();
    for (path, unit, text) in [
        ("app/main.rs", APP, main),
        ("first/lib.rs", LIB, SEED),
        (
            "second/lib.rs",
            second,
            "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{653_i64}}}",
        ),
    ] {
        let path = SourcePath::new(path).unwrap();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        keys.push(SourceSelection {
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
        });
        units.push(SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        vec![
            edge("renamed"),
            SourceDependency {
                consumer: APP,
                alias: "secondary".into(),
                provider: second,
            },
        ],
    )
    .unwrap();
    let views = keys
        .iter()
        .map(|key| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(key).unwrap() else {
                panic!("macro-use pair unit absent")
            };
            view
        })
        .collect();
    (views, inputs)
}
#[test]
fn extern_macro_use_import_priority_follows_original_and_generated_source_order() {
    for (source, expected) in [
        (
            "#[macro_use]extern crate renamed;#[macro_use]extern crate secondary;seed!();fn main()->i64{made()}",
            "653",
        ),
        (
            "#[macro_use]extern crate secondary;#[macro_use]extern crate renamed;seed!();fn main()->i64{made()}",
            "137",
        ),
    ] {
        let (views, inputs) = capture_macro_use_pair(source);
        assert_eq!(run_captured(views, inputs), expected);
    }
    let (views, _) = capture_macro_use_pair(
        "macro_rules! emit{()=>{#[macro_use]extern crate secondary;}}#[macro_use]extern crate renamed;emit!();seed!();fn main()->i64{made()}",
    );
    let error = drive_alias_refusal(discovery(&views));
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(error.message.contains("shadow"));
}
#[test]
fn extern_macro_use_import_priority_matches_native_macos() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../artifacts/macos-first/m013-extern-macro-use-priority-oracles/run-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0"));
    assert!(text.contains("host: aarch64-apple-darwin"));
    fs::write(root.join("rustc.stdout"), version.stdout).unwrap();
    for (name, source) in [
        ("renamed", SEED),
        (
            "secondary",
            "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{653_i64}}}",
        ),
    ] {
        let source_path = root.join(format!("{name}.rs"));
        fs::write(&source_path, source).unwrap();
        let output = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=rlib",
                "--crate-name",
            ])
            .arg(name)
            .arg(source_path)
            .arg("-o")
            .arg(root.join(format!("lib{name}.rlib")))
            .output()
            .unwrap();
        fs::write(root.join(format!("{name}.stderr")), &output.stderr).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for (ordinal,source) in [
        "#[macro_use]extern crate renamed;#[macro_use]extern crate secondary;seed!();fn main()->i64{made()}",
        "#[macro_use]extern crate secondary;#[macro_use]extern crate renamed;seed!();fn main()->i64{made()}",
        "macro_rules! emit{()=>{#[macro_use]extern crate secondary;}}#[macro_use]extern crate renamed;emit!();seed!();fn main()->i64{made()}",
    ].into_iter().enumerate() { let directory=root.join(ordinal.to_string());fs::create_dir_all(&directory).unwrap();fs::write(directory.join("live.rs"),source).unwrap();let oracle=directory.join("oracle.rs");fs::write(&oracle,source.replace("fn main()->i64","fn case()->i64")+"\nfn main(){println!(\"{}\",case());}\n").unwrap();let executable=directory.join("oracle");let output=Command::new("rustc").args(["+1.99.0","--edition=2024","--crate-name=oracle","--extern"]).arg(format!("renamed={}",root.join("librenamed.rlib").display())).arg("--extern").arg(format!("secondary={}",root.join("libsecondary.rlib").display())).arg(&oracle).arg("-o").arg(&executable).output().unwrap();fs::write(directory.join("compile.stderr"),&output.stderr).unwrap();if ordinal==2 { assert!(!output.status.success());assert!(String::from_utf8_lossy(&output.stderr).contains("already in scope"));assert!(String::from_utf8_lossy(&output.stderr).contains("may not shadow"));let (views,_)=capture_macro_use_pair(source);let error=drive_alias_refusal(discovery(&views));assert_eq!(error.code,"LRR-E-MACRO-SCOPE");assert!(error.message.contains("shadow"));continue; }assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));let output=Command::new(executable).output().unwrap();fs::write(directory.join("native.stdout"),&output.stdout).unwrap();assert!(output.status.success());let (views,inputs)=capture_macro_use_pair(source);let actual=run_captured(views,inputs);fs::write(directory.join("vm.stdout"),&actual).unwrap();assert_eq!(String::from_utf8(output.stdout).unwrap().trim(),actual); }
}

#[test]
fn generated_macro_use_preserves_existing_textual_macro() {
    assert_eq!(
        run(
            "macro_rules! seed{()=>{}}macro_rules! emit{()=>{#[macro_use]extern crate renamed;}}emit!();fn main()->i64{0_i64}",
            SEED
        ),
        "0"
    );
}
#[test]
fn macro_use_provider_edit_invalidates_old_seal_and_executes_new_body() {
    use lrr::db::{
        key::{BucketKey, QueryKey, SealedNameKey},
        provider::SyntaxProvider,
    };
    let main = "#[macro_use]extern crate renamed;seed!();fn main()->i64{made()}";
    let (views, inputs) = capture(main, SEED, vec![edge("renamed")]).unwrap();
    let mut work = discovery(&views);
    finish(&mut work);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    work.publish(&mut db).unwrap();
    let key = QueryKey::SealedName(Box::new(SealedNameKey {
        seal: work.key().clone(),
        bucket: BucketKey {
            scope: views[0].outline().root_scope.clone(),
            namespace: Namespace::Value,
            name: "made".into(),
        },
    }));
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    let (next_views, next) = capture_using(
        Some(&inputs),
        main,
        &SEED.replace("137_i64", "659_i64"),
        None,
    )
    .unwrap();
    next.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, key),
        QueryPoll::NeedInput(_)
    ));
    let revision = db.revision();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.revision(), revision);
    let mut changed = discovery(&next_views);
    finish(&mut changed);
    let body = next_views[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_discovery(&mut db, body, 10_000, changed)
    else {
        panic!("updated macro-use entry absent")
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("updated macro-use entry incomplete")
    };
    assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="659"));
}

#[test]
fn anonymous_extern_macro_use_imports_without_a_type_binding() {
    assert_eq!(
        run(
            "#[macro_use]extern crate renamed as _;seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn generated_anonymous_extern_macro_use_retains_captured_owner() {
    assert_eq!(
        run(
            "macro_rules! emit{()=>{#[macro_use]extern crate renamed as _;seed!();}}emit!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn anonymous_extern_declaration_does_not_conflict_with_local_module() {
    assert_eq!(
        run(
            "mod renamed{pub fn made()->i64{673_i64}}extern crate renamed as _;fn main()->i64{renamed::made()}",
            SEED
        ),
        "673"
    );
}
#[test]
fn repeated_anonymous_extern_declarations_remain_distinct_and_nameless() {
    assert_eq!(
        run(
            "extern crate renamed as _;macro_rules! emit{()=>{extern crate renamed as _;}}emit!();emit!();renamed::seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}

#[test]
fn anonymous_selective_macro_use_preserves_only_named_exports() {
    assert_eq!(
        run(
            "#[macro_use(seed)]extern crate renamed as _;seed!();fn main()->i64{made()}",
            SEED
        ),
        "137"
    );
}
#[test]
fn anonymous_extern_headers_have_no_type_candidates_or_module_placeholders() {
    let (views, _) = capture(
        "extern crate renamed as _;extern crate renamed as _;fn main()->i64{0_i64}",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let outline = views[0].outline();
    assert_eq!(outline.extern_crates.len(), 2);
    assert_ne!(
        outline.extern_crates[0].identity,
        outline.extern_crates[1].identity
    );
    assert!(outline.extern_crates.iter().all(|row| row.alias.is_none()));
    assert_eq!(outline.scopes.len(), 1);
    assert!(outline.scopes.values().all(|scope| {
        scope
            .candidates
            .keys()
            .all(|(namespace, _)| *namespace != Namespace::Type)
    }));
    assert!(outline.textual_events.iter().all(|event| !matches!(
        event.kind,
        lrr::syntax::outline::TextualEventKind::Producer {
            kind: lrr::syntax::outline::ProducerKind::ModuleSource,
            ..
        }
    )));
}
#[test]
fn anonymous_extern_still_requires_a_recorded_dependency() {
    let (views, _) = capture(
        "extern crate absent as _;fn main()->i64{0_i64}",
        SEED,
        vec![edge("renamed")],
    )
    .unwrap();
    let error = SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Value,
        Digest([221; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([222; 32]), Digest([223; 32]))
    .err()
    .expect("uncaptured anonymous dependency accepted");
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
    assert!(error.message.contains("not captured"));
}

#[test]
fn anonymous_extern_examples_match_native_macos() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let evidence = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../artifacts/macos-first/m013-anonymous-extern-oracles/run-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&evidence).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(version_text.contains("release: 1.99.0"));
    assert!(version_text.contains("host: aarch64-apple-darwin"));
    fs::write(evidence.join("rustc.stdout"), version.stdout).unwrap();
    let lib = evidence.join("lib.rs");
    fs::write(&lib, SEED).unwrap();
    let rlib = evidence.join("librenamed.rlib");
    let compiled = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name=renamed",
            "--crate-type=rlib",
        ])
        .arg(&lib)
        .arg("-o")
        .arg(&rlib)
        .output()
        .unwrap();
    fs::write(evidence.join("library.stderr"), &compiled.stderr).unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let examples = [
        "#[macro_use]extern crate renamed as _;seed!();fn main()->i64{made()}",
        "#[macro_use(seed)]extern crate renamed as _;seed!();fn main()->i64{made()}",
        "macro_rules! emit{()=>{#[macro_use]extern crate renamed as _;seed!();}}emit!();fn main()->i64{made()}",
        "mod renamed{pub fn made()->i64{673_i64}}extern crate renamed as _;fn main()->i64{renamed::made()}",
        "extern crate renamed as _;macro_rules! emit{()=>{extern crate renamed as _;}}emit!();emit!();renamed::seed!();fn main()->i64{made()}",
    ];
    for (ordinal, source) in examples.into_iter().enumerate() {
        let actual = run(source, SEED);
        let directory = evidence.join(ordinal.to_string());
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("live.rs"), source).unwrap();
        fs::write(directory.join("vm.stdout"), &actual).unwrap();
        let oracle = directory.join("oracle.rs");
        fs::write(
            &oracle,
            source.replace("fn main()->i64", "fn case()->i64")
                + "\nfn main(){::std::println!(\"{}\",case());}\n",
        )
        .unwrap();
        let executable = directory.join("oracle");
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=oracle",
                "--extern",
            ])
            .arg(format!("renamed={}", rlib.display()))
            .arg(&oracle)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        fs::write(directory.join("compile.stderr"), &compiled.stderr).unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(executable).output().unwrap();
        fs::write(directory.join("native.stdout"), &output.stdout).unwrap();
        fs::write(directory.join("native.stderr"), &output.stderr).unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), actual);
    }
    {
        let (name, source, code) = (
            "disabled_name",
            "#![no_implicit_prelude]extern crate renamed as _;renamed::seed!();fn main(){}",
            "E0433",
        );
        let denied = evidence.join(format!("{name}.rs"));
        fs::write(&denied, source).unwrap();
        let compiled = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-name=denied",
                "--extern",
            ])
            .arg(format!("renamed={}", rlib.display()))
            .arg(denied)
            .arg("-o")
            .arg(evidence.join(name))
            .output()
            .unwrap();
        fs::write(evidence.join(format!("{name}.stderr")), &compiled.stderr).unwrap();
        assert!(!compiled.status.success());
        assert!(String::from_utf8(compiled.stderr).unwrap().contains(code));
    }
}

fn capture_edition_dependency(library: &str) -> (Vec<PreparedSourceView>, PreparedSourceInputs) {
    let leaf = Digest([203; 32]);
    let cfg = CfgSet::guest_default();
    let mut units = Vec::new();
    let mut keys = Vec::new();
    for (path, unit, text, edition) in [
        (
            "app/main.rs",
            APP,
            "renamed::outer!();fn main()->i64{made()}",
            2024,
        ),
        ("dependency/lib.rs", LIB, library, 2015),
        ("helper/lib.rs", leaf, SEED, 2024),
    ] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let path = SourcePath::new(path).unwrap();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        keys.push(SourceSelection {
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
        });
        units.push(SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        vec![
            edge("renamed"),
            SourceDependency {
                consumer: LIB,
                alias: "helper".into(),
                provider: leaf,
            },
        ],
    )
    .unwrap();
    let views = keys
        .iter()
        .map(|key| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(key).unwrap() else {
                panic!("edition dependency unit absent")
            };
            view
        })
        .collect();
    (views, inputs)
}
const LEGACY_OUTER: &str = "#[macro_export]macro_rules! outer{()=>{fn made()->i64{677_i64}}}";
#[test]
fn rust2015_dependency_use_requires_an_explicit_extern_binding() {
    let (views, _) = capture_edition_dependency(&format!(
        "use helper::seed as picked;picked!();{LEGACY_OUTER}"
    ));
    let mut work = discovery(&views);
    for _ in 0..8 {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => work
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Incomplete { .. } => return,
            _ => panic!("legacy implicit extern path accepted"),
        }
    }
    panic!("legacy implicit extern refusal did not reach fixed point")
}
#[test]
fn rust2015_dependency_explicit_alias_retains_actual_definition_unit() {
    let (views, inputs) = capture_edition_dependency(
        "pub extern crate helper as tools;#[macro_export]macro_rules! outer{()=>{$crate::tools::seed!();}}",
    );
    assert_eq!(run_captured(views, inputs), "137");
}
#[test]
fn rust2015_dependency_absolute_macro_import_starts_at_crate_root() {
    let (views, inputs) = capture_edition_dependency(
        "pub extern crate helper as tools;pub use ::tools::seed as picked;#[macro_export]macro_rules! outer{()=>{$crate::picked!();}}",
    );
    assert_eq!(run_captured(views, inputs), "137");
}
#[test]
fn rust2015_dependency_plain_use_import_starts_at_crate_root() {
    let (views, inputs) = capture_edition_dependency(
        "pub extern crate helper as tools;mod inner{pub use tools::seed as picked;}pub use inner::picked;#[macro_export]macro_rules! outer{()=>{$crate::picked!();}}",
    );
    assert_eq!(run_captured(views, inputs), "137");
}

#[test]
fn all_official_edition_entries_can_be_prepared_without_dependency_roles() {
    for edition in [2015, 2018, 2021, 2024] {
        let prepared = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path: SourcePath::new("entry.rs").unwrap(),
                crate_key: APP,
                module_path: vec![],
                profile: ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
                cfg: CfgSet::guest_default(),
                blob: SourceBlob::new(b"fn main(){}".to_vec()).unwrap(),
            }],
            Default::default(),
        )
        .unwrap();
        let selection = SourceSelection {
            path: SourcePath::new("entry.rs").unwrap(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: SourceBlob::new(b"fn main(){}".to_vec()).unwrap().content(),
                    profile: ProfileIdentity {
                        edition,
                        ..ProfileIdentity::current()
                    },
                },
                crate_key: APP,
                module_path: vec![],
                cfg: CfgSet::guest_default().fingerprint(),
            },
        };
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("official edition root absent");
        };
        assert_eq!(view.selection().outline.parse.profile.edition, edition);
    }
}
#[test]
fn rust2015_dependency_relative_invocation_remains_local_to_its_module() {
    let (views, inputs) = capture_edition_dependency(
        "mod inner{macro_rules! local{()=>{pub fn seed()->i64{683_i64}}}local!();}#[macro_export]macro_rules! outer{()=>{fn made()->i64{683_i64}}}",
    );
    assert_eq!(run_captured(views, inputs), "683");
}
#[test]
fn rust2015_dependency_imports_match_native_macos() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let evidence = root.join(format!(
        "artifacts/macos-first/m013-rust2015-extern-native-{}",
        std::process::id()
    ));
    fs::create_dir_all(&evidence).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-vV"])
        .output()
        .unwrap();
    assert!(version.status.success());
    let text = String::from_utf8(version.stdout.clone()).unwrap();
    assert!(text.contains("release: 1.99.0") && text.contains("host: aarch64-apple-darwin"));
    fs::write(evidence.join("rustc.stdout"), version.stdout).unwrap();
    let helper = evidence.join("helper.rs");
    fs::write(&helper, SEED).unwrap();
    let rlib = evidence.join("libhelper.rlib");
    let output = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name=helper",
            "--crate-type=rlib",
        ])
        .arg(&helper)
        .arg("-o")
        .arg(&rlib)
        .output()
        .unwrap();
    fs::write(evidence.join("helper.stderr"), &output.stderr).unwrap();
    assert!(output.status.success());
    for (ordinal,source) in [
        "helper::seed!();#[macro_export]macro_rules! outer{()=>{fn made()->i64{677_i64}}}",
        "pub extern crate helper as tools;#[macro_export]macro_rules! outer{()=>{$crate::tools::seed!();}}",
        "pub extern crate helper as tools;pub use ::tools::seed as picked;#[macro_export]macro_rules! outer{()=>{$crate::picked!();}}",
        "pub extern crate helper as tools;mod inner{pub use tools::seed as picked;}pub use inner::picked;#[macro_export]macro_rules! outer{()=>{$crate::picked!();}}",
    ].into_iter().enumerate() {
        let directory=evidence.join(ordinal.to_string());fs::create_dir_all(&directory).unwrap();
        let library=directory.join("lib.rs");fs::write(&library,source).unwrap();
        let native_lib=directory.join("librenamed.rlib");
        let compiled=Command::new("rustc").args(["+1.99.0","--edition=2015","--crate-name=renamed","--crate-type=rlib","--extern"]).arg(format!("helper={}",rlib.display())).arg(&library).arg("-o").arg(&native_lib).output().unwrap();
        fs::write(directory.join("library.stderr"),&compiled.stderr).unwrap();assert!(compiled.status.success(),"{}",String::from_utf8_lossy(&compiled.stderr));
        let app=directory.join("oracle.rs");fs::write(&app,"renamed::outer!();fn main(){::std::println!(\"{}\",made());}").unwrap();
        let executable=directory.join("oracle");
        let compiled=Command::new("rustc").args(["+1.99.0","--edition=2024","--crate-name=oracle","--extern"]).arg(format!("renamed={}",native_lib.display())).arg("-L").arg(format!("dependency={}",evidence.display())).arg(&app).arg("-o").arg(&executable).output().unwrap();
        fs::write(directory.join("compile.stderr"),&compiled.stderr).unwrap();assert!(compiled.status.success(),"{}",String::from_utf8_lossy(&compiled.stderr));
        let output=Command::new(&executable).output().unwrap();fs::write(directory.join("native.stdout"),&output.stdout).unwrap();assert!(output.status.success());
        let (views,inputs)=capture_edition_dependency(source);let actual=run_captured(views,inputs);fs::write(directory.join("vm.stdout"),&actual).unwrap();assert_eq!(String::from_utf8(output.stdout).unwrap().trim(),actual);
    }
    let denied = evidence.join("denied.rs");
    fs::write(
        &denied,
        format!("use helper::seed as picked;picked!();{LEGACY_OUTER}"),
    )
    .unwrap();
    let compiled = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2015",
            "--crate-name=denied",
            "--crate-type=rlib",
            "--extern",
        ])
        .arg(format!("helper={}", rlib.display()))
        .arg(&denied)
        .arg("-o")
        .arg(evidence.join("libdenied.rlib"))
        .output()
        .unwrap();
    fs::write(evidence.join("denied.stderr"), &compiled.stderr).unwrap();
    assert!(!compiled.status.success());
    assert!(
        String::from_utf8(compiled.stderr)
            .unwrap()
            .contains("E0432")
    );
}

#[test]
fn rust2015_dependency_qualified_invocation_uses_the_recorded_extern_prelude() {
    let (views, inputs) = capture_edition_dependency(&format!("helper::seed!();{LEGACY_OUTER}"));
    assert_eq!(run_captured(views, inputs), "677");
}
