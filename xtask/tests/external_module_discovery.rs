use lrr::{
    core::{
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
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn capture(files: &[(&str, &[&str], &str)]) -> (Vec<PreparedSourceView>, PreparedSourceInputs) {
    try_capture(files).unwrap()
}
fn try_capture(
    files: &[(&str, &[&str], &str)],
) -> Result<(Vec<PreparedSourceView>, PreparedSourceInputs), lrr::core::diagnostic::Diagnostic> {
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let unit = Digest([241; 32]);
    let mut keys = Vec::new();
    let mut units = Vec::new();
    for (path, module, text) in files {
        let path = SourcePath::new(*path).unwrap();
        let module = module.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        keys.push(SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: module.clone(),
                cfg: cfg.fingerprint(),
            },
        });
        units.push(SourceUnitInput {
            path,
            crate_key: unit,
            module_path: module,
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh(None, units, Default::default())?;
    let views = keys
        .iter()
        .map(|key| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(key).unwrap() else {
                panic!("capture absent")
            };
            view
        })
        .collect();
    Ok((views, inputs))
}
fn discover(views: &[PreparedSourceView]) -> SourceDiscovery {
    let root = views
        .iter()
        .find(|v| v.outline().root_scope.module_path.is_empty())
        .unwrap();
    SourceDiscovery::new(
        views.iter().map(PreparedSourceView::share).collect(),
        root.outline().root_scope.clone(),
        Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}
fn finish(work: &mut SourceDiscovery) {
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => work
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => work
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
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
            } => panic!("incomplete {diagnostic:?} {blocked:?}"),
        }
    }
}
fn run(files: &[(&str, &[&str], &str)]) -> String {
    let (views, inputs) = capture(files);
    let root = views
        .iter()
        .find(|v| v.outline().root_scope.module_path.is_empty())
        .unwrap();
    let body = root
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut work = discover(&views);
    finish(&mut work);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(work.key().epoch).unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_discovery(&mut db, body, 10_000, work)
    else {
        panic!("entry absent")
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("external-module run did not complete")
    };
    let ValueSummary::Signed(n) = report.value else {
        panic!("noninteger result")
    };
    n
}
#[test]
fn external_module_inherits_parent_macro_at_the_module_declaration() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{7_i64}}}mod child;macro_rules! choose{()=>{pub fn made()->i64{9_i64}}}fn main()->i64{child::made()}"
            ),
            ("child.rs", &["child"], "choose!();")
        ]),
        "7"
    );
}
#[test]
fn recursive_external_modules_keep_nearest_frozen_parent_macro() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{7_i64}}}mod child;fn main()->i64{child::grand::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! choose{()=>{pub fn made()->i64{11_i64}}}pub mod grand;macro_rules! choose{()=>{pub fn made()->i64{13_i64}}}"
            ),
            ("child/grand.rs", &["child", "grand"], "choose!();")
        ]),
        "11"
    );
}
#[test]
fn qualified_external_export_uses_its_actual_captured_macro_body() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "mod child;crate::seed!();fn main()->i64{made()}"
            ),
            (
                "child.rs",
                &["child"],
                "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}"
            )
        ]),
        "7"
    );
}
#[test]
fn external_module_local_macro_shadows_its_parent() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{7_i64}}}mod child;fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! choose{()=>{pub fn made()->i64{11_i64}}}choose!();"
            )
        ]),
        "11"
    );
}
#[test]
fn uncaptured_external_module_prevents_a_partial_seal() {
    let (views, _) = capture(&[("main.rs", &[], "mod missing;fn main()->i64{7_i64}")]);
    let mut work = discover(&views);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("missing module became complete")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn external_module_cannot_see_a_parent_macro_declared_later() {
    let (views, _) = capture(&[
        (
            "main.rs",
            &[],
            "mod child;macro_rules! choose{()=>{pub fn made()->i64{7_i64}}}",
        ),
        ("child.rs", &["child"], "choose!();"),
    ]);
    let mut work = discover(&views);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("future parent macro leaked")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn external_module_observes_a_parent_macro_from_completed_earlier_expansion() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! bootstrap{()=>{macro_rules! choose{()=>{pub fn made()->i64{17_i64}}}}}bootstrap!();mod child;fn main()->i64{child::made()}"
            ),
            ("child.rs", &["child"], "choose!();")
        ]),
        "17"
    );
}

fn refused_capture(views: Vec<PreparedSourceView>) {
    let root = views
        .iter()
        .find(|v| v.outline().root_scope.module_path.is_empty())
        .unwrap();
    let scope = root.outline().root_scope.clone();
    let work = SourceDiscovery::new(views, scope, Namespace::Value, Digest([245; 32]), 1).unwrap();
    let error = work
        .with_declarative(Digest([243; 32]), Digest([244; 32]))
        .err()
        .expect("invalid captured parent was admitted");
    assert_eq!(error.code, "LRR-E-MACRO-SCOPE");
}

#[test]
fn external_module_refuses_a_different_parent_publication() {
    let (mut root, _) = capture(&[("main.rs", &[], "mod child;")]);
    let (child, _) = capture(&[("child.rs", &["child"], "")]);
    root.extend(child);
    refused_capture(root);
}

#[test]
fn external_module_refuses_an_undeclared_captured_parent() {
    let (views, _) = capture(&[("main.rs", &[], ""), ("child.rs", &["child"], "")]);
    refused_capture(views);
}

#[test]
fn external_module_capture_refuses_duplicate_parent_definition_identity() {
    let error = try_capture(&[
        ("main.rs", &[], "mod child;"),
        ("other.rs", &[], "fn anchor(){}mod child;"),
        ("child.rs", &["child"], ""),
    ])
    .err()
    .expect("duplicate parent definition identity was captured");
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(
        error.message,
        "definition identity appears in multiple source contexts"
    );
}

#[test]
fn external_module_import_alias_uses_the_frozen_parent_macro() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{19_i64}}}mod child;macro_rules! choose{()=>{pub fn made()->i64{23_i64}}}fn main()->i64{child::made()}"
            ),
            ("child.rs", &["child"], "use choose as picked;picked!();")
        ]),
        "19"
    );
}

#[test]
fn external_module_import_alias_keeps_nearest_parent_shadowing() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{19_i64}}}mod child;fn main()->i64{child::grand::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! choose{()=>{pub fn made()->i64{29_i64}}}pub mod grand;"
            ),
            (
                "child/grand.rs",
                &["child", "grand"],
                "use choose as picked;picked!();"
            )
        ]),
        "29"
    );
}

#[test]
fn external_module_import_alias_cannot_capture_a_future_parent_macro() {
    let (views, _) = capture(&[
        (
            "main.rs",
            &[],
            "mod child;macro_rules! choose{()=>{pub fn made()->i64{19_i64}}}",
        ),
        ("child.rs", &["child"], "use choose as picked;picked!();"),
    ]);
    let mut work = discover(&views);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("future parent macro import became complete")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn external_module_import_alias_observes_a_completed_parent_expansion() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! bootstrap{()=>{macro_rules! choose{()=>{pub fn made()->i64{31_i64}}}}}bootstrap!();mod child;fn main()->i64{child::made()}"
            ),
            ("child.rs", &["child"], "use choose as picked;picked!();")
        ]),
        "31"
    );
}

#[test]
fn external_module_import_alias_preserves_child_local_precedence() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{19_i64}}}mod child;fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! choose{()=>{pub fn made()->i64{37_i64}}}use choose as picked;picked!();"
            )
        ]),
        "37"
    );
}

#[test]
fn external_generated_call_inherits_its_captured_parent_macro() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{41_i64}}}mod child;macro_rules! choose{()=>{pub fn made()->i64{43_i64}}}fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! wrap{()=>{choose!();}}wrap!();"
            )
        ]),
        "41"
    );
}

#[test]
fn external_generated_call_uses_a_captured_qualified_owner() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "mod child;mod owner;fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! wrap{()=>{crate::seed!();}}wrap!();"
            ),
            (
                "owner.rs",
                &["owner"],
                "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{47_i64}}}"
            )
        ]),
        "47"
    );
}

#[test]
fn external_generated_import_uses_its_frozen_parent_macro() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{53_i64}}}mod child;macro_rules! choose{()=>{pub fn made()->i64{59_i64}}}fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! wrap{()=>{use choose as picked;picked!();}}wrap!();"
            )
        ]),
        "53"
    );
}

#[test]
fn external_nested_generated_call_keeps_nearest_parent_macro() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{61_i64}}}mod child;fn main()->i64{child::grand::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! choose{()=>{pub fn made()->i64{67_i64}}}pub mod grand;"
            ),
            (
                "child/grand.rs",
                &["child", "grand"],
                "macro_rules! wrap{()=>{macro_rules! inner{()=>{choose!();}}inner!();}}wrap!();"
            )
        ]),
        "67"
    );
}

#[test]
fn external_generated_call_observes_completed_generated_parent_binding() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! bootstrap{()=>{macro_rules! choose{()=>{pub fn made()->i64{71_i64}}}}}bootstrap!();mod child;fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! wrap{()=>{choose!();}}wrap!();"
            )
        ]),
        "71"
    );
}

#[test]
fn external_generated_call_cannot_see_a_future_parent_macro() {
    let (views, _) = capture(&[
        (
            "main.rs",
            &[],
            "mod child;macro_rules! choose{()=>{pub fn made()->i64{73_i64}}}",
        ),
        (
            "child.rs",
            &["child"],
            "macro_rules! wrap{()=>{choose!();}}wrap!();",
        ),
    ]);
    let mut work = discover(&views);
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => work
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                assert!(!blocked.is_empty());
                assert!(work.sealed_expansions().is_err());
                break;
            }
            _ => panic!("future generated parent call became ready or sealed"),
        }
    }
}

#[test]
fn external_generated_import_enables_a_later_original_call() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{79_i64}}}mod child;fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! wrap{()=>{use choose as picked;}}wrap!();picked!();"
            )
        ]),
        "79"
    );
}

#[test]
fn external_child_generated_import_retains_its_captured_route_snapshot() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! choose{()=>{pub fn made()->i64{83_i64}}}mod child;fn main()->i64{child::made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! wrap{()=>{macro_rules! inner{()=>{use choose as picked;}}inner!();}}wrap!();picked!();"
            )
        ]),
        "83"
    );
}

#[test]
fn completed_generated_textual_import_seals_without_a_later_original_call() {
    assert_eq!(
        run(&[(
            "main.rs",
            &[],
            "macro_rules! choose{()=>{pub fn made()->i64{89_i64}}}macro_rules! wrap{()=>{use choose as picked;picked!();}}wrap!();fn main()->i64{made()}"
        )]),
        "89"
    );
}

#[test]
fn captured_macro_use_module_exports_original_definition_to_parent() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "#[macro_use]mod child;leaf!();fn main()->i64{made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! leaf{()=>{pub fn made()->i64{521_i64}}}"
            )
        ]),
        "521"
    );
}

#[test]
fn captured_macro_use_module_waits_for_its_generated_definition() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "#[macro_use]mod child;leaf!();fn main()->i64{made()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn made()->i64{523_i64}}}}}seed!();"
            )
        ]),
        "523"
    );
}

#[test]
fn nested_captured_macro_use_modules_export_through_each_source_lease() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "#[macro_use]mod child;leaf!();fn main()->i64{made()}"
            ),
            ("child.rs", &["child"], "#[macro_use]mod grand;"),
            (
                "child/grand.rs",
                &["child", "grand"],
                "macro_rules! leaf{()=>{pub fn made()->i64{541_i64}}}"
            )
        ]),
        "541"
    );
}

#[test]
fn captured_macro_use_module_shadowing_preserves_calls_before_declaration() {
    assert_eq!(
        run(&[
            (
                "main.rs",
                &[],
                "macro_rules! leaf{()=>{pub fn before()->i64{547_i64}}}leaf!();#[macro_use]mod child;leaf!();fn main()->i64{before()+after()}"
            ),
            (
                "child.rs",
                &["child"],
                "macro_rules! leaf{()=>{pub fn after()->i64{557_i64}}}"
            )
        ]),
        "1104"
    );
}

#[test]
fn captured_macro_use_unknown_child_keeps_parent_exports_unpublished() {
    let (views, inputs) = capture(&[
        (
            "main.rs",
            &[],
            "#[macro_use]mod child;leaf!();fn main()->i64{made()}",
        ),
        (
            "child.rs",
            &["child"],
            "unknown!();macro_rules! leaf{()=>{pub fn made()->i64{563_i64}}}",
        ),
    ]);
    let mut work = discover(&views);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("unknown child producer exposed parent export");
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.lookup("made").is_err());
    assert!(work.sealed_expansions().is_err());
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let revision = db.revision();
    assert!(work.publish(&mut db).is_err());
    assert_eq!(db.revision(), revision);
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            work.key().clone()
        ))
        .is_none()
    );
}

#[test]
fn captured_macro_use_refuses_a_child_from_a_different_publication() {
    let (mut root, _) = capture(&[("main.rs", &[], "#[macro_use]mod child;leaf!();")]);
    let (child, _) = capture(&[(
        "child.rs",
        &["child"],
        "macro_rules! leaf{()=>{pub fn made(){}}}",
    )]);
    root.extend(child);
    refused_capture(root);
}

#[test]
fn captured_macro_use_child_edit_invalidates_ready_parent_seal() {
    use lrr::db::{key::QueryKey, provider::SyntaxProvider};
    let files = [
        (
            "main.rs",
            &[][..],
            "#[macro_use]mod child;leaf!();fn main()->i64{made()}",
        ),
        (
            "child.rs",
            &["child"][..],
            "macro_rules! leaf{()=>{pub fn made()->i64{569_i64}}}",
        ),
    ];
    let (views, inputs) = capture(&files);
    let mut old = discover(&views);
    finish(&mut old);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    old.publish(&mut db).unwrap();
    let seal = QueryKey::DiscoverySeal(old.key().clone());
    assert!(matches!(
        db.fetch(&SyntaxProvider, seal.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    let edited = [
        (files[0].0, files[0].1, files[0].2),
        (
            "child.rs",
            &["child"][..],
            "macro_rules! leaf{()=>{pub fn made()->i64{571_i64}}}",
        ),
    ];
    let (changed, changed_inputs) = capture(&edited);
    changed_inputs.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, seal),
        QueryPoll::NeedInput(_)
    ));
    let revision = db.revision();
    assert_eq!(
        old.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.revision(), revision);
    let root = changed
        .iter()
        .find(|v| v.outline().root_scope.module_path.is_empty())
        .unwrap();
    let body = root
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut updated = discover(&changed);
    finish(&mut updated);
    db.begin_expansion_epoch(updated.key().epoch).unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_discovery(&mut db, body, 10_000, updated)
    else {
        panic!("updated parent not ready");
    };
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("updated export did not execute");
    };
    assert!(matches!(report.value, ValueSummary::Signed(ref n) if n == "571"));
}

#[test]
fn captured_macro_use_examples_match_native_macos() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("artifacts/macos-first/m013-captured-macro-use-oracles");
    fs::create_dir_all(&root).unwrap();
    let work = root.join(format!(
        "run-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    fs::write(work.join("rustc-version.txt"), &version.stdout).unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    let cases = [
        (
            "original",
            vec![
                (
                    "main.rs",
                    vec![],
                    "#[macro_use]mod child;leaf!();fn main()->i64{made()}",
                ),
                (
                    "child.rs",
                    vec!["child"],
                    "macro_rules! leaf{()=>{pub fn made()->i64{521_i64}}}",
                ),
            ],
            "521",
        ),
        (
            "generated",
            vec![
                (
                    "main.rs",
                    vec![],
                    "#[macro_use]mod child;leaf!();fn main()->i64{made()}",
                ),
                (
                    "child.rs",
                    vec!["child"],
                    "macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn made()->i64{523_i64}}}}}seed!();",
                ),
            ],
            "523",
        ),
        (
            "nested",
            vec![
                (
                    "main.rs",
                    vec![],
                    "#[macro_use]mod child;leaf!();fn main()->i64{made()}",
                ),
                ("child.rs", vec!["child"], "#[macro_use]mod grand;"),
                (
                    "child/grand.rs",
                    vec!["child", "grand"],
                    "macro_rules! leaf{()=>{pub fn made()->i64{541_i64}}}",
                ),
            ],
            "541",
        ),
        (
            "shadow",
            vec![
                (
                    "main.rs",
                    vec![],
                    "macro_rules! leaf{()=>{pub fn before()->i64{547_i64}}}leaf!();#[macro_use]mod child;leaf!();fn main()->i64{before()+after()}",
                ),
                (
                    "child.rs",
                    vec!["child"],
                    "macro_rules! leaf{()=>{pub fn after()->i64{557_i64}}}",
                ),
            ],
            "1104",
        ),
    ];
    for (name, files, expected) in cases {
        let fixture = files
            .iter()
            .map(|(path, scope, text)| (*path, scope.as_slice(), *text))
            .collect::<Vec<_>>();
        assert_eq!(run(&fixture), expected);
        let case = work.join(name);
        fs::create_dir(&case).unwrap();
        for (path, _, text) in &files {
            let target = case.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            let text = text
                .replace(
                    "fn main()->i64{made()}",
                    "fn main(){println!(\"{}\",made());}",
                )
                .replace(
                    "fn main()->i64{before()+after()}",
                    "fn main(){println!(\"{}\",before()+after());}",
                );
            fs::write(target, text).unwrap();
        }
        let compiled = Command::new("rustc")
            .args(["+1.99.0", "--edition=2024"])
            .arg(case.join("main.rs"))
            .arg("-o")
            .arg(case.join("native"))
            .output()
            .unwrap();
        fs::write(case.join("compile.stdout"), &compiled.stdout).unwrap();
        fs::write(case.join("compile.stderr"), &compiled.stderr).unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let native = Command::new(case.join("native")).output().unwrap();
        fs::write(case.join("native.stdout"), &native.stdout).unwrap();
        fs::write(case.join("native.stderr"), &native.stderr).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), expected);
    }
}
