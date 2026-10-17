//! Actual current source proof reborrows over disclosed ordinary frame phases.
//! This is not automatic dispatcher/complete official nrvo execution.
use super::*;
use crate::{
    frontend::{
        constant_construction::{
            CurrentConstantConstruction,
            tests::{array, query, result, success, world},
        },
        constant_path_selection::{ConstantPathOutcome, ConstantPathSelection},
    },
    ir::op::PrimitiveType,
    runtime::{
        place::{Place, PlaceRoot, Projection},
        reference::Access,
        value::ValueRepr,
    },
    syntax::{body_index::BodyIndexLimits, outline::BodyHandle, spans::SourceRange},
};
const TEXT: &str =
    "const BIG:[u8;1024]=[0;1024];fn chunk_use(){let first=BIG;let second=BIG;}fn main(){}";
fn frame(
    db: &crate::db::memo::Database,
    view: &crate::source::changes::PreparedSourceView,
    store: &mut GuestStore,
) -> (Frame, BodyHandle, crate::ir::pc::NodeId) {
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "chunk_use")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .clone();
    let parsed = db.prepared_parse(&view.selection().outline.parse).unwrap();
    let index = parsed
        .index_body(&body, BodyIndexLimits::default())
        .unwrap();
    let node = index
        .nodes()
        .iter()
        .find(|node| {
            if node.role != NodeRole::Expression(ExpressionKind::Path) {
                return false;
            }
            let s = index.span(node.id).unwrap();
            &TEXT[(body.range.start + s.relative_start) as usize
                ..(body.range.start + s.relative_end) as usize]
                == "BIG"
        })
        .unwrap()
        .id;
    let mut frame = Frame::new(
        index.clone(),
        index.pc(node, crate::ir::pc::Phase::Entry).unwrap(),
        store,
    )
    .unwrap();
    let mut current = Some(
        frame
            .constant_slot_scope_for_test(index.nodes()[node.0 as usize].value.unwrap())
            .unwrap(),
    );
    let mut scopes = Vec::new();
    while let Some(scope) = current {
        scopes.push(scope);
        current = index.scopes()[scope.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.constant_enter_for_test(scope).unwrap();
    }
    (frame, body, node)
}
fn path(
    db: &crate::db::memo::Database,
    view: &crate::source::changes::PreparedSourceView,
    frame: &Frame,
    body: &BodyHandle,
    node: crate::ir::pc::NodeId,
    views: &[&crate::source::changes::PreparedSourceView],
) -> ConstantPathSelection {
    let s = frame.body.span(node).unwrap();
    let range = SourceRange {
        start: body.range.start + s.relative_start,
        end: body.range.start + s.relative_end,
    };
    let ConstantPathOutcome::Selected(path) =
        ConstantPathSelection::capture_current(db, view, body, range, views, 32 << 20).unwrap()
    else {
        panic!("BIG source not selected")
    };
    *path
}
#[test]
fn actual_array_source_proof_is_reborrowed_each_chunk_with_one_entry_and_stable_pc() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(TEXT, "BIG", edition);
        let ready = result(&db, &view, definition);
        let (_, d) = ready.array().unwrap();
        let child = ready
            .admission()
            .reselect_array_length_current(&db, &view, d, 32 << 20)
            .unwrap();
        let length = query(&mut db, &view, &child, &d.expected_wire());
        let parent = query(
            &mut db,
            &view,
            ready.admission(),
            &array(PrimitiveType::U8, 1024),
        );
        let mut store = GuestStore::for_epoch(1).unwrap();
        let (mut frame, body, node) = frame(&db, &view, &mut store);
        let views = [&view];
        let path = path(&db, &view, &frame, &body, node, &views);
        let pc = frame.pc().clone();
        let destination = frame.body.nodes()[node.0 as usize].value.unwrap();
        let image = success(&parent).payload().decode_value().unwrap();
        let mut charges = 0;
        let mut act = {
            let construction = CurrentConstantConstruction::capture_current(
                &db,
                &view,
                &ready,
                success(&parent),
                Some((&child, success(&length))),
                32 << 20,
            )
            .unwrap();
            let binding = construction.bind_guest_descriptor(&mut store).unwrap();
            let proof = AdmittedConstantUse::capture_current(
                &frame,
                destination,
                &view,
                &views,
                &path,
                binding,
                &store,
            )
            .unwrap();
            let prepared = frame
                .prepare_constant_publication(&proof, &image, construction.wire(), &store)
                .unwrap();
            frame
                .begin_constant_publication_chunks(prepared, &mut store, |frame| {
                    charges += 1;
                    frame.fuel.charge();
                    Ok(())
                })
                .unwrap()
        };
        assert_eq!(charges, 1);
        assert!(act.charged_on_entry());
        let mut dispatches = 0;
        let mut published = false;
        for _ in 0..20000 {
            // Construction/descriptor/source use proof are scoped borrows. None
            // is persisted in act; real current DB/dependencies are replayed.
            let construction = CurrentConstantConstruction::capture_current(
                &db,
                &view,
                &ready,
                success(&parent),
                Some((&child, success(&length))),
                32 << 20,
            )
            .unwrap();
            let binding = construction.bind_guest_descriptor(&mut store).unwrap();
            let proof = AdmittedConstantUse::capture_current(
                &frame,
                destination,
                &view,
                &views,
                &path,
                binding,
                &store,
            )
            .unwrap();
            match frame.advance_constant_publication_chunks(&mut act, &proof, &mut store) {
                ConstantPublicationChunk::Dispatch {
                    steps,
                    bytes,
                    cleaning,
                } => {
                    assert!(steps <= 256 && bytes <= 64 << 10 && !cleaning);
                    dispatches += 1;
                    assert!(matches!(
                        store.state(act.cell).unwrap(),
                        crate::runtime::cell::CellState::Uninit
                    ));
                }
                ConstantPublicationChunk::Published => {
                    published = true;
                    break;
                }
                ConstantPublicationChunk::Fault { diagnostic, .. } => {
                    panic!("{}: {}", diagnostic.code, diagnostic.message)
                }
            }
            assert_eq!(frame.pc(), &pc);
        }
        assert!(published && dispatches > 1);
        assert_eq!(frame.pc(), &pc);
        assert_eq!(charges, 1);
        assert_eq!(act.fields_charged(), 1024);
        assert_eq!(frame.fuel.snapshot().used, 1025);
        assert!(act.done);
        let place = Place {
            root: PlaceRoot::Cell(frame.cell(destination).unwrap()),
            projections: Box::new([Projection::Index(33)]),
        };
        let reference = store.borrow(&place, Access::Mutable).unwrap();
        let ticket = store
            .ticket(ValueRepr::unsigned(TypeId(9), 3).unwrap())
            .unwrap();
        let old = store.replace_reference(reference, ticket).unwrap();
        store.discard(old).unwrap();
        assert_eq!(store.read_reference(reference).unwrap().payload, 3);
        assert_eq!(image.elements().unwrap()[33].scalar().unwrap().payload, 0);
        // Root still owns clear-OpActivation/normal return PC/one success event.
    }
}
#[test]
fn source_loss_after_array_dispatch_keeps_cleanup_pending_until_private_owners_drained() {
    let (mut db, view, definition) = world(TEXT, "BIG", 2024);
    let ready = result(&db, &view, definition);
    let (_, d) = ready.array().unwrap();
    let child = ready
        .admission()
        .reselect_array_length_current(&db, &view, d, 32 << 20)
        .unwrap();
    let length = query(&mut db, &view, &child, &d.expected_wire());
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &array(PrimitiveType::U8, 1024),
    );
    let image = success(&parent).payload().decode_value().unwrap();
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (mut frame, body, node) = frame(&db, &view, &mut store);
    let views = [&view];
    let path = path(&db, &view, &frame, &body, node, &views);
    let destination = frame.body.nodes()[node.0 as usize].value.unwrap();
    let pc = frame.pc().clone();
    let mut act = {
        let c = CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&parent),
            Some((&child, success(&length))),
            32 << 20,
        )
        .unwrap();
        let b = c.bind_guest_descriptor(&mut store).unwrap();
        let proof = AdmittedConstantUse::capture_current(
            &frame,
            destination,
            &view,
            &views,
            &path,
            b,
            &store,
        )
        .unwrap();
        let p = frame
            .prepare_constant_publication(&proof, &image, c.wire(), &store)
            .unwrap();
        frame
            .begin_constant_publication_chunks(p, &mut store, |frame| {
                frame.fuel.charge();
                Ok(())
            })
            .unwrap()
    };
    {
        let c = CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&parent),
            Some((&child, success(&length))),
            32 << 20,
        )
        .unwrap();
        let b = c.bind_guest_descriptor(&mut store).unwrap();
        let proof = AdmittedConstantUse::capture_current(
            &frame,
            destination,
            &view,
            &views,
            &path,
            b,
            &store,
        )
        .unwrap();
        assert!(matches!(
            frame.advance_constant_publication_chunks(&mut act, &proof, &mut store),
            ConstantPublicationChunk::Dispatch {
                cleaning: false,
                ..
            }
        ));
    }
    let source_revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), source_revision);
    let actual_source_error = match CurrentConstantConstruction::capture_current(
        &db,
        &view,
        &ready,
        success(&parent),
        Some((&child, success(&length))),
        32 << 20,
    ) {
        Err(error) => error,
        Ok(_) => panic!("changed source epoch unexpectedly admitted"),
    };
    let expected_code = actual_source_error.code;
    frame
        .abort_constant_publication_chunks(&mut act, &mut store, actual_source_error)
        .unwrap();
    assert!(act.cleaning() && !act.done);
    let used = frame.fuel.snapshot().used;
    let mut drained = false;
    for _ in 0..20000 {
        match frame.drain_constant_publication_chunks(&mut act, &mut store) {
            ConstantPublicationChunk::Dispatch {
                steps,
                bytes,
                cleaning,
            } => assert!(steps <= 256 && bytes <= 64 << 10 && cleaning),
            ConstantPublicationChunk::Fault {
                diagnostic,
                cleanup_pending: false,
            } => {
                assert_eq!(diagnostic.code, expected_code);
                drained = true;
                break;
            }
            _ => panic!("source failure became success or unfinished failure"),
        }
    }
    assert!(drained && act.done);
    assert_eq!(frame.pc(), &pc);
    assert_eq!(frame.fuel.snapshot().used, used);
    assert!(matches!(
        store.state(act.cell).unwrap(),
        crate::runtime::cell::CellState::Uninit
    ));
    assert_eq!(frame.source_view_ref(destination).unwrap(), None);
}
#[test]
fn constant_array_entry_callback_must_really_charge_once_before_any_private_owner() {
    let (mut db, view, definition) = world(TEXT, "BIG", 2024);
    let ready = result(&db, &view, definition);
    let (_, d) = ready.array().unwrap();
    let child = ready
        .admission()
        .reselect_array_length_current(&db, &view, d, 32 << 20)
        .unwrap();
    let length = query(&mut db, &view, &child, &d.expected_wire());
    let parent = query(
        &mut db,
        &view,
        ready.admission(),
        &array(PrimitiveType::U8, 1024),
    );
    let image = success(&parent).payload().decode_value().unwrap();
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (mut frame, body, node) = frame(&db, &view, &mut store);
    let views = [&view];
    let path = path(&db, &view, &frame, &body, node, &views);
    let destination = frame.body.nodes()[node.0 as usize].value.unwrap();
    let c = CurrentConstantConstruction::capture_current(
        &db,
        &view,
        &ready,
        success(&parent),
        Some((&child, success(&length))),
        32 << 20,
    )
    .unwrap();
    let b = c.bind_guest_descriptor(&mut store).unwrap();
    let proof =
        AdmittedConstantUse::capture_current(&frame, destination, &view, &views, &path, b, &store)
            .unwrap();
    let p = frame
        .prepare_constant_publication(&proof, &image, c.wire(), &store)
        .unwrap();
    let before = store.budget().handles();
    assert!(
        frame
            .begin_constant_publication_chunks(p, &mut store, |_| Ok(()))
            .is_err()
    );
    assert_eq!(before, store.budget().handles());
    assert_eq!(frame.fuel.snapshot().used, 0);
}
