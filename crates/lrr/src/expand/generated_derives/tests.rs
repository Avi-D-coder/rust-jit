//! Controlled pure catalogs supplement the actual authorized native-chain regression.
use super::*;
use crate::expand::declarative_test_fixture as fixture;
use crate::{
    expand::{
        identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        scopes::MacroName,
    },
    frontend::{
        builtin_attributes::BuiltinAttributeCatalog,
        generated_derives::GeneratedDeriveResolution,
        macro_names::{MacroNamespaces, MacroPath, PathLookup},
        procedural_exports::{PreparedProcBinding, PreparedProcCatalog},
    },
    syntax::outline::ScopeKey,
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::Edition,
};
use std::sync::Arc;
fn scope() -> ScopeKey {
    ScopeKey {
        crate_key: Digest([2; 32]),
        module_path: vec![],
        owner: None,
    }
}
fn generated(source: &str) -> (GeneratedItems, usize) {
    generated_edition(source, Edition::E2015)
}
fn generated_edition(source: &str, edition: Edition) -> (GeneratedItems, usize) {
    let definition_edition = match edition {
        Edition::E2015 => 2015,
        Edition::E2018 => 2018,
        Edition::E2021 => 2021,
        Edition::E2024 => 2024,
    };
    // An untouched upstream source can be embedded in an edition-2015 macro
    // template. This pure component is not an upstream compiletest execution.
    let definition =
        DeclarativeMacro::new(fixture::definition(&format!("()=>{{{source}}};"), edition)).unwrap();
    let input = fixture::input("");
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: Digest([3; 32]),
        input_tokens: Digest::of_bytes(&input.encode().unwrap()),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest([5; 32]),
        definition_edition,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    };
    let (out, _) = definition
        .expand_hygienic_with_limits(
            &input,
            fixture::call_site(&input),
            DeclarativeSite {
                expansion: lrr_expand_adapter::token_wire::Key(key.fingerprint().0),
                defining_unit: lrr_expand_adapter::token_wire::Key([1; 32]),
                definition_edition: edition,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    let record = Arc::new(
        crate::expand::record::ExpansionRecord::from_wire(
            key.clone(),
            out.unwrap().tokens.encode().unwrap(),
        )
        .unwrap(),
    );
    let items = GeneratedItems::admit(&key, record, 7).unwrap();
    let index = items
        .items()
        .iter()
        .position(|h| {
            matches!(
                h.kind,
                crate::expand::generated_items::GeneratedHeaderKind::Struct
            )
        })
        .unwrap();
    (items, index)
}
fn bare_catalog(names: &[&str], kind: ProceduralKind) -> PreparedProcCatalog {
    PreparedProcCatalog::from_exports(
        names
            .iter()
            .map(|name| (Digest([12; 32]), Digest([13; 32]), 2015, *name, kind)),
        PreparedProcBinding {
            consumer: scope().crate_key,
            graph: Digest([14; 32]),
            epoch: 7,
            environment: Digest([4; 32]),
            preparation: Digest([15; 32]),
            aliases: BTreeMap::from([(
                MacroName::parse("multiple_derives", 2015).unwrap(),
                Digest([13; 32]),
            )]),
        },
    )
    .unwrap()
}
// Pure fixture facts model the exact producer unit/edition used above. This
// performs no preparation, native load or macro invocation; native-chain tests
// separately exercise the authorized service's source capture.
fn catalog(names: &[&str], kind: ProceduralKind) -> PreparedProcCatalog {
    use crate::frontend::procedural_exports::PreparedDefinitionSite;
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Key, SourceIdentity, TokenTree, Transparency,
    };
    let artifact = Digest([12; 32]);
    let producer = Digest([13; 32]);
    let blob =
        crate::source::snapshot::SourceBlob::new(b"pub fn controlled_producer() {}".to_vec())
            .unwrap();
    let text = blob.text().unwrap();
    let parsed = token_wire::parse_source(
        text,
        SourceIdentity {
            unit: Key(producer.0),
            content: Key(blob.content().0),
            path: "lib.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key(producer.0),
            edition: Edition::E2015,
        },
        Key(artifact.0),
    )
    .unwrap();
    let [TokenTree::Group { open, .. }] = parsed.roots() else {
        panic!("fixture producer root absent");
    };
    let span = parsed
        .spans()
        .iter()
        .find(|s| s.key() == *open)
        .unwrap()
        .clone();
    let registry =
        token_wire::TokenForest::new(vec![], parsed.contexts().to_vec(), vec![span]).unwrap();
    let encoded = registry.encode().unwrap();
    let mut proof =
        crate::core::digest::StableEncoder::new("lrr/v2/prepared-procedural-definition-site");
    proof.digest(artifact);
    proof.bytes(&open.0);
    proof.bytes(&encoded);
    proof.u64(0); // The synthetic derive-only site has no attribute origin entries.
    let registry = crate::syntax::token_wire::TokenForest::decode(&encoded).unwrap();
    let site = PreparedDefinitionSite {
        site: crate::syntax::token_wire::Key(open.0),
        source: registry.spans()[0].source.clone(),
        bytes: registry.allocated_bytes().unwrap()
            + text.len()
            + std::mem::size_of::<PreparedDefinitionSite>(),
        registry,
        text: Arc::from(text),
        fingerprint: proof.finish(),
        diagnostic_definitions: BTreeMap::new(),
    };
    let mut catalog = bare_catalog(names, kind);
    catalog
        .bind_definition_sites(BTreeMap::from([(artifact, Arc::new(site))]))
        .unwrap();
    catalog
}
fn namespace(catalog: &PreparedProcCatalog) -> (MacroNamespaces, BuiltinAttributeCatalog) {
    let mut names = MacroNamespaces::default();
    for export in catalog.exports() {
        names
            .define(
                MacroPath::new(
                    export.definition().defining_unit,
                    vec![export.name().clone()],
                )
                .unwrap(),
                export.definition().clone(),
            )
            .unwrap();
    }
    names
        .dependency(
            scope().crate_key,
            MacroName::parse("multiple_derives", 2015).unwrap(),
            Digest([13; 32]),
        )
        .unwrap();
    let builtin = BuiltinAttributeCatalog::new(
        Digest([42; 32]),
        2024,
        [crate::expand::builtin_attributes::descriptor("derive")
            .unwrap()
            .unwrap()],
    )
    .unwrap();
    names
        .define(
            MacroPath::new(
                scope().crate_key,
                vec![MacroName::parse("derive", 2015).unwrap()],
            )
            .unwrap(),
            builtin.definitions().next().unwrap().clone(),
        )
        .unwrap();
    (names, builtin)
}
fn alias(names: &mut MacroNamespaces, catalog: &PreparedProcCatalog, name: &str, original: &str) {
    let definition = catalog
        .exports()
        .find(|e| e.name().as_str() == original)
        .unwrap()
        .definition()
        .clone();
    names
        .define(
            MacroPath::new(
                scope().crate_key,
                vec![MacroName::parse(name, 2015).unwrap()],
            )
            .unwrap(),
            definition,
        )
        .unwrap();
}
#[test]
fn rustc_derive_order_selects_five_ordered_controlled_exports_without_executing() {
    let (g, index) = generated(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
    )));
    let catalog = catalog(
        &["First", "Second", "Third", "Fourth", "Fifth"],
        ProceduralKind::Derive,
    );
    let (mut names, builtin) = namespace(&catalog);
    for name in ["First", "Second", "Third", "Fourth", "Fifth"] {
        alias(&mut names, &catalog, name, name);
    }
    let annotations = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        scope(),
        &names,
        &builtin,
    )
    .unwrap();
    let exports = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &names,
            &builtin,
            &catalog,
        )
        .unwrap();
    assert_eq!(
        exports
            .rows()
            .iter()
            .map(|r| match r.selected().unwrap().identity() {
                MacroIdentity::Procedural {
                    export,
                    kind: ProceduralKind::Derive,
                    ..
                } => export.as_str(),
                _ => panic!("nonderive"),
            })
            .collect::<Vec<_>>(),
        ["First", "Second", "Third", "Fourth", "Fifth"]
    );
    assert!(exports.rows().iter().all(|r| r.helper_demand().is_some()));
    assert_eq!(
        exports.first_demand().unwrap(),
        exports.rows()[0].helper_demand()
    );
    exports
        .validate_replay(
            &annotations
                .exports(
                    &g,
                    index,
                    &CfgSet::guest_default(),
                    &names,
                    &builtin,
                    &catalog,
                )
                .unwrap(),
        )
        .unwrap();
}
#[test]
fn generated_derive_export_aliases_and_repetition_keep_distinct_occurrences() {
    let (g, index) = generated("#[derive(chosen, chosen)] struct Marker;");
    let catalog = catalog(&["First"], ProceduralKind::Derive);
    let (mut names, builtin) = namespace(&catalog);
    alias(&mut names, &catalog, "chosen", "First");
    let annotations = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        scope(),
        &names,
        &builtin,
    )
    .unwrap();
    let exports = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &names,
            &builtin,
            &catalog,
        )
        .unwrap();
    assert_eq!(
        exports.rows()[0].selected().unwrap().definition(),
        exports.rows()[1].selected().unwrap().definition()
    );
    assert_ne!(
        exports.rows()[0].occurrence(),
        exports.rows()[1].occurrence()
    );
    assert_ne!(
        exports.rows()[0].helper_demand(),
        exports.rows()[1].helper_demand()
    );
}
#[test]
fn generated_derive_export_replay_rejects_missing_ambiguous_and_changed_names() {
    let (g, index) = generated("#[derive(chosen)] struct Marker;");
    let catalog = catalog(&["First"], ProceduralKind::Derive);
    let (mut names, builtin) = namespace(&catalog);
    alias(&mut names, &catalog, "chosen", "First");
    let annotations = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        scope(),
        &names,
        &builtin,
    )
    .unwrap();
    let selected = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &names,
            &builtin,
            &catalog,
        )
        .unwrap();
    let (absent, _) = namespace(&catalog);
    let missing = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &absent,
            &builtin,
            &catalog,
        )
        .unwrap();
    assert!(matches!(missing.rows()[0].candidate(), PathLookup::Absent));
    assert_eq!(
        selected.validate_replay(&missing).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let user = crate::expand::scopes::MacroDefinition {
        identity: Digest([50; 32]),
        defining_unit: scope().crate_key,
        definition_edition: 2015,
    };
    let path = MacroPath::new(
        scope().crate_key,
        vec![MacroName::parse("chosen", 2015).unwrap()],
    )
    .unwrap();
    let mut changed = absent;
    changed.define(path.clone(), user.clone()).unwrap();
    let new = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &changed,
            &builtin,
            &catalog,
        )
        .unwrap();
    assert!(new.rows()[0].selected().is_none());
    assert_eq!(
        selected.validate_replay(&new).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    names.define(path, user).unwrap();
    let ambiguous = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &names,
            &builtin,
            &catalog,
        )
        .unwrap();
    assert_eq!(
        ambiguous.first_demand().unwrap_err().code,
        "LRR-E-MACRO-AMBIGUOUS"
    );
    assert_eq!(
        selected.validate_replay(&ambiguous).unwrap_err().code,
        "LRR-E-MACRO-AMBIGUOUS"
    );
}
#[test]
fn generated_derive_export_selection_refuses_wrong_kind_and_catalog_context() {
    let (g, index) = generated("#[derive(chosen)] struct Marker;");
    let catalog = catalog(&["First"], ProceduralKind::Function);
    let (mut names, builtin) = namespace(&catalog);
    alias(&mut names, &catalog, "chosen", "First");
    let annotations = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        scope(),
        &names,
        &builtin,
    )
    .unwrap();
    assert!(
        matches!(annotations.exports(&g,index,&CfgSet::guest_default(),&names,&builtin,&catalog),Err(e) if e.code=="LRR-E-MACRO-SCOPE")
    );
    let mut other_scope = scope();
    other_scope.crate_key = Digest([55; 32]);
    let other = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        other_scope,
        &names,
        &builtin,
    )
    .unwrap();
    assert!(
        matches!(other.exports(&g,index,&CfgSet::guest_default(),&names,&builtin,&catalog),Err(e) if e.code=="LRR-E-EXPANSION-INPUT")
    );
}

#[test]
fn generated_derive_exports_resolve_raw_absolute_crate_self_and_dollar_crate_paths() {
    let (g, index) = generated_edition(
        "#[derive(::multiple_derives::r#gen, crate::chosen, self::chosen, $crate::chosen)] struct Marker;",
        Edition::E2024,
    );
    let catalog = catalog(&["gen"], ProceduralKind::Derive);
    let (mut names, builtin) = namespace(&catalog);
    alias(&mut names, &catalog, "chosen", "gen");
    let definition = catalog.exports().next().unwrap().definition().clone();
    names
        .define(
            MacroPath::new(
                Digest([1; 32]),
                vec![MacroName::parse("chosen", 2015).unwrap()],
            )
            .unwrap(),
            definition.clone(),
        )
        .unwrap();
    let annotations = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        scope(),
        &names,
        &builtin,
    )
    .unwrap();
    let exports = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &names,
            &builtin,
            &catalog,
        )
        .unwrap();
    assert_eq!(exports.rows().len(), 4);
    for row in exports.rows() {
        assert_eq!(row.selected().unwrap().definition(), &definition);
    }
}

#[test]
fn generated_derive_declaration_requests_bind_catalog_origin_and_occurrence() {
    let (g, index) = generated("#[derive(chosen, chosen)] struct Marker;");
    let prepared = catalog(&["First"], ProceduralKind::Derive);
    let (mut names, builtin) = namespace(&prepared);
    alias(&mut names, &prepared, "chosen", "First");
    let annotations = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        scope(),
        &names,
        &builtin,
    )
    .unwrap();
    let exports = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &names,
            &builtin,
            &prepared,
        )
        .unwrap();
    let first = exports.declaration_request(0, &prepared).unwrap();
    let second = exports.declaration_request(1, &prepared).unwrap();
    assert_eq!(first.demand(), exports.rows()[0].helper_demand().unwrap());
    assert_eq!(first.occurrence(), exports.rows()[0].occurrence());
    assert_ne!(first.fingerprint(), second.fingerprint());
    let crate::expand::derive_origin::DeriveDeclarationOrigin::Generated(parent) = &first.origin
    else {
        panic!("generated request lost its expansion parent");
    };
    assert_eq!(parent.fingerprint(), g.record().descriptor().fingerprint());
    assert_ne!(first.origin.fingerprint(), parent.fingerprint());
    assert!(
        matches!(exports.declaration_request(2,&prepared),Err(e) if e.code=="LRR-E-EXPANSION-INPUT")
    );
    let changed = catalog(&["First", "Second"], ProceduralKind::Derive);
    assert!(
        matches!(exports.declaration_request(0,&changed),Err(e) if e.code=="LRR-E-EXPANSION-INPUT")
    );
    let (absent, _) = namespace(&prepared);
    let missing = annotations
        .exports(
            &g,
            index,
            &CfgSet::guest_default(),
            &absent,
            &builtin,
            &prepared,
        )
        .unwrap();
    assert!(
        matches!(missing.declaration_request(0,&prepared),Err(e) if e.code=="LRR-E-CATALOG-INCOMPLETE")
    );
}

fn helper_receipts(
    exports: &crate::frontend::generated_derive_exports::GeneratedDeriveExports,
    catalog: &PreparedProcCatalog,
    names: &[&str],
) -> BTreeMap<Digest, Arc<crate::expand::derive_receipt::PreparedDeriveDeclaration>> {
    exports
        .rows()
        .iter()
        .enumerate()
        .map(|(at, row)| {
            let request = exports.declaration_request(at, catalog).unwrap();
            (
                row.helper_demand().unwrap(),
                Arc::new(
                    crate::expand::derive_receipt::PreparedDeriveDeclaration::new(
                        &request,
                        Digest([90; 32]),
                        names.iter().map(|n| n.to_string()).collect(),
                    ),
                ),
            )
        })
        .collect()
}
fn helper_case(
    source: &str,
    derives: &[&str],
    helpers: &[&str],
) -> (
    GeneratedItems,
    usize,
    Arc<crate::frontend::generated_derive_helpers::GeneratedDeriveHelpers>,
) {
    let catalog = catalog(derives, ProceduralKind::Derive);
    helper_case_with_catalog(source, derives, helpers, &catalog)
}
fn helper_case_with_catalog(
    source: &str,
    derives: &[&str],
    helpers: &[&str],
    catalog: &PreparedProcCatalog,
) -> (
    GeneratedItems,
    usize,
    Arc<crate::frontend::generated_derive_helpers::GeneratedDeriveHelpers>,
) {
    let (items, index) = generated(source);
    let cfg = CfgSet::guest_default();
    let (mut names, builtin) = namespace(catalog);
    for name in derives {
        alias(&mut names, catalog, name, name);
    }
    let resolution =
        GeneratedDeriveResolution::resolve(&items, index, &cfg, scope(), &names, &builtin).unwrap();
    let exports = resolution
        .exports(&items, index, &cfg, &names, &builtin, catalog)
        .unwrap();
    let receipts = helper_receipts(&exports, catalog, helpers);
    let crate::frontend::generated_derive_helpers::GeneratedHelperPoll::Ready(result) = resolution
        .helpers(
            &items,
            index,
            &cfg,
            &names,
            &builtin,
            &crate::frontend::generated_derive_helpers::GeneratedHelperDeclarations {
                catalog,
                receipts: &receipts,
            },
        )
        .unwrap()
    else {
        panic!("exact declarations did not resolve helpers")
    };
    (items, index, result)
}
#[test]
fn rustc_helper_shadowing_keeps_field_helper_local_and_sibling_attribute_unresolved() {
    let source = include_str!(
        "../../../../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-helper-shadowing.rs"
    );
    let (items, _, result) = helper_case(source, &["Empty"], &["empty_helper"]);
    assert_eq!(result.legacy_outer(), [0]);
    assert_eq!(result.unresolved_outer(), [0]);
    assert_eq!(result.member_helpers().len(), 1);
    let cfg = CfgSet::guest_default();
    let catalog = catalog(&["Empty"], ProceduralKind::Derive);
    let (mut names, builtin) = namespace(&catalog);
    alias(&mut names, &catalog, "Empty", "Empty");
    let z = items
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "Z"))
        .unwrap();
    let resolution =
        GeneratedDeriveResolution::resolve(&items, z, &cfg, scope(), &names, &builtin).unwrap();
    let crate::frontend::generated_derive_helpers::GeneratedHelperPoll::Ready(sibling) = resolution
        .helpers(
            &items,
            z,
            &cfg,
            &names,
            &builtin,
            &crate::frontend::generated_derive_helpers::GeneratedHelperDeclarations {
                catalog: &catalog,
                receipts: &BTreeMap::new(),
            },
        )
        .unwrap()
    else {
        panic!("sibling facts pending");
    };
    assert_eq!(sibling.unresolved_outer(), [0]);
    assert_eq!(sibling.inert_count(), 0);
}

#[test]
fn rustc_configured_helpers_classify_exact_outer_and_field_scope_without_stripping_tokens() {
    let source = include_str!(
        "../../../../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-helper-configured.rs"
    );
    let (items, index, result) = helper_case(source, &["Empty"], &["empty_helper"]);
    assert_eq!(result.inert_count(), 2);
    assert_eq!(result.outer_helpers().len(), 1);
    assert_eq!(result.member_helpers().len(), 1);
    let cfg = CfgSet::guest_default();
    let catalog = catalog(&["Empty"], ProceduralKind::Derive);
    let (mut names, builtin) = namespace(&catalog);
    alias(&mut names, &catalog, "Empty", "Empty");
    let resolution =
        GeneratedDeriveResolution::resolve(&items, index, &cfg, scope(), &names, &builtin).unwrap();
    let plan = resolution.plan(&items, index, &cfg).unwrap();
    let before = plan
        .materialize_input(&items, index, &cfg)
        .unwrap()
        .encode()
        .unwrap();
    assert_eq!(
        result
            .materialize_input(&items, index, &cfg)
            .unwrap()
            .encode()
            .unwrap(),
        before
    );
    let mut changed = cfg.clone();
    changed.atoms.insert("different".into());
    assert!(result.materialize_input(&items, index, &changed).is_err());
}
#[test]
fn rustc_same_named_derive_helper_precedes_macro_candidate_on_its_field() {
    let source = include_str!(
        "../../../../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-helper-shadowing-2.rs"
    );
    let (_, _, result) = helper_case(source, &["same_name"], &["same_name"]);
    assert_eq!(result.inert_count(), 1);
    assert!(result.outer_helpers().is_empty());
    assert_eq!(result.member_helpers().len(), 1);
    assert!(result.unresolved_outer().is_empty());
    assert!(result.unresolved_members().is_empty());
}
#[test]
fn generated_helpers_refuse_missing_wrong_or_sibling_receipts_and_preserve_lexical_limits() {
    let source = "#[empty_helper] #[derive(Empty)] #[empty_helper] #[other::empty_helper] struct S { #[empty_helper] field:u8 } #[empty_helper] struct T;";
    let (items, index) = generated(source);
    let cfg = CfgSet::guest_default();
    let catalog = catalog(&["Empty"], ProceduralKind::Derive);
    let (mut names, builtin) = namespace(&catalog);
    alias(&mut names, &catalog, "Empty", "Empty");
    let resolution =
        GeneratedDeriveResolution::resolve(&items, index, &cfg, scope(), &names, &builtin).unwrap();
    let exports = resolution
        .exports(&items, index, &cfg, &names, &builtin, &catalog)
        .unwrap();
    assert!(
        matches!(resolution.helpers(&items,index,&cfg,&names,&builtin,&crate::frontend::generated_derive_helpers::GeneratedHelperDeclarations {catalog:&catalog,receipts:&BTreeMap::new()}).unwrap(),crate::frontend::generated_derive_helpers::GeneratedHelperPoll::NeedInput(d) if d==exports.rows()[0].helper_demand().unwrap())
    );
    let receipts = helper_receipts(&exports, &catalog, &["empty_helper"]);
    let crate::frontend::generated_derive_helpers::GeneratedHelperPoll::Ready(result) = resolution
        .helpers(
            &items,
            index,
            &cfg,
            &names,
            &builtin,
            &crate::frontend::generated_derive_helpers::GeneratedHelperDeclarations {
                catalog: &catalog,
                receipts: &receipts,
            },
        )
        .unwrap()
    else {
        panic!("helper resolution pending")
    };
    assert_eq!(result.inert_count(), 2);
    assert_eq!(result.legacy_outer(), &[0]);
    assert_eq!(result.unresolved_outer(), &[0, 3]);
    let sibling = items
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "T"))
        .unwrap();
    assert!(result.materialize_input(&items, sibling, &cfg).is_err());
    let sibling_resolution =
        GeneratedDeriveResolution::resolve(&items, sibling, &cfg, scope(), &names, &builtin)
            .unwrap();
    let crate::frontend::generated_derive_helpers::GeneratedHelperPoll::Ready(sibling_result) =
        sibling_resolution
            .helpers(
                &items,
                sibling,
                &cfg,
                &names,
                &builtin,
                &crate::frontend::generated_derive_helpers::GeneratedHelperDeclarations {
                    catalog: &catalog,
                    receipts: &receipts,
                },
            )
            .unwrap()
    else {
        panic!("sibling helper resolution pending")
    };
    assert_eq!(sibling_result.inert_count(), 0);
    assert_eq!(sibling_result.unresolved_outer(), &[0]);
    let (foreign, foreign_index) = generated("#[derive(Empty)] struct Different;");
    let foreign_resolution = GeneratedDeriveResolution::resolve(
        &foreign,
        foreign_index,
        &cfg,
        scope(),
        &names,
        &builtin,
    )
    .unwrap();
    let foreign_exports = foreign_resolution
        .exports(&foreign, foreign_index, &cfg, &names, &builtin, &catalog)
        .unwrap();
    let mut wrong = receipts;
    let demand = exports.rows()[0].helper_demand().unwrap();
    let other = helper_receipts(&foreign_exports, &catalog, &["empty_helper"])
        .into_values()
        .next()
        .unwrap();
    wrong.insert(demand, other);
    assert!(
        resolution
            .helpers(
                &items,
                index,
                &cfg,
                &names,
                &builtin,
                &crate::frontend::generated_derive_helpers::GeneratedHelperDeclarations {
                    catalog: &catalog,
                    receipts: &wrong
                }
            )
            .is_err()
    );
    let (_, _, repeated) = helper_case(
        "#[derive(Empty,Empty)] #[empty_helper] struct Repeated { #[cfg(any())] #[undeclared] cold:u8, #[empty_helper] live:u8 }",
        &["Empty"],
        &["empty_helper"],
    );
    assert_eq!(repeated.inert_count(), 2);
    assert!(repeated.unresolved_members().is_empty());
    let owners = repeated.outer_helpers().values().next().unwrap();
    assert_eq!(owners.len(), 2);
    assert_ne!(owners[0], owners[1]);
    assert_eq!(repeated.member_helpers().values().next().unwrap(), owners);
}

#[test]
fn generated_derive_requests_bind_ordered_occurrences_exact_helpers_and_item_tokens() {
    let source = r##"#[derive(Empty,Empty)] #[empty_helper("quoted", br#"raw"#)] struct S { #[empty_helper(b'x')] field:[u8; 1usize] } const C:u8=1u8;"##;
    let (items, index, helpers) = helper_case(source, &["Empty"], &["empty_helper"]);
    let cfg = CfgSet::guest_default();
    let catalog = catalog(&["Empty"], ProceduralKind::Derive);
    let definition = catalog.exports().next().unwrap().definition().clone();
    let build = |at| {
        crate::expand::generated_call_request::GeneratedCallRequest::prepare_derive(
            Arc::new(items.clone()),
            index,
            &cfg,
            at,
            definition.clone(),
            helpers.clone(),
        )
        .unwrap()
        .with_procedural(&catalog, &scope(), &[])
        .unwrap()
    };
    let missing = bare_catalog(&["Empty"], ProceduralKind::Derive);
    let (unbound_items, unbound_index, unbound_helpers) =
        helper_case_with_catalog(source, &["Empty"], &["empty_helper"], &missing);
    let unbound = crate::expand::generated_call_request::GeneratedCallRequest::prepare_derive(
        Arc::new(unbound_items),
        unbound_index,
        &cfg,
        0,
        definition.clone(),
        unbound_helpers,
    )
    .unwrap();
    let Err(fault) = unbound.with_procedural(&missing, &scope(), &[]) else {
        panic!("missing producer facts accepted");
    };
    assert_eq!(fault.code, "LRR-E-EXPANSION-INPUT");
    assert_eq!(
        fault.message,
        "generated procedural producer definition facts absent"
    );
    let first = build(0);
    let second = build(1);
    assert_ne!(first.key().invocation, second.key().invocation);
    assert_ne!(first.key().input_tokens, second.key().input_tokens);
    assert_eq!(
        first.procedural_request().unwrap().input.roots(),
        second.procedural_request().unwrap().input.roots()
    );
    let native = first.procedural_request().unwrap();
    let producer_site = native
        .input
        .spans()
        .iter()
        .find(|span| span.key() == native.definition_site)
        .unwrap();
    assert_eq!(producer_site.edition, Edition::E2015);
    assert_eq!(
        producer_site.source.unit,
        lrr_expand_adapter::token_wire::Key([13; 32])
    );
    assert!(
        native
            .input
            .spans()
            .iter()
            .any(|s| s.key() == native.call_site)
    );
    assert!(first.is_derive() && second.is_derive());
    assert_eq!(
        first.procedural_request().unwrap().form,
        lrr_expand_adapter::proc_client::ProcForm::Derive
    );
    assert!(first.procedural_request().unwrap().attributes.is_none());
    assert_eq!(first.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
    assert!(
        crate::expand::generated_call_request::GeneratedCallRequest::prepare_derive(
            Arc::new(items),
            index,
            &cfg,
            2,
            definition,
            helpers
        )
        .is_err()
    );
    let (blocked, index, helpers) =
        helper_case("#[undeclared] #[derive(Empty)] struct S;", &["Empty"], &[]);
    let definition = catalog.exports().next().unwrap().definition().clone();
    assert!(
        crate::expand::generated_call_request::GeneratedCallRequest::prepare_derive(
            Arc::new(blocked),
            index,
            &cfg,
            0,
            definition,
            helpers
        )
        .is_err()
    );
    let (queued, index, helpers) =
        helper_case("#[derive(Empty)] #[undeclared] struct S;", &["Empty"], &[]);
    // A later attribute remains an unresolved producer, not an inert helper.
    assert_eq!(helpers.unresolved_outer(), [1]);
    assert!(helpers.outer_helpers().is_empty());
    let request = crate::expand::generated_call_request::GeneratedCallRequest::prepare_derive(
        Arc::new(queued),
        index,
        &cfg,
        0,
        catalog.exports().next().unwrap().definition().clone(),
        helpers,
    )
    .unwrap();
    assert!(request.is_derive());
    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-PERMISSION");
}
