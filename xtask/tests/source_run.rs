use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
    },
    engine::run::{RunMachine, RunPoll, RunReport, ValueSummary},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn start(source: &str, name: &str, steps: u64) -> (Database, RunMachine, Digest) {
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let crate_key = Digest([84; 32]);
    let path = SourcePath::new("src/main.rs").unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("selected source missing")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let cold = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "cold")
        .map_or(Digest([0; 32]), |item| item.def_key.fingerprint());
    let mut db = Database::default();
    db.trace.detailed = true;
    prepared.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let run = match RunMachine::start_with_sources(&mut db, body, steps, vec![view]) {
        QueryPoll::Ready(Ok(run)) => run,
        QueryPoll::Ready(Err(e)) => panic!("{source}: {} {}", e.code, e.message),
        QueryPoll::NeedInput(i) => panic!("{source}: unexpected startup input {i:?}"),
    };
    (db, run, cold)
}
fn completed(poll: RunPoll) -> RunReport {
    match poll {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("{error}"),
        RunPoll::Panicked(report) => panic!("unexpected guest panic {:?}", report.panic.kind),
        RunPoll::NeedInput(request) => panic!("unexpected demand {request:?}"),
    }
}
#[test]
fn normative_main_executes_lrir_with_exact_counts_and_no_cold_semantic_queries() {
    let (mut db, mut run, cold) = start(
        "fn main() { let x: i64 = 40_i64; x; } fn cold<T: Missing>() { undefined::<Absent>(); }",
        "main",
        1000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.status, "completed");
    assert_eq!(report.opcode_counters.operations_started, 12);
    assert_eq!(report.opcode_counters.operations_completed, 12);
    assert_eq!(report.opcode_counters.exits_completed, 8);
    let mut expected_opcode_counts = [0; lrr::ir::op::OPCODE_COUNT];
    expected_opcode_counts[0] = 1;
    expected_opcode_counts[1] = 1;
    expected_opcode_counts[2] = 2;
    expected_opcode_counts[4] = 3;
    expected_opcode_counts[5] = 2;
    expected_opcode_counts[7] = 1;
    expected_opcode_counts[8] = 1;
    expected_opcode_counts[9] = 1;
    assert_eq!(
        report.opcode_counters.started_by_opcode,
        expected_opcode_counts
    );
    assert_eq!(report.opcode_counters.return_scopes_cleaned, 1);
    assert_eq!(report.installed_recipes, 8);
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::Signature(def) => *def != cold,
        QueryKey::BodyStructure(body) => body.definition != cold,
        QueryKey::Thunk(pc) => pc.body.definition != cold,
        _ => true,
    }));
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-INTERNAL-IR"));
    db.finish_run().unwrap();
}
#[test]
fn reached_annotation_and_local_reads_produce_source_dependent_actual_values() {
    for (literal, expected) in [("40_i64", "40"), ("41", "41"), ("0x2a_i64", "42")] {
        let source = format!("fn live()->i64{{let x:i64={literal}; x}} fn cold(){{missing();}}");
        let (mut db, mut run, _) = start(&source, "live", 1000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value,ValueSummary::Signed(value) if value==expected));
        assert_eq!(report.opcode_counters.started_by_opcode[7], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[8], 1);
        assert_eq!(report.opcode_counters.return_scopes_cleaned, 1);
        db.finish_run().unwrap();
    }
}
#[test]
fn shadow_initializer_reads_the_previous_executed_binding() {
    let (mut db, mut run, _) = start(
        "fn live()->i64{let x:i64=40_i64; let x:i64=x; x}",
        "live",
        1000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value,ValueSummary::Signed(value) if value=="40"));
    assert_eq!(report.opcode_counters.started_by_opcode[7], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[8], 2);
    db.finish_run().unwrap();
}
#[test]
fn reached_unsupported_and_dispatch_limit_are_faults_not_successful_reports() {
    let (mut db, mut run, _) = start("fn main(){missing();} fn cold(){absent();}", "main", 1000);
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-NAME-MISSING"));
    db.finish_run().unwrap();
    let (mut db, mut run, _) = start("fn main(){let x:i64=40_i64;x;}", "main", 0);
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
    db.finish_run().unwrap();
}

#[test]
fn local_ops_reject_non_pattern_binding_identities_before_execution() {
    use lrr::{
        ir::{
            op::{HintRecipe, Op},
            pc::{NodeId, Phase, SlotId},
            recipe::{Origin, RecipeDraft, RecipeExit},
            validate::verify,
        },
        syntax::parse::ParsedFile,
    };
    let blob = SourceBlob::new(b"fn main(){let x:i64=40_i64;x;}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([85; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let index = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let key = index.pc(NodeId(0), Phase::Entry).unwrap();
    for op in [
        Op::BindLocal {
            binding: NodeId(0),
            source: SlotId(0),
            hint: HintRecipe::None,
            writable: false,
        },
        Op::ReadLocal {
            binding: NodeId(u32::MAX),
            destination: SlotId(0),
        },
    ] {
        let draft = RecipeDraft {
            key: key.clone(),
            origin: Origin {
                span: index.span(NodeId(0)).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: vec![op],
            exits: vec![RecipeExit::Next(index.pc(NodeId(0), Phase::Exit).unwrap())],
            dependencies: vec![],
        };
        assert_eq!(verify(draft, &index).unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
}

#[test]
fn source_numeric_phases_use_reached_return_local_and_other_operand_contexts() {
    for (source, expected) in [
        ("fn live()->i64{3+4}", "7"),
        ("fn live()->i64{let x:i64=3+4;x}", "7"),
        ("fn live()->i64{let x=3;let y:i64=x;x+y}", "6"),
        ("fn live()->i64{(3_i8 as i64)+4}", "7"),
        ("fn live()->i64{!0_i64}", "-1"),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value,ValueSummary::Signed(value) if value==expected));
        db.finish_run().unwrap()
    }
    let (mut db, mut run, _) = start("fn live()->bool{3==3_i64}", "live", 1000);
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Bool(true)
    ));
    db.finish_run().unwrap();
}
#[test]
fn source_numeric_fallback_islands_keep_future_constraints_pending() {
    for (source, expected) in [
        ("fn live()->bool{3==4}", ValueSummary::Bool(false)),
        ("fn live()->i64{1 as i64}", ValueSummary::Signed("1".into())),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("literal-only numeric inference island did not complete")
        };
        assert_eq!(
            serde_json::to_value(report.value).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        db.finish_run().unwrap();
    }
    // The direct later annotation constrains the reached closed operator.
    {
        let (mut db, mut run, _) = start("fn main(){let x=3+4;let later:i64=x;}", "main", 1000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[10], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[7], 2);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        db.finish_run().unwrap();
    }
}
#[test]
fn source_minimum_literal_pairs_construct_minima_but_negating_a_value_panics() {
    for (source, expected) in [
        ("fn live()->i8{-128_i8}", "-128"),
        ("fn live()->i8{-(128_i8)}", "-128"),
        (
            "fn live()->i64{-9_223_372_036_854_775_808_i64}",
            "-9223372036854775808",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value,ValueSummary::Signed(value) if value==expected));
        assert_eq!(report.opcode_counters.started_by_opcode[11], 0);
        db.finish_run().unwrap()
    }
    let (mut db, mut run, _) = start("fn live()->i8{let x=-128_i8;-x}", "live", 1000);
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("materialized minimum negation must be a guest panic")
    };
    assert_eq!(report.panic.kind, lrr::vm::numeric::PanicKind::Overflow);
    // The overflowing tail owns a scope inside the function block.
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 2);
    assert_eq!(report.opcode_counters.return_scopes_cleaned, 0);
    db.finish_run().unwrap();
}
#[test]
fn source_shift_counts_use_i32_fallback_without_reinterpreting_explicit_counts() {
    for source in [
        "fn live()->i8{1_i8<<2}",
        "fn live()->i8{1_i8<<2_u64}",
        "fn live()->i8{1_i8<<(1+1)}",
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(value) if value=="4")
        );
        db.finish_run().unwrap();
    }
    for source in ["fn live()->i8{1_i8<<-1}", "fn live()->i8{1_i8<<8_u64}"] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        let RunPoll::Panicked(report) = run.poll(&mut db) else {
            panic!("invalid source shift count must be guest panic")
        };
        assert_eq!(report.panic.kind, lrr::vm::numeric::PanicKind::InvalidShift);
        db.finish_run().unwrap();
    }
    // Retained legacy count witness: source fallback must reject this literal
    // during range admission, rather than bypassing width checks and panicking.
    let (mut db, mut run, _) = start("fn live()->i8{1_i8<<18446744073709551616}", "live", 1000);
    assert!(matches!(run.poll(&mut db), RunPoll::Fault(error) if error.code == "LRR-E-VALUE-REPR"));
    db.finish_run().unwrap();
}
#[test]
fn source_arithmetic_panic_keeps_its_span_cleans_once_and_does_not_query_later_code() {
    let source = "fn main(){let x:i8=127_i8+1_i8;missing();} fn cold(){absent();}";
    let (mut db, mut run, cold) = start(source, "main", 1000);
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("missing source arithmetic panic")
    };
    assert_eq!(report.panic.kind, lrr::vm::numeric::PanicKind::Overflow);
    assert_eq!(report.status, "guest_panic");
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 2);
    assert_eq!(report.opcode_counters.return_scopes_cleaned, 0);
    let span = &report.panic.origin.span;
    assert_eq!(
        span.relative_end - span.relative_start,
        "127_i8+1_i8".len() as u32
    );
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(&event.key,QueryKey::Signature(def) if *def==cold))
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-INTERNAL-IR"));
    db.finish_run().unwrap();
}

#[test]
fn source_float_literals_contexts_casts_and_each_f32_operation_keep_actual_precision() {
    for (source, expected) in [
        ("fn live()->f32{(16_777_216_f32+1_f32)-16_777_216_f32}", "0"),
        ("fn live()->f32{-0_f32}", "-0"),
        ("fn live()->f32{0_f32/0_f32}", "NaN"),
        ("fn live()->f32{1_f32/0_f32}", "inf"),
        ("fn live()->f32{let x=1.0;let y:f32=x;x+y}", "2"),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(completed(run.poll(&mut db)).value,ValueSummary::F32(value) if value==expected)
        );
        db.finish_run().unwrap();
    }
    let (mut db, mut run, _) = start("fn live()->f64{1_2.5_0e+1}", "live", 1000);
    assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::F64(value) if value=="125"));
    db.finish_run().unwrap();
    let (mut db, mut run, _) = start("fn live()->i8{1000_f64 as i8}", "live", 1000);
    assert!(
        matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(value) if value=="127")
    );
    db.finish_run().unwrap();
}
#[test]
fn source_float_fallback_islands_preserve_range_and_pending_constraints() {
    for (source, expected) in [
        ("fn live()->bool{1.0==2.0}", ValueSummary::Bool(false)),
        ("fn live()->f64{1.0 as f64}", ValueSummary::F64("1".into())),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("literal-only float inference island did not complete")
        };
        assert_eq!(
            serde_json::to_value(report.value).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        db.finish_run().unwrap();
    }
    // The direct later annotation constrains the reached closed operator.
    {
        let (mut db, mut run, _) = start("fn main(){let x=1.0+2.0;let later:f64=x;}", "main", 1000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[10], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[7], 2);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        db.finish_run().unwrap();
    }
    let (mut db, mut run, _) = start("fn live()->f64{1e500_f64}", "live", 1000);
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-VALUE-REPR"));
    db.finish_run().unwrap();
    let mut store = lrr::runtime::cell::GuestStore::new(1, 20, 1 << 20).unwrap();
    let literal = store.float_literal("1e100".into()).unwrap();
    assert!(
        store
            .materialize_ticket(literal, lrr::runtime::shape::TypeId(14))
            .is_err()
    );
    let ticket = store
        .materialize_ticket(literal, lrr::runtime::shape::TypeId(15))
        .unwrap();
    assert_eq!(store.peek_ticket(ticket).unwrap().type_id, 15);
}
#[test]
fn source_char_and_byte_escapes_are_pinned_decoded_values_and_support_primitive_casts() {
    for (source, expected) in [
        (r"fn live()->char{'\u{1F980}'}", '🦀'),
        (r"fn live()->char{'\n'}", '\n'),
        ("fn live()->char{'β'}", 'β'),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(completed(run.poll(&mut db)).value,ValueSummary::Char(value) if value==expected)
        );
        db.finish_run().unwrap();
    }
    let (mut db, mut run, _) = start(r"fn live()->u8{b'\xff'}", "live", 1000);
    assert!(
        matches!(completed(run.poll(&mut db)).value,ValueSummary::Unsigned(value) if value=="255")
    );
    db.finish_run().unwrap();
    let (mut db, mut run, _) = start(r"fn live()->u32{'\u{1F980}' as u32}", "live", 1000);
    assert!(
        matches!(completed(run.poll(&mut db)).value,ValueSummary::Unsigned(value) if value=="129408")
    );
    db.finish_run().unwrap();
    let (mut db, mut run, _) = start("fn live()->bool{'a'<'β'}", "live", 1000);
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Bool(true)
    ));
    db.finish_run().unwrap();
}

#[test]
fn explicit_returns_supply_numeric_context_and_do_not_prepare_following_statements() {
    for (source, expected) in [
        ("fn live()->i64{return 3+4;missing();}", "7"),
        ("fn live()->i8{return -128_i8;missing();}", "-128"),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(value) if value==expected)
        );
        db.finish_run().unwrap();
    }
    let (mut db, mut run, _) = start("fn live()->f32{return 1.0+2.0;missing();}", "live", 1000);
    assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::F32(value) if value=="3"));
    db.finish_run().unwrap();
    let (mut db, mut run, _) = start("fn live()->i64{return 3_i32;}", "live", 1000);
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-CALL-RETURN-TYPE")
    );
    db.finish_run().unwrap();
    let (mut db, mut run, _) = start("fn live()->u64{let x=3;let y:u64=x;x}", "live", 1000);
    assert!(
        matches!(completed(run.poll(&mut db)).value,ValueSummary::Unsigned(value) if value=="3")
    );
    db.finish_run().unwrap();
}

#[test]
fn nested_blocks_preserve_results_outer_bindings_and_reached_local_contexts() {
    for (source, expected) in [
        (
            "fn live()->i64 { let x=5_i64; let y:i64={ let x=x+1; x+2 }; x+y }",
            "13",
        ),
        (
            "fn live()->f32 { let x:i64={3+4}; let y:f32={1.0+2.0}; y }",
            "3",
        ),
        ("fn live()->i64 { { let x:i64=3; { x+4 } } }", "7"),
        (
            "fn live()->i64 { { return 9; missing_after_return(); } missing_outer(); }",
            "9",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        let report = completed(run.poll(&mut db));
        let actual = match report.value {
            ValueSummary::Signed(s) | ValueSummary::F32(s) => s,
            value => panic!("unexpected {value:?}"),
        };
        assert_eq!(actual, expected);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
    }
    let (mut db, mut run, _) = start(
        "fn live()->i64 { { let hidden=3_i64; } hidden }",
        "live",
        1000,
    );
    assert!(matches!(run.poll(&mut db), RunPoll::Fault(_)));
}

#[test]
fn nested_empty_blocks_return_unit_and_retire_their_inner_scope() {
    let (mut db, mut run, _) = start("fn live() { { let x=4_i64; x; }; }", "live", 1000);
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.opcode_counters.started_by_opcode[4], 5);
    assert_eq!(report.opcode_counters.started_by_opcode[5], 4);
    assert_eq!(report.opcode_counters.return_scopes_cleaned, 1);
}

#[test]
fn modified_and_labeled_blocks_cannot_enter_plain_block_execution() {
    for expression in [
        "async { 3_i64 }",
        "async move { 3_i64 }",
        "const { 3_i64 }",
        "unsafe { 3_i64 }",
    ] {
        let source = format!("fn live() {{ {expression}; }}");
        let (mut db, mut run, _) = start(&source, "live", 1000);
        assert!(
            matches!(run.poll(&mut db), RunPoll::Fault(error) if error.code=="LRR-E-UNSUPPORTED")
        );
    }
    // This legacy fixture name retains its unsupported modifier checks; labels
    // now use supported control semantics and have a positive regression.
    let (mut db, mut run, _) = start("fn live()->i64{'label:{break 'label 3_i64;}}", "live", 1000);
    assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(s) if s=="3"));
}

#[test]
fn local_declarations_assignment_and_fixed_contracts_execute_at_reached_phases() {
    for (source, expected) in [
        ("fn live()->i64 { let x:i64; x=3+4; x }", "7"),
        ("fn live()->i64 { let mut x=1_i64; x=x+2; x }", "3"),
        (
            "fn live()->i64 { let mut x=1_i64; x={x=7_i64; x+2}; x }",
            "9",
        ),
        ("fn live()->i64 { let x; x=4; let y:i64=x; x+y }", "8"),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value, ValueSummary::Signed(s) if s==expected));
    }
    for (source, code) in [
        ("fn live()->i64 {let x:i64; x}", "LRR-E-OWNERSHIP"),
        (
            "fn live()->i64 {let x=1_i64; x=2_i64; x}",
            "LRR-E-OWNERSHIP",
        ),
        (
            "fn live()->i64 {let x:i64; x=1_i64; x=2_i64; x}",
            "LRR-E-OWNERSHIP",
        ),
        (
            "fn live()->i64 {let mut x=1_i64; x=2_i32; x}",
            "LRR-E-OWNERSHIP",
        ),
        ("fn live()->i64 {let x; x=3+4; x}", "LRR-E-TYPE-CONTEXT"),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(run.poll(&mut db), RunPoll::Fault(error) if error.code==code),
            "{source}"
        );
    }
}

#[test]
fn assignment_evaluates_rhs_before_missing_or_immutable_destination_faults() {
    for source in [
        "fn live(){ missing = 127_i8+1_i8; }",
        "fn live(){ let x=1_i8; x=127_i8+1_i8; }",
        "fn live(){ missing.field = 127_i8+1_i8; }",
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(run.poll(&mut db), RunPoll::Panicked(report) if report.panic.kind==lrr::vm::numeric::PanicKind::Overflow)
        );
    }
}

#[test]
fn assignment_query_trace_enters_rhs_before_destination_commit_without_lhs_fetch() {
    use lrr::{
        db::{provider::SyntaxProvider, value::QueryValue},
        ir::pc::Phase,
        syntax::body_index::{ExpressionKind, NodeRole},
    };
    let (mut db, mut run, _) = start("fn live(){ missing=3_i64; }", "live", 1000);
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-UNSUPPORTED"));
    let events = db.trace.events.clone();
    let body = events
        .iter()
        .find_map(|event| match &event.key {
            QueryKey::BodyStructure(body) => Some(body.clone()),
            _ => None,
        })
        .unwrap();
    let QueryPoll::Ready(Ok(value)) = db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
    else {
        panic!("body unavailable")
    };
    let QueryValue::BodyStructure(index) = value.value.as_ref() else {
        panic!("wrong index value")
    };
    let assignment = index
        .nodes()
        .iter()
        .find(|node| {
            node.role == NodeRole::Expression(ExpressionKind::Other) && node.operands.len() == 2
        })
        .unwrap();
    let left = assignment.operands[0];
    let right = assignment.operands[1];
    let rhs=events.iter().position(|event|matches!(&event.key,QueryKey::Thunk(pc) if pc.node==right.0&&pc.phase==Phase::Entry.code().unwrap())).unwrap();
    let commit=events.iter().position(|event|matches!(&event.key,QueryKey::Thunk(pc) if pc.node==assignment.id.0&&pc.phase==Phase::Operand(1).code().unwrap())).unwrap();
    assert!(rhs < commit);
    assert!(
        events
            .iter()
            .all(|event| !matches!(&event.key,QueryKey::Thunk(pc) if pc.node==left.0))
    );
}

#[test]
fn lazy_if_selects_only_the_reached_arm_and_propagates_primitive_context() {
    for (source, expected) in [
        ("fn live()->i64 {if true {3+4} else {missing()}}", "7"),
        ("fn live()->i64 {if false {missing()} else {8}}", "8"),
        (
            "fn live()->i64 {if false {missing()} else if true {9} else {missing()}}",
            "9",
        ),
        (
            "fn live()->i64 {let mut x=1_i64; if false {x=99_i64;} x}",
            "1",
        ),
        (
            "fn live()->i64 {if {true} {return 11;} else {missing()} missing();}",
            "11",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(s) if s==expected)
        );
    }
    let (mut db, mut run, _) = start(
        "fn live()->f32{if false {missing()} else {1.0+2.0}}",
        "live",
        1000,
    );
    assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::F32(s) if s=="3"));
}

#[test]
fn short_circuit_boolean_ops_skip_cold_rhs_and_execute_selected_rhs_once() {
    for (source, expected) in [
        ("fn live()->bool{false && missing()}", false),
        ("fn live()->bool{true || missing()}", true),
        ("fn live()->bool{true && false}", false),
        ("fn live()->bool{false || true}", true),
        (
            "fn live()->bool{let mut x=0_i64; let b=true && {x=x+1;true}; (x==1_i64) && b}",
            true,
        ),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Bool(b) if b==expected));
    }
    let (mut db, mut run, _) = start("fn live()->bool{true && missing()}", "live", 1000);
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-NAME-MISSING"));
}

#[test]
fn cold_if_arm_and_short_circuit_rhs_have_zero_thunk_events() {
    use lrr::{
        db::{provider::SyntaxProvider, value::QueryValue},
        syntax::body_index::{ExpressionKind, NodeRole},
    };
    for (source, block) in [
        ("fn live()->i64{if true{7}else{missing()}}", true),
        ("fn live()->bool{false && missing()}", false),
        ("fn live()->bool{true || missing()}", false),
        ("fn live(){while false {missing();}}", true),
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        completed(run.poll(&mut db));
        let events = db.trace.events.clone();
        let body = events
            .iter()
            .find_map(|event| match &event.key {
                QueryKey::BodyStructure(body) => Some(body.clone()),
                _ => None,
            })
            .unwrap();
        let QueryPoll::Ready(Ok(value)) = db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
        else {
            panic!("body unavailable")
        };
        let QueryValue::BodyStructure(index) = value.value.as_ref() else {
            panic!("wrong body value")
        };
        let cold = if block {
            index
                .nodes()
                .iter()
                .rev()
                .find(|n| n.role == NodeRole::Expression(ExpressionKind::Block))
                .unwrap()
        } else {
            index
                .nodes()
                .iter()
                .find(|n| n.role == NodeRole::Expression(ExpressionKind::Call))
                .unwrap()
        };
        let cold_ids: Vec<_> = index
            .nodes()
            .iter()
            .filter(|n| n.range.start >= cold.range.start && n.range.end <= cold.range.end)
            .map(|n| n.id.0)
            .collect();
        assert!(!cold_ids.is_empty());
        assert!(
            events
                .iter()
                .all(|e| !matches!(&e.key,QueryKey::Thunk(pc) if cold_ids.contains(&pc.node)))
        );
    }
}

#[test]
fn if_without_else_requires_unit_only_for_the_executed_arm() {
    for source in ["fn live(){if true {3_i64;}}", "fn live(){if false {3_i64}}"] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(matches!(
            completed(run.poll(&mut db)).value,
            ValueSummary::Unit
        ));
    }
    for source in ["fn live(){if true {3_i64}}", "fn live(){if true {3}}"] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-OP-TYPE"));
    }
    let (mut db, mut run, _) = start("fn live(){if true {\"text\"}}", "live", 1000);
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-UNSUPPORTED"));
}

#[test]
fn while_reenters_fresh_iteration_storage_and_reuses_installed_thunks() {
    for source in [
        "fn live()->i64 {let mut x=0_i64; while x<10_i64 {let y=x; x=y+1_i64;} x}",
        "fn live()->i64 {let mut x=0_i64; while {x<10_i64} {let y=x; x=y+1_i64;} x}",
        "fn live()->i64 {let mut x=0_i64; while x<5_i64 {if true {x=x+1_i64;} else {missing()}} x+5_i64}",
    ] {
        let (mut db, mut run, _) = start(source, "live", 10000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value,ValueSummary::Signed(s) if s=="10"));
        assert!(report.opcode_counters.backedges_completed >= 5);
        assert_eq!(
            report.opcode_counters.backedges_completed,
            report.opcode_counters.backedge_polls
        );
        let mut seen = std::collections::BTreeSet::new();
        for event in &db.trace.events {
            if let QueryKey::Thunk(pc) = &event.key {
                assert!(seen.insert(pc.clone()), "installed thunk was queried again");
            }
        }
    }
}

#[test]
fn false_while_keeps_body_cold_and_unbounded_while_hits_dispatch_limit() {
    let (mut db, mut run, _) = start("fn live(){while false {missing();}}", "live", 1000);
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Unit
    ));
    let source = "fn live(){while true {}}";
    let (mut db, mut run, _) = start(source, "live", 100);
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
    let count = db.trace.events.len();
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-INTERNAL-IR"));
    assert_eq!(db.trace.events.len(), count);
}

#[test]
fn exhausted_dispatch_limit_does_not_resolve_the_next_cold_thunk() {
    let (mut db, mut run, _) = start("fn live(){missing();}", "live", 1);
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
    assert_eq!(
        db.trace
            .events
            .iter()
            .filter(|e| matches!(e.key, QueryKey::Thunk(_)))
            .count(),
        1
    );
}

#[test]
fn loop_break_values_and_continue_retire_selected_scopes() {
    for (source, expected) in [
        ("fn live()->i64 {loop {break 7; missing();}}", "7"),
        (
            "fn live()->i64 {let mut x=0_i64; loop {let y=x; x=y+1_i64; if x<5_i64 {continue;} break x;} }",
            "5",
        ),
        (
            "fn live()->i64 {let mut x=0_i64; while true {x=x+1_i64; if x==5_i64 {break;}} x}",
            "5",
        ),
        (
            "fn live()->i64 {let mut x=0_i64; while x<5_i64 {x=x+1_i64; continue; missing();} x}",
            "5",
        ),
        ("fn live()->i64 {loop {'block: {break 8;}}}", "8"),
        ("fn live()->i64 {loop {break {break 9; missing()};}}", "9"),
    ] {
        let (mut db, mut run, _) = start(source, "live", 10000);
        assert!(
            matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(s) if s==expected)
        );
    }
}

#[test]
fn nested_labeled_break_continue_and_block_break_choose_lexical_targets() {
    for (source, expected) in [
        (
            "fn live()->i64 {let mut x=0_i64; 'outer: loop {x=x+1_i64; loop {if x<3_i64 {continue 'outer;} break 'outer x;} }}",
            "3",
        ),
        (
            "fn live()->i64 {let mut x=0_i64; 'outer: while x<4_i64 {x=x+1_i64; loop {continue 'outer;}} x}",
            "4",
        ),
        (
            "fn live()->i64 {'block: {loop {break 'block 9;} missing();}}",
            "9",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "live", 10000);
        assert!(
            matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(s) if s==expected)
        );
    }
}

#[test]
fn labels_shadow_lexically_and_invalid_targets_fail_before_value_evaluation() {
    let (mut db, mut run, _) = start(
        "fn live()->i64{'same:loop{let x:i64='same:loop{break 'same 9;};break 'same x+1;}}",
        "live",
        10000,
    );
    assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(s) if s=="10"));
    for source in [
        "fn live(){loop{break 'missing (127_i8+1_i8);}}",
        "fn live(){'block:{continue 'block;}}",
        "fn live(){while true{break (127_i8+1_i8);}}",
        "fn live(){continue;}",
    ] {
        let (mut db, mut run, _) = start(source, "live", 1000);
        assert!(
            matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-UNSUPPORTED")
        );
    }
}

#[test]
fn source_fuel_charges_each_opcode_and_exit_and_exact_limits_are_deterministic() {
    let source = "fn main(){let x:i64=40_i64;x;}";
    let (mut db, mut run, _) = start(source, "main", 23);
    let report = completed(run.poll(&mut db));
    assert_eq!(report.opcode_counters.fuel_used, 23);
    assert_eq!(report.opcode_counters.cleanup_elements_completed, 3);
    assert_eq!(
        run.fuel_snapshot().used,
        report.opcode_counters.operations_started
            + report.opcode_counters.exits_completed
            + report.opcode_counters.cleanup_elements_completed
    );
    let (mut db, mut run, _) = start(source, "main", 22);
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
    assert_eq!(run.fuel_snapshot().used, 22);
    let (mut db, mut run, _) = start("fn live(){loop{}}", "live", 100);
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
    assert_eq!(run.fuel_snapshot().used, 100);
}

#[test]
fn source_cancellation_and_deadline_stop_before_thunk_fetch_or_guest_execution() {
    let (mut db, mut run, _) = start("fn live(){loop{}}", "live", 1000);
    let token = run.cancellation_token();
    std::thread::spawn(move || token.cancel()).join().unwrap();
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
    assert_eq!(run.fuel_snapshot().used, 0);
    assert_eq!(
        run.fuel_snapshot().stop,
        Some(lrr::vm::fuel::FuelStop::Cancelled)
    );
    assert!(
        db.trace
            .events
            .iter()
            .all(|e| !matches!(e.key, QueryKey::Thunk(_)))
    );
    let (mut db, mut run, _) = start("fn live(){loop{}}", "live", 1000);
    run.set_deadline(std::time::Instant::now());
    assert!(
        matches!(run.poll(&mut db),RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
    assert_eq!(
        run.fuel_snapshot().stop,
        Some(lrr::vm::fuel::FuelStop::Deadline)
    );
    assert_eq!(run.fuel_snapshot().used, 0);
}

#[test]
fn another_thread_cancels_a_guest_loop_after_observed_opcode_progress() {
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut db, mut run, _) = start("fn live(){loop{}}", "live", 1_000_000_000);
        send.send(run.cancellation_token()).unwrap();
        let poll = run.poll(&mut db);
        assert!(matches!(poll,RunPoll::Fault(error) if error.code=="LRR-E-RUNTIME-RESOURCE"));
        assert_eq!(
            run.fuel_snapshot().stop,
            Some(lrr::vm::fuel::FuelStop::Cancelled)
        );
        run.fuel_snapshot().used
    });
    let token = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let started = std::time::Instant::now();
    while token.fuel_charged() < 256 && started.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::yield_now();
    }
    let observed = token.fuel_charged();
    token.cancel();
    let used = worker.join().unwrap();
    assert!(observed >= 256);
    assert!(used >= observed);
}

#[test]
fn direct_source_calls_return_through_nested_continuations_and_leave_cold_signatures_unforced() {
    let (mut db, mut run, cold) = start(
        "fn main()->i64{outer()+leaf()} fn outer()->i64{leaf()+1i64} fn leaf<U:Missing>()->i64{20i64} fn cold<T:Absent>(x:Undefined)->Unknown{bad();}",
        "main",
        1000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value, ValueSummary::Signed(ref n) if n == "41"));
    assert_eq!(report.opcode_counters.started_by_opcode[19], 3);
    assert_eq!(report.opcode_counters.return_scopes_cleaned, 4);
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::FunctionSignature(def) if def == cold))
    );
}
#[test]
fn direct_source_call_missing_identity_and_local_shadow_do_not_enter_a_module_body() {
    for (source, code) in [
        (
            "fn main(){absent();} fn cold(){1i64;}",
            "LRR-E-NAME-MISSING",
        ),
        (
            "fn main(){let chosen=1i64;chosen();} fn chosen(){missing();} fn cold(){}",
            "LRR-E-UNSUPPORTED",
        ),
        (
            "fn main(){fn chosen(){} chosen();} fn chosen(){missing();} fn cold(){}",
            "LRR-E-CALL-CONTEXT",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("expected selected call fault");
        };
        assert_eq!(error.code, code);
        if code == "LRR-E-CALL-CONTEXT" {
            // This helper intentionally uses start_with_sources without a
            // completed lexical namespace context. The reached local item
            // must not fall back to the same-named module function.
            assert_eq!(
                error.message,
                "lexical callee requires exact entered source context"
            );
            assert_eq!(run.action_snapshot().published, 0);
        }
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(event.key, QueryKey::FunctionSignature(_)))
        );
    }
}
#[test]
fn direct_source_call_return_mismatch_faults_only_when_returned_and_recursion_uses_shared_fuel() {
    let (mut db, mut run, _) = start(
        "type i64=bool;fn main(){chosen();}fn chosen()->i64{true}fn cold(){}",
        "main",
        1000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
    assert_eq!(report.opcode_counters.started_by_opcode[22], 0);
    assert_eq!(report.opcode_counters.started_by_opcode[23], 0);
    assert_eq!(report.opcode_counters.return_scopes_cleaned, 2);
    db.finish_run().unwrap();
    let (mut db, mut run, _) = start(
        "fn main(){chosen();}fn chosen()->(){}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Unit
    ));
    let (mut db, mut run, _) = start(
        "fn main()->i64{wrong()} fn wrong()->i64{return 1u64;} fn cold()->Unknown{missing();}",
        "main",
        1000,
    );
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("expected reached return mismatch");
    };
    assert_eq!(error.code, "LRR-E-CALL-RETURN-TYPE");
    for source in [
        "fn main()->i64{wrong()} fn wrong()->i64{1u64} fn cold(){}",
        "fn main()->i64{wrong()} fn wrong()->i64{} fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("expected implicit return mismatch");
        };
        assert_eq!(error.code, "LRR-E-CALL-RETURN-TYPE");
    }
    let (mut db, mut run, _) = start(
        "fn main(){again();}fn again(){again();}fn cold(){}",
        "main",
        80,
    );
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("recursion must exhaust run fuel");
    };
    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(run.fuel_snapshot().used, 80);
    assert_eq!(
        run.fuel_snapshot().stop,
        Some(lrr::vm::fuel::FuelStop::Exhausted)
    );
}

#[test]
fn direct_source_call_panic_cleans_all_entered_frames_once_without_later_queries() {
    let (mut db, mut run, cold) = start(
        "fn main(){wrap();absent();} fn wrap(){let x=1i64;boom();absent();} fn boom()->i8{127i8+1i8} fn cold(){missing();}",
        "main",
        1000,
    );
    let RunPoll::Panicked(report) = run.poll(&mut db) else {
        panic!("expected callee arithmetic panic");
    };
    assert_eq!(report.panic.kind, lrr::vm::numeric::PanicKind::Overflow);
    // Three function blocks, boom's arithmetic tail, and two suspended call statements.
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 6);
    assert_eq!(report.opcode_counters.started_by_opcode[19], 2);
    assert!(
        db.trace
            .events
            .iter()
            .all(|e| !matches!(e.key, QueryKey::FunctionSignature(d) if d==cold))
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code == "LRR-E-INTERNAL-IR"));
}
#[test]
fn recursive_source_calls_hit_the_checked_metadata_limit_before_large_run_fuel() {
    let (mut db, mut run, _) = start(
        "fn main(){again();}fn again(){again();}fn cold(){}",
        "main",
        1_000_000,
    );
    run.set_call_limits(lrr::engine::run::CallLimits {
        max_frames: 100_000,
        max_metadata_bytes: 32 * 1024 * 1024,
    })
    .unwrap();
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("recursive call stack must be bounded");
    };
    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(error.message, "call stack metadata budget exceeded");
    assert!(run.fuel_snapshot().used < 1_000_000);
}

#[test]
fn written_call_arguments_use_parameter_hints_and_nested_call_contexts() {
    for (source, expected) in [
        (
            "fn main()->i64{add(20,22)}fn add(a:i64,b:i64)->i64{a+b}fn cold<T:Missing>(){absent();}",
            "42",
        ),
        (
            "fn main()->i64{add(leaf(20),leaf(21))}fn leaf(x:i64)->i64{x+1}fn add(a:i64,b:i64)->i64{a+b}fn cold(){}",
            "43",
        ),
        (
            "type Word=i64;fn main()->i64{echo(42)}fn echo(x:Word)->i64{x}fn cold(){}",
            "42",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(
            matches!(completed(run.poll(&mut db)).value, ValueSummary::Signed(n) if n==expected)
        );
    }
}
#[test]
fn effectful_written_arguments_execute_once_left_to_right_across_nested_calls() {
    let (mut db, mut run, _) = start(
        "fn main()->i64{let mut x=0i64;let y=add({x=x+1;x},{x=x+1;leaf(x)});y*10+x}fn add(a:i64,b:i64)->i64{a+b}fn leaf(x:i64)->i64{x}fn cold(){}",
        "main",
        1000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value, ValueSummary::Signed(n) if n=="32"));
    assert_eq!(report.opcode_counters.started_by_opcode[16], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[19], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[22], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[23], 3);
}
#[test]
fn excess_arguments_execute_before_arity_failure_and_missing_identity_precedes_arguments() {
    for (source, expected) in [
        (
            "fn main(){chosen(1i64,127i8+1i8);}fn chosen(x:i64){}fn cold(){}",
            "panic",
        ),
        (
            "fn main(){chosen(1i64,2i64);}fn chosen(x:i64){}fn cold(){}",
            "arity",
        ),
        ("fn main(){chosen(1);}fn chosen(){}fn cold(){}", "arity"),
        (
            "fn main(){chosen(1u64,127i8+1i8);}fn chosen(x:i64,y:i64){}fn cold(){}",
            "panic",
        ),
        ("fn main(){absent(127i8+1i8);}fn cold(){}", "missing"),
        (
            "fn main(){chosen(1u64);}fn chosen(x:i64){}fn cold(){}",
            "type",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        match (expected, run.poll(&mut db)) {
            ("panic", RunPoll::Panicked(p)) => {
                assert_eq!(p.panic.kind, lrr::vm::numeric::PanicKind::Overflow)
            }
            ("arity", RunPoll::Fault(e)) => assert_eq!(e.code, "LRR-E-CALL-ARITY"),
            ("missing", RunPoll::Fault(e)) => assert_eq!(e.code, "LRR-E-NAME-MISSING"),
            ("type", RunPoll::Fault(e)) => assert_eq!(e.code, "LRR-E-CALL-ARG-TYPE"),
            _ => panic!("wrong call boundary outcome"),
        }
    }
}

#[test]
fn returning_from_an_argument_retires_its_unfinished_call_context_without_replay() {
    let (mut db, mut run, _) = start(
        "fn main()->i64{let mut i=0i64;let mut x=0i64;while i<5i64{x=x+early();i=i+1;}x}fn early()->i64{add({return 2i64;},99)}fn add(a:i64,b:i64)->i64{a+b}fn cold(){}",
        "main",
        2000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value,ValueSummary::Signed(n) if n=="10"));
    assert_eq!(report.opcode_counters.started_by_opcode[19], 5);
    assert_eq!(report.opcode_counters.started_by_opcode[22], 5);
}

#[test]
fn generic_source_returns_use_runtime_substitution_without_forcing_unused_bounds() {
    for (source, expected) in [
        (
            "fn main()->i64{identity(42i64)}fn identity<T:Missing,U:Other>(x:T)->T{x}fn cold<T:Undefined>(){missing();}",
            "42",
        ),
        (
            "fn main()->i64{identity(42i64)}fn identity<'a,T:Missing>(x:T)->T{x}fn cold(){}",
            "42",
        ),
        (
            "fn main()->i64{make()}fn make<T:Missing>()->T{42i64}fn cold(){}",
            "42",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        let report = completed(run.poll(&mut db));
        assert!(matches!(report.value,ValueSummary::Signed(n) if n==expected));
        assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
    }
}
#[test]
fn generic_return_mismatch_is_checked_only_at_the_reached_return() {
    let (mut db, mut run, _) = start(
        "fn main()->i64{wrong(1i64)}fn wrong<T:Missing>(x:T)->T{true}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-CALL-RETURN-TYPE"));
    let (mut db, mut run, _) = start(
        "fn main(){never(1i64);}fn never<T:Missing>(x:T)->T{loop{}}fn cold(){}",
        "main",
        100,
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-RUNTIME-RESOURCE"));
    assert_eq!(run.fuel_snapshot().used, 100);
}

#[test]
fn generic_result_context_types_arguments_without_defaulting_and_reports_conflicts() {
    for source in [
        "fn main()->i64{identity(42)}fn identity<T:Missing>(x:T)->T{x}fn cold(){}",
        "fn main()->i64{make()}fn make<T:Missing>()->T{42}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(n) if n=="42"));
    }
    let (mut db, mut run, _) = start(
        "fn main()->i64{identity(42u64)}fn identity<T:Missing>(x:T)->T{x}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-GENERIC-BINDING"));
    let (mut db, mut run, _) = start(
        "fn main(){make();}fn make<T:Missing>()->T{42i64}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Unit
    ));
    let (mut db, mut run, _) = start(
        "fn main(){make();}fn make<T:Missing>()->T{42}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-TYPE-CONTEXT"));
}

#[test]
fn explicit_type_arguments_bind_scalar_hints_and_preserve_declared_parameter_ordinals() {
    for source in [
        "fn main()->i64{identity::<i64>(42)}fn identity<T:Missing,U:Other>(x:T)->T{x}fn cold(){}",
        "fn main()->i64{identity::<i64>(42)}fn identity<'a,T:Missing>(x:T)->T{x}fn cold(){}",
        "fn main()->i64{make::<i64>()}fn make<T:Missing>()->T{42}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(n) if n=="42"));
    }
    let (mut db, mut run, _) = start(
        "fn main(){make::<i64>();}fn make<T:Missing>()->T{42}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Unit
    ));
}
#[test]
fn explicit_type_aliases_resolve_in_the_caller_scope_before_primitive_fallback() {
    let (mut db, mut run, _) = start(
        "type Chosen=i64;mod inner{type Chosen=u64;pub fn identity<T:Missing>(x:T)->T{x}}use inner::identity;fn main()->i64{identity::<Chosen>(42)}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(n) if n=="42"));
    let (mut db, mut run, _) = start(
        "type i64=bool;fn main()->bool{identity::<i64>(true)}fn identity<T:Missing>(x:T)->T{x}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Bool(true)
    ));
}
#[test]
fn explicit_type_conflicts_and_missing_callee_are_checked_without_defaulting() {
    for source in [
        "fn main(){identity::<i64>(42u64);}fn identity<T:Missing>(x:T)->T{x}fn cold(){}",
        "fn main()->u64{identity::<i64>(42)}fn identity<T:Missing>(x:T)->T{x}fn cold(){}",
        "fn main(){identity::<i64,u64>(42);}fn identity<T:Missing>(x:T)->T{x}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-GENERIC-BINDING"));
    }
    let (mut db, mut run, _) = start(
        "fn main(){absent::<Missing>(127i8+1i8);}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-NAME-MISSING"));
    for source in [
        "fn main(){let identity=1i64;identity::<i64>(42);}fn identity<T>(x:T)->T{x}fn cold(){}",
        "type Choice=i64;fn main(){type Choice=bool;identity::<Choice>(true);}fn identity<T>(x:T)->T{x}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-UNSUPPORTED"));
    }
}

#[test]
fn generic_local_annotations_check_initialized_and_deferred_bindings() {
    for source in [
        "fn main()->i64{copy(42)}fn copy<T:Missing>(x:T)->T{let y:T=x;y}fn cold(){}",
        "fn main()->i64{copy(42)}fn copy<T:Missing>(x:T)->T{let mut y:T;y=x;y}fn cold(){}",
        "fn main()->i64{copy(42)}fn copy<'a,T:Missing>(x:T)->T{let y:T=x;{let z:T=y;z}}fn cold(){}",
        "fn main()->i64{make::<i64>()}fn make<T:Missing>()->T{let y:T=42;y}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(n) if n=="42"));
    }
}
#[test]
fn declared_generic_names_precede_builtin_spellings_in_body_annotations() {
    let (mut db, mut run, _) = start(
        "fn main()->bool{copy::<bool>(true)}fn copy<i64:Missing>(x:i64)->i64{let y:i64=x;y}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(
        completed(run.poll(&mut db)).value,
        ValueSummary::Bool(true)
    ));
}
#[test]
fn generic_local_annotations_reject_mismatch_unknown_context_and_local_type_shadow() {
    for (source, code) in [
        (
            "fn main()->i64{copy(42)}fn copy<T:Missing>(x:T)->T{let y:T=true;x}fn cold(){}",
            "LRR-E-OWNERSHIP",
        ),
        (
            "fn main()->i64{copy(42)}fn copy<T:Missing>(x:T)->T{let mut y:T=x;y=true;y}fn cold(){}",
            "LRR-E-OWNERSHIP",
        ),
        (
            "fn main(){make();}fn make<T:Missing>()->T{let y:T=42i64;y}fn cold(){}",
            "LRR-E-TYPE-CONTEXT",
        ),
        (
            "fn main()->i64{copy(42)}fn copy<T:Missing>(x:T)->T{type T=bool;let y:T=true;x}fn cold(){}",
            "LRR-E-UNSUPPORTED",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code==code));
    }
}

#[test]
fn caller_generic_parameters_forward_through_explicit_and_inferred_calls() {
    for source in [
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{inner::<T>(x)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{let y:T=x;inner::<T>(y)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{let y=x;inner::<T>(y)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{inner::<T>(inner(x))}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{let mut y:T;y=x;inner::<T>(y)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        let report = match run.poll(&mut db) {
            RunPoll::Completed(report) => report,
            RunPoll::Fault(e) => panic!("{source}: {} {}", e.code, e.message),
            _ => panic!("{source}: unexpected run outcome"),
        };
        assert!(
            matches!(report.value,ValueSummary::Signed(n) if n=="42"),
            "{source}"
        );
    }
}

#[test]
fn source_parameter_identity_is_distinct_from_equal_runtime_descriptors() {
    for source in [
        "fn main()->i64{outer(42,43i64)}fn outer<T:Missing,U:Missing>(x:T,y:U)->T{y}fn cold(){}",
        "fn main()->i64{outer(42,43i64)}fn outer<T:Missing,U:Missing>(x:T,y:U)->T{inner::<T>(y)}fn inner<V:Missing>(x:V)->V{x}fn cold(){}",
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{inner::<i64>(x)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(
            matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-GENERIC-BINDING"),
            "{source}"
        );
    }
}

#[test]
fn local_source_contracts_reject_distinct_rigid_initializers_and_replacements() {
    for source in [
        "fn main()->i64{outer(42,43i64)}fn outer<T:Missing,U:Missing>(x:T,y:U)->T{let z:T=y;z}fn cold(){}",
        "fn main()->i64{outer(42,43i64)}fn outer<T:Missing,U:Missing>(x:T,y:U)->T{let mut z:T;z=y;z}fn cold(){}",
        "fn main()->i64{outer(42,43i64)}fn outer<T:Missing,U:Missing>(x:T,y:U)->T{let mut z=x;z=y;z}fn cold(){}",
        "fn main()->i64{outer(42,43i64)}fn outer<T:Missing,U:Missing>(x:T,y:U)->T{let mut z;z=x;z=y;z}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(
            matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-GENERIC-BINDING"),
            "{source}"
        );
    }
}

#[test]
fn written_and_inferred_local_views_survive_assignment_and_shadowing() {
    for source in [
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{let mut z;z=x;inner::<T>(z)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
        "fn main()->i64{outer(42i64)}fn outer<T:Missing>(x:T)->i64{let z:i64=x;inner::<i64>(z)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{let mut z:T=x;z=x;let z=z;inner::<T>(z)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
        "fn main()->i64{outer(42)}fn outer<T:Missing>(x:T)->T{let z:T=42;inner::<T>(z)}fn inner<U:Missing>(x:U)->U{x}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        let report = match run.poll(&mut db) {
            RunPoll::Completed(report) => report,
            RunPoll::Fault(e) => panic!("{source}: {} {}", e.code, e.message),
            _ => panic!("{source}: unexpected outcome"),
        };
        assert!(
            matches!(report.value,ValueSummary::Signed(n) if n=="42"),
            "{source}"
        );
    }
}

#[test]
fn nested_source_calls_publish_and_consume_each_run_action_once() {
    let (mut db, mut run, _) = start(
        "fn main()->i64{sum(left(),right())}fn sum(a:i64,b:i64)->i64{a+b}fn left()->i64{40}fn right()->i64{2}fn cold(){}",
        "main",
        1000,
    );
    assert_eq!(run.action_snapshot().published, 0);
    assert!(matches!(completed(run.poll(&mut db)).value,ValueSummary::Signed(n) if n=="42"));
    let actions = run.action_snapshot();
    assert_eq!(actions.published, 3);
    assert_eq!(actions.consumed, actions.published);
    assert!(actions.pending.is_none());
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-INTERNAL-IR"));
    assert_eq!(run.action_snapshot(), actions);
}

#[test]
fn qualified_module_calls_preserve_relative_roots_aliases_and_generic_arguments() {
    for source in [
        "mod values{pub fn answer()->i64{42}}fn main()->i64{values::answer()}fn cold(){}",
        "fn answer()->i64{99}mod values{pub fn answer()->i64{40}}fn main()->i64{values::answer()+2i64}fn cold(){}",
        "mod values{pub fn answer()->i64{42}}fn main()->i64{let values=99i64;values::answer()}fn cold(){}",
        "mod values{pub fn answer()->i64{42}}fn main()->i64{crate::values::answer()}fn cold(){}",
        "mod values{pub fn answer()->i64{42}}fn main()->i64{self::values::answer()}fn cold(){}",
        "mod values{pub fn answer()->i64{42}}use values as alias;fn main()->i64{alias::answer()}fn cold(){}",
        "mod outer{fn answer()->i64{42}pub mod inner{pub fn run()->i64{super::answer()}}}fn main()->i64{outer::inner::run()}fn cold(){}",
        "mod values{pub fn identity<T:Missing>(x:T)->T{x}}fn main()->i64{values::identity::<i64>(42)}fn cold(){}",
        "mod r#mod{pub fn r#type()->i64{42}}fn main()->i64{r#mod::r#type()}fn cold(){}",
        "mod r#mod{pub fn r#type()->i64{42}pub fn answer()->i64{self::r#type()}}fn main()->i64{r#mod::answer()}fn cold(){}",
        "mod café{pub fn r#gen()->i64{42}}fn main()->i64{cafe\u{301}::r#gen()}fn cold(){}",
        "pub mod values{pub fn answer()->i64{42}}mod layer{pub use crate::values as alias;}fn main()->i64{layer::alias::answer()}fn cold(){}",
        "mod values{pub fn answer()->i64{42}}fn main()->i64{fn answer(){} values::answer()}fn cold(){}",
        "mod values{pub fn answer()->i64{42}}fn main()->i64{mod values{} crate::values::answer()}fn cold(){}",
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        let report = match run.poll(&mut db) {
            RunPoll::Completed(report) => report,
            RunPoll::Fault(e) => panic!("{source}: {} {}", e.code, e.message),
            _ => panic!("{source}: unexpected outcome"),
        };
        assert!(
            matches!(report.value,ValueSummary::Signed(n) if n=="42"),
            "{source}"
        );
    }
}
#[test]
fn qualified_callee_identity_faults_before_arguments_and_local_roots_do_not_fallback() {
    for (source, code) in [
        (
            "fn main(){missing::answer(127i8+1i8);}fn cold(){}",
            "LRR-E-NAME-MISSING",
        ),
        (
            "mod hidden{fn answer(a:i8)->i64{42}}fn main(){hidden::answer(127i8+1i8);}fn cold(){}",
            "LRR-E-UNSUPPORTED",
        ),
        (
            "mod values{}fn main(){crate::values::answer(127i8+1i8);}fn cold(){}",
            "LRR-E-NAME-MISSING",
        ),
        (
            "mod values{pub fn answer()->i64{42}}fn main(){mod values{} values::answer();}fn cold(){}",
            "LRR-E-UNSUPPORTED",
        ),
        (
            "mod values{pub fn answer()->i64{42}}fn main(){struct values;values::answer();}fn cold(){}",
            "LRR-E-UNSUPPORTED",
        ),
        (
            "mod values{pub fn answer()->i64{42}}fn main(){values::<i64>::answer();}fn cold(){}",
            "LRR-E-UNSUPPORTED",
        ),
    ] {
        let (mut db, mut run, _) = start(source, "main", 1000);
        assert!(
            matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code==code),
            "{source}"
        );
        assert_eq!(run.action_snapshot().published, 0);
    }
}
#[test]
fn qualified_generic_source_owner_cannot_fallback_to_a_same_named_module() {
    let (mut db, mut run, _) = start(
        "mod T{pub fn answer(){missing();}}fn main(){outer::<i64>(42);}fn outer<T:Missing>(x:T){T::answer();}fn cold(){}",
        "main",
        1000,
    );
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-UNSUPPORTED"));
    assert_eq!(run.action_snapshot().published, 1);
}

#[test]
fn declaration_markers_leave_item_initializers_and_bodies_unprepared() {
    let (mut db, mut run, _) = start(
        "fn main()->i64{const BAD:Unknown=missing();static STATE:Unknown=other();fn local(x:Unknown)->Missing{never();}mod hidden{pub fn answer(){missing();}}type Alias=Unresolved;struct S{field:Unknown}enum E{V(Unknown)}union U{field:i64}trait Tr{fn method(&self)->Unknown;}impl S{fn method(){missing();}}use hidden as unused;42i64}fn cold(){}",
        "main",
        1000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value,ValueSummary::Signed(n) if n=="42"));
    assert_eq!(report.opcode_counters.started_by_opcode[24], 11);
    assert_eq!(report.opcode_counters.started_by_opcode[0], 1);
    assert_eq!(run.action_snapshot().published, 0);
    assert!(db.trace.events.iter().all(|event| !matches!(
        event.key,
        QueryKey::FunctionSignature(_) | QueryKey::Constant(_)
    )));
    assert_eq!(
        db.trace
            .events
            .iter()
            .filter_map(|event| match &event.key {
                QueryKey::BodyStructure(key) => Some(key.clone()),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1
    );
}

#[test]
fn dispatcher_recursion_executes_factorial_with_concrete_boundaries() {
    let (mut db, mut run, _) = start(
        "fn main()->i64{fact(20)}fn fact(n:i64)->i64{if n==0{1}else{n*fact(n-1)}}fn cold<Unused:Missing>(){bad();}",
        "main",
        100_000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value,ValueSummary::Signed(n) if n=="2432902008176640000"));
    assert_eq!(run.action_snapshot().published, 21);
    assert_eq!(run.action_snapshot().consumed, 21);
}
#[test]
fn two_thousand_guest_frames_complete_on_a_bounded_host_thread_stack() {
    let report=std::thread::Builder::new().name("lrr-2000-frame-oracle".into()).stack_size(256*1024).spawn(|| {
        let (mut db,mut run,_)=start("fn main()->i64{down(2000)}fn down(n:i64)->i64{if n==0{0}else{down(n-1)}}fn cold(){}","main",1_000_000);
        run.set_call_limits(lrr::engine::run::CallLimits{max_frames:4096,max_metadata_bytes:128*1024*1024}).unwrap();
        let report=completed(run.poll(&mut db));
        assert_eq!(run.action_snapshot().published,2001);
        assert_eq!(run.action_snapshot().consumed,2001);
        assert_eq!(run.peak_guest_frames(),2002);
        report
    }).unwrap().join().unwrap();
    assert!(matches!(report.value,ValueSummary::Signed(n) if n=="0"));
}

#[test]
fn guest_frame_limit_faults_before_publication_without_host_recursion() {
    let (mut db, mut run, _) = start(
        "fn main(){again();}fn again(){again();}fn cold(){}",
        "main",
        1_000_000,
    );
    run.set_call_limits(lrr::engine::run::CallLimits {
        max_frames: 64,
        max_metadata_bytes: 32 * 1024 * 1024,
    })
    .unwrap();
    let RunPoll::Fault(error) = run.poll(&mut db) else {
        panic!("frame quota must stop recursion")
    };
    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(error.message, "guest frame limit exceeded");
    assert_eq!(run.action_snapshot().published, 63);
    assert_eq!(run.action_snapshot().consumed, 63);
    assert_eq!(run.peak_guest_frames(), 64);
    assert!(run.action_snapshot().pending.is_none());
    assert!(run.fuel_snapshot().remaining > 0);
}
#[test]
fn invalid_and_late_call_limits_preserve_the_accepted_run_configuration() {
    use lrr::engine::run::CallLimits;
    let (mut db, mut run, _) = start("fn main()->i64{42}fn cold(){}", "main", 1000);
    let defaults = run.call_limits();
    for limits in [
        CallLimits {
            max_frames: 0,
            ..defaults
        },
        CallLimits {
            max_frames: 100_001,
            ..defaults
        },
        CallLimits {
            max_metadata_bytes: 0,
            ..defaults
        },
        CallLimits {
            max_metadata_bytes: 256 * 1024 * 1024 + 1,
            ..defaults
        },
    ] {
        assert_eq!(
            run.set_call_limits(limits).unwrap_err().code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(run.call_limits(), defaults);
    }
    completed(run.poll(&mut db));
    assert_eq!(
        run.set_call_limits(defaults).unwrap_err().code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(run.call_limits(), defaults);
}

#[test]
fn cancellation_of_recursive_dispatch_preserves_exact_once_action_counts() {
    let (mut db, mut run, _) = start(
        "fn main(){again();}fn again(){again();}fn cold(){}",
        "main",
        1_000_000,
    );
    run.set_call_limits(lrr::engine::run::CallLimits {
        max_frames: 100_000,
        max_metadata_bytes: 128 * 1024 * 1024,
    })
    .unwrap();
    let token = run.cancellation_token();
    let interrupter = std::thread::spawn(move || {
        while token.fuel_charged() < 64 {
            std::thread::yield_now();
        }
        token.cancel();
    });
    let outcome = run.poll(&mut db);
    interrupter.join().unwrap();
    assert!(
        matches!(outcome,RunPoll::Fault(e) if e.code=="LRR-E-RUNTIME-RESOURCE" && e.message=="guest run cancellation requested")
    );
    let actions = run.action_snapshot();
    assert!(actions.published > 0);
    assert_eq!(actions.published, actions.consumed);
    assert!(actions.pending.is_none());
    assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-INTERNAL-IR"));
    assert_eq!(run.action_snapshot(), actions);
}

#[test]
fn interrupted_recursive_arguments_keep_completed_effects_once_and_later_callees_cold() {
    for cancelled in [false, true] {
        let (mut db, mut run, cold) = start(
            "fn main()->i64{let mut x=0i64;add({x=x+1;spin()},cold())}fn add(a:i64,b:i64)->i64{a+b}fn spin()->i64{spin()}fn cold()->i64{missing();}",
            "main",
            if cancelled { 1_000_000 } else { 1200 },
        );
        let interrupter = if cancelled {
            let token = run.cancellation_token();
            Some(std::thread::spawn(move || {
                while token.fuel_charged() < 128 {
                    std::thread::yield_now();
                }
                token.cancel();
            }))
        } else {
            None
        };
        let outcome = run.poll(&mut db);
        if let Some(thread) = interrupter {
            thread.join().unwrap();
        }
        let RunPoll::Fault(error) = outcome else {
            panic!("interruption must fault")
        };
        assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
        assert_eq!(
            error.message,
            if cancelled {
                "guest run cancellation requested"
            } else {
                "guest opcode fuel exhausted"
            }
        );
        let counters = run.opcode_counters().unwrap();
        assert_eq!(counters.started_by_opcode[16], 1);
        assert!(run.action_snapshot().published > 0);
        assert_eq!(
            run.action_snapshot().published,
            run.action_snapshot().consumed
        );
        assert!(db.trace.events.iter().all(
            |event| !matches!(event.key,QueryKey::FunctionSignature(definition) if definition==cold)
        ));
        assert!(matches!(run.poll(&mut db),RunPoll::Fault(e) if e.code=="LRR-E-INTERNAL-IR"));
        let terminal = run.opcode_counters().unwrap();
        assert_eq!(terminal.operations_started, counters.operations_started);
        assert_eq!(terminal.started_by_opcode, counters.started_by_opcode);
    }
}
#[test]
fn run_counter_prefixes_are_not_duplicated_when_suspended_callers_resume() {
    let (mut db, mut run, _) = start(
        "fn main()->i64{let mut x=0i64;let a=leaf({x=x+1;x});let b=leaf({x=x+1;x});a+b+x}fn leaf(x:i64)->i64{x}fn cold(){}",
        "main",
        1000,
    );
    let report = completed(run.poll(&mut db));
    assert!(matches!(report.value,ValueSummary::Signed(n) if n=="5"));
    assert_eq!(report.opcode_counters.started_by_opcode[16], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[19], 2);
    assert_eq!(report.opcode_counters.started_by_opcode[21], 2);
    assert_eq!(
        run.opcode_counters().unwrap().started_by_opcode,
        report.opcode_counters.started_by_opcode
    );
}
