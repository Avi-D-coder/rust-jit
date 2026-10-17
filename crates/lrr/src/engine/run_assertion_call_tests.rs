//! LRR protocol regressions outside upstream originals. All original fixture
//! acceptance remains with the separately retained whole-main stock adapter.
use super::*;
use crate::{
    traits::term::{ParamKey, TyTerm},
    vm::{frame::FrameStatus, numeric::PanicKind},
};

const EDITIONS: [u16; 4] = [2015, 2018, 2021, 2024];

// The production standalone route: sealed discovery expands the library's own
// `assert_eq!`/`panic!` definitions, and the retained entry body executes them
// through ordinary operations. No hand-written seal, callable proof, source
// hint or guest cell is installed.
type Macros = crate::engine::standalone_macros::CompletedStandaloneRuntimeMacros;
fn start(source: &str, edition: u16) -> (Database, RunMachine, Macros) {
    borrows::tests::start_discovered_body(source, "main", edition)
}

fn poll(db: &mut Database, run: &mut RunMachine, macros: &Macros) -> RunPoll {
    let contexts = macros.contexts().unwrap();
    run.poll_with_runtime_macros(db, &contexts)
}

fn body(run: &RunMachine, name: &str) -> crate::source::manifest::BodyKey {
    run.sources[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key()
}

fn completed(db: &mut Database, run: &mut RunMachine, macros: &Macros, calls: u64) -> RunReport {
    let report = match poll(db, run, macros) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("source-call assertion faulted: {error:?}"),
        RunPoll::Panicked(report) => panic!("source-call assertion panicked: {report:?}"),
        RunPoll::NeedInput(request) => panic!("source-call assertion suspended: {request:?}"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    // Every written source call is entered exactly once.
    assert_eq!(report.opcode_counters.started_by_opcode[19], calls);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    assert!(run.calls.is_empty() && run.pending.is_empty());
    assert_eq!(run.call_bytes, 0);
    assert_eq!(run.pending_bytes, 0);
    assert!(run.returning.is_none() && run.unwinding.is_none());
    assert!(run.frame.active_scopes().is_empty());
    assert!(run.terminal);
    report
}

fn first_body_query(db: &Database, selected: &crate::source::manifest::BodyKey) -> usize {
    db.trace
        .events
        .iter()
        .position(|event| matches!(&event.key,
            QueryKey::BodyStructure(body) if body.definition == selected.definition))
        .expect("a completed source call must actually index its body")
}

/// The body of `selected` is never indexed or prepared. Its declared
/// signature may still answer a type goal without entering the body.
fn require_body_cold(db: &Database, selected: &crate::source::manifest::BodyKey) {
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => body.definition != selected.definition,
        QueryKey::Thunk(pc) => pc.body.definition != selected.definition,
        _ => true,
    }));
}

fn require_cold(db: &Database, selected: &crate::source::manifest::BodyKey) {
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::FunctionSignature(definition) | QueryKey::Signature(definition) => {
            *definition != selected.definition
        }
        QueryKey::BodyStructure(body) => body.definition != selected.definition,
        QueryKey::Thunk(pc) => pc.body.definition != selected.definition,
        _ => true,
    }));
}

#[test]
fn nested_assertion_calls_keep_outer_progress_until_inner_return_all_editions() {
    const SOURCE: &str = "fn id(x:i32)->i32{x} fn main(){assert_eq!(id(id(1)),1);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let report = completed(&mut db, &mut run, &macros, 2);
        assert_eq!(report.opcode_counters.started_by_opcode[22], 2);
        assert_eq!(report.opcode_counters.started_by_opcode[23], 2);
        // The outer id is selected while its argument runs, but is not entered
        // until the inner id has returned. This is not three simultaneous frames.
        assert_eq!(run.peak_guest_frames, 2);
        db.finish_run().unwrap();
    }
}

#[test]
fn sibling_assertion_calls_complete_left_before_right_all_editions() {
    const SOURCE: &str =
        "fn left()->i32{1} fn right()->i32{2} fn main(){assert_eq!(left()+right(),3);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let left = body(&run, "left");
        let right = body(&run, "right");
        completed(&mut db, &mut run, &macros, 2);
        assert!(first_body_query(&db, &left) < first_body_query(&db, &right));
        db.finish_run().unwrap();
    }
}

#[test]
fn sibling_left_panic_is_the_actual_first_effect_and_right_stays_cold_all_editions() {
    const SOURCE: &str = "fn left()->i32{panic!(\"left-first\")} fn right()->i32{panic!(\"right-second\")} fn main(){assert_eq!(left(),right());}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let left = body(&run, "left");
        let right = body(&run, "right");
        let RunPoll::Panicked(report) = poll(&mut db, &mut run, &macros) else {
            panic!("left sibling must produce its ordinary guest panic")
        };
        assert_eq!(report.panic.kind, PanicKind::Explicit);
        assert_eq!(report.panic.message.as_deref(), Some("left-first"));
        assert_eq!(report.panic.pc.body.definition, left.definition);
        require_cold(&db, &right);
        assert!(report.opcode_counters.panic_scopes_cleaned > 0);
        assert_eq!(run.frame.status(), FrameStatus::PanicCleaned);
        assert!(run.calls.is_empty() && run.pending.is_empty());
        assert_eq!(run.call_bytes, 0);
        assert_eq!(run.pending_bytes, 0);
        db.finish_run().unwrap();
    }
}

fn result_parameter(db: &mut Database, selected: &crate::source::manifest::BodyKey) -> ParamKey {
    let value = db
        .fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(selected.definition),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::FunctionSignature(signature) = value.value.as_ref() else {
        panic!("actual source signature query changed variant")
    };
    let Some(TyTerm::Param(parameter)) = &signature.result_symbolic else {
        panic!("source generic result must retain its actual rigid binder")
    };
    *parameter
}

#[test]
fn generic_assertion_calls_preserve_concrete_result_and_distinct_rigid_sources_all_editions() {
    const SOURCE: &str = "fn id<T>(x:T)->T{x} fn forward<T>(x:T)->T{id(x)} fn main(){let x:i64=1;assert_eq!(forward(id(x)),1i64);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let id = body(&run, "id");
        let forward = body(&run, "forward");
        let report = completed(&mut db, &mut run, &macros, 3);
        assert_eq!(report.opcode_counters.started_by_opcode[22], 3);
        assert_eq!(report.opcode_counters.started_by_opcode[23], 3);
        let id_parameter = result_parameter(&mut db, &id);
        let forward_parameter = result_parameter(&mut db, &forward);
        assert_eq!(id_parameter.definition, id.definition);
        assert_eq!(forward_parameter.definition, forward.definition);
        assert_ne!(id_parameter, forward_parameter);
        // The real call/return contracts must bind the independent source
        // parameters while keeping i64's descriptor, rather than the Macro
        // expression's Unit result or the comparison's Bool result.
        db.finish_run().unwrap();
    }
}

#[test]
fn zero_argument_bool_predicate_assertion_calls_its_predicate_once_all_editions() {
    const SOURCE: &str = "fn truth()->bool{true} fn main(){assert!(truth());}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let truth = body(&run, "truth");
        // The expanded `if !truth() { panic }` branches past its failure path;
        // the selected call still runs exactly once.
        completed(&mut db, &mut run, &macros, 1);
        first_body_query(&db, &truth);
        db.finish_run().unwrap();
    }
}

#[test]
fn earlier_argument_panic_prevents_later_unsupported_method_and_source_demand_all_editions() {
    const SOURCE: &str = "fn stop()->i32{panic!(\"first\")} fn later()->&'static str{\"later\"} fn f(_:i32,_:i32)->i32{0} fn main(){assert_eq!(f(stop(),later().len() as i32),0);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let stop = body(&run, "stop");
        let later = body(&run, "later");
        let f = body(&run, "f");
        let RunPoll::Panicked(report) = poll(&mut db, &mut run, &macros) else {
            panic!("earlier argument must panic before later method admission")
        };
        assert_eq!(report.panic.kind, PanicKind::Explicit);
        assert_eq!(report.panic.message.as_deref(), Some("first"));
        assert_eq!(report.panic.pc.body.definition, stop.definition);
        // `later().len()` may consult `later`'s declared signature for its
        // receiver type; neither later body nor `f` is ever entered.
        require_body_cold(&db, &later);
        require_body_cold(&db, &f);
        assert_eq!(run.frame.status(), FrameStatus::PanicCleaned);
        assert!(run.calls.is_empty() && run.pending.is_empty());
        assert_eq!(run.call_bytes, 0);
        assert_eq!(run.pending_bytes, 0);
        assert!(run.unwinding.is_none() && run.returning.is_none());
        assert!(run.frame.active_scopes().is_empty());
        db.finish_run().unwrap();
    }
}

#[test]
fn repeated_assertion_site_restarts_call_progress_without_old_slot_context_all_editions() {
    const SOURCE: &str =
        "fn id(x:i32)->i32{x} fn main(){let mut n:i32=0;while n<3{assert_eq!(id(n),n);n+=1;}}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let report = completed(&mut db, &mut run, &macros, 3);
        assert_eq!(report.opcode_counters.started_by_opcode[22], 3);
        assert_eq!(report.opcode_counters.started_by_opcode[23], 3);
        assert!(report.opcode_counters.backedge_polls >= 3);
        db.finish_run().unwrap();
    }
}


#[test]
fn alias_bool_match_uses_actual_primitive_parameter_source_all_editions() {
    const SOURCE: &str = "type Flag=bool;fn chosen(x:Flag)->bool{match x{true=>true,false=>false}}fn main(){assert_eq!(chosen(true),true);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        completed(&mut db, &mut run, &macros, 1);
        db.finish_run().unwrap();
    }
}
#[test]
fn suffixed_reference_argument_reaches_scalar_match_guard_before_owning_read_all_editions() {
    const SOURCE: &str =
        "fn chosen(x:&mut u8){match x{true=>(),_=>()}}fn main(){let mut n=0u8;chosen(&mut n);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let chosen = body(&run, "chosen");
        let RunPoll::Fault(error) = poll(&mut db, &mut run, &macros) else {
            panic!("a reference scrutinee must not become a scalar")
        };
        // The shared matcher peels the reference (default binding mode) and
        // refuses the Bool literal against the actual u8 referent.
        assert_eq!(error.code, "LRR-E-PATTERN-TYPE");
        assert_eq!(
            error.message,
            "pattern literal type differs from the scrutinee"
        );
        assert_eq!(run.frame.body.body().definition, chosen.definition);
        first_body_query(&db, &chosen);
        // The actual suffix supplied source identity at argument admission.
        // Once entered, a reference still cannot masquerade as a scalar value.
        assert_eq!(run.frame.counters().started_by_opcode[21], 0);
        db.finish_run().unwrap();
    }
}

#[test]
fn suffixed_local_reference_call_retains_source_without_invented_descriptor_identity_all_editions()
{
    // LRR call/certificate regression; stock acceptance uses unchanged files.
    const SOURCE: &str = "fn chosen(x:&mut u8){} fn main(){let mut n=0u8;chosen(&mut n);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let chosen = body(&run, "chosen");
        completed(&mut db, &mut run, &macros, 1);
        first_body_query(&db, &chosen);
        db.finish_run().unwrap();
    }
}

#[test]
fn negative_suffixed_local_reference_call_keeps_written_source_all_editions() {
    // LRR source/certificate regression; the unchanged signed radix initializer
    // and binding are separately exercised as upstream component evidence.
    for literal in ["-1isize", "-(1isize)", "-(((1isize)))"] {
        let source = format!("fn chosen(x:&isize){{}} fn main(){{let n={literal};chosen(&n);}}");
        for edition in EDITIONS {
            let (mut db, mut run, macros) = start(&source, edition);
            let chosen = body(&run, "chosen");
            completed(&mut db, &mut run, &macros, 1);
            first_body_query(&db, &chosen);
            db.finish_run().unwrap();
        }
    }
}

#[test]
fn byte_local_reference_call_keeps_intrinsic_source_all_editions() {
    // LRR source/certificate regression; real upstream byte matches are covered
    // separately without pretending the earlier byte-string setup executes.
    const SOURCE: &str = "fn chosen(x:&u8){} fn main(){let n=b'a';chosen(&n);}";
    for edition in EDITIONS {
        let (mut db, mut run, macros) = start(SOURCE, edition);
        let chosen = body(&run, "chosen");
        completed(&mut db, &mut run, &macros, 1);
        first_body_query(&db, &chosen);
        db.finish_run().unwrap();
    }
}
