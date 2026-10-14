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
fn generated_hygienic_function_and_type_headers_register_canonical_item_identities() {
    use lrr::{frontend::names::NameLookup, source::manifest::Namespace};
    let (items, p) =
        plan("()=>{pub fn emitted(){7_i64;let _:Missing=missing();}pub type Number=Unknown;};");
    assert_eq!(p.blocked().count(), 0);
    assert_eq!(
        fixture::literal_texts(
            &lrr_expand_adapter::token_wire::TokenForest::decode(&items.tokens().encode().unwrap())
                .unwrap()
        ),
        vec!["7_i64".to_string()]
    );
    let mut g = graph();
    p.publish(&mut g).unwrap();
    for (ordinal, namespace, name) in [
        (0, Namespace::Value, "emitted"),
        (1, Namespace::Type, "Number"),
    ] {
        let NameLookup::Defined(d) = g
            .lookup_unqualified(namespace, name, &scope(), 2024, None)
            .unwrap()
        else {
            panic!("generated header absent")
        };
        assert_eq!(d.identity, items.items()[ordinal].key.fingerprint());
        assert_eq!(d.defining_unit, scope().crate_key);
    }
}
#[test]
fn generated_cfg_and_private_visibility_use_existing_outline_rules() {
    use lrr::{
        frontend::names::{NameLookup, NameNamespaces},
        source::manifest::Namespace,
    };
    let (_, p) = plan("()=>{#[cfg(any())]pub fn hidden(){}fn private(){}};");
    let mut g = graph();
    p.publish(&mut g).unwrap();
    assert_eq!(
        g.lookup_unqualified(Namespace::Value, "hidden", &scope(), 2024, None)
            .unwrap(),
        NameLookup::Absent
    );
    let caller = lrr::syntax::outline::ScopeKey {
        crate_key: Digest([9; 32]),
        module_path: vec![],
        owner: None,
    };
    let path = NameNamespaces::path(scope().crate_key, &["private"], 2024).unwrap();
    assert!(matches!(
        g.lookup(Namespace::Value, &path, &caller).unwrap(),
        NameLookup::Inaccessible(_)
    ));
}
#[test]
fn generated_import_or_macro_frontiers_prevent_partial_namespace_absence() {
    use lrr::{frontend::names::NameLookup, source::manifest::Namespace};
    for rules in [
        "()=>{pub use missing::chosen;pub fn known(){}};",
        "()=>{macro_rules! next{()=>{}}pub fn known(){}};",
    ] {
        let (_, p) = plan(rules);
        assert!(p.blocked().count() > 0);
        let mut g = graph();
        p.publish(&mut g).unwrap();
        assert!(matches!(
            g.lookup_unqualified(Namespace::Value, "absent", &scope(), 2024, None)
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
}
#[test]
fn generated_header_plan_rejects_wrong_owner_context_and_stale_epoch() {
    let g = GeneratedItems::admit(&key(), output("()=>{pub fn emitted(){}};"), 1).unwrap();
    let cfg = lrr::source::snapshot::CfgSet::guest_default();
    assert!(
        lrr::frontend::generated_headers::GeneratedHeaderPlan::prepare(
            &g,
            scope(),
            lrr::syntax::token_wire::Key([9; 32]),
            &cfg,
            1
        )
        .is_err()
    );
    assert!(
        lrr::frontend::generated_headers::GeneratedHeaderPlan::prepare(
            &g,
            scope(),
            owner_context(),
            &cfg,
            2
        )
        .is_err()
    );
}

#[test]
fn generated_inline_module_headers_preserve_member_visibility_and_structural_identity() {
    use lrr::{
        frontend::names::{NameLookup, NameNamespaces},
        source::manifest::Namespace,
    };
    let (items, plan) = plan(
        "()=>{pub mod exposed{pub fn open(){let _:Missing=missing();}fn closed(){}}mod hidden{pub fn open(){}}};",
    );
    assert_eq!(plan.blocked().count(), 0);
    let open = items
        .items()
        .iter()
        .filter(|h| h.name.as_ref().is_some_and(|name| name.text == "open"))
        .collect::<Vec<_>>();
    assert_eq!(open.len(), 2);
    assert_ne!(open[0].key.structural_path, open[1].key.structural_path);
    assert_ne!(open[0].key.fingerprint(), open[1].key.fingerprint());
    let mut graph = graph();
    plan.publish(&mut graph).unwrap();
    let caller = lrr::syntax::outline::ScopeKey {
        crate_key: Digest([9; 32]),
        module_path: vec![],
        owner: None,
    };
    let public = NameNamespaces::path(scope().crate_key, &["exposed", "open"], 2024).unwrap();
    let NameLookup::Defined(found) = graph.lookup(Namespace::Value, &public, &caller).unwrap()
    else {
        panic!("public generated member absent");
    };
    assert_eq!(found.identity, open[0].key.fingerprint());
    for names in [&["exposed", "closed"][..], &["hidden", "open"][..]] {
        let path = NameNamespaces::path(scope().crate_key, names, 2024).unwrap();
        assert!(matches!(
            graph.lookup(Namespace::Value, &path, &caller).unwrap(),
            NameLookup::Inaccessible(_)
        ));
    }
}

#[test]
fn generated_external_module_source_prevents_a_partial_namespace_seal() {
    let (_, plan) = plan("()=>{mod child;pub fn reached()->i64{1_i64}};");
    assert_eq!(
        plan.blocked().count(),
        1,
        "external module source was treated as an empty completed module"
    );
    let mut graph = graph();
    plan.publish(&mut graph).unwrap();
    assert!(matches!(
        graph
            .lookup_unqualified(
                lrr::source::manifest::Namespace::Value,
                "reached",
                &scope(),
                2024,
                None
            )
            .unwrap(),
        lrr::frontend::names::NameLookup::NeedInput(_)
    ));
}
#[test]
fn generated_cfg_disabled_external_module_does_not_block_scope_completion() {
    let (_, plan) = plan("()=>{#[cfg(any())]mod absent;pub fn reached()->i64{1_i64}};");
    assert_eq!(plan.blocked().count(), 0);
    let mut graph = graph();
    plan.publish(&mut graph).unwrap();
    assert!(matches!(
        graph
            .lookup_unqualified(
                lrr::source::manifest::Namespace::Value,
                "reached",
                &scope(),
                2024,
                None
            )
            .unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
}

#[test]
fn generated_module_source_demands_keep_logical_paths_and_owned_origins() {
    let (items, plan) =
        plan("()=>{pub mod outer{#[path=\"chosen.rs\"]pub mod child;}#[cfg(any())]mod absent;};");
    let [demand] = plan.module_sources() else {
        panic!("enabled module demand absent");
    };
    assert_eq!(demand.scope().module_path, vec!["outer", "child"]);
    assert_eq!(demand.request().directory, vec!["outer"]);
    assert_eq!(demand.request().module_path, vec!["outer", "child"]);
    assert_eq!(demand.request().path_override.as_deref(), Some("chosen.rs"));
    assert!(
        items
            .items()
            .iter()
            .any(|item| item.key == *demand.header())
    );
    assert!(
        items
            .tokens()
            .spans()
            .iter()
            .any(|span| span.key() == demand.site())
    );
    assert!(plan.blocked().any(|id| *id == demand.producer()));
}
#[test]
fn generated_module_source_demand_keeps_structural_paths_and_invalidates_changed_origins() {
    let (_, first) = plan("()=>{mod child;fn cold(){1_i64}};");
    let (_, changed) = plan("()=>{mod child;fn cold(){missing();missing();}};");
    assert_eq!(
        first.module_sources()[0].header().structural_path,
        changed.module_sources()[0].header().structural_path
    );
    assert_ne!(
        first.module_sources()[0].producer(),
        changed.module_sources()[0].producer()
    );
    let (_, renamed) = plan("()=>{mod other;fn cold(){1_i64}};");
    assert_ne!(
        first.module_sources()[0].producer(),
        renamed.module_sources()[0].producer()
    );
}
