//! Common lexical-route and source-context controls. Imported originals stay
//! unchanged; a component execution does not establish compiler acceptance.
use super::retained_body_tests::{prepare_expanded_body_source, prepare_expanded_source};
use crate::{
    core::digest::StableEncode,
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    engine::run::{RunMachine, RunPoll, ValueSummary},
    ir::{op::Op, pc::Phase},
    syntax::body_index::{ExpressionKind, NodeRole},
};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("ordinary lexical preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("ordinary lexical preparation suspended: {input:?}"),
    }
}
fn completed(poll: RunPoll) -> crate::engine::run::RunReport {
    match poll {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("ordinary lexical execution: {error:?}"),
        RunPoll::NeedInput(input) => panic!("ordinary lexical execution suspended: {input:?}"),
        RunPoll::Panicked(report) => panic!("ordinary lexical execution panicked: {report:?}"),
    }
}

#[test]
fn unchanged_statement_metavariable_main_executes_its_generated_binding_all_editions() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-stmt-2.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let mut db = Database::default();
        let (input, discovery) =
            prepare_expanded_body_source(&mut db, ORIGINAL, edition, 1, "main");
        let source = db
            .prepared_source(input.prepared().source())
            .unwrap()
            .unwrap();
        let definition = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .def_key
            .fingerprint();
        ready(db.publish_retained_syntax(input.clone()));
        let body = ready(db.publish_retained_body(input, definition));
        let mut run = ready(RunMachine::start_with_discovery(
            &mut db, body, 100_000, discovery,
        ));
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[7], 1); // ordinary BindLocal
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
    }
    // Upstream asks check-pass; execution of its main is component runtime
    // evidence only, with compile-time acceptance still separately outstanding.
}

#[test]
fn ordinary_local_lookup_preserves_caller_and_generated_binding_contexts() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "fn value()->i32{let x=7_i32;{let x=9_i32;}x} fn main(){}",
            "macro_rules! shadow {()=>{let x=9_i32;}} fn value()->i32{let x=7_i32;shadow!();x} fn main(){}",
            "macro_rules! shadow {()=>{let x=9_i32;}} macro_rules! wrapper {()=>{shadow!();}} fn value()->i32{let x=7_i32;wrapper!();x} fn main(){}",
            "macro_rules! same {()=>{{let x=7_i32;x}}} fn value()->i32{let x=9_i32;same!()} fn main(){}",
            "macro_rules! forwarded {($name:ident)=>{{let x=9_i32;$name}}} fn value()->i32{let x=7_i32;forwarded!(x)} fn main(){}",
        ] {
            let mut db = Database::default();
            let (input, discovery) =
                prepare_expanded_body_source(&mut db, text, edition, 1, "value");
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let definition = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "value")
                .unwrap()
                .def_key
                .fingerprint();
            ready(db.publish_retained_syntax(input.clone()));
            let body = ready(db.publish_retained_body(input, definition));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, body, 100_000, discovery,
            ));
            let report = completed(run.poll(&mut db));
            assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "7"));
            assert_eq!(report.opcode_counters.started_by_opcode[19], 0);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(run.peak_guest_frames(), 1);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
        }
    }
}

#[test]
fn ordinary_parameter_lookup_drives_retained_match_without_a_generated_source_exclusion() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "fn value(x:bool)->i32{match x{true=>7_i32,false=>9_i32}} fn entry()->i32{value(true)} fn main(){}",
            "macro_rules! forwarded {($name:ident)=>{$name}} fn value(x:bool)->i32{match forwarded!(x){true=>7_i32,false=>9_i32}} fn entry()->i32{value(true)} fn main(){}",
            "macro_rules! forwarded {($name:ident)=>{$name}} macro_rules! wrapper {($name:ident)=>{forwarded!($name)}} fn value(x:bool)->i32{match wrapper!(x){true=>7_i32,false=>9_i32}} fn entry()->i32{value(true)} fn main(){}",
        ] {
            let mut db = Database::default();
            let (input, discovery) = prepare_expanded_source(&mut db, text, edition, 1, true);
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let definition = |name: &str| {
                source
                    .outline()
                    .items
                    .iter()
                    .find(|item| item.def_key.declared_name == name)
                    .unwrap()
                    .def_key
                    .fingerprint()
            };
            ready(db.publish_retained_syntax(input.clone()));
            ready(db.publish_retained_body(input.clone(), definition("value")));
            let body = ready(db.publish_retained_body(input, definition("entry")));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, body, 100_000, discovery,
            ));
            let report = completed(run.poll(&mut db));
            assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "7"));
            // The scrutinee is a place expression: the ordinary parameter
            // lookup addresses the original argument cell (ParameterPlace)
            // and the shared matcher tests it in place, with no owning read.
            assert_eq!(report.opcode_counters.started_by_opcode[44], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[21], 0);
            assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(run.peak_guest_frames(), 2);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
        }
    }
}

#[test]
fn missing_definition_site_scope_join_never_captures_a_same_spelled_caller() {
    // Native Rust returns 7. The cross-context definition-site lookup is still
    // missing; this authority control requires an explicit unresolved frontier,
    // rather than admitting either caller binding by spelling and returning 9.
    // Refusal is not a runtime-contract pass for this valid program.
    const SOURCE: &str =
        "fn value()->i32{let x=7_i32;macro_rules! read {()=>{x}} let x=9_i32;read!()} fn main(){}";
    for edition in [2015, 2018, 2021, 2024] {
        let mut db = Database::default();
        let (input, _) = prepare_expanded_body_source(&mut db, SOURCE, edition, 1, "value");
        let source = db
            .prepared_source(input.prepared().source())
            .unwrap()
            .unwrap();
        let definition = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "value")
            .unwrap()
            .def_key
            .fingerprint();
        ready(db.publish_retained_syntax(input.clone()));
        let body = ready(db.publish_retained_body(input.clone(), definition));
        let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body)));
        let QueryValue::BodyStructure(indexed) = indexed.value.as_ref() else {
            panic!("ordinary body index absent")
        };
        let mut selected = None;
        for node in indexed.nodes() {
            if node.role == NodeRole::Expression(ExpressionKind::Path) {
                assert!(selected.replace(node.id).is_none());
            }
        }
        let pc = indexed
            .pc(selected.expect("actual generated reference"), Phase::Entry)
            .unwrap();
        let recipe = ready(db.fetch(&SyntaxProvider, QueryKey::Thunk(pc)));
        let QueryValue::Thunk(recipe) = recipe.value.as_ref() else {
            panic!("ordinary path thunk absent")
        };
        assert!(
            recipe
                .recipe()
                .operations
                .iter()
                .any(|operation| matches!(operation, Op::Unsupported(_)))
        );
        assert!(
            !recipe.recipe().operations.iter().any(|operation| matches!(
                operation,
                Op::ReadLocal { .. } | Op::ReadParameter { .. }
            ))
        );
    }
}
