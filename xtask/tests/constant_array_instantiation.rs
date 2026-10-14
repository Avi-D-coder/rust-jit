//! Physical T054 adapters; these do not execute complete upstream fixtures.
use lrr::{
    frontend::constants::{
        ConstValue,
        wire::{self, ConstantWireType},
    },
    ir::op::PrimitiveType,
    runtime::{
        cell::{AggregateCleanupStep, CellId, CellOwner, GuestStore, StorageContract},
        place::{Place, PlaceRoot, Projection},
        reference::Access,
        shape::{TypeId, TypeKey},
        value::ValueRepr,
    },
    vm::fuel::RunFuel,
};
fn shape(bytes: &mut Vec<u8>, ty: &ConstantWireType) {
    match ty {
        ConstantWireType::Scalar(p) => {
            let tag: u32 = match p {
                PrimitiveType::Unit => 0,
                PrimitiveType::I32 => 5,
                PrimitiveType::U8 => 9,
                _ => panic!("fixture scalar"),
            };
            bytes.push(0);
            bytes.extend_from_slice(&tag.to_le_bytes());
        }
        ConstantWireType::Array { element, length } => {
            bytes.push(1);
            bytes.extend_from_slice(&length.to_le_bytes());
            shape(bytes, element);
        }
    }
}
// Immutable fixture bytes enter ONLY the existing canonical codec. This helper
// constructs fixture data; it is neither a runtime decoder nor source authority.
fn decoded(ty: &ConstantWireType, leaves: &[ValueRepr]) -> ConstValue {
    let mut bytes = b"LRRCNST\0".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    shape(&mut bytes, ty);
    for v in leaves {
        bytes.extend_from_slice(&v.kind.to_le_bytes());
        bytes.extend_from_slice(&v.type_id.to_le_bytes());
        bytes.extend_from_slice(&v.payload.to_le_bytes());
    }
    wire::decode(&bytes, ty, 1 << 20).unwrap()
}
fn array(p: PrimitiveType, length: u64) -> ConstantWireType {
    ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(p)),
        length,
    }
}
fn store() -> GuestStore {
    GuestStore::new(1, 100_000, 32 << 20).unwrap()
}
fn install(s: &mut GuestStore, t: lrr::runtime::cell::TicketId, ty: TypeId, slot: u32) -> CellId {
    let frame = s.new_frame_anchor().unwrap();
    let cell = s
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot },
        )
        .unwrap();
    s.initialize(cell, t).unwrap();
    cell
}
fn element(cell: CellId, indices: &[u32]) -> Place {
    Place {
        root: PlaceRoot::Cell(cell),
        projections: indices.iter().map(|n| Projection::Index(*n)).collect(),
    }
}
fn physical(
    s: &mut GuestStore,
    v: &ConstValue,
    w: &ConstantWireType,
    t: TypeId,
    f: &mut RunFuel,
) -> lrr::runtime::cell::TicketId {
    let p = s.prepare_constant_instantiation(v, w, t).unwrap();
    s.instantiate_constant(p, f).unwrap()
}
#[test]
fn constant_array_original_declarations_remain_unchanged_partial_source_controls() {
    let lint = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/lint/lint-const-item-mutation.rs"
    );
    assert!(
        lint.contains("//@ check-pass")
            && lint.contains("const ARRAY: [u8; 1] = [25];")
            && lint.contains("ARRAY[0] = 5; //~ WARN attempting to modify")
    );
    let runtime = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
    );
    assert!(
        runtime.contains("//@ run-pass")
            && runtime.contains("const ARR: [i32; add(1, 2)] = [5, 6, 7];")
    );
    // Full warning output/UB siblings and const-fn source execution are unforced.
}
#[test]
fn constant_array_two_uses_have_independent_cells_mutation_and_leases() {
    let mut s = store();
    let w = array(PrimitiveType::U8, 1);
    let v = decoded(&w, &[ValueRepr::unsigned(TypeId(9), 25).unwrap()]);
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 1,
        })
        .unwrap();
    let mut f = RunFuel::new(20).unwrap();
    let first = physical(&mut s, &v, &w, ty, &mut f);
    let second = physical(&mut s, &v, &w, ty, &mut f);
    assert_ne!(
        s.peek_ticket(first).unwrap().payload,
        s.peek_ticket(second).unwrap().payload
    );
    let a = install(&mut s, first, ty, 0);
    let b = install(&mut s, second, ty, 1);
    let ra = s.borrow(&element(a, &[0]), Access::Mutable).unwrap();
    let rb = s.borrow(&element(b, &[0]), Access::Mutable).unwrap();
    assert_ne!(ra, rb);
    let changed = s
        .ticket(ValueRepr::unsigned(TypeId(9), 5).unwrap())
        .unwrap();
    let old = s.replace_reference(ra, changed).unwrap();
    s.discard(old).unwrap();
    assert_eq!(s.read_reference(ra).unwrap().payload, 5);
    assert_eq!(s.read_reference(rb).unwrap().payload, 25);
    assert_eq!(v.elements().unwrap()[0].scalar().unwrap().payload, 25);
    let dropped = s.begin_drop(a).unwrap();
    loop {
        match s.aggregate_cleanup_step(dropped).unwrap() {
            AggregateCleanupStep::Field(t) => s.discard(t).unwrap(),
            AggregateCleanupStep::Skipped => {}
            AggregateCleanupStep::Complete => break,
        }
    }
    s.discard(dropped).unwrap();
    s.retire_cell(a).unwrap();
    assert!(s.read_reference(ra).is_err());
    assert_eq!(s.read_reference(rb).unwrap().payload, 25);
    assert_eq!(f.snapshot().used, 4);
}
#[test]
fn constant_nested_arrays_create_fresh_complete_field_cells() {
    let mut s = store();
    let inner = array(PrimitiveType::I32, 2);
    let w = ConstantWireType::Array {
        element: Box::new(inner),
        length: 2,
    };
    let v = decoded(
        &w,
        &[1, 2, 3, 4].map(|n| ValueRepr::signed(TypeId(6), n).unwrap()),
    );
    let child = s
        .intern_type(TypeKey::Array {
            element: TypeId(6),
            len: 2,
        })
        .unwrap();
    let ty = s
        .intern_type(TypeKey::Array {
            element: child,
            len: 2,
        })
        .unwrap();
    let mut f = RunFuel::new(20).unwrap();
    let a = physical(&mut s, &v, &w, ty, &mut f);
    let b = physical(&mut s, &v, &w, ty, &mut f);
    let a = install(&mut s, a, ty, 0);
    let b = install(&mut s, b, ty, 1);
    let reference = s.borrow(&element(a, &[1, 0]), Access::Mutable).unwrap();
    let t = s.ticket(ValueRepr::signed(TypeId(6), 9).unwrap()).unwrap();
    let old = s.replace_reference(reference, t).unwrap();
    s.discard(old).unwrap();
    assert_eq!(s.read_place(&element(a, &[1, 0])).unwrap().payload, 9);
    assert_eq!(s.read_place(&element(b, &[1, 0])).unwrap().payload, 3);
    assert_eq!(f.snapshot().used, 14);
}
#[test]
fn constant_empty_array_retains_exact_element_descriptor_and_owning_object() {
    let mut s = store();
    let w = array(PrimitiveType::U8, 0);
    let v = decoded(&w, &[]);
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 0,
        })
        .unwrap();
    let mut f = RunFuel::new(2).unwrap();
    let t = physical(&mut s, &v, &w, ty, &mut f);
    assert_eq!(s.peek_ticket(t).unwrap().kind, 8);
    assert_eq!(s.peek_ticket(t).unwrap().type_id, ty.0);
    assert_eq!(f.snapshot().used, 1);
    assert!(s.prepare_constant_instantiation(&v, &w, TypeId(1)).is_err());
}
#[test]
fn constant_unit_elements_charge_even_without_scalar_payload_bytes() {
    let mut s = store();
    let w = array(PrimitiveType::Unit, 3);
    let v = decoded(&w, &[ValueRepr::unit(); 3]);
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(1),
            len: 3,
        })
        .unwrap();
    let mut f = RunFuel::new(4).unwrap();
    let t = physical(&mut s, &v, &w, ty, &mut f);
    let cell = install(&mut s, t, ty, 0);
    for i in 0..3 {
        assert_eq!(
            s.read_place(&element(cell, &[i])).unwrap(),
            ValueRepr::unit()
        );
    }
    assert_eq!(f.snapshot().used, 4);
}
#[test]
fn constant_shape_mismatch_refuses_before_guest_storage_or_fuel() {
    let mut s = store();
    let w = array(PrimitiveType::U8, 1);
    let v = decoded(&w, &[ValueRepr::unsigned(TypeId(9), 25).unwrap()]);
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(6),
            len: 1,
        })
        .unwrap();
    let before = (s.budget().handles(), s.budget().bytes());
    assert!(s.prepare_constant_instantiation(&v, &w, ty).is_err());
    let wrong = s
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 2,
        })
        .unwrap();
    assert!(s.prepare_constant_instantiation(&v, &w, wrong).is_err());
    let huge = array(PrimitiveType::U8, u64::MAX);
    assert!(s.prepare_constant_instantiation(&v, &huge, wrong).is_err());
    assert_eq!(before, (s.budget().handles(), s.budget().bytes()));
}
#[test]
fn constant_partial_fuel_failure_preserves_existing_array_and_immutable_data() {
    let mut s = store();
    let w = array(PrimitiveType::I32, 3);
    let v = decoded(
        &w,
        &[5, 6, 7].map(|n| ValueRepr::signed(TypeId(6), n).unwrap()),
    );
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(6),
            len: 3,
        })
        .unwrap();
    let old = physical(&mut s, &v, &w, ty, &mut RunFuel::new(20).unwrap());
    let old = install(&mut s, old, ty, 0);
    let p = s.prepare_constant_instantiation(&v, &w, ty).unwrap();
    let mut f = RunFuel::new(2).unwrap();
    assert!(s.instantiate_constant(p, &mut f).is_err());
    assert_eq!(f.snapshot().used, 2);
    assert_eq!(s.read_place(&element(old, &[2])).unwrap().payload, 7);
    assert_eq!(v.elements().unwrap()[0].scalar().unwrap().payload, 5);
    let t = physical(&mut s, &v, &w, ty, &mut RunFuel::new(20).unwrap());
    assert_eq!(s.peek_ticket(t).unwrap().type_id, ty.0);
}
#[test]
fn constant_cancellation_and_resource_preflight_create_no_guest_values() {
    let mut s = GuestStore::new(1, 2, 32 << 20).unwrap();
    let w = array(PrimitiveType::Unit, 3);
    let v = decoded(&w, &[ValueRepr::unit(); 3]);
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(1),
            len: 3,
        })
        .unwrap();
    let p = s.prepare_constant_instantiation(&v, &w, ty).unwrap();
    let mut f = RunFuel::new(20).unwrap();
    f.cancellation().cancel();
    let before = (s.budget().handles(), s.budget().bytes());
    assert!(s.instantiate_constant(p, &mut f).is_err());
    assert_eq!(before, (s.budget().handles(), s.budget().bytes()));
    assert_eq!(f.snapshot().used, 0);
    let p = s.prepare_constant_instantiation(&v, &w, ty).unwrap();
    assert!(
        s.instantiate_constant(p, &mut RunFuel::new(20).unwrap())
            .is_err()
    );
    assert_eq!(before, (s.budget().handles(), s.budget().bytes()));
    let mut memory = GuestStore::new(1, 1000, 0).unwrap();
    let ty = memory
        .intern_type(TypeKey::Array {
            element: TypeId(1),
            len: 3,
        })
        .unwrap();
    let p = memory.prepare_constant_instantiation(&v, &w, ty).unwrap();
    assert!(
        memory
            .instantiate_constant(p, &mut RunFuel::new(20).unwrap())
            .is_err()
    );
    assert_eq!((memory.budget().handles(), memory.budget().bytes()), (0, 0));
}
#[test]
fn constant_plan_old_epoch_cannot_publish_in_fresh_guest_run() {
    let mut s = store();
    let w = array(PrimitiveType::Unit, 0);
    let v = decoded(&w, &[]);
    let ty = s
        .intern_type(TypeKey::Array {
            element: TypeId(1),
            len: 0,
        })
        .unwrap();
    let p = s.prepare_constant_instantiation(&v, &w, ty).unwrap();
    s.reset(2).unwrap();
    let mut f = RunFuel::new(10).unwrap();
    assert_eq!(
        s.instantiate_constant(p, &mut f).unwrap_err().code,
        "LRR-E-STALE-HANDLE"
    );
    assert_eq!(f.snapshot().used, 0);
    assert_eq!(s.budget().handles(), 0);
}

#[test]
fn constant_scalar_and_deadline_precommit_controls() {
    let mut s = store();
    let value = ConstValue::from_scalar(ValueRepr::signed(TypeId(6), -9).unwrap()).unwrap();
    let wire = ConstantWireType::Scalar(PrimitiveType::I32);
    let plan = s
        .prepare_constant_instantiation(&value, &wire, TypeId(6))
        .unwrap();
    let mut fuel = RunFuel::new(1).unwrap();
    let ticket = s.instantiate_constant(plan, &mut fuel).unwrap();
    assert_eq!(s.peek_ticket(ticket).unwrap(), value.scalar().unwrap());
    assert_eq!(fuel.snapshot().used, 1);
    s.discard(ticket).unwrap();
    let plan = s
        .prepare_constant_instantiation(&value, &wire, TypeId(6))
        .unwrap();
    let mut fuel = RunFuel::new(10).unwrap();
    fuel.set_deadline(std::time::Instant::now());
    let before = (s.budget().handles(), s.budget().bytes());
    assert!(s.instantiate_constant(plan, &mut fuel).is_err());
    assert_eq!(before, (s.budget().handles(), s.budget().bytes()));
    assert_eq!(fuel.snapshot().used, 0);
}
