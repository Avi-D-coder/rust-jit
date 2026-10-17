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
    // The retained namespace has the actual block-local constant, but its
    // declaration-name occurrence has not yet joined LocalTypeNames. Demand
    // that exact declaration; a missing prefix must never become a binding.
    let roots = plan.allocation_roots();
    let facts = roots
        .namespace()
        .map(|namespace| &namespace.facts)
        .unwrap_or(plan.context().facts());
    let mut constants = facts.items.iter().filter(|row| {
        row.item.def_key.declared_name == "X"
            && row.item.def_key.item_kind == crate::source::manifest::ItemKind::Constant
            && row.namespaces.contains(&Namespace::Value)
    });
    let constant = constants
        .next()
        .expect("actual constant declaration absent");
    assert!(
        constants.next().is_none(),
        "constant source owner is ambiguous"
    );
    let declaration = plan
        .context()
        .source()
        .parsed()
        .syntax_at(constant.item.declaration_span, |node| {
            ast::Const::can_cast(node.kind())
        })
        .unwrap()
        .and_then(ast::Const::cast)
        .expect("constant inventory is not attached to its actual source");
    assert_eq!(declaration.name().unwrap().text(), "X");
    assert!(goal.declaration().is_none());
    assert_eq!(
        goal.pending(),
        Some(ValueGoalDemand::Prefix(constant.item.def_key.fingerprint())),
        "the valid constant-pattern runtime contract still needs its exact declaration occurrence"
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

#[test]
fn unchanged_function_name_is_shadowed_by_actual_binding_all_editions_and_source_modes() {
    const AUTO: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/auto-traits/auto-is-contextual.rs"
    );
    // Crate-inner attributes belong to the crate, not the Items fragment of
    // emit!/wrap!. Preserve their exact bytes outside the arbitrary emitter;
    // actual item bytes and their declaration/body operations remain unchanged.
    fn emitted_items(original: &str, edition: u16, wrapper: bool) -> String {
        let edition = match edition {
            2015 => ra_ap_syntax::Edition::Edition2015,
            2018 => ra_ap_syntax::Edition::Edition2018,
            2021 => ra_ap_syntax::Edition::Edition2021,
            2024 => ra_ap_syntax::Edition::Edition2024,
            _ => panic!("unsupported source-control edition"),
        };
        let parsed = ra_ap_syntax::SourceFile::parse(original, edition);
        assert!(parsed.errors().is_empty());
        let mut attributes = String::new();
        let mut items = String::new();
        let mut previous = 0;
        for attribute in parsed
            .tree()
            .syntax()
            .children()
            .filter_map(ast::Attr::cast)
        {
            let range = range(attribute.syntax().text_range());
            let start = range.start as usize;
            let end = range.end as usize;
            items.push_str(&original[previous..start]);
            attributes.push_str(&original[start..end]);
            attributes.push('\n');
            previous = end;
        }
        items.push_str(&original[previous..]);
        format!(
            "{attributes}macro_rules! emit{{()=>{{{items}}}}}{}",
            if wrapper {
                "macro_rules! wrap{()=>{emit!();}}wrap!();"
            } else {
                "emit!();"
            }
        )
    }
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..4 {
            let text = match mode {
                0 | 1 => AUTO.to_owned(),
                _ => emitted_items(AUTO, edition, mode == 3),
            };
            let (mut db, plan) = world(&text, edition, mode == 1);
            let body = plan.context().source().index().body().clone();
            let start = db.trace.events.len();
            let mut binding = None;
            let mut uses = Vec::new();
            let mut function = None;
            for (node, name, pattern) in selected_nodes(&plan) {
                if name != "auto" {
                    continue;
                }
                let key = PcKey {
                    body: body.clone(),
                    node: node.0,
                    phase: 0,
                };
                let result = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key.clone())));
                let goal = observed(result.value.as_ref());
                assert!(
                    goal.pending().is_none(),
                    "function shadowing is pending: {goal:?}"
                );
                if pattern {
                    let target = goal.binding_target().unwrap();
                    assert_eq!(goal.local_binding(), Some(target.identity()));
                    assert!(goal.declaration().is_none());
                    assert!(binding.replace((key, goal.clone())).is_none());
                } else if let Some(local) = goal.local_binding() {
                    uses.push(local);
                } else {
                    let declaration = goal.declaration().expect("actual function header missing");
                    assert!(function.replace(declaration.definition().clone()).is_none());
                }
            }
            let (key, target) = binding.expect("actual bare target absent");
            let identity = target.local_binding().unwrap();
            assert_eq!(uses, vec![identity, identity]);
            let function = function.expect("preceding function use absent");
            assert_ne!(function.identity, identity);
            assert!(
                db.trace.events[start..].iter().all(|event| {
                    !matches!(&event.key, QueryKey::Thunk(_))
                        && !matches!(&event.key, QueryKey::BodyStructure(other) if other != &body)
                }),
                "binding classification prepared child code"
            );
            let reused = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key)));
            assert_eq!(
                observed(reused.value.as_ref()).local_binding(),
                Some(identity)
            );

            // Corrupt only the supplied header at the private source-join seam;
            // the actual selected definition/source/context remain unchanged.
            // The real canonical declaration must reject this mismatched view.
            let source = db
                .input(&InputKey::DefinitionSource(function.identity))
                .unwrap();
            let header = db
                .input(&InputKey::DefinitionHeader(function.identity))
                .unwrap();
            let context = db
                .input(&InputKey::DeclarationContext(function.identity))
                .unwrap();
            let (
                InputValue::DefinitionSource(source),
                InputValue::DefinitionHeader(header),
                InputValue::DeclarationContext(context),
            ) = (source.as_ref(), header.as_ref(), context.as_ref())
            else {
                panic!()
            };
            let mut changed = header.clone();
            changed.tokens.clear();
            match crate::frontend::source_value_goals::shadowed_definition_binding(
                &mut db,
                target.clone(),
                function,
                source,
                &changed,
                context,
                None,
            ) {
                QueryPoll::Ready(Err(_)) => {}
                other => panic!("mismatched function source/header admitted: {other:?}"),
            }
        }
    }
}
