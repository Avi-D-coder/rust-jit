//! Structural/query controls around an unchanged upstream numeric witness.
//! These are not interpreter execution or full-fixture acceptance.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{parse::ParsedFile, source_constraints::FactKind},
};
use std::sync::Arc;

const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
fn world(text: &str, edition: u16) -> (Database, BodyKey) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let unit = SourceUnitInput {
        path: SourcePath::new("upstream-source-constraints.rs").unwrap(),
        crate_key: Digest([221; 32]),
        module_path: vec![],
        profile: profile.clone(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    };
    let parsed = ParsedFile::parse(&unit.blob, profile).unwrap();
    let outline = parsed.outline(unit.crate_key, vec![], &unit.cfg).unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let prepared = PreparedSourceInputs::refresh_with_dependencies(
        None,
        vec![unit],
        Default::default(),
        vec![],
    )
    .unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    db.trace.detailed = true;
    (db, body)
}
#[test]
fn unchanged_suffix_source_constraints_retain_real_call_and_binding_nodes_once() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, body) = world(SUFFIX, edition);
        let key = QueryKey::SourceConstraints(body.clone());
        let first = db
            .fetch(&SyntaxProvider, key.clone())
            .into_ready()
            .unwrap()
            .unwrap();
        let second = db
            .fetch(&SyntaxProvider, key)
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&first.value, &second.value));
        let QueryValue::SourceConstraints(plan) = first.value.as_ref() else {
            panic!("source plan absent");
        };
        assert_eq!(plan.body(), &body);
        let structure = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!();
        };
        assert_eq!(plan.structure(), index.semantic_identity());
        assert_eq!(plan.facts().len(), index.nodes().len());
        assert_eq!(plan.paths_named("j").len(), 9);
        assert_eq!(plan.paths_named("r#j"), plan.paths_named("j"));
        assert_eq!(plan.paths_named("id_i8").len(), 2);
        assert!(plan.paths_named("not_a_source_use").is_empty());
        for (parent, fact) in plan.facts().iter().enumerate() {
            for (ordinal, child) in fact.operands().iter().enumerate() {
                if let Some(child) = child {
                    assert!(
                        plan.uses(*child)
                            .iter()
                            .any(|edge| edge.parent.0 as usize == parent
                                && edge.ordinal as usize == ordinal)
                    );
                }
            }
        }
        let calls: Vec<_> = plan
            .facts()
            .iter()
            .filter(|fact| fact.kind() == FactKind::Call)
            .collect();
        assert_eq!(calls.len(), 18);
        for call in calls {
            assert_eq!(call.operands().len(), 2);
            let callee = call.operands()[0].unwrap();
            assert_eq!(plan.fact(callee).unwrap().kind(), FactKind::Path);
            assert!(call.operands()[1].is_some());
        }
        let bindings: Vec<_> = plan
            .facts()
            .iter()
            .filter(|fact| fact.kind() == FactKind::Let)
            .collect();
        assert_eq!(bindings.len(), 18);
        assert_eq!(
            bindings
                .iter()
                .filter(|fact| fact.annotation().is_some())
                .count(),
            9
        );
        assert!(
            bindings
                .iter()
                .all(|fact| fact.operands().iter().all(Option::is_some))
        );
        assert!(
            plan.facts()
                .iter()
                .filter(|fact| fact.kind() == FactKind::Prefix)
                .all(|fact| fact.operator().is_some())
        );
        assert!(plan.estimated_bytes() <= 32 * 1024 * 1024);
        assert_eq!(
            db.trace.counts.get("SourceConstraints:ProviderRun"),
            Some(&1)
        );
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_) | QueryKey::FunctionSignature(_)
        )));
        assert!(
            db.trace.events.iter().all(
                |event| !matches!(&event.key, QueryKey::BodyStructure(other) if other != &body)
            )
        );
    }
}
#[test]
fn cold_source_call_shape_does_not_prepare_its_signature_or_executable_body() {
    for edition in [2015, 2018, 2021, 2024] {
        // Adaptation outside the imported original tests the preparation route,
        // not a new source-language acceptance example.
        for cold in ["7", "missing_cold_source_call()"] {
            let text = SUFFIX.replace(
                "let _i: i8 = -128;",
                &format!("let _i: i8 = if false {{ {cold} }} else {{ -128 }};"),
            );
            let (mut db, body) = world(&text, edition);
            let result = db
                .fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            assert!(
                result
                    .value
                    .matches(&QueryKey::SourceConstraints(body.clone()))
            );
            assert!(!db.trace.events.iter().any(|event| matches!(
                event.key,
                QueryKey::Thunk(_) | QueryKey::FunctionSignature(_)
            )));
            assert!(db.trace.events.iter().all(
                |event| !matches!(&event.key, QueryKey::BodyStructure(other) if other != &body)
            ));
        }
    }
}
#[test]
fn operator_binding_and_hidden_region_facts_are_retained_structurally() {
    use crate::syntax::source_constraints::SourceOperator as O;
    let text = "fn main() {\n    let n = 3;\n    let mut m = -n + 1u16;\n    m <<= 2;\n    m = m % 7;\n    if n == m { }\n    let _ = n;\n    let f = || n;\n    n;\n    unexpanded!(n);\n}\n";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, body) = world(text, edition);
        let value = db
            .fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::SourceConstraints(plan) = value.value.as_ref() else {
            panic!("source plan absent");
        };
        let operations: Vec<_> = plan
            .facts()
            .iter()
            .filter_map(|fact| fact.operation())
            .collect();
        assert_eq!(
            operations,
            [
                O::Arithmetic,
                O::Negate,
                O::CompoundShift,
                O::Assign,
                O::Arithmetic,
                O::Comparison
            ]
        );
        let suffixed = plan
            .facts()
            .iter()
            .filter(|fact| fact.kind() == FactKind::SuffixedInteger)
            .map(|fact| fact.suffix())
            .collect::<Vec<_>>();
        assert_eq!(suffixed, [Some(crate::ir::op::PrimitiveType::U16)]);
        // Every indexed `n` read (the closure body's included) is a
        // candidate path; the unexpanded macro call is retained only as a
        // hidden region that may read `n`.
        let row = plan.named("n").expect("binding spelling row");
        assert_eq!(row.paths().len(), 5);
        assert_eq!(row.bindings().len(), 1);
        assert_eq!(row.hidden().len(), 1);
        assert_eq!(plan.paths_named("n"), row.paths());
        let binding = row.bindings()[0];
        assert_eq!(plan.fact(binding).unwrap().kind(), FactKind::BindingPattern);
        assert_eq!(plan.name_of(binding).unwrap().0, "n");
        assert!(row.hidden()[0] > binding);
        // `let` pattern edges, the pattern engine's retained trees for every
        // `let` root (the wildcard included) and discarding statements.
        assert!(plan.uses(binding).iter().any(
            |edge| edge.ordinal == 0 && plan.fact(edge.parent).unwrap().kind() == FactKind::Let
        ));
        let occurrence = plan
            .occurrence(binding)
            .expect("binding occurrence in its tree");
        assert_eq!(occurrence.root, binding);
        assert!(occurrence.steps.is_empty() && !occurrence.by_reference);
        let lets: Vec<_> = plan
            .facts()
            .iter()
            .filter(|fact| fact.kind() == FactKind::Let)
            .map(|fact| fact.operand(0).unwrap())
            .collect();
        assert_eq!(lets.len(), 4);
        assert!(lets.iter().all(|root| plan.tree(*root).is_some()));
        assert!(lets.iter().any(|root| matches!(
            plan.tree(*root).unwrap().root(),
            crate::syntax::pattern_tree::PatternNode::Wildcard
        )));
        assert_eq!(
            plan.facts()
                .iter()
                .filter(|fact| fact.kind() == FactKind::Discard)
                .count(),
            4
        );
        let conditional = plan
            .facts()
            .iter()
            .find(|fact| fact.kind() == FactKind::If)
            .unwrap();
        assert_eq!(conditional.operands().len(), 3);
        assert!(conditional.operand(1).is_some() && conditional.operand(2).is_none());
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_) | QueryKey::FunctionSignature(_) | QueryKey::EnteredValue(_)
        )));
    }
}
