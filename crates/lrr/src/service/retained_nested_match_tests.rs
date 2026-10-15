//! Common intrinsic/control semantics across actual retained source origins.
//! Supplementary route controls; unchanged rustc expr-match.rs is the language bank.
use super::retained_body_tests::prepare_expanded_source;
use crate::{
    core::digest::StableEncode,
    db::{key::QueryKey, memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    syntax::body_index::{ExpressionKind, NodeRole},
};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("common match preparation failed: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("common match preparation suspended: {input:?}"),
    }
}
#[test]
fn retained_intrinsic_nested_match_uses_the_ordinary_route_across_macro_origins() {
    for edition in [2015, 2018, 2021, 2024] {
        for control in [
            "match true {true=>1_i32,false=>cold()}",
            "if true {1_i32} else {cold()}",
            "{ let unused = 0_i32; match true {true=>1_i32,false=>cold()} }",
        ] {
            for text in [
                format!(
                    "fn cold()->i32{{2_i32}} fn value()->i32{{match ({control}) {{1_i32=>7_i32,_=>8_i32}}}} fn main(){{}}"
                ),
                format!(
                    "macro_rules! value_source {{()=>{{{control}}}}} fn cold()->i32{{2_i32}} fn value()->i32{{match value_source!() {{1_i32=>7_i32,_=>8_i32}}}} fn main(){{}}"
                ),
                format!(
                    "macro_rules! value_source {{()=>{{{control}}}}} macro_rules! wrapper {{()=>{{value_source!()}}}} fn cold()->i32{{2_i32}} fn value()->i32{{match wrapper!() {{1_i32=>7_i32,_=>8_i32}}}} fn main(){{}}"
                ),
            ] {
                let mut db = Database::default();
                db.trace.detailed = true;
                let (input, discovery) = prepare_expanded_source(&mut db, &text, edition, 1, true);
                let source = db
                    .prepared_source(input.prepared().source())
                    .unwrap()
                    .unwrap();
                let target = source
                    .outline()
                    .items
                    .iter()
                    .find(|item| item.def_key.declared_name == "value")
                    .unwrap()
                    .def_key
                    .fingerprint();
                ready(db.publish_retained_syntax(input.clone()));
                let body = ready(db.publish_retained_body(input, target));
                let outcome = ready(db.fetch(
                    &crate::db::provider::SyntaxProvider,
                    QueryKey::BodyStructure(body.clone()),
                ));
                let crate::db::value::QueryValue::BodyStructure(index) = outcome.value.as_ref()
                else {
                    panic!("ordinary body absent")
                };
                assert_eq!(
                    index
                        .nodes()
                        .iter()
                        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Match))
                        .count(),
                    if control.starts_with("if ") { 1 } else { 2 }
                );
                let cold: std::collections::BTreeSet<_> = index
                    .nodes()
                    .iter()
                    .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
                    .map(|node| node.id.0)
                    .collect();
                assert_eq!(cold.len(), 1);
                let trace_start = db.trace.events.len();
                let mut run = ready(RunMachine::start_with_discovery(
                    &mut db,
                    body.clone(),
                    100_000,
                    discovery,
                ));
                let RunPoll::Completed(report) = run.poll(&mut db) else {
                    panic!("ordinary retained nested match failed in edition {edition}: {text}")
                };
                assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "7"));
                assert_eq!(
                    report.opcode_counters.started_by_opcode[26], 0,
                    "no assertion executor"
                );
                assert_eq!(
                    report.opcode_counters.started_by_opcode[22], 0,
                    "no cold source call"
                );
                assert_eq!(run.peak_guest_frames(), 1);
                assert!(
                    db.trace.events[trace_start..]
                        .iter()
                        .all(|event| match &event.key {
                            QueryKey::BodyStructure(actual) => *actual == body,
                            QueryKey::Thunk(pc) if pc.body == body => !cold.contains(&pc.node),
                            _ => true,
                        })
                );
                assert_eq!(
                    report.opcode_counters.operations_started,
                    report.opcode_counters.operations_completed
                );
                drop(run);
            }
        }
    }
}
