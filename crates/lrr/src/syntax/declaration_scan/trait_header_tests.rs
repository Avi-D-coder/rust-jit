//! Actual declaration capture, not callable/impl-catalog or guest acceptance.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    expand::{
        generated_declarations::GeneratedDeclarations, source_declarative::SourceDeclarativeDemand,
    },
    source::snapshot::SourceBlob,
};
use std::sync::Arc;

const STOCK: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-as-fn-body.rs"
);

fn parsed(text: &str, edition: u16) -> ParsedFile {
    ParsedFile::parse_declarations(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap()
}

fn capture(text: &str, edition: u16) -> DeclarationScan {
    parsed(text, edition)
        .scan_declarations(
            SourcePath::new("trait-capture.rs").unwrap(),
            ScopeKey {
                crate_key: Digest([205; 32]),
                module_path: vec![],
                owner: None,
            },
            &CfgSet::guest_default(),
            Default::default(),
        )
        .unwrap()
}

fn actual_outputs(text: &str, edition: u16) -> Vec<GeneratedDeclarations> {
    let (provider, index, _inputs) =
        crate::expand::source_declarative::assembly::tests::world(text, edition);
    let cfg = provider.module_source().captured_macro_cfg().unwrap();
    let mut outputs = Vec::new();
    for producer in index.scan().producers().iter().filter(|producer| {
        producer.kind == DeclarationProducerKind::Macro
            && producer.path.iter().any(|token| token.text == "def_fn")
    }) {
        let SourceDeclarativeDemand::Ready(request) = provider
            .declaration_demand(
                &index,
                producer.identity,
                Digest([206; 32]),
                crate::expand::source_declarative::engine_version(),
            )
            .unwrap()
        else {
            panic!("actual stock def_fn producer must resolve")
        };
        let key = request.key().clone();
        let record = Arc::new(request.expand().unwrap());
        let output =
            GeneratedDeclarations::admit(&key, record, 1, index.scan(), cfg, Default::default())
                .unwrap();
        assert_eq!(output.parent_inventory(), index.scan().fingerprint());
        assert_eq!(output.parent_invocation(), producer.identity);
        outputs.push(output);
    }
    outputs
}

#[test]
fn unchanged_stock_trait_and_impl_macro_headers_capture_real_owners_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let scan = capture(STOCK, edition);
        assert_eq!(scan.traits().len(), 1);
        assert_eq!(scan.impls().len(), 2);
        assert!(scan.associated_fragments().is_empty());
        let actual_trait = &scan.traits()[0];
        let name = actual_trait.name.as_ref().unwrap();
        assert_eq!(name.spelling, "Foo");
        assert_eq!(name.edition, edition);
        assert_eq!(name.context, DeclarationNameContext::Original);
        assert_eq!(
            &STOCK[name.range.start as usize..name.range.end as usize],
            "Foo"
        );
        assert_eq!(actual_trait.associated.len(), 1);
        let row = &actual_trait.associated[0];
        assert_eq!(row.kind, AssociatedDeclarationKind::MacroProducer);
        assert_eq!(
            row.owner,
            AssociatedDeclarationOwner::Trait(actual_trait.identity)
        );
        assert!(row.name.is_none());
        assert!(row.body.is_none());
        let producer = scan
            .producers()
            .iter()
            .find(|p| Some(p.identity) == row.producer)
            .unwrap();
        assert_eq!(producer.range, row.range);
        assert_eq!(producer.lexical_scope, row.lexical_scope);
        assert_eq!(producer.grammar, GrammarSlot::TraitItems);
        let impl_owner = scan
            .impls()
            .iter()
            .find(|item| !item.associated.is_empty())
            .unwrap();
        assert_eq!(impl_owner.associated.len(), 1);
        let row = &impl_owner.associated[0];
        assert_eq!(
            row.owner,
            AssociatedDeclarationOwner::Impl(impl_owner.identity)
        );
        let producer = scan
            .producers()
            .iter()
            .find(|p| Some(p.identity) == row.producer)
            .unwrap();
        assert_eq!(producer.grammar, GrammarSlot::ImplItems);
        assert_eq!(producer.lexical_scope, row.lexical_scope);
        assert!(
            scan.lexical_scopes()
                .iter()
                .any(|scope| scope.identity == row.lexical_scope)
        );
    }
}

#[test]
fn real_stock_generated_associated_slots_and_identical_outputs_keep_source_identity() {
    let supplement = "\nmod lrr_repeat_one { trait Marker { def_fn!({}); } }\nmod lrr_repeat_two { trait Marker { def_fn!({}); } }\n";
    for edition in [2015, 2018, 2021, 2024] {
        let text = format!("{STOCK}{supplement}");
        assert!(text.starts_with(STOCK));
        let outputs = actual_outputs(&text, edition);
        assert_eq!(outputs.len(), 5);
        let mut trait_slots = 0;
        let mut impl_slots = 0;
        let mut items = 0;
        for output in &outputs {
            let grammar = output.record().key().grammar;
            if grammar == GrammarSlot::Items {
                items += 1;
                assert!(output.scan().associated_fragments().is_empty());
                continue;
            }
            match grammar {
                GrammarSlot::TraitItems => trait_slots += 1,
                GrammarSlot::ImplItems => impl_slots += 1,
                _ => panic!("stock declaration output has an unexpected grammar"),
            }
            let rows = output.scan().associated_fragments();
            assert_eq!(rows.len(), 1);
            let row = &rows[0];
            assert_eq!(row.kind, AssociatedDeclarationKind::Function);
            assert!(row.body.is_some());
            assert_eq!(row.producer, None);
            assert_eq!(
                row.owner,
                AssociatedDeclarationOwner::Fragment {
                    grammar,
                    lexical_scope: output.scan().lexical_scopes()[0].identity,
                }
            );
            let name = row.name.as_ref().unwrap();
            assert_eq!(name.spelling, "bar");
            assert_eq!(name.edition, edition);
            assert!(matches!(name.context, DeclarationNameContext::Captured(_)));
            assert!(!row.header.iter().any(|token| token.text == "println"));
        }
        assert_eq!((trait_slots, impl_slots, items), (3, 1, 1));
        let repeats = outputs
            .iter()
            .filter(|output| {
                output
                    .scan()
                    .scope()
                    .module_path
                    .first()
                    .is_some_and(|name| name.starts_with("lrr_repeat_"))
            })
            .collect::<Vec<_>>();
        assert_eq!(repeats.len(), 2);
        assert_eq!(repeats[0].text(), repeats[1].text());
        assert_ne!(repeats[0].record().key(), repeats[1].record().key());
        assert_ne!(
            repeats[0].parent_invocation(),
            repeats[1].parent_invocation()
        );
        assert_ne!(repeats[0].scan().scope(), repeats[1].scan().scope());
        assert_ne!(
            repeats[0].scan().associated_fragments()[0].identity,
            repeats[1].scan().associated_fragments()[0].identity
        );
    }
}

const DIRECT_HEADERS: &str = "\ntrait LrrHeader<'a, T: 'a>: Sized where T: Copy {\n    type Item<U>;\n    const N: u8 = 11;\n    fn value<'b>(&'b self) -> u8 { 11 }\n    #[cfg(any())] fn disabled() { missing(); }\n}\nimpl<'a, T: Copy + 'a> LrrHeader<'a, T> for () {\n    type Item<U> = U;\n    const N: u8 = 11;\n    fn value<'b>(&'b self) -> u8 { 11 }\n}\n#[cfg(any())] trait LrrDisabled { fn absent() { missing(); } }\n";

#[test]
fn cfg_binders_and_body_ranges_are_facts_without_changing_impl_header_semantics() {
    let first = format!("{STOCK}{DIRECT_HEADERS}");
    let second = format!("{STOCK}{}", DIRECT_HEADERS.replace("11", "22"));
    let before = capture(&first, 2024);
    let after = capture(&second, 2024);
    assert_eq!(before.traits().len(), 2);
    assert!(before.cost().disabled_subtrees >= 2);
    let t = before
        .traits()
        .iter()
        .find(|row| row.name.as_ref().unwrap().spelling == "LrrHeader")
        .unwrap();
    let changed_t = after
        .traits()
        .iter()
        .find(|row| row.name.as_ref().unwrap().spelling == "LrrHeader")
        .unwrap();
    assert!(t.generic_params.is_some());
    assert!(t.where_clause.is_some());
    assert!(t.supertraits.is_some());
    assert_eq!(t.associated.len(), 3);
    assert_eq!(t.header_identity, changed_t.header_identity);
    assert_ne!(t.identity, changed_t.identity);
    assert_ne!(before.fingerprint(), after.fingerprint());
    let i = before
        .impls()
        .iter()
        .find(|row| row.associated.len() == 3)
        .unwrap();
    let changed_i = after
        .impls()
        .iter()
        .find(|row| row.associated.len() == 3)
        .unwrap();
    assert_eq!(i.header_identity, changed_i.header_identity);
    assert_ne!(i.identity, changed_i.identity);
    for row in t.associated.iter().chain(&i.associated) {
        assert!(row.name.is_some());
        assert!(
            !row.header
                .iter()
                .any(|token| token.text == "11" || token.text == "22" || token.text == "missing")
        );
        match row.kind {
            AssociatedDeclarationKind::Function => {
                assert!(row.body.is_some());
                assert!(row.generic_params.is_some());
            }
            AssociatedDeclarationKind::Const => assert!(row.body.is_some()),
            AssociatedDeclarationKind::TypeAlias => {
                assert!(row.generic_params.is_some());
                assert!(row.body.is_none());
            }
            AssociatedDeclarationKind::MacroProducer => {
                panic!("direct supplemental headers contain no macro producer")
            }
        }
    }
}

#[test]
fn unattached_generated_context_and_checked_resource_refusal_do_not_mint_authority() {
    let generated = generated_declaration_fixture(STOCK, GrammarSlot::Items);
    let cfg = CfgSet::guest_default();
    let scope = ScopeKey {
        crate_key: Digest([205; 32]),
        module_path: vec![],
        owner: None,
    };
    let path = SourcePath::new("trait-capture.rs").unwrap();
    let captured = generated
        .scan_declarations(path.clone(), scope.clone(), &cfg, Default::default())
        .unwrap();
    assert_eq!(
        captured.traits()[0].name.as_ref().unwrap().context,
        DeclarationNameContext::UnattachedGenerated
    );
    let mut limits = DeclarationScanLimits::default();
    limits.metadata_bytes = captured.retained_bytes().checked_sub(1).unwrap();
    let error = generated
        .scan_declarations(path, scope, &cfg, limits)
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-DECLARATION-SCAN-LIMIT");
}
