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
        parent_context: Digest(owner_context().0),
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

fn owner_context() -> lrr::syntax::token_wire::Key {
    use lrr::syntax::token_wire::{ContextRecord, Edition, Key, Transparency};
    ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([2; 32]),
        edition: Edition::E2024,
    }
    .key()
}
fn scope() -> lrr::syntax::outline::ScopeKey {
    lrr::syntax::outline::ScopeKey {
        crate_key: Digest([2; 32]),
        module_path: vec![],
        owner: None,
    }
}

fn fragment(
    rules: &str,
) -> (
    GeneratedItems,
    lrr::frontend::generated_macros::GeneratedMacroCatalog,
) {
    let g = GeneratedItems::admit(&key(), output(rules), 1).unwrap();
    let c = lrr::frontend::generated_macros::GeneratedMacroCatalog::for_discovery(
        &g,
        scope(),
        owner_context(),
        &lrr::source::snapshot::CfgSet::guest_default(),
        1,
    )
    .unwrap();
    (g, c)
}
#[test]
fn generated_fragment_resolves_owned_call_and_expands_real_definition() {
    use lrr::frontend::generated_macros::GeneratedCallDemand;
    let (g, mut c) = fragment("()=>{macro_rules! leaf{()=>{pub fn made(){7_i64;}}}leaf!();};");
    let GeneratedCallDemand::Ready(r) = c.next_call().unwrap() else {
        panic!("ready call absent")
    };
    assert_eq!(r.key().invocation, g.calls()[0].key.fingerprint());
    assert_ne!(r.key().parent_context, Digest(owner_context().0));
    let record = Arc::new(r.expand().unwrap());
    let out = c.complete_call(record).unwrap();
    assert_eq!(out.items()[0].name.as_ref().unwrap().text, "made");
    assert_eq!(
        fixture::literal_texts(
            &lrr_expand_adapter::token_wire::TokenForest::decode(&out.tokens().encode().unwrap())
                .unwrap()
        ),
        vec!["7_i64"]
    );
    assert!(matches!(
        c.next_call().unwrap(),
        GeneratedCallDemand::Complete
    ));
}
#[test]
fn generated_fragment_applies_cfg_before_scheduling_calls() {
    use lrr::frontend::generated_macros::GeneratedCallDemand;
    let (g, mut c) = fragment("()=>{macro_rules! leaf{()=>{}}#[cfg(any())]unknown!();leaf!();};");
    assert_eq!(g.calls().len(), 2);
    let GeneratedCallDemand::Ready(r) = c.next_call().unwrap() else {
        panic!("inactive producer blocked")
    };
    assert_eq!(r.key().invocation, g.calls()[1].key.fingerprint());
    c.complete_call(Arc::new(r.expand().unwrap())).unwrap();
    assert!(matches!(
        c.next_call().unwrap(),
        GeneratedCallDemand::Complete
    ));
}
#[test]
fn generated_fragment_unknown_preceding_call_keeps_later_call_pending() {
    use lrr::frontend::generated_macros::GeneratedCallDemand;
    let (g, mut c) = fragment("()=>{unknown!();macro_rules! leaf{()=>{}}leaf!();};");
    assert!(
        matches!(c.next_call().unwrap(),GeneratedCallDemand::NeedInput(id) if id==g.calls()[0].key.fingerprint())
    );
    assert!(
        matches!(c.next_call().unwrap(),GeneratedCallDemand::NeedInput(id) if id==g.calls()[0].key.fingerprint())
    );
}
#[test]
fn generated_fragment_rejects_wrong_reply_without_losing_pending_demand() {
    use lrr::frontend::generated_macros::GeneratedCallDemand;
    let (_, mut c) = fragment("()=>{macro_rules! leaf{()=>{}}leaf!();};");
    let GeneratedCallDemand::Ready(r) = c.next_call().unwrap() else {
        panic!("demand absent")
    };
    let record = Arc::new(r.expand().unwrap());
    let mut wrong = r.key().clone();
    wrong.invocation = Digest([249; 32]);
    let bad = Arc::new(ExpansionRecord::from_tokens(wrong, &record.tokens().unwrap()).unwrap());
    assert_eq!(
        c.complete_call(bad).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    let GeneratedCallDemand::Ready(again) = c.next_call().unwrap() else {
        panic!("pending demand lost")
    };
    assert_eq!(again.key(), r.key());
    c.complete_call(record).unwrap();
}
#[test]
fn generated_fragment_advances_ordered_calls_and_preserves_new_producer_frontier() {
    use lrr::frontend::generated_macros::GeneratedCallDemand;
    let (_, mut c) = fragment("()=>{macro_rules! leaf{()=>{}}leaf!();leaf!();};");
    let mut ids = Vec::new();
    for _ in 0..2 {
        let GeneratedCallDemand::Ready(r) = c.next_call().unwrap() else {
            panic!("ordered demand absent")
        };
        ids.push(r.key().invocation);
        c.complete_call(Arc::new(r.expand().unwrap())).unwrap();
    }
    assert_ne!(ids[0], ids[1]);
    assert!(matches!(
        c.next_call().unwrap(),
        GeneratedCallDemand::Complete
    ));
    let (_, mut c) = fragment("()=>{macro_rules! leaf{()=>{unknown!();}}leaf!();};");
    let GeneratedCallDemand::Ready(r) = c.next_call().unwrap() else {
        panic!("demand absent")
    };
    assert_eq!(
        c.complete_call(Arc::new(r.expand().unwrap()))
            .unwrap_err()
            .code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    assert!(matches!(
        c.next_call().unwrap(),
        GeneratedCallDemand::Ready(_)
    ));
}
