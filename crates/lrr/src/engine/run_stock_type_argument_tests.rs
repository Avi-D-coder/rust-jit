//! Whole unchanged official nested statement macros and actual generic calls.
//! An unsupported preparation/runtime result is a test failure, never acceptance.
use super::*;
use crate::syntax::spans::SourceRange;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasGenericParams, HasName},
};
use std::collections::BTreeSet;

const NESTED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);

fn actual_nested_statement_main(edition: u16) {
    let mut h = start(NESTED, edition, "main");
    let main = h.run.frame.body.body().clone();
    assert_eq!(h.parent.context().source().body().key(), main);
    assert_eq!(h.run.frame.body.profile().edition, edition);
    assert_eq!(h.run.opcode_counters().unwrap().operations_started, 0);
    // A generic signature may be discovered, but no callee executable body is
    // prepared before the actual first call reaches ordinary call entry.
    assert!(h.db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => body == &main,
        QueryKey::Thunk(pc) => pc.body == main,
        _ => true,
    }));

    let source = h.parent.context().source();
    let index = source.index();
    assert!(
        index
            .nodes()
            .iter()
            .all(|node| { node.role != NodeRole::Expression(ExpressionKind::Macro) }),
        "completed statement macro output must be attached before ordinary execution"
    );
    let nominal_definitions = ["Bar", "Baz", "Xyzzy"].map(|name| {
        let mut items = h.parent.context().facts().items.iter().filter(|row| {
            row.item.def_key.declared_name == name
                && row.item.def_key.namespace == crate::source::manifest::Namespace::Type
                && row.item.def_key.item_kind == crate::source::manifest::ItemKind::Struct
        });
        let item = items
            .next()
            .expect("actual generated nominal declaration absent");
        assert!(
            items.next().is_none(),
            "actual generated nominal identity ambiguous"
        );
        item.item.def_key.fingerprint()
    });
    let call_nodes = index
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .collect::<Vec<_>>();
    assert_eq!(
        call_nodes.len(),
        3,
        "complete original main has three actual calls"
    );
    let mut call_sites = Vec::new();
    let mut type_sites = Vec::new();
    for (node, expected) in call_nodes.iter().zip(["Bar", "Baz", "Xyzzy"]) {
        let absolute = SourceRange::checked(
            source
                .body()
                .range
                .start
                .checked_add(node.range.start)
                .unwrap(),
            source
                .body()
                .range
                .start
                .checked_add(node.range.end)
                .unwrap(),
        )
        .unwrap();
        let syntax = source
            .parsed()
            .syntax_at(absolute, |node| ast::CallExpr::can_cast(node.kind()))
            .unwrap()
            .expect("indexed call must retain its actual CallExpr");
        let call = ast::CallExpr::cast(syntax).unwrap();
        let Some(ast::Expr::PathExpr(path)) = call.expr() else {
            panic!("official call must retain its actual PathExpr")
        };
        let segment = path.path().unwrap().segment().unwrap();
        assert_eq!(segment.name_ref().unwrap().text(), "static_assert_exists");
        let arguments = segment
            .generic_arg_list()
            .unwrap()
            .generic_args()
            .collect::<Vec<_>>();
        assert_eq!(arguments.len(), 1);
        let ast::GenericArg::TypeArg(argument) = &arguments[0] else {
            panic!("official turbofish must retain an actual TypeArg")
        };
        let ty = argument.ty().unwrap();
        let ast::Type::PathType(path_type) = &ty else {
            panic!("actual generated nominal Type must remain PathType")
        };
        assert_eq!(
            path_type
                .path()
                .unwrap()
                .segment()
                .unwrap()
                .name_ref()
                .unwrap()
                .text(),
            expected
        );
        let absolute_type = crate::syntax::ast_adapter::range(ty.syntax().text_range());
        let relative = SourceRange::checked(
            absolute_type
                .start
                .checked_sub(source.body().range.start)
                .unwrap(),
            absolute_type
                .end
                .checked_sub(source.body().range.start)
                .unwrap(),
        )
        .unwrap();
        let canonical_type = index
            .nodes_at_range(relative)
            .find(|node| node.role == NodeRole::Syntax)
            .expect("actual TypeArg must own a canonical indexed Syntax node");
        call_sites.push(index.pc(node.id, Phase::Entry).unwrap());
        type_sites.push(index.pc(canonical_type.id, Phase::Entry).unwrap());
    }
    assert_eq!(
        call_sites
            .iter()
            .map(|pc| pc.node)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        type_sites
            .iter()
            .map(|pc| pc.node)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    let report = completed(&mut h);
    assert!(matches!(report.value, ValueSummary::Unit));
    let counts = &report.opcode_counters;
    assert_eq!(counts.started_by_opcode[19], 3); // real zero-argument CallSeal
    assert_eq!(counts.started_by_opcode[20], 0); // no parameter slots
    assert_eq!(counts.started_by_opcode[21], 0);
    assert_eq!(counts.started_by_opcode[22], 0);
    assert_eq!(counts.started_by_opcode[23], 0);
    assert_eq!(counts.started_by_opcode[26], 0); // no legacy Assert
    assert_eq!(counts.operations_started, counts.operations_completed);
    assert!(counts.return_scopes_cleaned >= 4);
    assert_eq!(counts.panic_scopes_cleaned, 0);
    assert_eq!(h.run.peak_guest_frames, 2);
    assert!(h.run.calls.is_empty() && h.run.pending.is_empty());
    assert!(h.run.frame.active_scopes().is_empty());
    assert!(h.run.terminal);

    for (kind, sites) in [("EnteredCall", &call_sites), ("EnteredType", &type_sites)] {
        for site in sites {
            let key = if kind == "EnteredCall" {
                QueryKey::EnteredCall(site.clone())
            } else {
                QueryKey::EnteredType(site.clone())
            };
            assert_eq!(
                h.db.trace
                    .events
                    .iter()
                    .filter(|event| { event.key == key && event.action == Action::ProviderRun })
                    .count(),
                1,
                "actual entered source goal must be retained: {key:?}"
            );
            assert!(h.db.memo(&key).is_some());
        }
    }
    // Only inspect Type goals after execution has actually exercised them;
    // fetching a goal here must not manufacture its runtime-route evidence.
    for (site, nominal) in type_sites.into_iter().zip(nominal_definitions) {
        let value = ready(h.db.fetch(&SyntaxProvider, QueryKey::EnteredType(site)));
        let QueryValue::EnteredType(goal) = value.value.as_ref() else {
            panic!("actual Type argument goal variant differs")
        };
        assert!(
            matches!(goal.source_type().source(),
            crate::traits::term::TyTerm::Nominal(definition, args)
                if *definition == nominal && args.is_empty()),
            "actual Type use must select its genuine generated declaration"
        );
    }
    let children =
        h.db.trace
            .events
            .iter()
            .filter_map(|event| match &event.key {
                QueryKey::Thunk(pc) if pc.body != main && event.action == Action::ProviderRun => {
                    Some(pc.body.clone())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
    assert_eq!(
        children.len(),
        1,
        "three actual calls share the genuine generic source body"
    );
    let child = children.into_iter().next().unwrap();
    // Observe the body/source that execution actually used. An empty module
    // function need not prepare a lexical namespace merely for this observer.
    let structure = ready(h.db.fetch(&SyntaxProvider, QueryKey::BodyStructure(child.clone())));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        panic!("actual executed generic callee body differs")
    };
    assert_eq!(index.body(), &child);
    let source =
        h.db.input(&InputKey::DefinitionSource(child.definition))
            .unwrap();
    let InputValue::DefinitionSource(source) = source.as_ref() else {
        panic!("actual executed generic callee source differs")
    };
    let body =
        h.db.input(&InputKey::DefinitionBody(child.definition))
            .unwrap();
    let InputValue::DefinitionBody(body) = body.as_ref() else {
        panic!("actual executed generic callee body locator differs")
    };
    assert_eq!(body.key(), child);
    let parsed = ready(h.db.fetch(&SyntaxProvider, QueryKey::ParseFile(source.clone())));
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        panic!("actual executed generic callee parse differs")
    };
    assert_eq!(parsed.key(), *source);
    let block = parsed
        .syntax_at(body.range, |node| ast::BlockExpr::can_cast(node.kind()))
        .unwrap()
        .expect("actual generic callee retains its original body AST");
    let function = block
        .parent()
        .and_then(ast::Fn::cast)
        .expect("actual child body belongs to its genuine source function");
    assert_eq!(function.name().unwrap().text(), "static_assert_exists");
    assert_eq!(
        function
            .generic_param_list()
            .unwrap()
            .generic_params()
            .count(),
        1
    );
    assert!(h.db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => body == &main || body == &child,
        QueryKey::Thunk(pc) => pc.body == main || pc.body == child,
        _ => true,
    }));
    h.db.finish_run().unwrap();
}

#[test]
fn unchanged_nested_statement_macro_main_uses_actual_type_arguments_2015() {
    actual_nested_statement_main(2015);
}

#[test]
fn unchanged_nested_statement_macro_main_uses_actual_type_arguments_all_editions() {
    for edition in EDITIONS {
        actual_nested_statement_main(edition);
    }
}
