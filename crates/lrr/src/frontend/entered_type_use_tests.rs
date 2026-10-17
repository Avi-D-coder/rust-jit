//! Actual indexed Type-use authority. No guest or full-fixture pass is claimed.
use super::*;
use crate::{
    frontend::entered_type_goals::EnteredTypeGoal,
    ir::{
        op::PrimitiveType,
        pc::{PcKey, Phase},
    },
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob},
    },
    syntax::{
        ast_adapter::range,
        body_index::{ExpressionKind, NodeRole},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};

const NESTED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
// LRR authority control outside imported originals: neither the cold field
// Type nor the callee's executable body is needed to bind its explicit Type.
const CONTROL: &str = "struct Marker{field:MissingColdLayout} fn accept<T>(){missing_cold_body();} fn main(){accept::<u8>();accept::<Marker>();}";

fn sites(plan: &PreparedSourceLocalPlan) -> Vec<(PcKey, SourceRange, String)> {
    let source = plan.context().source();
    let index = source.index();
    let mut sites = Vec::new();
    for node in index
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
    {
        let call = source
            .parsed()
            .entered_node_syntax(source.body(), index, node.id)
            .unwrap()
            .and_then(ast::CallExpr::cast)
            .expect("actual indexed caller CallExpr");
        let target = call
            .expr()
            .and_then(|expr| match expr {
                ast::Expr::PathExpr(path) => path.path(),
                _ => None,
            })
            .expect("actual explicit-call PathExpr");
        let list = target.segment().unwrap().generic_arg_list().unwrap();
        for argument in list.generic_args() {
            let ast::GenericArg::TypeArg(argument) = argument else {
                panic!("selected Type-use witness must retain its actual TypeArg")
            };
            let ty = argument.ty().unwrap();
            assert!(matches!(&ty, ast::Type::PathType(_)));
            let absolute = range(ty.syntax().text_range());
            assert!(source.body().range.contains(absolute));
            let relative = SourceRange::checked(
                absolute
                    .start
                    .checked_sub(source.body().range.start)
                    .unwrap(),
                absolute.end.checked_sub(source.body().range.start).unwrap(),
            )
            .unwrap();
            let canonical = index
                .nodes_at_range(relative)
                .find(|node| node.role == NodeRole::Syntax)
                .unwrap();
            sites.push((
                index.pc(canonical.id, Phase::Entry).unwrap(),
                absolute,
                ty.syntax().text().to_string(),
            ));
        }
    }
    sites
}

fn type_goal(value: &QueryValue) -> &EnteredTypeGoal {
    let QueryValue::EnteredType(goal) = value else {
        panic!("actual indexed Type goal variant differs")
    };
    goal
}
fn no_executable_or_layout(db: &Database, start: usize) {
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Constant(_)
        )),
        "Type use forced executable code or field layout"
    );
}

#[test]
fn full_official_nested_type_arguments_require_exact_nominal_source_occurrences() {
    for edition in [2015, 2018, 2021, 2024] {
        // Actual retained main includes grault's nested declaration output;
        // every imported original byte is preserved.
        let (mut db, plan) = world(NESTED, edition, true);
        let actual = sites(&plan);
        assert_eq!(
            actual
                .iter()
                .map(|(_, _, name)| name.as_str())
                .collect::<Vec<_>>(),
            ["Bar", "Baz", "Xyzzy"]
        );
        let start = db.trace.events.len();
        for (key, absolute, _) in actual {
            // A missing generated declaration proof is a real test failure,
            // never a skipped nominal Type or primitive fallback.
            let owner = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredType(key.clone())));
            let goal = type_goal(owner.value.as_ref());
            let source = goal
                .source_for(plan.context().source().index(), &key, absolute)
                .unwrap();
            assert!(matches!(source.source(), TyTerm::Nominal(_, args) if args.is_empty()));
            assert!(std::ptr::eq(source, goal.source_type()));
            let error = goal
                .primitive_for(plan.context().source().index(), &key, absolute)
                .unwrap_err();
            assert_eq!(
                error.message,
                "selected cast target requires nonprimitive cast integration"
            );
        }
        no_executable_or_layout(&db, start);
    }
}

#[test]
fn concrete_type_use_preserves_cold_nominal_fields_and_rejects_wrong_site_or_coordinates() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, plan) = world(CONTROL, edition, false);
        let (_, foreign) = world(&CONTROL.replace("::<u8>", "::<u16>"), edition, false);
        let index = plan.context().source().index();
        let actual = sites(&plan);
        assert_eq!(actual.len(), 2);
        let start = db.trace.events.len();
        for (ordinal, (key, absolute, _)) in actual.iter().enumerate() {
            let owner = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredType(key.clone())));
            let goal = type_goal(owner.value.as_ref());
            let source = goal.source_for(index, key, *absolute).unwrap();
            assert!(
                goal.source_for(foreign.context().source().index(), key, *absolute)
                    .is_err()
            );
            if ordinal == 0 {
                assert_eq!(source.primitive(), Some(PrimitiveType::U8));
                // The primitive cast adapter accepts a distinct operation Pc
                // with the same owning body, preserving its existing contract.
                let call = index
                    .nodes()
                    .iter()
                    .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
                    .unwrap();
                let operation = index.pc(call.id, Phase::Entry).unwrap();
                assert_ne!(&operation, key);
                assert_eq!(
                    goal.primitive_for(index, &operation, *absolute).unwrap(),
                    PrimitiveType::U8
                );
                assert!(goal.source_for(index, &operation, *absolute).is_err());
            } else {
                assert!(matches!(source.source(), TyTerm::Nominal(_, args) if args.is_empty()));
                assert!(goal.primitive_for(index, key, *absolute).is_err());
            }
            let mut wrong = key.clone();
            wrong.phase = Phase::Operand(0).code().unwrap();
            assert!(goal.source_for(index, &wrong, *absolute).is_err());
            wrong = key.clone();
            wrong.body.definition = Digest::of_bytes(b"wrong Type-use body");
            assert!(goal.source_for(index, &wrong, *absolute).is_err());
            assert!(goal.primitive_for(index, &wrong, *absolute).is_err());
            let shifted =
                SourceRange::checked(absolute.start.checked_add(1).unwrap(), absolute.end).unwrap();
            assert!(goal.source_for(index, key, shifted).is_err());
            let relative = index.nodes()[key.node as usize].range;
            assert_ne!(relative, *absolute);
            assert!(goal.source_for(index, key, relative).is_err());
            assert!(
                goal.source_for(index, &actual[1 - ordinal].0, *absolute)
                    .is_err()
            );
        }
        no_executable_or_layout(&db, start);
    }
}

#[test]
fn exact_type_use_retains_external_q43_source_cfg_and_epoch_guards_without_queries() {
    for change in 0..3 {
        let (mut db, plan) = world(CONTROL, 2024, false);
        let (key, absolute, _) = sites(&plan).remove(0);
        let owner = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedTypePlan(key.clone())));
        let QueryValue::PreparedTypePlan(prepared) = owner.value.as_ref() else {
            panic!()
        };
        prepared.guard().require_current(&db).unwrap();
        prepared
            .goal()
            .source_for(plan.context().source().index(), &key, absolute)
            .unwrap();
        match change {
            0 => {
                db.begin_expansion_epoch(2).unwrap();
            }
            1 => {
                db.publish_inputs(BTreeMap::new()).unwrap();
            }
            2 => {
                let original = plan.context().module_source().unwrap();
                let selected = original.selection();
                let mut cfg = CfgSet::guest_default();
                cfg.atoms.insert("lrr_type_use_changed_cfg".into());
                assert_ne!(cfg.fingerprint(), selected.outline.cfg);
                // A genuine new configured source publication replaces the
                // old universe. This does not claim isolated one-row mutation.
                let inputs = PreparedSourceInputs::refresh(
                    None,
                    vec![SourceUnitInput {
                        path: selected.path.clone(),
                        crate_key: selected.outline.crate_key,
                        module_path: selected.outline.module_path.clone(),
                        profile: selected.outline.parse.profile.clone(),
                        cfg,
                        blob: SourceBlob::new(CONTROL.as_bytes().to_vec()).unwrap(),
                    }],
                    Default::default(),
                )
                .unwrap();
                inputs.publish(&mut db).unwrap();
            }
            _ => unreachable!(),
        }
        let start = db.trace.events.len();
        assert!(prepared.guard().require_current(&db).is_err());
        assert_eq!(start, db.trace.events.len());
    }
}
