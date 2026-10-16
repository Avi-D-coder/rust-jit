//! External source/descriptor authority controls, not stock runtime receipts.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        op::{Op, PrimitiveType},
        pc::Phase,
    },
    runtime::shape::PrimitiveIdentity,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::BodyIndex, parse::ParsedFile},
};
use ra_ap_syntax::{AstNode, SyntaxKind, ast};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
);

// This is source-fact extraction for the control's prepared aggregate setup.
// It performs no guest execution and grants no general Type-path resolution.
fn source_shape(expression: ast::Expr) -> TyTerm {
    match expression {
        ast::Expr::TupleExpr(tuple) => TyTerm::Tuple(tuple.fields().map(source_shape).collect()),
        ast::Expr::Literal(literal)
            if matches!(
                literal.token().kind(),
                SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW
            ) =>
        {
            TyTerm::Primitive(PrimitiveType::Bool)
        }
        _ => panic!("selected upstream initializer changed its intrinsic source shape"),
    }
}
fn type_for_source(store: &mut GuestStore, source: &TyTerm) -> TypeId {
    match source {
        TyTerm::Primitive(primitive) => PrimitiveIdentity::from_recipe(*primitive).unwrap().id(),
        TyTerm::Tuple(members) => {
            let members = members
                .iter()
                .map(|member| type_for_source(store, member))
                .collect::<Vec<_>>();
            store
                .intern_type(TypeKey::Tuple(members.into_boxed_slice()))
                .unwrap()
        }
        _ => panic!("prepared component source outside this control"),
    }
}
fn prepared_tuple(store: &mut GuestStore, source: &TyTerm) -> TicketId {
    // Dummy values establish a component's storage graph. They are not an
    // execution of the upstream initializer or a stock-fixture receipt.
    let TyTerm::Tuple(members) = source else {
        return store.ticket(ValueRepr::boolean(false)).unwrap();
    };
    let types = members
        .iter()
        .map(|member| type_for_source(store, member))
        .collect::<Vec<_>>();
    let target = type_for_source(store, source);
    let guard = store.begin_construction(&types).unwrap();
    for (i, member) in members.iter().enumerate() {
        let ticket = prepared_tuple(store, member);
        store
            .construction_push(guard, u32::try_from(i).unwrap(), ticket)
            .unwrap();
    }
    store.commit_construction(guard, target).unwrap()
}
fn original_source() -> (
    ParsedFile,
    crate::syntax::outline::BodyHandle,
    BodyIndex,
    TyTerm,
    NodeId,
) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([83; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "search")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    let root = parsed.entered_body_root(&body).unwrap();
    let initializer = root
        .descendants()
        .find_map(ast::LetStmt::cast)
        .unwrap()
        .initializer()
        .unwrap();
    let range = initializer.syntax().text_range();
    let id = index
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == u32::from(range.start()) - body.range.start
                && node.range.end == u32::from(range.end()) - body.range.start
                && matches!(
                    node.role,
                    crate::syntax::body_index::NodeRole::Expression(_)
                )
        })
        .unwrap()
        .id;
    let source = source_shape(initializer);
    (parsed, body, index, source, id)
}

#[test]
fn ordinary_tuple_projection_preserves_admitted_nested_source_and_does_not_consume_fields() {
    let (_, _, index, source, node) = original_source();
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
    let mut frame = Frame::new(std::sync::Arc::new(index), entry, &mut store).unwrap();
    frame.enter(frame.body.nodes()[0].scope).unwrap();
    let source_slot = frame.body.nodes()[node.0 as usize].value.unwrap();
    let source_scope = frame.slot(source_slot).unwrap().scope;
    let mut pending = Vec::new();
    let mut scope = source_scope;
    while !frame.active_scopes.contains(&scope) {
        pending.push(scope);
        scope = frame.body.scopes()[scope.0 as usize].parent.unwrap();
    }
    for scope in pending.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let base = frame.body.nodes()[node.0 as usize].place.unwrap();
    let destination = frame.body.nodes()[0].place.unwrap();
    let ticket = prepared_tuple(&mut store, &source);
    frame.write(source_slot, ticket, &mut store).unwrap();
    frame
        .set_source_view(source_slot, Some(source.clone()))
        .unwrap();
    frame.value_place(source_slot, base, &store).unwrap();
    let source_cell = frame.cell(source_slot).unwrap();
    let before = (
        store.state(source_cell).unwrap(),
        store.location_epoch(source_cell).unwrap(),
        store.budget().handles(),
    );
    let prepared = frame
        .prepare_projection(
            &Op::ProjectTuple {
                base,
                field: 1,
                destination,
            },
            &store,
        )
        .unwrap()
        .unwrap();
    frame.commit_projection(prepared).unwrap();
    let TyTerm::Tuple(members) = source else {
        unreachable!();
    };
    assert_eq!(
        frame.place_slot(destination).unwrap().source.as_ref(),
        Some(&members[1])
    );
    assert_eq!(
        (
            store.state(source_cell).unwrap(),
            store.location_epoch(source_cell).unwrap(),
            store.budget().handles()
        ),
        before
    );
}

#[test]
fn structural_source_mismatch_and_missing_member_proof_refuse_without_guest_changes() {
    let (_, _, _, source, _) = original_source();
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let target = type_for_source(&mut store, &source);
    Frame::require_structural_source_target(&source, target, &store).unwrap();
    let before = (store.budget().handles(), store.budget().bytes());
    let mut wrong = source;
    if let TyTerm::Tuple(members) = &mut wrong {
        members[1] = TyTerm::Primitive(PrimitiveType::Char);
    } else {
        unreachable!();
    }
    assert!(
        matches!(Frame::require_structural_source_target(&wrong,target,&store),Err(error) if error.code=="LRR-E-OWNERSHIP")
    );
    if let TyTerm::Tuple(members) = &mut wrong {
        members[1] = TyTerm::Nominal(Digest([84; 32]), vec![]);
    }
    assert!(
        matches!(Frame::require_structural_source_target(&wrong,target,&store),Err(error) if error.code=="LRR-E-UNSUPPORTED")
    );
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
}

#[test]
fn structural_source_unit_and_depth_limits_have_explicit_outcomes() {
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    Frame::require_structural_source_target(&TyTerm::Tuple(vec![]), TypeId(1), &store).unwrap();
    let mut source = TyTerm::Primitive(PrimitiveType::Bool);
    let mut target = TypeId(2);
    for _ in 0..256 {
        source = TyTerm::Tuple(vec![source]);
        target = store
            .intern_type(TypeKey::Tuple(Box::new([target])))
            .unwrap();
    }
    assert!(
        matches!(Frame::require_structural_source_target(&source,target,&store),Err(error) if error.code=="LRR-E-RUNTIME-RESOURCE")
    );
}

fn enter_slot_scopes(frame: &mut Frame, slot: SlotId) {
    let mut pending = Vec::new();
    let mut scope = frame.slots[slot.0 as usize].scope;
    while !frame.active_scopes.contains(&scope) {
        pending.push(scope);
        let Some(parent) = frame.body.scopes()[scope.0 as usize].parent else {
            break;
        };
        scope = parent;
    }
    for scope in pending.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
}

#[test]
fn composite_source_quota_refusal_is_atomic_and_releasing_one_view_allows_retry() {
    let (_, _, index, _, _) = original_source();
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
    let mut frame = Frame::new(Arc::new(index), entry, &mut store).unwrap();
    let view = TyTerm::Tuple(vec![TyTerm::Primitive(PrimitiveType::Bool); 50_000]);
    let heap = Frame::source_heap(Some(&view)).unwrap();
    assert!(heap > 2 * std::mem::size_of::<TyTerm>());
    let before = (store.budget().handles(), store.budget().bytes());
    let mut installed = Vec::new();
    let mut refused = None;
    for ordinal in 0..frame.slots.len() {
        let slot = SlotId(u32::try_from(ordinal).unwrap());
        let charged = frame.source_extra_bytes;
        match frame.set_source_view(slot, Some(view.clone())) {
            Ok(()) => installed.push(slot),
            Err(error) => {
                assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
                assert_eq!(frame.source_extra_bytes, charged);
                assert!(frame.source_view_ref(slot).unwrap().is_none());
                refused = Some(slot);
                break;
            }
        }
    }
    let refused = refused.expect("actual composite heap must exhaust the frame budget");
    let released = installed.pop().unwrap();
    frame.set_source_view(released, None).unwrap();
    frame.set_source_view(refused, Some(view)).unwrap();
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
    for slot in installed.into_iter().chain([refused]) {
        frame.set_source_view(slot, None).unwrap();
    }
    assert_eq!(frame.source_extra_bytes, 0);
}

#[test]
fn composite_call_source_batch_refuses_before_publishing_any_argument() {
    let (_, _, index, _, _) = original_source();
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
    let frame = Frame::new(Arc::new(index), entry, &mut store).unwrap();
    let source = TyTerm::Tuple(vec![TyTerm::Primitive(PrimitiveType::Bool); 90_000]);
    let heap = Frame::source_heap(Some(&source)).unwrap();
    let count = (32 * 1024 * 1024) / (2 * heap) + 2;
    let slots = frame
        .body
        .nodes()
        .iter()
        .filter_map(|node| node.value)
        .take(count)
        .collect::<Vec<_>>();
    assert_eq!(slots.len(), count);
    let hints = slots
        .iter()
        .map(|slot| (*slot, None, Some(source.clone())))
        .collect::<Vec<_>>();
    let before = (
        store.budget().handles(),
        store.budget().bytes(),
        frame.source_extra_bytes,
    );
    assert_eq!(
        frame.preflight_call_source_views(&hints).unwrap_err().code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_eq!(
        (
            store.budget().handles(),
            store.budget().bytes(),
            frame.source_extra_bytes
        ),
        before
    );
    assert!(
        slots
            .iter()
            .all(|slot| frame.source_view_ref(*slot).unwrap().is_none())
    );
    let small = slots
        .iter()
        .map(|slot| {
            (
                *slot,
                Some(TypeId(2)),
                Some(TyTerm::Primitive(PrimitiveType::Bool)),
            )
        })
        .collect::<Vec<_>>();
    frame.preflight_call_source_views(&small).unwrap();
    let duplicate = vec![small[0].clone(), small[0].clone()];
    assert_eq!(
        frame
            .preflight_call_source_views(&duplicate)
            .unwrap_err()
            .code,
        "LRR-E-INTERNAL-IR"
    );
}

#[test]
fn source_heap_moves_with_places_and_cleanup_releases_it_once() {
    let (_, _, index, _, node) = original_source();
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
    let mut frame = Frame::new(Arc::new(index), entry, &mut store).unwrap();
    let source = frame.body.nodes()[node.0 as usize].place.unwrap();
    let destination = frame.body.nodes()[0].place.unwrap();
    enter_slot_scopes(&mut frame, source);
    let ticket = store.ticket(ValueRepr::boolean(false)).unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(TypeId(2)),
            true,
            CellOwner::FrameSlot {
                frame: frame.anchor,
                slot: source.0,
            },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    // This is an address/accounting component control: the large annotation is
    // never used to authorize a descriptor read or a projected guest operation.
    frame.slots[source.0 as usize].place = Some(crate::runtime::place::Place::cell(cell));
    let view = TyTerm::Tuple(vec![TyTerm::Primitive(PrimitiveType::Bool); 4096]);
    frame.set_source_view(source, Some(view)).unwrap();
    let charged = frame.source_extra_bytes;
    assert!(charged > 0);
    frame.transfer_place(source, destination, &store).unwrap();
    assert_eq!(frame.source_extra_bytes, charged);
    assert!(frame.source_view_ref(source).unwrap().is_none());
    frame.clear_place(destination).unwrap();
    assert_eq!(frame.source_extra_bytes, 0);
    frame.clear_place(destination).unwrap();
    assert_eq!(frame.source_extra_bytes, 0);
    store.retire_cell(cell).unwrap_err(); // A live owner is still live after address cleanup.
    let ticket = store.begin_drop(cell).unwrap();
    store.discard(ticket).unwrap();
    store.retire_cell(cell).unwrap();
}

#[test]
fn scope_cleanup_releases_composite_annotations_but_keeps_the_return_source_owned() {
    let (_, _, index, _, node) = original_source();
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
    let mut frame = Frame::new(Arc::new(index), entry, &mut store).unwrap();
    let source = frame.body.nodes()[node.0 as usize].value.unwrap();
    enter_slot_scopes(&mut frame, source);
    let view = TyTerm::Tuple(vec![TyTerm::Primitive(PrimitiveType::Bool); 4096]);
    frame.set_source_view(source, Some(view.clone())).unwrap();
    frame.set_return_source_view(Some(view)).unwrap();
    let returned = Frame::source_excess(frame.return_source.as_ref()).unwrap();
    assert_eq!(frame.source_extra_bytes, 2 * returned);
    let must_yield = frame.slots.len() > 256;
    let mut yielded = false;
    while let Some(scope) = frame.active_scopes.last().copied() {
        frame.charge_fuel().unwrap();
        assert!(
            frame
                .begin_scope_cleanup(0, &Op::ScopeLeave { scope }, &mut store)
                .unwrap()
        );
        loop {
            let complete = frame.advance_scope_cleanup(&mut store).unwrap();
            let measured = frame
                .slots
                .iter()
                .map(|slot| Frame::source_excess(slot.source.as_ref()).unwrap())
                .chain(
                    frame
                        .binding_sources
                        .iter()
                        .map(|source| Frame::source_excess(source.as_ref()).unwrap()),
                )
                .chain(
                    frame
                        .parameter_sources
                        .iter()
                        .map(|source| Frame::source_excess(Some(source)).unwrap()),
                )
                .chain([Frame::source_excess(frame.return_source.as_ref()).unwrap()])
                .sum::<usize>();
            assert_eq!(frame.source_extra_bytes, measured);
            assert!(frame.return_source.is_some());
            if complete {
                break;
            }
            yielded = true;
            assert!(frame.cleanup_activation.is_some());
        }
    }
    assert!(!must_yield || yielded);
    assert_eq!(frame.source_extra_bytes, returned);
    assert!(frame.source_view_ref(source).unwrap().is_none());
    assert!(frame.return_source.is_some());
    frame.set_return_source_view(None).unwrap();
    assert_eq!(frame.source_extra_bytes, 0);
}

#[test]
fn original_inner_tuple_constructor_retains_actual_field_sources_through_guard_and_finish() {
    use crate::syntax::constructors::ConstructorKind;
    let (parsed, handle, index, _, outer) = original_source();
    let outer = parsed
        .constructor_plan(&handle, &index, outer, &CfgSet::guest_default())
        .unwrap()
        .unwrap();
    let ConstructorKind::Tuple(outer_fields) = outer.kind() else {
        unreachable!()
    };
    let plan = parsed
        .constructor_plan(
            &handle,
            &index,
            outer_fields[0].node,
            &CfgSet::guest_default(),
        )
        .unwrap()
        .unwrap();
    let ConstructorKind::Tuple(fields) = plan.kind() else {
        unreachable!()
    };
    assert_eq!(fields.len(), 2);
    let guard = index.nodes()[plan.node().0 as usize]
        .constructor_guard
        .unwrap();
    let destination = index.nodes()[plan.node().0 as usize].value.unwrap();
    let mut store = GuestStore::new(1, 10000, 16 << 20).unwrap();
    let entry = index.pc(plan.node(), Phase::Entry).unwrap();
    let mut frame = Frame::new(Arc::new(index), entry, &mut store).unwrap();
    enter_slot_scopes(&mut frame, guard);
    frame
        .prepare_observed_structural_constructor(&plan)
        .unwrap();
    frame.start_prepared_constructor(guard, &mut store).unwrap();
    let active = frame.slots[guard.0 as usize]
        .construction
        .as_ref()
        .unwrap()
        .clone();
    assert!(Arc::ptr_eq(
        active.source_operands.as_ref().unwrap(),
        frame.slots[guard.0 as usize]
            .construction
            .as_ref()
            .unwrap()
            .source_operands
            .as_ref()
            .unwrap()
    ));
    assert!(frame.constructor_mapping_bytes > 0);
    for (ordinal, field) in fields.iter().enumerate() {
        enter_slot_scopes(&mut frame, field.value);
        let entry = frame.body.pc(field.node, Phase::Entry).unwrap();
        let recipe = parsed.initial_thunk(&handle, &frame.body, &entry).unwrap();
        frame.pc = entry;
        crate::vm::step::step(&mut frame, &mut store, &recipe).unwrap();
        frame
            .constructor_field(
                guard,
                u32::try_from(ordinal).unwrap(),
                field.value,
                &mut store,
            )
            .unwrap();
        assert_eq!(
            frame.source_view_ref(field.value).unwrap(),
            Some(&TyTerm::Primitive(PrimitiveType::Bool))
        );
        assert_eq!(
            store.state(frame.cell(field.value).unwrap()).unwrap(),
            CellState::Moved
        );
    }
    let before = (store.budget().handles(), store.budget().bytes());
    frame
        .set_source_view(
            fields[0].value,
            Some(TyTerm::Primitive(PrimitiveType::Char)),
        )
        .unwrap();
    assert_eq!(
        frame
            .preflight_constructor_finish(guard, destination, &store)
            .unwrap_err()
            .code,
        "LRR-E-OWNERSHIP"
    );
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
    assert!(frame.slots[guard.0 as usize].construction.is_some());
    frame
        .set_source_view(
            fields[0].value,
            Some(TyTerm::Primitive(PrimitiveType::Bool)),
        )
        .unwrap();
    frame
        .finish_constructor(guard, destination, &mut store)
        .unwrap();
    assert_eq!(
        frame.source_view_ref(destination).unwrap(),
        Some(&TyTerm::Tuple(vec![
            TyTerm::Primitive(PrimitiveType::Bool);
            2
        ]))
    );
    assert_eq!(frame.constructor_mapping_bytes, 0);
    assert!(
        frame
            .finish_constructor(guard, destination, &mut store)
            .is_err()
    );
    let place = frame.body.nodes()[plan.node().0 as usize].place.unwrap();
    let copied = frame.body.nodes()[0].value.unwrap();
    frame.value_place(destination, place, &store).unwrap();
    let source = frame.source_view(place).unwrap();
    let read = Op::ReadPlace {
        place,
        destination: copied,
    };
    frame.preflight_read_place(place, copied, &store).unwrap();
    assert!(frame.begin_aggregate_copy(0, &read, &mut store).unwrap());
    let before = (
        store.state(frame.cell(destination).unwrap()).unwrap(),
        store.budget().handles(),
    );
    // A suspended activation's source facts are currentness constraints. A
    // mismatch must refuse before advancing/consuming its prepared Copy work.
    frame.set_source_view(place, None).unwrap();
    assert_eq!(
        frame.advance_aggregate_copy(&mut store).unwrap_err().code,
        "LRR-E-INTERNAL-IR"
    );
    assert_eq!(
        (
            store.state(frame.cell(destination).unwrap()).unwrap(),
            store.budget().handles()
        ),
        before
    );
    assert!(frame.copy_activation.is_some());
    frame.set_source_view(place, source.clone()).unwrap();
    while !frame.advance_aggregate_copy(&mut store).unwrap() {}
    assert_eq!(frame.source_view_ref(copied).unwrap(), source.as_ref());
    assert_eq!(
        store.state(frame.cell(destination).unwrap()).unwrap(),
        before.0
    );
    assert!(frame.copy_activation.is_none());
}
