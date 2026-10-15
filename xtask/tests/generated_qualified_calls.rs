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
fn generated_qualified_call_invokes_captured_original_export() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}macro_rules! outer{()=>{crate::seed!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_qualified_call_uses_its_generated_export_body() {
    let (report, _) = run_original(
        "macro_rules! outer{()=>{#[macro_export]macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}crate::emitted!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_call_uses_a_generated_import_alias_within_the_same_fragment() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}macro_rules! outer{()=>{use crate::seed as selected;selected!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn child_generated_qualified_call_preserves_nested_request_context() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}macro_rules! middle{()=>{crate::seed!();}}macro_rules! outer{()=>{middle!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_dollar_crate_call_uses_explicit_definition_site_authority() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}macro_rules! outer{()=>{$crate::seed!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn unresolved_generated_qualified_call_never_publishes_a_seal() {
    let (v, _) = source("macro_rules! outer{()=>{crate::missing!();}}outer!();");
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = work.poll().unwrap()
    else {
        panic!("missing qualified macro became ready")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn generated_qualified_call_preserves_raw_keyword_spelling() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! r#gen{()=>{pub fn made()->i64{7_i64}}}macro_rules! outer{()=>{crate::r#gen!();}}outer!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_qualified_call_refuses_private_import_access() {
    let (v, _) = source(
        "mod hidden{macro_rules! seed{()=>{}}use seed as selected;}macro_rules! outer{()=>{hidden::selected!();}}outer!();",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let error = match work.poll() {
        Err(e) => e,
        Ok(_) => panic!("private macro route became executable"),
    };
    assert_eq!(error.code, "LRR-E-MACRO-INACCESSIBLE");
    assert!(work.sealed_expansions().is_err());
}

#[test]
fn generated_qualified_request_preserves_canonical_generated_header_identity() {
    let (v, _) = source(
        "macro_rules! outer{()=>{#[macro_export]macro_rules! emitted{()=>{pub fn made()->i64{7_i64}}}crate::emitted!();}}outer!();",
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
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("qualified generated target absent")
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
fn generated_qualified_call_reports_import_ambiguity() {
    let (v, _) = source(
        "#[macro_export]macro_rules! a{()=>{}}#[macro_export]macro_rules! b{()=>{}}mod names{pub use crate::a as selected;pub use crate::b as selected;}macro_rules! outer{()=>{names::selected!();}}outer!();",
    );
    let mut work = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("outer absent")
    };
    work.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let error = match work.poll() {
        Err(e) => e,
        Ok(_) => panic!("ambiguous generated macro became executable"),
    };
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
    assert!(work.sealed_expansions().is_err());
}
#[test]
fn generated_super_call_uses_its_captured_module_scope() {
    let (report, _) = run_original(
        "#[macro_export]macro_rules! seed{()=>{pub fn made()->i64{7_i64}}}mod area{macro_rules! outer{()=>{super::seed!();}}outer!();}fn main()->i64{area::made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
