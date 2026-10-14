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
fn catalog(
    rules: &str,
) -> (
    GeneratedItems,
    lrr::frontend::generated_macros::GeneratedMacroCatalog,
) {
    let g = GeneratedItems::admit(&key(), output(rules), 1).unwrap();
    let c = lrr::frontend::generated_macros::GeneratedMacroCatalog::new(
        &g,
        scope(),
        owner_context(),
        &lrr::source::snapshot::CfgSet::guest_default(),
        1,
    )
    .unwrap();
    (g, c)
}
fn selected(
    c: &lrr::frontend::generated_macros::GeneratedMacroCatalog,
    name: &str,
    before: u64,
) -> lrr::expand::scopes::MacroDefinition {
    let lrr::frontend::macro_scopes::HygienicLookup::Selection(selection) =
        c.lookup(name, before, owner_context()).unwrap()
    else {
        panic!("generated macro pending")
    };
    selection.require_definition().unwrap()
}
fn expand(
    c: &lrr::frontend::generated_macros::GeneratedMacroCatalog,
    d: &lrr::expand::scopes::MacroDefinition,
) -> GeneratedItems {
    let input = fixture::input("");
    let mut next = key();
    next.macro_identity = MacroIdentity::Declaration {
        definition: d.identity,
    };
    next.invocation = Digest([8; 32]);
    next.input_tokens = Digest::of_bytes(&input.encode().unwrap());
    next.definition_edition = d.definition_edition;
    let record = c
        .expand(
            d.identity,
            &next,
            &lrr::syntax::token_wire::TokenForest::decode(&input.encode().unwrap()).unwrap(),
            lrr::syntax::token_wire::Key(fixture::call_site(&input).0),
        )
        .unwrap();
    GeneratedItems::admit(&next, Arc::new(record), 1).unwrap()
}
#[test]
fn actual_generated_macro_resolves_and_expands_to_later_generated_function() {
    let (items, c) = catalog("()=>{macro_rules! next{()=>{pub fn made(){7_i64;}}}};");
    let d = selected(&c, "next", u64::MAX);
    assert_eq!(d.identity, items.items()[0].key.fingerprint());
    assert_eq!(d.defining_unit, scope().crate_key);
    let g = expand(&c, &d);
    assert_eq!(g.items()[0].name.as_ref().unwrap().text, "made");
    assert_eq!(
        fixture::literal_texts(
            &lrr_expand_adapter::token_wire::TokenForest::decode(&g.tokens().encode().unwrap())
                .unwrap()
        ),
        vec!["7_i64".to_string()]
    );
}
#[test]
fn generated_macro_textual_shadowing_uses_existing_source_order_resolver() {
    let (items, c) = catalog(
        "()=>{macro_rules! next{()=>{pub fn first(){}}}macro_rules! next{()=>{pub fn second(){}}}};",
    );
    let first = selected(&c, "next", u64::from(items.items()[1].start));
    let last = selected(&c, "next", u64::MAX);
    assert_ne!(first.identity, last.identity);
    assert_eq!(
        expand(&c, &first).items()[0].name.as_ref().unwrap().text,
        "first"
    );
    assert_eq!(
        expand(&c, &last).items()[0].name.as_ref().unwrap().text,
        "second"
    );
}
#[test]
fn generated_macro_cfg_is_applied_and_malformed_rules_remain_lazy() {
    let (_, c) = catalog(
        "()=>{#[cfg(any())]macro_rules! hidden{()=>{}}macro_rules! broken{bad}macro_rules! good{()=>{pub fn made(){}}}};",
    );
    assert!(matches!(
        c.lookup("hidden", u64::MAX, owner_context()).unwrap(),
        lrr::frontend::macro_scopes::HygienicLookup::Selection(
            lrr::frontend::macro_origins::MacroSelection::Absent
        )
    ));
    assert_eq!(
        expand(&c, &selected(&c, "good", u64::MAX)).items()[0]
            .name
            .as_ref()
            .unwrap()
            .text,
        "made"
    );
    let d = selected(&c, "broken", u64::MAX);
    let input = fixture::input("");
    let mut k = key();
    k.macro_identity = MacroIdentity::Declaration {
        definition: d.identity,
    };
    k.definition_edition = d.definition_edition;
    k.input_tokens = Digest::of_bytes(&input.encode().unwrap());
    let error = c
        .expand(
            d.identity,
            &k,
            &lrr::syntax::token_wire::TokenForest::decode(&input.encode().unwrap()).unwrap(),
            lrr::syntax::token_wire::Key(fixture::call_site(&input).0),
        )
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-MACRO-EXPANSION");
}

#[test]
fn generated_macro_demand_rejects_substituted_identity_input_and_edition() {
    let (_, c) = catalog("()=>{macro_rules! next{()=>{pub fn made(){}}}};");
    let d = selected(&c, "next", u64::MAX);
    let input = fixture::input("");
    let input_wire = input.encode().unwrap();
    let input = lrr::syntax::token_wire::TokenForest::decode(&input_wire).unwrap();
    let site = input.spans()[0].key();
    let mut k = key();
    k.macro_identity = MacroIdentity::Declaration {
        definition: d.identity,
    };
    k.definition_edition = d.definition_edition;
    k.input_tokens = Digest::of_bytes(&input_wire);
    for changed in [0, 1, 2, 3, 4] {
        let mut wrong = k.clone();
        match changed {
            0 => {
                wrong.macro_identity = MacroIdentity::Declaration {
                    definition: Digest([9; 32]),
                }
            }
            1 => wrong.input_tokens = Digest([9; 32]),
            2 => wrong.definition_edition = 2021,
            3 => wrong.parent_context = Digest([9; 32]),
            4 => wrong.call_edition = 2021,
            _ => unreachable!(),
        }
        assert_eq!(
            c.expand(d.identity, &wrong, &input, site).unwrap_err().code,
            "LRR-E-EXPANSION-INPUT"
        );
    }
}

#[test]
fn generated_macro_catalog_rejects_wrong_owner_and_stale_epoch() {
    use lrr::frontend::generated_macros::GeneratedMacroCatalog;
    let g = GeneratedItems::admit(&key(), output("()=>{macro_rules! next{()=>{}}};"), 1).unwrap();
    let cfg = lrr::source::snapshot::CfgSet::guest_default();
    assert!(
        matches!(GeneratedMacroCatalog::new(&g, scope(), owner_context(), &cfg, 2), Err(e) if e.code == "LRR-E-EXPANSION-EPOCH")
    );
    let mut wrong = scope();
    wrong.crate_key = Digest([9; 32]);
    assert!(
        matches!(GeneratedMacroCatalog::new(&g, wrong, owner_context(), &cfg, 1), Err(e) if e.code == "LRR-E-EXPANSION-INPUT")
    );
    assert!(
        matches!(GeneratedMacroCatalog::new(&g, scope(), lrr::syntax::token_wire::Key([9;32]), &cfg, 1), Err(e) if e.code == "LRR-E-EXPANSION-INPUT")
    );
}

#[test]
fn generated_macro_catalog_keeps_unresolved_transform_and_item_producers_incomplete() {
    use lrr::frontend::generated_macros::GeneratedMacroCatalog;
    for rules in [
        "()=>{#[opaque]macro_rules! next{()=>{}}};",
        "()=>{opaque!();macro_rules! next{()=>{}}};",
        "()=>{#[derive(Custom)]struct S;macro_rules! next{()=>{}}};",
        "()=>{mod external;macro_rules! next{()=>{}}};",
    ] {
        let g = GeneratedItems::admit(&key(), output(rules), 1).unwrap();
        assert!(
            matches!(GeneratedMacroCatalog::new(&g, scope(), owner_context(), &lrr::source::snapshot::CfgSet::guest_default(), 1), Err(e) if e.code == "LRR-E-CATALOG-INCOMPLETE"),
            "{rules}"
        );
    }
}

#[test]
fn generated_external_module_keeps_a_typed_frontier_and_parent_textual_cutoff() {
    use lrr::frontend::{
        generated_macros::{GeneratedCallDemand, GeneratedMacroCatalog},
        macro_origins::MacroSelection,
        macro_scopes::HygienicLookup,
    };
    let rules = "()=>{macro_rules! helper{()=>{pub fn before()->i64{1733_i64}}}mod child;macro_rules! helper{()=>{pub fn after()->i64{1741_i64}}}macro_rules! late{()=>{}}};";
    let items = GeneratedItems::admit(&key(), output(rules), 1).unwrap();
    let mut c = GeneratedMacroCatalog::for_discovery(
        &items,
        scope(),
        owner_context(),
        &lrr::source::snapshot::CfgSet::guest_default(),
        1,
    )
    .unwrap();
    let producer = c.unresolved_module_sources().next().unwrap().producer();
    let HygienicLookup::Selection(selection) = c
        .lookup_module_parent(producer, "helper", owner_context())
        .unwrap()
    else {
        panic!("parent selection pending");
    };
    let definition = selection.require_definition().unwrap();
    assert_eq!(definition.identity, items.items()[0].key.fingerprint());
    assert_eq!(
        expand(&c, &definition).items()[0]
            .name
            .as_ref()
            .unwrap()
            .text,
        "before"
    );
    assert_eq!(
        c.lookup_module_parent(producer, "late", owner_context())
            .unwrap(),
        HygienicLookup::Selection(MacroSelection::Absent)
    );
    assert!(matches!(c.next_call().unwrap(), GeneratedCallDemand::NeedInput(id) if id == producer));
    assert!(matches!(c.next_call().unwrap(), GeneratedCallDemand::NeedInput(id) if id == producer));
    assert!(
        c.lookup_module_parent(Digest([255; 32]), "helper", owner_context())
            .is_err()
    );
    let actual = fixture::literal_texts(
        &lrr_expand_adapter::token_wire::TokenForest::decode(
            &expand(&c, &definition).tokens().encode().unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(actual.len(), 1);
    let actual = actual[0].strip_suffix("_i64").unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/m013-module-parent-native-{}",
            std::process::id()
        ));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("main.rs"),
        format!(
            "macro_rules! emit{{{rules}}}emit!();fn main(){{println!(\"{{}}\",child::value());}}"
        ),
    )
    .unwrap();
    std::fs::write(
        path.join("child.rs"),
        "helper!();pub fn value()->i64{before()}",
    )
    .unwrap();
    std::fs::write(path.join("pinned-literal.txt"), actual).unwrap();
    std::fs::write(
        path.join("pinned-expanded.rs"),
        expand(&c, &definition).text(),
    )
    .unwrap();
    let built = std::process::Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name=oracle"])
        .arg(path.join("main.rs"))
        .arg("-o")
        .arg(path.join("oracle"))
        .output()
        .unwrap();
    std::fs::write(path.join("compile.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = std::process::Command::new(path.join("oracle"))
        .output()
        .unwrap();
    std::fs::write(path.join("native.stdout"), &native.stdout).unwrap();
    assert!(native.status.success());
    assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), actual);
}

#[test]
fn generated_module_parent_scopes_keep_nested_shadowing_cfg_and_macro_use_pending() {
    use lrr::frontend::{
        generated_macros::{GeneratedCallDemand, GeneratedMacroCatalog},
        macro_scopes::HygienicLookup,
    };
    let items = GeneratedItems::admit(&key(), output("()=>{macro_rules! helper{()=>{pub fn root(){}}}mod outer{macro_rules! helper{()=>{pub fn inner(){}}}mod child;macro_rules! helper{()=>{pub fn late(){}}}}};"), 1).unwrap();
    let mut c = GeneratedMacroCatalog::for_discovery(
        &items,
        scope(),
        owner_context(),
        &lrr::source::snapshot::CfgSet::guest_default(),
        1,
    )
    .unwrap();
    let demand = c.unresolved_module_sources().next().unwrap();
    assert_eq!(demand.scope().module_path, ["outer", "child"]);
    let producer = demand.producer();
    let HygienicLookup::Selection(selection) = c
        .lookup_module_parent(producer, "helper", owner_context())
        .unwrap()
    else {
        panic!("nested parent macro pending");
    };
    let definition = selection.require_definition().unwrap();
    assert_eq!(
        expand(&c, &definition).items()[0]
            .name
            .as_ref()
            .unwrap()
            .text,
        "inner"
    );
    assert!(matches!(c.next_call().unwrap(), GeneratedCallDemand::NeedInput(id) if id == producer));

    let disabled = GeneratedItems::admit(
        &key(),
        output("()=>{#[cfg(any())]mod hidden;macro_rules! helper{()=>{}}};"),
        1,
    )
    .unwrap();
    let mut disabled = GeneratedMacroCatalog::for_discovery(
        &disabled,
        scope(),
        owner_context(),
        &lrr::source::snapshot::CfgSet::guest_default(),
        1,
    )
    .unwrap();
    assert_eq!(disabled.unresolved_module_sources().count(), 0);
    assert!(matches!(
        disabled.next_call().unwrap(),
        GeneratedCallDemand::Complete
    ));

    let imported =
        GeneratedItems::admit(&key(), output("()=>{#[macro_use]mod child;};"), 1).unwrap();
    let mut imported = GeneratedMacroCatalog::for_discovery(
        &imported,
        scope(),
        owner_context(),
        &lrr::source::snapshot::CfgSet::guest_default(),
        1,
    )
    .unwrap();
    let producer = imported
        .unresolved_module_sources()
        .next()
        .unwrap()
        .producer();
    assert!(
        matches!(imported.lookup("unknown", u64::MAX, owner_context()).unwrap(), HygienicLookup::NeedInput(id) if id == producer)
    );
    assert!(
        matches!(imported.next_call().unwrap(), GeneratedCallDemand::NeedInput(id) if id == producer)
    );
}
