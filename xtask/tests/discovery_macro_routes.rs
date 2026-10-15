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
            SourceDiscoveryStep::NeedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => w
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
fn qualified_exported_declarative_macro_executes_selected_owned_body() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! make{()=>{pub fn made()->i64{7_i64}}}crate::make!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn imported_declarative_macro_preserves_definition_identity() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! make{()=>{pub fn made()->i64{7_i64}}}use crate::make as selected;selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn originating_macro_resolves_through_cyclic_reexports() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! make{()=>{pub fn made()->i64{7_i64}}}mod a{pub use crate::make;pub use crate::b::*;}mod b{pub use crate::a::*;}b::make!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn later_macro_import_uses_actual_generated_textual_publication() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}}}outer!();use emitted as selected;selected!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn unresolved_qualified_macro_cycle_never_publishes_a_partial_seal() {
    let (v, _) = source(
        "mod a{pub use crate::b::missing;}mod b{pub use crate::a::missing;}b::missing!();fn main()->i64{7_i64}",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("unresolved macro cycle did not stay incomplete")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
    assert!(work.lookup("main").is_err());
}

#[test]
fn same_named_export_cannot_satisfy_an_unresolved_import_target() {
    let (v, _) = source(
        "#[macro_export]macro_rules! make{()=>{}}use crate::missing as make;crate::make!();fn main()->i64{7_i64}",
    );
    let mut work = discover(&v);
    match work.poll().unwrap() {
        SourceDiscoveryStep::NeedExpansion { request } => work
            .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap(),
        other => panic!(
            "exported macro not selected: {}",
            matches!(other, SourceDiscoveryStep::Incomplete { .. })
        ),
    }
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("wrong import target became resolved")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn module_owned_exported_macro_keeps_its_selected_source_body() {
    let (report, _) = run_original(
        "mod defs{#[macro_export]macro_rules! make{()=>{pub fn made()->i64{7_i64}}}}crate::make!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}

#[test]
fn textual_macro_import_cannot_hide_destination_ambiguity() {
    let (v, _) = source(
        "macro_rules! original{()=>{}}#[macro_export]macro_rules! same{()=>{}}#[macro_export]macro_rules! trigger{()=>{}}use original as same;crate::trigger!();fn main()->i64{7_i64}",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("trigger not selected")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("ambiguous import destination was certified")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}
