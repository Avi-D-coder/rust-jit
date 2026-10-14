use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, BodyIndexLimits, ExpressionKind, NodeRole},
        conditions::{ConditionClause, ConditionLimits},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
fn source(text: &str) -> (ParsedFile, BodyHandle, BodyIndex, lrr::ir::pc::NodeId) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([211; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline.items[0].body.clone().unwrap();
    let index = parsed
        .index_body(&body, BodyIndexLimits::default())
        .unwrap();
    let conditional = index
        .nodes()
        .iter()
        .find(|n| {
            matches!(
                n.role,
                NodeRole::Expression(ExpressionKind::If | ExpressionKind::While)
            )
        })
        .unwrap()
        .id;
    (parsed, body, index, conditional)
}
#[test]
fn condition_plan_preserves_written_boolean_pattern_scrutinee_nodes_without_resolution() {
    let (parsed, body, index, node) =
        source("fn case(){if first() && let Some(ref value)=unknown() && later(){}} ");
    let plan = parsed
        .condition_plan(&body, &index, node, ConditionLimits::default())
        .unwrap();
    assert_eq!(plan.clauses().len(), 3);
    assert!(matches!(plan.clauses()[0], ConditionClause::Boolean { .. }));
    let ConditionClause::Pattern {
        node: pattern_clause,
        pattern,
        scrutinee,
    } = plan.clauses()[1]
    else {
        panic!("pattern clause absent")
    };
    assert!(pattern_clause.0 < scrutinee.0);
    assert_eq!(index.nodes()[pattern.0 as usize].role, NodeRole::Pattern);
    assert!(matches!(plan.clauses()[2], ConditionClause::Boolean { .. }));
    let keys = plan
        .clauses()
        .iter()
        .map(|c| c.node().0)
        .collect::<Vec<_>>();
    assert!(keys.windows(2).all(|p| p[0] < p[1]));
    assert_eq!(
        plan.fingerprint(),
        parsed
            .condition_plan(&body, &index, node, ConditionLimits::default())
            .unwrap()
            .fingerprint()
    );
    plan.require_source(&parsed.key(), index.body()).unwrap();
    let mut stale = parsed.key();
    stale.content = Digest([212; 32]);
    assert!(plan.require_source(&stale, index.body()).is_err());
}
#[test]
fn condition_plan_keeps_boolean_parentheses_and_nested_conditional_boundaries() {
    let (parsed, body, index, node) = source(
        "fn case(){if (true) && {if let true=missing(){true}else{false}} && let _=later(){}} ",
    );
    let plan = parsed
        .condition_plan(&body, &index, node, ConditionLimits::default())
        .unwrap();
    assert_eq!(plan.clauses().len(), 3);
    assert!(matches!(plan.clauses()[0], ConditionClause::Boolean { .. }));
    assert!(matches!(plan.clauses()[1], ConditionClause::Boolean { .. }));
    assert!(matches!(plan.clauses()[2], ConditionClause::Pattern { .. }));
    assert_eq!(
        parsed
            .condition_plan(
                &body,
                &index,
                node,
                ConditionLimits {
                    clauses: 2,
                    metadata_bytes: 32 * 1024 * 1024
                }
            )
            .unwrap_err()
            .code,
        "LRR-E-METADATA-LIMIT"
    );
}
#[test]
fn reached_condition_plan_rejects_non_top_level_let_grammar_without_operand_preparation() {
    for condition in [
        "true || let _=missing()",
        "!(let _=missing())",
        "(let _=missing())",
        "predicate(let _=missing())",
    ] {
        let (parsed, body, index, node) = source(&format!("fn case(){{if {condition} {{}}}}"));
        assert_eq!(
            parsed
                .condition_plan(&body, &index, node, ConditionLimits::default())
                .unwrap_err()
                .code,
            "LRR-E-UNSUPPORTED"
        );
    }
}

fn guest(
    text: &str,
) -> (
    lrr::db::memo::Database,
    lrr::engine::run::RunMachine,
    BodyIndex,
    lrr::engine::run::RunPoll,
) {
    use lrr::{
        db::{key::OutlineKey, memo::Database, poll::QueryPoll},
        engine::run::RunMachine,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::SourcePath,
        },
        syntax::parse::ParseKey,
    };
    let (_, _, index, _) = source(text);
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([211; 32]);
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
    let prepared = PreparedSourceInputs::refresh(
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
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let result = run.poll(&mut db);
    (db, run, index, result)
}
fn assert_calls_cold(
    db: &lrr::db::memo::Database,
    run: &lrr::engine::run::RunMachine,
    index: &BodyIndex,
) {
    use lrr::{db::key::QueryKey, ir::pc::Phase};
    for node in index
        .nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Call))
    {
        assert!(
            db.memo(&QueryKey::Thunk(index.pc(node.id, Phase::Entry).unwrap()))
                .is_none()
        );
        assert!(
            !run.installed_recipes()
                .recipes()
                .any(
                    |r| r.recipe().origin.span.relative_start == node.range.start
                        && r.recipe().origin.span.relative_end == node.range.end
                )
        );
    }
    for event in &db.trace.events {
        if let QueryKey::Thunk(pc) = &event.key {
            assert!(
                !(pc.body == *index.body()
                    && index.nodes().get(pc.node as usize).is_some_and(
                        |node| node.role == NodeRole::Expression(ExpressionKind::Call)
                    )),
                "cold call produced a query event"
            );
        }
    }
    assert_eq!(run.action_snapshot().published, 0);
    assert_eq!(run.action_snapshot().consumed, 0);
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[19], 0);
}
#[test]
fn false_initial_clause_keeps_pattern_scrutinee_unprepared_in_actual_if_and_while() {
    for text in [
        "fn case(){if false && let true=missing(){}else{}}",
        "fn case(){while false && let true=missing(){}}",
    ] {
        let (mut db, run, index, result) = guest(text);
        assert!(matches!(result, lrr::engine::run::RunPoll::Completed(_)));
        assert_calls_cold(&db, &run, &index);
        db.finish_run().unwrap();
    }
}
#[test]
fn reached_misplaced_let_faults_before_boolean_evaluation_and_cold_misplaced_condition_stays_unprepared()
 {
    let (mut db, run, index, result) = guest("fn case(){if true || let _=missing(){}}");
    let lrr::engine::run::RunPoll::Fault(error) = result else {
        panic!("malformed reached condition did not fault")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[0], 0);
    assert_calls_cold(&db, &run, &index);
    db.finish_run().unwrap();
    let (mut db, run, index, result) =
        guest("fn case(){if true{}else{if true || let _=missing(){}}}");
    assert!(matches!(result, lrr::engine::run::RunPoll::Completed(_)));
    assert_calls_cold(&db, &run, &index);
    db.finish_run().unwrap();
}

#[test]
fn reached_boolean_literal_patterns_execute_normal_vm_tests_and_select_actual_branches() {
    use lrr::engine::run::{RunPoll, ValueSummary};
    for (text, wanted, calls) in [
        (
            "fn case()->i64{if let true=true{7_i64}else{missing()}}",
            "7",
            0,
        ),
        (
            "fn case()->i64{if let true=false{missing()}else{9_i64}}",
            "9",
            0,
        ),
        (
            "fn case()->i64{if let false=false{7_i64}else{missing()}}",
            "7",
            0,
        ),
        (
            "fn case()->i64{if let true=predicate() && true{7_i64}else{9_i64}}fn predicate()->bool{true}",
            "7",
            1,
        ),
        (
            "fn case()->i64{if let true=predicate() && let false=second(){7_i64}else{9_i64}}fn predicate()->bool{true}fn second()->bool{true}",
            "9",
            2,
        ),
    ] {
        let (mut db, run, _, result) = guest(text);
        let RunPoll::Completed(report) = result else {
            panic!("boolean pattern did not complete")
        };
        assert!(matches!(report.value,ValueSummary::Signed(ref value) if value==wanted));
        assert!(report.opcode_counters.started_by_opcode[10] > 0);
        assert_eq!(report.opcode_counters.started_by_opcode[25], 0);
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
        assert_eq!(run.action_snapshot().published, calls);
        assert_eq!(run.action_snapshot().consumed, calls);
        db.finish_run().unwrap();
    }
}
#[test]
fn failed_boolean_pattern_skips_later_call_and_pattern_scrutinee_without_guest_panic() {
    for text in [
        "fn case(){if let true=false && missing(){}else{}}",
        "fn case(){if let true=false && let true=missing(){}else{}}",
    ] {
        let (mut db, run, index, result) = guest(text);
        assert!(matches!(result, lrr::engine::run::RunPoll::Completed(_)));
        assert_calls_cold(&db, &run, &index);
        assert_eq!(run.opcode_counters().unwrap().started_by_opcode[10], 1);
        db.finish_run().unwrap();
    }
}
#[test]
fn boolean_pattern_scrutinee_type_and_fuel_faults_never_become_pattern_mismatch() {
    use lrr::engine::run::RunPoll;
    for (text, code) in [
        ("fn case(){if let true=1_i64{}else{}}", "LRR-E-OWNERSHIP"),
        (
            "fn case(){if let true=wrong(){}else{}}fn wrong()->i64{1_i64}",
            "LRR-E-CALL-RETURN-TYPE",
        ),
        (
            "fn case(){if let true=looping(){}else{}}fn looping()->bool{loop{}}",
            "LRR-E-RUNTIME-RESOURCE",
        ),
    ] {
        let (mut db, run, _, result) = guest(text);
        let RunPoll::Fault(error) = result else {
            panic!("fault became pattern mismatch/completion")
        };
        assert_eq!(error.code, code);
        assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 0);
        assert_eq!(run.opcode_counters().unwrap().started_by_opcode[26], 0);
        db.finish_run().unwrap();
    }
}

#[test]
fn boolean_while_patterns_reenter_scalar_storage_and_preserve_continue_break_call_counts() {
    use lrr::engine::run::{RunPoll, ValueSummary};
    for (text, calls) in [
        (
            "fn case()->i64{let mut n=0_i64;while let true=(n<4_i64){n=n+1_i64;if n==1_i64{continue;}if n==3_i64{break;}}n}",
            0,
        ),
        (
            "fn case()->i64{let mut n=0_i64;while let true=(n<4_i64) && let false=(n==3_i64){n=n+1_i64;}n}",
            0,
        ),
        (
            "fn case()->i64{let mut n=0_i64;while let true=check(n){n=n+1_i64;if n==1_i64{continue;}}n}fn check(n:i64)->bool{n<3_i64}",
            4,
        ),
    ] {
        let (mut db, run, _, result) = guest(text);
        let RunPoll::Completed(report) = result else {
            panic!("while pattern did not complete")
        };
        assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="3"));
        assert!(report.opcode_counters.started_by_opcode[10] > 0);
        assert_eq!(run.action_snapshot().published, calls);
        assert_eq!(run.action_snapshot().consumed, calls);
        assert_eq!(run.action_snapshot().pending, None);
        db.finish_run().unwrap();
    }
}
