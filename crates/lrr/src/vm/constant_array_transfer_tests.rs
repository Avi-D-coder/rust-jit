//! Physical T054 publication controls. The test-only opaque injection certifies
//! no production source authority, namespace, query currentness or full fixture.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    engine::constant_array_publication::physical_fixture_use,
    ir::constant_data::wire,
    ir::{
        op::{Op, PrimitiveType},
        pc::Phase,
    },
    runtime::{
        place::{Place, PlaceRoot, Projection},
        reference::Access,
        shape::TypeKey,
        value::ValueRepr,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::BodyIndexLimits, parse::ParsedFile},
    traits::term::{ConstTerm, TyTerm},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/lint-const-item-mutation.rs"
);
const ADAPTER: &str =
    "const ARRAY:[u8;1]=[25];fn main(){let mut first=ARRAY;let mut second=ARRAY;}";
fn value() -> (ConstValue, ConstantWireType) {
    let wire = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Scalar(PrimitiveType::U8)),
        length: 1,
    };
    let mut bytes = b"LRRCNST\0".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&9u32.to_le_bytes());
    let v = ValueRepr::unsigned(TypeId(9), 25).unwrap();
    bytes.extend_from_slice(&v.kind.to_le_bytes());
    bytes.extend_from_slice(&v.type_id.to_le_bytes());
    bytes.extend_from_slice(&v.payload.to_le_bytes());
    (wire::decode(&bytes, &wire, 1 << 20).unwrap(), wire)
}
fn source() -> TyTerm {
    TyTerm::Array(
        Box::new(TyTerm::Primitive(PrimitiveType::U8)),
        ConstTerm::Value(1),
    )
}
fn context(text: &str, edition: u16) -> (Frame, GuestStore, Vec<NodeId>) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(
        &blob,
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"T054 physical temporary controls"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = parsed
        .index_body(handle, BodyIndexLimits::default())
        .unwrap();
    let paths = body
        .nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Path))
        .filter_map(|n| {
            let span = body.span(n.id).unwrap();
            let start = handle.range.start + span.relative_start;
            let end = handle.range.start + span.relative_end;
            (&text[start as usize..end as usize] == "ARRAY").then_some(n.id)
        })
        .collect::<Vec<_>>();
    assert!(!paths.is_empty());
    let mut store = GuestStore::for_epoch(1).unwrap();
    let frame = Frame::new(
        body.clone(),
        body.pc(paths[0], Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    (frame, store, paths)
}

#[test]
fn store_session_frame_and_constant_proof_refuse_equal_bit_foreign_store() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut a, mut sa, pa) = context(ADAPTER, edition);
        let (mut b, mut sb, pb) = context(ADAPTER, edition);
        reach(&mut a, pa[0]);
        reach(&mut b, pb[0]);
        assert_eq!(
            a.constant_frame_anchor().bits(),
            b.constant_frame_anchor().bits()
        );
        assert_eq!(a.pc(), b.pc());
        let key = TypeKey::Array {
            element: TypeId(9),
            len: 1,
        };
        let ta = sa.intern_type(key.clone()).unwrap();
        let tb = sb.intern_type(key).unwrap();
        assert_eq!(ta, tb);
        let (v, w) = value();
        let proof = physical_fixture_use(&a, target(&a), source(), ta, &v, &w).unwrap();
        let before = (
            sb.budget().handles(),
            sb.budget().bytes(),
            b.fuel.snapshot().used,
        );
        assert_eq!(a.require_store(&sb).unwrap_err().code, "LRR-E-STALE-HANDLE");
        assert_eq!(
            b.preflight_constant_use(&proof, &sb).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert!(
            matches!(b.prepare_constant_publication(&proof, &v, &w, &sb), Err(e) if e.code == "LRR-E-STALE-HANDLE")
        );
        assert!(
            matches!(Frame::new_in_run(b.body.clone(), b.pc.clone(), &mut sb, &mut a), Err(e) if e.code == "LRR-E-STALE-HANDLE")
        );
        assert_eq!(
            before,
            (
                sb.budget().handles(),
                sb.budget().bytes(),
                b.fuel.snapshot().used
            )
        );
        assert!(a.preflight_constant_use(&proof, &sa).is_ok());
    }
}
fn reach(frame: &mut Frame, node: NodeId) {
    frame.pc = frame.body.pc(node, Phase::Entry).unwrap();
    let destination = frame.body.nodes()[node.0 as usize].value.unwrap();
    let mut scope = Some(frame.slots[destination.0 as usize].scope);
    let mut scopes = Vec::new();
    while let Some(s) = scope {
        scopes.push(s);
        scope = frame.body.scopes()[s.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        if !frame.active_scopes.contains(&scope) {
            frame.enter(scope).unwrap();
        }
    }
}
fn target(frame: &Frame) -> SlotId {
    frame.body.nodes()[frame.pc.node as usize].value.unwrap()
}
fn publish_chunks(
    frame: &mut Frame,
    prepared: PreparedConstantPublication<'_, '_>,
    proof: &AdmittedConstantUse<'_>,
    store: &mut GuestStore,
) -> Result<(), Diagnostic> {
    let mut activation = frame.begin_constant_publication_chunks(prepared, store, |frame| {
        frame.fuel.charge();
        Ok(())
    })?;
    for _ in 0..20000 {
        match frame.advance_constant_publication_chunks(&mut activation, proof, store) {
            chunks::ConstantPublicationChunk::Dispatch { steps, bytes, .. } => {
                assert!(steps <= 256 && bytes <= 64 << 10)
            }
            chunks::ConstantPublicationChunk::Published => return Ok(()),
            chunks::ConstantPublicationChunk::Fault {
                diagnostic,
                cleanup_pending: false,
            } => return Err(diagnostic),
            chunks::ConstantPublicationChunk::Fault {
                cleanup_pending: true,
                ..
            } => panic!("private control cleanup did not drain"),
        }
    }
    panic!("constant control did not finish bounded dispatches")
}
fn publish(frame: &mut Frame, store: &mut GuestStore, ty: TypeId) -> SlotId {
    let (v, w) = value();
    let dst = target(frame);
    let proof = physical_fixture_use(frame, dst, source(), ty, &v, &w).unwrap();
    let prepared = frame
        .prepare_constant_publication(&proof, &v, &w, store)
        .unwrap();
    publish_chunks(frame, prepared, &proof, store).unwrap();
    dst
}
fn element(cell: CellId) -> Place {
    Place {
        root: PlaceRoot::Cell(cell),
        projections: Box::new([Projection::Index(0)]),
    }
}
fn leave_current(frame: &mut Frame, store: &mut GuestStore) {
    let scope = *frame.active_scopes.last().unwrap();
    assert!(
        frame
            .begin_scope_cleanup(0, &Op::ScopeLeave { scope }, store)
            .unwrap()
    );
    let mut done = false;
    for _ in 0..1000 {
        if frame.advance_scope_cleanup(store).unwrap() {
            done = true;
            break;
        }
    }
    assert!(done);
    assert!(!frame.has_cleanup_activation());
}
fn binding(frame: &Frame, text: &str, name: &str) -> NodeId {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        frame.body.profile().clone(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"T054 physical temporary controls"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    frame
        .body
        .nodes()
        .iter()
        .find(|n| {
            if n.role != NodeRole::Pattern {
                return false;
            }
            let span = frame.body.span(n.id).unwrap();
            let raw = &text[(handle.range.start + span.relative_start) as usize
                ..(handle.range.start + span.relative_end) as usize];
            raw == format!("mut {name}") || raw == name
        })
        .unwrap()
        .id
}
#[test]
fn original_constant_array_path_publishes_physical_temporary_in_all_editions() {
    assert!(
        ORIGINAL.contains("//@ check-pass") && ORIGINAL.contains("const ARRAY: [u8; 1] = [25];")
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut frame, mut store, paths) = context(ORIGINAL, edition);
        reach(&mut frame, paths[0]);
        let ty = store
            .intern_type(TypeKey::Array {
                element: TypeId(9),
                len: 1,
            })
            .unwrap();
        let dst = publish(&mut frame, &mut store, ty);
        assert_eq!(frame.source_view_ref(dst).unwrap(), Some(&source()));
        assert_eq!(
            store
                .read_place(&element(frame.cell(dst).unwrap()))
                .unwrap()
                .payload,
            25
        );
        assert_eq!(frame.fuel.snapshot().used, 2);
        while !frame.active_scopes.is_empty() {
            leave_current(&mut frame, &mut store);
        }
        assert!(frame.slots[dst.0 as usize].cell.is_none());
    }
}
#[test]
fn two_constant_uses_transfer_to_independent_mutable_variable_cells_and_cleanup() {
    let (mut frame, mut store, paths) = context(ADAPTER, 2024);
    let ty = store
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 1,
        })
        .unwrap();
    reach(&mut frame, paths[0]);
    let first = publish(&mut frame, &mut store, ty);
    let first_pattern = binding(&frame, ADAPTER, "first");
    frame
        .bind(first_pattern, first, Some(ty), true, &mut store)
        .unwrap();
    let first_cell = frame.binding(first_pattern).unwrap();
    assert_eq!(
        frame.binding_sources[first_pattern.0 as usize].as_ref(),
        Some(&source())
    );
    assert_eq!(
        store.state(frame.cell(first).unwrap()).unwrap(),
        CellState::Moved
    );
    leave_current(&mut frame, &mut store);
    reach(&mut frame, paths[1]);
    let second = publish(&mut frame, &mut store, ty);
    let second_pattern = binding(&frame, ADAPTER, "second");
    frame
        .bind(second_pattern, second, Some(ty), true, &mut store)
        .unwrap();
    let second_cell = frame.binding(second_pattern).unwrap();
    assert_ne!(first_cell, second_cell);
    let a = store.borrow(&element(first_cell), Access::Mutable).unwrap();
    let b = store
        .borrow(&element(second_cell), Access::Mutable)
        .unwrap();
    let v = store
        .ticket(ValueRepr::unsigned(TypeId(9), 5).unwrap())
        .unwrap();
    let old = store.replace_reference(a, v).unwrap();
    store.discard(old).unwrap();
    assert_eq!(store.read_reference(a).unwrap().payload, 5);
    assert_eq!(store.read_reference(b).unwrap().payload, 25);
    while !frame.active_scopes.is_empty() {
        leave_current(&mut frame, &mut store);
    }
    assert!(store.read_reference(a).is_err() && store.read_reference(b).is_err());
    assert!(
        frame.bindings[first_pattern.0 as usize].is_none()
            && frame.bindings[second_pattern.0 as usize].is_none()
    );
    assert!(frame.counters.cleanup_elements_completed >= 4);
}
#[test]
fn foreign_frame_wrong_pc_and_altered_content_refuse_before_publication() {
    let (mut frame, mut store, paths) = context(ADAPTER, 2024);
    reach(&mut frame, paths[0]);
    let ty = store
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 1,
        })
        .unwrap();
    let (v, w) = value();
    let proof = physical_fixture_use(&frame, target(&frame), source(), ty, &v, &w).unwrap();
    let prepared = frame
        .prepare_constant_publication(&proof, &v, &w, &store)
        .unwrap();
    let before = (store.budget().handles(), store.budget().bytes());
    frame.pc = frame.body.pc(paths[1], Phase::Entry).unwrap();
    assert!(publish_chunks(&mut frame, prepared, &proof, &mut store).is_err());
    assert_eq!(before, (store.budget().handles(), store.budget().bytes()));
    let mut other = Frame::new(
        frame.body.clone(),
        frame.body.pc(paths[0], Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    reach(&mut other, paths[0]);
    assert_ne!(other.anchor, proof.frame());
    assert!(
        other
            .prepare_constant_publication(&proof, &v, &w, &store)
            .is_err()
    );
    frame.pc = frame.body.pc(paths[0], Phase::Entry).unwrap();
    let changed = ConstValue::from_scalar(ValueRepr::unsigned(TypeId(9), 1).unwrap()).unwrap();
    assert!(
        frame
            .prepare_constant_publication(
                &proof,
                &changed,
                &ConstantWireType::Scalar(PrimitiveType::U8),
                &store
            )
            .is_err()
    );
}
#[test]
fn source_descriptor_scope_and_live_destination_refusals_preserve_published_owner() {
    let (mut frame, mut store, paths) = context(ADAPTER, 2024);
    let ty = store
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 1,
        })
        .unwrap();
    let (v, w) = value();
    let dst = target(&frame);
    let proof = physical_fixture_use(&frame, dst, source(), ty, &v, &w).unwrap();
    assert!(
        frame
            .prepare_constant_publication(&proof, &v, &w, &store)
            .is_err()
    );
    reach(&mut frame, paths[0]);
    let wrong = physical_fixture_use(
        &frame,
        dst,
        TyTerm::Array(
            Box::new(TyTerm::Primitive(PrimitiveType::I32)),
            ConstTerm::Value(1),
        ),
        ty,
        &v,
        &w,
    )
    .unwrap();
    assert!(
        frame
            .prepare_constant_publication(&wrong, &v, &w, &store)
            .is_err()
    );
    let dst = publish(&mut frame, &mut store, ty);
    let cell = frame.cell(dst).unwrap();
    let before = store.read_place(&element(cell)).unwrap();
    assert!(
        frame
            .prepare_constant_publication(&proof, &v, &w, &store)
            .is_err()
    );
    assert_eq!(store.read_place(&element(cell)).unwrap(), before);
}

#[test]
fn nested_source_and_conflicting_source_context_refuse_before_read_or_allocation() {
    let (mut frame, mut store, paths) = context(ADAPTER, 2024);
    reach(&mut frame, paths[0]);
    let ty = store
        .intern_type(TypeKey::Array {
            element: TypeId(9),
            len: 1,
        })
        .unwrap();
    let (v, w) = value();
    let dst = target(&frame);
    let nested = TyTerm::Array(Box::new(source()), ConstTerm::Value(1));
    let proof = physical_fixture_use(&frame, dst, nested, ty, &v, &w).unwrap();
    let before = (store.budget().handles(), store.budget().bytes());
    assert!(
        frame
            .prepare_constant_publication(&proof, &v, &w, &store)
            .is_err()
    );
    assert_eq!(before, (store.budget().handles(), store.budget().bytes()));
    frame.slots[dst.0 as usize].source = Some(TyTerm::Primitive(PrimitiveType::Bool));
    let proof = physical_fixture_use(&frame, dst, source(), ty, &v, &w).unwrap();
    assert!(
        frame
            .prepare_constant_publication(&proof, &v, &w, &store)
            .is_err()
    );
    assert_eq!(before, (store.budget().handles(), store.budget().bytes()));
}
