use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const GENERATED_SHADOW: &str = "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{macro_rules! provide{()=>{pub fn wrong(){}}}pub fn made()->i64{223_i64}}}}}#[macro_export]macro_rules! relay{()=>{provide!();}}seed!();crate::relay!();fn main()->i64{made()}";
const SCHEDULING_SHADOW: &str = "macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{241_i64}}}macro_rules! picked{()=>{pub fn other()->i64{251_i64}}}}}macro_rules! outer{()=>{use crate::later as picked;picked!();provide!();picked!();}}outer!();fn main()->i64{made()+other()}";
const BARE_SHADOW: &str = "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{macro_rules! provide{()=>{pub fn wrong(){}}}pub fn made()->i64{197_i64}}}}}seed!();provide!();fn main()->i64{made()}";
const ORIGINAL_IMPORT_SHADOW: &str = "#[macro_export]macro_rules! later{()=>{pub fn wrong()->i64{251_i64}}}use crate::later as picked;macro_rules! outer{()=>{macro_rules! picked{()=>{pub fn made()->i64{269_i64}}}}}outer!();picked!();fn main()->i64{made()}";
const PARKED_IMPORT_SHADOW: &str = "macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{307_i64}}}macro_rules! picked{()=>{pub fn wrong()->i64{311_i64}}}}}macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! outer{()=>{seed!();provide!();}}outer!();fn main()->i64{made()}";
const INLINE_MODULE: &str = "macro_rules! outer{()=>{pub mod emitted{pub mod nested{pub fn value()->i64{317_i64}pub fn made()->i64{value()}}}}}outer!();";
const INLINE_MODULE_IMPORT: &str = "macro_rules! outer{()=>{pub mod emitted{pub fn value()->i64{331_i64}pub use self::value as picked;}pub use emitted::picked as made;}}outer!();";
const INLINE_MODULE_CFG: &str = "macro_rules! outer{()=>{#[cfg(any())]pub mod absent{missing!();}pub mod emitted{#[cfg(any())]pub fn disabled(){let _:Missing=missing();}pub fn value()->i64{337_i64}}}}outer!();";
const MACRO_USE_SHADOW: &str = "macro_rules! leaf{()=>{pub fn old()->i64{487_i64}}}macro_rules! outer{()=>{#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{491_i64}}}}}}outer!();leaf!();fn main()->i64{made()}";
fn source(text: &str) -> (PreparedSourceView, PreparedSourceInputs) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([241; 32]);
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
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    (view, inputs)
}

fn discover(v: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}

fn finish(w: &mut SourceDiscovery) {
    loop {
        match w.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            SourceDiscoveryStep::NeedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                panic!("fixture failed to complete discovery: {diagnostic:?} {blocked:?}")
            }
        }
    }
}
fn run_original(text: &str) -> (lrr::engine::run::RunReport, lrr::db::memo::Database) {
    let (v, inputs) = source(text);
    let mut work = discover(&v);
    finish(&mut work);
    let body = v
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(work.key().epoch).unwrap();
    let mut run =
        match lrr::engine::run::RunMachine::start_with_discovery(&mut db, body, 10_000, work) {
            lrr::db::poll::QueryPoll::Ready(Ok(run)) => run,
            lrr::db::poll::QueryPoll::Ready(Err(error)) => {
                panic!("original caller entry failed: {error:?}")
            }
            lrr::db::poll::QueryPoll::NeedInput(input) => {
                panic!("original caller entry incomplete: {input:?}")
            }
        };
    let report = match run.poll(&mut db) {
        lrr::engine::run::RunPoll::Completed(report) => report,
        lrr::engine::run::RunPoll::Fault(error) => panic!(
            "original generated call fault {}: {}",
            error.code, error.message
        ),
        _ => panic!("original generated call did not complete"),
    };
    (report, db)
}
#[test]
fn macro_generated_import_and_macro_enable_a_later_original_invocation() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}use emitted as selected;}}outer!();selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_macro_export_is_selected_by_its_qualified_canonical_identity() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{#[macro_export]macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}}}outer!();crate::emitted!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_import_of_original_export_uses_the_captured_original_body() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}macro_rules! outer{()=>{use crate::seed as selected;}}outer!();selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_unresolved_macro_import_keeps_the_real_fixed_point_incomplete() {
    let (v, _) = source(
        "macro_rules! outer{()=>{use crate::missing as selected;}}outer!();selected!();fn main()->i64{7_i64}",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer not selected")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("missing generated import became complete")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn cfg_disabled_generated_macro_import_does_not_create_a_route() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{#[cfg(any())]use crate::missing as selected;pub fn made()->i64{7_i64}}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}

#[test]
fn generated_import_request_keeps_the_macro_header_canonical_identity() {
    let (v, _) = source(
        "macro_rules! outer{()=>{macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}use emitted as selected;}}outer!();selected!();",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    let items =
        lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
            .unwrap();
    let canonical = items
        .items()
        .iter()
        .find(|h| h.name.as_ref().is_some_and(|n| n.text == "emitted"))
        .unwrap()
        .key
        .fingerprint();
    work.admit_expansion(record).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("import target absent")
    };
    assert_eq!(
        request.key().macro_identity,
        lrr::expand::identity::MacroIdentity::Declaration {
            definition: canonical
        }
    );
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    finish(&mut work);
    assert_eq!(work.sealed_expansions().unwrap().len(), 2);
}
#[test]
fn generated_textual_macro_import_retains_destination_ambiguity() {
    let (v, _) = source(
        "#[macro_export]macro_rules! trigger{()=>{}}macro_rules! outer{()=>{macro_rules! emitted{()=>{}}#[macro_export]macro_rules! selected{()=>{}}use emitted as selected;}}outer!();crate::trigger!();",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("trigger absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("ambiguous generated import was certified")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn generated_macro_import_retries_after_an_independent_later_expansion() {
    let (report, _) = run_original(
        "macro_rules! seed{()=>{use crate::later as picked;picked!();}}#[macro_export]macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{163_i64}}}}}seed!();crate::provide!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="163"));
}

#[test]
fn bare_declarative_producer_unblocks_the_preserved_import_retry_witness() {
    let (report, _) = run_original(
        "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{163_i64}}}}}seed!();provide!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="163"));
}

#[test]
fn provisional_bare_selection_cannot_publish_after_generated_shadowing() {
    let (v, inputs) = source(BARE_SHADOW);
    let mut work = discover(&v);
    let mut expansions = 0;
    let error = loop {
        match work.poll() {
            Ok(
                SourceDiscoveryStep::NeedDeriveAppend { .. }
                | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
                | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
                | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
                | SourceDiscoveryStep::NeedDeriveDeclaration { .. },
            ) => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                expansions += 1;
                assert!(expansions <= 4);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            Ok(SourceDiscoveryStep::NeedGeneratedExpansion { request }) => {
                expansions += 1;
                assert!(expansions <= 4);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            Err(error) => break error,
            Ok(SourceDiscoveryStep::NeedProceduralSelection { .. })
            | Ok(SourceDiscoveryStep::NeedAttributeSelection { .. }) => {
                panic!("unexpected procedural selection in this fixture")
            }
            Ok(SourceDiscoveryStep::Sealed(_)) => panic!("changed provisional selection sealed"),
            Ok(SourceDiscoveryStep::Incomplete { .. }) => {
                panic!("independent producer was never expanded")
            }
        }
    };
    assert_eq!(expansions, 3);
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert_eq!(work.poll().err().unwrap().code, error.code);
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            work.key().clone()
        ))
        .is_none()
    );
}

#[test]
fn bare_interleaving_shadow_refusal_matches_macos_native_origin_ambiguity() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("artifacts/macos-first/m013-bare-interleaving-oracles");
    fs::create_dir_all(&root).unwrap();
    let work = root.join(format!(
        "shadow-{}-{}",
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
    let nested = GENERATED_SHADOW.replace(
        "#[macro_export]macro_rules! relay{()=>{provide!();}}",
        "macro_rules! middle{()=>{provide!();}}#[macro_export]macro_rules! relay{()=>{middle!();}}",
    );
    let earlier_child = PARKED_IMPORT_SHADOW.replace("seed!();provide!();", "provide!();seed!();");
    let earlier_nested = PARKED_IMPORT_SHADOW.replace(
        "macro_rules! outer{()=>{seed!();provide!();}}",
        "macro_rules! middle{()=>{seed!();}}macro_rules! outer{()=>{provide!();middle!();}}",
    );
    for (name, text) in [
        ("earlier_child_import", earlier_child.as_str()),
        ("earlier_grandchild_import", earlier_nested.as_str()),
        ("macro_use_shadow", MACRO_USE_SHADOW),
        ("bare", BARE_SHADOW),
        ("generated", GENERATED_SHADOW),
        ("nested", nested.as_str()),
        ("scheduling", SCHEDULING_SHADOW),
        ("original_import", ORIGINAL_IMPORT_SHADOW),
    ] {
        let case = work.join(name);
        fs::create_dir(&case).unwrap();
        fs::write(
            case.join("main.rs"),
            text.replace("fn main()->i64{made()}", "fn main(){let _ = made();}")
                .replace(
                    "fn main()->i64{made()+other()}",
                    "fn main(){let _ = made()+other();}",
                ),
        )
        .unwrap();
        let compile = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "-A",
                "macro_expanded_macro_exports_accessed_by_absolute_paths",
            ])
            .arg(case.join("main.rs"))
            .arg("-o")
            .arg(case.join("native"))
            .output()
            .unwrap();
        fs::write(case.join("compile.stdout"), &compile.stdout).unwrap();
        fs::write(case.join("compile.stderr"), &compile.stderr).unwrap();
        assert!(!compile.status.success());
        assert!(
            String::from_utf8_lossy(&compile.stderr).contains(if name == "macro_use_shadow" {
                "error[E0659]: `leaf` is ambiguous"
            } else if matches!(
                name,
                "scheduling"
                    | "original_import"
                    | "earlier_child_import"
                    | "earlier_grandchild_import"
            ) {
                "error[E0659]: `picked` is ambiguous"
            } else {
                "error[E0659]: `provide` is ambiguous"
            })
        );
    }
}

#[test]
fn generated_macro_import_retries_after_an_independent_earlier_expansion() {
    let (report, _) = run_original(
        "macro_rules! seed{()=>{use crate::later as picked;picked!();}}#[macro_export]macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{167_i64}}}}}crate::provide!();seed!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="167"));
}

#[test]
fn generated_qualified_producer_unblocks_an_earlier_generated_import() {
    let (report, _) = run_original(
        "macro_rules! seed{()=>{use crate::later as picked;picked!();}}#[macro_export]macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{179_i64}}}}}#[macro_export]macro_rules! relay{()=>{crate::provide!();}}seed!();crate::relay!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="179"));
}

#[test]
fn generated_bare_relay_unblocks_an_earlier_generated_import() {
    let (report, _) = run_original(
        "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{211_i64}}}}}#[macro_export]macro_rules! relay{()=>{provide!();}}seed!();crate::relay!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="211"));
}

#[test]
fn generated_fragment_schedules_later_independent_producer_before_blocked_import_call() {
    let (report, _) = run_original(
        "macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{233_i64}}}}}macro_rules! outer{()=>{use crate::later as picked;picked!();provide!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="233"));
}

#[test]
fn blocked_nested_child_allows_an_independent_parent_sibling_to_supply_its_macro() {
    let (report, _) = run_original(
        "macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{277_i64}}}}}macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! outer{()=>{seed!();provide!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="277"));
}

#[test]
fn multiple_parked_descendants_retain_their_exact_parent_and_source_order() {
    let (report, _) = run_original(
        "macro_rules! seed_a{()=>{use crate::later_a as picked_a;picked_a!();}}macro_rules! seed_b{()=>{use crate::later_b as picked_b;picked_b!();}}macro_rules! seeds{()=>{seed_a!();seed_b!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later_a{()=>{pub fn a()->i64{283_i64}}}#[macro_export]macro_rules! later_b{()=>{pub fn b()->i64{293_i64}}}}}macro_rules! outer{()=>{seeds!();provide!();}}outer!();fn made()->i64{a()+b()}fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="576"));
}

#[test]
fn parked_child_import_excludes_a_later_sibling_textual_binding() {
    let (report, _) = run_original(PARKED_IMPORT_SHADOW);
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="307"));
}

#[test]
fn parked_grandchild_import_excludes_a_later_sibling_textual_binding() {
    let text = PARKED_IMPORT_SHADOW.replace(
        "macro_rules! outer{()=>{seed!();provide!();}}",
        "macro_rules! middle{()=>{seed!();}}macro_rules! outer{()=>{middle!();provide!();}}",
    );
    let (report, _) = run_original(&text);
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="307"));
}

#[test]
fn earlier_sibling_textual_origin_conflicts_with_a_generated_child_import() {
    let text = PARKED_IMPORT_SHADOW.replace("seed!();provide!();", "provide!();seed!();");
    refuse_generated_shadow(&text, 3);
}

#[test]
fn earlier_sibling_textual_origin_conflicts_with_a_generated_grandchild_import() {
    let text = PARKED_IMPORT_SHADOW.replace(
        "macro_rules! outer{()=>{seed!();provide!();}}",
        "macro_rules! middle{()=>{seed!();}}macro_rules! outer{()=>{provide!();middle!();}}",
    );
    refuse_generated_shadow(&text, 4);
}

#[test]
fn sibling_order_edit_invalidates_a_parked_selection_seal_before_conflict_and_restore() {
    use lrr::db::{
        dependency::InputKey, key::QueryKey, memo::Database, poll::QueryPoll,
        provider::SyntaxProvider,
    };
    let (original, inputs) = source(PARKED_IMPORT_SHADOW);
    let mut completed = discover(&original);
    finish(&mut completed);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    completed.publish(&mut db).unwrap();
    let key = QueryKey::DiscoverySeal(completed.key().clone());
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::Ready(Ok(_))
    ));

    let text = PARKED_IMPORT_SHADOW.replace("seed!();provide!();", "provide!();seed!();");
    let (changed, changed_inputs) = source(&text);
    changed_inputs.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::NeedInput(_)
    ));
    let revision = db.revision();
    assert_eq!(
        completed.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.revision(), revision);
    let mut conflicting = discover(&changed);
    let mut records = Vec::new();
    let error = loop {
        let record = match conflicting.poll() {
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                std::sync::Arc::new(request.expand().unwrap())
            }
            Ok(SourceDiscoveryStep::NeedGeneratedExpansion { request }) => {
                std::sync::Arc::new(request.expand().unwrap())
            }
            Err(error) => break error,
            Ok(_) => panic!("edited conflicting origins did not refuse selection"),
        };
        records.push(record.key().fingerprint());
        assert!(records.len() <= 3);
        conflicting.admit_expansion(record).unwrap();
    };
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert_eq!(records.len(), 3);
    let revision = db.revision();
    assert!(conflicting.publish(&mut db).is_err());
    assert_eq!(db.revision(), revision);
    assert!(
        db.input(&InputKey::DiscoverySeal(conflicting.key().clone()))
            .is_none()
    );
    for record in records {
        assert!(db.input(&InputKey::ExpansionRecord(record)).is_none());
    }

    let (restored, restored_inputs) = source(PARKED_IMPORT_SHADOW);
    restored_inputs.publish(&mut db).unwrap();
    let mut work = discover(&restored);
    finish(&mut work);
    db.begin_expansion_epoch(work.key().epoch).unwrap();
    work.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, key),
        QueryPoll::Ready(Ok(_))
    ));
    let body = restored
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
        lrr::engine::run::RunMachine::start_with_discovery(&mut db, body, 10_000, work)
    else {
        panic!("restored source failed to start");
    };
    let lrr::engine::run::RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("restored parked selection failed to execute");
    };
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="307"));
}

#[test]
fn generated_inline_nested_modules_preserve_member_call_identity() {
    let (report, _) = run_original(&format!(
        "{INLINE_MODULE}fn main()->i64{{emitted::nested::made()}}"
    ));
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="317"));
}

#[test]
fn generated_inline_module_imports_resolve_inside_and_outside_the_container() {
    let (report, _) = run_original(&format!("{INLINE_MODULE_IMPORT}fn main()->i64{{made()}}"));
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="331"));
}

#[test]
fn generated_inline_module_cfg_excludes_disabled_producers_and_cold_headers() {
    let (report, _) = run_original(&format!(
        "{INLINE_MODULE_CFG}fn main()->i64{{emitted::value()}}"
    ));
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="337"));
}

#[test]
fn generated_inline_module_alias_preserves_nested_member_identity() {
    let (report, _) = run_original(&format!(
        "{INLINE_MODULE}use emitted as chosen;fn main()->i64{{chosen::nested::made()}}"
    ));
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="317"));
}

#[test]
fn generated_inline_sibling_modules_keep_same_named_members_distinct() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{pub mod left{pub fn value()->i64{347_i64}}pub mod right{pub fn value()->i64{349_i64}}}}outer!();fn main()->i64{left::value()+right::value()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="696"));
}

#[test]
fn generated_inline_module_unknown_producer_keeps_partial_members_unpublished() {
    let (view, inputs) = source(
        "macro_rules! outer{()=>{pub mod emitted{unknown!();pub fn known()->i64{353_i64}}}}outer!();fn main()->i64{emitted::known()}",
    );
    let mut work = discover(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer not selected");
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    let record_key = record.key().fingerprint();
    work.admit_expansion(record).unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("unknown nested producer sealed");
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(blocked.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    assert!(work.lookup("emitted").is_err());
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    let revision = db.revision();
    assert!(work.publish(&mut db).is_err());
    assert_eq!(db.revision(), revision);
    assert!(
        db.input(&lrr::db::dependency::InputKey::ExpansionRecord(record_key))
            .is_none()
    );
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            work.key().clone()
        ))
        .is_none()
    );
}

#[test]
fn generated_module_local_item_macro_emits_a_scoped_callable() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn made()->i64{367_i64}}}leaf!();}}}outer!();fn main()->i64{emitted::made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="367"));
}

#[test]
fn generated_module_item_macro_inherits_the_captured_original_scope() {
    let (report, _) = run_original(
        "macro_rules! leaf{()=>{pub fn made()->i64{373_i64}}}macro_rules! outer{()=>{pub mod emitted{leaf!();}}}outer!();fn main()->i64{emitted::made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="373"));
}

#[test]
fn generated_sibling_module_macros_keep_their_lexical_definitions_distinct() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{pub mod left{macro_rules! leaf{()=>{pub fn made()->i64{379_i64}}}leaf!();}pub mod right{macro_rules! leaf{()=>{pub fn made()->i64{383_i64}}}leaf!();}}}outer!();fn main()->i64{left::made()+right::made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="762"));
}

#[test]
fn generated_module_recursive_macro_output_retains_child_scope_frontiers() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{pub mod emitted{macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn made()->i64{401_i64}}}leaf!();}}seed!();}}}outer!();fn main()->i64{emitted::made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="401"));
}

#[test]
fn generated_module_named_macro_import_preserves_its_lexical_definition() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn made()->i64{409_i64}}}use leaf as picked;picked!();}}}outer!();fn main()->i64{emitted::made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="409"));
}

#[test]
fn generated_module_local_macro_does_not_leak_into_the_original_parent_scope() {
    let (view, _) = source(
        "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn made()->i64{419_i64}}}leaf!();}}}outer!();leaf!();fn main()->i64{emitted::made()}",
    );
    let mut work = discover(&view);
    let mut count = 0;
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            SourceDiscoveryStep::NeedExpansion { request } => {
                count += 1;
                assert!(count <= 2);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                count += 1;
                assert!(count <= 2);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                assert_eq!(count, 2);
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                assert!(!blocked.is_empty());
                break;
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => {
                panic!("module-local macro leaked into parent source scope")
            }
        }
    }
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn generated_module_macro_shadowing_freezes_each_lexical_scope() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{macro_rules! leaf{()=>{pub fn root()->i64{433_i64}}}pub mod emitted{macro_rules! leaf{()=>{pub fn made()->i64{439_i64}}}leaf!();}}}outer!();leaf!();fn main()->i64{emitted::made()+root()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="872"));
}

#[test]
fn generated_module_macro_reexport_is_selected_by_an_original_qualified_import() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn made()->i64{449_i64}}}pub(crate) use leaf as renamed;}}}outer!();use emitted::renamed as picked;picked!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="449"));
}

#[test]
fn generated_module_local_macro_parent_rejection_matches_native_macos() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("artifacts/macos-first/m013-module-macro-oracles");
    fs::create_dir_all(&root).unwrap();
    let case = root.join(format!(
        "no-leak-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&case).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    fs::write(case.join("rustc-version.txt"), &version.stdout).unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(case.join("main.rs"), "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn made()->i64{419_i64}}}leaf!();}}}outer!();leaf!();fn main(){let _ = emitted::made();}").unwrap();
    let compiled = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(case.join("main.rs"))
        .arg("-o")
        .arg(case.join("native"))
        .output()
        .unwrap();
    fs::write(case.join("compile.stdout"), &compiled.stdout).unwrap();
    fs::write(case.join("compile.stderr"), &compiled.stderr).unwrap();
    assert!(!compiled.status.success());
    assert!(String::from_utf8_lossy(&compiled.stderr).contains("cannot find macro `leaf`"));
}

#[test]
fn original_inline_macro_use_module_extends_parent_textual_scope() {
    let (report, _) = run_original(
        "#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{457_i64}}}}leaf!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="457"));
}

#[test]
fn generated_macro_use_module_feeds_a_later_generated_item_call() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{461_i64}}}}leaf!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="461"));
}

#[test]
fn generated_macro_use_module_feeds_a_later_original_item_call() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{467_i64}}}}}}outer!();leaf!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="467"));
}

#[test]
fn recursive_generated_macro_use_module_retains_exported_child_definitions() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{macro_rules! spawn{()=>{#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{479_i64}}}}}}spawn!();leaf!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="479"));
}

#[test]
fn macro_use_module_shadowing_changes_only_calls_after_its_close() {
    let (report, _) = run_original(
        "macro_rules! leaf{()=>{pub fn before()->i64{487_i64}}}leaf!();#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn after()->i64{491_i64}}}}leaf!();fn main()->i64{before()+after()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="978"));
}

#[test]
fn nested_generated_macro_use_modules_export_through_each_parent_close() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{#[macro_use]mod first{#[macro_use]mod second{macro_rules! leaf{()=>{pub fn made()->i64{503_i64}}}}}leaf!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="503"));
}

#[test]
fn generated_macro_use_export_preserves_native_origin_ambiguity() {
    refuse_generated_shadow(MACRO_USE_SHADOW, 1);
}

#[test]
fn macro_use_module_unknown_producer_keeps_parent_seal_unavailable() {
    let (view, inputs) = source(
        "macro_rules! leaf{()=>{pub fn made()->i64{509_i64}}}#[macro_use]mod exported{unknown!();}leaf!();fn main()->i64{509_i64}",
    );
    let mut work = discover(&view);
    let mut count = 0;
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            SourceDiscoveryStep::NeedExpansion { request } => {
                count += 1;
                assert!(count <= 1);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { .. } => {
                panic!("unknown module producer fabricated a generated expansion")
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                assert_eq!(count, 1);
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                assert!(!blocked.is_empty());
                assert!(blocked.windows(2).all(|pair| pair[0].0 <= pair[1].0));
                break;
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => {
                panic!("unknown macro-use module producer sealed parent")
            }
        }
    }
    assert!(work.lookup("made").is_err());
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
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
fn blocked_nested_cycle_keeps_a_seal_unavailable_after_parent_sibling_progress() {
    let (v, inputs) = source(
        "macro_rules! provide{()=>{pub fn visible()->i64{281_i64}}}macro_rules! seed{()=>{use crate::right as left;use crate::left as right;left!();}}macro_rules! outer{()=>{seed!();provide!();}}outer!();fn main()->i64{visible()}",
    );
    let mut work = discover(&v);
    let mut count = 0;
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            SourceDiscoveryStep::NeedExpansion { request } => {
                count += 1;
                assert!(count <= 3);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                count += 1;
                assert!(count <= 3);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                assert_eq!(count, 3);
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                assert!(!blocked.is_empty());
                break;
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => panic!("blocked nested cycle sealed"),
        }
    }
    assert!(work.lookup("visible").is_err());
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn generated_ready_scheduling_refuses_import_and_textual_origin_ambiguity() {
    let (v, inputs) = source(SCHEDULING_SHADOW);
    let mut work = discover(&v);
    let mut count = 0;
    let error = loop {
        match work.poll() {
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                count += 1;
                assert!(count <= 3);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            Ok(SourceDiscoveryStep::NeedGeneratedExpansion { request }) => {
                count += 1;
                assert!(count <= 3);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            Err(error) => break error,
            Ok(_) => panic!("import/textual collision did not refuse selection"),
        }
    };
    assert_eq!(count, 3);
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert!(work.lookup("made").is_err());
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            work.key().clone()
        ))
        .is_none()
    );
}

#[test]
fn generated_textual_macro_precedes_a_descendant_import_without_false_ambiguity() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! later{()=>{pub fn wrong()->i64{251_i64}}}macro_rules! insert{()=>{use crate::later as picked;}}macro_rules! provide{()=>{macro_rules! picked{()=>{pub fn made()->i64{257_i64}}}insert!();}}macro_rules! outer{()=>{provide!();picked!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="257"));
}

#[test]
fn original_call_revalidation_refuses_generated_textual_and_original_import_ambiguity() {
    let (v, inputs) = source(ORIGINAL_IMPORT_SHADOW);
    let mut work = discover(&v);
    let mut count = 0;
    let error = loop {
        match work.poll() {
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                count += 1;
                assert!(count <= 2);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            Err(error) => break error,
            Ok(_) => panic!("original import/textual collision did not refuse"),
        }
    };
    assert_eq!(count, 2);
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn generated_fragment_ready_producer_preserves_source_order_and_blocks_a_true_cycle() {
    let (v, inputs) = source(
        "macro_rules! provide{()=>{pub fn visible()->i64{239_i64}}}macro_rules! outer{()=>{use crate::right as left;use crate::left as right;left!();provide!();}}outer!();fn main()->i64{visible()}",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer missing")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("ready producer was hidden behind the cycle")
    };
    let SourceDiscoveryStep::NeedGeneratedExpansion { request: again } = work.poll().unwrap()
    else {
        panic!("pending producer lost")
    };
    assert!(std::sync::Arc::ptr_eq(&request, &again));
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("cycle sealed after independent producer")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.lookup("visible").is_err());
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
}

#[test]
fn nested_generated_bare_relay_revalidates_retained_parent_call_positions() {
    let (report, _) = run_original(
        "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{227_i64}}}}}macro_rules! middle{()=>{provide!();}}#[macro_export]macro_rules! relay{()=>{middle!();}}seed!();crate::relay!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="227"));
}

#[test]
fn generated_bare_relay_refuses_an_unrelated_completed_shadow_before_sealing() {
    refuse_generated_shadow(GENERATED_SHADOW, 4);
}

#[test]
fn nested_generated_bare_relay_revalidates_completed_descendant_selection() {
    let text = GENERATED_SHADOW.replace(
        "#[macro_export]macro_rules! relay{()=>{provide!();}}",
        "macro_rules! middle{()=>{provide!();}}#[macro_export]macro_rules! relay{()=>{middle!();}}",
    );
    refuse_generated_shadow(&text, 5);
}

fn refuse_generated_shadow(text: &str, expected_expansions: usize) {
    let (v, inputs) = source(text);
    let mut work = discover(&v);
    let mut count = 0;
    let error = loop {
        match work.poll() {
            Ok(
                SourceDiscoveryStep::NeedDeriveAppend { .. }
                | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
                | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
                | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
                | SourceDiscoveryStep::NeedDeriveDeclaration { .. },
            ) => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            Ok(SourceDiscoveryStep::NeedExpansion { request }) => {
                count += 1;
                assert!(count <= expected_expansions);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            Ok(SourceDiscoveryStep::NeedGeneratedExpansion { request }) => {
                count += 1;
                assert!(count <= expected_expansions);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            Err(error) => break error,
            Ok(SourceDiscoveryStep::Incomplete { .. }) => {
                panic!("generated independent relay stayed blocked")
            }
            Ok(SourceDiscoveryStep::NeedProceduralSelection { .. })
            | Ok(SourceDiscoveryStep::NeedAttributeSelection { .. }) => {
                panic!("unexpected procedural selection in this fixture")
            }
            Ok(SourceDiscoveryStep::Sealed(_)) => panic!("changed generated selection sealed"),
        }
    };
    assert_eq!(count, expected_expansions);
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert!(work.sealed_expansions().is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert!(work.publish(&mut db).is_err());
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            work.key().clone()
        ))
        .is_none()
    );
}

#[test]
fn independent_qualified_producer_ignores_a_same_named_textual_shadow() {
    let (report, _) = run_original(
        "macro_rules! seed{()=>{use crate::later as picked;picked!();}}#[macro_export]macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{181_i64}}}}}macro_rules! provide{()=>{pub fn wrong()->i64{191_i64}}}seed!();crate::provide!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="181"));
}

#[test]
fn independent_import_retry_matches_macos_native_rust() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("artifacts/macos-first/m013-import-retry-oracles");
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
    let parked_nested = PARKED_IMPORT_SHADOW.replace(
        "macro_rules! outer{()=>{seed!();provide!();}}",
        "macro_rules! middle{()=>{seed!();}}macro_rules! outer{()=>{middle!();provide!();}}",
    );
    for (name, text, value) in [
        (
            "macro_use_original",
            "#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{457_i64}}}}leaf!();",
            "457",
        ),
        (
            "macro_use_generated",
            "macro_rules! outer{()=>{#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{461_i64}}}}leaf!();}}outer!();",
            "461",
        ),
        (
            "macro_use_outward",
            "macro_rules! outer{()=>{#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{467_i64}}}}}}outer!();leaf!();",
            "467",
        ),
        (
            "macro_use_recursive",
            "macro_rules! outer{()=>{macro_rules! spawn{()=>{#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn made()->i64{479_i64}}}}}}spawn!();leaf!();}}outer!();",
            "479",
        ),
        (
            "macro_use_shadow",
            "macro_rules! leaf{()=>{pub fn before()->i64{487_i64}}}leaf!();#[macro_use]mod exported{macro_rules! leaf{()=>{pub fn after()->i64{491_i64}}}}leaf!();fn made()->i64{before()+after()}",
            "978",
        ),
        (
            "macro_use_nested",
            "macro_rules! outer{()=>{#[macro_use]mod first{#[macro_use]mod second{macro_rules! leaf{()=>{pub fn made()->i64{503_i64}}}}}leaf!();}}outer!();",
            "503",
        ),
        (
            "module_local",
            "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn inner()->i64{367_i64}}}leaf!();}}}outer!();fn made()->i64{emitted::inner()}",
            "367",
        ),
        (
            "module_inherited",
            "macro_rules! leaf{()=>{pub fn inner()->i64{373_i64}}}macro_rules! outer{()=>{pub mod emitted{leaf!();}}}outer!();fn made()->i64{emitted::inner()}",
            "373",
        ),
        (
            "module_siblings",
            "macro_rules! outer{()=>{pub mod left{macro_rules! leaf{()=>{pub fn inner()->i64{379_i64}}}leaf!();}pub mod right{macro_rules! leaf{()=>{pub fn inner()->i64{383_i64}}}leaf!();}}}outer!();fn made()->i64{left::inner()+right::inner()}",
            "762",
        ),
        (
            "module_recursive",
            "macro_rules! outer{()=>{pub mod emitted{macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn inner()->i64{401_i64}}}leaf!();}}seed!();}}}outer!();fn made()->i64{emitted::inner()}",
            "401",
        ),
        (
            "module_import",
            "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn inner()->i64{409_i64}}}use leaf as picked;picked!();}}}outer!();fn made()->i64{emitted::inner()}",
            "409",
        ),
        (
            "module_shadow",
            "macro_rules! outer{()=>{macro_rules! leaf{()=>{pub fn root()->i64{433_i64}}}pub mod emitted{macro_rules! leaf{()=>{pub fn inner()->i64{439_i64}}}leaf!();}}}outer!();leaf!();fn made()->i64{emitted::inner()+root()}",
            "872",
        ),
        (
            "module_reexport",
            "macro_rules! outer{()=>{pub mod emitted{macro_rules! leaf{()=>{pub fn made()->i64{449_i64}}}pub(crate) use leaf as renamed;}}}outer!();use emitted::renamed as picked;picked!();",
            "449",
        ),
        (
            "inline_module_alias",
            "macro_rules! outer{()=>{pub mod emitted{pub mod nested{pub fn value()->i64{317_i64}pub fn made()->i64{value()}}}}}outer!();use emitted as chosen;fn made()->i64{chosen::nested::made()}",
            "317",
        ),
        (
            "inline_module_siblings",
            "macro_rules! outer{()=>{pub mod left{pub fn value()->i64{347_i64}}pub mod right{pub fn value()->i64{349_i64}}}}outer!();fn made()->i64{left::value()+right::value()}",
            "696",
        ),
        (
            "inline_modules",
            "macro_rules! outer{()=>{pub mod emitted{pub mod nested{pub fn value()->i64{317_i64}pub fn made()->i64{value()}}}}}outer!();fn made()->i64{emitted::nested::made()}",
            "317",
        ),
        ("inline_module_import", INLINE_MODULE_IMPORT, "331"),
        (
            "inline_module_cfg",
            "macro_rules! outer{()=>{#[cfg(any())]pub mod absent{missing!();}pub mod emitted{#[cfg(any())]pub fn disabled(){let _:Missing=missing();}pub fn value()->i64{337_i64}}}}outer!();fn made()->i64{emitted::value()}",
            "337",
        ),
        (
            "parked_import_later_text",
            PARKED_IMPORT_SHADOW
                .strip_suffix("fn main()->i64{made()}")
                .unwrap(),
            "307",
        ),
        (
            "parked_grandchild_later_text",
            parked_nested
                .strip_suffix("fn main()->i64{made()}")
                .unwrap(),
            "307",
        ),
        (
            "parked_descendants",
            "macro_rules! seed_a{()=>{use crate::later_a as picked_a;picked_a!();}}macro_rules! seed_b{()=>{use crate::later_b as picked_b;picked_b!();}}macro_rules! seeds{()=>{seed_a!();seed_b!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later_a{()=>{pub fn a()->i64{283_i64}}}#[macro_export]macro_rules! later_b{()=>{pub fn b()->i64{293_i64}}}}}macro_rules! outer{()=>{seeds!();provide!();}}outer!();fn made()->i64{a()+b()}",
            "576",
        ),
        (
            "parked_child",
            "macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{277_i64}}}}}macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! outer{()=>{seed!();provide!();}}outer!();",
            "277",
        ),
        (
            "descendant_import",
            "#[macro_export]macro_rules! later{()=>{pub fn wrong()->i64{251_i64}}}macro_rules! insert{()=>{use crate::later as picked;}}macro_rules! provide{()=>{macro_rules! picked{()=>{pub fn made()->i64{257_i64}}}insert!();}}macro_rules! outer{()=>{provide!();picked!();}}outer!();",
            "257",
        ),
        (
            "same_fragment",
            "macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{233_i64}}}}}macro_rules! outer{()=>{use crate::later as picked;picked!();provide!();}}outer!();",
            "233",
        ),
        (
            "nested_generated_bare",
            "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{227_i64}}}}}macro_rules! middle{()=>{provide!();}}#[macro_export]macro_rules! relay{()=>{middle!();}}seed!();crate::relay!();",
            "227",
        ),
        (
            "generated_bare",
            "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{211_i64}}}}}#[macro_export]macro_rules! relay{()=>{provide!();}}seed!();crate::relay!();",
            "211",
        ),
        (
            "bare",
            "macro_rules! seed{()=>{use crate::later as picked;picked!();}}macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{163_i64}}}}}seed!();provide!();",
            "163",
        ),
        (
            "later",
            "macro_rules! seed{()=>{use crate::later as picked;picked!();}}#[macro_export]macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{163_i64}}}}}seed!();crate::provide!();",
            "163",
        ),
        (
            "earlier",
            "macro_rules! seed{()=>{use crate::later as picked;picked!();}}#[macro_export]macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{167_i64}}}}}crate::provide!();seed!();",
            "167",
        ),
        (
            "generated",
            "macro_rules! seed{()=>{use crate::later as picked;picked!();}}#[macro_export]macro_rules! provide{()=>{#[macro_export]macro_rules! later{()=>{pub fn made()->i64{179_i64}}}}}#[macro_export]macro_rules! relay{()=>{crate::provide!();}}seed!();crate::relay!();",
            "179",
        ),
    ] {
        let case = work.join(name);
        fs::create_dir(&case).unwrap();
        fs::write(
            case.join("main.rs"),
            format!("{text}fn main(){{println!(\"{{}}\",made());}}\n"),
        )
        .unwrap();
        let compile = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "-A",
                "macro_expanded_macro_exports_accessed_by_absolute_paths",
            ])
            .arg(case.join("main.rs"))
            .arg("-o")
            .arg(case.join("native"))
            .output()
            .unwrap();
        fs::write(case.join("compile.stdout"), &compile.stdout).unwrap();
        fs::write(case.join("compile.stderr"), &compile.stderr).unwrap();
        assert!(
            compile.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let native = Command::new(case.join("native")).output().unwrap();
        fs::write(case.join("native.stdout"), &native.stdout).unwrap();
        fs::write(case.join("native.stderr"), &native.stderr).unwrap();
        assert!(native.status.success());
        assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), value);
        let (report, _) = run_original(&format!("{text}fn main()->i64{{made()}}"));
        let lrr::engine::run::ValueSummary::Signed(actual) = report.value else {
            panic!("VM result is not a signed integer")
        };
        assert_eq!(actual, value);
        fs::write(case.join("vm-result.txt"), actual).unwrap();
    }
}

#[test]
fn generated_macro_import_cycle_retains_sorted_blocked_demands_and_no_seal() {
    let (v, inputs) = source(
        "macro_rules! seed{()=>{use crate::right as left;left!();}}#[macro_export]macro_rules! provide{()=>{use crate::left as right;right!();}}seed!();crate::provide!();fn main()->i64{173_i64}",
    );
    let mut work = discover(&v);
    let mut expanded = 0;
    let blocked = loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            SourceDiscoveryStep::NeedExpansion { request } => {
                expanded += 1;
                assert!(expanded <= 2);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { .. } => {
                panic!("origin-free cycle selected a generated macro")
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => panic!("unexpanded cycle sealed"),
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                break blocked;
            }
        }
    };
    assert_eq!(expanded, 2);
    assert!(!blocked.is_empty());
    assert!(blocked.windows(2).all(|w| w[0].0 < w[1].0));
    for (_, waiting) in &blocked {
        assert!(!waiting.is_empty());
        assert!(waiting.windows(2).all(|w| w[0] < w[1]));
    }
    let SourceDiscoveryStep::Incomplete { blocked: again, .. } = work.poll().unwrap() else {
        panic!("cycle changed without new inputs")
    };
    assert_eq!(blocked, again);
    assert!(work.sealed_expansions().is_err());
    assert!(work.lookup("made").is_err());
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    assert_eq!(
        work.publish(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    assert!(
        db.input(&lrr::db::dependency::InputKey::DiscoverySeal(
            work.key().clone()
        ))
        .is_none()
    );
}

#[test]
fn provisional_bare_producer_does_not_turn_an_origin_free_cycle_into_a_seal() {
    let (v, _) = source(
        "macro_rules! seed{()=>{use crate::right as left;left!();}}macro_rules! provide{()=>{use crate::left as right;right!();}}seed!();provide!();fn main()->i64{199_i64}",
    );
    let mut work = discover(&v);
    let mut count = 0;
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("unexpected derive helper demand in declarative fixture")
            }
            SourceDiscoveryStep::NeedExpansion { request } => {
                count += 1;
                assert!(count <= 2);
                work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                assert_eq!(count, 2);
                assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                assert!(!blocked.is_empty());
                assert!(work.sealed_expansions().is_err());
                break;
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { .. } => {
                panic!("cycle selected a generated definition")
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => panic!("cycle sealed"),
        }
    }
}
