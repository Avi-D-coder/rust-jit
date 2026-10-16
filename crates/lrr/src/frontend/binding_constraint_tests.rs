//! Real common source-goal joins, not interpreter/full-fixture acceptance.
use super::*;
use crate::{
    frontend::binding_constraints::BindingConstraintState, ir::op::PrimitiveType,
    syntax::source_constraints::FactKind,
};
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
const RADIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-radix.rs"
);
#[test]
fn unchanged_suffix_shadowed_bindings_use_actual_later_parameter_source_goals() {
    let expected = [
        PrimitiveType::I8,
        PrimitiveType::I16,
        PrimitiveType::I32,
        PrimitiveType::I64,
        PrimitiveType::Usize,
        PrimitiveType::U8,
        PrimitiveType::U16,
        PrimitiveType::U32,
        PrimitiveType::U64,
    ];
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(SUFFIX, edition, false);
        let body = local.context().source().index().body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let targets: Vec<_> = graph
            .facts()
            .iter()
            .enumerate()
            .filter(|(_, fact)| fact.kind() == FactKind::Let && fact.annotation().is_none())
            .map(|(id, _)| crate::ir::pc::PcKey {
                body: body.clone(),
                node: u32::try_from(id).unwrap(),
                phase: 0,
            })
            .collect();
        assert_eq!(targets.len(), expected.len());
        for (key, expected) in targets.into_iter().zip(expected) {
            let start = db.trace.events.len();
            let query = QueryKey::BindingConstraints(key);
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let QueryValue::BindingConstraints(source) = first.value.as_ref() else {
                panic!();
            };
            assert_eq!(
                source.state(),
                BindingConstraintState::Resolved(expected),
                "real binding goal {source:?}"
            );
            let second = ready(db.fetch(&SyntaxProvider, query));
            assert!(Arc::ptr_eq(&first.value, &second.value));
            assert!(
                !db.trace.events[start..].iter().any(|event| matches!(
                    event.key,
                    QueryKey::Thunk(_) | QueryKey::PatternPlan(_)
                ))
            );
            assert!(db.trace.events[start..].iter().all(
                |event| !matches!(&event.key, QueryKey::BodyStructure(other) if other != &body)
            ));
        }
        for item in local
            .context()
            .facts()
            .items
            .iter()
            .filter(|row| row.item.def_key.item_kind == ItemKind::Function)
        {
            let definition = item.item.def_key.fingerprint();
            let memo = db
                .memo(&QueryKey::FunctionSignature(definition))
                .expect("actual selected signature memo");
            assert!(!memo.dependencies.iter().any(|dependency| matches!(dependency,
                crate::db::dependency::DependencyStamp::Input { key: InputKey::DefinitionBody(owner), .. } if *owner == definition)),
                "declared parameter source must not depend on a future executable body locator");
        }
    }
}

#[test]
fn unchanged_radix_suffixed_literals_never_become_inference_variables() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(RADIX, edition, false);
        let body = local.context().source().index().body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let bindings: Vec<_> = graph
            .facts()
            .iter()
            .enumerate()
            .filter(|(_, fact)| fact.kind() == FactKind::Let)
            .map(|(id, _)| id)
            .collect();
        assert_eq!(bindings.len(), 6);
        assert_eq!(
            graph
                .facts()
                .iter()
                .filter(|fact| fact.kind() == FactKind::SuffixedInteger)
                .count(),
            6
        );
        for node in bindings {
            let start = db.trace.events.len();
            let key = crate::ir::pc::PcKey {
                body: body.clone(),
                node: u32::try_from(node).unwrap(),
                phase: 0,
            };
            let result = ready(db.fetch(&SyntaxProvider, QueryKey::BindingConstraints(key)));
            let QueryValue::BindingConstraints(source) = result.value.as_ref() else {
                panic!();
            };
            assert_eq!(source.state(), BindingConstraintState::NoNumericDemand);
            assert!(!db.trace.events[start..].iter().any(|event| matches!(
                event.key,
                QueryKey::EnteredValue(_)
                    | QueryKey::DeclaredParameterGoal { .. }
                    | QueryKey::Thunk(_)
            )));
        }
    }
}

#[test]
fn ordinary_binding_phases_share_one_prepared_guard_and_reject_revoked_source() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, local) = world(SUFFIX, edition, false);
        let index = local.context().source().index();
        let body = index.body().clone();
        let graph = ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
        let QueryValue::SourceConstraints(graph) = graph.value.as_ref() else {
            panic!();
        };
        let node = graph
            .facts()
            .iter()
            .position(|fact| fact.kind() == FactKind::Let && fact.annotation().is_none())
            .unwrap();
        let key = index
            .pc(
                crate::ir::pc::NodeId(u32::try_from(node).unwrap()),
                crate::ir::pc::Phase::Entry,
            )
            .unwrap();
        let prepared = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedBindingPlan(key.clone())));
        let QueryValue::PreparedBindingPlan(plan) = prepared.value.as_ref() else {
            panic!();
        };
        assert_eq!(
            plan.constraints().state(),
            BindingConstraintState::Resolved(PrimitiveType::I8)
        );
        let guard = plan
            .guard()
            .expect("resolved ordinary plan retains its actual input closure");
        let owners = Arc::strong_count(guard);
        let mut recipes = Vec::new();
        for phase in [crate::ir::pc::Phase::Entry, crate::ir::pc::Phase::Commit] {
            let phase_key = index.pc(crate::ir::pc::NodeId(key.node), phase).unwrap();
            let value = ready(db.fetch(&SyntaxProvider, QueryKey::Thunk(phase_key)));
            let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                panic!();
            };
            assert_eq!(
                recipe.prepared_inputs_fingerprint(),
                Some(guard.fingerprint())
            );
            recipe.revalidate_namespace(&db).unwrap();
            recipes.push(value);
        }
        assert_eq!(
            Arc::strong_count(guard),
            owners + 2,
            "each ordinary phase shares the original guard Arc"
        );
        assert_eq!(
            db.trace.counts.get("PreparedBindingPlan:ProviderRun"),
            Some(&1)
        );
        assert_eq!(
            db.trace.counts.get("BindingConstraints:ProviderRun"),
            Some(&1)
        );
        assert_eq!(
            db.trace.counts.get("SourceConstraints:ProviderRun"),
            Some(&1)
        );
        db.begin_expansion_epoch(2).unwrap();
        let before = db.trace.events.len();
        for value in recipes {
            let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                panic!();
            };
            assert_eq!(
                recipe.revalidate_namespace(&db).unwrap_err().code,
                "LRR-E-INTERNAL-SNAPSHOT"
            );
        }
        assert_eq!(
            db.trace.events.len(),
            before,
            "installed guards must not fetch or replan source"
        );
    }
}
