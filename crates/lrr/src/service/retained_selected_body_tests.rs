//! Actual per-body readiness and publication controls; upstream originals stay
//! unchanged. A scoped source is not whole-file execution/namespace authority.
use super::{
    retained_body_tests::prepare_expanded_body_source,
    retained_syntax_tests::{fixture, seal},
};
use crate::{
    core::digest::StableEncode,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        retained_syntax::RetainedSyntaxInput,
        value::QueryValue,
    },
    engine::{
        install::InstalledRecipes,
        run::{RunMachine, RunPoll, ValueSummary},
    },
    ir::pc::{NodeId, Phase},
};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("body readiness failed: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("body readiness suspended: {input:?}"),
    }
}

#[test]
fn parsing_another_body_keeps_installed_namespace_current_but_epoch_changes_do_not() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut f = fixture(edition, true);
        seal(&mut f);
        let source = f.db.prepared_source(f.prepared.source()).unwrap().unwrap();
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
        let main = definition("main");
        let other = definition("static_assert_exists");
        let first = f
            .discovery
            .prepare_body_syntax(source.selection(), main)
            .unwrap();
        let first = ready(RetainedSyntaxInput::prepare(
            &mut f.db,
            first,
            f.discovery.key().clone(),
        ));
        ready(f.db.publish_retained_syntax(first.clone()));
        let body = ready(f.db.publish_retained_body(first, main));
        let outcome = ready(f.db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body)));
        let QueryValue::BodyStructure(index) = outcome.value.as_ref() else {
            panic!("ordinary retained body index absent")
        };
        let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
        let outcome = ready(f.db.fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone())));
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("ordinary retained entry thunk absent")
        };
        let recipe = recipe
            .as_ref()
            .clone()
            .with_namespace_witness(&f.db)
            .unwrap();
        let mut installed = InstalledRecipes::new(
            f.db.source_revision(),
            crate::engine::telemetry::INSTALLED_LRIR_BUDGET,
        )
        .unwrap();
        let id = installed
            .install_prepared(&f.db, pc.clone(), &recipe)
            .unwrap();
        let namespace = f.db.lexical_namespace_stamps().unwrap();
        let source_revision = f.db.source_revision();
        let revision = f.db.revision();
        let next = f
            .discovery
            .prepare_body_syntax(source.selection(), other)
            .unwrap();
        let next = ready(RetainedSyntaxInput::prepare(
            &mut f.db,
            next,
            f.discovery.key().clone(),
        ));
        assert!(
            f.db.published_input(&InputKey::RetainedSyntax(next.key().content))
                .is_none()
        );
        ready(f.db.publish_retained_syntax(next));
        assert!(f.db.revision() > revision);
        assert_eq!(f.db.source_revision(), source_revision);
        assert_eq!(f.db.lexical_namespace_stamps().unwrap(), namespace);
        assert_eq!(installed.get_current(&f.db, id).unwrap().recipe().key, pc);
        // Preparing another body is not permission to ignore an actual change
        // to this recipe's source world. The unchanged stock calls are not run.
        f.db.begin_expansion_epoch(2).unwrap();
        assert_eq!(
            installed.get_current(&f.db, id).unwrap_err().code,
            "LRR-E-NAMESPACE-STALE"
        );
    }
}
#[test]
fn unchanged_nested_statement_body_gets_a_distinct_exact_readiness_scope() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut f = fixture(edition, true);
        seal(&mut f);
        let source = f.db.prepared_source(f.prepared.source()).unwrap().unwrap();
        let main = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .def_key
            .fingerprint();
        let other = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "static_assert_exists")
            .unwrap()
            .def_key
            .fingerprint();
        let prepared = f
            .discovery
            .prepare_body_syntax(source.selection(), main)
            .unwrap();
        assert_eq!(prepared.syntax().identity(), f.prepared.syntax().identity());
        assert_ne!(prepared.identity(), f.prepared.identity());
        assert!(prepared.pending_calls().is_empty());
        assert_eq!(prepared.outputs().len(), 2);
        let input = ready(RetainedSyntaxInput::prepare(
            &mut f.db,
            prepared,
            f.discovery.key().clone(),
        ));
        ready(f.db.publish_retained_syntax(input.clone()));
        let before = f.db.revision();
        assert!(
            matches!(f.db.publish_retained_body(input.clone(), other), QueryPoll::Ready(Err(error)) if error.code == "LRR-E-EXPANSION-INPUT" && error.message.contains("another original body"))
        );
        assert_eq!(f.db.revision(), before);
        assert_eq!(
            ready(f.db.publish_retained_body(input, main)).definition,
            main
        );
    }
    // The original generic calls are not executed by this readiness control.
}

#[test]
fn selected_function_executes_while_unowned_cold_calls_remain_pending() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "macro_rules! compute {()=>{2_i32+5_i32}} #[cfg(any())] fn cold(){unknown!();} fn value()->i32{compute!()} fn other(){} fn main(){}",
            "macro_rules! compute {()=>{2_i32+5_i32}} fn value()->i32{#[cfg(any())] fn cold(){unknown!();} compute!()} fn other(){} fn main(){}",
        ] {
            let mut db = Database::default();
            db.trace.detailed = true;
            let (input, discovery) =
                prepare_expanded_body_source(&mut db, text, edition, 1, "value");
            assert_eq!(input.prepared().pending_calls().len(), 1);
            assert_eq!(input.prepared().outputs().len(), 1);
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let selected = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "value")
                .unwrap()
                .def_key
                .fingerprint();
            let other = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "other")
                .unwrap()
                .def_key
                .fingerprint();
            ready(db.publish_retained_syntax(input.clone()));
            let revision = db.revision();
            assert!(
                matches!(db.publish_retained_body(input.clone(), other), QueryPoll::Ready(Err(error)) if error.code == "LRR-E-EXPANSION-INPUT" && error.message.contains("another original body"))
            );
            assert_eq!(db.revision(), revision);
            assert!(
                matches!(db.published_input(&InputKey::DefinitionSource(other)), Some((InputValue::DefinitionSource(parse), _)) if parse == &source.selection().outline.parse)
            );
            let body = ready(db.publish_retained_body(input.clone(), selected));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, body, 100_000, discovery,
            ));
            let RunPoll::Completed(report) = run.poll(&mut db) else {
                panic!("selected body failed while a cold call stayed pending")
            };
            assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "7"));
            assert_eq!(report.opcode_counters.started_by_opcode[10], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[22], 0);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            drop(run);
            db.begin_expansion_epoch(2).unwrap();
            let revision = db.revision();
            assert!(
                matches!(db.publish_retained_body(input, selected), QueryPoll::Ready(Err(error)) if error.code == "LRR-E-EXPANSION-INPUT")
            );
            assert_eq!(db.revision(), revision);
        }
    }
}
