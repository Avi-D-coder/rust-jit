//! Original declarations are unchanged; caller/phase controls are disclosed adapters.
//! No automatic dispatcher or complete upstream fixture is exercised by this file.
use super::*;
use crate::{
    db::poll::InputRequest,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        constant_construction::tests::{result, world},
        constant_path_selection::ConstantPathOutcome,
        constants::ConstLimits,
    },
    runtime::cell::CellState,
    source::manifest::Namespace,
    syntax::{body_index::BodyIndexLimits, outline::BodyHandle, spans::SourceRange},
};
use std::{collections::BTreeSet, sync::Arc};
const MODULE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/module-const-array-size-13763.rs"
);
const NESTED: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-nested.rs");
fn source(text: &str, name: &str, edition: u16) -> (Database, PreparedSourceView, Digest) {
    let (mut db, view, definition) = world(text, name, edition);
    let selected = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.fingerprint() == definition)
        .unwrap()
        .scope
        .clone();
    let scopes = view
        .outline()
        .items
        .iter()
        .map(|i| i.scope.clone())
        .collect::<BTreeSet<_>>();
    for scope in scopes.into_iter().filter(|scope| *scope != selected) {
        for namespace in [Namespace::Value, Namespace::Type] {
            let mut discovery = SourceDiscovery::new(
                vec![view.share()],
                scope.clone(),
                namespace,
                Digest::of_bytes(b"result type current world"),
                1,
            )
            .unwrap()
            .with_declarative_compiler(
                Digest::of_bytes(b"result type declarative"),
                Digest::of_bytes(b"result type compiler"),
                Arc::new(
                    crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                        &view.selection().outline.parse.profile,
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
            loop {
                match discovery.poll().unwrap() {
                    SourceDiscoveryStep::Sealed(_) => break,
                    SourceDiscoveryStep::NeedExpansion { request } => discovery
                        .admit_expansion(Arc::new(request.expand().unwrap()))
                        .unwrap(),
                    SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                        .admit_expansion(Arc::new(request.expand().unwrap()))
                        .unwrap(),
                    _ => {
                        panic!("scalar source requires unestablished source preparation authority")
                    }
                }
            }
            discovery.publish(&mut db).unwrap();
        }
    }
    (db, view, definition)
}
fn source_frame(
    db: &Database,
    view: &PreparedSourceView,
    text: &str,
    needle: &str,
    store: &mut GuestStore,
) -> (Frame, BodyHandle, SourceRange) {
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "scalar_use")
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
            let span = index.span(node.id).unwrap();
            &text[(body.range.start + span.relative_start) as usize
                ..(body.range.start + span.relative_end) as usize]
                == needle
        })
        .unwrap();
    let span = index.span(node.id).unwrap();
    let range = SourceRange {
        start: body.range.start + span.relative_start,
        end: body.range.start + span.relative_end,
    };
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let mut frame = Frame::new(index.clone(), pc, store).unwrap();
    let mut current = Some(
        frame
            .constant_slot_scope_for_test(node.value.unwrap())
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
    (frame, body, range)
}
fn selected_path(
    db: &Database,
    view: &PreparedSourceView,
    body: &BodyHandle,
    range: SourceRange,
    views: &[&PreparedSourceView],
) -> ConstantPathSelection {
    let ConstantPathOutcome::Selected(path) =
        ConstantPathSelection::capture_current(db, view, body, range, views, MAX_BYTES).unwrap()
    else {
        panic!("original source constant path unresolved")
    };
    *path
}
fn ready<'a>(poll: ScalarUsePreparation<'a>) -> PreparedScalarConstantUse<'a> {
    match poll {
        ScalarUsePreparation::Ready(Ok(value)) => value,
        ScalarUsePreparation::Ready(Err(error)) => panic!("{}: {}", error.code, error.message),
        ScalarUsePreparation::NeedInput(_) => panic!("complete source scalar suspended"),
    }
}

#[test]
fn store_session_actual_source_scalar_site_refuses_foreign_store_before_query() {
    let text = format!("{NESTED}\nfn scalar_use(){{let actual=X;}}");
    let (mut db, view, definition) = source(&text, "X", 2024);
    let result = result(&db, &view, definition);
    let mut sa = GuestStore::new(1, 10000, 4 << 20).unwrap();
    let mut sb = GuestStore::new(1, 10000, 4 << 20).unwrap();
    let (a, body, range) = source_frame(&db, &view, &text, "X", &mut sa);
    let (b, _, _) = source_frame(&db, &view, &text, "X", &mut sb);
    assert_eq!(a.constant_frame_anchor(), b.constant_frame_anchor());
    let views = [&view];
    let path = selected_path(&db, &view, &body, range, &views);
    let sources = ScalarUseSources {
        caller: &view,
        views: &views,
        target: &view,
        result: &result,
        path: &path,
    };
    let site = ScalarConstantUseSite::capture_current(&db, &a, &sa, sources, MAX_BYTES).unwrap();
    let before = (
        db.trace.events.len(),
        sb.budget().handles(),
        sb.budget().bytes(),
        b.fuel.snapshot().used,
    );
    assert!(
        matches!(ScalarConstantUseSite::capture_current(&db, &a, &sb, sources, MAX_BYTES), Err(e) if e.code == "LRR-E-STALE-HANDLE")
    );
    assert!(
        matches!(site.fetch_current(&mut db, &b, &sb, ConstLimits::default()), QueryPoll::Ready(Err(e)) if e.code == "LRR-E-CONST-USE")
    );
    assert_eq!(
        before,
        (
            db.trace.events.len(),
            sb.budget().handles(),
            sb.budget().bytes(),
            b.fuel.snapshot().used
        )
    );
}
#[test]
fn unchanged_module_bits_and_nested_x_scalar_recipe_publish_with_one_charge_all_editions() {
    for (original, name, needle, expected) in
        [(MODULE, "BITS", "u8::BITS", 8), (NESTED, "X", "X", 22)]
    {
        let text = format!("{original}\nfn scalar_use(){{let actual={needle};}}");
        for edition in [2015, 2018, 2021, 2024] {
            let (mut db, view, definition) = source(&text, name, edition);
            let result = result(&db, &view, definition);
            let mut store = GuestStore::new(1, 10000, 4 << 20).unwrap();
            let (mut frame, body, range) = source_frame(&db, &view, &text, needle, &mut store);
            let views = [&view];
            let path = selected_path(&db, &view, &body, range, &views);
            let site = ScalarConstantUseSite::capture_current(
                &db,
                &frame,
                &store,
                ScalarUseSources {
                    caller: &view,
                    views: &views,
                    target: &view,
                    result: &result,
                    path: &path,
                },
                MAX_BYTES,
            )
            .unwrap();
            let query = site.fetch_current(&mut db, &frame, &store, ConstLimits::default());
            let before = (
                store.budget().handles(),
                frame.fuel.snapshot().used,
                db.trace.events.len(),
            );
            let recipe = ready(site.accept_poll(&db, &frame, &store, &query));
            assert_eq!(
                before,
                (
                    store.budget().handles(),
                    frame.fuel.snapshot().used,
                    db.trace.events.len()
                )
            );
            let mut charges = 0;
            recipe
                .publish_with_opcode_charge(&mut frame, &mut store, |frame| {
                    charges += 1;
                    frame.fuel.charge()
                })
                .unwrap();
            assert_eq!(charges, 1);
            assert_eq!(frame.fuel.snapshot().used, 1);
            let CellState::Live(value) =
                store.state(frame.cell(site.destination).unwrap()).unwrap()
            else {
                panic!("scalar result not published")
            };
            assert_eq!(value.payload, expected);
            assert_eq!(
                frame.source_view_ref(site.destination).unwrap(),
                Some(recipe.construction.source_term())
            );
            let mut duplicate_charges = 0;
            assert!(
                recipe
                    .publish_with_opcode_charge(&mut frame, &mut store, |frame| {
                        duplicate_charges += 1;
                        frame.fuel.charge()
                    })
                    .is_err()
            );
            assert_eq!(duplicate_charges, 0);
            // Source query + physical publication only. Original main's assert,
            // struct/array and complete interpreter continuations remain unexecuted.
        }
    }
}
#[test]
fn scalar_adapter_transport_only_needinput_and_readyerr_preserve_no_value_state() {
    let text = "const X:u32=22;fn scalar_use(){let value=X;}fn main(){}";
    let (db, view, definition) = source(text, "X", 2024);
    let result = result(&db, &view, definition);
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    let (frame, body, range) = source_frame(&db, &view, text, "X", &mut store);
    let views = [&view];
    let path = selected_path(&db, &view, &body, range, &views);
    let site = ScalarConstantUseSite::capture_current(
        &db,
        &frame,
        &store,
        ScalarUseSources {
            caller: &view,
            views: &views,
            target: &view,
            result: &result,
            path: &path,
        },
        MAX_BYTES,
    )
    .unwrap();
    // Disclosed transport control: current scalar provider is input-complete.
    // This typed demand is not claimed as a real constant-query suspension.
    let query = QueryPoll::NeedInput(InputRequest::CaptureCompileEnvironment {
        unit: view.selection().outline.crate_key,
    });
    let before = (
        frame.pc().clone(),
        store.budget().handles(),
        frame.fuel.snapshot().used,
        db.trace.events.len(),
    );
    let ScalarUsePreparation::NeedInput(input) = site.accept_poll(&db, &frame, &store, &query)
    else {
        panic!()
    };
    let QueryPoll::NeedInput(original) = &query else {
        panic!()
    };
    assert!(std::ptr::eq(input, original));
    let error = QueryPoll::Ready(Err(Diagnostic::new(
        "LRR-E-SCALAR-TRANSPORT-CONTROL",
        "exact error".into(),
    )));
    let ScalarUsePreparation::Ready(Err(actual)) = site.accept_poll(&db, &frame, &store, &error)
    else {
        panic!()
    };
    assert_eq!(actual.code, "LRR-E-SCALAR-TRANSPORT-CONTROL");
    assert_eq!(actual.message, "exact error");
    assert_eq!(
        before,
        (
            frame.pc().clone(),
            store.budget().handles(),
            frame.fuel.snapshot().used,
            db.trace.events.len()
        )
    );
}
#[test]
fn genuine_cached_scalar_numeric_failure_remains_exact_diagnostic_without_recipe() {
    let text = "const BAD:u8=1u8/0u8;fn scalar_use(){let value=BAD;}fn main(){}";
    let (mut db, view, definition) = source(text, "BAD", 2024);
    let result = result(&db, &view, definition);
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    let (frame, body, range) = source_frame(&db, &view, text, "BAD", &mut store);
    let views = [&view];
    let path = selected_path(&db, &view, &body, range, &views);
    let site = ScalarConstantUseSite::capture_current(
        &db,
        &frame,
        &store,
        ScalarUseSources {
            caller: &view,
            views: &views,
            target: &view,
            result: &result,
            path: &path,
        },
        MAX_BYTES,
    )
    .unwrap();
    let query = site.fetch_current(&mut db, &frame, &store, ConstLimits::default());
    let QueryPoll::Ready(Ok(outcome)) = &query else {
        match &query {
            QueryPoll::Ready(Err(error)) => panic!(
                "real constant query refused: {} {}",
                error.code, error.message
            ),
            QueryPoll::NeedInput(request) => panic!("real constant query suspended: {:?}", request),
            _ => unreachable!("matched Ready success"),
        }
    };
    let QueryValue::ConstantFailure(failure) = outcome.value.as_ref() else {
        panic!("numeric failure was not typed")
    };
    let before = (
        store.budget().handles(),
        frame.fuel.snapshot().used,
        db.trace.events.len(),
    );
    let ScalarUsePreparation::Ready(Err(actual)) = site.accept_poll(&db, &frame, &store, &query)
    else {
        panic!("cached failure became a value")
    };
    assert_eq!(actual.code, failure.diagnostic().code);
    assert_eq!(actual.message, failure.diagnostic().message);
    assert_eq!(
        before,
        (
            store.budget().handles(),
            frame.fuel.snapshot().used,
            db.trace.events.len()
        )
    );
}
#[test]
fn current_other_scalar_query_cannot_fill_selected_constant_recipe() {
    let text = "const A:u8=25;const B:u8=26;fn scalar_use(){let first=A;let second=B;}fn main(){}";
    let (mut db, view, definition) = source(text, "A", 2024);
    let selected = result(&db, &view, definition);
    let b = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "B")
        .unwrap()
        .def_key
        .fingerprint();
    let other = result(&db, &view, b);
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    let (frame, body, range) = source_frame(&db, &view, text, "A", &mut store);
    let views = [&view];
    let path = selected_path(&db, &view, &body, range, &views);
    let site = ScalarConstantUseSite::capture_current(
        &db,
        &frame,
        &store,
        ScalarUseSources {
            caller: &view,
            views: &views,
            target: &view,
            result: &selected,
            path: &path,
        },
        MAX_BYTES,
    )
    .unwrap();
    let (other_frame, other_body, other_range) = source_frame(&db, &view, text, "B", &mut store);
    let other_path = selected_path(&db, &view, &other_body, other_range, &views);
    let other_site = ScalarConstantUseSite::capture_current(
        &db,
        &other_frame,
        &store,
        ScalarUseSources {
            caller: &view,
            views: &views,
            target: &view,
            result: &other,
            path: &other_path,
        },
        MAX_BYTES,
    )
    .unwrap();
    let query = other_site.fetch_current(&mut db, &other_frame, &store, ConstLimits::default());
    assert!(matches!(
        site.accept_poll(&db, &frame, &store, &query),
        ScalarUsePreparation::Ready(Err(_))
    ));
    assert_eq!(frame.fuel.snapshot().used, 0);
}
#[test]
fn scalar_site_source_query_revision_and_worker_epoch_require_fresh_capture() {
    for change_source in [false, true] {
        let text = "const X:u32=22;fn scalar_use(){let value=X;}fn main(){}";
        let (mut db, view, definition) = source(text, "X", 2024);
        let result = result(&db, &view, definition);
        let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
        let (frame, body, range) = source_frame(&db, &view, text, "X", &mut store);
        let views = [&view];
        let path = selected_path(&db, &view, &body, range, &views);
        let site = ScalarConstantUseSite::capture_current(
            &db,
            &frame,
            &store,
            ScalarUseSources {
                caller: &view,
                views: &views,
                target: &view,
                result: &result,
                path: &path,
            },
            MAX_BYTES,
        )
        .unwrap();
        let query = site.fetch_current(&mut db, &frame, &store, ConstLimits::default());
        if change_source {
            let revision = db.source_revision();
            db.begin_expansion_epoch(2).unwrap();
            assert_eq!(db.source_revision(), revision);
        } else {
            store.reset(2).unwrap();
        }
        assert!(matches!(
            site.accept_poll(&db, &frame, &store, &query),
            ScalarUsePreparation::Ready(Err(_))
        ));
        let trace = db.trace.events.len();
        assert!(matches!(
            site.fetch_current(&mut db, &frame, &store, ConstLimits::default()),
            QueryPoll::Ready(Err(_))
        ));
        assert_eq!(trace, db.trace.events.len());
    }
}

#[test]
fn scalar_site_foreign_frame_and_changed_pc_refuse_before_opcode_charge() {
    let text = "const X:u32=22;fn scalar_use(){let value=X;}fn main(){}";
    let (mut db, view, definition) = source(text, "X", 2024);
    let result = result(&db, &view, definition);
    let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
    let (mut frame, body, range) = source_frame(&db, &view, text, "X", &mut store);
    let views = [&view];
    let path = selected_path(&db, &view, &body, range, &views);
    let site = ScalarConstantUseSite::capture_current(
        &db,
        &frame,
        &store,
        ScalarUseSources {
            caller: &view,
            views: &views,
            target: &view,
            result: &result,
            path: &path,
        },
        MAX_BYTES,
    )
    .unwrap();
    let query = site.fetch_current(&mut db, &frame, &store, ConstLimits::default());
    let foreign = Frame::new(frame.body.clone(), frame.pc().clone(), &mut store).unwrap();
    assert!(matches!(
        site.accept_poll(&db, &foreign, &store, &query),
        ScalarUsePreparation::Ready(Err(_))
    ));
    frame.pc.phase = Phase::Commit.code().unwrap();
    assert!(matches!(
        site.accept_poll(&db, &frame, &store, &query),
        ScalarUsePreparation::Ready(Err(_))
    ));
    assert_eq!(frame.fuel.snapshot().used, 0);
}
#[test]
fn scalar_site_metadata_and_array_readiness_refuse_without_query_or_guest_work() {
    for (text, name, needle) in [
        (
            "const X:u32=22;fn scalar_use(){let value=X;}fn main(){}",
            "X",
            "X",
        ),
        (
            "const A:[u8;1]=[25];fn scalar_use(){let value=A;}fn main(){}",
            "A",
            "A",
        ),
    ] {
        let (db, view, definition) = source(text, name, 2024);
        let result = result(&db, &view, definition);
        let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
        let (frame, body, range) = source_frame(&db, &view, text, needle, &mut store);
        let views = [&view];
        let path = selected_path(&db, &view, &body, range, &views);
        let before = (store.budget().handles(), db.trace.events.len());
        assert!(
            ScalarConstantUseSite::capture_current(
                &db,
                &frame,
                &store,
                ScalarUseSources {
                    caller: &view,
                    views: &views,
                    target: &view,
                    result: &result,
                    path: &path
                },
                if name == "X" { 1 } else { MAX_BYTES }
            )
            .is_err()
        );
        assert_eq!(before, (store.budget().handles(), db.trace.events.len()));
    }
}
#[test]
fn scalar_charge_cancellation_or_exhaustion_never_publishes_reserved_value() {
    for cancel in [false, true] {
        let text = "const X:u32=22;fn scalar_use(){let value=X;}fn main(){}";
        let (mut db, view, definition) = source(text, "X", 2024);
        let result = result(&db, &view, definition);
        let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
        let (mut frame, body, range) = source_frame(&db, &view, text, "X", &mut store);
        let views = [&view];
        let path = selected_path(&db, &view, &body, range, &views);
        let site = ScalarConstantUseSite::capture_current(
            &db,
            &frame,
            &store,
            ScalarUseSources {
                caller: &view,
                views: &views,
                target: &view,
                result: &result,
                path: &path,
            },
            MAX_BYTES,
        )
        .unwrap();
        let query = site.fetch_current(&mut db, &frame, &store, ConstLimits::default());
        let recipe = ready(site.accept_poll(&db, &frame, &store, &query));
        let before = store.budget().handles();
        let mut charges = 0;
        if !cancel {
            frame.fuel = crate::vm::fuel::RunFuel::new(0).unwrap();
        }
        assert!(
            recipe
                .publish_with_opcode_charge(&mut frame, &mut store, |frame| {
                    charges += 1;
                    if cancel {
                        frame.fuel.cancellation().cancel();
                    }
                    frame.fuel.charge()
                })
                .is_err()
        );
        assert_eq!(charges, 1);
        assert_eq!(before, store.budget().handles());
        assert!(frame.cell(site.destination).is_err());
        assert_eq!(frame.source_view_ref(site.destination).unwrap(), None);
    }
}
