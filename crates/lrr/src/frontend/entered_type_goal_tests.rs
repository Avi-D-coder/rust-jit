//! Actual source Type query/guard controls; no guest/full-fixture acceptance.
use super::*;
use crate::{
    db::trace::Action,
    frontend::source_body_context::type_occurrences::{BodyTypeOrigin, BodyTypeUse},
    ir::{
        op::{Op, PrimitiveType},
        pc::{NodeId, PcKey, Phase},
    },
    syntax::{ast_adapter::range, body_index::NodeRole, source_constraints::FactKind},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};
const AUTO: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/auto-traits/auto-is-contextual.rs"
);

// Preserve the unchanged original's crate attributes at their actual crate
// position. Only item bytes enter the arbitrary user macro or wrapper.
fn emitted_items(original: &str, edition: u16, wrapper: bool) -> String {
    let edition = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        2024 => ra_ap_syntax::Edition::Edition2024,
        _ => panic!("unsupported Type source-control edition"),
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
        let span = range(attribute.syntax().text_range());
        let start = span.start as usize;
        let end = span.end as usize;
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
fn source(mode: u8, edition: u16, original: &str) -> String {
    match mode {
        0 | 1 => original.to_owned(),
        2 | 3 => emitted_items(original, edition, mode == 3),
        _ => panic!("unsupported Type source-control mode"),
    }
}
fn cast_site(db: &mut Database, local: &PreparedSourceLocalPlan) -> (PcKey, PcKey) {
    let index = local.context().source().index();
    let value = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceConstraints(index.body().clone()),
    ));
    let QueryValue::SourceConstraints(facts) = value.value.as_ref() else {
        panic!("actual cast source facts differ")
    };
    let casts: Vec<_> = facts
        .facts()
        .iter()
        .enumerate()
        .filter(|(_, fact)| fact.kind() == FactKind::Cast)
        .collect();
    assert_eq!(casts.len(), 1, "complete original has exactly one cast");
    let (ordinal, fact) = casts[0];
    let span = fact.annotation().expect("actual relative cast Type range");
    let body_start = local.context().source().body().range.start;
    let absolute = SourceRange::checked(
        body_start.checked_add(span.start).unwrap(),
        body_start.checked_add(span.end).unwrap(),
    )
    .unwrap();
    let parsed = local.context().source().parsed();
    let syntax = parsed
        .syntax_at(absolute, |syntax| ast::Type::can_cast(syntax.kind()))
        .unwrap()
        .expect("actual cast Type AST");
    assert!(ast::PathType::cast(syntax).is_some());
    // The index also retains nested Syntax nodes at this range. Match both
    // PreparedBodyTypeOccurrences and the normal cast consumer's canonical
    // first Syntax row, rather than assuming the range has only one row.
    let target = index
        .nodes_at_range(span)
        .find(|node| node.role == NodeRole::Syntax)
        .expect("actual canonical cast Type node");
    let occurrence = ready(local.context().body_type_occurrence(db, target.id))
        .expect("actual retained Type occurrence");
    assert_eq!(occurrence.path_range(), absolute);
    assert_eq!(occurrence.node(), target.id);
    (
        index
            .pc(NodeId(u32::try_from(ordinal).unwrap()), Phase::Entry)
            .unwrap(),
        PcKey {
            body: index.body().clone(),
            node: target.id.0,
            phase: 0,
        },
    )
}
fn no_executable_child(db: &Database, start: usize) {
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Constant(_)
        )),
        "Type source goal prepared executable code"
    );
}

#[test]
fn unchanged_cast_type_goals_use_actual_origin_and_one_epoch_guard_in_all_source_modes() {
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..4 {
            let text = source(mode, edition, AUTO);
            let (mut db, local) = world(&text, edition, mode == 1);
            let (cast, target) = cast_site(&mut db, &local);
            let occurrence = ready(
                local
                    .context()
                    .body_type_occurrence(&mut db, NodeId(target.node)),
            )
            .expect("actual cast Type occurrence");
            if mode >= 2 {
                let unit = local
                    .context()
                    .source()
                    .generated_source()
                    .expect("whole emitter main uses its actual direct GeneratedSyntax source");
                assert!(
                    local.context().source().generated_boundary().is_some(),
                    "direct generated main lacks its real discovery boundary"
                );
                assert_eq!(
                    occurrence.origin(),
                    BodyTypeOrigin::Generated(crate::syntax::token_wire::Key(
                        unit.items.record().key().fingerprint().0
                    ))
                );
            } else {
                assert_eq!(occurrence.origin(), BodyTypeOrigin::Original);
            }
            let entered_key = QueryKey::EnteredType(target.clone());
            let plan_key = QueryKey::PreparedTypePlan(target.clone());
            let from = db.trace.events.len();
            let first = ready(db.fetch(&SyntaxProvider, entered_key.clone()));
            let second = ready(db.fetch(&SyntaxProvider, entered_key.clone()));
            assert!(Arc::ptr_eq(&first.value, &second.value));
            let QueryValue::EnteredType(goal) = first.value.as_ref() else {
                panic!("actual entered Type query differs")
            };
            assert_eq!(goal.key(), &target);
            assert_eq!(goal.source_type().primitive(), Some(PrimitiveType::U8));
            assert_eq!(
                goal.retained_plan().source_identity(),
                local.source_identity()
            );
            let first_plan = ready(db.fetch(&SyntaxProvider, plan_key.clone()));
            let second_plan = ready(db.fetch(&SyntaxProvider, plan_key.clone()));
            assert!(Arc::ptr_eq(&first_plan.value, &second_plan.value));
            let QueryValue::PreparedTypePlan(prepared) = first_plan.value.as_ref() else {
                panic!("actual prepared Type plan differs")
            };
            assert!(first.cacheable && first_plan.cacheable);
            for query in [&entered_key, &plan_key] {
                assert_eq!(
                    db.trace.events[from..]
                        .iter()
                        .filter(|event| event.key == *query && event.action == Action::ProviderRun)
                        .count(),
                    1
                );
            }
            no_executable_child(&db, from);
            prepared.guard().require_current(&db).unwrap();

            // Only after query-only assertions, prepare the ordinary cast's two
            // actual phases. Both retain the same already prepared guard Arc.
            let owners = Arc::strong_count(prepared.guard());
            let mut recipes = Vec::new();
            for phase in [Phase::Entry, Phase::Operand(0)] {
                let pc = local
                    .context()
                    .source()
                    .index()
                    .pc(NodeId(cast.node), phase)
                    .unwrap();
                let value = ready(db.fetch(&SyntaxProvider, QueryKey::Thunk(pc)));
                let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                    panic!(
                        "ordinary cast did not produce a verified recipe: {:?}",
                        value.value
                    )
                };
                assert_eq!(
                    recipe.prepared_inputs_fingerprint(),
                    Some(prepared.guard().fingerprint())
                );
                if phase == Phase::Operand(0) {
                    assert!(recipe.recipe().operations.iter().any(|op| matches!(
                        op,
                        Op::PrimitiveCast {
                            target: PrimitiveType::U8,
                            ..
                        }
                    )));
                }
                recipe.revalidate_namespace(&db).unwrap();
                recipes.push(value);
            }
            assert_eq!(Arc::strong_count(prepared.guard()), owners + 2);
            for query in [&entered_key, &plan_key] {
                assert_eq!(
                    db.trace.events[from..]
                        .iter()
                        .filter(|event| event.key == *query && event.action == Action::ProviderRun)
                        .count(),
                    1
                );
            }
            db.begin_expansion_epoch(2).unwrap();
            let before = db.trace.events.len();
            assert!(prepared.guard().require_current(&db).is_err());
            for value in &recipes {
                let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                    unreachable!()
                };
                assert!(recipe.revalidate_namespace(&db).is_err());
            }
            assert_eq!(
                before,
                db.trace.events.len(),
                "installed stale guards fetched source"
            );
            match db.fetch(&SyntaxProvider, entered_key) {
                QueryPoll::Ready(Err(_)) | QueryPoll::NeedInput(_) => {}
                QueryPoll::Ready(Ok(value)) => {
                    panic!("stale Type query admitted {:?}", value.value)
                }
            }
        }
    }
}

#[test]
fn emitted_cast_inside_retained_original_body_keeps_its_missing_structural_authority() {
    // A direct GeneratedSyntax source with its real boundary is different from
    // an emitted position inside an assembled RetainedSyntax original body.
    // Keep every original byte in order and add only this external LRR control.
    let parsed = ra_ap_syntax::SourceFile::parse(AUTO, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let main = parsed
        .tree()
        .syntax()
        .children()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "main"))
        .expect("unchanged original main");
    let main_start = range(main.syntax().text_range()).start as usize;
    let body = main
        .syntax()
        .children()
        .find_map(ast::BlockExpr::cast)
        .expect("actual main block");
    let insert = usize::try_from(range(body.syntax().text_range()).end)
        .unwrap()
        .checked_sub(1)
        .unwrap();
    assert_eq!(&AUTO[insert..insert + 1], "}");
    let text = format!(
        "{}macro_rules! lrr_emitted_cast {{()=>{{0_u8 as u8}}}}\n{}\nlrr_emitted_cast!();\n{}",
        &AUTO[..main_start],
        &AUTO[main_start..insert],
        &AUTO[insert..]
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(&text, edition, true);
        assert!(
            local
                .context()
                .source()
                .retained_inputs()
                .any(|input| matches!(input.as_ref(), InputValue::RetainedSyntax(_)))
        );
        assert!(local.context().source().generated_source().is_none());
        let mut generated = Vec::new();
        for node in local.context().source().index().nodes() {
            let Some(occurrence) = ready(local.context().body_type_occurrence(&mut db, node.id))
            else {
                continue;
            };
            if occurrence.usage() == BodyTypeUse::TypeSyntax
                && occurrence.origin() == BodyTypeOrigin::NeedRetainedOccurrence
            {
                generated.push(PcKey {
                    body: occurrence.body().clone(),
                    node: node.id.0,
                    phase: 0,
                });
            }
        }
        assert_eq!(
            generated.len(),
            1,
            "only the actual emitted cast Type lacks its structural join"
        );
        let from = db.trace.events.len();
        match db.fetch(&SyntaxProvider, QueryKey::EnteredType(generated.remove(0))) {
            QueryPoll::Ready(Err(error)) => {
                assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
                assert!(
                    error.message.contains("RetainedExpansionOccurrence"),
                    "unexpected generated Type refusal: {error:?}"
                );
            }
            QueryPoll::NeedInput(input) => {
                panic!("expected the actual retained-occurrence hold, got {input:?}")
            }
            QueryPoll::Ready(Ok(value)) => panic!(
                "generated Type received authority without its occurrence proof: {:?}",
                value.value
            ),
        }
        no_executable_child(&db, from);
    }
}

#[test]
fn actual_same_spelled_type_declaration_cannot_receive_primitive_cast_authority() {
    // The complete original remains byte-for-byte present; this separate LRR
    // control adds a real Type-namespace declaration outside the imported input.
    let original_with_control = format!("{AUTO}\n#[allow(non_camel_case_types)] struct u8;\n");
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..4 {
            let text = source(mode, edition, &original_with_control);
            let (mut db, local) = world(&text, edition, mode == 1);
            let (_, target) = cast_site(&mut db, &local);
            let from = db.trace.events.len();
            match db.fetch(&SyntaxProvider, QueryKey::EnteredType(target)) {
                QueryPoll::Ready(Ok(value)) => {
                    let QueryValue::EnteredType(goal) = value.value.as_ref() else {
                        panic!("actual shadowed Type goal differs")
                    };
                    assert_eq!(
                        goal.source_type().primitive(),
                        None,
                        "a same-spelled nominal declaration became builtin u8"
                    );
                }
                QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-TYPE-CONTEXT"),
                QueryPoll::NeedInput(_) => {} // A real missing source proof grants no primitive.
            }
            no_executable_child(&db, from);
        }
    }
}

#[test]
fn prepared_cast_guard_rejects_actual_source_input_revocation_without_queries() {
    let (mut db, local) = world(AUTO, 2024, false);
    let (_, target) = cast_site(&mut db, &local);
    let value = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedTypePlan(target)));
    let QueryValue::PreparedTypePlan(prepared) = value.value.as_ref() else {
        panic!("actual prepared Type plan differs")
    };
    db.publish_inputs(BTreeMap::new()).unwrap();
    let before = db.trace.events.len();
    assert!(prepared.guard().require_current(&db).is_err());
    assert_eq!(before, db.trace.events.len());
}

#[test]
fn cfg_disabled_cast_uses_the_ordinary_skip_without_a_type_goal() {
    // Outside the imported file, insert one LRR authority control at the actual
    // main block's AST closing boundary; every original byte stays in order.
    let parsed = ra_ap_syntax::SourceFile::parse(AUTO, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let main = parsed
        .tree()
        .syntax()
        .children()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "main"))
        .expect("unchanged original main");
    let body = main
        .syntax()
        .children()
        .find_map(ast::BlockExpr::cast)
        .expect("actual main block");
    let insert = usize::try_from(range(body.syntax().text_range()).end)
        .unwrap()
        .checked_sub(1)
        .unwrap();
    assert_eq!(&AUTO[insert..insert + 1], "}");
    let controlled = format!(
        "{}\n#[cfg(any())] (0_i32 as MissingCastType);\n{}",
        &AUTO[..insert],
        &AUTO[insert..]
    );
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..4 {
            let text = source(mode, edition, &controlled);
            let (mut db, local) = world(&text, edition, mode == 1);
            let source = local.context().source();
            let index = source.index();
            let mut selected = Vec::new();
            for node in index.nodes() {
                if !matches!(node.role, NodeRole::Expression(_)) {
                    continue;
                }
                // cfg owns the actual OUTER parenthesized expression. Entering
                // its unentered inner cast would demand an operation Rust skips.
                let Some(parenthesized) = source
                    .parsed()
                    .entered_node_syntax(source.body(), index, node.id)
                    .unwrap()
                    .and_then(ast::ParenExpr::cast)
                else {
                    continue;
                };
                let Some(ast::Expr::CastExpr(cast)) = parenthesized.expr() else {
                    continue;
                };
                if cast
                    .ty()
                    .is_some_and(|ty| ty.syntax().text().to_string() == "MissingCastType")
                {
                    selected.push(node.id);
                }
            }
            assert_eq!(
                selected.len(),
                1,
                "actual controlled cfg owner is unique: edition={edition} mode={mode}"
            );
            let pc = index.pc(selected[0], Phase::Entry).unwrap();
            let query = QueryKey::Thunk(pc);
            let from = db.trace.events.len();
            let value = match db.fetch(&SyntaxProvider, query.clone()) {
                QueryPoll::Ready(Ok(value)) => value,
                other => panic!(
                    "actual cfg owner: edition={edition} mode={mode} query={query:?} result={other:?}"
                ),
            };
            let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                panic!(
                    "disabled cfg owner did not use ordinary lowering: {:?}",
                    value.value
                )
            };
            assert!(
                matches!(recipe.recipe().operations.as_ref(), [Op::Unit { .. }]),
                "disabled cfg owner did not select the ordinary expression skip"
            );
            assert!(!db.trace.events[from..].iter().any(|event| matches!(
                event.key,
                QueryKey::EnteredType(_) | QueryKey::PreparedTypePlan(_)
            )));
            assert!(
                db.trace.events[from..]
                    .iter()
                    .all(|event| !matches!(&event.key, QueryKey::Thunk(_) if event.key != query)),
                "disabled cfg owner prepared its inner cast or operand"
            );
        }
    }
}
