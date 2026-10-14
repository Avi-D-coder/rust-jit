//! Source-selected barriers from unchanged stock lifetime tests. Whole bodies,
//! nominal return execution and rustc's compile-fail diagnostics remain pending.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    frontend::{
        callables::{CallResolver, CallSelection, SourceCallable},
        names::NameNamespaces,
    },
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        outline::BodyHandle,
        parse::{ParseKey, ParsedFile},
        scope_fixture_facts,
    },
    vm::{
        call::CallStartFacts,
        step::{StepOutcome, step},
    },
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/rvalue-borrow-scope-error.rs"
);
const TUPLE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/lifetimes/temporary-lifetime-extension-tuple-ctor.rs"
);
const DROP_DECLARATION: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-describe-field-generic-param-owned-box.rs"
);
struct Context {
    inputs: PreparedSourceInputs,
    view: PreparedSourceView,
    sdk: PreparedSourceView,
    db: Database,
    parsed: ParsedFile,
    handle: BodyHandle,
    frame: Frame,
    store: GuestStore,
    outer: NodeId,
    borrow: NodeId,
    operand: NodeId,
    outer_name: String,
    operand_name: String,
}
fn context(original: &str, binding: &str) -> Context {
    let facts = scope_fixture_facts::original_call_scope(original, binding);
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(original.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("stock.rs").unwrap();
    let unit = Digest::of_bytes(b"stock-call-temporary-scope");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
    // V5's missing producer was the original std::ops::Drop import. Supply a
    // separate captured declaration facet, without editing the original case
    // or manufacturing a completed import/seal. Runtime Drop remains untested.
    let (sdk_source, sdk_range) =
        scope_fixture_facts::original_drop_declaration_facet(DROP_DECLARATION);
    let original_sdk = &DROP_DECLARATION[sdk_range.start as usize..sdk_range.end as usize];
    assert!(sdk_source.contains(original_sdk));
    let sdk_blob = SourceBlob::new(sdk_source.as_bytes().to_vec()).unwrap();
    let sdk_unit = Digest::of_bytes(sdk_source.as_bytes());
    let sdk_path = SourcePath::new("stock-sdk-declaration.rs").unwrap();
    let sdk_selection = SourceSelection {
        path: sdk_path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: sdk_blob.content(),
                profile: profile.clone(),
            },
            crate_key: sdk_unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![
            SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile: profile.clone(),
                cfg: cfg.clone(),
                blob,
            },
            SourceUnitInput {
                path: sdk_path,
                crate_key: sdk_unit,
                module_path: vec![],
                profile,
                cfg,
                blob: sdk_blob,
            },
        ],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("stock source missing")
    };
    let SourceSelectionOutcome::Selected(sdk) = inputs.select(&sdk_selection).unwrap() else {
        panic!("stock SDK declaration missing")
    };
    let handle = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let body = view
        .index_body(view.selection(), &handle.key(), Default::default())
        .unwrap();
    let id = |range| {
        body.nodes()
            .iter()
            .find(|node| node.range == range)
            .unwrap()
            .id
    };
    let (outer, borrow, operand) = (id(facts.outer), id(facts.borrow), id(facts.operand));
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(
        body.clone(),
        body.pc(outer, Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    frame.set_fuel_limit(100).unwrap();
    let mut scopes = vec![];
    let mut scope = Some(body.nodes()[outer.0 as usize].scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    Context {
        inputs,
        view,
        sdk,
        db,
        parsed,
        handle,
        frame,
        store,
        outer,
        borrow,
        operand,
        outer_name: facts.outer_name,
        operand_name: facts.operand_name,
    }
}
fn select(c: &mut Context, name: &str) -> SourceCallable {
    let mut graph = NameNamespaces::from_outlines(&[c.view.outline(), c.sdk.outline()]).unwrap();
    graph
        .dependency(
            c.view.outline().root_scope.crate_key,
            "std",
            c.sdk.outline().root_scope.crate_key,
            2024,
        )
        .unwrap();
    let imports = graph.resolve_imports().unwrap();
    assert!(
        imports.pending.is_empty()
            && imports.missing.is_empty()
            && imports.inaccessible.is_empty()
            && imports.ambiguous.is_empty(),
        "stock import resolution incomplete: {imports:?}"
    );
    let expected = c
        .view
        .outline()
        .imports
        .iter()
        .map(NameNamespaces::import_identity)
        .collect::<Vec<_>>();
    assert_eq!(imports.resolved, expected);
    let sources = [&c.view, &c.sdk];
    match CallResolver::new(&graph, &sources)
        .unwrap()
        .module_function(&mut c.db, c.frame.body.body().definition, name)
    {
        QueryPoll::Ready(Ok(CallSelection::Source(callable))) => *callable,
        other => panic!("stock source function did not select: {other:?}"),
    }
}
fn recipe(c: &Context) -> VerifiedRecipe {
    c.parsed
        .initial_thunk(&c.handle, &c.frame.body, c.frame.pc())
        .unwrap()
}
fn prepare(c: &mut Context) -> VerifiedRecipe {
    let name = c.outer_name.clone();
    let callee = select(c, &name);
    let ordinary = callee
        .ordinary_call_witness(&mut c.db, c.frame.body.body().clone(), c.outer)
        .unwrap();
    let r = recipe(c);
    let arguments = r
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::CallPrepare {
                site, arguments, ..
            } if site.0 == c.outer.0 => Some(arguments),
            _ => None,
        })
        .unwrap();
    c.frame.call_start = Some(CallStartFacts {
        ordinary,
        site: c.outer.0,
        hints: arguments.iter().map(|slot| (*slot, None, None)).collect(),
    });
    r
}
fn assert_barrier(binding: &str) {
    let mut c = context(ORIGINAL, binding);
    let r = prepare(&mut c);
    assert!(c.frame.ordinary_calls.iter().all(Option::is_none));
    let node = &c.frame.body.nodes()[c.operand.0 as usize];
    let extension = node.extended_to.as_ref().unwrap();
    assert_eq!(extension.constructors, [c.outer]);
    let destination = node.value.unwrap();
    let actual_scope = c.frame.slots[destination.0 as usize].scope;
    assert_ne!(actual_scope, extension.scope);
    assert!(matches!(
        step(&mut c.frame, &mut c.store, &r).unwrap(),
        StepOutcome::Next(_)
    ));
    assert!(c.frame.ordinary_calls[c.outer.0 as usize].is_some());
    assert_eq!(c.frame.pc().node, c.borrow.0);
    let r = recipe(&c);
    assert!(matches!(
        step(&mut c.frame, &mut c.store, &r).unwrap(),
        StepOutcome::Next(_)
    ));
    assert_eq!(c.frame.pc().node, c.operand.0);
    assert_eq!(c.frame.slots[destination.0 as usize].scope, actual_scope);
    assert!(c.frame.cell(destination).is_err());
    assert_eq!(c.frame.counters.started_by_opcode[31], 0);
    c.frame.preflight_temporary_slot(destination).unwrap();
}
#[test]
fn rustc_ordinary_call_blocks_borrowed_argument_extension_before_operand_execution() {
    assert_barrier("x1");
}
#[test]
fn rustc_extending_reference_pattern_does_not_cross_ordinary_call() {
    assert_barrier("x5");
}
#[test]
fn rustc_unselected_call_refuses_before_borrow_or_value_publication() {
    let mut c = context(ORIGINAL, "x1");
    c.frame.pc = c.frame.body.pc(c.borrow, Phase::Entry).unwrap();
    let r = recipe(&c);
    let handles = c.store.budget().handles();
    let fuel = c.frame.fuel_snapshot().used;
    let destination = c.frame.body.nodes()[c.operand.0 as usize].value.unwrap();
    assert_eq!(
        c.frame
            .preflight_temporary_slot(destination)
            .unwrap_err()
            .code,
        "LRR-E-TEMPORARY-SCOPE"
    );
    assert_eq!(
        step(&mut c.frame, &mut c.store, &r).unwrap_err().code,
        "LRR-E-TEMPORARY-SCOPE"
    );
    assert_eq!(c.frame.status(), FrameStatus::Faulted);
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.fuel_snapshot().used, fuel);
    assert_eq!(c.frame.counters.operations_started, 0);
    assert!(c.frame.cell(destination).is_err());
}
#[test]
fn rustc_constructor_value_frontier_requires_selected_identity_before_borrow() {
    let mut c = context(TUPLE, "g");
    assert_eq!(c.outer_name, "some"); // original Fn-value alias, not a constructor proof
    c.frame.pc = c.frame.body.pc(c.borrow, Phase::Entry).unwrap();
    let r = recipe(&c);
    let handles = c.store.budget().handles();
    assert_eq!(
        step(&mut c.frame, &mut c.store, &r).unwrap_err().code,
        "LRR-E-TEMPORARY-SCOPE"
    );
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.counters.operations_started, 0);
}
#[test]
fn rustc_uppercase_function_witness_requires_exact_caller_site_and_current_source() {
    let mut c = context(ORIGINAL, "x1");
    let name = c.operand_name.clone();
    let callee = select(&mut c, &name);
    assert_eq!(callee.definition().declared_name, "AddFlags");
    assert_eq!(
        callee.definition().item_kind,
        crate::source::manifest::ItemKind::Function
    );
    let caller = c.frame.body.body().clone();
    let witness = callee
        .ordinary_call_witness(&mut c.db, caller.clone(), c.operand)
        .unwrap();
    assert_eq!(
        witness.require(&caller, c.operand).unwrap(),
        callee.identity()
    );
    assert_eq!(
        witness.require(&caller, c.outer).unwrap_err().code,
        "LRR-E-CALL-CONTEXT"
    );
    let mut foreign = caller.clone();
    foreign.exact_body_bytes = Digest::of_bytes(b"foreign-body");
    assert_eq!(
        witness.require(&foreign, c.operand).unwrap_err().code,
        "LRR-E-CALL-CONTEXT"
    );
    assert_eq!(
        callee
            .ordinary_call_witness(&mut c.db, foreign, c.operand)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    c.db.publish_inputs(Default::default()).unwrap();
    assert_eq!(
        callee
            .ordinary_call_witness(&mut c.db, caller, c.operand)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    // The original source owner remains retained; no test edits its bytes.
    assert!(c.inputs.select(c.view.selection()).is_ok());
}
#[test]
fn rustc_call_barrier_is_not_published_on_fuel_refusal() {
    let mut c = context(ORIGINAL, "x1");
    let r = prepare(&mut c);
    c.frame.set_fuel_limit(0).unwrap();
    let handles = c.store.budget().handles();
    assert_eq!(
        step(&mut c.frame, &mut c.store, &r).unwrap_err().code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert!(c.frame.ordinary_calls.iter().all(Option::is_none));
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.counters.operations_started, 0);
}
#[test]
fn rustc_call_barrier_is_not_published_on_cancellation() {
    let mut c = context(ORIGINAL, "x1");
    let r = prepare(&mut c);
    c.frame.fuel.cancellation().cancel();
    let handles = c.store.budget().handles();
    assert_eq!(
        step(&mut c.frame, &mut c.store, &r).unwrap_err().code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert!(c.frame.ordinary_calls.iter().all(Option::is_none));
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.counters.operations_started, 0);
}

#[test]
fn rustc_pending_drop_import_suspends_call_selection_until_source_facet_resolution() {
    let mut c = context(ORIGINAL, "x1");
    let graph = NameNamespaces::from_outlines(&[c.view.outline(), c.sdk.outline()]).unwrap();
    let sources = [&c.view, &c.sdk];
    let expected = NameNamespaces::import_identity(&c.view.outline().imports[0]);
    let handles = c.store.budget().handles();
    let fuel = c.frame.fuel_snapshot().used;
    assert!(
        matches!(CallResolver::new(&graph, &sources).unwrap().module_function(
        &mut c.db, c.frame.body.body().definition, &c.outer_name),
        QueryPoll::Ready(Ok(CallSelection::NeedProducer(producer))) if producer == expected)
    );
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.fuel_snapshot().used, fuel);
    assert!(c.frame.ordinary_calls.iter().all(Option::is_none));
    assert!(c.db.trace.events.iter().all(|event| !matches!(
        event.key,
        crate::db::key::QueryKey::FunctionSignature(_)
            | crate::db::key::QueryKey::BodyStructure(_)
            | crate::db::key::QueryKey::Thunk(_)
    )));
    let name = c.outer_name.clone();
    let selected = select(&mut c, &name);
    assert_eq!(selected.definition().declared_name, name);
    let witness = selected
        .ordinary_call_witness(&mut c.db, c.frame.body.body().clone(), c.outer)
        .unwrap();
    assert_eq!(
        witness.require(c.frame.body.body(), c.outer).unwrap(),
        selected.identity()
    );
    assert_eq!(c.store.budget().handles(), handles);
    assert_eq!(c.frame.fuel_snapshot().used, fuel);
    assert!(c.frame.ordinary_calls.iter().all(Option::is_none));
}
