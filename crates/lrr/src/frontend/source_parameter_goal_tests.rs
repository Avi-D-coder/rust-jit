//! Authority controls outside the complete unchanged official numeric witness.
use super::*;
use crate::{
    db::trace::Action,
    frontend::source_parameter_goals::{ParameterGoalObservation, ParameterTypeGoal},
    ir::op::PrimitiveType,
    syntax::signatures::FunctionSignature,
    traits::term::ParamKey,
};
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
fn declarations(plan: &PreparedSourceLocalPlan) -> Vec<(String, Digest)> {
    plan.context()
        .facts()
        .items
        .iter()
        .filter(|row| row.item.def_key.item_kind == ItemKind::Function)
        .map(|row| {
            (
                row.item.def_key.declared_name.clone(),
                row.item.def_key.fingerprint(),
            )
        })
        .collect()
}
fn signature(db: &mut Database, definition: Digest) -> FunctionSignature {
    let outcome = ready(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
    let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
        panic!("actual signature query differs")
    };
    signature.as_ref().clone()
}
fn goal(value: &QueryValue) -> &ParameterGoalObservation {
    let QueryValue::DeclaredParameterGoal(goal) = value else {
        panic!("actual parameter query differs: {value:?}")
    };
    goal
}
fn no_child_code(db: &Database, start: usize) {
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Constant(_)
        )),
        "declared header goal entered executable preparation"
    );
}
fn check_suffix(db: &mut Database, plan: &PreparedSourceLocalPlan) {
    let expected = [
        ("id_i8", PrimitiveType::I8),
        ("id_i16", PrimitiveType::I16),
        ("id_i32", PrimitiveType::I32),
        ("id_i64", PrimitiveType::I64),
        ("id_uint", PrimitiveType::Usize),
        ("id_u8", PrimitiveType::U8),
        ("id_u16", PrimitiveType::U16),
        ("id_u32", PrimitiveType::U32),
        ("id_u64", PrimitiveType::U64),
    ];
    let declarations = declarations(plan);
    assert_eq!(declarations.len(), expected.len());
    for (name, primitive) in expected {
        let definition = declarations
            .iter()
            .find(|(actual, _)| actual == name)
            .unwrap()
            .1;
        let key = QueryKey::DeclaredParameterGoal {
            definition,
            ordinal: 0,
        };
        let start = db.trace.events.len();
        let first = ready(db.fetch(&SyntaxProvider, key.clone()));
        let ParameterTypeGoal::Ready(source) = goal(first.value.as_ref()).goal() else {
            panic!(
                "unchanged parameter Type goal pending: {:?}",
                goal(first.value.as_ref()).goal()
            )
        };
        assert_eq!(source.source(), &TyTerm::Primitive(primitive));
        assert_eq!(source.definition(), definition);
        assert_eq!(source.ordinal(), 0);
        let second = ready(db.fetch(&SyntaxProvider, key.clone()));
        assert!(
            Arc::ptr_eq(&first.value, &second.value),
            "immutable goal was reconstructed"
        );
        assert_eq!(
            db.trace.events[start..]
                .iter()
                .filter(|event| event.key == key && event.action == Action::ProviderRun)
                .count(),
            1
        );
        no_child_code(db, start);
    }
}
#[test]
fn unchanged_declared_parameter_goals_are_memoized_without_child_code_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for retained in [false, true] {
            let (mut db, plan) = world(SUFFIX, edition, retained);
            check_suffix(&mut db, &plan);
        }
    }
}
#[test]
fn emitted_and_wrapper_parameter_headers_use_the_same_goal_and_real_epoch() {
    for edition in [2015, 2018, 2021, 2024] {
        for wrapper in [false, true] {
            let text = format!(
                "macro_rules! emit{{()=>{{{SUFFIX}}}}}{}",
                if wrapper {
                    "macro_rules! wrap{()=>{emit!();}}wrap!();"
                } else {
                    "emit!();"
                }
            );
            let (mut db, plan) = world(&text, edition, false);
            check_suffix(&mut db, &plan);
            let definition = declarations(&plan)[0].1;
            db.begin_expansion_epoch(2).unwrap();
            match db.fetch(
                &SyntaxProvider,
                QueryKey::DeclaredParameterGoal {
                    definition,
                    ordinal: 0,
                },
            ) {
                QueryPoll::Ready(Err(_)) => {}
                other => panic!("stale generated parameter authority was not refused: {other:?}"),
            }
        }
    }
}
#[test]
fn parameter_goal_rejects_wrong_ordinal_header_and_revoked_parent() {
    let (mut db, plan) = world(SUFFIX, 2024, false);
    let definition = declarations(&plan)[0].1;
    let start = db.trace.events.len();
    let actual = signature(&mut db, definition);
    match db.fetch(
        &SyntaxProvider,
        QueryKey::DeclaredParameterGoal {
            definition,
            ordinal: 1,
        },
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-TYPE-CONTEXT"),
        other => panic!("wrong ordinal accepted: {other:?}"),
    }
    let mut wrong = actual.clone();
    wrong.parameters[0].ty[0].text.push_str("_different");
    refused(
        plan.prepare_parameter_type(&mut db, &wrong, 0),
        "substituted parameter header",
    );
    let key = QueryKey::DeclaredParameterGoal {
        definition,
        ordinal: 0,
    };
    ready(db.fetch(&SyntaxProvider, key.clone()));
    no_child_code(&db, start);
    db.publish_inputs(BTreeMap::new()).unwrap();
    match db.fetch(&SyntaxProvider, key) {
        QueryPoll::Ready(Err(_)) => {}
        other => panic!("revoked parent authority was not refused: {other:?}"),
    }
}
#[test]
fn header_goal_uses_own_generic_identity_and_keeps_nested_type_occurrences_pending() {
    // LRR authority control appended outside the unchanged fixture. It does
    // not replace the official runtime main or assert language acceptance.
    let end = SUFFIX.rfind('}').unwrap();
    let text = format!(
        "{}fn own<T>(n:T) {{}} fn nested<T>(n:&T) {{}}{}",
        &SUFFIX[..end],
        &SUFFIX[end..]
    );
    let (mut db, plan) = world(&text, 2024, false);
    let declarations = declarations(&plan);
    let own = declarations
        .iter()
        .find(|(name, _)| name == "own")
        .unwrap()
        .1;
    let nested = declarations
        .iter()
        .find(|(name, _)| name == "nested")
        .unwrap()
        .1;
    let start = db.trace.events.len();
    let selected = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::DeclaredParameterGoal {
            definition: own,
            ordinal: 0,
        },
    ));
    let ParameterTypeGoal::Ready(source) = goal(selected.value.as_ref()).goal() else {
        panic!(
            "actual own generic goal pending: {:?}",
            goal(selected.value.as_ref()).goal()
        )
    };
    assert_eq!(
        source.source(),
        &TyTerm::Param(ParamKey {
            definition: own,
            ordinal: 0
        })
    );
    let pending = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::DeclaredParameterGoal {
            definition: nested,
            ordinal: 0,
        },
    ));
    assert!(
        matches!(
            goal(pending.value.as_ref()).goal(),
            ParameterTypeGoal::NeedName(SignatureName::NeedOwnerContext(_))
        ),
        "complex generic occurrence was certified without nested hygiene: {:?}",
        goal(pending.value.as_ref()).goal()
    );
    no_child_code(&db, start);
}
