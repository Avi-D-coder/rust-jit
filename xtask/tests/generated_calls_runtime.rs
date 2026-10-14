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
fn original_caller_uses_generated_import_target_with_real_parameter_contracts() {
    let (report, _) = run_original(
        "use made as selected;macro_rules! make{()=>{pub fn made(x:i64)->i64{x+1_i64}}}make!();fn main()->i64{selected(7_i64)}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="8"));
    assert!(report.opcode_counters.operations_completed > 0);
}
#[test]
fn generated_callee_can_invoke_another_generated_function() {
    let (report, _) = run_original(
        "use made as selected;macro_rules! make{()=>{pub fn leaf()->i64{7_i64}pub fn made()->i64{leaf()}}}make!();fn main()->i64{selected()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn certified_missing_callee_refuses_before_argument_body_entry() {
    let (v, inputs) = source("fn side_effect()->i64{1_i64}fn main()->i64{missing(side_effect())}");
    let mut work = discover(&v);
    finish(&mut work);
    let main = v
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let side = v
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "side_effect")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut db = lrr::db::memo::Database::default();
    inputs.publish(&mut db).unwrap();
    let lrr::db::poll::QueryPoll::Ready(Ok(mut run)) =
        lrr::engine::run::RunMachine::start_with_discovery(&mut db, main, 10_000, work)
    else {
        panic!("entry absent")
    };
    let lrr::engine::run::RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("missing identity did not fail")
    };
    assert_eq!(error.code, "LRR-E-NAME-MISSING");
    assert!(
        db.memo(&lrr::db::key::QueryKey::BodyStructure(side))
            .is_none()
    );
}

#[test]
fn generated_caller_can_invoke_original_function() {
    let (report, _) = run_original(
        "fn base(x:i64)->i64{x+2_i64}macro_rules! make{()=>{pub fn made(x:i64)->i64{base(x)}}}make!();fn main()->i64{made(5_i64)}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_caller_preserves_explicit_generic_argument_context() {
    let (report, _) = run_original(
        "fn identity<T>(x:T)->T{x}macro_rules! make{()=>{pub fn made()->i64{identity::<i64>(7_i64)}}}make!();fn main()->i64{made()}",
    );
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
}
#[test]
fn generated_callee_return_mismatch_is_not_accepted_as_success() {
    let (v, inputs) =
        source("macro_rules! make{()=>{pub fn made()->i64{true}}}make!();fn main()->i64{made()}");
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
        panic!("caller entry absent")
    };
    let lrr::engine::run::RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("generated return mismatch became success")
    };
    assert_eq!(error.code, "LRR-E-CALL-RETURN-TYPE");
}
