use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    facade::macros::RuntimeMacroKind,
    frontend::{
        macro_names::PathLookup,
        runtime_macro_catalog::RuntimeMacroCatalog,
        runtime_macro_plan::{
            RuntimeIntrinsicPlan, RuntimeMacroPreparation, StaticPanicRecipeLowering,
            prepare_runtime_macro,
        },
    },
    ir::{
        op::Op,
        pc::{NodeId, Phase},
        recipe::{Origin, RecipeDraft, RecipeExit},
        validate::verify,
    },
    runtime::cell::GuestStore,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole, ScopeId},
        parse::{ParseKey, ParsedFile},
        spans::SourceRange,
    },
    vm::{
        frame::{Frame, FrameStatus},
        step::{StepOutcome, step},
    },
};
fn source(
    text: &str,
) -> (
    ParsedFile,
    PreparedSourceView,
    BodyIndex,
    Vec<NodeId>,
    PreparedSourceInputs,
) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([176; 32]);
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
    let key = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "case")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let body = view
        .index_body(view.selection(), &key, Default::default())
        .unwrap();
    let nodes = body
        .nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Macro))
        .map(|n| n.id)
        .collect();
    (parsed, view, body, nodes, inputs)
}
fn plan(parsed: &ParsedFile, text: &str, whole: &str) -> Box<RuntimeIntrinsicPlan> {
    let start = whole.find(text).unwrap() as u32;
    let call = parsed
        .macro_call_at(SourceRange {
            start,
            end: start + text.len() as u32,
        })
        .unwrap();
    let catalog = RuntimeMacroCatalog::new(Digest([175; 32]), 2024).unwrap();
    let definition = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap() == Some(RuntimeMacroKind::Panic))
        .unwrap()
        .clone();
    let RuntimeMacroPreparation::Intrinsic(plan) =
        prepare_runtime_macro(parsed, call, PathLookup::Defined(definition), &catalog).unwrap()
    else {
        panic!("plan absent")
    };
    plan
}
#[test]
fn static_panic_recipe_emission_validates_source_node_and_executes_guest_origin() {
    let text = "fn case(){renamed!(\"source-bound panic\");}";
    let (parsed, view, body, nodes, _inputs) = source(text);
    let plan = plan(&parsed, "renamed!(\"source-bound panic\")", text);
    let key = body.pc(nodes[0], Phase::Entry).unwrap();
    let next = body.pc(NodeId(0), Phase::Exit).unwrap();
    let StaticPanicRecipeLowering::Recipe(recipe) = plan
        .lower_static_panic_recipe(&view, &body, &key, next)
        .unwrap()
    else {
        panic!("unexpected formatting demand")
    };
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    let mut frame = Frame::new(
        body.clone(),
        body.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    let enter = verify(
        RecipeDraft {
            key: body.pc(NodeId(0), Phase::Entry).unwrap(),
            origin: Origin {
                span: body.span(NodeId(0)).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: vec![Op::ScopeEnter { scope: ScopeId(0) }],
            exits: vec![RecipeExit::Next(key.clone())],
            dependencies: vec![],
        },
        &body,
    )
    .unwrap();
    step(&mut frame, &mut store, &enter).unwrap();
    let StepOutcome::GuestPanic(panic) = step(&mut frame, &mut store, &recipe).unwrap() else {
        panic!("emitted recipe did not panic")
    };
    assert_eq!(panic.description(), "source-bound panic");
    assert_eq!(panic.pc, key);
    assert_eq!(panic.origin.span, body.span(nodes[0]).unwrap());
    frame.finish_panic(&mut store).unwrap();
    assert_eq!(frame.status(), FrameStatus::PanicCleaned);
}
#[test]
fn static_panic_recipe_emission_refuses_wrong_macro_phase_body_and_stale_publication() {
    let text = "fn case(){renamed!(\"one\");renamed!(\"two\");}";
    let (parsed, view, body, nodes, _inputs) = source(text);
    let plan = plan(&parsed, "renamed!(\"one\")", text);
    let next = body.pc(NodeId(0), Phase::Exit).unwrap();
    for key in [
        body.pc(nodes[1], Phase::Entry).unwrap(),
        body.pc(nodes[0], Phase::Commit).unwrap(),
        body.pc(NodeId(0), Phase::Entry).unwrap(),
    ] {
        assert!(
            plan.lower_static_panic_recipe(&view, &body, &key, next.clone())
                .is_err()
        );
    }
    let (_, other, other_body, _, _inputs) = source("fn case(){renamed!(\"changed\");}");
    let key = body.pc(nodes[0], Phase::Entry).unwrap();
    assert!(
        plan.lower_static_panic_recipe(&other, &body, &key, next.clone())
            .is_err()
    );
    assert!(
        plan.lower_static_panic_recipe(&view, &other_body, &key, next)
            .is_err()
    );
}
#[test]
fn static_panic_recipe_emission_preserves_formatting_demand_without_emitting_ir() {
    let text = "fn case(){renamed!(\"{unknown}\");}";
    let (parsed, view, body, nodes, _inputs) = source(text);
    let plan = plan(&parsed, "renamed!(\"{unknown}\")", text);
    assert!(matches!(
        plan.lower_static_panic_recipe(
            &view,
            &body,
            &body.pc(nodes[0], Phase::Entry).unwrap(),
            body.pc(NodeId(0), Phase::Exit).unwrap()
        )
        .unwrap(),
        StaticPanicRecipeLowering::NeedsFormatting
    ));
}

#[test]
fn dispatcher_reaches_prepared_static_panic_and_installs_source_checked_operation_lazily() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let text = "fn case(){renamed!(\"dispatcher panic\");}";
    let (parsed, view, body, nodes, inputs) = source(text);
    let prepared = plan(&parsed, "renamed!(\"dispatcher panic\")", text);
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run did not start")
    };
    let events = db.trace.events.len();
    run.queue_static_panic_plan(&db, view, prepared).unwrap();
    assert_eq!(run.installed_recipes().len(), 0);
    assert_eq!(db.trace.events.len(), events);
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("reached prepared panic did not panic")
    };
    assert_eq!(report.panic.description(), "dispatcher panic");
    assert_eq!(report.panic.origin.span, body.span(nodes[0]).unwrap());
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 1);
    let key = body.pc(nodes[0], Phase::Entry).unwrap();
    assert!(db.memo(&QueryKey::Thunk(key.clone())).is_none());
    assert!(
        run.installed_recipes()
            .recipes()
            .any(|r| r.recipe().key == key)
    );
    db.finish_run().unwrap();
}
#[test]
fn queued_cold_static_panic_has_no_thunk_or_installed_operation_preparation() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll, ValueSummary},
    };
    let text = "fn case(){if true {} else {renamed!(\"cold panic\");}}";
    let (parsed, view, body, nodes, inputs) = source(text);
    let prepared = plan(&parsed, "renamed!(\"cold panic\")", text);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    run.queue_static_panic_plan(&db, view, prepared).unwrap();
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("cold panic branch was prepared")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    let key = body.pc(nodes[0], Phase::Entry).unwrap();
    assert!(db.memo(&QueryKey::Thunk(key.clone())).is_none());
    assert!(
        run.installed_recipes()
            .recipes()
            .all(|r| r.recipe().key != key)
    );
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 0);
    db.finish_run().unwrap();
}
#[test]
fn static_panic_dispatch_queue_refuses_foreign_duplicate_late_and_dynamic_formatting_as_panic() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let text = "fn case(){renamed!(\"{unknown}\");}";
    let (parsed, view, body, _, inputs) = source(text);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let (_, foreign, _, _, _) = source(text);
    assert!(
        run.queue_static_panic_plan(&db, foreign, plan(&parsed, "renamed!(\"{unknown}\")", text))
            .is_err()
    );
    run.queue_static_panic_plan(
        &db,
        view.share(),
        plan(&parsed, "renamed!(\"{unknown}\")", text),
    )
    .unwrap();
    assert!(
        run.queue_static_panic_plan(
            &db,
            view.share(),
            plan(&parsed, "renamed!(\"{unknown}\")", text)
        )
        .is_err()
    );
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("formatting demand became a guest panic")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 0);
    assert!(
        run.queue_static_panic_plan(&db, view, plan(&parsed, "renamed!(\"{unknown}\")", text))
            .is_err()
    );
    db.finish_run().unwrap();
}

#[test]
fn prepared_callee_panic_unwinds_actual_suspended_callers_once() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let text = "fn leaf(){renamed!(\"callee panic\");} fn middle(){leaf();} fn case(){middle();}";
    let (parsed, view, body, _, inputs) = source(text);
    let leaf = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "leaf")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 10000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let events = db.trace.events.len();
    run.queue_static_panic_plan(
        &db,
        view.share(),
        plan(&parsed, "renamed!(\"callee panic\")", text),
    )
    .unwrap();
    assert_eq!(db.trace.events.len(), events);
    assert!(db.memo(&QueryKey::BodyStructure(leaf.clone())).is_none());
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("callee panic was not dispatched")
    };
    assert_eq!(report.panic.description(), "callee panic");
    assert_eq!(report.panic.origin.span.body_key, leaf);
    assert_eq!(run.action_snapshot().published, 2);
    assert_eq!(run.action_snapshot().consumed, 2);
    assert_eq!(run.action_snapshot().pending, None);
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 1);
    assert_eq!(run.opcode_counters().unwrap().panic_scopes_cleaned, 6);
    db.finish_run().unwrap();
}
#[test]
fn queued_uncalled_callee_panic_leaves_body_and_installed_origins_cold() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let text = "fn leaf(){renamed!(\"uncalled\");} fn case(){}";
    let (parsed, view, body, _, inputs) = source(text);
    let leaf = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "leaf")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    run.queue_static_panic_plan(&db, view, plan(&parsed, "renamed!(\"uncalled\")", text))
        .unwrap();
    assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
    assert!(db.memo(&QueryKey::BodyStructure(leaf.clone())).is_none());
    assert!(
        run.installed_recipes()
            .recipes()
            .all(|r| r.recipe().key.body != leaf)
    );
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 0);
    assert_eq!(run.action_snapshot().published, 0);
    db.finish_run().unwrap();
}
#[test]
fn recursive_prepared_callee_panic_preserves_exact_once_actions_and_dispatcher_cleanup() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let text = "fn dive(n:i64){if n==0i64 {renamed!(\"recursive panic\");} else {dive(n-1i64);}} fn case(){dive(200i64);}";
    let (parsed, view, body, _, inputs) = source(text);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 100000, vec![view.share()])
    else {
        panic!("run absent")
    };
    run.queue_static_panic_plan(
        &db,
        view,
        plan(&parsed, "renamed!(\"recursive panic\")", text),
    )
    .unwrap();
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("recursive panic absent")
    };
    assert_eq!(report.panic.description(), "recursive panic");
    assert_eq!(run.peak_guest_frames(), 202);
    assert_eq!(run.action_snapshot().published, 201);
    assert_eq!(run.action_snapshot().consumed, 201);
    assert_eq!(run.action_snapshot().pending, None);
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 1);
    assert!(run.opcode_counters().unwrap().panic_scopes_cleaned >= 202);
    db.finish_run().unwrap();
}

fn macro_namespace(
    view: &PreparedSourceView,
) -> (
    lrr::frontend::source_macros::PublishedMacroIndex,
    lrr::frontend::macro_names::MacroNamespaces,
    RuntimeMacroCatalog,
) {
    use lrr::{
        expand::scopes::MacroName,
        frontend::{
            hygiene::HygieneFacts, source_macro_routes::SourceMacroRoutes,
            source_macros::PublishedMacroIndex,
        },
        syntax::token_wire as runtime,
    };
    use lrr_expand_adapter::{
        hygiene::HygieneStore,
        token_wire::{Edition, Key, TokenForest},
    };
    let core = Digest([175; 32]);
    let unit = view.selection().outline.crate_key;
    let mut hygiene = HygieneStore::default();
    let context = hygiene.root(Key(unit.0), Edition::E2024).unwrap();
    let mut facts = || {
        HygieneFacts::new(
            runtime::TokenForest::decode(
                &TokenForest::new(vec![], hygiene.records(), vec![])
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
            .unwrap()
            .contexts()
            .to_vec(),
            serde_json::from_slice(
                &serde_json::to_vec(&hygiene.resolution_contexts(&[context]).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_name_contexts(
            serde_json::from_slice(
                &serde_json::to_vec(&hygiene.name_contexts(&[context]).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let routes = SourceMacroRoutes::from_outline(
        view.outline(),
        runtime::Key(context.0),
        facts(),
        &std::collections::BTreeMap::from([(MacroName::parse("core", 2024).unwrap(), core)]),
    )
    .unwrap();
    let catalog = RuntimeMacroCatalog::new(core, 2024).unwrap();
    let names =
        SourceMacroRoutes::candidate_graph(&[catalog.source_routes().unwrap(), routes]).unwrap();
    let index =
        PublishedMacroIndex::from_view(view.share(), runtime::Key(context.0), facts()).unwrap();
    (index, names, catalog)
}
fn invocation_range(text: &str, invocation: &str) -> SourceRange {
    let start = u32::try_from(text.find(invocation).unwrap()).unwrap();
    SourceRange {
        start,
        end: start + u32::try_from(invocation.len()).unwrap(),
    }
}
#[test]
fn entered_source_alias_resolves_static_panic_into_actual_dispatcher() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    let text = "use core::panic as stop; fn case(){stop!(\"source resolved\");}";
    let (_, view, body, _, inputs) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    let entered = index.enter_body(view.selection(), body.body()).unwrap();
    let RuntimeMacroPreparation::Intrinsic(plan) = entered
        .prepare_runtime_macro(
            view.selection(),
            body.body(),
            invocation_range(text, "stop!(\"source resolved\")"),
            &names,
            &catalog,
        )
        .unwrap()
    else {
        panic!("alias did not resolve intrinsic")
    };
    assert_eq!(plan.arguments().kind(), RuntimeMacroKind::Panic);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run failed")
    };
    run.queue_static_panic_plan(&db, view, plan).unwrap();
    assert_eq!(run.installed_recipes().len(), 0);
    let RunPoll::Panicked(panic) = run.poll(&mut db) else {
        panic!("guest panic absent")
    };
    assert_eq!(panic.panic.description(), "source resolved");
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[25], 1);
    db.finish_run().unwrap();
}
#[test]
fn entered_source_macro_shadowing_retains_user_grammar_and_unresolved_outcomes() {
    let text = "use core::panic as stop; fn case(){macro_rules! stop { ($($t:tt)*)=>{}; } stop!(custom => grammar); absent!(custom => grammar);}";
    let (_, view, body, _, _) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    let entered = index.enter_body(view.selection(), body.body()).unwrap();
    let RuntimeMacroPreparation::SourceExpansion {
        definition,
        invocation,
    } = entered
        .prepare_runtime_macro(
            view.selection(),
            body.body(),
            invocation_range(text, "stop!(custom => grammar)"),
            &names,
            &catalog,
        )
        .unwrap()
    else {
        panic!("user shadowing lost")
    };
    assert_eq!(catalog.select(&definition).unwrap(), None);
    assert_eq!(
        invocation.range(),
        invocation_range(text, "stop!(custom => grammar)")
    );
    assert!(matches!(
        entered
            .prepare_runtime_macro(
                view.selection(),
                body.body(),
                invocation_range(text, "absent!(custom => grammar)"),
                &names,
                &catalog
            )
            .unwrap(),
        RuntimeMacroPreparation::Resolution {
            lookup: PathLookup::NeedInput(_),
            ..
        }
    ));
    let missing = "fn case(){absent!(custom => grammar);}";
    let (_, missing_view, missing_body, _, _) = source(missing);
    let (missing_index, missing_names, missing_catalog) = macro_namespace(&missing_view);
    let missing_entered = missing_index
        .enter_body(missing_view.selection(), missing_body.body())
        .unwrap();
    assert!(matches!(
        missing_entered
            .prepare_runtime_macro(
                missing_view.selection(),
                missing_body.body(),
                invocation_range(missing, "absent!(custom => grammar)"),
                &missing_names,
                &missing_catalog
            )
            .unwrap(),
        RuntimeMacroPreparation::Resolution {
            lookup: PathLookup::Absent,
            ..
        }
    ));
}

#[test]
fn entered_runtime_preparation_rejects_stale_source_body_and_non_invocation_ranges() {
    let text = "use core::panic as stop; fn case(){stop!(\"owned\");} fn other(){}";
    let (_, view, body, _, _) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    let entered = index.enter_body(view.selection(), body.body()).unwrap();
    let range = invocation_range(text, "stop!(\"owned\")");
    let mut stale = view.selection().clone();
    stale.outline.parse.content = Digest([199; 32]);
    assert!(
        entered
            .prepare_runtime_macro(&stale, body.body(), range, &names, &catalog)
            .is_err()
    );
    let other = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "other")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    assert!(
        entered
            .prepare_runtime_macro(view.selection(), &other, range, &names, &catalog)
            .is_err()
    );
    assert!(
        entered
            .prepare_runtime_macro(
                view.selection(),
                body.body(),
                SourceRange {
                    start: range.start,
                    end: range.end - 1
                },
                &names,
                &catalog
            )
            .is_err()
    );
}

#[test]
fn reached_macro_context_executes_callee_alias_without_manual_plan_attachment() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
    };
    let text = "use core::panic as abort; fn leaf(){abort!(\"lazy callee\");} fn case(){leaf();}";
    let (_, view, body, _, inputs) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    assert_eq!(run.installed_recipes().len(), 0);
    let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
        RunPoll::Panicked(report) => report,
        RunPoll::Fault(error) => panic!("callee fault: {error:?}"),
        _ => panic!("callee alias did not panic"),
    };
    assert_eq!(report.panic.description(), "lazy callee");
    assert_eq!(report.opcode_counters.started_by_opcode[25], 1);
    assert_eq!(run.action_snapshot().published, 1);
    assert_eq!(run.action_snapshot().consumed, 1);
    db.finish_run().unwrap();
}
#[test]
fn reached_macro_context_keeps_cold_branch_and_uncalled_body_unprepared() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
    };
    let text = "use core::panic as abort; fn cold(){abort!(malformed => grammar);} fn case(){if false {abort!(malformed => grammar);} }";
    let (_, view, body, _, inputs) = source(text);
    let cold = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "cold")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Completed(report) = run.poll_with_runtime_macros(&mut db, &[context]) else {
        panic!("cold invalid arguments were forced")
    };
    assert_eq!(report.opcode_counters.started_by_opcode[25], 0);
    assert!(db.memo(&QueryKey::BodyStructure(cold)).is_none());
    assert!(run.installed_recipes().recipes().all(|r| {
        r.recipe()
            .operations
            .iter()
            .all(|op| !matches!(op, Op::Panic { .. }))
    }));
    db.finish_run().unwrap();
}
#[test]
fn runtime_macro_harness_accepts_actual_panic_and_rejects_user_expansion_fault() {
    use lrr::{
        db::memo::Database,
        engine::{
            runtime_macros::RuntimeMacroContext, test_catalog::TestCatalog,
            test_harness::run_original_with_runtime_macros,
        },
    };
    for (text, passes, code) in [
        (
            "use core::panic as abort; #[test] #[should_panic(expected=\"expected\")] fn case(){abort!(\"expected guest panic\");}",
            true,
            None,
        ),
        (
            "macro_rules! panic {($($t:tt)*)=>{};} #[test] #[should_panic] fn case(){panic!(custom => grammar);}",
            false,
            Some("LRR-E-CATALOG-INCOMPLETE"),
        ),
        (
            "#[test] #[should_panic] fn case(){absent!(custom => grammar);}",
            false,
            Some("LRR-E-MACRO-ABSENT"),
        ),
        (
            "use core::panic as stop; use core::assert as stop; #[test] #[should_panic] fn case(){stop!(custom => grammar);}",
            false,
            Some("LRR-E-MACRO-AMBIGUOUS"),
        ),
        (
            "mod hidden {use core::panic as secret;} #[test] #[should_panic] fn case(){hidden::secret!(custom => grammar);}",
            false,
            Some("LRR-E-MACRO-SCOPE"),
        ),
        (
            "use core::panic as stop; #[test] #[should_panic] fn case(){stop!(\"{unknown}\");}",
            false,
            Some("LRR-E-UNSUPPORTED"),
        ),
    ] {
        let (_, view, _, _, inputs) = source(text);
        let (index, names, catalog) = macro_namespace(&view);
        let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
        let selected = TestCatalog::original(&[view.share()]).unwrap();
        let cases = selected.select(Some("case"), true);
        assert_eq!(cases.len(), 1);
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        let result = run_original_with_runtime_macros(
            &mut db,
            &[view.share()],
            cases[0],
            false,
            1000,
            &[context],
        )
        .unwrap();
        assert_eq!(result.passed(), passes);
        assert_eq!(
            result.diagnostic.as_ref().map(|d| d.code),
            code,
            "{:?}",
            result.diagnostic
        );
    }
}
#[test]
fn runtime_macro_context_refuses_foreign_publication_and_duplicate_run_bindings() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
    };
    let text = "fn case(){}";
    let (_, view, body, _, inputs) = source(text);
    let (_, foreign, _, _, _) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    assert!(RuntimeMacroContext::new(&foreign, &index, &names, &catalog).is_err());
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let duplicate = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    assert!(matches!(
        run.poll_with_runtime_macros(&mut db, &[context, duplicate]),
        RunPoll::Fault(_)
    ));
    assert_eq!(run.fuel_snapshot().used, 0);
    assert_eq!(run.installed_recipes().len(), 0);
    assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
    db.finish_run().unwrap();
}

#[test]
fn literal_assertion_success_uses_vm_predicate_and_keeps_failure_message_cold() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll, ValueSummary},
            runtime_macros::RuntimeMacroContext,
        },
    };
    let text = "use core::assert as check; fn broken(){unresolved();} fn case(){check!(true, \"unvisited {}\", broken());}";
    let (_, view, body, nodes, inputs) = source(text);
    let broken = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "broken")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("assertion success fault: {error:?}"),
        _ => panic!("assertion did not complete"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.opcode_counters.started_by_opcode[0], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    assert!(db.memo(&QueryKey::BodyStructure(broken)).is_none());
    assert!(
        run.installed_recipes()
            .recipes()
            .all(|r| r.recipe().key.node != nodes[0].0
                || r.recipe().key.phase != Phase::Commit.code().unwrap())
    );
    let entry = run
        .installed_recipes()
        .recipes()
        .find(|r| {
            r.recipe().key.node == nodes[0].0
                && r.recipe().key.phase == Phase::Entry.code().unwrap()
        })
        .unwrap();
    let Op::Literal {
        literal,
        destination,
        ..
    } = &entry.recipe().operations[0]
    else {
        panic!("normal VM literal was not installed")
    };
    let handle = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "case")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    assert_eq!(
        &text[(handle.range.start + literal.source.relative_start) as usize
            ..(handle.range.start + literal.source.relative_end) as usize],
        "true"
    );
    assert_eq!(
        *destination,
        body.nodes()[nodes[0].0 as usize].runtime_temporaries[0].unwrap()
    );
    db.finish_run().unwrap();
}
#[test]
fn literal_assertion_failure_preserves_actual_source_message_origin_and_cleanup() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
        vm::numeric::PanicKind,
    };
    for (invocation, expected) in [
        ("check!(false)", "assertion failed: false"),
        ("check!(false, \"failure\\n 🦀\")", "failure\n 🦀"),
        ("check!(false, r#\"raw \\n\"#)", "raw \\n"),
        ("check!(false, \"\")", ""),
    ] {
        let text = format!("use core::assert as check; fn case(){{if true {{{invocation};}} }}");
        let (_, view, body, nodes, inputs) = source(&text);
        let (index, names, catalog) = macro_namespace(&view);
        let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) =
            RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
        else {
            panic!("run absent")
        };
        let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
            RunPoll::Panicked(report) => report,
            RunPoll::Fault(error) => panic!("assertion failure fault: {error:?}"),
            _ => panic!("assertion did not panic"),
        };
        assert_eq!(report.panic.kind, PanicKind::Assertion);
        assert_eq!(report.panic.description(), expected);
        assert_eq!(report.panic.origin.span, body.span(nodes[0]).unwrap());
        assert_eq!(report.opcode_counters.started_by_opcode[26], 1);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed + 1
        );
        // Function block, final if tail, condition, consequent block and assertion statement.
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 5);
        db.finish_run().unwrap();
    }
}
#[test]
fn literal_assertion_message_formatting_is_demanded_only_on_failure_and_faults_stay_faults() {
    use lrr::{
        db::memo::Database,
        engine::{
            runtime_macros::RuntimeMacroContext, test_catalog::TestCatalog,
            test_harness::run_original_with_runtime_macros,
        },
    };
    for (text, passes, code) in [
        (
            "use core::assert as check; #[test] fn case(){check!(true, \"{}\", unresolved());}",
            true,
            None,
        ),
        (
            "use core::assert as check; #[test] #[should_panic(expected=\"assertion failed\")] fn case(){check!(false);}",
            true,
            None,
        ),
        (
            "use core::assert as check; #[test] #[should_panic] fn case(){check!(false, \"{}\", unresolved());}",
            false,
            Some("LRR-E-UNSUPPORTED"),
        ),
        (
            "use core::assert as check; #[test] #[should_panic] fn case(){check!(missing());}",
            false,
            Some("LRR-E-NAME-MISSING"),
        ),
    ] {
        let (_, view, _, _, inputs) = source(text);
        let (index, names, catalog) = macro_namespace(&view);
        let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
        let tests = TestCatalog::original(&[view.share()]).unwrap();
        let cases = tests.select(Some("case"), true);
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        let result = run_original_with_runtime_macros(
            &mut db,
            &[view.share()],
            cases[0],
            false,
            1000,
            &[context],
        )
        .unwrap();
        assert_eq!(result.passed(), passes);
        assert_eq!(result.diagnostic.as_ref().map(|d| d.code), code);
    }
}
#[test]
fn runtime_macro_temporaries_are_deterministic_bounded_and_retire_in_repeated_scopes() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
        ir::pc::SlotKind,
        syntax::body_index::BodyIndexLimits,
    };
    let text = "use core::assert as check; fn case(){let mut n=0_i32; while n<3_i32 {check!(true, \"{}\", missing()); n=n+1_i32;}}";
    let (parsed, view, body, nodes, inputs) = source(text);
    let handle = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "case")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    assert_eq!(
        body,
        parsed
            .index_body(handle, BodyIndexLimits::default())
            .unwrap()
    );
    assert_eq!(
        body.fingerprint(),
        parsed
            .index_body(handle, BodyIndexLimits::default())
            .unwrap()
            .fingerprint()
    );
    let node = &body.nodes()[nodes[0].0 as usize];
    assert!(node.opaque && node.operands.is_empty());
    for slot in node.runtime_temporaries.into_iter().flatten() {
        assert_eq!(body.slot_kind(slot).unwrap(), SlotKind::Value);
        assert_ne!(Some(slot), node.value);
    }
    assert_eq!(
        parsed
            .index_body(
                handle,
                BodyIndexLimits {
                    slots: body.slot_count() - 1,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("repeated assertion fault: {error:?}"),
        _ => panic!("repeated assertion did not complete"),
    };
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    assert_eq!(report.opcode_counters.started_by_opcode[0], 11);
    assert_eq!(
        run.installed_recipes()
            .recipes()
            .filter(|r| r.recipe().key.node == nodes[0].0
                && r.recipe().key.phase == Phase::Entry.code().unwrap())
            .count(),
        1
    );
    db.finish_run().unwrap();
}

#[test]
fn literal_assertion_recipe_admission_refuses_wrong_source_body_phase_and_kind() {
    let text = "use core::assert as check; use core::panic as stop; fn case(){check!(true);} fn other(){stop!();}";
    let (_, view, body, nodes, _) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    let entered = index.enter_body(view.selection(), body.body()).unwrap();
    let RuntimeMacroPreparation::Intrinsic(plan) = entered
        .prepare_runtime_macro(
            view.selection(),
            body.body(),
            invocation_range(text, "check!(true)"),
            &names,
            &catalog,
        )
        .unwrap()
    else {
        panic!("assertion identity absent")
    };
    let key = body.pc(nodes[0], Phase::Entry).unwrap();
    let next = body.pc(NodeId(0), Phase::Exit).unwrap();
    assert!(matches!(
        plan.lower_assertion_recipe(&view, &body, &key, next.clone())
            .unwrap(),
        StaticPanicRecipeLowering::Recipe(_)
    ));
    let (_, stale, _, _, _) = source("fn case(){check!(false);}");
    assert!(
        plan.lower_assertion_recipe(&stale, &body, &key, next.clone())
            .is_err()
    );
    let mut wrong_body = key.clone();
    wrong_body.body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "other")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    assert!(
        plan.lower_assertion_recipe(&view, &body, &wrong_body, next.clone())
            .is_err()
    );
    let mut wrong_phase = key.clone();
    wrong_phase.phase = 3;
    assert!(
        plan.lower_assertion_recipe(&view, &body, &wrong_phase, next.clone())
            .is_err()
    );
    let ordinary = body.pc(NodeId(0), Phase::Entry).unwrap();
    assert!(
        plan.lower_assertion_recipe(&view, &body, &ordinary, next.clone())
            .is_err()
    );
    let other_key = wrong_body.body.clone();
    let other_entered = index.enter_body(view.selection(), &other_key).unwrap();
    let RuntimeMacroPreparation::Intrinsic(panic) = other_entered
        .prepare_runtime_macro(
            view.selection(),
            &other_key,
            invocation_range(text, "stop!()"),
            &names,
            &catalog,
        )
        .unwrap()
    else {
        panic!("panic identity absent")
    };
    assert!(
        panic
            .lower_assertion_recipe(&view, &body, &key, next)
            .is_err()
    );
}

#[test]
fn source_call_assertion_predicate_returns_to_scoped_slot_once_and_keeps_message_cold() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll, ValueSummary},
            runtime_macros::RuntimeMacroContext,
        },
    };
    let text = "use core::assert as check; fn predicate()->bool{true} fn broken(){missing();} fn case(){check!(predicate(), \"{}\", broken());}";
    let (_, view, body, nodes, inputs) = source(text);
    let broken = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "broken")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("predicate call fault: {error:?}"),
        _ => panic!("predicate call did not complete"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    assert_eq!(run.action_snapshot().published, 1);
    assert_eq!(run.action_snapshot().consumed, 1);
    assert_eq!(run.action_snapshot().pending, None);
    assert_eq!(run.peak_guest_frames(), 2);
    assert!(db.memo(&QueryKey::BodyStructure(broken)).is_none());
    let entry = run
        .installed_recipes()
        .recipes()
        .find(|r| {
            r.recipe().key.node == nodes[0].0
                && r.recipe().key.body == *body.body()
                && r.recipe().key.phase == 0
        })
        .unwrap();
    let RecipeExit::Call { result, resume, .. } = &entry.recipe().exit else {
        panic!("normal call exit absent")
    };
    assert_eq!(
        Some(*result),
        body.nodes()[nodes[0].0 as usize].runtime_temporaries[0]
    );
    assert_eq!(*resume, body.pc(nodes[0], Phase::Operand(0)).unwrap());
    assert!(
        run.installed_recipes()
            .recipes()
            .all(|r| r.recipe().key.body != *body.body()
                || r.recipe().key.node != nodes[0].0
                || r.recipe().key.phase != 1)
    );
    db.finish_run().unwrap();
}
#[test]
fn source_call_assertion_false_preserves_default_message_and_macro_panic_origin() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
        vm::numeric::PanicKind,
    };
    for (name, expected) in [
        ("predicate", "assertion failed: predicate()"),
        ("r#gen", "assertion failed: r#gen()"),
        ("e\u{301}", "assertion failed: é()"),
    ] {
        let text = format!(
            "use core::assert as check; fn {name}()->bool{{false}} fn case(){{check!({name}( /* comment */ ));}}"
        );
        let (_, view, body, nodes, inputs) = source(&text);
        let (index, names, catalog) = macro_namespace(&view);
        let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) =
            RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
        else {
            panic!("run absent")
        };
        let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
            RunPoll::Panicked(report) => report,
            RunPoll::Fault(error) => panic!("false predicate call fault: {error:?}"),
            _ => panic!("false predicate did not panic"),
        };
        assert_eq!(report.panic.kind, PanicKind::Assertion);
        assert_eq!(report.panic.description(), expected);
        assert_eq!(report.panic.origin.span, body.span(nodes[0]).unwrap());
        assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[26], 1);
        assert_eq!(run.action_snapshot().published, 1);
        assert_eq!(run.action_snapshot().consumed, 1);
        assert_eq!(run.peak_guest_frames(), 2);
        db.finish_run().unwrap();
    }
}
#[test]
fn assertion_predicate_call_cannot_fall_back_past_lexical_bindings_or_hoisted_functions() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
    };
    for code in [
        "let predicate=1_i32; check!(predicate());",
        "check!(predicate()); fn predicate()->bool{false}",
    ] {
        let text =
            format!("use core::assert as check; fn predicate()->bool{{true}} fn case(){{{code}}}");
        let (_, view, body, _, inputs) = source(&text);
        let predicate = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "predicate")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        let (index, names, catalog) = macro_namespace(&view);
        let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) =
            RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
        else {
            panic!("run absent")
        };
        let RunPoll::Fault(error) = run.poll_with_runtime_macros(&mut db, &[context]) else {
            panic!("lexical predicate fell back to module function")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert!(error.message.contains("lexical callable context"));
        assert_eq!(run.action_snapshot().published, 0);
        assert_eq!(run.action_snapshot().consumed, 0);
        assert!(db.memo(&QueryKey::BodyStructure(predicate)).is_none());
        db.finish_run().unwrap();
    }
}
#[test]
fn assertion_call_admission_requires_exact_temporary_boolean_hint_and_resume_phase() {
    let text =
        "use core::assert as check; fn predicate()->bool{true} fn case(){check!(predicate());}";
    let (_, view, body, nodes, _) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    let entered = index.enter_body(view.selection(), body.body()).unwrap();
    let RuntimeMacroPreparation::Intrinsic(plan) = entered
        .prepare_runtime_macro(
            view.selection(),
            body.body(),
            invocation_range(text, "check!(predicate())"),
            &names,
            &catalog,
        )
        .unwrap()
    else {
        panic!("assertion identity absent")
    };
    let key = body.pc(nodes[0], Phase::Entry).unwrap();
    let StaticPanicRecipeLowering::Recipe(recipe) = plan
        .lower_assertion_recipe(&view, &body, &key, body.pc(NodeId(0), Phase::Exit).unwrap())
        .unwrap()
    else {
        panic!("call recipe absent")
    };
    let valid = RecipeDraft {
        key: key.clone(),
        origin: recipe.recipe().origin.clone(),
        operations: recipe.recipe().operations.to_vec(),
        exits: vec![recipe.recipe().exit.clone()],
        dependencies: vec![],
    };
    assert!(verify(valid.clone(), &body).is_ok());
    let mut wrong = valid.clone();
    if let RecipeExit::Call { result, .. } = &mut wrong.exits[0] {
        *result = body.nodes()[nodes[0].0 as usize].value.unwrap();
    }
    assert!(verify(wrong, &body).is_err());
    let mut wrong = valid.clone();
    if let RecipeExit::Call { resume, .. } = &mut wrong.exits[0] {
        resume.phase = 2;
    }
    assert!(verify(wrong, &body).is_err());
    let mut wrong = valid.clone();
    wrong.operations.remove(0);
    assert!(verify(wrong, &body).is_err());
    let mut wrong = valid;
    if let Op::CallSeal { arguments, .. } = &mut wrong.operations[1] {
        arguments.push(body.nodes()[nodes[0].0 as usize].runtime_temporaries[1].unwrap());
    }
    assert!(verify(wrong, &body).is_err());
}

#[test]
fn assertion_predicate_return_mismatch_refuses_before_callee_body_or_guest_panic() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
    };
    let text = "use core::assert as check; fn predicate()->i32{1_i32/0_i32} fn case(){check!(predicate());}";
    let (_, view, body, _, inputs) = source(text);
    let predicate = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "predicate")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let error = match run.poll_with_runtime_macros(&mut db, &[context]) {
        RunPoll::Fault(error) => error,
        RunPoll::NeedInput(input) => panic!("non-bool predicate suspended: {input:?}"),
        RunPoll::Completed(report) => panic!("non-bool predicate completed: {report:?}"),
        RunPoll::Panicked(report) => panic!("non-bool predicate entered guest panic: {report:?}"),
    };
    assert_eq!(error.code, "LRR-E-CALL-RETURN-TYPE");
    assert!(db.memo(&QueryKey::BodyStructure(predicate)).is_none());
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[19], 0);
    assert_eq!(run.opcode_counters().unwrap().started_by_opcode[26], 0);
    assert_eq!(run.action_snapshot().published, 0);
    assert_eq!(run.action_snapshot().consumed, 0);
    db.finish_run().unwrap();
}
#[test]
fn assertion_predicate_callee_panic_unwinds_once_without_preparing_failure_message() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
        vm::numeric::PanicKind,
    };
    let text = "use core::assert as check; fn predicate()->bool{let n=1_i32/0_i32; true} fn case(){check!(predicate(), \"{}\", missing());}";
    let (_, view, body, nodes, inputs) = source(text);
    let predicate = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "predicate")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
        RunPoll::Panicked(report) => report,
        RunPoll::Fault(error) => panic!("predicate guest panic became fault: {error:?}"),
        _ => panic!("predicate did not panic"),
    };
    assert_eq!(report.panic.kind, PanicKind::DivisionByZero);
    assert_eq!(report.panic.origin.span.body_key, predicate);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 4);
    assert_eq!(run.action_snapshot().published, 1);
    assert_eq!(run.action_snapshot().consumed, 1);
    assert_eq!(run.action_snapshot().pending, None);
    assert!(
        run.installed_recipes()
            .recipes()
            .all(|r| r.recipe().key.body != *body.body()
                || r.recipe().key.node != nodes[0].0
                || r.recipe().key.phase == 0)
    );
    db.finish_run().unwrap();
}

#[test]
fn assertion_predicate_parameter_shadowing_preserves_caller_action_without_module_fallback() {
    use lrr::{
        db::{key::QueryKey, memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            runtime_macros::RuntimeMacroContext,
        },
    };
    let text = "use core::assert as check; fn predicate()->bool{true} fn helper(predicate:bool){check!(predicate());} fn case(){helper(true);}";
    let (_, view, body, _, inputs) = source(text);
    let predicate = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "predicate")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let (index, names, catalog) = macro_namespace(&view);
    let context = RuntimeMacroContext::new(&view, &index, &names, &catalog).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let RunPoll::Fault(error) = run.poll_with_runtime_macros(&mut db, &[context]) else {
        panic!("parameter fell back to module predicate")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert!(error.message.contains("lexical callable context"));
    assert_eq!(run.action_snapshot().published, 1);
    assert_eq!(run.action_snapshot().consumed, 1);
    assert!(db.memo(&QueryKey::BodyStructure(predicate)).is_none());
    db.finish_run().unwrap();
}

#[test]
fn no_item_certificate_retires_exact_original_producer_and_preserves_compiler_identity() {
    let text = "use core::assert as check; use core::panic as stop; fn case(){check!(true); stop!();} fn other(){}";
    let (_, view, body, _, _) = source(text);
    let (index, names, catalog) = macro_namespace(&view);
    let mut entered = index.enter_body(view.selection(), body.body()).unwrap();
    let later = invocation_range(text, "stop!()");
    assert!(matches!(
        entered
            .lookup_invocation(view.selection(), body.body(), later, &names)
            .unwrap(),
        PathLookup::NeedInput(_)
    ));
    let RuntimeMacroPreparation::Intrinsic(plan) = entered
        .prepare_runtime_macro(
            view.selection(),
            body.body(),
            invocation_range(text, "check!(true)"),
            &names,
            &catalog,
        )
        .unwrap()
    else {
        panic!("compiler assertion absent")
    };
    let proof = plan.no_item_proof(body.body()).unwrap();
    assert_eq!(proof.fingerprint(), proof.clone().fingerprint());
    let wrong_catalog = RuntimeMacroCatalog::new(Digest([197; 32]), 2024).unwrap();
    assert!(
        entered
            .accept_no_item_proof(view.selection(), body.body(), &proof, &wrong_catalog)
            .is_err()
    );
    let mut stale = view.selection().clone();
    stale.outline.parse.content = Digest([198; 32]);
    assert!(
        entered
            .accept_no_item_proof(&stale, body.body(), &proof, &catalog)
            .is_err()
    );
    let other = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "other")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    assert!(
        entered
            .accept_no_item_proof(view.selection(), &other, &proof, &catalog)
            .is_err()
    );
    entered
        .accept_no_item_proof(view.selection(), body.body(), &proof, &catalog)
        .unwrap();
    assert!(
        entered
            .accept_no_item_proof(view.selection(), body.body(), &proof, &catalog)
            .is_err()
    );
    let RuntimeMacroPreparation::Intrinsic(plan) = entered
        .prepare_runtime_macro(view.selection(), body.body(), later, &names, &catalog)
        .unwrap()
    else {
        panic!("later compiler macro stayed blocked")
    };
    assert_eq!(plan.arguments().kind(), RuntimeMacroKind::Panic);
}
#[test]
fn no_item_certificate_is_unavailable_for_transitive_argument_declarations_and_nested_macros() {
    for invocation in [
        "check!(true, \"{}\", unknown!())",
        "check!(true, \"{}\", {fn f(){} 1})",
        "check!(true, \"{}\", {impl Missing for Absent{} 1})",
        "check!(true, \"{}\", {#[producer] struct S; 1})",
    ] {
        let text = format!("use core::assert as check; fn case(){{{invocation};}}");
        let (_, view, body, _, _) = source(&text);
        let (index, names, catalog) = macro_namespace(&view);
        let entered = index.enter_body(view.selection(), body.body()).unwrap();
        let RuntimeMacroPreparation::Intrinsic(plan) = entered
            .prepare_runtime_macro(
                view.selection(),
                body.body(),
                invocation_range(&text, invocation),
                &names,
                &catalog,
            )
            .unwrap()
        else {
            panic!("compiler assertion absent")
        };
        assert!(plan.no_item_proof(body.body()).is_none());
    }
}
#[test]
fn sequential_runtime_certificates_preserve_actual_predicate_actions_and_panic_counts() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            standalone_macros::StandaloneRuntimeMacros,
        },
        vm::numeric::PanicKind,
    };
    let text = "fn predicate()->bool{true} fn case(){assert!(predicate(), \"{}\", missing()); assert!(true); panic!(\"last\");}";
    let (_, view, body, _, inputs) = source(text);
    let macros = StandaloneRuntimeMacros::new(&view).unwrap();
    let context = macros.context(&view).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, body.body().clone(), 1000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
        RunPoll::Panicked(report) => report,
        RunPoll::Fault(error) => panic!("sequential macro fault: {error:?}"),
        _ => panic!("last macro did not panic"),
    };
    assert_eq!(report.panic.kind, PanicKind::Explicit);
    assert_eq!(report.panic.description(), "last");
    assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[25], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    assert_eq!(run.action_snapshot().published, 1);
    assert_eq!(run.action_snapshot().consumed, 1);
    assert_eq!(run.action_snapshot().pending, None);
    db.finish_run().unwrap();
}

#[test]
fn binding_assertions_use_current_guest_cells_once_per_reach_and_normal_parameter_reads() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::{
            run::{RunMachine, RunPoll},
            standalone_macros::StandaloneRuntimeMacros,
        },
        vm::numeric::PanicKind,
    };
    for (text, local_reads, parameter_reads, calls) in [
        (
            "fn case(){let mut value=true; let mut n=0i32; while n<3i32{assert!(value);n=n+1i32;} value=false;assert!(value);}",
            11,
            0,
            0,
        ),
        (
            "fn verify(value:bool){assert!(value,\"parameter failed\");} fn case(){verify(false);}",
            0,
            1,
            1,
        ),
    ] {
        let (_, view, body, _, inputs) = source(text);
        let macros = StandaloneRuntimeMacros::new(&view).unwrap();
        let context = macros.context(&view).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut run)) = RunMachine::start_with_sources(
            &mut db,
            body.body().clone(),
            10_000,
            vec![view.share()],
        ) else {
            panic!("run absent")
        };
        let report = match run.poll_with_runtime_macros(&mut db, &[context]) {
            RunPoll::Panicked(report) => report,
            RunPoll::Fault(e) => panic!("binding fault: {e:?}"),
            _ => panic!("assertion absent"),
        };
        assert_eq!(report.panic.kind, PanicKind::Assertion);
        assert_eq!(report.opcode_counters.started_by_opcode[8], local_reads);
        assert_eq!(
            report.opcode_counters.started_by_opcode[21],
            parameter_reads
        );
        assert_eq!(report.opcode_counters.started_by_opcode[26], 1);
        assert_eq!(run.action_snapshot().published, calls);
        assert_eq!(run.action_snapshot().consumed, calls);
        assert_eq!(run.action_snapshot().pending, None);
        db.finish_run().unwrap();
    }
}
