//! Stock-derived aggregate shapes plus LRR transfer/cleanup invariants.
use super::*;

fn reference_from_ticket(store: &GuestStore, ticket: TicketId) -> Result<RefId, Diagnostic> {
    let value = store.peek_ticket(ticket)?;
    store.object_owner(value, ValueOwner::Ticket(ticket))?;
    assert_eq!(value.kind, 9);
    Ok(RefId::from_bits(value.payload))
}

fn summary(store: &GuestStore, cell: CellId) -> (bool, Vec<bool>) {
    let value = store.admit_live_value(cell).unwrap();
    let ObjectStorage::Aggregate(storage) = &store
        .objects
        .get(ObjectId::from_bits(value.payload))
        .unwrap()
        .storage
    else {
        panic!("stock aggregate absent")
    };
    (
        storage.initialized.complete_value(),
        (0..storage.fields.len())
            .map(|index| storage.initialized.field(index).unwrap())
            .collect(),
    )
}

#[test]
fn rustc_tuple_partial_move_cached_summary_refuses_whole_transfer_and_argument_packet() {
    let (mut store, root, field, sibling, lease) = tests::stock_tuple_reference_owner();
    assert_eq!(summary(&store, root), (true, vec![true, true]));
    let original = store.admit_move(root).unwrap();
    let extracted = store
        .read_owning_place(&store.project_tuple(&Place::cell(root), 0).unwrap())
        .unwrap();
    let before = (
        store.state(root).unwrap(),
        store.location_epoch(root).unwrap(),
        store.state(sibling).unwrap(),
        store.budget.handles(),
        store.budget.bytes(),
    );
    assert_eq!(summary(&store, root), (false, vec![false, true]));
    for error in [
        store.admit_move(root).unwrap_err(),
        store.move_from(root).unwrap_err(),
        store
            .seal_arguments(&[root], &[TypeId(original.type_id)])
            .unwrap_err(),
    ] {
        assert_eq!(error.code, "LRR-E-USE-AFTER-MOVE");
    }
    assert_eq!(
        (
            store.state(root).unwrap(),
            store.location_epoch(root).unwrap(),
            store.state(sibling).unwrap(),
            store.budget.handles(),
            store.budget.bytes()
        ),
        before
    );
    assert_eq!(store.state(field).unwrap(), CellState::Moved);
    assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
    // Extracted field stays uniquely owned by its existing ticket.
    assert_eq!(reference_from_ticket(&store, extracted).unwrap(), lease);
}

#[test]
fn rustc_tuple_field_reinitialization_restores_summary_without_reviving_old_epoch() {
    let (mut store, root, field, sibling, lease) = tests::stock_tuple_reference_owner();
    let sibling_before = store.state(sibling).unwrap();
    let ticket = store
        .read_owning_place(&store.project_tuple(&Place::cell(root), 0).unwrap())
        .unwrap();
    let epoch = store.location_epoch(field).unwrap();
    // Supplemental checked-store reinitialization; guest AssignPlace/source
    // writable ancestry and complete original consume/Drop remain pending.
    store.initialize(field, ticket).unwrap();
    assert_eq!(summary(&store, root), (true, vec![true, true]));
    assert_eq!(store.location_epoch(field).unwrap(), epoch);
    assert_eq!(store.state(sibling).unwrap(), sibling_before);
    assert_eq!(store.reference_from_cell(field).unwrap(), lease);
    let whole = store.move_from(root).unwrap();
    assert_eq!(store.aggregate_field(whole, 0).unwrap(), field);
    assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
}

#[test]
fn rustc_partial_tuple_cleanup_skips_moved_field_and_keeps_its_external_owner() {
    let (mut store, root, field, sibling, lease) = tests::stock_tuple_reference_owner();
    let extracted = store
        .read_owning_place(&store.project_tuple(&Place::cell(root), 0).unwrap())
        .unwrap();
    let root_ticket = store.begin_drop(root).unwrap();
    assert!(matches!(
        store.aggregate_cleanup_step(root_ticket).unwrap(),
        AggregateCleanupStep::Skipped
    ));
    assert!(store.cells.get(field).unwrap().retired);
    let AggregateCleanupStep::Field(sibling_ticket) =
        store.aggregate_cleanup_step(root_ticket).unwrap()
    else {
        panic!("remaining sibling cleanup obligation absent")
    };
    assert!(store.cells.get(sibling).unwrap().retired);
    assert!(matches!(
        store.aggregate_cleanup_step(root_ticket).unwrap(),
        AggregateCleanupStep::Complete
    ));
    assert_eq!(reference_from_ticket(&store, extracted).unwrap(), lease);
    assert_eq!(store.read_reference(lease).unwrap(), ValueRepr::unit());
    // This schedules the exact source-bound WithDrop sibling, not its body.
    // No selected guest destructor transcript or full fixture pass is claimed.
    assert_eq!(store.aggregate_variant(sibling_ticket).unwrap(), Some(0));
    store.discard(root_ticket).unwrap();
    assert_eq!(reference_from_ticket(&store, extracted).unwrap(), lease);
}

#[test]
fn rustc_nested_tuple_initialization_summary_propagates_without_tree_rescan() {
    let source =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/tuple/index-float.rs");
    let depth = crate::syntax::place_fixture_facts::nested_unit_tuple_depth(source);
    let mut store = GuestStore::new(1, 20_000, 16 * 1024 * 1024).unwrap();
    // Stock-derived depth and a stress multiple use the actual existing
    // constructor/field-owner graph. Raw leaf transfer is a supplemental LRR
    // state transition; ordinary source reads of Unit use Copy instead.
    let (root, leaf) = tests::nested_unit_cells(&mut store, depth * 256, true);
    assert!(summary(&store, root).0);
    let moved = store.move_from(leaf).unwrap();
    assert!(!summary(&store, root).0);
    assert_eq!(
        store.move_from(root).unwrap_err().code,
        "LRR-E-USE-AFTER-MOVE"
    );
    let epoch = store.location_epoch(leaf).unwrap();
    store.initialize(leaf, moved).unwrap();
    assert!(summary(&store, root).0);
    assert_eq!(store.location_epoch(leaf).unwrap(), epoch);
    store.move_from(root).unwrap();
}

#[test]
fn rustc_tuple_argument_owner_and_field_overlap_refuses_before_packet_or_summary_change() {
    let (mut store, root, field, sibling, _) = tests::stock_tuple_reference_owner();
    let root_value = store.admit_move(root).unwrap();
    let field_value = store.admit_move(field).unwrap();
    let before = (
        store.state(root).unwrap(),
        store.state(field).unwrap(),
        store.state(sibling).unwrap(),
        store.budget.handles(),
        store.budget.bytes(),
    );
    for cells in [[root, field], [field, root]] {
        let types = cells.map(|cell| {
            if cell == root {
                TypeId(root_value.type_id)
            } else {
                TypeId(field_value.type_id)
            }
        });
        assert_eq!(
            store.seal_arguments(&cells, &types).unwrap_err().code,
            "LRR-E-OWNERSHIP"
        );
        assert_eq!(summary(&store, root), (true, vec![true, true]));
        assert_eq!(
            (
                store.state(root).unwrap(),
                store.state(field).unwrap(),
                store.state(sibling).unwrap(),
                store.budget.handles(),
                store.budget.bytes()
            ),
            before
        );
    }
}

#[test]
fn rustc_tuple_field_quota_refusal_preserves_cached_initialization_and_owners() {
    let (mut store, root, field, sibling, _) = tests::stock_tuple_reference_owner();
    let place = store.project_tuple(&Place::cell(root), 0).unwrap();
    while store.budget.handles() < 1000 {
        store.ticket(ValueRepr::unit()).unwrap();
    }
    let before = (
        store.state(root).unwrap(),
        store.state(field).unwrap(),
        store.state(sibling).unwrap(),
        store.budget.handles(),
        store.budget.bytes(),
    );
    let PlaceFailure::RuntimeFault(error) = store.read_owning_place(&place).unwrap_err() else {
        panic!("quota did not produce a runtime fault")
    };
    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(summary(&store, root), (true, vec![true, true]));
    assert_eq!(
        (
            store.state(root).unwrap(),
            store.state(field).unwrap(),
            store.state(sibling).unwrap(),
            store.budget.handles(),
            store.budget.bytes()
        ),
        before
    );
}

#[test]
fn rustc_array_initialization_word_boundary_stress_restores_all_live_fields() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/vec-fixed-length.rs"
    );
    let values = crate::syntax::place_fixture_facts::array_binding(original);
    // LRR-specific 17x width expansion of the stock four-element array crosses
    // a bitset word boundary. This is not full unchanged program execution.
    let count = values.len().checked_mul(17).unwrap();
    assert!(count > 64);
    let mut store = GuestStore::new(1, 10_000, 1024 * 1024).unwrap();
    let ty = store
        .intern_type(TypeKey::Array {
            element: TypeId(8),
            len: u32::try_from(count).unwrap(),
        })
        .unwrap();
    let guard = store.begin_construction(&vec![TypeId(8); count]).unwrap();
    for (index, value) in values.iter().cycle().take(count).enumerate() {
        let ticket = store
            .ticket(ValueRepr::signed(TypeId(8), *value).unwrap())
            .unwrap();
        store
            .construction_push(guard, u32::try_from(index).unwrap(), ticket)
            .unwrap();
    }
    let ticket = store.commit_construction(guard, ty).unwrap();
    let fields = [
        store.aggregate_field(ticket, 63).unwrap(),
        store.aggregate_field(ticket, 64).unwrap(),
    ];
    let frame = store.new_frame_anchor().unwrap();
    let root = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(root, ticket).unwrap();
    let first = store.move_from(fields[0]).unwrap();
    let second = store.move_from(fields[1]).unwrap();
    assert!(!summary(&store, root).0);
    store.initialize(fields[0], first).unwrap();
    assert!(!summary(&store, root).0);
    store.initialize(fields[1], second).unwrap();
    assert!(summary(&store, root).0);
    store.move_from(root).unwrap();
}
