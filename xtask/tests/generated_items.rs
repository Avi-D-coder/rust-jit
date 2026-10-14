use lrr::{
    core::digest::{Digest, StableEncode},
    expand::{
        generated_items::GeneratedItems,
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
    },
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::{Edition, Key},
};
use std::sync::Arc;
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
fn key() -> ExpansionKey {
    ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: Digest([2; 32]),
        input_tokens: Digest([3; 32]),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest([5; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    }
}
fn output(rules: &str) -> Arc<ExpansionRecord> {
    let d = DeclarativeMacro::new(fixture::definition(rules, Edition::E2024)).unwrap();
    let input = fixture::input("");
    let (out, _) = d
        .expand_hygienic_with_limits(
            &input,
            fixture::call_site(&input),
            DeclarativeSite {
                expansion: Key(key().fingerprint().0),
                defining_unit: Key([1; 32]),
                definition_edition: Edition::E2024,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    Arc::new(ExpansionRecord::from_wire(key(), out.unwrap().tokens.encode().unwrap()).unwrap())
}
#[test]
fn actual_generated_items_preserve_import_macro_and_cold_function_headers_with_owned_origins() {
    let r = output(
        "()=>{pub use source::chosen;macro_rules! next{()=>{pub fn generated()->i64{7_i64}}}pub fn cold(){let _:Missing=missing();}};",
    );
    let g = GeneratedItems::admit(&key(), r.clone(), 1).unwrap();
    assert_eq!(g.items().len(), 3);
    assert_eq!(
        fixture::literal_texts(
            &lrr_expand_adapter::token_wire::TokenForest::decode(&g.tokens().encode().unwrap())
                .unwrap()
        ),
        vec!["7_i64".to_string()]
    );
    assert!(!g.origins().is_empty());
    assert_eq!(g.items()[1].name.as_ref().unwrap().text, "next");
    assert_eq!(g.items()[2].name.as_ref().unwrap().text, "cold");
    for origin in g.origins() {
        assert!(
            g.tokens()
                .spans()
                .iter()
                .any(|s| s.key().0 == origin.span.0)
        );
    }
    for (i, item) in g.items().iter().enumerate() {
        assert_eq!(item.key.expansion, key().fingerprint());
        assert_eq!(item.key.structural_path, vec![i as u32]);
        item.key.validate().unwrap();
    }
    let again = GeneratedItems::admit(&key(), r, 1).unwrap();
    assert_eq!(g.items(), again.items());
}
#[test]
fn generated_item_admission_rejects_wrong_key_epoch_and_item_grammar() {
    let r = output("()=>{pub fn generated(){}};");
    let mut wrong = key();
    wrong.parent_context = Digest([9; 32]);
    assert_eq!(
        GeneratedItems::admit(&wrong, r.clone(), 1)
            .unwrap_err()
            .code,
        "LRR-E-EXPANSION-INPUT"
    );
    assert_eq!(
        GeneratedItems::admit(&key(), r, 0).unwrap_err().code,
        "LRR-E-EXPANSION-EPOCH"
    );
    let r = output("()=>{1_i64+2_i64};");
    assert_eq!(
        GeneratedItems::admit(&key(), r, 1).unwrap_err().code,
        "LRR-E-MACRO-OUTPUT"
    );
}
#[test]
fn generated_function_header_identity_excludes_body_text_but_keeps_source_origins() {
    let g =
        GeneratedItems::admit(&key(), output("()=>{pub fn cold()->i64{missing()}};"), 1).unwrap();
    let item = &g.items()[0];
    assert!(g.text()[item.header_start as usize..item.header_end as usize].contains("cold"));
    assert!(!g.text()[item.header_start as usize..item.header_end as usize].contains("missing"));
    assert!(g.text().contains("missing"));
    assert!(
        g.origins()
            .iter()
            .any(|o| o.start <= item.name.as_ref().unwrap().start
                && o.end > item.name.as_ref().unwrap().start)
    );
}

#[test]
fn generated_record_epoch_check_does_not_authorize_procedural_execution() {
    let source = output("()=>{pub fn generated(){}};");
    let mut expected = key();
    expected.macro_identity = MacroIdentity::Procedural {
        artifact: Digest([9; 32]),
        export: "emit".into(),
        kind: lrr::expand::identity::ProceduralKind::Function,
    };
    expected.procedural_epoch = Some(7);
    let record = Arc::new(
        ExpansionRecord::from_wire(expected.clone(), source.tokens().unwrap().encode().unwrap())
            .unwrap(),
    );
    assert_eq!(
        GeneratedItems::admit(&expected, record, 8)
            .unwrap_err()
            .code,
        "LRR-E-EXPANSION-EPOCH"
    );
}
