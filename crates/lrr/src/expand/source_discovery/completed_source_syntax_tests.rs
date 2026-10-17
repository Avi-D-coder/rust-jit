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
            .generated_declaration_demand(&nested, invocation)
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
