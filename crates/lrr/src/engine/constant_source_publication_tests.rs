//! Real borrowed source/query/descriptor admission with scoped physical phases.
//! Phase entry is an adapter; this is not automatic dispatch/full fixture execution.
use super::*;
use crate::{
    frontend::{
        constant_construction::{
            CurrentConstantConstruction,
            tests::{array, query, result, success, world},
        },
        constant_path_selection::{ConstantPathOutcome, ConstantPathSelection},
    },
    ir::{
        op::PrimitiveType,
        pc::{NodeId, Phase},
    },
    runtime::{
        place::{Place, PlaceRoot, Projection},
        reference::Access,
        value::ValueRepr,
    },
    syntax::{
        body_index::{BodyIndexLimits, ExpressionKind, NodeRole},
        spans::SourceRange,
    },
    vm::frame::Frame,
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
);
fn frame(
    view: &PreparedSourceView,
    db: &crate::db::memo::Database,
    text: &str,
    store: &mut GuestStore,
) -> (Frame, Vec<NodeId>, crate::syntax::outline::BodyHandle) {
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "caller")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .clone();
    let parsed = db.prepared_parse(&view.selection().outline.parse).unwrap();
    let index = parsed
        .index_body(&body, BodyIndexLimits::default())
        .unwrap();
    let nodes = index
        .nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Path))
        .filter_map(|n| {
            let s = index.span(n.id).unwrap();
            (&text[(body.range.start + s.relative_start) as usize
                ..(body.range.start + s.relative_end) as usize]
                == "ARR")
                .then_some(n.id)
        })
        .collect::<Vec<_>>();
    let frame = Frame::new(
        index.clone(),
        index.pc(nodes[0], Phase::Entry).unwrap(),
        store,
    )
    .unwrap();
    (frame, nodes, body)
}
fn enter(frame: &mut Frame, node: NodeId) {
    frame.pc = frame.body.pc(node, Phase::Entry).unwrap();
    let mut scopes = Vec::new();
    let slot = frame.body.nodes()[node.0 as usize].value.unwrap();
    let mut current = Some(frame.constant_slot_scope_for_test(slot).unwrap());
    while let Some(scope) = current {
        scopes.push(scope);
        current = frame.body.scopes()[scope.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        if !frame.constant_scope_active_for_test(scope) {
            frame.constant_enter_for_test(scope).unwrap();
        }
    }
}
#[test]
fn actual_borrowed_source_publication_joins_real_length_parent_and_two_fresh_temporaries() {
    let text = format!("{ORIGINAL}\nfn caller(){{let mut first=ARR;let mut second=ARR;}}");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(&text, "ARR", edition);
        let ready = result(&db, &view, definition);
        let (_, demand) = ready.array().unwrap();
        let child = ready
            .admission()
            .reselect_array_length_current(&db, &view, demand, 32 << 20)
            .unwrap();
        let length = query(&mut db, &view, &child, &demand.expected_wire());
        let parent = query(
            &mut db,
            &view,
            ready.admission(),
            &array(PrimitiveType::I32, 3),
        );
        let joined = CurrentConstantConstruction::capture_current(
            &db,
            &view,
            &ready,
            success(&parent),
            Some((&child, success(&length))),
            32 << 20,
        )
        .unwrap();
        let mut store = GuestStore::for_epoch(1).unwrap();
        let (mut frame, nodes, body) = frame(&view, &db, &text, &mut store);
        let views = [&view];
        let immutable = success(&parent).payload().decode_value().unwrap();
        let mut cells = Vec::new();
        for (ordinal, node) in nodes.into_iter().enumerate() {
            enter(&mut frame, node);
            let destination = frame.body.nodes()[node.0 as usize].value.unwrap();
            let span = frame.body.span(node).unwrap();
            let range = SourceRange {
                start: body.range.start + span.relative_start,
                end: body.range.start + span.relative_end,
            };
            let ConstantPathOutcome::Selected(path) =
                ConstantPathSelection::capture_current(&db, &view, &body, range, &views, 32 << 20)
                    .unwrap()
            else {
                panic!("original source array member not selected")
            };
            let binding = joined.bind_guest_descriptor(&mut store).unwrap();
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
                .prepare_constant_publication(&proof, &immutable, joined.wire(), &store)
                .unwrap();
            let mut activation = frame
                .begin_constant_publication_chunks(prepared, &mut store, |frame| {
                    frame.fuel.charge();
                    Ok(())
                })
                .unwrap();
            let mut published = false;
            for _ in 0..20000 {
                match frame.advance_constant_publication_chunks(&mut activation, &proof, &mut store)
                {
                    crate::vm::frame::ConstantPublicationChunk::Dispatch {
                        steps,
                        bytes,
                        cleaning,
                    } => assert!(steps <= 256 && bytes <= 64 << 10 && !cleaning),
                    crate::vm::frame::ConstantPublicationChunk::Published => {
                        published = true;
                        break;
                    }
                    crate::vm::frame::ConstantPublicationChunk::Fault { diagnostic, .. } => {
                        panic!("{}: {}", diagnostic.code, diagnostic.message)
                    }
                }
            }
            assert!(published);
            let name = if ordinal == 0 {
                "mut first"
            } else {
                "mut second"
            };
            let pattern = frame
                .body
                .nodes()
                .iter()
                .find(|candidate| {
                    if candidate.role != NodeRole::Pattern {
                        return false;
                    }
                    let span = frame.body.span(candidate.id).unwrap();
                    &text[(body.range.start + span.relative_start) as usize
                        ..(body.range.start + span.relative_end) as usize]
                        == name
                })
                .unwrap()
                .id;
            frame
                .bind(pattern, destination, Some(proof.ty()), true, &mut store)
                .unwrap();
            cells.push(frame.constant_binding_for_test(pattern).unwrap());
            let scope = frame.constant_active_scope_for_test().unwrap();
            assert!(
                frame
                    .begin_scope_cleanup(0, &crate::ir::op::Op::ScopeLeave { scope }, &mut store)
                    .unwrap()
            );
            let mut done = false;
            for _ in 0..1000 {
                if frame.advance_scope_cleanup(&mut store).unwrap() {
                    done = true;
                    break;
                }
            }
            assert!(done);
        }
        assert_eq!(cells.len(), 2);
        assert_ne!(cells[0], cells[1]);
        let place = |cell| Place {
            root: PlaceRoot::Cell(cell),
            projections: Box::new([Projection::Index(0)]),
        };
        let a = store.borrow(&place(cells[0]), Access::Mutable).unwrap();
        let b = store.borrow(&place(cells[1]), Access::Mutable).unwrap();
        let replacement = store
            .ticket(ValueRepr::signed(TypeId(6), 99).unwrap())
            .unwrap();
        let old = store.replace_reference(a, replacement).unwrap();
        store.discard(old).unwrap();
        assert_eq!(store.read_reference(a).unwrap().payload, 99);
        assert_eq!(store.read_reference(b).unwrap().payload, 5);
        assert_eq!(
            immutable.elements().unwrap()[0].scalar().unwrap().payload,
            5
        );
        while let Some(scope) = frame.constant_active_scope_for_test() {
            assert!(
                frame
                    .begin_scope_cleanup(0, &crate::ir::op::Op::ScopeLeave { scope }, &mut store)
                    .unwrap()
            );
            let mut done = false;
            for _ in 0..1000 {
                if frame.advance_scope_cleanup(&mut store).unwrap() {
                    done = true;
                    break;
                }
            }
            assert!(done);
        }
        assert!(store.read_reference(a).is_err() && store.read_reference(b).is_err());
        // Existing full source lowering/caller statements/cleanup dispatch remain
        // a root hook: this bounded join never reports original main as executed.
    }
}
#[test]
fn same_definition_length_query_cannot_substitute_for_named_constant_value() {
    let text = "const ARR:[u8;1]=[25];fn caller(){let value=ARR;}fn main(){}";
    let (mut db, view, definition) = world(text, "ARR", 2024);
    let ready = result(&db, &view, definition);
    let (_, demand) = ready.array().unwrap();
    let child = ready
        .admission()
        .reselect_array_length_current(&db, &view, demand, 32 << 20)
        .unwrap();
    let length = query(&mut db, &view, &child, &demand.expected_wire());
    let child_ready =
        crate::frontend::constant_result_type::CurrentConstantResultType::capture_current(
            &db,
            &view,
            child,
            32 << 20,
        )
        .unwrap();
    let joined = CurrentConstantConstruction::capture_current(
        &db,
        &view,
        &child_ready,
        success(&length),
        None,
        32 << 20,
    )
    .unwrap();
    let mut store = GuestStore::for_epoch(1).unwrap();
    let (mut frame, nodes, body) = frame(&view, &db, text, &mut store);
    enter(&mut frame, nodes[0]);
    let node = nodes[0];
    let span = frame.body.span(node).unwrap();
    let views = [&view];
    let range = SourceRange {
        start: body.range.start + span.relative_start,
        end: body.range.start + span.relative_end,
    };
    let ConstantPathOutcome::Selected(path) =
        ConstantPathSelection::capture_current(&db, &view, &body, range, &views, 32 << 20).unwrap()
    else {
        panic!()
    };
    let binding = joined.bind_guest_descriptor(&mut store).unwrap();
    let before = store.budget().handles();
    let destination = frame.body.nodes()[node.0 as usize].value.unwrap();
    assert!(
        AdmittedConstantUse::capture_current(
            &frame,
            destination,
            &view,
            &views,
            &path,
            binding,
            &store
        )
        .is_err()
    );
    assert_eq!(before, store.budget().handles());
}
