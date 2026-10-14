//! Direct later-use numeric source constraints; no general-inference claim.
//! Original native error-pattern formatting/compiletest remains unexecuted.
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
enum Expected {
    Unit,
    Fault(&'static str, &'static str),
    NumericThenWildcardFrontier,
}

fn run_program(source: &str, label: &str, expected: Expected) {
    run_world(source, label, expected, &[]);
}

fn run_world(source: &str, label: &str, expected: Expected, modules: &[(&str, &str, &str)]) {
    for edition in [2015, 2018, 2021, 2024] {
        let owner = Digest::of_bytes(label.as_bytes());
        let cfg = CfgSet::guest_default();
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let mut units = Vec::new();
        let mut selections = Vec::new();
        let mut sources = vec![("original.rs", None, source)];
        sources.extend(
            modules
                .iter()
                .map(|(path, module, text)| (*path, Some(*module), *text)),
        );
        for (path, module, text) in sources {
            let path = SourcePath::new(path).unwrap();
            let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
            let module_path = module.into_iter().map(str::to_owned).collect::<Vec<_>>();
            selections.push(SourceSelection {
                path: path.clone(),
                outline: OutlineKey {
                    parse: ParseKey {
                        content: blob.content(),
                        profile: profile.clone(),
                    },
                    crate_key: owner,
                    module_path: module_path.clone(),
                    cfg: cfg.fingerprint(),
                },
            });
            units.push(SourceUnitInput {
                path,
                crate_key: owner,
                module_path,
                profile: profile.clone(),
                cfg: cfg.clone(),
                blob,
            });
        }
        let inputs = PreparedSourceInputs::refresh(None, units, Default::default()).unwrap();
        let views = selections
            .iter()
            .map(|selection| {
                let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap()
                else {
                    panic!("recorded original source world unavailable");
                };
                view
            })
            .collect::<Vec<_>>();
        let view = &views[0];
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
            views.iter().map(|view| view.share()).collect(),
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
                view, &discovery,
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
                    matches!(expected, Expected::Unit),
                    "{label} edition{edition}: expected arithmetic panic did not occur"
                );
                report
            }
            RunPoll::Fault(error) if matches!(expected, Expected::NumericThenWildcardFrontier) => {
                assert_eq!(error.code, "LRR-E-UNSUPPORTED");
                assert_eq!(
                    error.message,
                    "wildcard place requires non-moving type integration"
                );
                let counters = machine.opcode_counters().unwrap();
                assert_eq!(counters.started_by_opcode[10], 1);
                assert_eq!(counters.started_by_opcode[7], 2);
                assert_eq!(counters.started_by_opcode[8], 1);
                // The explicit Unsupported operation is charged once and never
                // completes; both earlier bindings and arithmetic committed.
                assert_eq!(counters.started_by_opcode[6], 1);
                assert_eq!(
                    counters.operations_started,
                    counters.operations_completed + 1
                );
                assert_eq!(counters.panic_scopes_cleaned, 0);
                let stack = error
                    .guest_stack
                    .as_ref()
                    .expect("exact reached source origin");
                assert_eq!(stack.frames[0].body.body_key, main);
                assert!(stack.frames[0].locations.iter().any(|at| {
                    at.source.path == "original.rs"
                        && source.get(at.range.start as usize..at.range.end as usize)
                            == Some("let _:i64=later;")
                }));
                db.finish_run().unwrap();
                continue;
            }
            RunPoll::Fault(error) if matches!(expected, Expected::Fault(_, _)) => {
                let Expected::Fault(code, message) = expected else {
                    unreachable!()
                };
                assert_eq!(
                    error.code, code,
                    "{label} edition{edition} exact refusal code"
                );
                assert_eq!(
                    error.message, message,
                    "{label} edition{edition} exact refusal message"
                );
                let counters = machine.opcode_counters().unwrap();
                assert_eq!(
                    counters.started_by_opcode[10], 0,
                    "refused source inference must precede arithmetic"
                );
                assert_eq!(
                    counters.started_by_opcode[0], 0,
                    "operator operands remain cold"
                );
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
            RunPoll::Panicked(report) => {
                panic!(
                    "{label} edition{edition}: unexpected guest panic {:?}",
                    report.panic.kind
                );
            }
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

const NON_DIRECT: &str = "closed numeric binding awaits non-direct local-use type inference";
const SYMBOLIC: &str = "closed numeric binding annotation requires symbolic type resolution";
const CONFLICT: &str = "closed numeric binding has incompatible direct source constraints";

#[test]
fn original_infer_binary_operand_behind_reference_whole_main_unchanged() {
    run_program(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/inference/infer-binary-operand-behind-reference.rs"
        ),
        "stock binary reference inference whole original",
        Expected::Unit,
    );
}
#[test]
fn original_cast_does_fallback_whole_main_unchanged() {
    run_program(
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cast/cast-does-fallback.rs"),
        "stock cast does not constrain operand whole original",
        Expected::Unit,
    );
}
#[test]
fn component_direct_later_numeric_annotations_constrain_closed_operator_binding() {
    for source in [
        "fn main(){let x=3+4;let later:i64=x;}",
        "fn main(){let total=((3+4));let later:u64=((total));}",
        "fn main(){let value=200+100;let later:u16=value;}",
        "fn main(){let value=1.25+2.25;let later:f32=value;}",
        "fn main(){let value=(&3+&4);let later:i64=value;}",
        "fn main(){let r#value=3+4;let later:i64=value;let another:i64=r#value;}",
        "fn main(){let value=3+4;let later:i64=value;fn cold(){missing_body_dependency();}}",
    ] {
        run_program(
            source,
            "LRR direct later source numeric constraint",
            Expected::Unit,
        );
    }
}
#[test]
fn component_direct_numeric_commits_before_known_wildcard_place_frontier() {
    // This observes a precise unsupported frontier, not whole-program success.
    run_program(
        "fn main(){let value=3+4;let later:i64=value;let _:i64=later;}",
        "LRR direct numeric constraint before wildcard place",
        Expected::NumericThenWildcardFrontier,
    );
}
#[test]
fn component_opaque_uses_keep_operator_operands_and_producer_bodies_cold() {
    for source in [
        "fn producer(x:i64)->i64{missing_body_dependency();x} fn main(){let x=3+4;let alias=producer(x);let later:i64=x;}",
        "fn main(){let x=3+4;let alias=x;let later:i64=x;}",
        "fn main(){let mut x=3+4;let later:i64=x;}",
        "fn main(){let x=3+4;assert_eq!(x,7_i64);let later:i64=x;}",
        "fn main(){let x=3+4;let later:i64=x;let x=5_i64;}",
        "fn main(){let x=3+4;let (x,)=(5_i64,);let later:i64=x;}",
        "fn main(){let x=3+4;let ignored={let inner=x;inner};let later:i64=x;}",
    ] {
        run_program(
            source,
            "LRR unknown local constraint frontier",
            Expected::Fault("LRR-E-UNSUPPORTED", NON_DIRECT),
        );
    }
}
#[test]
fn component_symbolic_and_conflicting_annotations_refuse_without_operand_erasure() {
    for source in [
        "type i64=u8;fn main(){let x=3+4;let later:i64=x;}",
        "macro_rules! types {()=>{type i64=u8;}} types!(); fn main(){let x=3+4;let later:i64=x;}",
        "mod types{pub type Alias=u8;} use types::Alias as i64; fn main(){let x=3+4;let later:i64=x;}",
        "fn main(){let x=3+4;type i64=u8;let later:i64=x;}",
    ] {
        run_program(
            source,
            "LRR symbolic later type frontier",
            Expected::Fault("LRR-E-UNSUPPORTED", SYMBOLIC),
        );
    }
    run_program(
        "fn main(){let x=3+4;let a:i64=x;let b:i32=x;}",
        "LRR conflicting direct constraints",
        Expected::Fault("LRR-E-UNSUPPORTED", CONFLICT),
    );
}
#[test]
fn component_shadow_binding_identity_and_written_types_preserve_source_types() {
    for source in [
        "fn main(){let x=200+100;let x=1_u8;let later:u8=x;}",
        "fn main(){let x:i32=3+4;let later:i32=x;}",
        "fn main(){let x=3_i64+4_i64;let later:i64=x;}",
        "fn main(){let x=3+4;let x=5_i64;let later:i64=x;}",
        "fn main(){let x=3+4;return;producer();} fn producer(){missing_body_dependency();}",
    ] {
        run_program(
            source,
            "LRR exact binding/source type control",
            Expected::Unit,
        );
    }
}

#[test]
fn component_direct_let_else_rejects_opaque_context_before_numeric_operands() {
    for source in [
        "fn producer(_:i64){missing_body_dependency();} fn main(){let x=3+4;let later:i64=x else {producer(x);return;};}",
        "fn producer(_:i64){missing_body_dependency();} fn main(){let x=3+4;let other:i8=0 else {producer(x);return;};let later:i64=x;}",
    ] {
        run_program(
            source,
            "LRR let-else leaves producer and operands cold",
            Expected::Fault("LRR-E-UNSUPPORTED", NON_DIRECT),
        );
    }
}
