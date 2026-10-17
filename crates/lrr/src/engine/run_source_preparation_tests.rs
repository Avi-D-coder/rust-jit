//! Actual ordinary interpreter controls. The primary program is the complete,
//! unchanged rustc suffix-inference source, not a selected-body replacement.
#[path = "run_stock_local_call_tests.rs"]
mod stock_local_call_tests;
#[path = "run_stock_type_argument_tests.rs"]
mod stock_type_argument_tests;
use super::super::lexical_numeric_callees_tests::{ORIGINAL, setup};
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        trace::Action,
    },
    engine::standalone_macros::CompletedStandaloneRuntimeMacros,
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        entered_macros::EnteredMacroFacts,
    },
};
const EDITIONS: [u16; 4] = [2015, 2018, 2021, 2024];
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("reached local source preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("reached local source demand: {input:?}"),
    }
}
struct Harness {
    db: Database,
    run: RunMachine,
    macros: CompletedStandaloneRuntimeMacros,
    parent: Arc<PreparedSourceLocalPlan>,
    facts: Arc<EnteredMacroFacts>,
}
fn start(text: &str, edition: u16, entry: &str) -> Harness {
    let (mut db, source, discovery) = setup(text, edition);
    db.begin_expansion_epoch(1).unwrap();
    discovery.publish(&mut db).unwrap();
    // Use the same genuine sealed entry selector as the standalone service.
    // Generated declarations need not be present in the original outline.
    let selected = ready(discovery.entry_function(&mut db, entry));
    let crate::frontend::callables::CallSelection::Source(selected) = selected else {
        panic!("reached source entry did not select an ordinary function: {selected:?}");
    };
    // Use the production entry adapter: completed macro outputs must join the
    // ordinary retained source tree before indexing/executing the selected body.
    let selected =
        crate::cli::retained_entry::prepare(&mut db, &discovery, &source, entry, selected).unwrap();
    selected.revalidate(&mut db).unwrap();
    let body = selected.body().expect("selected source entry body").key();
    let parent = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
    let facts = Arc::clone(parent.context().facts());
    let macros = CompletedStandaloneRuntimeMacros::from_discovery(&source, &discovery).unwrap();
    let run = ready(RunMachine::start_with_discovery(
        &mut db, body, 1_000_000, discovery,
    ));
    db.begin_run().unwrap();
    Harness {
        db,
        run,
        macros,
        parent,
        facts,
    }
}
fn child_bodies(h: &Harness) -> Vec<crate::source::manifest::BodyKey> {
    h.facts
        .items
        .iter()
        .filter_map(|row| row.item.body.as_ref().map(|body| body.key()))
        .collect()
}
fn assert_children_cold(h: &Harness) {
    let children = child_bodies(h);
    assert!(!children.is_empty());
    for child in &children {
        assert!(
            h.db.published_input(&InputKey::BodyEnvironment(child.clone()))
                .is_none()
        );
    }
    assert!(h.db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => !children.contains(body),
        QueryKey::Thunk(pc) => !children.contains(&pc.body),
        _ => true,
    }));
    assert_eq!(h.run.opcode_counters().unwrap().operations_started, 0);
}
fn completed(h: &mut Harness) -> RunReport {
    let contexts = h.macros.contexts().unwrap();
    match h.run.poll_with_runtime_macros(&mut h.db, &contexts) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("unchanged local program faulted: {error:?}"),
        RunPoll::Panicked(report) => panic!("unchanged local program panicked: {report:?}"),
        RunPoll::NeedInput(input) => panic!("unchanged local program suspended: {input:?}"),
    }
}
fn assert_complete_suffix_main(text: &str) {
    for edition in EDITIONS {
        let mut h = start(text, edition, "main");
        let children = child_bodies(&h);
        assert_eq!(children.len(), 9);
        let calls = h
            .run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
            .count();
        assert_eq!(
            calls, 18,
            "full upstream source must retain both calls to every local"
        );
        assert_children_cold(&h);
        let revision = h.db.revision();
        let source_revision = h.db.source_revision();
        let report = completed(&mut h);
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[19], 18); // CallSeal
        assert_eq!(report.opcode_counters.started_by_opcode[20], 18); // CheckParameters
        assert_eq!(report.opcode_counters.started_by_opcode[21], 18); // ReadParameter
        assert_eq!(report.opcode_counters.started_by_opcode[22], 18); // CallPrepare
        assert_eq!(report.opcode_counters.started_by_opcode[23], 18); // CallAdvance
        assert_eq!(report.opcode_counters.started_by_opcode[8], 9); // each written j argument
        assert_eq!(report.opcode_counters.started_by_opcode[0], 27); // all written integer values
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0); // legacy Assert
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert!(report.opcode_counters.return_scopes_cleaned >= 19);
        assert_eq!(h.db.source_revision(), source_revision);
        // Nine genuine body rows + nine genuine child environments. The second
        // reached calls must publish neither again. Pure memo insertions do not
        // advance the source input revision.
        assert_eq!(h.db.revision(), revision + 18);
        for child in children {
            let input =
                h.db.input(&InputKey::BodyEnvironment(child.clone()))
                    .expect("actual reached child environment");
            let InputValue::BodyEnvironment(environment) = input.as_ref() else {
                panic!("wrong child input variant")
            };
            assert_eq!(environment.body(), &child);
            let actual_parent = environment
                .source()
                .local_parent()
                .expect("actual retained lexical parent");
            assert_eq!(actual_parent.source_identity(), h.parent.source_identity());
            let plan = ready(h.db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(child.clone())));
            let QueryValue::SourceLocalPlan(plan) = plan.value.as_ref() else {
                panic!("wrong child memo variant")
            };
            assert_eq!(
                plan.environment().unwrap().identity(),
                environment.identity()
            );
            assert!(Arc::ptr_eq(
                plan.context().source().index(),
                environment.source().index()
            ));
            assert_eq!(
                h.db.trace
                    .events
                    .iter()
                    .filter(
                        |event| event.key == QueryKey::SourceLocalPlan(child.clone())
                            && event.action == Action::ProviderRun
                    )
                    .count(),
                1,
                "child local tables were reconstructed instead of memoized"
            );
        }
        for (kind, expected) in [("EnteredCall", 18), ("PreparedReturnPlan", 9)] {
            assert_eq!(
                h.db.trace
                    .events
                    .iter()
                    .filter(|event| event.key.kind() == kind && event.action == Action::ProviderRun)
                    .count(),
                expected,
                "common source preparation must be retained across phases and repeated calls"
            );
        }
        assert!(h.run.peak_guest_frames >= 2);
        assert!(h.run.calls.is_empty() && h.run.pending.is_empty());
        assert_eq!(h.run.call_bytes, 0);
        assert_eq!(h.run.pending_bytes, 0);
        assert!(h.run.returning.is_none() && h.run.unwinding.is_none());
        assert!(h.run.frame.active_scopes().is_empty());
        assert!(h.run.terminal);
        h.db.finish_run().unwrap();
    }
}
#[test]
fn unchanged_suffix_main_executes_local_calls_and_reuses_one_current_child_plan_all_editions() {
    assert_complete_suffix_main(ORIGINAL);
}
#[test]
fn upstream_suffix_user_macro_output_uses_same_ordinary_calls_all_editions() {
    // Whole upstream payload, unchanged. Only its source origin differs.
    let source = format!("macro_rules! emit {{ () => {{ {ORIGINAL} }} }} emit!();");
    assert_complete_suffix_main(&source);
}
#[test]
fn upstream_suffix_wrapper_macro_output_uses_same_ordinary_calls_all_editions() {
    let source = format!(
        "macro_rules! emit {{ () => {{ {ORIGINAL} }} }} \
         macro_rules! wrapper {{ () => {{ emit!(); }} }} wrapper!();"
    );
    assert_complete_suffix_main(&source);
}
#[test]
fn wrong_runtime_argument_type_never_executes_the_selected_local_callee_all_editions() {
    // This appended LRR-only negative does not replace or edit any original
    // source. Its ill-typed call checks runtime packet refusal, not Rust syntax
    // acceptance. The complete original main is exercised by the test above.
    const CONTROL: &str =
        "\nfn lrr_wrong_type(){fn lrr_rejected_child(n:i8)->i8{n}lrr_rejected_child(true);}\n";
    let text = format!("{ORIGINAL}{CONTROL}");
    for edition in EDITIONS {
        let mut h = start(&text, edition, "lrr_wrong_type");
        let children = child_bodies(&h);
        assert_eq!(children.len(), 1);
        assert_children_cold(&h);
        let contexts = h.macros.contexts().unwrap();
        match h.run.poll_with_runtime_macros(&mut h.db, &contexts) {
            RunPoll::Fault(error) => {
                assert_eq!(error.code, "LRR-E-CALL-ARG-TYPE", "{error:?}");
                assert_eq!(
                    error.message,
                    "argument descriptor differs from the reached concrete signature",
                    "{error:?}"
                );
            }
            RunPoll::Completed(_) => panic!("wrong runtime argument executed"),
            RunPoll::Panicked(report) => panic!("wrong runtime argument panicked: {report:?}"),
            RunPoll::NeedInput(input) => {
                panic!("wrong runtime argument needed authority: {input:?}")
            }
        }
        let counters = h.run.opcode_counters().unwrap();
        assert_eq!(counters.started_by_opcode[0], 1); // actual Bool argument evaluated
        assert_eq!(counters.started_by_opcode[19], 0); // no call action committed
        assert_eq!(counters.started_by_opcode[20], 0); // child entry never ran
        assert_eq!(counters.started_by_opcode[21], 0); // child parameter never read
        assert_eq!(counters.started_by_opcode[26], 0);
        assert!(h.db.trace.events.iter().all(|event| !matches!(&event.key,
            QueryKey::Thunk(pc) if children.contains(&pc.body))));
        assert!(h.run.calls.is_empty());
        assert_eq!(h.run.peak_guest_frames, 1);
        h.db.finish_run().unwrap();
    }
}

#[test]
fn revoked_selected_local_source_is_refused_before_child_body_query() {
    let mut h = start(ORIGINAL, 2024, "main");
    let site = h
        .parent
        .context()
        .source()
        .index()
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .unwrap()
        .id;
    let key = h
        .parent
        .context()
        .source()
        .index()
        .pc(site, Phase::Entry)
        .unwrap();
    let selected = ready(h.db.fetch(&SyntaxProvider, QueryKey::EnteredCall(key)));
    let QueryValue::EnteredCall(goal) = selected.value.as_ref() else {
        panic!()
    };
    let crate::frontend::entered_call_goals::EnteredCallSelection::Local(proof) = goal.selection()
    else {
        panic!()
    };
    h.db.publish_ordinary_local_function(proof).unwrap();
    let callee = goal.local_callable(&mut h.db).unwrap();
    let child = callee.body().unwrap().key();
    assert!(h.db.memo(&QueryKey::BodyStructure(child.clone())).is_none());
    h.db.finish_run().unwrap();
    h.db.begin_expansion_epoch(2).unwrap();
    let start = h.db.trace.events.len();
    match RunMachine::selected_source_body(&mut h.db, &callee) {
        QueryPoll::Ready(Err(error)) => {
            assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
            assert_eq!(error.message, "prepared operation input changed");
        }
        other => panic!("revoked selected source was not refused: {other:?}"),
    }
    assert!(
        !h.db.trace.events[start..]
            .iter()
            .any(|event| matches!(&event.key, QueryKey::BodyStructure(body) if *body == child))
    );
}

enum RetainedLocalSource {
    Admitted(Arc<AdmittedBodyContext>),
    Prepared(Arc<crate::frontend::source_body_context::PreparedBodyContext>),
    Macros(Arc<SealedBodyMacroEnvironment>),
}
impl RetainedLocalSource {
    fn source(&self) -> Option<&Arc<AdmittedBodyContext>> {
        match self {
            Self::Admitted(source) => Some(source),
            Self::Prepared(context) => Some(context.source()),
            Self::Macros(_) => None,
        }
    }
}
#[test]
fn retired_local_preparation_roots_are_charged_until_last_raw_source_context_or_macro_release() {
    for kind in 0..3 {
        let mut h = start(ORIGINAL, 2024, "main");
        completed(&mut h);
        let children = child_bodies(&h);
        let child = children[0].clone();
        let retained = if kind == 1 {
            let selected =
                ready(h.db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(child.clone())));
            let QueryValue::SourceLocalPlan(plan) = selected.value.as_ref() else {
                panic!("actual child plan absent")
            };
            RetainedLocalSource::Prepared(Arc::clone(plan.context()))
        } else {
            let selected = h.db.input(&InputKey::BodyEnvironment(child)).unwrap();
            let InputValue::BodyEnvironment(environment) = selected.as_ref() else {
                panic!("actual child environment absent")
            };
            if kind == 0 {
                RetainedLocalSource::Admitted(Arc::clone(environment.source()))
            } else {
                RetainedLocalSource::Macros(Arc::clone(environment.macros()))
            }
        };
        if let Some(source) = retained.source() {
            assert!(source.local_parent().is_some());
        }
        let parent_body = h.parent.context().source().index().body().clone();
        let parent_identity = h.parent.source_identity();
        h.db.finish_run().unwrap();
        let Harness {
            mut db,
            run,
            macros,
            parent,
            facts,
        } = h;
        drop(run);
        drop(macros);
        drop(parent);
        drop(facts);
        // Genuine public whole-universe revocation. The only external source
        // owner left is the raw child admitted/prepared/macro handle above,
        // not a BodyEnvironment InputValue/LocalPlan/query outer wrapper.
        db.publish_inputs(std::collections::BTreeMap::new())
            .unwrap();
        for body in std::iter::once(parent_body).chain(children) {
            match db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body)) {
                QueryPoll::Ready(Err(error)) => {
                    assert_eq!(error.code, "LRR-E-BODY-ENVIRONMENT", "{error:?}");
                    assert_eq!(
                        error.message, "owning source environment is not published",
                        "{error:?}"
                    );
                }
                QueryPoll::Ready(Ok(_)) => panic!("revoked source-local plan still admitted"),
                QueryPoll::NeedInput(input) => {
                    panic!("revoked source-local plan demanded another input: {input:?}")
                }
            }
        }
        match &retained {
            RetainedLocalSource::Macros(macros) => match macros.require_current(&mut db) {
                QueryPoll::NeedInput(crate::db::poll::InputRequest::Discover { key }) => {
                    assert_eq!(&key, macros.seal())
                }
                QueryPoll::Ready(Err(error)) => {
                    panic!("revoked macros returned another typed outcome: {error:?}")
                }
                QueryPoll::Ready(Ok(())) => {
                    panic!("raw macros granted revoked discovery authority")
                }
                QueryPoll::NeedInput(input) => {
                    panic!("raw macros demanded another input: {input:?}")
                }
            },
            _ => {
                let source = retained.source().unwrap();
                assert_eq!(
                    source.local_parent().unwrap().source_identity(),
                    parent_identity
                );
                match source.require_current(&mut db) {
                    QueryPoll::Ready(Err(error)) => assert_eq!(
                        error.message, "published local environment was replaced",
                        "{error:?}"
                    ),
                    QueryPoll::Ready(Ok(())) => {
                        panic!("raw retained source granted revoked execution authority")
                    }
                    QueryPoll::NeedInput(input) => {
                        panic!("raw retained source demanded another input: {input:?}")
                    }
                }
            }
        }
        let zero = crate::engine::telemetry::CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            source_input_bytes: 0,
        };
        let error = db.evict_between_runs(zero).unwrap_err();
        assert_eq!(error.code, "LRR-E-METADATA-LIMIT", "{error:?}");
        assert_eq!(
            error.message, "pinned cache/source inputs exceed configured cap",
            "{error:?}"
        );
        let pinned = db.cache_usage().unwrap();
        assert!(pinned.pinned_memo_bytes > 0);
        assert!(pinned.parse_bytes > 0 && pinned.source_input_bytes > 0);
        drop(retained);
        let released = db.evict_between_runs(zero).unwrap();
        assert_eq!(released.after.memo_bytes, 0);
        assert_eq!(released.after.parse_bytes, 0);
        assert_eq!(released.after.source_input_bytes, 0);
    }
}

#[path = "run_stock_module_cast_tests.rs"]
mod stock_module_cast_tests;
