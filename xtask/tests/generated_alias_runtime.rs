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
            SourceDiscoveryStep::Incomplete { .. } => {
                panic!("fixture failed to complete discovery")
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
    db.trace.detailed = true;
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
fn generated_parameter_alias_uses_canonical_import_identity() {
    let (report, _) = run_original(
        "use Word as Arg;macro_rules! make{()=>{pub type Word=i64;pub fn made(x:Arg)->i64{x+1_i64}}}make!();fn main()->i64{made(7_i64)}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="8"));
}
#[test]
fn alias_chain_crosses_original_and_generated_declarations() {
    let (report, _) = run_original(
        "type Start=Middle;type End=i64;macro_rules! make{()=>{pub type Middle=End;pub fn made(x:Start)->i64{x}}}make!();fn main()->i64{made(7_i64)}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_alias_is_resolved_in_explicit_generic_arguments() {
    let (report, _) = run_original(
        "fn identity<T>(x:T)->T{x}macro_rules! make{()=>{pub type Word=i64;pub fn made()->i64{identity::<Word>(7_i64)}}}make!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn cold_generated_alias_cycle_is_not_normalized() {
    let (report, db) = run_original(
        "macro_rules! make{()=>{pub type A=B;pub type B=A;pub fn made(x:i64)->i64{x}}}make!();fn main()->i64{made(7_i64)}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
    assert!(
        db.trace
            .events
            .iter()
            .all(|e| !matches!(e.key, lrr::db::key::QueryKey::TypeAlias(_)))
    );
}
#[test]
fn reached_generated_alias_cycle_refuses_before_argument_body_entry() {
    let (v, inputs) = source(
        "macro_rules! make{()=>{pub type A=B;pub type B=A;pub fn made(x:A)->i64{1_i64}}}make!();fn side()->i64{7_i64}fn main()->i64{made(side())}",
    );
    let mut work = discover(&v);
    finish(&mut work);
    let key = |name: &str| {
        v.outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == name)
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key()
    };
    let mut db = lrr::db::memo::Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let lrr::db::poll::QueryPoll::Ready(Ok(mut run)) =
        lrr::engine::run::RunMachine::start_with_discovery(&mut db, key("main"), 10_000, work)
    else {
        panic!("entry absent")
    };
    let lrr::engine::run::RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("alias cycle became success")
    };
    assert_eq!(error.code, "LRR-E-TYPE-ALIAS-CYCLE");
    assert!(
        db.memo(&lrr::db::key::QueryKey::BodyStructure(key("side")))
            .is_none()
    );
    assert!(error.message.contains(" -> "));
}
