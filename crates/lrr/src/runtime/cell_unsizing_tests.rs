//! Slice references as element windows of actual array locations.
use super::*;
use crate::runtime::place::{SliceRange, checked_slice_range};

const I32: TypeId = TypeId(6);
const USIZE: TypeId = TypeId(13);

/// A live `[i32; N]` local and its array descriptor.
fn array(store: &mut GuestStore, values: &[i64], writable: bool) -> (CellId, TypeId) {
    let ty = store
        .intern_type(TypeKey::Array {
            element: I32,
            len: values.len() as u32,
        })
        .unwrap();
    let fields = vec![I32; values.len()];
    let guard = store.begin_construction(&fields).unwrap();
    for (index, value) in values.iter().enumerate() {
        let ticket = store.ticket(ValueRepr::signed(I32, *value).unwrap()).unwrap();
        store.construction_push(guard, index as u32, ticket).unwrap();
    }
    let ticket = store.commit_construction(guard, ty).unwrap();
    let frame = store.new_frame_anchor().unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            writable,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    (cell, ty)
}
fn slice_ref(store: &GuestStore, mutable: bool) -> TypeId {
    let slice = store.find_type(&TypeKey::Slice { element: I32 }).unwrap();
    store
        .find_type(&TypeKey::Ref {
            mutable,
            target: slice,
        })
        .unwrap()
}
fn index(i: u64) -> ValueRepr {
    ValueRepr::unsigned(USIZE, i).unwrap()
}
fn element(store: &GuestStore, base: &Place, i: u64) -> Result<i64, PlaceFailure> {
    let place = store.project_index(base, index(i))?;
    Ok(store.read_place(&place)?.payload as i64)
}
fn reference(store: &GuestStore, ticket: TicketId) -> RefId {
    RefId::from_bits(store.peek_ticket(ticket).unwrap().payload)
}
const FULL: SliceRange = SliceRange {
    start: 0,
    end: None,
    inclusive: false,
};

#[test]
fn unsizing_coercion_views_the_same_array_location_as_its_element_window() {
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (cell, _) = array(&mut store, &[10, 20, 30, 40], false);
    // `&a` at a `&[i32]` coercion site.
    let ticket = store
        .borrow_ticket(&Place::cell(cell), Access::Shared)
        .unwrap();
    let expected = slice_ref(&store, false);
    store.coerce_unsized_ticket(ticket, expected).unwrap();
    let value = store.peek_ticket(ticket).unwrap();
    assert_eq!(TypeId(value.type_id), expected);
    assert_eq!(store.referent_len(value).unwrap(), 4);
    let id = reference(&store, ticket);
    assert_eq!(store.reference_window(id).unwrap(), Some((0, 4)));
    let deref = Place::deref(id);
    assert_eq!(element(&store, &deref, 1).unwrap(), 20);
    assert_eq!(element(&store, &deref, 3).unwrap(), 40);
    // rustc: `index out of bounds: the len is 4 but the index is 4`.
    let Err(PlaceFailure::GuestPanic(panic)) = element(&store, &deref, 4) else {
        panic!("slice index must bounds-check its window")
    };
    assert_eq!(
        panic.message(),
        "index out of bounds: the len is 4 but the index is 4"
    );
    // The unsized `[i32]` place is borrowed or indexed, never read.
    let PlaceFailure::RuntimeFault(error) = store.read_place(&deref).unwrap_err() else {
        panic!("unsized read is a fault")
    };
    assert_eq!(error.code, "LRR-E-OP-TYPE");
    assert_eq!(
        store.place_type_id(&deref).unwrap(),
        store.find_type(&TypeKey::Slice { element: I32 }).unwrap()
    );
}

#[test]
fn unsizing_never_adds_mutability_or_changes_the_element_type() {
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (cell, _) = array(&mut store, &[1, 2], true);
    let shared = store
        .borrow_ticket(&Place::cell(cell), Access::Shared)
        .unwrap();
    let value = store.peek_ticket(shared).unwrap();
    assert!(
        store
            .unsizing_coercion(value, slice_ref(&store, true))
            .unwrap()
            .is_none()
    );
    let u8_slice = store
        .intern_type(TypeKey::Slice {
            element: TypeId(9),
        })
        .unwrap();
    let u8_ref = store
        .intern_type(TypeKey::Ref {
            mutable: false,
            target: u8_slice,
        })
        .unwrap();
    assert!(store.unsizing_coercion(value, u8_ref).unwrap().is_none());
    assert!(
        store
            .unsizing_coercion(value, slice_ref(&store, false))
            .unwrap()
            .is_some()
    );
}

#[test]
fn mutable_slice_assignment_writes_the_borrowed_array_element() {
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (cell, _) = array(&mut store, &[1, 2, 3], true);
    let ticket = store
        .borrow_ticket(&Place::cell(cell), Access::Mutable)
        .unwrap();
    let old = reference(&store, ticket);
    store
        .coerce_unsized_ticket(ticket, slice_ref(&store, true))
        .unwrap();
    // The consumed `&mut [i32; 3]` lease is destroyed, not duplicated.
    assert_eq!(
        store.reference_window(old).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    let id = reference(&store, ticket);
    let target = store.project_index(&Place::deref(id), index(0)).unwrap();
    let five = store.ticket(ValueRepr::signed(I32, 5).unwrap()).unwrap();
    let previous = store.replace_place(&target, five).unwrap();
    store.discard(previous).unwrap();
    store.discard(ticket).unwrap();
    assert_eq!(element(&store, &Place::cell(cell), 0).unwrap(), 5);
}

#[test]
fn range_projection_borrows_a_checked_subwindow() {
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (cell, _) = array(&mut store, &[10, 20, 30, 40, 50], false);
    // `&a[1..4]`
    let window = store
        .project_range(
            &Place::cell(cell),
            SliceRange {
                start: 1,
                end: Some(4),
                inclusive: false,
            },
        )
        .unwrap();
    let ticket = store.borrow_ticket(&window, Access::Shared).unwrap();
    assert_eq!(
        TypeId(store.peek_ticket(ticket).unwrap().type_id),
        slice_ref(&store, false)
    );
    let id = reference(&store, ticket);
    assert_eq!(store.reference_window(id).unwrap(), Some((1, 3)));
    let deref = Place::deref(id);
    assert_eq!(element(&store, &deref, 0).unwrap(), 20);
    assert_eq!(element(&store, &deref, 2).unwrap(), 40);
    assert!(matches!(
        element(&store, &deref, 3),
        Err(PlaceFailure::GuestPanic(PlacePanic::IndexOutOfBounds {
            len: 3,
            index: 3
        }))
    ));
    // `&s[1..]` of that window, then `s[..]` of the whole array.
    let nested = store
        .project_range(
            &deref,
            SliceRange {
                start: 1,
                end: None,
                inclusive: false,
            },
        )
        .unwrap();
    let nested = store.borrow_ticket(&nested, Access::Shared).unwrap();
    let nested = reference(&store, nested);
    assert_eq!(store.reference_window(nested).unwrap(), Some((2, 2)));
    assert_eq!(element(&store, &Place::deref(nested), 1).unwrap(), 40);
    let full = store.project_range(&Place::cell(cell), FULL).unwrap();
    let full = store.borrow_ticket(&full, Access::Shared).unwrap();
    let full = store.peek_ticket(full).unwrap();
    assert_eq!(store.referent_len(full).unwrap(), 5);
    // A range past the window panics with rustc's message.
    let Err(PlaceFailure::GuestPanic(panic)) = store.project_range(
        &deref,
        SliceRange {
            start: 0,
            end: Some(4),
            inclusive: false,
        },
    ) else {
        panic!("range end must be checked against the window")
    };
    assert_eq!(
        panic.message(),
        "range end index 4 out of range for slice of length 3"
    );
}

#[test]
fn slice_lease_is_stale_after_the_array_location_moves() {
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (cell, _) = array(&mut store, &[7, 8], false);
    let ticket = store
        .borrow_ticket(&Place::cell(cell), Access::Shared)
        .unwrap();
    store
        .coerce_unsized_ticket(ticket, slice_ref(&store, false))
        .unwrap();
    let id = reference(&store, ticket);
    let moved = store.move_from(cell).unwrap();
    let PlaceFailure::RuntimeFault(error) = element(&store, &Place::deref(id), 0).unwrap_err()
    else {
        panic!("stale lease is a fault")
    };
    assert_eq!(error.code, "LRR-E-STALE-HANDLE");
    store.discard(moved).ok();
}

#[test]
fn slice_range_bounds_follow_rustc_slice_index_fail() {
    let range = |start, end, inclusive| SliceRange {
        start,
        end,
        inclusive,
    };
    let message = |r, len| checked_slice_range(r, len).unwrap_err().message();
    assert_eq!(checked_slice_range(range(1, Some(3), false), 4), Ok((1, 3)));
    assert_eq!(checked_slice_range(range(1, Some(2), true), 4), Ok((1, 3)));
    assert_eq!(checked_slice_range(range(4, None, false), 4), Ok((4, 4)));
    assert_eq!(checked_slice_range(range(0, None, false), 0), Ok((0, 0)));
    assert_eq!(
        message(range(0, Some(5), false), 4),
        "range end index 5 out of range for slice of length 4"
    );
    assert_eq!(
        message(range(3, Some(2), false), 4),
        "slice index starts at 3 but ends at 2"
    );
    assert_eq!(
        message(range(5, None, false), 4),
        "range start index 5 out of range for slice of length 4"
    );
    assert_eq!(
        message(range(6, Some(5), false), 4),
        "range start index 6 out of range for slice of length 4"
    );
    // `..=4` on four elements: rustc reports the inclusive end itself.
    assert_eq!(
        message(range(0, Some(4), true), 4),
        "range end index 4 out of range for slice of length 4"
    );
    assert_eq!(
        message(range(3, Some(1), true), 4),
        "slice index starts at 3 but ends at 2"
    );
}
