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
    let lrr::db::poll::QueryPoll::Ready(Ok(mut run)) =
        lrr::engine::run::RunMachine::start_with_discovery(&mut db, body, 10_000, work)
    else {
        panic!("original caller entry not ready")
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
fn child_generated_macro_import_enables_later_original_invocation() {
    let (report, _) = run_original(
        "macro_rules! middle{()=>{macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}use emitted as selected;}}macro_rules! outer{()=>{middle!();}}outer!();selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn child_generated_export_keeps_its_qualified_route() {
    let (report, _) = run_original(
        "macro_rules! middle{()=>{#[macro_export]macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}}}macro_rules! outer{()=>{middle!();}}outer!();crate::emitted!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn child_generated_import_of_original_export_uses_captured_owner() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}macro_rules! middle{()=>{use crate::seed as selected;}}macro_rules! outer{()=>{middle!();}}outer!();selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn multiple_child_levels_preserve_generated_import_routes() {
    let (report, _) = run_original(
        "macro_rules! inner{()=>{macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}use emitted as selected;}}macro_rules! middle{()=>{inner!();}}macro_rules! outer{()=>{middle!();}}outer!();selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn unresolved_child_import_cannot_publish_a_partial_seal() {
    let (v, _) = source(
        "macro_rules! middle{()=>{use crate::missing as selected;}}macro_rules! outer{()=>{middle!();}}outer!();selected!();",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("middle absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("unknown child import was certified")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn child_import_uses_the_parent_macro_visible_at_its_actual_call_position() {
    let (report, _) = run_original(
        "macro_rules! middle{()=>{use parent_seed as selected;}}macro_rules! outer{()=>{macro_rules! parent_seed{()=>{pub fn made()->i64{7_i64}}}middle!();}}outer!();selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn child_import_cannot_capture_a_later_parent_textual_macro() {
    let (v, _) = source(
        "macro_rules! middle{()=>{use parent_seed as selected;}}macro_rules! outer{()=>{middle!();macro_rules! parent_seed{()=>{pub fn made()->i64{7_i64}}}}}outer!();selected!();",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("middle absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("later parent textual macro leaked into child import")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn child_import_request_preserves_the_child_header_canonical_identity() {
    let (v, _) = source(
        "macro_rules! middle{()=>{macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}use emitted as selected;}}macro_rules! outer{()=>{middle!();}}outer!();selected!();",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("middle absent")
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
        panic!("later import target absent")
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
    assert_eq!(work.sealed_expansions().unwrap().len(), 3);
}
