//! Folded unary-negation source controls. Static rustc diagnostics and the
//! complete suffix-inference program are not counted as LRR full passes.
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
    engine::run::{RunMachine, RunPoll, ValueSummary},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};

const UNSIGNED: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/unary-negation-unsigned-integer-error.rs"
);

fn run(source: &str, fault: Option<&str>, failed_opcode: Option<usize>, exact_prefix_start: usize) {
    for edition in [2015, 2018, 2021, 2024] {
        let path = SourcePath::new("folded-negative.rs").unwrap();
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let cfg = CfgSet::guest_default();
        let owner = Digest::of_bytes(b"LRR folded unary negation context acceptance");
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
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("captured source absent")
        };
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
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
            .and_then(|item| item.body.as_ref())
            .map(|body| body.key());
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        let source_revision = db.source_revision();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_sources(&mut db, body.clone(), 100_000, vec![view])
        else {
            panic!("captured source startup incomplete")
        };
        match machine.poll(&mut db) {
            RunPoll::Completed(report) => {
                assert!(
                    fault.is_none(),
                    "edition{edition}: unsigned negation unexpectedly completed"
                );
                assert!(matches!(report.value, ValueSummary::Unit));
                assert_eq!(report.live_edition, edition);
                assert_eq!(
                    report.opcode_counters.operations_started,
                    report.opcode_counters.operations_completed
                );
            }
            RunPoll::Fault(error) => {
                assert_eq!(error.code, "LRR-E-OP-TYPE", "edition{edition}: {error}");
                let spelling = fault.expect("positive source unexpectedly refused");
                let stack = error
                    .guest_stack
                    .as_ref()
                    .expect("refusal retains entered source");
                assert_eq!(stack.frames[0].body.body_key, body);
                assert!(
                    stack.frames[0].locations.iter().any(|at| {
                        at.source.path == "folded-negative.rs"
                            && at.range.start as usize == exact_prefix_start
                            && source.get(at.range.start as usize..at.range.end as usize)
                                == Some(spelling)
                    }),
                    "unsigned inference refusal must attach to the exact requested original prefix"
                );
                assert!(
                    stack.frames[0].locations.iter().any(|at| at.source.path
                        == "folded-negative.rs"
                        && source
                            .get(at.range.start as usize..at.range.end as usize)
                            .is_some_and(|text| text.contains(spelling))),
                    "refusal lost exact negative source spelling"
                );
                let counters = machine.opcode_counters().unwrap();
                if let Some(opcode) = failed_opcode {
                    assert_eq!(
                        counters.started_by_opcode[opcode], 0,
                        "failed opcode must remain uncharged"
                    );
                }
                assert_eq!(counters.operations_started, counters.operations_completed);
                assert_eq!(counters.cleanup_elements_completed, 0);
                assert_eq!(
                    counters.fuel_used,
                    counters
                        .operations_started
                        .checked_add(counters.exits_completed)
                        .unwrap()
                );
                assert_eq!(counters.panic_scopes_cleaned, 0);
            }
            RunPoll::NeedInput(input) => panic!("unexpected reached source suspension: {input:?}"),
            RunPoll::Panicked(report) => panic!("unexpected guest panic: {:?}", report.panic.kind),
        }
        if let Some(cold) = cold {
            assert_eq!(db.trace.truncated, 0);
            assert!(
                !db.trace.events.iter().any(|event| match &event.key {
                    QueryKey::BodyStructure(body) => body == &cold,
                    QueryKey::Thunk(pc) => pc.body == cold,
                    _ => false,
                }),
                "unreached unsigned-zero body must remain cold"
            );
        }
        assert_eq!(db.source_revision(), source_revision);
        db.finish_run().unwrap();
    }
}

#[test]
fn unchanged_unsigned_cast_original_refuses_its_first_prefix_instead_of_later_initializer() {
    let marker = "let x = -1 as usize;";
    let absolute = UNSIGNED.find(marker).unwrap() + marker.find("-1").unwrap();
    run(UNSIGNED, Some("-1"), None, absolute);
}

#[test]
fn exact_original_unsigned_cast_statements_refuse_in_disclosed_isolated_main_adapters() {
    for marker in ["let x = -1 as usize;", "let x = (-1) as usize;"] {
        assert!(UNSIGNED.contains(marker));
        let source = format!("fn main(){{{marker}}}");
        let absolute = source.find("-1").unwrap();
        run(&source, Some("-1"), None, absolute);
    }
}
