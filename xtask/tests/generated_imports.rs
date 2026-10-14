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
fn graph() -> lrr::frontend::names::NameNamespaces {
    let mut g = lrr::frontend::names::NameNamespaces::default();
    g.declare_module(
        scope().crate_key,
        &[],
        2024,
        lrr::syntax::visibility::VisibilityScope::Public,
    )
    .unwrap();
    g
}
fn plan(
    rules: &str,
) -> (
    GeneratedItems,
    lrr::frontend::generated_headers::GeneratedHeaderPlan,
) {
    let g = GeneratedItems::admit(&key(), output(rules), 1).unwrap();
    let p = lrr::frontend::generated_headers::GeneratedHeaderPlan::prepare(
        &g,
        scope(),
        owner_context(),
        &lrr::source::snapshot::CfgSet::guest_default(),
        1,
    )
    .unwrap();
    (g, p)
}

#[test]
fn generated_named_imports_resolve_generated_headers_and_preserve_canonical_identity() {
    use lrr::{frontend::names::NameLookup, source::manifest::Namespace};
    let (items, p) = plan(
        "()=>{pub use emitted as selected;pub use selected as again;pub fn emitted(){7_i64;}};",
    );
    assert_eq!(
        fixture::literal_texts(
            &lrr_expand_adapter::token_wire::TokenForest::decode(&items.tokens().encode().unwrap())
                .unwrap()
        ),
        vec!["7_i64".to_string()]
    );
    let mut g = graph();
    p.publish(&mut g).unwrap();
    let r = g.resolve_imports().unwrap();
    assert_eq!(r.resolved.len(), 2);
    assert!(r.pending.is_empty() && r.missing.is_empty() && r.ambiguous.is_empty());
    let wanted = items.items()[2].key.fingerprint();
    for name in ["selected", "again"] {
        let NameLookup::Defined(d) = g
            .lookup_unqualified(Namespace::Value, name, &scope(), 2024, None)
            .unwrap()
        else {
            panic!("generated import missing")
        };
        assert_eq!(d.identity, wanted);
    }
}
#[test]
fn generated_import_can_resolve_an_existing_source_definition_without_identity_rewrite() {
    use lrr::{
        core::profile::ProfileIdentity,
        frontend::names::{NameLookup, NameNamespaces},
        source::{
            manifest::Namespace,
            snapshot::{CfgSet, SourceBlob},
        },
        syntax::parse::ParsedFile,
    };
    let (_, p) = plan("()=>{pub use existing as selected;};");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(b"pub fn existing(){}".to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(scope().crate_key, vec![], &CfgSet::guest_default())
        .unwrap();
    let wanted = outline.items[0].def_key.fingerprint();
    let mut g = NameNamespaces::from_outlines(&[&outline]).unwrap();
    p.publish(&mut g).unwrap();
    assert_eq!(g.resolve_imports().unwrap().resolved.len(), 1);
    let NameLookup::Defined(d) = g
        .lookup_unqualified(Namespace::Value, "selected", &scope(), 2024, None)
        .unwrap()
    else {
        panic!("source target lost")
    };
    assert_eq!(d.identity, wanted);
}
#[test]
fn generated_missing_import_keeps_typed_incomplete_frontier_after_resolution() {
    use lrr::{frontend::names::NameLookup, source::manifest::Namespace};
    let (_, p) = plan("()=>{pub use unavailable as selected;};");
    let mut g = graph();
    p.publish(&mut g).unwrap();
    let r = g.resolve_imports().unwrap();
    assert!(!r.missing.is_empty() || !r.pending.is_empty());
    assert!(matches!(
        g.lookup_unqualified(Namespace::Value, "selected", &scope(), 2024, None)
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
}

#[test]
fn generated_glob_import_resolves_captured_source_exports_using_existing_fixed_point() {
    use lrr::{
        core::profile::ProfileIdentity,
        frontend::names::{NameLookup, NameNamespaces},
        source::{
            manifest::Namespace,
            snapshot::{CfgSet, SourceBlob},
        },
        syntax::parse::ParsedFile,
    };
    let (_, p) = plan("()=>{pub use source::*;};");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(b"pub mod source{pub fn chosen(){}}".to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(scope().crate_key, vec![], &CfgSet::guest_default())
        .unwrap();
    let wanted = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "chosen")
        .unwrap()
        .def_key
        .fingerprint();
    let mut g = NameNamespaces::from_outlines(&[&outline]).unwrap();
    p.publish(&mut g).unwrap();
    assert_eq!(g.resolve_imports().unwrap().resolved.len(), 1);
    let NameLookup::Defined(d) = g
        .lookup_unqualified(Namespace::Value, "chosen", &scope(), 2024, None)
        .unwrap()
    else {
        panic!("generated glob lost source export")
    };
    assert_eq!(d.identity, wanted);
}
#[test]
fn generated_dollar_crate_import_keeps_definition_unit_routing_pending() {
    use lrr::{frontend::names::NameLookup, source::manifest::Namespace};
    let (_, p) = plan("()=>{pub use $crate::chosen as selected;};");
    assert!(p.blocked().count() > 0);
    let mut g = graph();
    p.publish(&mut g).unwrap();
    assert!(matches!(
        g.lookup_unqualified(Namespace::Value, "selected", &scope(), 2024, None)
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
}
