//! Physical storage supplements, not source/type authority or original execution.
use super::*;
use crate::{
    ir::{constant_data::wire, op::PrimitiveType},
    runtime::{
        place::{Place, PlaceRoot, Projection},
        reference::Access,
    },
};
const NRVO: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-eval/nrvo.rs");
fn data(unit: bool, outer: Option<u64>) -> (ConstValue, ConstantWireType) {
    let primitive = if unit {
        PrimitiveType::Unit
    } else {
        PrimitiveType::U8
    };
    let mut wire = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(primitive)),
        length: 1024,
    };
    let mut bytes = b"LRRCNST\0".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    if let Some(count) = outer {
        wire = ConstantWireType::Array {
            element: Box::new(wire),
            length: count,
        };
        bytes.push(1);
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    bytes.push(1);
    bytes.extend_from_slice(&1024u64.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&(if unit { 0u32 } else { 9u32 }).to_le_bytes());
    let value = if unit {
        ValueRepr::unit()
    } else {
        ValueRepr::unsigned(TypeId(9), 0).unwrap()
    };
    for _ in 0..1024 * outer.unwrap_or(1) {
        bytes.extend_from_slice(&value.kind.to_le_bytes());
        bytes.extend_from_slice(&value.type_id.to_le_bytes());
        bytes.extend_from_slice(&value.payload.to_le_bytes());
    }
    (wire::decode(&bytes, &wire, 1 << 20).unwrap(), wire)
}
fn setup(unit: bool, outer: Option<u64>) -> (GuestStore, ConstantChunkActivation, TypeId) {
    let mut store = GuestStore::new(1, 30000, 32 << 20).unwrap();
    let (value, wire) = data(unit, outer);
    let child = store
        .intern_type(TypeKey::Array {
            element: if unit { TypeId(1) } else { TypeId(9) },
            len: 1024,
        })
        .unwrap();
    let ty = if let Some(count) = outer {
        store
            .intern_type(TypeKey::Array {
                element: child,
                len: count as u32,
            })
            .unwrap()
    } else {
        child
    };
    let plan = store
        .prepare_constant_instantiation(&value, &wire, ty)
        .unwrap();
    let act = store.prepare_constant_chunks(plan, 0).unwrap();
    (store, act, ty)
}

#[test]
fn store_session_chunk_cursor_refuses_foreign_progress_and_abort_without_mutation() {
    let (mut a, mut activation, _) = setup(false, None);
    let (mut b, _, _) = setup(false, None);
    let mut fuel = RunFuel::new(10000).unwrap();
    let before = (
        b.budget().handles(),
        b.budget().bytes(),
        activation.fields_charged(),
    );
    let ConstantChunkPoll::Failed(error) = b.advance_constant_chunks(&mut activation, &mut fuel)
    else {
        panic!("foreign cursor advanced")
    };
    assert_eq!(error.code, "LRR-E-STALE-HANDLE");
    assert_eq!(
        b.abort_constant_chunks(&mut activation, resource())
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(fuel.snapshot().used, 0);
    assert_eq!(
        before,
        (
            b.budget().handles(),
            b.budget().bytes(),
            activation.fields_charged()
        )
    );
    assert!(!activation.cleaning());
    assert!(!activation.finished_for_frame());
    let (pending, _) = complete(&mut a, &mut activation, &mut fuel);
    a.abort_pending_constant(pending).unwrap();
}
fn complete(
    store: &mut GuestStore,
    a: &mut ConstantChunkActivation,
    fuel: &mut RunFuel,
) -> (PendingConstantResult, usize) {
    let mut dispatches = 0;
    for _ in 0..20000 {
        match store.advance_constant_chunks(a, fuel) {
            ConstantChunkPoll::Dispatch {
                steps,
                bytes,
                cleaning,
            } => {
                assert!(steps <= 256 && bytes <= 64 << 10);
                assert!(!cleaning);
                dispatches += 1;
            }
            ConstantChunkPoll::Ready(pending) => return (pending, dispatches),
            ConstantChunkPoll::Failed(e) => panic!("{}: {}", e.code, e.message),
        }
    }
    panic!("bounded activation did not complete")
}
fn failure(
    store: &mut GuestStore,
    a: &mut ConstantChunkActivation,
    fuel: &mut RunFuel,
) -> Diagnostic {
    let mut cleanup_dispatches = 0;
    for _ in 0..20000 {
        match store.advance_constant_chunks(a, fuel) {
            ConstantChunkPoll::Dispatch {
                steps,
                bytes,
                cleaning,
            } => {
                assert!(steps <= 256 && bytes <= 64 << 10);
                assert!(cleaning);
                cleanup_dispatches += 1;
            }
            ConstantChunkPoll::Failed(e) => {
                assert!(a.finished_for_frame());
                assert!(cleanup_dispatches > 0);
                return e;
            }
            ConstantChunkPoll::Ready(_) => panic!("interruption became a value"),
        }
    }
    panic!("private rollback did not drain")
}
fn retired(store: &GuestStore) {
    for i in 1..=store.budget.handles() {
        let bits = (1u64 << 32) | u64::from(i);
        assert!(
            store
                .constructions
                .get(Handle::<ConstructionRecord>::from_bits(bits))
                .is_err()
        );
        assert!(
            store
                .objects
                .get(Handle::<ObjectRecord>::from_bits(bits))
                .is_err()
        );
        assert!(store.cells.get(Handle::<Cell>::from_bits(bits)).is_err());
        assert!(
            store
                .tickets
                .get(Handle::<TransferTicket>::from_bits(bits))
                .is_err()
        );
    }
}
#[test]
fn nrvo_1024_shape_physical_chunks_keep_mutation_cells_and_immutable_initial_image_separate() {
    assert!(NRVO.contains("const BUF: [u8; 1024] = nrvo();"));
    let (mut store, mut a, ty) = setup(false, None);
    let mut fuel = RunFuel::new(2000).unwrap();
    fuel.charge().unwrap();
    let (mut pending, dispatches) = complete(&mut store, &mut a, &mut fuel);
    assert!(dispatches > 1);
    assert_eq!(a.fields_charged(), 1024);
    assert_eq!(fuel.snapshot().used, 1025);
    let frame = store.new_frame_anchor().unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    assert!(store.commit_pending_constant(&mut pending, cell).is_ok());
    for (index, value) in [(33, 3), (444, 4)] {
        let place = Place {
            root: PlaceRoot::Cell(cell),
            projections: Box::new([Projection::Index(index)]),
        };
        let reference = store.borrow(&place, Access::Mutable).unwrap();
        let ticket = store
            .ticket(ValueRepr::unsigned(TypeId(9), value).unwrap())
            .unwrap();
        let old = store.replace_reference(reference, ticket).unwrap();
        store.discard(old).unwrap();
        assert_eq!(store.read_reference(reference).unwrap().payload, value);
    }
    let (image, _) = data(false, None);
    assert_eq!(image.elements().unwrap()[33].scalar().unwrap().payload, 0);
    // The original constfn borrows/init/index/assert source program is unexecuted.
}
#[test]
fn unit_constant_chunks_charge_all_1024_zero_size_fields_once() {
    let (mut s, mut a, _) = setup(true, None);
    let mut f = RunFuel::new(2000).unwrap();
    f.charge().unwrap();
    let (p, d) = complete(&mut s, &mut a, &mut f);
    assert!(d > 1);
    assert_eq!(a.fields_charged(), 1024);
    assert_eq!(f.snapshot().used, 1025);
    s.cleanup_ready_constant_chunks(
        &mut a,
        p,
        Diagnostic::new("LRR-E-CHUNK-CONTROL", "explicit private abort".into()),
    )
    .unwrap();
    let error = failure(&mut s, &mut a, &mut f);
    assert_eq!(error.code, "LRR-E-CHUNK-CONTROL");
    retired(&s);
}
#[test]
fn nested_constant_chunks_preserve_parent_child_charge_and_commit_cursors() {
    let (mut s, mut a, ty) = setup(true, Some(2));
    let mut f = RunFuel::new(3000).unwrap();
    f.charge().unwrap();
    let (p, d) = complete(&mut s, &mut a, &mut f);
    assert!(d > 2);
    assert_eq!(a.fields_charged(), 2050);
    assert_eq!(f.snapshot().used, 2051);
    assert_eq!(s.validate_pending_constant(&p).unwrap().type_id, ty.0);
    s.cleanup_ready_constant_chunks(
        &mut a,
        p,
        Diagnostic::new("LRR-E-CHUNK-CONTROL", "explicit private abort".into()),
    )
    .unwrap();
    let error = failure(&mut s, &mut a, &mut f);
    assert_eq!(error.code, "LRR-E-CHUNK-CONTROL");
    retired(&s);
}
#[test]
fn cancellation_in_constant_build_drains_private_rollback_in_bounded_dispatches() {
    let (mut s, mut a, _) = setup(false, None);
    let mut f = RunFuel::new(2000).unwrap();
    f.charge().unwrap();
    // Cancel with enough genuinely constructed private fields to require more
    // than one bounded cleanup chunk; a small prefix can legitimately drain at once.
    for _ in 0..32 {
        assert!(matches!(
            s.advance_constant_chunks(&mut a, &mut f),
            ConstantChunkPoll::Dispatch {
                cleaning: false,
                ..
            }
        ));
        if a.fields_charged() >= 512 {
            break;
        }
    }
    assert!(a.fields_charged() >= 512 && a.fields_charged() < 1024);
    f.cancellation().cancel();
    let used = f.snapshot().used;
    let e = failure(&mut s, &mut a, &mut f);
    assert_eq!(e.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(used, f.snapshot().used);
    retired(&s);
}
#[test]
fn cancellation_in_constant_commit_retires_partial_cells_guard_and_constructing_object() {
    let (mut s, mut a, _) = setup(false, None);
    let mut f = RunFuel::new(2000).unwrap();
    f.charge().unwrap();
    let mut reached = false;
    for _ in 0..20000 {
        match s.advance_constant_chunks(&mut a, &mut f) {
            ConstantChunkPoll::Dispatch {
                steps,
                bytes,
                cleaning: false,
            } => {
                assert!(steps <= 256 && bytes <= 64 << 10);
                if a.commit
                    .as_ref()
                    .is_some_and(|c| c.next > 0 && c.next < c.count)
                {
                    reached = true;
                    break;
                }
            }
            _ => panic!("missed partial commit"),
        }
    }
    assert!(reached);
    assert_eq!(f.snapshot().used, 1025);
    f.cancellation().cancel();
    let e = failure(&mut s, &mut a, &mut f);
    assert_eq!(e.code, "LRR-E-RUNTIME-RESOURCE");
    assert_eq!(f.snapshot().used, 1025);
    retired(&s);
}
#[test]
fn exhausted_constant_field_is_not_recharged_during_owned_cleanup() {
    let (mut s, mut a, _) = setup(false, None);
    let mut f = RunFuel::new(50).unwrap();
    f.charge().unwrap();
    let mut failed = false;
    for _ in 0..20000 {
        match s.advance_constant_chunks(&mut a, &mut f) {
            ConstantChunkPoll::Dispatch { steps, bytes, .. } => {
                assert!(steps <= 256 && bytes <= 64 << 10)
            }
            ConstantChunkPoll::Failed(e) => {
                assert_eq!(e.code, "LRR-E-RUNTIME-RESOURCE");
                failed = true;
                break;
            }
            ConstantChunkPoll::Ready(_) => panic!("exhaustion became success"),
        }
    }
    assert!(failed && a.finished_for_frame());
    assert_eq!(a.fields_charged(), 49);
    assert_eq!(f.snapshot().used, 50);
    retired(&s);
}
#[test]
fn complete_constant_reservation_includes_extra_destination_and_refuses_before_entry() {
    let (mut s, _a, ty) = setup(false, None);
    let (value, wire) = data(false, None);
    let plan = s.prepare_constant_instantiation(&value, &wire, ty).unwrap();
    let before = s.budget.handles();
    s.prepare_constant_chunks(plan, 1).unwrap();
    assert_eq!(before, s.budget.handles());
    let mut small = GuestStore::new(1, 2051, 32 << 20).unwrap();
    let ty = small
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 1024,
        })
        .unwrap();
    let plan = small
        .prepare_constant_instantiation(&value, &wire, ty)
        .unwrap();
    assert!(small.prepare_constant_chunks(plan, 1).is_err());
    assert_eq!(small.budget.handles(), 0);
}
#[test]
fn old_large_one_shot_constant_kernel_refuses_instead_of_hiding_dispatch_loop() {
    let (mut s, _a, ty) = setup(false, None);
    let (v, w) = data(false, None);
    let p = s.prepare_constant_instantiation(&v, &w, ty).unwrap();
    let before = s.budget.handles();
    let mut f = RunFuel::new(2000).unwrap();
    assert!(s.instantiate_constant(p, &mut f).is_err());
    assert_eq!(f.snapshot().used, 1);
    assert_eq!(before, s.budget.handles());
}
