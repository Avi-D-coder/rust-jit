//! LRR protocol regressions outside upstream originals. All original fixture
//! acceptance remains with the separately retained whole-main stock adapter.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::key::OutlineKey,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
    traits::term::{ParamKey, TyTerm},
    vm::{frame::FrameStatus, numeric::PanicKind},
};

const EDITIONS: [u16; 4] = [2015, 2018, 2021, 2024];

// The source snapshot/publication and runtime macro catalogue are genuine. No
// hand-written seal, callable proof, source hint or guest cell is installed.
fn start(source: &str, edition: u16) -> (Database, RunMachine) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("assertion-call-regression.rs").unwrap();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let unit = Digest::of_bytes(b"LRR assertion source-call protocol regression");
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
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("published regression source is absent")
    };
    let main = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    db.begin_run().unwrap();
    let run = RunMachine::start_with_sources(&mut db, main, 1_000_000, vec![view.share()])
        .into_ready()
        .unwrap()
        .unwrap();
    (db, run)
}

fn poll(db: &mut Database, run: &mut RunMachine) -> RunPoll {
    let view = run.sources[0].share();
    let macros = crate::engine::standalone_macros::StandaloneRuntimeMacros::new(&view).unwrap();
    let contexts = [macros.context(&view).unwrap()];
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

fn completed(db: &mut Database, run: &mut RunMachine, assertions: u64, calls: u64) -> RunReport {
    let report = match poll(db, run) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("source-call assertion faulted: {error:?}"),
        RunPoll::Panicked(report) => panic!("source-call assertion panicked: {report:?}"),
        RunPoll::NeedInput(request) => panic!("source-call assertion suspended: {request:?}"),
    };
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.opcode_counters.started_by_opcode[26], assertions);
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
        .position(|event| matches!(&event.key, QueryKey::BodyStructure(body) if body == selected))
        .expect("a completed source call must actually index its body")
}

fn require_cold(db: &Database, selected: &crate::source::manifest::BodyKey) {
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::FunctionSignature(definition) | QueryKey::Signature(definition) => {
            *definition != selected.definition
        }
        QueryKey::BodyStructure(body) => body != selected,
        QueryKey::Thunk(pc) => pc.body != *selected,
        _ => true,
    }));
}

#[test]
fn nested_assertion_calls_keep_outer_progress_until_inner_return_all_editions() {
    const SOURCE: &str = "fn id(x:i32)->i32{x} fn main(){assert_eq!(id(id(1)),1);}";
    for edition in EDITIONS {
        let (mut db, mut run) = start(SOURCE, edition);
        let report = completed(&mut db, &mut run, 1, 2);
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
        let (mut db, mut run) = start(SOURCE, edition);
        let left = body(&run, "left");
        let right = body(&run, "right");
        completed(&mut db, &mut run, 1, 2);
        assert!(first_body_query(&db, &left) < first_body_query(&db, &right));
        db.finish_run().unwrap();
    }
}

#[test]
fn sibling_left_panic_is_the_actual_first_effect_and_right_stays_cold_all_editions() {
    const SOURCE: &str = "fn left()->i32{panic!(\"left-first\")} fn right()->i32{panic!(\"right-second\")} fn main(){assert_eq!(left(),right());}";
    for edition in EDITIONS {
        let (mut db, mut run) = start(SOURCE, edition);
        let left = body(&run, "left");
        let right = body(&run, "right");
        let RunPoll::Panicked(report) = poll(&mut db, &mut run) else {
            panic!("left sibling must produce its ordinary guest panic")
        };
        assert_eq!(report.panic.kind, PanicKind::Explicit);
        assert_eq!(report.panic.message.as_deref(), Some("left-first"));
        assert_eq!(report.panic.pc.body, left);
        require_cold(&db, &right);
        assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[25], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
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
        let (mut db, mut run) = start(SOURCE, edition);
        let id = body(&run, "id");
        let forward = body(&run, "forward");
        let report = completed(&mut db, &mut run, 1, 3);
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
fn legacy_zero_argument_bool_predicate_route_still_executes_all_editions() {
    const SOURCE: &str = "fn truth()->bool{true} fn main(){assert!(truth());}";
    for edition in EDITIONS {
        let (mut db, mut run) = start(SOURCE, edition);
        let truth = body(&run, "truth");
        // The legacy predicate route branches to Unit on success. Assert is
        // reached only on its failure path; the selected call still runs once.
        let report = completed(&mut db, &mut run, 0, 1);
        assert_eq!(report.opcode_counters.started_by_opcode[23], 0);
        first_body_query(&db, &truth);
        db.finish_run().unwrap();
    }
}

#[test]
fn earlier_argument_panic_prevents_later_unsupported_method_and_source_demand_all_editions() {
    const SOURCE: &str = "fn stop()->i32{panic!(\"first\")} fn later()->&'static str{\"later\"} fn f(_:i32,_:i32)->i32{0} fn main(){assert_eq!(f(stop(),later().len() as i32),0);}";
    for edition in EDITIONS {
        let (mut db, mut run) = start(SOURCE, edition);
        let stop = body(&run, "stop");
        let later = body(&run, "later");
        let f = body(&run, "f");
        let RunPoll::Panicked(report) = poll(&mut db, &mut run) else {
            panic!("earlier argument must panic before later method admission")
        };
        assert_eq!(report.panic.kind, PanicKind::Explicit);
        assert_eq!(report.panic.message.as_deref(), Some("first"));
        assert_eq!(report.panic.pc.body, stop);
        require_cold(&db, &later);
        assert!(db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body != &f,
            QueryKey::Thunk(pc) => pc.body != f,
            _ => true,
        }));
        assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[23], 0);
        assert_eq!(report.opcode_counters.started_by_opcode[25], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
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
        let (mut db, mut run) = start(SOURCE, edition);
        let report = completed(&mut db, &mut run, 3, 3);
        assert_eq!(report.opcode_counters.started_by_opcode[22], 3);
        assert_eq!(report.opcode_counters.started_by_opcode[23], 3);
        assert!(report.opcode_counters.backedge_polls >= 3);
        db.finish_run().unwrap();
    }
}

#[test]
fn cold_incompatible_result_relation_uses_local_numeric_default_all_editions() {
    // LRR reached-semantics controls, intentionally compiler-negative. Runtime
    // skips the incompatible return; static rejection remains a separate gate.
    for source in [
        "fn chosen()->bool{let n=0;if false{return n;}true}fn main(){assert_eq!(chosen(),true);}",
        "fn chosen()->bool{let n=0.0;if false{return n;}true}fn main(){assert_eq!(chosen(),true);}",
        "fn chosen()->f64{let n=0;if false{return n;}1.0}fn main(){assert_eq!(chosen(),1.0f64);}",
    ] {
        for edition in EDITIONS {
            let (mut db, mut run) = start(source, edition);
            completed(&mut db, &mut run, 1, 1);
            db.finish_run().unwrap();
        }
    }
}

#[test]
fn alias_bool_match_uses_actual_primitive_parameter_source_all_editions() {
    const SOURCE: &str = "type Flag=bool;fn chosen(x:Flag)->bool{match x{true=>true,false=>false}}fn main(){assert_eq!(chosen(true),true);}";
    for edition in EDITIONS {
        let (mut db, mut run) = start(SOURCE, edition);
        completed(&mut db, &mut run, 1, 1);
        db.finish_run().unwrap();
    }
}
#[test]
fn reference_argument_admission_keeps_scalar_match_callee_cold_all_editions() {
    const SOURCE: &str =
        "fn chosen(x:&mut u8){match x{true=>(),_=>()}}fn main(){let mut n=0u8;chosen(&mut n);}";
    for edition in EDITIONS {
        let (mut db, mut run) = start(SOURCE, edition);
        let chosen = body(&run, "chosen");
        let RunPoll::Fault(error) = poll(&mut db, &mut run) else {
            panic!("a non-scalar scrutinee must require ordinary pattern integration")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        // This genuine whole-call control still refuses the argument before
        // body admission: the unannotated numeric local has no retained source
        // view, and a reference descriptor cannot invent its source identity.
        // Suffix/local-source propagation remains a required runtime feature;
        // the certified-reference scalar guard has a separate direct control.
        assert_eq!(
            error.message,
            "source view requires further value-family integration"
        );
        // Resolving the callee's parameter header is necessary to refuse its
        // reference argument. Only executable body demands must remain absent.
        for event in &db.trace.events {
            let cold = match &event.key {
                QueryKey::BodyStructure(body) => body != &chosen,
                QueryKey::Thunk(pc) => pc.body != chosen,
                _ => true,
            };
            assert!(
                cold,
                "callee body demanded before argument admission: {:?}",
                event.key
            );
        }
        assert_ne!(run.frame.body.body(), &chosen);
        assert_eq!(run.frame.counters().started_by_opcode[21], 0);
        db.finish_run().unwrap();
    }
}
