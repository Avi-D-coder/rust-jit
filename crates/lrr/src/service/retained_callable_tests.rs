//! Ordinary module calls select the genuine original declaration after its
//! retained body publication. No macro-name runtime dispatch is authorized here.
use super::retained_body_tests::prepare_expanded_source;
use crate::{
    core::digest::StableEncode,
    db::{memo::Database, poll::QueryPoll, retained_syntax::RetainedSyntaxInput},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    frontend::{
        callables::{CallResolver, CallSelection},
        names::NameNamespaces,
    },
    source::{changes::PreparedSourceView, manifest::BodyKey},
};
use std::sync::Arc;
fn completed(poll: RunPoll) -> crate::engine::run::RunReport {
    match poll {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("ordinary retained execution fault: {error:?}"),
        RunPoll::NeedInput(input) => panic!("ordinary retained execution suspended: {input:?}"),
        RunPoll::Panicked(report) => panic!("ordinary retained execution panicked: {report:?}"),
    }
}

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("ordinary retained call failed: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("ordinary retained call suspended: {input:?}"),
    }
}
fn source(db: &Database, input: &RetainedSyntaxInput) -> PreparedSourceView {
    db.prepared_source(input.prepared().source())
        .unwrap()
        .unwrap()
}
fn body(source: &PreparedSourceView, name: &str) -> BodyKey {
    source
        .outline()
        .items
        .iter()
        .find(|item| {
            item.def_key.declared_name == name
                && item.def_key.item_kind == crate::source::manifest::ItemKind::Function
        })
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key()
}
fn select(
    db: &mut Database,
    source: &PreparedSourceView,
    caller: &BodyKey,
    name: &str,
) -> Box<crate::frontend::callables::SourceCallable> {
    let graph = NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let sources = [source];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let CallSelection::Source(callee) =
        ready(resolver.source_function(db, caller.definition, name))
    else {
        panic!("ordinary source function was not selected")
    };
    callee
}

const CALL: &str = "macro_rules! compute { () => { 2_i32 + 5_i32 } } fn value()->i32 { compute!() } fn main() { value(); }";

#[test]
fn ordinary_calls_enter_retained_callee_from_original_and_retained_callers_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for retain_caller in [false, true] {
            let mut db = Database::default();
            let (input, discovery) = prepare_expanded_source(&mut db, CALL, edition, 1, true);
            let source = source(&db, &input);
            let original_main = body(&source, "main");
            let original_value = body(&source, "value");
            ready(db.publish_retained_syntax(input.clone()));
            let callee = ready(db.publish_retained_body(input.clone(), original_value.definition));
            let main = if retain_caller {
                ready(db.publish_retained_body(input, original_main.definition))
            } else {
                original_main
            };
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, main, 100_000, discovery,
            ));
            let report = completed(run.poll(&mut db));
            assert!(matches!(report.value, ValueSummary::Unit));
            assert_eq!(report.opcode_counters.started_by_opcode[10], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(run.peak_guest_frames(), 2);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert_ne!(callee, original_value);
        }
    }
}

#[test]
fn unchanged_upstream_contextual_macro_name_calls_the_original_retained_function() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/auto-traits/auto-is-contextual.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let mut db = Database::default();
        let (input, discovery) = prepare_expanded_source(&mut db, ORIGINAL, edition, 1, true);
        let source = source(&db, &input);
        let main = body(&source, "main");
        let auto = body(&source, "auto");
        ready(db.publish_retained_syntax(input.clone()));
        ready(db.publish_retained_body(input.clone(), auto.definition));
        // The whole original program includes an item-producing macro. Its
        // ordinary Type namespace must inspect the expanded source too; leaving
        // main on the original tree correctly refuses a cast in an unresolved
        // macro-containing scope. The preceding control separately covers
        // original callers whose namespace needs no such expansion.
        let main = ready(db.publish_retained_body(input, main.definition));
        let mut run = ready(RunMachine::start_with_discovery(
            &mut db, main, 100_000, discovery,
        ));
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[12], 1);
        assert_eq!(run.peak_guest_frames(), 2);
    }
    // Whole original source execution, not official compiletest/CLI acceptance.
}

#[test]
fn retained_syntax_alone_cannot_retarget_a_selected_original_callable() {
    let mut db = Database::default();
    let (input, _) = prepare_expanded_source(&mut db, CALL, 2024, 1, true);
    let source = source(&db, &input);
    let main = body(&source, "main");
    let original = body(&source, "value");
    ready(db.publish_retained_syntax(input.clone()));
    let before = select(&mut db, &source, &main, "value");
    assert_eq!(before.body().unwrap().key(), original);
    let current = ready(db.publish_retained_body(input, original.definition));
    assert!(before.revalidate(&mut db).is_err());
    let after = select(&mut db, &source, &main, "value");
    assert_eq!(after.identity(), before.identity());
    assert_eq!(after.definition().fingerprint(), original.definition);
    assert_eq!(after.body().unwrap().key(), current);
    after.revalidate(&mut db).unwrap();
}

#[test]
fn retained_callable_revalidation_refuses_an_old_expansion_epoch_before_entry() {
    let mut db = Database::default();
    let (input, _) = prepare_expanded_source(&mut db, CALL, 2024, 1, true);
    let source = source(&db, &input);
    let main = body(&source, "main");
    let original = body(&source, "value");
    ready(db.publish_retained_syntax(Arc::clone(&input)));
    ready(db.publish_retained_body(input, original.definition));
    let callee = select(&mut db, &source, &main, "value");
    callee.revalidate(&mut db).unwrap();
    let source_revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), source_revision);
    assert_eq!(
        callee.revalidate(&mut db).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    db.require_body_unowned(original.definition).unwrap();
}
