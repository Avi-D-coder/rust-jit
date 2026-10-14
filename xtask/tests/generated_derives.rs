use lrr::{
    core::digest::{Digest, StableEncode},
    expand::{
        builtin_attributes::descriptor,
        generated_derives::{GeneratedDeriveAnnotations, GeneratedDerivePlan},
        generated_items::GeneratedItems,
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
    },
    source::snapshot::CfgSet,
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::{Edition, Key},
};
use std::sync::Arc;
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;

fn items(rules: &str, epoch: u64) -> GeneratedItems {
    expanded(rules, fixture::input(""), epoch, 2024)
}
fn expanded(
    rules: &str,
    input: lrr_expand_adapter::token_wire::TokenForest,
    epoch: u64,
    call_edition: u16,
) -> GeneratedItems {
    expanded_forests(
        fixture::definition(rules, Edition::E2024),
        input,
        epoch,
        Edition::E2024,
        call_edition,
    )
}
fn expanded_forests(
    definition: lrr_expand_adapter::token_wire::TokenForest,
    input: lrr_expand_adapter::token_wire::TokenForest,
    epoch: u64,
    definition_edition: Edition,
    call_edition: u16,
) -> GeneratedItems {
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: Digest([2; 32]),
        input_tokens: Digest([3; 32]),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest([5; 32]),
        definition_edition: match definition_edition {
            Edition::E2015 => 2015,
            Edition::E2018 => 2018,
            Edition::E2021 => 2021,
            Edition::E2024 => 2024,
        },
        call_edition,
        grammar: GrammarSlot::Items,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    };
    let definition = DeclarativeMacro::new(definition).unwrap();
    let (output, _) = definition
        .expand_hygienic_with_limits(
            &input,
            fixture::call_site(&input),
            DeclarativeSite {
                expansion: Key(key.fingerprint().0),
                defining_unit: Key([1; 32]),
                definition_edition,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    let record = Arc::new(
        ExpansionRecord::from_wire(key.clone(), output.unwrap().tokens.encode().unwrap()).unwrap(),
    );
    GeneratedItems::admit(&key, record, epoch).unwrap()
}
fn upstream(source: &str, name: &str) -> GeneratedItems {
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Delimiter, SourceIdentity, TokenForest, TokenTree, Transparency,
    };
    // Keep the copied file unchanged. The owned identity-macro envelope is a test
    // adaptation with its own source identity; it is not the upstream source file.
    let envelope = format!("fixture!({source});");
    let forest = token_wire::parse_source(
        &envelope,
        SourceIdentity {
            unit: Key([2; 32]),
            content: Key(*blake3::hash(envelope.as_bytes()).as_bytes()),
            path: format!("upstream-envelope/{name}"),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([2; 32]),
            edition: Edition::E2015,
        },
        Key([3; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("envelope absent")
    };
    let group = children
        .iter()
        .find(|t| {
            matches!(
                t,
                TokenTree::Group {
                    delimiter: Delimiter::Parenthesis,
                    ..
                }
            )
        })
        .unwrap();
    let input = TokenForest::new(
        vec![group.clone()],
        forest.contexts().to_vec(),
        forest.spans().to_vec(),
    )
    .unwrap();
    expanded("($($tokens:tt)*)=>{$($tokens)*};", input, 19, 2015)
}
fn adt_plan(g: &GeneratedItems, name: &str, indexes: &[usize]) -> GeneratedDerivePlan {
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == name))
        .unwrap();
    let proof = GeneratedDeriveAnnotations::new(
        g,
        index,
        &CfgSet::guest_default(),
        indexes
            .iter()
            .map(|i| (*i, descriptor("derive").unwrap().unwrap())),
    )
    .unwrap();
    GeneratedDerivePlan::prepare(g, index, &CfgSet::guest_default(), &proof).unwrap()
}

#[test]
fn rustc_expand_to_derive_uses_original_mbe_and_filters_nested_cfg_with_source_origins() {
    use lrr_expand_adapter::token_wire::{
        ContextRecord, Delimiter, SourceIdentity, TokenForest, TokenTree, Transparency,
        parse_source,
    };
    let source = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/expand-to-derive.rs"
    );
    let path = "tests/ui/proc-macro/expand-to-derive.rs";
    let forest = parse_source(
        source,
        SourceIdentity {
            unit: Key([2; 32]),
            content: Key(*blake3::hash(source.as_bytes()).as_bytes()),
            path: path.into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([2; 32]),
            edition: Edition::E2015,
        },
        Key([3; 32]),
    )
    .unwrap();
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("source root absent")
    };
    let bodies: Vec<_> = children
        .windows(3)
        .filter_map(|tokens| match tokens {
            [
                TokenTree::Ident { text, .. },
                TokenTree::Punct { scalar: '!', .. },
                group @ TokenTree::Group {
                    delimiter: Delimiter::Brace,
                    ..
                },
            ] if text == "expand_to_derive" => Some(group.clone()),
            _ => None,
        })
        .collect();
    let definition = children
        .windows(4)
        .find_map(|tokens| match tokens {
            [
                TokenTree::Ident { text, .. },
                TokenTree::Punct { scalar: '!', .. },
                TokenTree::Ident { text: name, .. },
                group @ TokenTree::Group {
                    delimiter: Delimiter::Brace,
                    ..
                },
            ] if text == "macro_rules" && name == "expand_to_derive" => Some(group.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(bodies.len(), 1);
    let owned = |root| {
        TokenForest::new(
            vec![root],
            forest.contexts().to_vec(),
            forest.spans().to_vec(),
        )
        .unwrap()
    };
    // Expand the copied source's actual rule and invocation, without an identity envelope.
    let generated = expanded_forests(
        owned(definition),
        owned(bodies[0].clone()),
        19,
        Edition::E2015,
        2015,
    );
    let plan = adt_plan(&generated, "Foo", &[0]);
    assert_eq!(plan.annotations()[0].paths()[0].segments, ["Print"]);
    assert_eq!(plan.facts().members.len(), 2);
    assert!(!plan.facts().members[0].enabled);
    assert!(plan.facts().members[1].enabled);
    let index = generated
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "Foo"))
        .unwrap();
    let output = plan
        .materialize_input(&generated, index, &CfgSet::guest_default())
        .unwrap();
    let names = identifiers(&output);
    for retained in ["Foo", "field", "Inner", "other_inner_field", "rustc_dummy"] {
        assert!(
            names.iter().any(|name| name == retained),
            "missing {retained}"
        );
    }
    for removed in [
        "derive",
        "cfg",
        "cfg_attr",
        "removed",
        "removed_inner_field",
    ] {
        assert!(
            !names.iter().any(|name| name == removed),
            "retained {removed}"
        );
    }
    check_original_records(&generated, &output);
    assert!(
        output.spans().iter().all(|span| span.source.path == path
            && span.edition == lrr::syntax::token_wire::Edition::E2015)
    );
    // This is a payload component assertion, not Print execution or a compiletest pass.
}

#[test]
fn rustc_derive_order_payload_matches_original_printed_annotation_groups() {
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
        ),
        "derive-expand-order.rs",
    );
    let plan = adt_plan(&g, "Foo", &[0, 1, 2, 3]);
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "Foo"))
        .unwrap();
    let expected: Vec<_> = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.stdout"
    )
    .lines()
    .map(|line| line.split_once(": ").unwrap().1)
    .collect();
    assert_eq!(
        expected.len(),
        plan.annotations().iter().flat_map(|a| a.paths()).count()
    );
    for (row, expected) in expected.iter().enumerate() {
        let output = plan
            .materialize_occurrence_input(&g, index, &CfgSet::guest_default(), row)
            .unwrap();
        let expected = lrr::syntax::token_wire::TokenForest::decode(
            &fixture::input(expected).encode().unwrap(),
        )
        .unwrap();
        assert_eq!(
            identifiers(&output),
            identifiers(&expected),
            "upstream payload occurrence {row}"
        );
        check_original_records(&g, &output);
    }
}

#[test]
fn rustc_generated_derive_order_fixture_preserves_five_ordered_occurrences() {
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
        ),
        "derive-expand-order.rs",
    );
    let plan = adt_plan(&g, "Foo", &[0, 1, 2, 3]);
    assert_eq!(
        plan.annotations()
            .iter()
            .flat_map(|a| a.paths())
            .map(|p| p.segments.clone())
            .collect::<Vec<_>>(),
        vec![
            vec!["First"],
            vec!["Second"],
            vec!["Third"],
            vec!["Fourth"],
            vec!["Fifth"]
        ]
    );
    assert_eq!(
        plan.annotations()
            .iter()
            .map(|a| a.index())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    for annotation in plan.annotations() {
        for path in annotation.paths() {
            let span = annotation
                .arguments()
                .spans()
                .iter()
                .find(|s| s.key() == path.first_span)
                .unwrap();
            assert_eq!(span.edition, lrr::syntax::token_wire::Edition::E2015);
            assert_eq!(span.source.path, "upstream-envelope/derive-expand-order.rs");
        }
    }
}

#[test]
fn rustc_generated_derive_cfg_fixture_keeps_helper_pending_and_disables_field() {
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-attr-cfg.rs"
        ),
        "derive-attr-cfg.rs",
    );
    let plan = adt_plan(&g, "S", &[0]);
    assert_eq!(plan.annotations()[0].paths()[0].segments, ["Foo"]);
    assert_eq!(plan.unresolved_attributes(), [1]);
    assert_eq!(plan.facts().item.attributes[1].name, "foo");
    assert_eq!(plan.facts().members.len(), 1);
    assert!(!plan.facts().members[0].enabled);
}

#[test]
fn rustc_generated_derive_configured_helper_fixture_preserves_outer_and_field_metas() {
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-helper-configured.rs"
        ),
        "derive-helper-configured.rs",
    );
    let plan = adt_plan(&g, "S", &[0]);
    assert_eq!(plan.annotations()[0].paths()[0].segments, ["Empty"]);
    assert_eq!(plan.unresolved_attributes(), [1]);
    assert_eq!(plan.facts().item.attributes[1].name, "empty_helper");
    assert_eq!(plan.facts().members.len(), 1);
    assert_eq!(plan.facts().members[0].attributes[0].name, "empty_helper");
    assert!(plan.facts().members[0].enabled);
}

#[test]
fn rustc_nested_generated_derive_cfg_fixture_keeps_nested_adt_separate() {
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/nested-derive-cfg.rs"
        ),
        "nested-derive-cfg.rs",
    );
    let plan = adt_plan(&g, "Foo", &[0]);
    assert_eq!(plan.annotations()[0].paths()[0].segments, ["Print"]);
    assert_eq!(plan.facts().members.len(), 2);
    assert!(!plan.facts().members[0].enabled);
    assert!(plan.facts().members[1].enabled);
    let original =
        lrr_expand_adapter::token_wire::TokenForest::decode(&g.tokens().encode().unwrap()).unwrap();
    assert!(fixture::literal_texts(&original).iter().any(|t| t == "0"));
}

#[test]
fn generated_derive_path_limit_refuses_before_unbounded_split_retention() {
    let rules = format!(
        "()=>{{#[derive({})] struct Marker;}};",
        "First,".repeat(4097)
    );
    let g = items(&rules, 21);
    assert_eq!(
        GeneratedDerivePlan::prepare(&g, 0, &CfgSet::guest_default(), &annotations(&g, &[0]))
            .err()
            .unwrap()
            .code,
        "LRR-E-MACRO-OUTPUT"
    );
    let rules = format!(
        "()=>{{#[derive({})] #[derive({})] struct Marker;}};",
        "First,".repeat(3000),
        "Second,".repeat(3000)
    );
    let g = items(&rules, 21);
    assert_eq!(
        GeneratedDerivePlan::prepare(&g, 0, &CfgSet::guest_default(), &annotations(&g, &[0, 1]))
            .err()
            .unwrap()
            .code,
        "LRR-E-DISCOVERY-LIMIT"
    );
}

fn compiler_namespace() -> (
    lrr::frontend::macro_names::MacroNamespaces,
    lrr::frontend::builtin_attributes::BuiltinAttributeCatalog,
) {
    use lrr::{
        expand::scopes::MacroName,
        frontend::{
            builtin_attributes::BuiltinAttributeCatalog,
            macro_names::{MacroNamespaces, MacroPath},
        },
    };
    let builtin = BuiltinAttributeCatalog::new(
        Digest([42; 32]),
        2024,
        [descriptor("derive").unwrap().unwrap()],
    )
    .unwrap();
    let mut names = MacroNamespaces::default();
    names
        .define(
            MacroPath::new(
                Digest([2; 32]),
                vec![MacroName::parse("derive", 2024).unwrap()],
            )
            .unwrap(),
            builtin.definitions().next().unwrap().clone(),
        )
        .unwrap();
    (names, builtin)
}
fn caller() -> lrr::syntax::outline::ScopeKey {
    lrr::syntax::outline::ScopeKey {
        crate_key: Digest([2; 32]),
        module_path: vec![],
        owner: None,
    }
}
#[test]
fn rustc_generated_derive_order_resolves_exact_compiler_definition_and_replays() {
    use lrr::frontend::generated_derives::GeneratedDeriveResolution;
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
        ),
        "derive-expand-order.rs",
    );
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "Foo"))
        .unwrap();
    let (names, builtin) = compiler_namespace();
    let selected = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        caller(),
        &names,
        &builtin,
    )
    .unwrap();
    let plan = selected.plan(&g, index, &CfgSet::guest_default()).unwrap();
    assert_eq!(plan.annotations().len(), 4);
    assert_eq!(
        plan.annotations()
            .iter()
            .flat_map(|a| a.paths())
            .map(|p| p.segments.clone())
            .collect::<Vec<_>>(),
        vec![
            vec!["First"],
            vec!["Second"],
            vec!["Third"],
            vec!["Fourth"],
            vec!["Fifth"]
        ]
    );
    selected
        .revalidate(&g, index, &CfgSet::guest_default(), &names, &builtin)
        .unwrap();
}

#[test]
fn generated_derive_namespace_alias_preserves_definition_and_shadow_is_not_builtin() {
    use lrr::{
        expand::scopes::{MacroDefinition, MacroName},
        frontend::{
            generated_derives::GeneratedDeriveResolution,
            macro_names::{MacroNamespaces, MacroPath, PathLookup},
        },
    };
    let g = items("()=>{#[alias(First)] struct Marker;};", 25);
    let (mut names, builtin) = compiler_namespace();
    let path = |name| {
        MacroPath::new(
            caller().crate_key,
            vec![MacroName::parse(name, 2024).unwrap()],
        )
        .unwrap()
    };
    names.reexport(path("alias"), path("derive")).unwrap();
    let selected = GeneratedDeriveResolution::resolve(
        &g,
        0,
        &CfgSet::guest_default(),
        caller(),
        &names,
        &builtin,
    )
    .unwrap();
    assert_eq!(
        selected
            .plan(&g, 0, &CfgSet::guest_default())
            .unwrap()
            .annotations()[0]
            .paths()[0]
            .segments,
        ["First"]
    );
    let user = MacroDefinition {
        identity: Digest([43; 32]),
        defining_unit: caller().crate_key,
        definition_edition: 2024,
    };
    let mut shadow = MacroNamespaces::default();
    shadow.define(path("alias"), user.clone()).unwrap();
    let selected = GeneratedDeriveResolution::resolve(
        &g,
        0,
        &CfgSet::guest_default(),
        caller(),
        &shadow,
        &builtin,
    )
    .unwrap();
    assert_eq!(selected.candidates()[&0], PathLookup::Defined(user));
    let plan = selected.plan(&g, 0, &CfgSet::guest_default()).unwrap();
    assert!(plan.annotations().is_empty());
    assert_eq!(plan.unresolved_attributes(), [0]);
}

#[test]
fn generated_derive_namespace_replay_refuses_missing_pending_changed_and_ambiguous_selection() {
    use lrr::{
        expand::scopes::{MacroDefinition, MacroName},
        frontend::{
            generated_derives::GeneratedDeriveResolution,
            macro_names::{MacroNamespaces, MacroPath},
        },
    };
    let g = items("()=>{#[derive(First)] struct Marker;};", 27);
    let (names, builtin) = compiler_namespace();
    let selected = GeneratedDeriveResolution::resolve(
        &g,
        0,
        &CfgSet::guest_default(),
        caller(),
        &names,
        &builtin,
    )
    .unwrap();
    let path = || {
        MacroPath::new(
            caller().crate_key,
            vec![MacroName::parse("derive", 2024).unwrap()],
        )
        .unwrap()
    };
    let user = MacroDefinition {
        identity: Digest([45; 32]),
        defining_unit: caller().crate_key,
        definition_edition: 2024,
    };
    let absent = MacroNamespaces::default();
    assert_eq!(
        selected
            .revalidate(&g, 0, &CfgSet::guest_default(), &absent, &builtin)
            .unwrap_err()
            .code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let mut pending = MacroNamespaces::default();
    pending.pending(path(), Digest([46; 32])).unwrap();
    assert_eq!(
        selected
            .revalidate(&g, 0, &CfgSet::guest_default(), &pending, &builtin)
            .unwrap_err()
            .code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    let mut changed = MacroNamespaces::default();
    changed.define(path(), user.clone()).unwrap();
    assert_eq!(
        selected
            .revalidate(&g, 0, &CfgSet::guest_default(), &changed, &builtin)
            .unwrap_err()
            .code,
        "LRR-E-EXPANSION-INPUT"
    );
    changed
        .define(path(), builtin.definitions().next().unwrap().clone())
        .unwrap();
    assert_eq!(
        selected
            .revalidate(&g, 0, &CfgSet::guest_default(), &changed, &builtin)
            .unwrap_err()
            .code,
        "LRR-E-MACRO-AMBIGUOUS"
    );
}

#[test]
fn rustc_generated_derive_helpers_keep_pending_namespace_candidates_without_false_completion() {
    use lrr::frontend::{generated_derives::GeneratedDeriveResolution, macro_names::PathLookup};
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-helper-configured.rs"
        ),
        "derive-helper-configured.rs",
    );
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "S"))
        .unwrap();
    let (names, builtin) = compiler_namespace();
    let selected = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        caller(),
        &names,
        &builtin,
    )
    .unwrap();
    assert_eq!(selected.candidates()[&1], PathLookup::Absent);
    let plan = selected.plan(&g, index, &CfgSet::guest_default()).unwrap();
    assert_eq!(plan.annotations().len(), 1);
    assert_eq!(plan.unresolved_attributes(), [1]);
}

#[test]
fn rustc_generated_attribute_after_derive_keeps_replacement_pending_in_both_orders() {
    use lrr::{
        expand::scopes::{MacroDefinition, MacroName},
        frontend::{
            generated_derives::GeneratedDeriveResolution,
            macro_names::{MacroPath, PathLookup},
        },
    };
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/attribute-after-derive.rs"
        ),
        "attribute-after-derive.rs",
    );
    let (mut names, builtin) = compiler_namespace();
    let replacement = MacroDefinition {
        identity: Digest([47; 32]),
        defining_unit: caller().crate_key,
        definition_edition: 2015,
    };
    names
        .define(
            MacroPath::new(
                caller().crate_key,
                vec![MacroName::parse("print_attr", 2015).unwrap()],
            )
            .unwrap(),
            replacement.clone(),
        )
        .unwrap();
    for (name, derive_index, replacement_index) in
        [("AttributeDerive", 1, 0), ("DeriveAttribute", 0, 1)]
    {
        let index = g
            .items()
            .iter()
            .position(|h| h.name.as_ref().is_some_and(|n| n.text == name))
            .unwrap();
        let selected = GeneratedDeriveResolution::resolve(
            &g,
            index,
            &CfgSet::guest_default(),
            caller(),
            &names,
            &builtin,
        )
        .unwrap();
        let plan = selected.plan(&g, index, &CfgSet::guest_default()).unwrap();
        assert_eq!(plan.annotations()[0].index(), derive_index);
        assert_eq!(plan.annotations()[0].paths()[0].segments, ["Print"]);
        assert_eq!(plan.unresolved_attributes(), [replacement_index]);
        assert_eq!(
            selected.candidates()[&replacement_index],
            PathLookup::Defined(replacement.clone())
        );
        assert!(!plan.facts().members[0].enabled);
    }
}

fn identifiers(tokens: &lrr::syntax::token_wire::TokenForest) -> Vec<String> {
    use lrr::syntax::token_wire::TokenTree;
    let mut names = Vec::new();
    let mut pending = tokens.roots().iter().rev().collect::<Vec<_>>();
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Ident { text, .. } => names.push(text.clone()),
            TokenTree::Group { children, .. } => pending.extend(children.iter().rev()),
            _ => {}
        }
    }
    names
}
fn check_original_records(g: &GeneratedItems, output: &lrr::syntax::token_wire::TokenForest) {
    for row in output.spans() {
        assert!(g.tokens().spans().iter().any(|original| original == row));
    }
    for row in output.contexts() {
        assert!(g.tokens().contexts().iter().any(|original| original == row));
    }
    let adapter =
        lrr_expand_adapter::token_wire::TokenForest::decode(&output.encode().unwrap()).unwrap();
    lrr_expand_adapter::item_output::materialize(&adapter).unwrap();
}
#[test]
fn rustc_generated_derive_cfg_materialization_removes_disabled_field_and_retains_helper_origins() {
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-attr-cfg.rs"
        ),
        "derive-attr-cfg.rs",
    );
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "S"))
        .unwrap();
    let plan = adt_plan(&g, "S", &[0]);
    let output = plan
        .materialize_input(&g, index, &CfgSet::guest_default())
        .unwrap();
    assert_eq!(identifiers(&output), ["foo", "struct", "S"]);
    assert_eq!(plan.unresolved_attributes(), [1]);
    check_original_records(&g, &output);
}
#[test]
fn rustc_generated_derive_configured_helper_materialization_reconstructs_item_and_field_attributes()
{
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-helper-configured.rs"
        ),
        "derive-helper-configured.rs",
    );
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "S"))
        .unwrap();
    let output = adt_plan(&g, "S", &[0])
        .materialize_input(&g, index, &CfgSet::guest_default())
        .unwrap();
    assert_eq!(
        identifiers(&output),
        ["empty_helper", "struct", "S", "empty_helper", "field", "u8"]
    );
    check_original_records(&g, &output);
}
#[test]
fn rustc_nested_derive_cfg_materialization_filters_nested_adt_without_borrowing_outer_helper_scope()
{
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/nested-derive-cfg.rs"
        ),
        "nested-derive-cfg.rs",
    );
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "Foo"))
        .unwrap();
    let plan = adt_plan(&g, "Foo", &[0]);
    assert_eq!(plan.facts().members.len(), 2);
    let output = plan
        .materialize_input(&g, index, &CfgSet::guest_default())
        .unwrap();
    let names = identifiers(&output);
    assert!(
        !names
            .iter()
            .any(|n| n == "removed" || n == "removed_inner_field")
    );
    assert!(names.iter().any(|n| n == "non_removed_inner_field"));
    assert!(names.iter().any(|n| n == "Inner"));
    check_original_records(&g, &output);
}
#[test]
fn generated_derive_materialization_edits_occurrences_with_shared_span_keys_independently() {
    let g = expanded(
        "($($attr:tt)*)=>{$($attr)* $($attr)* struct Marker;};",
        fixture::input("#[derive(First)]"),
        29,
        2024,
    );
    let plan =
        GeneratedDerivePlan::prepare(&g, 0, &CfgSet::guest_default(), &annotations(&g, &[0]))
            .unwrap();
    let output = plan
        .materialize_input(&g, 0, &CfgSet::guest_default())
        .unwrap();
    assert_eq!(
        identifiers(&output)
            .iter()
            .filter(|n| *n == "derive")
            .count(),
        1
    );
    assert_eq!(plan.unresolved_attributes(), [1]);
    check_original_records(&g, &output);
    let other = items("()=>{#[derive(First)] struct Other;};", 29);
    assert_eq!(
        plan.materialize_input(&other, 0, &CfgSet::guest_default())
            .err()
            .unwrap()
            .code,
        "LRR-E-EXPANSION-INPUT"
    );
}

#[test]
fn rustc_75930_generated_derive_cfg_filters_all_targets_and_preserves_original_spacing() {
    use lrr::syntax::token_wire::TokenTree;
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/issue-75930-derive-cfg.rs"
        ),
        "issue-75930-derive-cfg.rs",
    );
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "Foo"))
        .unwrap();
    let plan = adt_plan(&g, "Foo", &[3]);
    let output = plan
        .materialize_input(&g, index, &CfgSet::guest_default())
        .unwrap();
    let names = identifiers(&output);
    for removed in [
        "A",
        "first",
        "Bar",
        "removed_fn",
        "plain_removed_fn",
        "should_be_removed",
        "String",
    ] {
        assert!(
            !names.iter().any(|n| n == removed),
            "retained disabled target {removed}"
        );
    }
    // The helper argument `a` survives; the cfg-disabled local named `a` does not.
    assert_eq!(names.iter().filter(|n| *n == "a").count(), 1);
    for kept in [
        "B",
        "Inner",
        "kept_fn",
        "TupleStruct",
        "TupleEnum",
        "my_val",
        "second",
        "fourth",
    ] {
        assert!(
            names.iter().any(|n| n == kept),
            "discarded retained target {kept}"
        );
    }
    let [TokenTree::Group { children, .. }] = output.roots() else {
        panic!("materialized root absent")
    };
    // This component uses an identity-MBE envelope, not rustc's full original
    // pipeline. Preserve its captured spacing, including the `<` whose next
    // generic parameter changes after cfg removal. Full snapshot acceptance is
    // pending the production attribute/derive coordinator.
    let mut original = g.tokens().roots().iter().collect::<Vec<_>>();
    let mut original_puncts = Vec::new();
    while let Some(token) = original.pop() {
        match token {
            TokenTree::Punct { .. } => original_puncts.push(token),
            TokenTree::Group { children, .. } => original.extend(children.iter()),
            _ => {}
        }
    }
    let opening = children
        .windows(2)
        .find_map(|pair| match (&pair[0], &pair[1]) {
            (TokenTree::Punct { scalar: '<', .. }, TokenTree::Ident { text, .. })
                if text == "B" =>
            {
                Some(&pair[0])
            }
            _ => None,
        })
        .expect("configured generic parameter B missing");
    assert!(
        original_puncts.contains(&opening),
        "changed original punctuation or spacing"
    );

    check_original_records(&g, &output);
}
fn annotations(g: &GeneratedItems, indexes: &[usize]) -> GeneratedDeriveAnnotations {
    GeneratedDeriveAnnotations::new(
        g,
        0,
        &CfgSet::guest_default(),
        indexes
            .iter()
            .map(|i| (*i, descriptor("derive").unwrap().unwrap())),
    )
    .unwrap()
}

#[test]
fn generated_derive_paths_retain_order_raw_absolute_repetitions_and_real_origins() {
    let g = items(
        "()=>{#[derive(First, ::renamed::r#gen, First,)] #[derive(crate::Last)] struct Marker;};",
        7,
    );
    let plan =
        GeneratedDerivePlan::prepare(&g, 0, &CfgSet::guest_default(), &annotations(&g, &[0, 1]))
            .unwrap();
    let paths = plan
        .annotations()
        .iter()
        .flat_map(|a| a.paths())
        .collect::<Vec<_>>();
    assert_eq!(
        paths.iter().map(|p| p.segments.clone()).collect::<Vec<_>>(),
        vec![
            vec!["First"],
            vec!["renamed", "r#gen"],
            vec!["First"],
            vec!["crate", "Last"]
        ]
    );
    assert_eq!(
        paths.iter().map(|p| p.absolute).collect::<Vec<_>>(),
        [false, true, false, false]
    );
    assert_ne!(paths[0].identity, paths[2].identity);
    assert!(plan.unresolved_attributes().is_empty());
    for path in paths {
        for span in [path.first_span, path.last_span] {
            let original = g.tokens().spans().iter().find(|s| s.key() == span).unwrap();
            assert_eq!(original.edition, lrr::syntax::token_wire::Edition::E2024);
            assert_eq!(original.source.path, "unit-1.rs");
        }
    }
    for path in plan.annotations().iter().flat_map(|a| a.paths()) {
        for span in path.tokens().spans() {
            assert_eq!(
                g.tokens().spans().iter().find(|s| s.key() == span.key()),
                Some(span)
            );
        }
        assert!(!path.tokens().roots().is_empty());
    }
    assert_eq!(plan.item_identity(), g.items()[0].key.fingerprint());
}

#[test]
fn generated_derive_cfg_attr_paths_and_disabled_member_facts_do_not_demand_cold_tokens() {
    let g = items(
        "()=>{#[cfg_attr(any(), derive(Bad<T>))] #[cfg_attr(all(), derive(Active))] enum Marker { #[cfg(any())] Hidden { #[cfg_attr(all(), broken)] missing: Unknown }, #[cfg_attr(all(), helper)] Visible { field: Unknown } }};",
        9,
    );
    let plan =
        GeneratedDerivePlan::prepare(&g, 0, &CfgSet::guest_default(), &annotations(&g, &[0]))
            .unwrap();
    assert_eq!(plan.annotations()[0].paths()[0].segments, ["Active"]);
    let facts = plan.facts();
    assert_eq!(facts.members.len(), 3);
    assert!(!facts.members[0].enabled);
    assert!(facts.members[1].enabled);
    assert_eq!(facts.members[1].attributes[0].name, "helper");
    assert!(
        facts
            .members
            .iter()
            .flat_map(|m| &m.attributes)
            .all(|a| a.name != "broken")
    );
    let arguments = plan.annotations()[0].arguments();
    assert!(
        arguments
            .spans()
            .iter()
            .all(|s| g.tokens().spans().iter().any(|old| old == s))
    );
}

#[test]
fn generated_derive_proof_binds_output_header_cfg_and_epoch() {
    let g = items("()=>{#[derive(First)] struct Marker;};", 11);
    let proof = annotations(&g, &[0]);
    for other in [
        items("()=>{#[derive(Second)] struct Marker;};", 11),
        items("()=>{#[derive(First)] struct Marker;};", 12),
    ] {
        assert_eq!(
            GeneratedDerivePlan::prepare(&other, 0, &CfgSet::guest_default(), &proof)
                .err()
                .unwrap()
                .code,
            "LRR-E-EXPANSION-INPUT"
        );
    }
    let sibling = items(
        "()=>{#[derive(First)] struct Marker; #[derive(First)] struct Sibling;};",
        11,
    );
    let proof = annotations(&sibling, &[0]);
    assert!(GeneratedDerivePlan::prepare(&sibling, 1, &CfgSet::guest_default(), &proof).is_err());
    let mut changed = CfgSet::guest_default();
    changed.atoms.insert("extra".into());
    assert!(GeneratedDerivePlan::prepare(&g, 0, &changed, &annotations(&g, &[0])).is_err());
}

#[test]
fn generated_derive_requires_exact_annotation_kind_and_keeps_helpers_and_transforms_unresolved() {
    let g = items(
        "()=>{#[replace] #[alias(First)] #[helper] #[derive(Second)] struct Marker;};",
        13,
    );
    // The caller's namespace-selected descriptor can certify an alias, not just spelling.
    let plan =
        GeneratedDerivePlan::prepare(&g, 0, &CfgSet::guest_default(), &annotations(&g, &[1]))
            .unwrap();
    assert_eq!(plan.annotations()[0].paths()[0].segments, ["First"]);
    assert_eq!(plan.unresolved_attributes(), [0, 2, 3]);
    assert!(
        GeneratedDeriveAnnotations::new(
            &g,
            0,
            &CfgSet::guest_default(),
            [(1, descriptor("test").unwrap().unwrap())]
        )
        .is_err()
    );
    assert!(
        GeneratedDeriveAnnotations::new(
            &g,
            0,
            &CfgSet::guest_default(),
            [
                (1, descriptor("derive").unwrap().unwrap()),
                (1, descriptor("derive").unwrap().unwrap())
            ]
        )
        .is_err()
    );
}

#[test]
fn generated_derive_refuses_malformed_paths_non_adts_and_disabled_items() {
    for rules in [
        "()=>{#[derive(First<T>)] struct Marker;};",
        "()=>{#[derive(First,,Last)] struct Marker;};",
        "()=>{#[derive(&First)] struct Marker;};",
    ] {
        let g = items(rules, 15);
        assert_eq!(
            GeneratedDerivePlan::prepare(&g, 0, &CfgSet::guest_default(), &annotations(&g, &[0]))
                .err()
                .unwrap()
                .code,
            "LRR-E-MACRO-OUTPUT"
        );
    }
    for rules in [
        "()=>{#[derive(First)] fn invalid(){undefined();}};",
        "()=>{#[cfg(any())] #[derive(Bad<T>)] struct Disabled;};",
    ] {
        let g = items(rules, 15);
        assert!(
            GeneratedDeriveAnnotations::new(
                &g,
                0,
                &CfgSet::guest_default(),
                [(0, descriptor("derive").unwrap().unwrap())]
            )
            .is_err()
        );
    }
}

#[test]
fn standard_compiler_annotation_exports_retain_core_identity_in_edition_preludes() {
    use lrr::{
        core::profile::ProfileIdentity,
        expand::scopes::MacroName,
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            macro_names::{MacroPath, PathLookup},
            source_macro_routes::SourceMacroRoutes,
        },
    };
    let catalog = CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap();
    let attributes = catalog.attribute_catalog().unwrap();
    let routes = catalog.source_routes().unwrap();
    let names = SourceMacroRoutes::candidate_graph(&[routes]).unwrap();
    for definition in attributes.definitions() {
        let descriptor = attributes.select(definition).unwrap().unwrap();
        for prefix in [
            vec![],
            vec!["prelude", "v1"],
            vec!["prelude", "rust_2015"],
            vec!["prelude", "rust_2018"],
            vec!["prelude", "rust_2021"],
            vec!["prelude", "rust_2024"],
        ] {
            let mut path = prefix
                .iter()
                .map(|n| MacroName::parse(n, 2024).unwrap())
                .collect::<Vec<_>>();
            path.push(MacroName::parse(descriptor.name(), 2024).unwrap());
            let expected = if prefix.is_empty() && descriptor.name() == "test" {
                PathLookup::Absent
            } else {
                PathLookup::Defined(definition.clone())
            };
            assert_eq!(
                names
                    .lookup(&MacroPath::new(definition.defining_unit, path).unwrap())
                    .unwrap(),
                expected
            );
        }
        let mut forged = definition.clone();
        forged.definition_edition = 2021;
        assert!(attributes.select(&forged).is_err());
        assert!(catalog.select(definition).unwrap().is_none());
    }
}

#[test]
fn custom_compile_input_facet_cannot_grant_standard_annotation_identity() {
    use lrr::frontend::compile_input_catalog::CompileInputCatalog;
    let catalog = CompileInputCatalog::new(
        Digest([99; 32]),
        2024,
        [lrr::expand::compile_input_identity::descriptor("env")
            .unwrap()
            .unwrap()],
    )
    .unwrap();
    assert!(catalog.attribute_catalog().is_none());
    assert_eq!(catalog.source_routes().unwrap().definitions().len(), 1);
}

#[test]
fn rustc_derive_order_uses_production_standard_annotation_facet() {
    use lrr::{
        core::profile::ProfileIdentity,
        expand::scopes::MacroName,
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            generated_derives::GeneratedDeriveResolution,
            macro_names::{MacroNamespaces, MacroPath},
        },
    };
    let g = upstream(
        include_str!(
            "../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/derive-expand-order.rs"
        ),
        "derive-expand-order.rs",
    );
    let index = g
        .items()
        .iter()
        .position(|h| h.name.as_ref().is_some_and(|n| n.text == "Foo"))
        .unwrap();
    let catalog = CompileInputCatalog::standalone(&ProfileIdentity::current()).unwrap();
    let attributes = catalog.attribute_catalog().unwrap();
    let definition = attributes
        .definitions()
        .find(|d| attributes.select(d).unwrap().unwrap().name() == "derive")
        .unwrap();
    let mut names = MacroNamespaces::default();
    names
        .define(
            MacroPath::new(
                caller().crate_key,
                vec![MacroName::parse("derive", 2015).unwrap()],
            )
            .unwrap(),
            definition.clone(),
        )
        .unwrap();
    let resolution = GeneratedDeriveResolution::resolve(
        &g,
        index,
        &CfgSet::guest_default(),
        caller(),
        &names,
        attributes,
    )
    .unwrap();
    let plan = resolution
        .plan(&g, index, &CfgSet::guest_default())
        .unwrap();
    assert_eq!(
        plan.annotations()
            .iter()
            .flat_map(|a| a.paths())
            .map(|p| p.segments[0].as_str())
            .collect::<Vec<_>>(),
        ["First", "Second", "Third", "Fourth", "Fifth"]
    );
    resolution
        .revalidate(&g, index, &CfgSet::guest_default(), &names, attributes)
        .unwrap();
}
