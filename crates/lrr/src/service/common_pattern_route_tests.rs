//! LRR route controls supplement unchanged upstream range originals. These
//! exercise genuine retained publication, ordinary PCs, cold arms and exact-once
//! calls; they are not an upstream fixture or full transaction acceptance suite.
use super::retained_body_tests::{prepare_expanded_body_source, prepare_expanded_source};
use crate::{
    core::digest::StableEncode,
    db::{key::QueryKey, memo::Database, poll::QueryPoll, value::QueryValue},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    ir::{op::Op, pc::Phase},
    syntax::body_index::{ExpressionKind, NodeRole},
};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("common pattern preparation failed: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("common pattern preparation suspended: {input:?}"),
    }
}

fn run(control: &str, edition: u16, calls: u64, comparisons: u64) {
    for body in [control.to_owned(), "emit!()".into(), "wrapper!()".into()] {
        let text = format!(
            "macro_rules! emit {{()=>{{{control}}}}} \
             macro_rules! wrapper {{()=>{{emit!()}}}} \
             fn entered(){{}} fn reject()->bool{{false}} fn cold()->i32{{99_i32}} \
             fn value()->i32{{{body}}} fn main(){{}}"
        );
        let mut db = Database::default();
        db.trace.detailed = true;
        let (input, discovery) = prepare_expanded_source(&mut db, &text, edition, 1, true);
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
        let target = definition("value");
        let cold = definition("cold");
        ready(db.publish_retained_syntax(input.clone()));
        let body = ready(db.publish_retained_body(input, target));
        let outcome = ready(db.fetch(
            &crate::db::provider::SyntaxProvider,
            QueryKey::BodyStructure(body.clone()),
        ));
        let QueryValue::BodyStructure(index) = outcome.value.as_ref() else {
            panic!("ordinary indexed body absent")
        };
        assert!(
            index
                .nodes()
                .iter()
                .any(|node| node.role == NodeRole::Pattern)
        );
        assert!(
            !index
                .nodes()
                .iter()
                .any(|node| node.role == NodeRole::Expression(ExpressionKind::Macro))
        );
        let trace_start = db.trace.events.len();
        let mut run = ready(RunMachine::start_with_discovery(
            &mut db,
            body.clone(),
            100_000,
            discovery,
        ));
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("common pattern route failed in edition {edition}: {text}")
        };
        assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "7"));
        assert_eq!(report.opcode_counters.started_by_opcode[10], comparisons);
        assert_eq!(
            report.opcode_counters.started_by_opcode[22], calls,
            "reached source/guard calls run once; cold result calls stay cold"
        );
        assert_eq!(
            report.opcode_counters.started_by_opcode[26], 0,
            "no assertion operand evaluator"
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert!(db.trace.events[trace_start..].iter().all(|event| {
            match &event.key {
                QueryKey::BodyStructure(actual) => actual.definition != cold,
                QueryKey::Thunk(pc) if pc.body == body => index
                    .nodes()
                    .get(pc.node as usize)
                    .is_some_and(|node| node.role != NodeRole::Expression(ExpressionKind::Macro)),
                _ => true,
            }
        }));
    }
}

#[test]
fn reached_scalar_patterns_share_ordinary_runtime_routes_across_actual_macro_origins() {
    for edition in [2015, 2018, 2021, 2024] {
        for (control, calls, comparisons) in [
            (
                "match {let _=entered();2_i32} {1_i32..=3_i32 if reject()=>cold(),_=>7_i32}",
                2,
                2,
            ),
            (
                "if let 1_i32..=3_i32={let _=entered();2_i32}{7_i32}else{cold()}",
                1,
                2,
            ),
            (
                "if let true={let _=entered();true}{7_i32}else{cold()}",
                1,
                1,
            ),
            (
                "if let false={let _=entered();true}{cold()}else{7_i32}",
                1,
                1,
            ),
            ("if let 10_i32..=UNRESOLVED=2_i32{cold()}else{7_i32}", 0, 1),
            ("match 2_i32{..=3_i32=>7_i32,_=>cold()}", 0, 1),
            ("match 2_i32{2_i32.. =>7_i32,_=>cold()}", 0, 1),
            ("{while let 1_i32..=3_i32=2_i32{break;}7_i32}", 0, 2),
            ("{while let false=true{cold();}7_i32}", 0, 1),
        ] {
            run(control, edition, calls, comparisons);
        }
    }
}

#[test]
fn reached_scalar_condition_chains_share_test_and_clause_cleanup_routes() {
    // Let-chain admission follows its actual owning edition. This is a runtime
    // cleanup control, not a claim about a compiler diagnostic in older editions.
    for (control, comparisons) in [
        (
            "if let 1_i32..=3_i32=2_i32 && let 4_i32..=6_i32=5_i32{7_i32}else{cold()}",
            4,
        ),
        (
            "if let 1_i32..=3_i32=2_i32 && let 8_i32..=UNRESOLVED=5_i32{cold()}else{7_i32}",
            3,
        ),
    ] {
        run(control, 2024, 0, comparisons);
    }
}

#[test]
fn changing_only_a_cold_pattern_result_to_a_source_call_keeps_it_unprepared() {
    for edition in [2015, 2018, 2021, 2024] {
        for result in ["99_i32", "cold()"] {
            run(
                &format!("match 2_i32{{1_i32..=3_i32=>7_i32,_=>{result}}}"),
                edition,
                0,
                2,
            );
            run(
                &format!("if let 1_i32..=3_i32=2_i32{{7_i32}}else{{{result}}}"),
                edition,
                0,
                2,
            );
        }
    }
}

#[test]
fn a_generated_condition_cannot_capture_a_same_spelled_caller_without_its_scope_join() {
    // Native Rust selects the definition-site x=2 and returns 7. LRR still lacks
    // that cross-context source join: explicit refusal is the required authority
    // control, never a runtime-contract pass for this valid Rust program.
    const SOURCE: &str = "fn value()->i32{let x=2_i32;macro_rules! test {()=>{if let 1_i32..=3_i32=x{7_i32}else{99_i32}}}let x=8_i32;test!()}fn main(){}";
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
        let body = ready(db.publish_retained_body(input, definition));
        let indexed = ready(db.fetch(
            &crate::db::provider::SyntaxProvider,
            QueryKey::BodyStructure(body),
        ));
        let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
            panic!("ordinary condition body absent")
        };
        let paths: Vec<_> = index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Path))
            .collect();
        assert_eq!(paths.len(), 1);
        let recipe = ready(db.fetch(
            &crate::db::provider::SyntaxProvider,
            QueryKey::Thunk(index.pc(paths[0].id, Phase::Entry).unwrap()),
        ));
        let QueryValue::Thunk(recipe) = recipe.value.as_ref() else {
            panic!("ordinary condition scrutinee thunk absent")
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

#[test]
fn false_guards_retry_selected_alternatives_without_replaying_the_source() {
    // Supplemental scalar frontier for unchanged search-via-bindings.rs. Its
    // nested tuple/binding Cartesian search remains a required stock gap.
    for edition in [2015, 2018, 2021, 2024] {
        run(
            "match {let _=entered();2_i32}{1_i32..=3_i32 | 2_i32 if reject()=>cold(),_=>7_i32}",
            edition,
            3,
            3,
        );
        run(
            "match {let _=entered();true}{true | true if reject()=>cold(),_=>7_i32}",
            edition,
            3,
            2,
        );
        run(
            "match true{_ | _ if reject()=>cold(),_=>7_i32}",
            edition,
            2,
            0,
        );
    }
}

#[test]
fn conditional_pending_bindings_commit_once_through_the_shared_transaction_owner() {
    for edition in [2015, 2018, 2021, 2024] {
        run("if let value=7_i32{value}else{cold()}", edition, 0, 0);
        run(
            "if let mut value=6_i32{value+=1_i32;value}else{cold()}",
            edition,
            0,
            1,
        );
    }
}
