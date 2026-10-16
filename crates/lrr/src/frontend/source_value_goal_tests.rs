//! Source/authority controls over whole unchanged upstream inputs. These do not
//! execute the fixtures or turn a pending constant contract into a pass.
use super::*;
use crate::{
    frontend::source_value_goals::{EnteredValueGoal, ValueGoalDemand},
    ir::pc::{NodeId, PcKey},
    syntax::{
        ast_adapter::range,
        body_index::{ExpressionKind, NodeRole},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
const CONSTANT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/pattern/match-constant-and-byte-literal.rs"
);
fn observed(value: &QueryValue) -> &EnteredValueGoal {
    let QueryValue::EnteredValue(goal) = value else {
        panic!("actual Value goal variant differs: {value:?}")
    };
    goal
}
fn selected_nodes(plan: &PreparedSourceLocalPlan) -> Vec<(NodeId, String, bool)> {
    let source = plan.context().source();
    source
        .index()
        .nodes()
        .iter()
        .filter_map(|node| {
            let syntax = source
                .parsed()
                .entered_node_syntax(source.body(), source.index(), node.id)
                .unwrap()?;
            match node.role {
                NodeRole::Pattern => {
                    let pattern = ast::IdentPat::cast(syntax)?;
                    Some((node.id, pattern.name()?.text().to_owned(), true))
                }
                NodeRole::Expression(ExpressionKind::Path) => {
                    let path = ast::PathExpr::cast(syntax)?.path()?;
                    if path.qualifier().is_some() {
                        return None;
                    }
                    Some((
                        node.id,
                        path.segment()?.name_ref()?.text().to_owned(),
                        false,
                    ))
                }
                _ => None,
            }
        })
        .collect()
}
fn check_suffix(db: &mut Database, plan: &PreparedSourceLocalPlan) {
    let body = plan.context().source().index().body().clone();
    let start = db.trace.events.len();
    let mut targets = std::collections::BTreeSet::new();
    let mut uses = Vec::new();
    let mut functions = 0;
    for (node, name, pattern) in selected_nodes(plan) {
        if name != "j" && !name.starts_with("id_") {
            continue;
        }
        let key = QueryKey::EnteredValue(PcKey {
            body: body.clone(),
            node: node.0,
            phase: 0,
        });
        let first = ready(db.fetch(&SyntaxProvider, key.clone()));
        let goal = observed(first.value.as_ref());
        assert!(
            goal.pending().is_none(),
            "unchanged source occurrence is pending: {goal:?}"
        );
        assert_eq!(goal.body(), &body);
        assert_eq!(goal.node(), node);
        if pattern {
            let target = goal
                .binding_target()
                .expect("actual target metadata missing");
            assert_eq!(target.name(), "j");
            assert_eq!(
                target.classification(),
                crate::syntax::local_bindings::PatternNameKind::BindingOrConstant
            );
            assert_eq!(goal.local_binding(), Some(target.identity()));
            assert!(
                targets.insert(target.identity()),
                "shadowed lets share a forged identity"
            );
            let recipe = plan
                .context()
                .facts()
                .bindings
                .iter()
                .find(|recipe| recipe.name_range == target.name_range())
                .unwrap();
            assert_eq!(recipe.identity, target.identity());
            assert_eq!(recipe.raw_name, target.raw_name());
        } else if name == "j" {
            let source = plan.context().source();
            let syntax = source
                .parsed()
                .entered_node_syntax(source.body(), source.index(), node)
                .unwrap()
                .unwrap();
            let at = range(syntax.text_range()).start;
            let expected = plan
                .context()
                .facts()
                .bindings
                .iter()
                .filter(|recipe| recipe.name == "j" && recipe.visible_from <= u64::from(at))
                .max_by_key(|recipe| recipe.visible_from)
                .unwrap();
            assert_eq!(goal.local_binding(), Some(expected.identity));
            uses.push(expected.identity);
        } else {
            functions += 1;
            let declaration = goal.declaration().expect("selected local header missing");
            let expected = plan
                .context()
                .facts()
                .items
                .iter()
                .find(|row| row.item.def_key.declared_name == name)
                .unwrap();
            assert_eq!(declaration.identity(), expected.item.def_key.fingerprint());
            assert_eq!(declaration.definition().namespace, Namespace::Value);
            assert_eq!(
                declaration.source(),
                &plan.context().source().parsed().key()
            );
            assert_eq!(declaration.header(), expected.item.header.fingerprint());
            assert_eq!(declaration.context().range, expected.item.declaration_span);
            assert_eq!(declaration.owner(), Some(&body));
            assert!(goal.local_binding().is_none());
        }
        let second = ready(db.fetch(&SyntaxProvider, key));
        assert!(
            Arc::ptr_eq(&first.value, &second.value),
            "immutable Value goal reconstructed"
        );
    }
    assert_eq!(targets.len(), 9);
    assert_eq!(uses.len(), 9);
    assert_eq!(functions, 18);
    assert!(uses.into_iter().all(|identity| targets.contains(&identity)));
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            &event.key,
            QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Constant(_)
                | QueryKey::FunctionSignature(_)
        )),
        "Value metadata prepared child executable/signature code"
    );
    assert!(
        db.trace.events[start..]
            .iter()
            .all(|event| !matches!(&event.key, QueryKey::BodyStructure(other) if other != &body))
    );
}
#[test]
fn unchanged_suffix_value_headers_and_shadowed_binding_identities_share_ordinary_goals_all_editions()
 {
    for edition in [2015, 2018, 2021, 2024] {
        for retained in [false, true] {
            let (mut db, plan) = world(SUFFIX, edition, retained);
            check_suffix(&mut db, &plan);
        }
    }
}
#[test]
fn emitted_and_wrapper_value_goals_use_actual_indexed_context_and_epoch() {
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
            let node = selected_nodes(&plan)
                .into_iter()
                .find(|(_, name, pattern)| name == "j" && *pattern)
                .unwrap()
                .0;
            let key = QueryKey::EnteredValue(PcKey {
                body: plan.context().source().index().body().clone(),
                node: node.0,
                phase: 0,
            });
            db.begin_expansion_epoch(2).unwrap();
            match db.fetch(&SyntaxProvider, key) {
                QueryPoll::Ready(Err(_)) => {}
                other => panic!("stale generated Value authority accepted: {other:?}"),
            }
        }
    }
}
#[test]
fn actual_constant_pattern_is_not_mislabeled_as_a_local_binding_and_bad_pc_is_refused() {
    let (mut db, plan) = world(CONSTANT, 2024, false);
    let node = selected_nodes(&plan)
        .into_iter()
        .find(|(_, name, pattern)| name == "X" && *pattern)
        .unwrap()
        .0;
    let mut key = PcKey {
        body: plan.context().source().index().body().clone(),
        node: node.0,
        phase: 0,
    };
    let value = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key.clone())));
    let goal = observed(value.value.as_ref());
    assert!(goal.local_binding().is_none());
    assert!(
        matches!(
            goal.pending(),
            Some(ValueGoalDemand::BindingClassification(_))
        ),
        "the valid constant-pattern runtime contract remains unsupported: {goal:?}"
    );
    key.phase = 1;
    match db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key.clone())) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-QUERY-REQUEST"),
        other => panic!("wrong phase accepted: {other:?}"),
    }
    key.phase = 0;
    key.node = u32::MAX;
    match db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key)) {
        QueryPoll::Ready(Err(_)) => {}
        other => panic!("foreign indexed node accepted: {other:?}"),
    }
}
