//! Actual normal opcode/run/continuation supplements around pristine upstream
//! declarations. Probe bodies are disclosed LRR adapters, not full-fixture passes.
use super::*;
use crate::{
    frontend::constant_construction::tests::world, ir::op::SOURCE_CONSTANT_OPCODE,
    runtime::cell::CellState,
};
const ARRAY: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
);
const SCALAR: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-nested.rs");
fn start(text: &str, name: &str, entry: &str, edition: u16, fuel: u64) -> (Database, RunMachine) {
    let (mut db, view, _) = world(text, name, edition);
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == entry)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let run = RunMachine::start_with_sources(&mut db, body, fuel, vec![view.share()])
        .into_ready()
        .unwrap()
        .unwrap();
    (db, run)
}
fn completed(db: &mut Database, run: &mut RunMachine) -> RunReport {
    match run.poll(db) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(e) => panic!("actual source dispatch failed {}: {}", e.code, e.message),
        RunPoll::NeedInput(i) => panic!("actual source dispatch still requires {i:?}"),
        RunPoll::Panicked(p) => panic!("actual source dispatch panicked {}", p.panic.description()),
    }
}
fn reach_constant(db: &mut Database, run: &mut RunMachine) {
    for _ in 0..256 {
        let pc = run.frame.pc().clone();
        if pc.phase == Phase::Entry.code().unwrap()
            && run.frame.body.nodes()[pc.node as usize].role
                == crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Path,
                )
        {
            return;
        }
        let id = run.installed.resolve(db, pc).into_ready().unwrap().unwrap();
        let outcome = step::step(
            &mut run.frame,
            &mut run.store,
            run.installed.get(id).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, StepOutcome::Next(_)));
    }
    panic!("source adapter did not reach its actual constant path")
}
fn one_constant_chunk(
    db: &mut Database,
    run: &mut RunMachine,
) -> (StepOutcome, crate::runtime::cell::OwnerInventory) {
    let pc = run.frame.pc().clone();
    let recipe = run
        .prepare_source_constant_recipe(db)
        .into_ready()
        .unwrap()
        .unwrap()
        .unwrap();
    let id = run
        .installed
        .install_prepared(db, pc.clone(), &recipe)
        .unwrap();
    let plan = run.constant_plans.get(&pc).unwrap();
    let construction = plan.construction(db, &run.sources).unwrap();
    let views = run.sources.iter().collect::<Vec<_>>();
    let proof = plan
        .proof(
            &construction,
            &run.frame,
            &mut run.store,
            &run.sources,
            &views,
        )
        .unwrap();
    let mut baseline = run.store.owner_inventory_for_test();
    let destination = run.frame.body.nodes()[pc.node as usize].value.unwrap();
    let prior = run.frame.cell(destination).ok();
    let outcome = step::step_with_constant(
        &mut run.frame,
        &mut run.store,
        run.installed.get(id).unwrap(),
        Some(&proof),
    )
    .unwrap();
    if prior.is_none() {
        baseline.cells.push((
            run.frame.cell(destination).unwrap().bits(),
            CellState::Uninit,
        ));
        baseline.cells.sort_by_key(|cell| cell.0);
    }
    (outcome, baseline)
}
#[test]
fn original_nested_const_fn_scalar_runs_normal_opcode_and_return_all_editions() {
    let text = format!("{SCALAR}\nfn constant_probe()->u32{{X}}\n");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run) = start(&text, "X", "constant_probe", edition, 1000);
        let report = completed(&mut db, &mut run);
        // A constant path is an ordinary `ValuePath` evaluated by the one pure
        // constant route; the removed SourceConstant opcode is not reached.
        assert!(matches!(report.value,ValueSummary::Unsigned(ref n) if n=="22"));
        assert!(report.opcode_counters.return_scopes_cleaned > 0);
        assert!(
            matches!(run.poll(&mut db),RunPoll::Fault(e) if e.message=="terminal run cannot execute again")
        );
    }
}
#[test]
fn original_header_array_two_uses_get_independent_mutable_storage_normal_run() {
    let text = format!(
        "{ARRAY}\nfn array_probe()->i32{{let mut first=ARR;let mut second=ARR;first[0]=11;second[1]=12;first[0]+second[0]*100}}\n"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run) = start(&text, "ARR", "array_probe", edition, 10000);
        let report = completed(&mut db, &mut run);
        assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="511"));
        assert!(report.opcode_counters.return_scopes_cleaned > 0);
        let owners = run.store.owner_inventory_for_test();
        assert!(owners.objects.is_empty());
        assert!(owners.tickets.is_empty());
        assert!(owners.guards.is_empty());
        // Numeric literal memo records are retained run data, distinct from
        // the owning array objects, construction guards and transfer tickets.
        assert!(owners.cells.iter().all(|(_, state)| matches!(
            state,
            CellState::Dropped | CellState::Moved | CellState::Uninit
        )));
    }
}
#[test]
fn unchanged_empty_main_never_demands_its_cold_constant_initializer() {
    let (mut db, mut run) = start(ARRAY, "ARR", "main", 2024, 1000);
    let report = completed(&mut db, &mut run);
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(
        report.opcode_counters.started_by_opcode[usize::from(SOURCE_CONSTANT_OPCODE)],
        0
    );
    assert!(run.constant_plans.is_empty());
    assert!(
        !db.trace
            .events
            .iter()
            .any(|e| matches!(e.key, QueryKey::Constant(_)))
    );
}
#[test]
fn source_epoch_change_drains_active_constant_without_publishing_old_payload() {
    let text = "const LARGE:[u8;1024]=[0;1024];fn main(){let a=LARGE;}";
    let (mut db, mut run) = start(text, "LARGE", "main", 2024, 10000);
    reach_constant(&mut db, &mut run);
    let (outcome, retained_owners) = one_constant_chunk(&mut db, &mut run);
    assert!(matches!(outcome, StepOutcome::Dispatch));
    let used = run.frame.fuel.snapshot().used;
    // Publication is only legal between coordinator runs. The retained VM
    // activation must refuse the newly published epoch after that real barrier.
    db.finish_run().unwrap();
    db.begin_expansion_epoch(2).unwrap();
    db.begin_run().unwrap();
    let RunPoll::Fault(e) = run.poll(&mut db) else {
        panic!("stale source constant became a value")
    };
    assert_eq!(e.code, "LRR-E-CONST-PATH-SOURCE");
    assert_eq!(e.message, "constant path caller/input membership changed");
    assert_eq!(run.frame.fuel.snapshot().used, used);
    assert!(!run.frame.has_source_constant_activation());
    assert_eq!(run.store.owner_inventory_for_test(), retained_owners);
}

#[test]
fn original_header_array_reentered_path_reuses_memo_but_creates_new_owned_array_each_time() {
    let text = format!(
        "{ARRAY}\nfn repeated_probe()->i32{{let mut total:i32=0;let mut count:i32=0;while count<3{{let mut a=ARR;a[0]=count;total=total+a[1];count=count+1;}}total}}\n"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run) = start(&text, "ARR", "repeated_probe", edition, 10000);
        let report = completed(&mut db, &mut run);
        assert!(matches!(report.value, ValueSummary::Signed(ref n) if n=="18"));
        let owners = run.store.owner_inventory_for_test();
        assert!(owners.objects.is_empty() && owners.guards.is_empty() && owners.tickets.is_empty());
    }
}
#[test]
fn large_constant_normal_dispatch_resumes_once_then_returns_and_cleans_all_array_owners() {
    // LRR resource/continuation regression; upstream language declarations are
    // exercised by the separate unchanged-source probes above.
    let text = "const LARGE:[u8;1024]=[0;1024];fn main()->u8{let a=LARGE;a[777]}";
    let (mut db, mut run) = start(text, "LARGE", "main", 2024, 10000);
    let report = completed(&mut db, &mut run);
    assert!(matches!(report.value,ValueSummary::Unsigned(ref n) if n=="0"));
    let owners = run.store.owner_inventory_for_test();
    assert!(owners.objects.is_empty() && owners.guards.is_empty() && owners.tickets.is_empty());
}

#[test]
fn original_array_direct_constant_receiver_mutation_does_not_change_later_uses_all_editions() {
    // Pristine upstream declaration, disclosed reached-use adapter. Each const
    // expression instantiates its own temporary; mutation is local to that use.
    let text = format!("{ARRAY}\nfn direct_probe()->i32{{ARR[0]=11;ARR[0]*100+ARR[1]}}\n");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run) = start(&text, "ARR", "direct_probe", edition, 10000);
        let report = completed(&mut db, &mut run);
        assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="506"));
        let owners = run.store.owner_inventory_for_test();
        assert!(owners.objects.is_empty() && owners.guards.is_empty() && owners.tickets.is_empty());
    }
}

#[test]
fn constant_receiver_exit_verifier_refuses_other_slots_phases_and_effect_prefix() {
    use crate::ir::recipe::RecipeDraft;
    let text = format!("{ARRAY}\nfn direct_probe()->i32{{ARR[0]}}\n");
    let (mut db, mut run) = start(&text, "ARR", "direct_probe", 2024, 10000);
    reach_constant(&mut db, &mut run);
    assert!(matches!(
        one_constant_chunk(&mut db, &mut run).0,
        StepOutcome::Next(_)
    ));
    let pc = run.frame.pc().clone();
    assert_eq!(pc.phase, Phase::Exit.code().unwrap());
    let id = run
        .installed
        .resolve(&mut db, pc.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    let recipe = run.installed.get(id).unwrap().recipe();
    let base = RecipeDraft {
        key: recipe.key.clone(),
        origin: recipe.origin.clone(),
        operations: recipe.operations.to_vec(),
        exits: vec![recipe.exit.clone()],
        dependencies: recipe.dependencies.to_vec(),
    };
    let [
        Op::ValuePlace {
            source,
            destination,
        },
    ] = base.operations.as_slice()
    else {
        panic!("original receiver exit is not a location formation")
    };
    let source = *source;
    let destination = *destination;
    let foreign_value = run
        .frame
        .body
        .nodes()
        .iter()
        .filter_map(|node| node.value)
        .find(|slot| *slot != source)
        .unwrap();
    let foreign_place = run
        .frame
        .body
        .nodes()
        .iter()
        .filter_map(|node| node.place)
        .find(|slot| *slot != destination)
        .unwrap();
    let mut wrong = base.clone();
    wrong.operations = vec![Op::ValuePlace {
        source: foreign_value,
        destination,
    }];
    assert!(crate::ir::validate::verify(wrong, &run.frame.body).is_err());
    let mut wrong = base.clone();
    wrong.operations = vec![Op::ValuePlace {
        source,
        destination: foreign_place,
    }];
    assert!(crate::ir::validate::verify(wrong, &run.frame.body).is_err());
    let mut wrong = base.clone();
    wrong.key.phase = Phase::Entry.code().unwrap();
    assert!(crate::ir::validate::verify(wrong, &run.frame.body).is_err());
    let mut wrong = base;
    wrong.operations.insert(
        0,
        Op::ScopeEnter {
            scope: run.frame.body.nodes()[pc.node as usize].scope,
        },
    );
    assert!(crate::ir::validate::verify(wrong, &run.frame.body).is_err());
    let report = completed(&mut db, &mut run);
    assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="5"));
}

#[test]
fn normal_reached_original_panic_declarations_use_actual_message_before_publication() {
    // These original declarations are compiler-negative fixtures. This disclosed
    // reached-use adapter validates LRR behavior, not their compiler diagnostics.
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/const_panic.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for (name, message) in [
            ("Z", "cheese"),
            ("Z2", "explicit panic"),
            ("Z_CORE", "cheese"),
            ("Z2_CORE", "explicit panic"),
        ] {
            let text = format!("{original}\nfn macro_probe(){{{name};}}\n");
            let (mut db, mut run) = start(&text, name, "macro_probe", edition, 1000);
            let RunPoll::Fault(error) = run.poll(&mut db) else {
                panic!("reached initializer did not fail")
            };
            assert_eq!(error.code, "LRR-E-CONST-PANIC");
            assert_eq!(error.message, message);
            assert_eq!(
                run.frame.counters.started_by_opcode[usize::from(SOURCE_CONSTANT_OPCODE)],
                0
            );
            assert!(!run.frame.has_source_constant_activation());
            assert!(run.constant_plans.is_empty());
            let owners = run.store.owner_inventory_for_test();
            assert!(
                owners.objects.is_empty() && owners.guards.is_empty() && owners.tickets.is_empty()
            );
        }
    }
}
#[test]
fn normal_const_call_macro_uses_the_entered_helper_body_without_cold_effects() {
    // LRR currentness/entry supplement around the untouched upstream declarations.
    let text = format!(
        "{SCALAR}\nconst fn need()->u32{{panic!(\"entered helper\")}} const C:u32=need(); fn macro_probe()->u32{{C}}\n"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run) = start(&text, "C", "macro_probe", edition, 1000);
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("helper macro did not fail")
        };
        assert_eq!(error.code, "LRR-E-CONST-PANIC");
        assert_eq!(error.message, "entered helper");
        assert_eq!(
            run.frame.counters.started_by_opcode[usize::from(SOURCE_CONSTANT_OPCODE)],
            0
        );
        assert!(!run.frame.has_source_constant_activation());
    }
}
#[test]
fn normal_cold_constant_macro_branch_runs_without_macro_admission_or_activation() {
    let text = format!(
        "{SCALAR}\nconst C:u32=if false{{panic!(\"cold\")}}else{{7}};fn macro_probe()->u32{{C}}\n"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, mut run) = start(&text, "C", "macro_probe", edition, 1000);
        let report = completed(&mut db, &mut run);
        assert!(matches!(report.value,ValueSummary::Unsigned(ref n) if n == "7"));
        assert_eq!(
            report.opcode_counters.started_by_opcode[usize::from(SOURCE_CONSTANT_OPCODE)],
            1
        );
        assert_eq!(run.constant_plans.len(), 1);
        assert!(!run.frame.has_source_constant_activation());
    }
}
