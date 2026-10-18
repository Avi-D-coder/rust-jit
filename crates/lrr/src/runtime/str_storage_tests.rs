//! Storage/lease authority controls outside unchanged upstream originals. These
//! do not execute Rust literals, infer source types or accept fixture refusals.
use super::*;
use crate::runtime::shape::{CopyStatus, ShapeCategory};
use ra_ap_syntax::{AstNode, AstToken, Edition, ast};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-borrowed-str.rs"
);
const NULS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/str/nul-char-equivalence.rs"
);
fn first_string(source: &str) -> String {
    ast::SourceFile::parse(source, Edition::Edition2015)
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .find_map(ast::String::cast)
        .unwrap()
        .value()
        .unwrap()
        .into_owned()
}
fn lease(store: &GuestStore, ticket: TicketId) -> (RefId, BufferLease) {
    let id = RefId::from_bits(store.peek_ticket(ticket).unwrap().payload);
    match &store.references.get(id).unwrap().lease {
        ReferenceLease::Buffer(buffer) => (id, *buffer),
        ReferenceLease::Location(_) | ReferenceLease::Elements(_) | ReferenceLease::Detached(_) => {
            panic!("expected actual buffer lease")
        }
    }
}
#[test]
fn original_literal_bytes_have_unsized_shared_backing_and_deduplicate() {
    let text = first_string(ORIGINAL);
    let mut store = GuestStore::for_epoch(1).unwrap();
    let first = store.immutable_str_ticket(text.as_bytes()).unwrap();
    let before_handles = store.budget().handles();
    let again = store.immutable_str_ticket(text.as_bytes()).unwrap();
    assert_eq!(store.budget().handles() - before_handles, 2); // lease + ticket only
    assert_eq!(store.strings.rows.len(), 1);
    assert_eq!(
        lease(&store, first).1.backing,
        lease(&store, again).1.backing
    );
    assert_eq!(store.str_reference_contents(first).unwrap(), text);
    let value = store.peek_ticket(first).unwrap();
    assert!(matches!(
        store
            .type_descriptor(TypeId(value.type_id))
            .unwrap()
            .key
            .as_ref(),
        TypeKey::Ref {
            mutable: false,
            target: TypeId(16)
        }
    ));
    let str_descriptor = store.type_descriptor(TypeId(16)).unwrap();
    assert_eq!(str_descriptor.shape, ShapeCategory::Unsized);
    assert_eq!(str_descriptor.copy, CopyStatus::NotCopy);
    assert_eq!(
        store
            .references
            .get(lease(&store, first).0)
            .unwrap()
            .owned_bytes()
            .unwrap(),
        0
    );
    assert!(
        store
            .objects
            .get(Handle::from_bits((1u64 << 32) | 1))
            .is_err()
    );
}
#[test]
fn original_nul_contents_and_multibyte_boundaries_are_checked_without_loss() {
    let text = first_string(NULS);
    let mut store = GuestStore::for_epoch(1).unwrap();
    let empty_backing = store.immutable_str_ticket(b"").unwrap();
    assert_eq!(store.str_reference_contents(empty_backing).unwrap(), "");
    let nuls = store.immutable_str_ticket(text.as_bytes()).unwrap();
    assert_eq!(
        store.str_reference_contents(nuls).unwrap().as_bytes(),
        text.as_bytes()
    );
    assert!(text.as_bytes().contains(&0));
    // LRR byte-range authority payload, not a replacement language program.
    let utf8 = "aé🦀z";
    let middle = store
        .immutable_str_range_ticket(utf8.as_bytes(), 1..7)
        .unwrap();
    assert_eq!(store.str_reference_contents(middle).unwrap(), "é🦀");
    let empty = store
        .immutable_str_range_ticket(utf8.as_bytes(), 7..7)
        .unwrap();
    assert_eq!(store.str_reference_contents(empty).unwrap(), "");
    for range in [2..7, 1..6, 7..1, 0..usize::MAX] {
        let before = (
            store.budget().handles(),
            store.budget().bytes(),
            store.strings.rows.len(),
        );
        assert_eq!(
            store
                .immutable_str_range_ticket(utf8.as_bytes(), range)
                .unwrap_err()
                .code,
            "LRR-E-STR-RANGE"
        );
        assert_eq!(
            (
                store.budget().handles(),
                store.budget().bytes(),
                store.strings.rows.len()
            ),
            before
        );
    }
    let before = (store.budget().handles(), store.budget().bytes());
    assert_eq!(
        store
            .immutable_str_ticket(&[0xff, 0xc0, 0x80])
            .unwrap_err()
            .code,
        "LRR-E-STR-UTF8"
    );
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
}
#[test]
fn shared_copy_outlives_its_original_frame_cell_without_owning_str() {
    let text = first_string(ORIGINAL);
    let mut store = GuestStore::for_epoch(1).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let ticket = store.immutable_str_ticket(text.as_bytes()).unwrap();
    let ty = TypeId(store.peek_ticket(ticket).unwrap().type_id);
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            false,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    let copied = store.copy_from(cell).unwrap();
    let cleanup = store.begin_drop(cell).unwrap();
    store.discard(cleanup).unwrap();
    store.retire_cell(cell).unwrap();
    assert_eq!(store.str_reference_contents(copied).unwrap(), text);
    // Buffer call/return and deref-as-owning-cell integration is deliberately
    // held. This is an authority negative, not a valid Rust runtime-contract pass.
    let id = lease(&store, copied).0;
    assert_eq!(
        store.read_reference(id).unwrap_err().code,
        "LRR-E-UNSUPPORTED"
    );
    assert!(
        store
            .borrow_ticket(&Place::deref(id), Access::Mutable)
            .is_err()
    );
    assert!(
        store
            .ticket(ValueRepr {
                kind: 1,
                type_id: 16,
                payload: 0
            })
            .is_err()
    );
    assert!(
        store
            .ticket(ValueRepr {
                kind: 9,
                type_id: 16,
                payload: id.bits()
            })
            .is_err()
    );
}
#[test]
fn backing_and_reference_generations_and_store_session_are_not_forgeable() {
    let text = first_string(ORIGINAL);
    let mut first = GuestStore::for_epoch(1).unwrap();
    let ticket = first.immutable_str_ticket(text.as_bytes()).unwrap();
    let (id, buffer) = lease(&first, ticket);
    let mut reset = GuestStore::for_epoch(2).unwrap();
    reset.immutable_str_ticket(text.as_bytes()).unwrap();
    assert_eq!(
        reset.strings.resolve(buffer).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(
        reset.reference_ticket(id).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    let mut same_epoch = GuestStore::for_epoch(1).unwrap();
    let second = same_epoch.immutable_str_ticket(text.as_bytes()).unwrap();
    let (_, second_buffer) = lease(&same_epoch, second);
    assert_eq!(
        buffer.backing.handle.bits(),
        second_buffer.backing.handle.bits()
    );
    assert_ne!(buffer.backing, second_buffer.backing);
    assert_eq!(
        same_epoch.strings.resolve(buffer).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    for bits in [0, 1u64 << 32, (1u64 << 32) | 1000] {
        let mut bad = buffer;
        bad.backing.handle = Handle::from_bits(bits);
        assert_eq!(
            first.strings.resolve(bad).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
    }
}
#[test]
fn forged_mutability_target_or_range_does_not_grant_buffer_access() {
    let mut store = GuestStore::for_epoch(1).unwrap();
    let ticket = store.immutable_str_ticket("é".as_bytes()).unwrap();
    let (id, buffer) = lease(&store, ticket);
    store.references.get_mut(id).unwrap().access = Access::Mutable;
    assert_eq!(
        store.reference_ticket(id).unwrap_err().code,
        "LRR-E-REFERENCE-ACCESS"
    );
    store.references.get_mut(id).unwrap().access = Access::Shared;
    store.references.get_mut(id).unwrap().target = TypeId(9);
    assert_eq!(
        store.reference_ticket(id).unwrap_err().code,
        "LRR-E-REFERENCE-ACCESS"
    );
    store.references.get_mut(id).unwrap().target = TypeId(16);
    let ReferenceLease::Buffer(current) = &mut store.references.get_mut(id).unwrap().lease else {
        panic!()
    };
    current.start = 1;
    assert_eq!(
        store.str_reference_contents(ticket).unwrap_err().code,
        "LRR-E-STR-RANGE"
    );
    store.references.get_mut(id).unwrap().lease = ReferenceLease::Buffer(buffer);
    assert_eq!(store.str_reference_contents(ticket).unwrap(), "é");
    let owned = store.string("é").unwrap();
    assert!(store.str_reference_contents(owned).is_err());
    assert!(store.string_contents(ticket).is_err());
}
