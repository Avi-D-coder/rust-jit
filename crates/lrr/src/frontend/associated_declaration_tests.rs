//! Source attachment controls; no associated call or full-fixture acceptance.
use super::*;

const STOCK: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-as-fn-body.rs"
);
fn source(text: &str, edition: u16) -> PreparedSourceView {
    let (provider, _, _) = crate::expand::source_declarative::assembly::tests::world(text, edition);
    provider.module_source().share()
}
fn prepared(source: &PreparedSourceView) -> PreparedOriginalAssociatedDeclarations<'_> {
    match PreparedOriginalAssociatedDeclarations::prepare(source).unwrap() {
        AssociatedDeclarationsPreparation::Original(value) => value,
        AssociatedDeclarationsPreparation::NeedGeneratedParent => {
            panic!("unchanged original capture unexpectedly requires generated parent")
        }
    }
}
#[test]
fn unchanged_stock_owners_join_canonical_headers_and_real_producer_slots_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let view = source(STOCK, edition);
        let value = prepared(&view);
        assert!(std::ptr::eq(value.source(), &view));
        assert_eq!(value.owner_count(), 3);
        assert_eq!(value.slot_count(), 2);
        let scan = Arc::clone(value.scan());
        let scan_ptr = Arc::as_ptr(value.scan());
        for actual_trait in scan.traits() {
            let owner = value.owner(actual_trait.identity).unwrap();
            let AssociatedOwnerSource::Trait(raw) = owner.source else {
                panic!("trait source entry changed kind")
            };
            assert!(std::ptr::eq(raw, actual_trait));
            let canonical = owner.canonical.unwrap();
            assert_eq!(canonical.def_key.item_kind, ItemKind::Trait);
            assert_eq!(canonical.def_key.declared_name, "Foo");
            assert_eq!(canonical.header.profile.edition, edition);
            assert_eq!(owner.member_owner, Some(canonical.def_key.fingerprint()));
            assert_eq!(canonical.declaration_span, raw.range);
            assert_eq!(canonical.header.tokens, raw.header);
            assert_ne!(canonical.def_key.fingerprint(), raw.identity);
            for row in &raw.associated {
                let slot = value.associated(row.identity).unwrap();
                assert!(std::ptr::eq(slot.source, row));
                assert!(std::ptr::eq(slot.owner.canonical.unwrap(), canonical));
                let OriginalAssociatedTarget::Producer(producer) = slot.target else {
                    panic!("an unexpanded associated macro became a callable declaration")
                };
                assert_eq!(Some(producer.identity), row.producer);
                assert_eq!(producer.grammar, GrammarSlot::TraitItems);
                assert_eq!(producer.lexical_scope, row.lexical_scope);
            }
        }
        for actual_impl in scan.impls() {
            let owner = value.owner(actual_impl.identity).unwrap();
            let AssociatedOwnerSource::Impl(raw) = owner.source else {
                panic!("impl source entry changed kind")
            };
            assert!(std::ptr::eq(raw, actual_impl));
            let canonical = owner.canonical.unwrap();
            assert_eq!(canonical.def_key.item_kind, ItemKind::Impl);
            assert_eq!(canonical.declaration_span, raw.range);
            assert_eq!(canonical.header.tokens, raw.header);
            // The canonical impl child scope is the actual Outline header,
            // never the differently-domained scanned semantic header digest.
            assert_eq!(
                canonical.def_key.enclosing_impl,
                Some(canonical.header.fingerprint())
            );
            assert_ne!(canonical.header.fingerprint(), raw.header_identity);
            assert_eq!(owner.member_owner, Some(canonical.header.fingerprint()));
            for row in &raw.associated {
                let OriginalAssociatedTarget::Producer(producer) =
                    value.associated(row.identity).unwrap().target
                else {
                    panic!("macro slot became method")
                };
                assert_eq!(producer.grammar, GrammarSlot::ImplItems);
            }
        }
        for _ in 0..8 {
            for row in scan.traits() {
                assert!(value.owner(row.identity).unwrap().canonical.is_some());
                for member in &row.associated {
                    assert!(value.associated(member.identity).is_some());
                }
            }
            assert_eq!(Arc::as_ptr(value.scan()), scan_ptr);
        }
        assert!(value.metadata_bytes() >= std::mem::size_of_val(&value));
    }
}
const DIRECT: &str = "\ntrait LrrDirect {\n type Item;\n const COUNT: u8 = 11;\n fn same() { let _ = 11; }\n fn required();\n #[cfg(any())] fn disabled() { missing(); }\n}\nimpl LrrDirect for Baz {\n type Item = ();\n const COUNT: u8 = 11;\n fn same() { let _ = 11; }\n fn required() {}\n}\nimpl Qux { fn same() { let _ = 11; } }\nfn lrr_cold() { struct Local; impl Local { fn hidden() { missing(); } } }\n";
#[test]
fn actual_direct_members_keep_distinct_canonical_owners_and_cold_local_gap() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = format!("{STOCK}{DIRECT}");
        assert!(text.starts_with(STOCK));
        let view = source(&text, edition);
        let value = prepared(&view);
        let mut direct = 0;
        let mut producers = 0;
        let mut local_gaps = 0;
        let mut same = Vec::new();
        let rows = value
            .scan()
            .traits()
            .iter()
            .flat_map(|r| &r.associated)
            .chain(value.scan().impls().iter().flat_map(|r| &r.associated));
        for row in rows {
            let slot = value.associated(row.identity).unwrap();
            match slot.target {
                OriginalAssociatedTarget::Declaration(item) => {
                    direct += 1;
                    let parent = slot.owner.canonical.unwrap();
                    let owner = if parent.def_key.item_kind == ItemKind::Trait {
                        parent.def_key.fingerprint()
                    } else {
                        parent.header.fingerprint()
                    };
                    assert_eq!(item.scope.owner, Some(owner));
                    assert_eq!(item.def_key.enclosing_impl, Some(owner));
                    assert_eq!(item.header.context, parent.header.tokens);
                    assert_eq!(item.header.tokens, row.header);
                    assert_eq!(item.declaration_span, row.range);
                    assert_eq!(item.body.as_ref().map(|b| b.range), row.body);
                    assert!(
                        !row.header
                            .iter()
                            .any(|t| t.text == "missing" || t.text == "11")
                    );
                    if item.def_key.declared_name == "same" {
                        same.push((owner, item.def_key.fingerprint()));
                    }
                }
                OriginalAssociatedTarget::Producer(_) => {
                    producers += 1;
                    assert!(slot.owner.canonical.is_some());
                }
                OriginalAssociatedTarget::NeedCanonicalOwner => {
                    local_gaps += 1;
                    assert!(slot.owner.canonical.is_none());
                    assert!(slot.owner.member_owner.is_none());
                    assert_eq!(row.name.as_ref().unwrap().spelling, "hidden");
                    assert!(row.body.is_some());
                }
            }
        }
        assert_eq!((direct, producers, local_gaps), (9, 2, 1));
        assert_eq!(same.len(), 3);
        same.sort_unstable();
        assert!(
            same.windows(2)
                .all(|r| r[0].0 != r[1].0 && r[0].1 != r[1].1)
        );
        assert!(
            !value
                .scan()
                .traits()
                .iter()
                .flat_map(|r| &r.associated)
                .any(|r| r.name.as_ref().is_some_and(|n| n.spelling == "disabled"))
        );
    }
}
#[test]
fn changed_actual_source_cannot_reuse_join_but_equivalent_immutable_capture_can() {
    let first = format!("{STOCK}{DIRECT}");
    let second = format!("{STOCK}{}", DIRECT.replace("11", "22"));
    let a = source(&first, 2024);
    let b = source(&second, 2024);
    let equivalent = source(&first, 2024);
    let old = prepared(&a);
    let new = prepared(&b);
    assert_ne!(old.fingerprint(), new.fingerprint());
    assert_eq!(old.fingerprint(), prepared(&equivalent).fingerprint());
    assert_eq!(
        PreparedOriginalAssociatedDeclarations::from_scan(&b, Arc::clone(old.scan()))
            .err()
            .unwrap()
            .code,
        "LRR-E-ASSOCIATED-DECLARATION"
    );
    // Repeated lookups return retained facts and never consult the other view.
    for row in old.scan().traits() {
        let owner = old.owner(row.identity).unwrap();
        let actual = owner.canonical.unwrap();
        assert!(a.outline().items.iter().any(|i| std::ptr::eq(i, actual)));
        assert!(!b.outline().items.iter().any(|i| std::ptr::eq(i, actual)));
    }
}
#[test]
fn original_component_rejects_generated_slot_scan_and_checked_index_overflow() {
    use crate::expand::{
        generated_declarations::GeneratedDeclarations,
        source_declarative::{SourceDeclarativeDemand, engine_version},
    };
    let (provider, index, _) =
        crate::expand::source_declarative::assembly::tests::world(STOCK, 2015);
    let producer = index
        .scan()
        .producers()
        .iter()
        .find(|r| r.grammar == GrammarSlot::TraitItems && r.kind == DeclarationProducerKind::Macro)
        .unwrap();
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(
            &index,
            producer.identity,
            Digest([207; 32]),
            engine_version(),
        )
        .unwrap()
    else {
        panic!("actual trait slot demand absent")
    };
    let record = Arc::new(request.expand().unwrap());
    let generated = GeneratedDeclarations::admit(
        request.key(),
        record,
        1,
        index.scan(),
        provider.module_source().captured_macro_cfg().unwrap(),
        Default::default(),
    )
    .unwrap();
    assert!(!generated.scan().associated_fragments().is_empty());
    let error = PreparedOriginalAssociatedDeclarations::from_scan(
        provider.module_source(),
        Arc::clone(generated.scan()),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "LRR-E-ASSOCIATED-DECLARATION");
    let mut budget = 0;
    assert!(reserve::<SlotLocation>(MAX_ROWS + 1, &mut budget).is_err());
    assert!(reserve::<SlotLocation>(usize::MAX, &mut budget).is_err());
    assert!(checked_add(MAX_BYTES, 1).is_err());
}
