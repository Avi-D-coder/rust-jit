//! Structural source controls; no runtime or discovery-certificate acceptance.
use super::*;
use crate::{
    db::seals::SealDomain,
    expand::source_declarative::{
        SourceDeclarativeDemand,
        assembly::{
            SourceSyntaxAssembly,
            tests::{output, request, world},
        },
    },
    frontend::impl_index::{DeclarationMacroParent, GeneratedDeclarationMacroIndex},
};
const STOCK: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
fn seal(epoch: u64) -> SealKey {
    SealKey {
        domain: SealDomain::TestCatalog {
            id: Digest([91; 32]),
        },
        unit_graph: Digest([92; 32]),
        epoch,
    }
}
#[test]
fn retained_inputs_replay_actual_nested_stock_outputs_and_issued_receipts() {
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, index, _) = world(STOCK, edition);
        let outer = request(&provider, &index);
        let generated = output(&provider, index.scan(), &outer, 1);
        let nested = GeneratedDeclarationMacroIndex::new(
            generated.clone(),
            DeclarationMacroParent::Source(index.clone()),
        )
        .unwrap();
        let invocation = generated
            .scan()
            .producers()
            .iter()
            .find(|p| p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro)
            .unwrap()
            .identity;
        let SourceDeclarativeDemand::Ready(inner) = provider
            .generated_declaration_demand(&nested, invocation, None)
            .unwrap()
        else {
            panic!("actual nested request incomplete")
        };
        let child = output(&provider, generated.scan(), &inner, 1);
        let expected = SourceSyntaxAssembly::new(&provider, &index, 1)
            .unwrap()
            .attach(&outer, generated.clone())
            .unwrap()
            .attach(&inner, child.clone())
            .unwrap()
            .finish()
            .unwrap();
        let seed = SourceAssemblySeed::prepare(&provider, &index, 1).unwrap();
        let mut replies = BTreeMap::new();
        for (issued, actual) in [(&outer, &generated), (&inner, &child)] {
            replies.insert(
                (actual.parent_inventory(), actual.parent_invocation()),
                SyntaxReply {
                    output: actual.clone(),
                    receipt: Some(Arc::new(SyntaxRequestReceipt::capture(issued).unwrap())),
                },
            );
        }
        let plan = CompletedSourceSyntax::capture(seed.clone(), replies, seal(1), Digest([93; 32]))
            .unwrap();
        let before = plan.fingerprint();
        for _ in 0..2 {
            let actual = plan.prepare_source().unwrap();
            assert_eq!(actual.identity(), expected.identity());
            assert_eq!(actual.source(), expected.source());
            assert_eq!(actual.outputs().len(), 2);
            assert!(actual.pending_calls().is_empty());
        }
        assert_eq!(plan.fingerprint(), before);
        let main = provider
            .module_source()
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "main")
            .unwrap()
            .def_key
            .fingerprint();
        let missing = CompletedSourceSyntax::capture(
            seed.clone(),
            BTreeMap::new(),
            seal(1),
            Digest([93; 32]),
        )
        .unwrap();
        assert!(
            missing
                .prepare_body(main)
                .err()
                .unwrap()
                .message
                .contains("actual expansion output")
        );
        let mut no_receipt = BTreeMap::new();
        no_receipt.insert(
            (generated.parent_inventory(), generated.parent_invocation()),
            SyntaxReply {
                output: generated,
                receipt: None,
            },
        );
        // Absence is pending metadata at completion, never attachment authority.
        let missing =
            CompletedSourceSyntax::capture(seed.clone(), no_receipt, seal(1), Digest([93; 32]))
                .unwrap();
        assert!(
            missing
                .prepare_body(main)
                .err()
                .unwrap()
                .message
                .contains("actual issued request receipt")
        );
        assert!(
            CompletedSourceSyntax::capture(seed, BTreeMap::new(), seal(2), Digest([93; 32]))
                .is_err()
        );
    }
}
#[test]
fn unselected_function_missing_output_remains_cold() {
    let text = "macro_rules! value { () => { 1_u8 } } fn cold() { value!(); } fn main() {}";
    let (provider, index, _) = world(text, 2024);
    let main = provider
        .module_source()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let cold = provider
        .module_source()
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "cold")
        .unwrap()
        .def_key
        .fingerprint();
    let seed = SourceAssemblySeed::prepare(&provider, &index, 1).unwrap();
    let plan =
        CompletedSourceSyntax::capture(seed, BTreeMap::new(), seal(1), Digest([93; 32])).unwrap();
    let selected = plan.prepare_body(main).unwrap();
    assert!(selected.permits_body(main));
    assert!(!selected.permits_body(cold));
    assert!(!selected.pending_calls().is_empty());
    assert!(plan.prepare_body(cold).is_err());
    let issued = request(&provider, &index);
    let actual = output(&provider, index.scan(), &issued, 1);
    let mut replies = BTreeMap::new();
    replies.insert(
        (actual.parent_inventory(), actual.parent_invocation()),
        SyntaxReply {
            output: actual,
            receipt: None,
        },
    );
    let seed = SourceAssemblySeed::prepare(&provider, &index, 1).unwrap();
    let plan = CompletedSourceSyntax::capture(seed, replies, seal(1), Digest([93; 32])).unwrap();
    assert!(plan.prepare_body(main).unwrap().permits_body(main));
    assert!(
        plan.prepare_body(cold)
            .err()
            .unwrap()
            .message
            .contains("actual issued request receipt")
    );
}
#[test]
fn generated_function_preparation_attaches_only_its_chain_and_own_calls() {
    let text = "macro_rules! unit { () => { () } } \
                macro_rules! mk { () => { fn g() { unit!(); } fn h() { unit!(); } } } \
                mk!(); fn main() { unit!(); }";
    for edition in [2015, 2021] {
        let (provider, index, _) = world(text, edition);
        let item = |scan: &crate::syntax::declaration_scan::DeclarationScan| {
            scan.producers()
                .iter()
                .find(|p| {
                    p.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Macro
                        && p.grammar == crate::expand::identity::GrammarSlot::Items
                })
                .unwrap()
                .identity
        };
        let invocation = item(index.scan());
        let SourceDeclarativeDemand::Ready(outer) = provider
            .declaration_demand(
                &index,
                invocation,
                Digest([17; 32]),
                crate::expand::source_declarative::engine_version(),
            )
            .unwrap()
        else {
            panic!("actual item request incomplete")
        };
        let generated = output(&provider, index.scan(), &outer, 1);
        let nested = GeneratedDeclarationMacroIndex::new(
            generated.clone(),
            DeclarationMacroParent::Source(index.clone()),
        )
        .unwrap();
        let mut replies = BTreeMap::new();
        replies.insert(
            (generated.parent_inventory(), generated.parent_invocation()),
            SyntaxReply {
                output: generated.clone(),
                receipt: Some(Arc::new(SyntaxRequestReceipt::capture(&outer).unwrap())),
            },
        );
        for producer in generated.scan().producers() {
            let SourceDeclarativeDemand::Ready(inner) = provider
                .generated_declaration_demand(&nested, producer.identity, None)
                .unwrap()
            else {
                panic!("actual generated body request incomplete")
            };
            let child = output(&provider, generated.scan(), &inner, 1);
            replies.insert(
                (child.parent_inventory(), child.parent_invocation()),
                SyntaxReply {
                    output: child,
                    receipt: Some(Arc::new(SyntaxRequestReceipt::capture(&inner).unwrap())),
                },
            );
        }
        assert_eq!(replies.len(), 3);
        let header = |name: &str| {
            generated
                .headers()
                .iter()
                .find(|h| h.name.as_ref().is_some_and(|n| n.text == name))
                .unwrap()
                .clone()
        };
        let (g, h) = (header("g"), header("h"));
        let seed = SourceAssemblySeed::prepare(&provider, &index, 1).unwrap();
        let plan =
            CompletedSourceSyntax::capture(seed, replies, seal(1), Digest([93; 32])).unwrap();
        let mut bearing = plan.generated_macro_bearing_functions();
        bearing.sort();
        let mut expected = vec![g.key.fingerprint(), h.key.fingerprint()];
        expected.sort();
        assert_eq!(bearing, expected);
        for _ in 0..2 {
            let prepared = plan.prepare_body(g.key.fingerprint()).unwrap();
            assert!(prepared.permits_body(g.key.fingerprint()));
            assert!(!prepared.permits_body(h.key.fingerprint()));
            let anchor = prepared.selected_generated().unwrap();
            assert_eq!(anchor.definition(), g.key.fingerprint());
            assert_eq!(anchor.record(), generated.record().key().fingerprint());
            assert_eq!(
                (anchor.output_range().start, anchor.output_range().end),
                (g.start, g.end)
            );
            // Only the producer chain and g's own call are attached; the
            // calls owned by h and main stay pending structural markers.
            assert_eq!(prepared.outputs().len(), 2);
            assert_eq!(prepared.pending_calls().len(), 2);
            let syntax = &prepared.syntax().syntax().text;
            let body =
                &syntax[anchor.body_range().start as usize..anchor.body_range().end as usize];
            assert!(!body.contains("unit!"), "{body}");
            assert!(body.contains("()"), "{body}");
            let declaration = &syntax[anchor.declaration_range().start as usize
                ..anchor.declaration_range().end as usize];
            assert!(declaration.starts_with("fn") && declaration.ends_with('}'));
        }
        assert_ne!(
            plan.prepare_body(g.key.fingerprint()).unwrap().identity(),
            plan.prepare_body(h.key.fingerprint()).unwrap().identity()
        );
        assert!(plan.prepare_body(Digest([5; 32])).is_err());
    }
}
