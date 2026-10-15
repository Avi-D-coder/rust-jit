//! Dispatcher checks at the original nested-return frontier. The completed Err
//! result is an intrinsic scalar and completed Bar.x an intrinsic array. Original
//! Result/Bar/Foo construction, user Drop and whole compiletest remain unexercised.
use super::*;
use crate::{
    runtime::{
        cell::{CellId, CellState, TicketId},
        shape::{TypeId, TypeKey},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        scope_fixture_facts,
    },
    vm::frame::FrameStatus,
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/nested-return-drop-order.rs"
);
#[test]
fn rustc_original_return_nil_resumes_caller_once_after_callee_cleanup() {
    const SOURCE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/return/return-nil.rs");
    let (mut db, mut run) = borrows::tests::start_stock_body(SOURCE, "main");
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("unchanged stock local-unit return and caller did not complete")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert!(run.calls.is_empty());
    assert_eq!(run.call_bytes, 0);
    assert!(run.returning.is_none());
    assert!(run.frame.active_scopes().is_empty());
    assert!(report.opcode_counters.return_scopes_cleaned >= 2);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(run.peak_guest_frames >= 2);
    assert!(run.terminal);
    db.finish_run().unwrap();
    // Exact original main/f execute. Rust2024 profile adaptation still does
    // not constitute official compiletest acceptance.
}
#[test]
fn rustc_original_explicit_unit_return_keeps_unreachable_let_cold() {
    const SOURCE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/return/early-return-with-unreachable-code-24353.rs"
    );
    let (mut db, mut run) = borrows::tests::start_stock_body(SOURCE, "main");
    let range = scope_fixture_facts::original_first_let_body_range(SOURCE, "main");
    let cold: std::collections::BTreeSet<_> = run
        .frame
        .body
        .nodes()
        .iter()
        .filter(|node| node.range.start >= range.start && node.range.end <= range.end)
        .map(|node| node.id.0)
        .collect();
    assert!(!cold.is_empty());
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("unchanged stock unit return did not complete")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert!(run.frame.active_scopes().is_empty());
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::Thunk(pc) if pc.body == run.root_body => !cold.contains(&pc.node),
        _ => true,
    }));
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert!(report.opcode_counters.return_scopes_cleaned > 0);
    db.finish_run().unwrap();
}
#[test]
fn rustc_original_immediate_return_bodies_execute_with_other_bodies_cold() {
    for name in ["aa", "bb"] {
        let expected = scope_fixture_facts::original_single_return_literal(ORIGINAL, name);
        let (mut db, mut run) = borrows::tests::start_stock_body(ORIGINAL, name);
        let selected = run.root_body.clone();
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("unchanged original immediate-return body did not complete")
        };
        assert!(matches!(report.value, ValueSummary::Signed(value) if value == expected));
        assert!(run.returning.is_none());
        assert!(run.frame.active_scopes().is_empty());
        assert!(report.opcode_counters.return_scopes_cleaned > 0);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert!(db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => *body == selected,
            QueryKey::Thunk(pc) => pc.body == selected,
            _ => true,
        }));
        db.finish_run().unwrap();
    }
    // These two complete original function bodies do not exercise original main,
    // Result/Box/dyn/Drop/derive/output assertions or constitute a whole fixture.
}
struct Frontier {
    db: Database,
    run: RunMachine,
    ticket: TicketId,
    owner: SlotId,
    root: CellId,
    fields: Vec<CellId>,
    expected: i64,
}
fn frontier() -> Frontier {
    let (db, mut run) = borrows::tests::start_stock_body(ORIGINAL, "foo");
    let facts = scope_fixture_facts::original_nested_return(ORIGINAL);
    let body = run.frame.body.clone();
    let returned = body
        .nodes()
        .iter()
        .find(|node| {
            node.range == facts.returned
                && node.role == NodeRole::Expression(ExpressionKind::Return)
        })
        .unwrap();
    let scope = returned.scope;
    let pc = body.pc(returned.id, Phase::Operand(0)).unwrap();
    let owner = body
        .nodes()
        .iter()
        .find(|node| node.range == facts.prepared_owner)
        .unwrap()
        .value
        .unwrap();
    let handle = run.sources[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "foo")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let blob = crate::source::snapshot::SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
    let parsed = crate::syntax::parse::ParsedFile::parse(
        &blob,
        crate::core::profile::ProfileIdentity::current(),
    )
    .unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [Op::ResultTransfer { source, .. }] = recipe.recipe().operations.as_ref() else {
        panic!("original nested-return transfer absent")
    };
    let source = *source;
    // Prepare only reached storage, then execute the unchanged original Return
    // recipe. Source entry and constructor/call evaluation are not claimed.
    run.frame.pc = pc;
    run.frame.set_fuel_limit(10000).unwrap();
    let mut ancestry = Vec::new();
    let mut current = Some(scope);
    while let Some(scope) = current {
        ancestry.push(scope);
        current = body.scopes()[scope.0 as usize].parent;
    }
    for scope in ancestry.into_iter().rev() {
        run.frame.enter(scope).unwrap();
    }
    let expected = facts.returned_tag.parse::<i64>().unwrap();
    let ticket = run
        .store
        .ticket(ValueRepr::signed(TypeId(8), expected).unwrap())
        .unwrap();
    run.frame.write(source, ticket, &mut run.store).unwrap();
    let ty = run
        .store
        .intern_type(TypeKey::Array {
            element: TypeId(8),
            len: 257,
        })
        .unwrap();
    let guard = run.store.begin_construction(&vec![TypeId(8); 257]).unwrap();
    for field in 0..257 {
        let ticket = run
            .store
            .ticket(ValueRepr::signed(TypeId(8), facts.field_tag).unwrap())
            .unwrap();
        run.store.construction_push(guard, field, ticket).unwrap();
    }
    let ticket = run.store.commit_construction(guard, ty).unwrap();
    let fields = (0..257)
        .map(|field| run.store.aggregate_field(ticket, field).unwrap())
        .collect();
    run.frame.write(owner, ticket, &mut run.store).unwrap();
    let root = run.frame.cell(owner).unwrap();
    let StepOutcome::Return(ticket) = step::step(&mut run.frame, &mut run.store, &recipe).unwrap()
    else {
        panic!("original nested return did not return")
    };
    Frontier {
        db,
        run,
        ticket,
        owner,
        root,
        fields,
        expected,
    }
}
#[test]
fn rustc_nested_return_dispatcher_publishes_root_only_after_bounded_cleanup() {
    let mut f = frontier();
    let pc = f.run.frame.pc().clone();
    let before = f.run.frame.counters().clone();
    let value = f.run.store.peek_ticket(f.ticket).unwrap();
    f.run.begin_return_cleanup(&mut f.db, f.ticket).unwrap();
    assert!(f.run.advance_return_cleanup().unwrap().is_none());
    assert!(f.run.returning.is_some());
    assert!(!f.run.terminal);
    assert!(f.run.terminal_facts().is_none());
    assert_eq!(f.run.frame.status, FrameStatus::Returned);
    assert_eq!(f.run.frame.pc(), &pc);
    assert_eq!(f.run.frame.cell(f.owner).unwrap(), f.root);
    assert_eq!(f.run.store.peek_ticket(f.ticket).unwrap(), value);
    let RunPoll::Completed(report) = f.run.poll(&mut f.db) else {
        panic!("completed dispatcher cleanup did not publish its root result")
    };
    assert!(
        matches!(report.value, ValueSummary::Signed(ref value) if *value == f.expected.to_string())
    );
    assert!(f.run.returning.is_none());
    assert!(f.run.terminal);
    assert!(f.run.frame.active_scopes().is_empty());
    assert_eq!(
        report.opcode_counters.operations_started,
        before.operations_started
    );
    assert_eq!(
        report.opcode_counters.operations_completed,
        before.operations_completed
    );
    assert_eq!(
        report.opcode_counters.exits_completed,
        before.exits_completed
    );
    assert!(report.opcode_counters.return_scopes_cleaned > 0);
    assert_eq!(f.run.store.state(f.root).unwrap(), CellState::Dropped);
    assert!(
        f.fields
            .iter()
            .all(|field| f.run.store.state(*field).unwrap() == CellState::Dropped)
    );
    assert_eq!(
        f.run.store.peek_ticket(f.ticket).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    assert!(
        matches!(f.run.poll(&mut f.db), RunPoll::Fault(error) if error.code == "LRR-E-INTERNAL-IR")
    );
    f.db.finish_run().unwrap();
}
#[test]
fn rustc_nested_return_dispatcher_cancel_preserves_pending_ticket_without_publication() {
    let mut f = frontier();
    f.run.begin_return_cleanup(&mut f.db, f.ticket).unwrap();
    assert!(f.run.advance_return_cleanup().unwrap().is_none());
    let value = f.run.store.peek_ticket(f.ticket).unwrap();
    let scopes = f.run.frame.active_scopes().to_vec();
    let states: Vec<_> = f
        .fields
        .iter()
        .map(|field| f.run.store.state(*field).unwrap())
        .collect();
    let used = f.run.frame.fuel_snapshot().used;
    f.run.frame.fuel.cancellation().cancel();
    assert!(
        matches!(f.run.poll(&mut f.db), RunPoll::Fault(error) if error.code == "LRR-E-RUNTIME-RESOURCE")
    );
    assert!(f.run.terminal);
    assert_eq!(f.run.frame.status, FrameStatus::Faulted);
    assert!(f.run.returning.is_some());
    assert!(f.run.frame.has_cleanup_activation());
    assert_eq!(f.run.store.peek_ticket(f.ticket).unwrap(), value);
    assert_eq!(f.run.frame.active_scopes(), scopes.as_slice());
    assert_eq!(f.run.frame.fuel_snapshot().used, used);
    assert_eq!(f.run.frame.cell(f.owner).unwrap(), f.root);
    assert_eq!(
        f.fields
            .iter()
            .map(|field| f.run.store.state(*field).unwrap())
            .collect::<Vec<_>>(),
        states
    );
    assert!(matches!(
        f.run.terminal_facts().unwrap().outcome,
        TerminalExecution::Fault {
            code: "LRR-E-RUNTIME-RESOURCE"
        }
    ));
    f.db.finish_run().unwrap();
}

/// Prepared frontier of the unchanged original main -> f call. Both selected
/// source contracts and the once-consumed CallAction come from normal machinery.
/// Stop at the original Return before dispatcher cleanup, so admission can be
/// inspected without adding a test-only yield to production polling.
pub(super) fn original_caller_return_frontier() -> (Database, RunMachine, TicketId) {
    const SOURCE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/return/return-nil.rs");
    let (mut db, mut run) = borrows::tests::start_stock_body(SOURCE, "main");
    for _ in 0..64 {
        let outcome = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(run.frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("original call thunk absent")
        };
        for op in &recipe.recipe().operations {
            match op {
                Op::ConstructStart { .. } => {
                    // The original callee's `let x = ();` is a reached zero-field
                    // constructor. Supply normal dispatcher facts before step.
                    run.prepare_constructor(&mut db, &[])
                        .into_ready()
                        .unwrap()
                        .unwrap();
                }
                Op::CallPrepare {
                    site,
                    name,
                    explicit,
                    arguments,
                } => {
                    assert!(arguments.is_empty());
                    let selected = run
                        .select_argument_call(&mut db, name, explicit, arguments)
                        .into_ready()
                        .unwrap()
                        .unwrap();
                    let ordinary = selected
                        .callee
                        .ordinary_call_witness(
                            &mut db,
                            run.frame.body.body().clone(),
                            NodeId(site.0),
                        )
                        .unwrap();
                    run.frame.call_start = Some(crate::vm::call::CallStartFacts {
                        ordinary,
                        site: site.0,
                        hints: vec![],
                    });
                }
                Op::CallSeal {
                    name,
                    explicit,
                    arguments,
                    ..
                } => {
                    assert!(arguments.is_empty());
                    run.prepared_call = Some(
                        run.prepare_source_call(&mut db, name, None, &[], None, explicit)
                            .into_ready()
                            .unwrap()
                            .unwrap(),
                    );
                    run.frame.call_types.clear();
                    run.frame.call_cells.clear();
                    run.frame.call_ready = true;
                }
                _ => {}
            }
        }
        match step::step(&mut run.frame, &mut run.store, recipe).unwrap() {
            StepOutcome::Call {
                action,
                site,
                resume,
                result,
            } => {
                assert!(run.calls.is_empty());
                let consumed = action
                    .consume(&run.frame.actions, run.frame.pc(), site, &resume, result)
                    .unwrap();
                let mut call = run.prepared_call.take().unwrap();
                call.callee.revalidate(&mut db).unwrap();
                assert!(call.contracts.parameter_descriptors.is_empty());
                let callee_lease = call.body_lease.take().unwrap();
                let mut callee = Frame::new_in_run(
                    call.body.clone(),
                    call.body.pc(NodeId(0), Phase::Entry).unwrap(),
                    &mut run.store,
                    &mut run.frame,
                )
                .unwrap();
                let packet = run.frame.call_arguments.take().unwrap();
                callee
                    .install_context(call.context.take().unwrap())
                    .unwrap();
                callee
                    .install_parameter_sources(call.contracts.source_parameters.clone())
                    .unwrap();
                callee
                    .initialize_parameters(&packet, &[], &mut run.store)
                    .unwrap();
                run.collected.accumulate(run.frame.counters()).unwrap();
                run.frame.counters = OpcodeCounters::default();
                let caller = std::mem::replace(&mut run.frame, callee);
                let body_lease = std::mem::replace(&mut run.body_lease, callee_lease);
                run.call_bytes = call.retained_bytes;
                run.peak_guest_frames = 2;
                run.calls.push(SuspendedCall {
                    action: consumed.id,
                    origin: consumed.origin,
                    caller,
                    body_lease,
                    resume: consumed.resume,
                    result: consumed.result,
                    call,
                    pending: Default::default(),
                });
            }
            StepOutcome::Return(ticket) => {
                assert_eq!(run.calls.len(), 1);
                assert_eq!(run.frame.status, FrameStatus::Returned);
                assert_eq!(run.store.peek_ticket(ticket).unwrap(), ValueRepr::unit());
                assert!(run.returning.is_none());
                return (db, run, ticket);
            }
            StepOutcome::Next(_) => {}
            other => panic!("unexpected original call/return frontier: {other:?}"),
        }
    }
    panic!("original caller return frontier did not finish within its bounded recipe count")
}
#[test]
fn rustc_original_caller_return_destination_refuses_before_cleanup_or_owner_consumption() {
    let (mut db, mut run, ticket) = original_caller_return_frontier();
    let scopes = run.frame.active_scopes().to_vec();
    let before = (
        run.frame.pc().clone(),
        run.frame.fuel_snapshot().used,
        run.store.budget().handles(),
        run.store.peek_ticket(ticket).unwrap(),
    );
    let suspended = run.calls.last_mut().unwrap();
    suspended
        .caller
        .declared_hint(suspended.result, Some(TypeId(2)))
        .unwrap();
    assert!(suspended.caller.cell(suspended.result).is_err());
    assert_eq!(
        run.begin_return_cleanup(&mut db, ticket).unwrap_err().code,
        "LRR-E-OWNERSHIP"
    );
    assert!(run.returning.is_none());
    assert!(!run.frame.has_cleanup_activation());
    assert_eq!(run.frame.status, FrameStatus::Returned);
    assert_eq!(run.frame.active_scopes(), scopes.as_slice());
    assert_eq!(
        (
            run.frame.pc().clone(),
            run.frame.fuel_snapshot().used,
            run.store.budget().handles(),
            run.store.peek_ticket(ticket).unwrap()
        ),
        before
    );
    assert_eq!(run.frame.counters.return_scopes_cleaned, 0);
    assert_eq!(run.calls.len(), 1);
    assert!(!run.terminal);
    let suspended = run.calls.last().unwrap();
    assert_eq!(suspended.caller.status, FrameStatus::Suspended);
    assert!(suspended.caller.cell(suspended.result).is_err());
    db.finish_run().unwrap();
    // Pure admission refusal is inspected directly; no rejected run is resumed
    // or counted as a stock success. Full compiletest acceptance stays separate.
}
#[test]
fn rustc_original_caller_return_reserves_destination_before_cleanup_and_resumes_once() {
    let (mut db, mut run, ticket) = original_caller_return_frontier();
    let value = run.store.peek_ticket(ticket).unwrap();
    let before = run.store.budget().handles();
    let suspended = run.calls.last().unwrap();
    let (destination, resume) = (suspended.result, suspended.resume.clone());
    assert!(suspended.caller.cell(destination).is_err());
    run.begin_return_cleanup(&mut db, ticket).unwrap();
    let caller = &run.calls.last().unwrap().caller;
    let reserved = caller.cell(destination).unwrap();
    assert_eq!(caller.status, FrameStatus::Suspended);
    assert_eq!(run.store.state(reserved).unwrap(), CellState::Uninit);
    assert_eq!(run.store.budget().handles(), before.checked_add(1).unwrap());
    assert_eq!(run.store.peek_ticket(ticket).unwrap(), value);
    assert_eq!(run.frame.counters.return_scopes_cleaned, 0);
    assert!(run.returning.is_some());
    assert!(run.advance_return_cleanup().unwrap().is_none());
    assert!(run.calls.is_empty());
    assert!(run.returning.is_none());
    assert_eq!(run.call_bytes, 0);
    assert_eq!(run.frame.status, FrameStatus::Running);
    assert_eq!(run.frame.pc(), &resume);
    assert_eq!(run.frame.cell(destination).unwrap(), reserved);
    assert_eq!(run.store.state(reserved).unwrap(), CellState::Live(value));
    assert_eq!(
        run.store.peek_ticket(ticket).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("original caller did not complete after its single resumption")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert!(run.calls.is_empty() && run.returning.is_none());
    db.finish_run().unwrap();
    // Original source thunks/contracts execute through a prepared frontier;
    // harness setup is disclosed and is not full compiletest acceptance.
}

#[test]
fn rustc_original_caller_return_rejects_other_canonical_resume_and_result_before_effects() {
    let (mut db, mut run, ticket) = original_caller_return_frontier();
    let suspended = run.calls.last_mut().unwrap();
    let resume = suspended.resume.clone();
    let result = suspended.result;
    let other_resume = suspended.caller.body.pc(NodeId(0), Phase::Entry).unwrap();
    assert_ne!(other_resume, resume);
    let other_result = suspended
        .caller
        .body
        .nodes()
        .iter()
        .filter_map(|node| node.value)
        .find(|slot| *slot != result)
        .expect("original caller has a distinct value destination");
    let before = (
        suspended.caller.pc().clone(),
        suspended.caller.fuel_snapshot().used,
        serde_json::to_value(&suspended.caller.counters).unwrap(),
        run.store.budget().handles(),
        run.store.peek_ticket(ticket).unwrap(),
    );
    for (candidate_resume, candidate_result) in [(&other_resume, result), (&resume, other_result)] {
        assert_eq!(
            suspended
                .caller
                .reserve_resume_call(candidate_resume, candidate_result, ticket, &mut run.store)
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(suspended.caller.status, FrameStatus::Suspended);
        assert!(suspended.caller.cell(result).is_err());
        assert_eq!(
            (
                suspended.caller.pc().clone(),
                suspended.caller.fuel_snapshot().used,
                serde_json::to_value(&suspended.caller.counters).unwrap(),
                run.store.budget().handles(),
                run.store.peek_ticket(ticket).unwrap(),
            ),
            before
        );
    }
    assert!(run.returning.is_none());
    assert!(!run.frame.has_cleanup_activation());
    assert_eq!(run.frame.counters.return_scopes_cleaned, 0);
    // The legitimate, unchanged original continuation still reserves and resumes.
    run.begin_return_cleanup(&mut db, ticket).unwrap();
    assert!(run.advance_return_cleanup().unwrap().is_none());
    let RunPoll::Completed(report) = run.poll(&mut db) else {
        panic!("original caller did not complete after pure forged-continuation refusals")
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    db.finish_run().unwrap();
}

#[test]
fn rustc_original_concrete_return_rejects_rigid_source_before_cleanup_or_publication() {
    use crate::traits::term::{ParamKey, TyTerm};
    let (mut db, mut run, ticket) = original_caller_return_frontier();
    let scopes = run.frame.active_scopes().to_vec();
    let before = (
        run.frame.pc().clone(),
        run.frame.fuel_snapshot().used,
        run.store.budget().handles(),
        run.store.peek_ticket(ticket).unwrap(),
        run.call_bytes,
    );
    // Corrupt only the LRR return packet's retained source view after the
    // unchanged original main -> f call and Return have executed. Unit's
    // concrete descriptor is still valid; it is not proof of this rigid type.
    run.frame.return_source = Some(TyTerm::Param(ParamKey {
        definition: run.frame.body.body().definition,
        ordinal: 0,
    }));
    let caller = run.calls.last().unwrap();
    let destination = caller.result;
    let caller_source = caller.caller.source_view(destination).unwrap();
    assert_eq!(
        run.begin_return_cleanup(&mut db, ticket).unwrap_err().code,
        "LRR-E-GENERIC-BINDING"
    );
    assert!(run.returning.is_none());
    assert_eq!(run.calls.len(), 1);
    assert_eq!(run.frame.active_scopes(), scopes.as_slice());
    assert_eq!(
        (
            run.frame.pc().clone(),
            run.frame.fuel_snapshot().used,
            run.store.budget().handles(),
            run.store.peek_ticket(ticket).unwrap(),
            run.call_bytes,
        ),
        before
    );
    assert_eq!(
        run.calls
            .last()
            .unwrap()
            .caller
            .source_view(destination)
            .unwrap(),
        caller_source
    );
    run.frame.return_source = Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit));
    run.begin_return_cleanup(&mut db, ticket).unwrap();
    let report = loop {
        if let Some(report) = run.advance_return_cleanup().unwrap() {
            break report;
        }
        if run.returning.is_none() {
            match run.poll(&mut db) {
                RunPoll::Completed(report) => break report,
                _ => panic!("original caller did not resume after accepted return"),
            }
        }
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert!(run.calls.is_empty());
    assert_eq!(run.call_bytes, 0);
    db.finish_run().unwrap();
}

const NAMED_RETURN_STOCK: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-temporary.rs"
);

fn named_return_stock(edition: u16) -> (Database, RunMachine, crate::traits::term::ParamKey) {
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(NAMED_RETURN_STOCK.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("src/original.rs").unwrap();
    let unit = Digest::of_bytes(b"LRR stock named return world");
    let selection = SourceSelection {
        path: path.clone(),
        outline: crate::db::key::OutlineKey {
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
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("original named return source missing")
    };
    let definition = |name: &str| {
        view.outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .unwrap()
    };
    let rigid = crate::traits::term::ParamKey {
        definition: definition("apply").def_key.fingerprint(),
        ordinal: 0,
    };
    let body = definition("mk").body.as_ref().unwrap().key();
    db.begin_run().unwrap();
    let run = RunMachine::start_with_sources(&mut db, body, 1000, vec![view.share()])
        .into_ready()
        .unwrap()
        .unwrap();
    (db, run, rigid)
}

#[test]
fn original_named_primitive_result_context_preserves_rigid_source_identity() {
    use crate::traits::term::TyTerm;
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, run, rigid) = named_return_stock(edition);
        let wrong = ArgumentType {
            source: TyTerm::Param(rigid),
            descriptor: TypeId(8),
        };
        let before = run.frame.pc().clone();
        // Select the unchanged mk header through the real source-call resolver.
        // This LRR result packet is corrupted; original main's fn-pointer call
        // is unexecuted and this control is not a full stock fixture pass.
        match run.prepare_source_call(&mut db, "mk", None, &[], Some(&wrong), &[]) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-GENERIC-BINDING"),
            _ => {
                panic!("named primitive return admitted rigid result identity at equal descriptor")
            }
        }
        assert_eq!(run.frame.pc(), &before);
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(event.key, QueryKey::Thunk(_)))
        );
        let correct = ArgumentType {
            source: TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize),
            descriptor: TypeId(8),
        };
        let selected = run
            .prepare_source_call(&mut db, "mk", None, &[], Some(&correct), &[])
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(selected.callee.signature().header.profile.edition, edition);
        assert_eq!(
            selected.contracts.source_result,
            Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize))
        );
        assert_eq!(selected.contracts.result_descriptor, Some(TypeId(8)));
        assert!(selected.contracts.normalized_deferred_return.is_some());
        db.finish_run().unwrap();
    }
}

#[test]
fn original_named_primitive_return_refuses_rigid_view_before_selected_source_admission() {
    use crate::traits::term::TyTerm;
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run, rigid) = named_return_stock(edition);
        let selected = run
            .prepare_source_call(&mut db, "mk", None, &[], None, &[])
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(selected.contracts.result_descriptor, Some(TypeId(8)));
        assert_eq!(
            selected.contracts.source_result,
            Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize))
        );
        assert!(selected.contracts.normalized_deferred_return.is_some());
        let before = (
            run.frame.pc().clone(),
            run.frame.fuel_snapshot().used,
            run.call_bytes,
        );
        // Corrupt only the retained return view at the descriptor-check seam.
        // No guest Return or cleanup has executed during this admission control.
        run.frame.return_source = Some(TyTerm::Param(rigid));
        assert_eq!(
            run.check_source_return(&mut db, &selected, TypeId(8))
                .unwrap_err()
                .code,
            "LRR-E-GENERIC-BINDING"
        );
        assert_eq!(
            (
                run.frame.pc().clone(),
                run.frame.fuel_snapshot().used,
                run.call_bytes
            ),
            before
        );
        assert!(run.returning.is_none());
        assert!(run.calls.is_empty());
        run.frame.return_source = Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize));
        run.check_source_return(&mut db, &selected, TypeId(8))
            .unwrap();
        assert_eq!(
            run.check_source_return(&mut db, &selected, TypeId(7))
                .unwrap_err()
                .code,
            "LRR-E-CALL-RETURN-TYPE"
        );
        // Separately execute the complete original mk body in this owning
        // edition. Original main/apply/chk remain unexecuted.
        let expected = crate::syntax::scope_fixture_facts::original_single_return_literal(
            NAMED_RETURN_STOCK,
            "mk",
        );
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("original mk body did not complete")
        };
        assert!(matches!(report.value, ValueSummary::Signed(value) if value == expected));
        assert_eq!(report.live_edition, edition);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        db.finish_run().unwrap();
    }
}

const ALIAS_RETURN_STOCK: &[(&str, &str, crate::ir::op::PrimitiveType, u32)] = &[
    (
        include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/union/union-const-eval-field.rs"
        ),
        "read_field2",
        crate::ir::op::PrimitiveType::F32,
        14,
    ),
    (
        include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/use-redundant/use-redundant.rs"
        ),
        "baz",
        crate::ir::op::PrimitiveType::I32,
        6,
    ),
];

fn alias_return_stock(text: &str, function: &str, edition: u16) -> (Database, RunMachine) {
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("src/original-alias.rs").unwrap();
    let unit = Digest::of_bytes(b"LRR original alias return world");
    let selection = SourceSelection {
        path: path.clone(),
        outline: crate::db::key::OutlineKey {
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
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("original alias source not selected")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let run = RunMachine::start_with_sources(&mut db, body, 1000, vec![view.share()])
        .into_ready()
        .unwrap()
        .unwrap();
    (db, run)
}

#[test]
fn original_alias_return_contracts_normalize_in_owning_source_context() {
    use crate::traits::term::TyTerm;
    for &(text, function, primitive, descriptor) in ALIAS_RETURN_STOCK {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, run) = alias_return_stock(text, function, edition);
            let actual = ArgumentType {
                source: TyTerm::Primitive(primitive),
                descriptor: TypeId(descriptor),
            };
            let mut selected = run
                .prepare_source_call(&mut db, function, None, &[], Some(&actual), &[])
                .into_ready()
                .unwrap()
                .unwrap();
            assert_eq!(selected.callee.signature().header.profile.edition, edition);
            assert_eq!(
                selected.contracts.source_result,
                Some(TyTerm::Primitive(primitive))
            );
            assert_eq!(
                selected.contracts.result_descriptor,
                Some(TypeId(descriptor))
            );
            selected.contracts.bind_return_parameter(&actual).unwrap();
            assert!(
                db.trace
                    .events
                    .iter()
                    .all(|event| !matches!(event.key, QueryKey::Thunk(_)))
            );
            db.finish_run().unwrap();
        }
    }
    // Only selected original headers and alias declarations are admitted.
    // The separate body control records the exact unsupported outcome;
    // neither complete upstream fixture is claimed passed.
}

#[test]
fn original_alias_return_contracts_preserve_source_identity_at_equal_descriptor() {
    use crate::traits::term::TyTerm;
    for &(text, function, primitive, descriptor) in ALIAS_RETURN_STOCK {
        for edition in [2015, 2018, 2021, 2024] {
            let (mut binder_db, _, rigid) = named_return_stock(edition);
            binder_db.finish_run().unwrap();
            let (mut db, run) = alias_return_stock(text, function, edition);
            let wrong = ArgumentType {
                source: TyTerm::Param(rigid),
                descriptor: TypeId(descriptor),
            };
            let before = (
                run.frame.pc().clone(),
                run.frame.fuel_snapshot().used,
                run.call_bytes,
            );
            match run.prepare_source_call(&mut db, function, None, &[], Some(&wrong), &[]) {
                QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-GENERIC-BINDING"),
                _ => panic!("resolved alias admitted foreign rigid result source"),
            }
            assert_eq!(
                (
                    run.frame.pc().clone(),
                    run.frame.fuel_snapshot().used,
                    run.call_bytes
                ),
                before
            );
            let mut selected = run
                .prepare_source_call(&mut db, function, None, &[], None, &[])
                .into_ready()
                .unwrap()
                .unwrap();
            assert_eq!(
                selected
                    .contracts
                    .bind_return_parameter(&wrong)
                    .unwrap_err()
                    .code,
                "LRR-E-GENERIC-BINDING"
            );
            let correct = ArgumentType {
                source: TyTerm::Primitive(primitive),
                descriptor: TypeId(descriptor),
            };
            selected.contracts.bind_return_parameter(&correct).unwrap();
            let source_result = selected.contracts.source_result.clone();
            selected.contracts.source_result = Some(TyTerm::Param(rigid));
            assert_eq!(
                crate::vm::context::GenericEnv::capture_current(
                    &mut db,
                    &selected.callee,
                    &selected.contracts,
                )
                .unwrap_err()
                .code,
                "LRR-E-CALL-CONTEXT"
            );
            selected.contracts.source_result = None;
            assert_eq!(
                crate::vm::context::GenericEnv::capture_current(
                    &mut db,
                    &selected.callee,
                    &selected.contracts,
                )
                .unwrap_err()
                .code,
                "LRR-E-CALL-CONTEXT"
            );
            selected.contracts.source_result = source_result;
            selected.contracts.result_descriptor = Some(TypeId(1));
            assert_eq!(
                crate::vm::context::GenericEnv::capture_current(
                    &mut db,
                    &selected.callee,
                    &selected.contracts,
                )
                .unwrap_err()
                .code,
                "LRR-E-CALL-CONTEXT"
            );
            selected.contracts.result_descriptor = Some(TypeId(descriptor));
            let env = crate::vm::context::GenericEnv::capture_current(
                &mut db,
                &selected.callee,
                &selected.contracts,
            )
            .unwrap();
            assert_eq!(
                env.signature_key(),
                selected.callee.signature().fingerprint()
            );
            assert_eq!(env.definition(), selected.callee.signature());
            assert_eq!(env.definition().result_symbolic, None);
            assert!(
                db.trace
                    .events
                    .iter()
                    .all(|event| !matches!(event.key, QueryKey::Thunk(_)))
            );
            db.finish_run().unwrap();
        }
    }
}

#[test]
fn original_alias_return_baz_body_reports_symbolic_hint_frontier() {
    const SOURCE: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/use-redundant/use-redundant.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run) = alias_return_stock(SOURCE, "baz", edition);
        let selected = run
            .prepare_source_call(&mut db, "baz", None, &[], None, &[])
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(
            selected.contracts.source_result,
            Some(crate::traits::term::TyTerm::Primitive(
                crate::ir::op::PrimitiveType::I32
            ))
        );
        let literal_nodes: std::collections::BTreeSet<_> = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Literal))
            .map(|node| node.id.0)
            .collect();
        assert!(!literal_nodes.is_empty());
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("original alias body did not expose its recorded hint frontier")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert_eq!(error.message, "return type requires symbolic resolution");
        assert!(db.trace.events.iter().all(|event| !matches!(&event.key,
            QueryKey::Thunk(pc) if pc.body == run.root_body && literal_nodes.contains(&pc.node))));
        assert!(run.calls.is_empty() && run.returning.is_none());
        db.finish_run().unwrap();
    }
    // Actual unchanged body-entry attempts are unsupported in all four
    // editions, before its literal executes. This typed frontier regression
    // is not a body/fixture success and does not exercise original main.
}
