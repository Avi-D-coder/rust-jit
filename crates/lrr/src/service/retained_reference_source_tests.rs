//! LRR route/currentness controls. Stock main and standard-macro expansion are
//! separate acceptance requirements; these execute ordinary reference calls.
use super::retained_body_tests::prepare_expanded_source;
use crate::{
    core::digest::StableEncode,
    db::{key::QueryKey, memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    frontend::{
        call_contracts::{ArgumentHintPreparation, prepare_argument_hint},
        callables::{CallResolver, CallSelection},
        names::NameNamespaces,
    },
    source::{changes::PreparedSourceView, manifest::BodyKey},
};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("reference source preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("reference source suspended: {input:?}"),
    }
}
fn body(source: &PreparedSourceView, name: &str) -> BodyKey {
    source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key()
}

#[test]
fn retained_reference_calls_share_ordinary_source_entry_all_editions() {
    // Same primitive reference operation as regions-infer-call.rs, moved
    // across ordinary source, arbitrary user output and a macro wrapper.
    for edition in [2015, 2018, 2021, 2024] {
        for declarations in [
            "fn take(x:&i32)->i32{*x}",
            "macro_rules! emit{()=>{fn take(x:&i32)->i32{*x}}}emit!();",
            "macro_rules! emit{()=>{fn take(x:&i32)->i32{*x}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
        ] {
            let text = format!(
                "{declarations}fn main()->i32{{let value=22_i32;take(&value)}}fn cold<T:Missing>(){{unknown();}}"
            );
            let mut db = Database::default();
            db.trace.detailed = true;
            let (input, discovery) = prepare_expanded_source(&mut db, &text, edition, 1, true);
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let original_main = body(&source, "main");
            ready(db.publish_retained_syntax(input.clone()));
            // Only original members are reconciled here. Generated members
            // retain their genuine discovery-published GeneratedSyntax source.
            if let Some(take) = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "take")
            {
                ready(db.publish_retained_body(input.clone(), take.def_key.fingerprint()));
            }
            let main = ready(db.publish_retained_body(input, original_main.definition));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, main, 100_000, discovery,
            ));
            let report = match run.poll(&mut db) {
                RunPoll::Completed(report) => report,
                RunPoll::Fault(error) => panic!("ordinary reference call faulted: {error:?}"),
                RunPoll::NeedInput(input) => panic!("ordinary reference call suspended: {input:?}"),
                RunPoll::Panicked(report) => panic!("ordinary reference call panicked: {report:?}"),
            };
            assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "22"));
            assert_eq!(run.peak_guest_frames(), 2);
            assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::FunctionSignature(_)))
            );
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::BodyStructure(_)))
            );
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::Thunk(_)))
            );
        }
    }
}

#[test]
fn retained_reference_parameter_proof_refuses_changed_expansion_epoch() {
    let mut db = Database::default();
    let (input, _) = prepare_expanded_source(
        &mut db,
        "macro_rules! id{($x:expr)=>{$x}}fn take<'a>(x:&'a i32)->i32{id!(*x)}fn main(){}",
        2024,
        1,
        true,
    );
    let source = db
        .prepared_source(input.prepared().source())
        .unwrap()
        .unwrap();
    let original = body(&source, "take");
    let main = body(&source, "main");
    ready(db.publish_retained_syntax(input.clone()));
    let retained = ready(db.publish_retained_body(input, original.definition));
    assert_ne!(retained, original);
    let graph = NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let views = [&source];
    let resolver = CallResolver::new(&graph, &views).unwrap();
    let CallSelection::Source(callee) =
        ready(resolver.module_function(&mut db, main.definition, "take"))
    else {
        panic!()
    };
    let ArgumentHintPreparation::Ready(hints) =
        ready(prepare_argument_hint(&mut db, &callee, &graph, &views, 0))
    else {
        panic!("retained parameter lost its actual source authority")
    };
    let proof = hints[0].closed_reference.as_ref().unwrap();
    proof.require_current(&mut db, &callee).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert!(proof.require_current(&mut db, &callee).is_err());
}
