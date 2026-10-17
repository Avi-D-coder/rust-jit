//! Base-name facts over whole pinned originals; generic argument semantics stay
//! with their actual Type/const services and ordinary contracts, never strings.
use super::*;
use crate::{
    db::trace::Action,
    frontend::{
        entered_call_goals::{
            EnteredCallGoal, EnteredCallSelection, EnteredGenericArgumentKind, EnteredSourceName,
            EnteredTypeArgument,
        },
        source_locals::owned_plan::type_declarations::{BodyTypeDemand, BodyTypeOutcome},
    },
    ir::pc::Phase,
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        signatures::GenericKind,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};
const NESTED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
// This selected single original/import row is root-owned; no fixture or
// manifest mutation is performed by this test leaf.
const LOCAL_CONST: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/const-generics/issues/issue-75299.rs"
);
fn goal(value: &QueryValue) -> &EnteredCallGoal {
    let QueryValue::EnteredCall(goal) = value else {
        panic!("actual entered call query differs")
    };
    goal
}
fn assert_actual_arguments(plan: &PreparedSourceLocalPlan, target: &EnteredSourceName) {
    let source = plan.context().source();
    let actual = source
        .parsed()
        .entered_node_syntax(source.body(), source.index(), target.node())
        .unwrap()
        .and_then(ast::PathExpr::cast)
        .unwrap();
    let list = actual
        .path()
        .unwrap()
        .segment()
        .unwrap()
        .generic_arg_list()
        .unwrap();
    let actual = list.generic_args().collect::<Vec<_>>();
    assert_eq!(actual.len(), target.arguments().len());
    for (actual, observed) in actual.iter().zip(target.arguments()) {
        assert_eq!(
            observed.range(),
            crate::syntax::ast_adapter::range(actual.syntax().text_range())
        );
        assert!(target.range().contains(observed.range()));
        let kind = match actual {
            ast::GenericArg::TypeArg(_) => EnteredGenericArgumentKind::Type,
            ast::GenericArg::ConstArg(_) => EnteredGenericArgumentKind::Const,
            ast::GenericArg::LifetimeArg(_) => EnteredGenericArgumentKind::Lifetime,
            ast::GenericArg::AssocTypeArg(_) => EnteredGenericArgumentKind::AssociatedType,
        };
        assert_eq!(observed.kind(), kind);
        if let ast::GenericArg::TypeArg(argument) = actual {
            let ty = argument.ty().unwrap();
            let EnteredTypeArgument::Indexed(site) = observed.type_argument().unwrap() else {
                panic!("real fixture Type argument lacks its actual Syntax location")
            };
            assert_eq!(
                site.range(),
                crate::syntax::ast_adapter::range(ty.syntax().text_range())
            );
            assert_eq!(site.key().body, *source.index().body());
            assert_eq!(site.key().phase, 0);
            assert_eq!(site.is_path(), matches!(ty, ast::Type::PathType(_)));
            assert_eq!(
                source
                    .body()
                    .range
                    .start
                    .checked_add(site.indexed_range().start),
                Some(site.range().start)
            );
            assert_eq!(
                source
                    .body()
                    .range
                    .start
                    .checked_add(site.indexed_range().end),
                Some(site.range().end)
            );
            let canonical = source
                .index()
                .nodes_at_range(site.indexed_range())
                .find(|node| node.role == NodeRole::Syntax)
                .unwrap();
            assert_eq!(site.key().node, canonical.id.0);
        } else {
            assert!(observed.type_argument().is_none());
        }
    }
}
fn no_argument_type_or_body_preparation(db: &Database, start: usize) {
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::Constant(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::TypeSchema(_)
                | QueryKey::TypeAlias(_)
                | QueryKey::EnteredType(_)
                | QueryKey::PreparedTypePlan(_)
        )),
        "base Value fact prepared explicit argument semantics or child code"
    );
}
fn source(mode: usize, original: &str) -> String {
    match mode {
        0 => original.to_owned(),
        1 => format!("macro_rules! emit{{()=>{{{original}}}}}emit!();"),
        _ => format!(
            "macro_rules! emit{{()=>{{{original}}}}}macro_rules! wrap{{()=>{{emit!();}}}}wrap!();"
        ),
    }
}
#[test]
fn whole_nested_statement_macro_calls_retain_module_base_facts_and_written_types() {
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..3 {
            let text = source(mode, NESTED);
            // Real original body macro output is attached before the ordinary
            // owning plan, not a synthetic selected call/function body.
            let (mut db, plan) = world(&text, edition, mode == 0);
            let index = plan.context().source().index();
            let sites = index
                .nodes()
                .iter()
                .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
                .map(|node| index.pc(node.id, Phase::Entry).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(sites.len(), 3);
            let start = db.trace.events.len();
            for (site, written) in sites.iter().zip(["Bar", "Baz", "Xyzzy"]) {
                let key = QueryKey::EnteredCall(site.clone());
                let first = ready(db.fetch(&SyntaxProvider, key.clone()));
                let observation = goal(first.value.as_ref());
                let EnteredCallSelection::Module(declaration) = observation.selection() else {
                    panic!("explicit source module name was not selected")
                };
                assert!(declaration.owner().is_none());
                assert!(observation.value_identity().is_some());
                let target = observation.written_source_name().unwrap();
                assert_eq!(target.name(), "static_assert_exists");
                assert_eq!(target.explicit(), [written]);
                assert_actual_arguments(&plan, target);
                assert_eq!(
                    target.arguments()[0].kind(),
                    EnteredGenericArgumentKind::Type
                );
                assert_eq!(
                    target.indexed_range(),
                    index.nodes()[target.node().0 as usize].range
                );
                assert!(
                    observation
                        .indexed_local_source_name(index, site)
                        .unwrap()
                        .is_none()
                );
                let signature = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::FunctionSignature(declaration.identity()),
                ));
                let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
                    panic!()
                };
                assert_eq!(signature.generics.len(), 1);
                assert_eq!(signature.generics[0].kind, GenericKind::Type);
                let second = ready(db.fetch(&SyntaxProvider, key.clone()));
                assert!(Arc::ptr_eq(&first.value, &second.value));
                assert_eq!(
                    db.trace.events[start..]
                        .iter()
                        .filter(|event| event.key == key && event.action == Action::ProviderRun)
                        .count(),
                    1
                );
            }
            no_argument_type_or_body_preparation(&db, start);
            let retained =
                ready(db.fetch(&SyntaxProvider, QueryKey::EnteredCall(sites[0].clone())));
            db.begin_expansion_epoch(2).unwrap();
            assert!(goal(retained.value.as_ref()).require_current(&db).is_err());
        }
    }
}
#[test]
fn whole_official_local_const_generic_call_admits_only_its_base_name_and_header() {
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..3 {
            let text = source(mode, LOCAL_CONST);
            let (mut db, plan) = world(&text, edition, false);
            let index = plan.context().source().index();
            let calls = index
                .nodes()
                .iter()
                .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), 1);
            let site = index.pc(calls[0].id, Phase::Entry).unwrap();
            let start = db.trace.events.len();
            let key = QueryKey::EnteredCall(site.clone());
            let first = ready(db.fetch(&SyntaxProvider, key.clone()));
            let observation = goal(first.value.as_ref());
            let EnteredCallSelection::Local(admission) = observation.selection() else {
                panic!("actual local constgeneric base name was not selected")
            };
            let target = observation
                .local_source_name(&db, index, &site)
                .unwrap()
                .unwrap();
            assert_eq!(target.name(), "foo");
            assert_eq!(target.explicit(), ["1"]);
            assert_actual_arguments(&plan, target);
            assert_eq!(
                target.arguments()[0].kind(),
                EnteredGenericArgumentKind::Const
            );
            assert!(target.arguments()[0].type_argument().is_none());
            assert_eq!(target.node(), index.nodes()[site.node as usize].operands[0]);
            let signature = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::FunctionSignature(admission.item().def_key.fingerprint()),
            ));
            let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
                panic!()
            };
            assert_eq!(signature.generics.len(), 1);
            assert_eq!(signature.generics[0].kind, GenericKind::Const);
            assert!(!signature.result.is_empty()); // Actual array return remains metadata.
            assert!(
                db.input(&InputKey::DefinitionBody(signature.definition))
                    .is_none()
            );
            let second = ready(db.fetch(&SyntaxProvider, key.clone()));
            assert!(Arc::ptr_eq(&first.value, &second.value));
            assert_eq!(
                db.trace.events[start..]
                    .iter()
                    .filter(|event| event.key == key && event.action == Action::ProviderRun)
                    .count(),
                1
            );
            no_argument_type_or_body_preparation(&db, start);
        }
    }
}

#[test]
fn body_local_primitive_spelling_alias_retains_actual_type_site_and_refuses_primitive_goal() {
    const ALIAS: &str = "fn main(){type u8=u16;fn f<T>(){} f::<u8>();}";
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..3 {
            let text = source(mode, ALIAS);
            let (mut db, plan) = world(&text, edition, false);
            let index = plan.context().source().index();
            let call = index
                .nodes()
                .iter()
                .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
                .unwrap();
            let key = index.pc(call.id, Phase::Entry).unwrap();
            let start = db.trace.events.len();
            let value = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredCall(key.clone())));
            let observation = goal(value.value.as_ref());
            assert!(matches!(
                observation.selection(),
                EnteredCallSelection::Local(_)
            ));
            let target = observation
                .local_source_name(&db, index, &key)
                .unwrap()
                .unwrap();
            assert_eq!(target.explicit(), ["u8"]);
            assert_actual_arguments(&plan, target);
            no_argument_type_or_body_preparation(&db, start);
            let EnteredTypeArgument::Indexed(site) = target.arguments()[0].type_argument().unwrap()
            else {
                panic!()
            };
            assert!(site.is_path());
            let typed =
                ready(plan.prepare_body_type(&mut db, crate::ir::pc::NodeId(site.key().node)));
            assert!(
                matches!(
                    typed,
                    BodyTypeOutcome::Pending(BodyTypeDemand::AliasContext(_))
                ),
                "actual body alias must not acquire primitive source authority"
            );
            match db.fetch(&SyntaxProvider, QueryKey::EnteredType(site.key().clone())) {
                QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-TYPE-CONTEXT"),
                _ => panic!("body-local alias u8 became an entered primitive Type"),
            }
        }
    }
}

#[test]
fn other_generic_argument_kinds_retain_ranges_without_type_grants() {
    // Source metadata control only; no static checking or generic call execution.
    const SOURCE: &str = "fn f<'a,T>(){} fn main(){f::<'static, Item=u8, &u16>();}";
    let (mut db, plan) = world(SOURCE, 2024, false);
    let index = plan.context().source().index();
    let call = index
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .unwrap();
    let start = db.trace.events.len();
    let value = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::EnteredCall(index.pc(call.id, Phase::Entry).unwrap()),
    ));
    let target = goal(value.value.as_ref()).written_source_name().unwrap();
    assert_actual_arguments(&plan, target);
    assert_eq!(
        target
            .arguments()
            .iter()
            .map(|argument| argument.kind())
            .collect::<Vec<_>>(),
        [
            EnteredGenericArgumentKind::Lifetime,
            EnteredGenericArgumentKind::AssociatedType,
            EnteredGenericArgumentKind::Type
        ]
    );
    assert!(target.arguments()[0].type_argument().is_none());
    assert!(target.arguments()[1].type_argument().is_none());
    let EnteredTypeArgument::Indexed(site) = target.arguments()[2].type_argument().unwrap() else {
        panic!()
    };
    assert!(!site.is_path()); // Structural Type metadata is not a Q42 named Type grant.
    no_argument_type_or_body_preparation(&db, start);
}
