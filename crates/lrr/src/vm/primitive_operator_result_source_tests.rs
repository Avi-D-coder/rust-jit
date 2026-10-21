//! Ordinary source/operator controls plus explicit LRR metadata-adversary tests.
//! No fabricated callable, namespace or return certificate is used here.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::{
        pc::{Phase, SlotId},
        recipe::VerifiedRecipe,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    traits::term::{ParamKey, TyTerm},
};
const STOCK_COMPARE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cast/cast-to-char-compare.rs"
);
const STOCK_RUNTIME: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numeric/type-limit-comparisons-6130.rs"
);
fn source(text: &str, edition: u16) -> (Database, PreparedSourceView) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("operator-result.rs").unwrap();
    let unit = Digest::of_bytes(b"LRR reached primitive operator result source");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: crate::syntax::parse::ParseKey {
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
        panic!("actual captured source absent")
    };
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    (db, view)
}
struct Operator {
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    destination: SlotId,
}
fn reached(text: &str, edition: u16) -> Operator {
    let (mut db, view) = source(text, edition);
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
    let indexed = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!("body kind")
    };
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(
        index.as_ref().clone(),
        index.pc(crate::ir::pc::NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    for _ in 0..256 {
        let thunk = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = thunk.value.as_ref() else {
            panic!("thunk kind")
        };
        if let Some(destination) = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::NumericBinary { destination, .. } | Op::NumericUnary { destination, .. } => {
                Some(*destination)
            }
            _ => None,
        }) {
            assert_eq!(
                recipe.recipe().operations.len(),
                1,
                "guard controls require an exact sole numeric opcode"
            );
            return Operator {
                frame,
                store,
                recipe: recipe.as_ref().clone(),
                destination,
            };
        }
        assert!(matches!(
            step(&mut frame, &mut store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("ordinary source never reached selected operator")
}
fn run(c: &mut Operator) {
    assert!(matches!(
        step(&mut c.frame, &mut c.store, &c.recipe).unwrap(),
        StepOutcome::Next(_)
    ));
}
fn left(c: &Operator) -> SlotId {
    match &c.recipe.recipe().operations[0] {
        Op::NumericBinary { left, .. } => *left,
        Op::NumericUnary { source, .. } => *source,
        _ => unreachable!(),
    }
}
fn snapshot(
    c: &Operator,
) -> (
    crate::runtime::cell::OwnerInventory,
    u32,
    usize,
    crate::vm::steps::StepSnapshot,
    serde_json::Value,
) {
    (
        c.store.owner_inventory_for_test(),
        c.store.budget().handles(),
        c.store.budget().bytes(),
        c.frame.fuel_snapshot(),
        serde_json::to_value(c.frame.counters()).unwrap(),
    )
}
#[test]
fn unchanged_char_comparison_component_and_closed_comparisons_retain_intrinsic_bool_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        // This official check-pass source's comparison is executed, stopping
        // before either print body. Not a full fixture/runtime-output pass.
        for text in [
            STOCK_COMPARE,
            "fn main(){1_i64==1_i64;}",
            "fn main(){true!=false;}",
        ] {
            let mut c = reached(text, edition);
            run(&mut c);
            assert_eq!(
                c.frame.source_view(c.destination).unwrap(),
                Some(TyTerm::Primitive(PrimitiveType::Bool))
            );
            assert!(
                c.store
                    .read_scalar(c.frame.cell(c.destination).unwrap())
                    .unwrap()
                    .payload
                    != 0
            );
        }
    }
}
#[test]
fn arithmetic_unary_forward_only_actual_written_source_and_source_less_arithmetic_stays_absent() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "fn main(){let x:i64=40; x+1_i64;}",
            "fn main(){let x:i64=40; -x;}",
            "fn main(){1_i64+2_i64;}",
        ] {
            let mut c = reached(text, edition);
            assert_eq!(
                c.frame.source_view(left(&c)).unwrap(),
                Some(TyTerm::Primitive(PrimitiveType::I64))
            );
            run(&mut c);
            assert_eq!(
                c.frame.source_view(c.destination).unwrap(),
                Some(TyTerm::Primitive(PrimitiveType::I64))
            );
        }
        let mut c = reached("fn main(){1+2;}", edition);
        assert_eq!(c.frame.source_view(left(&c)).unwrap(), None);
        run(&mut c);
        assert_eq!(
            c.frame.source_view(c.destination).unwrap(),
            None,
            "runtime layout cannot invent numeric source identity"
        );
    }
}
#[test]
fn rigid_or_wrong_operand_source_and_rigid_result_refuse_before_fuel_or_owning_change() {
    // Case 0 (a rigid operand on a primitive value) dispatches the operator's
    // trait method instead; see the next test.
    for bad in 1..3 {
        let mut c = reached("fn main(){1_i64==2_i64;}", 2024);
        let operand = left(&c);
        let rigid = TyTerm::Param(ParamKey {
            definition: c.frame.body.body().definition,
            ordinal: 0,
        });
        let slot = if bad == 2 { c.destination } else { operand };
        let source = if bad == 1 {
            TyTerm::Primitive(PrimitiveType::U64)
        } else {
            rigid
        };
        // Deliberately corrupt only retained LRR source metadata. This grants
        // no caller/trait/certificate authority and must not survive admission.
        c.frame.set_source_view(slot, Some(source.clone())).unwrap();
        let before = snapshot(&c);
        let error = step(&mut c.frame, &mut c.store, &c.recipe).unwrap_err();
        assert_eq!(error.code, "LRR-E-GENERIC-BINDING");
        assert_eq!(snapshot(&c), before);
        assert_eq!(c.frame.source_view(slot).unwrap(), Some(source));
        assert!(c.frame.cell(c.destination).is_err());
    }
}
#[test]
fn rigid_operand_source_on_a_primitive_value_dispatches_the_operator_trait_method() {
    let mut c = reached("fn main(){1_i64==2_i64;}", 2024);
    let operand = left(&c);
    let rigid = TyTerm::Param(ParamKey {
        definition: c.frame.body.body().definition,
        ordinal: 0,
    });
    c.frame.set_source_view(operand, Some(rigid.clone())).unwrap();
    let outcome = step(&mut c.frame, &mut c.store, &c.recipe).unwrap();
    assert!(matches!(outcome, StepOutcome::Dispatch));
    assert_eq!(c.frame.source_view(operand).unwrap(), Some(rigid));
}
#[test]
fn unchanged_runtime_comparison_main_executes_three_original_assertions_all_explicit_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        // The library's own `assert!` expands through the ordinary pipeline.
        let (mut db, mut run, macros) =
            crate::engine::run::start_discovered_body(STOCK_RUNTIME, "main", edition);
        let contexts = macros.contexts().unwrap();
        let report = match run.poll_with_runtime_macros(&mut db, &contexts) {
            crate::engine::run::RunPoll::Completed(report) => report,
            crate::engine::run::RunPoll::Fault(error) => panic!(
                "original runtime main failed {}: {}",
                error.code, error.message
            ),
            crate::engine::run::RunPoll::NeedInput(input) => {
                panic!("original runtime main lacks preparation {input:?}")
            }
            crate::engine::run::RunPoll::Panicked(panic) => panic!(
                "original runtime assertion failed {}",
                panic.panic.description()
            ),
        };
        assert!(matches!(
            report.value,
            crate::engine::run::ValueSummary::Unit
        ));
        db.finish_run().unwrap();
        // Whole original main execution control at explicit LRR editions;
        // authentic compiletest configuration/full-fixture claims are separate.
    }
}

#[test]
fn arithmetic_panic_retains_existing_output_cell_allocation_order_without_publishing_source() {
    let mut c = reached("fn main(){let x:i64=1; x/0_i64;}", 2024);
    let source = c.frame.source_view(c.destination).unwrap();
    let before = c.frame.counters.operations_started;
    let outcome = step(&mut c.frame, &mut c.store, &c.recipe).unwrap();
    assert!(matches!(
        outcome,
        StepOutcome::GuestPanic(GuestPanic {
            kind: super::super::numeric::PanicKind::DivisionByZero,
            ..
        })
    ));
    assert_eq!(c.frame.counters.operations_started, before + 1);
    assert_eq!(
        c.store.state(c.frame.cell(c.destination).unwrap()).unwrap(),
        crate::runtime::cell::CellState::Uninit
    );
    assert_eq!(c.frame.source_view(c.destination).unwrap(), source);
}
