//! Executes unchanged official rustc numeric main programs; no compiletest/native claim.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};

#[derive(Clone, Copy)]
enum Frontier {
    OperandPreparation(&'static str, &'static str),
    FloatingMethod,
}
impl Frontier {
    fn diagnostic(self) -> (&'static str, &'static str) {
        match self {
            Self::OperandPreparation(code, message) => (code, message),
            Self::FloatingMethod => (
                "LRR-E-UNSUPPORTED",
                "reached assertion operand requires pending non-scalar expression integration",
            ),
        }
    }
}

fn run_original(source: &str, label: &str) {
    run_source(source, label, None);
}

// Exact-frontier controls exercise original source but never certify a full
// original fixture pass. Unrelated errors, panics and completions fail the check.
fn run_source(source: &str, label: &str, frontier: Option<Frontier>) {
    for edition in [2015, 2018, 2021, 2024] {
        let owner = Digest::of_bytes(label.as_bytes());
        let cfg = CfgSet::guest_default();
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let path = SourcePath::new("original.rs").unwrap();
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: owner,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: owner,
                module_path: vec![],
                profile: profile.clone(),
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("unchanged stock source selection unavailable");
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
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"LRR stock numeric discovery"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"LRR stock numeric declarative environment"),
            Digest::of_bytes(b"LRR stock numeric compiler facade"),
            std::sync::Arc::new(
                lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                    .unwrap(),
            ),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => break,
                SourceDiscoveryStep::NeedExpansion { request } => {
                    discovery
                        .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                        .unwrap();
                }
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                    discovery
                        .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                        .unwrap();
                }
                SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => {
                    panic!("{label} edition{edition} original discovery: {diagnostic}; {blocked:?}")
                }
                _ => panic!("original requires unsupported procedural/derive authority"),
            }
        }
        let macros =
            lrr::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
                &view, &discovery,
            )
            .unwrap();
        let contexts = macros.contexts().unwrap();
        db.begin_run().unwrap();
        let mut machine =
            match RunMachine::start_with_discovery(&mut db, main.clone(), 100_000, discovery) {
                QueryPoll::Ready(Ok(machine)) => machine,
                QueryPoll::Ready(Err(error)) => {
                    panic!("{label} edition{edition} original startup: {error}")
                }
                QueryPoll::NeedInput(input) => {
                    panic!("{label} edition{edition} original startup requested {input:?}")
                }
            };
        let report = match machine.poll_with_runtime_macros(&mut db, &contexts) {
            RunPoll::Completed(report) => {
                assert!(
                    frontier.is_none(),
                    "known frontier unexpectedly completed; update full-fixture acceptance"
                );
                report
            }
            RunPoll::Fault(error) if frontier.is_some() => {
                let frontier = frontier.unwrap();
                let (code, message) = frontier.diagnostic();
                assert_eq!(error.code, code, "{label} edition{edition} frontier code");
                assert_eq!(
                    error.message, message,
                    "{label} edition{edition} frontier message"
                );
                let counters = machine.opcode_counters().unwrap();
                match frontier {
                    Frontier::OperandPreparation(_, _) => {
                        assert_eq!(counters.started_by_opcode[9], 0);
                        assert_eq!(counters.started_by_opcode[10], 0);
                    }
                    Frontier::FloatingMethod => {
                        assert_eq!(counters.started_by_opcode[41], 1);
                        assert_eq!(counters.started_by_opcode[7], 1);
                        assert_eq!(counters.started_by_opcode[26], 0);
                        assert_eq!(counters.started_by_opcode[10], 0);
                        let stack = error.guest_stack.as_ref().expect("original method origin");
                        assert_eq!(stack.frames[0].body.body_key, main);
                        assert!(stack.frames[0].locations.iter().any(|at| {
                            at.source.path == "original.rs"
                                && source.get(at.range.start as usize..at.range.end as usize)
                                    == Some("assert!(nan.is_nan())")
                        }));
                    }
                }
                assert_eq!(counters.panic_scopes_cleaned, 0);
                db.finish_run().unwrap();
                continue;
            }
            RunPoll::Fault(error) => panic!(
                "{label} edition{edition} original fault: {error}; counters {:?}; last queries {:?}",
                machine.opcode_counters().unwrap(),
                db.trace
                    .events
                    .iter()
                    .rev()
                    .take(8)
                    .map(|event| &event.key)
                    .collect::<Vec<_>>()
            ),
            RunPoll::Panicked(report) => panic!(
                "{label} edition{edition} original guest panic: {:?}",
                report.panic.kind
            ),
            RunPoll::NeedInput(input) => {
                panic!("{label} edition{edition} original requested {input:?}")
            }
        };
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.live_edition, edition);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        assert!(report.opcode_counters.operations_completed > 0);
        db.finish_run().unwrap();
    }
}

#[test]
fn original_float_literals() {
    run_original(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/floatlits.rs"
        ),
        "tests/ui/numbers-arithmetic/floatlits.rs",
    );
}
#[test]
fn original_signed_i8_increment() {
    run_original(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/i8-incr.rs"
        ),
        "tests/ui/numbers-arithmetic/i8-incr.rs",
    );
}
#[test]
fn component_original_mixed_signed_unsigned_shift_counts_exact_unsupported_frontier() {
    run_source(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/shift-various-types.rs"
        ),
        "tests/ui/numbers-arithmetic/shift-various-types.rs",
        Some(Frontier::OperandPreparation(
            "LRR-E-UNSUPPORTED",
            "argument source hints require further type integration",
        )),
    );
}
#[test]
fn component_original_float_nan_exact_unsupported_frontier() {
    run_source(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/float-nan.rs"
        ),
        "tests/ui/numbers-arithmetic/float-nan.rs",
        Some(Frontier::FloatingMethod),
    );
}
#[test]
fn original_u8_to_char() {
    run_original(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/cast/u8-to-char-cast-9918.rs"
        ),
        "tests/ui/cast/u8-to-char-cast-9918.rs",
    );
}
#[test]
fn original_cast_does_fallback() {
    run_original(
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cast/cast-does-fallback.rs"),
        "tests/ui/cast/cast-does-fallback.rs",
    );
}
#[test]
fn original_type_limit_comparisons() {
    run_original(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/numeric/type-limit-comparisons-6130.rs"
        ),
        "tests/ui/numeric/type-limit-comparisons-6130.rs",
    );
}

/// LRR-specific operator/loan integration, supplementary to unchanged fixtures.
#[test]
fn component_forwarded_shared_primitive_operator_matrix() {
    run_original(
        r#"fn main() {
        let r0: i8 = &3i8 + 2i8;
        assert_eq!(r0, 5i8);
        let r1: i16 = 7i16 - &4i16;
        assert_eq!(r1, 3i16);
        let r2: f32 = &3f32 * &3f32;
        assert_eq!(r2, 9f32);
        let r3: f64 = &6f64 / 2f64;
        assert_eq!(r3, 3f64);
        let r4: i64 = 7i64 % &4i64;
        assert_eq!(r4, 3i64);
        let r5: u8 = &10u8 | &12u8;
        assert_eq!(r5, 14u8);
        let r6: u16 = &10u16 & 12u16;
        assert_eq!(r6, 8u16);
        let r7: u32 = 10u32 ^ &12u32;
        assert_eq!(r7, 6u32);
        let r8: u64 = &10u64 << &2u32;
        assert_eq!(r8, 40u64);
        let r9: u64 = &10u64 >> &2i16;
        assert_eq!(r9, 2u64);
        let r10: i32 = -&3i32;
        assert_eq!(r10, -3i32);
        let r11: u8 = !&3u8;
        assert_eq!(r11, 252u8);
        let r12: bool = !&true;
        assert_eq!(r12, false);
    }"#,
        "LRR component primitive operator reference forwarding",
    );
}

#[test]
fn component_reference_cast_is_not_primitive_operator_forwarding() {
    run_source(
        "fn main() { let _ = (&42u8) as usize; }",
        "LRR component nonprimitive reference cast refusal",
        Some(Frontier::OperandPreparation(
            "LRR-E-VALUE-REPR",
            "guest value kind, descriptor, or payload invariant failed",
        )),
    );
}
