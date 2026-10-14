//! Source-derived local ordering, with disclosed scalar retirement adaptation.
//! No closure/pattern/LogDrop body or full stock-program execution is claimed.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::pc::Phase,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::NodeRole, parse::ParsedFile, scope_fixture_facts},
};

const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/or-pattern-drop-order.rs"
);

#[test]
fn rustc_deferred_local_cleanup_keeps_primary_declaration_order_after_reverse_initialization() {
    let facts = scope_fixture_facts::original_deferred_binding_order(ORIGINAL);
    let ty = crate::runtime::shape::PrimitiveIdentity::from_recipe(facts.tag_type)
        .unwrap()
        .id();
    // The original independent native assertion supplies these expected tags.
    // A reverse-initialization model demonstrably disagrees with that oracle.
    let buggy: Vec<_> = facts
        .initializations
        .iter()
        .rev()
        .map(|(_, tag)| *tag)
        .collect();
    assert_ne!(buggy, facts.expected_tags);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(facts.indexed_source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"rustc-deferred-local-order-adaptation"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let body = parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap();
    let primary: Vec<_> = facts
        .primary_bindings
        .iter()
        .map(|(name, range)| {
            let node = body
                .nodes()
                .iter()
                .find(|node| node.role == NodeRole::Pattern && node.range == *range)
                .unwrap();
            (name.clone(), node.id, node.scope)
        })
        .collect();
    let scope = primary[0].2;
    assert!(primary.iter().all(|(_, _, found)| *found == scope));
    // Each independent run stops after a different number of actual retirements.
    // It never retries a fatal resource fault or changes tested source bytes.
    for completed in 0..=facts.expected_tags.len() {
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        frame
            .set_fuel_limit(u64::try_from(completed).unwrap())
            .unwrap();
        let mut ancestry = Vec::new();
        let mut current = Some(scope);
        while let Some(id) = current {
            ancestry.push(id);
            current = body.scopes()[id.0 as usize].parent;
        }
        for id in ancestry.into_iter().rev() {
            frame.enter(id).unwrap();
        }
        // Primary pattern identities retain original declaration order. The
        // pattern engine/closure itself remains pending T050/T053 integration.
        for (_, binding, _) in &primary {
            frame
                .declare_local(*binding, Some(ty), false, &mut store)
                .unwrap();
        }
        for (name, tag) in &facts.initializations {
            let (_, binding, _) = primary.iter().find(|(found, _, _)| found == name).unwrap();
            let cell = frame.binding(*binding).unwrap();
            // LogDrop's written tag is u32. Scalar values expose retirement
            // order without claiming execution of its user destructor or log.
            let ticket = store
                .ticket(ValueRepr::unsigned(ty, *tag).unwrap())
                .unwrap();
            store.initialize(cell, ticket).unwrap();
        }
        let cells: Vec<_> = primary
            .iter()
            .map(|(_, binding, _)| {
                let cell = frame.binding(*binding).unwrap();
                (
                    cell,
                    store.state(cell).unwrap(),
                    store.location_epoch(cell).unwrap(),
                )
            })
            .collect();
        let result = frame.leave(scope, &mut store);
        if completed == facts.expected_tags.len() {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
            assert_eq!(frame.active_scopes().last(), Some(&scope));
        }
        assert_eq!(
            frame.counters.cleanup_elements_completed,
            u64::try_from(completed).unwrap()
        );
        for (cell, original, epoch) in &cells {
            let CellState::Live(value) = original else {
                unreachable!()
            };
            let retired = facts
                .expected_tags
                .iter()
                .position(|tag| *tag == value.payload)
                .unwrap()
                < completed;
            if retired {
                assert_eq!(store.state(*cell).unwrap(), CellState::Dropped);
                assert_eq!(
                    store.location_epoch(*cell).unwrap_err().code,
                    "LRR-E-STALE-HANDLE"
                );
            } else {
                assert_eq!(store.state(*cell).unwrap(), *original);
                assert_eq!(store.location_epoch(*cell).unwrap(), *epoch);
            }
        }
    }
}
