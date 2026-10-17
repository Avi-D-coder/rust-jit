//! Admission controls execute actual ordinary caller syntax, not fabricated PCs.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{key::OutlineKey, provider::SyntaxProvider, value::QueryValue},
    frontend::call_contracts::ClosedReferenceArgument,
    ir::recipe::VerifiedRecipe,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{body_index::NodeRole, parse::ParseKey},
    traits::term::{RegionTerm, TyTerm},
    vm::{
        call::CallStartFacts,
        step::{StepOutcome, step},
    },
};
struct Caller {
    db: Database,
    source: PreparedSourceView,
    callee: Box<SourceCallable>,
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    slot: SlotId,
    hint: ArgumentHint,
}
fn reached_call(text: &str, edition: u16) -> Caller {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let unit = SourceUnitInput {
        path: SourcePath::new("reference-local-source.rs").unwrap(),
        crate_key: Digest([199; 32]),
        module_path: vec![],
        profile: profile.clone(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    };
    let selection = SourceSelection {
        path: unit.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: unit.blob.content(),
                profile,
            },
            crate_key: unit.crate_key,
            module_path: vec![],
            cfg: unit.cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        vec![unit],
        Default::default(),
        vec![],
    )
    .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let SourceSelectionOutcome::Selected(source) = inputs.select(&selection).unwrap() else {
        panic!()
    };
    let main = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap();
    let definition = main.def_key.fingerprint();
    let body = main.body.as_ref().unwrap().key();
    let mut graph = NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let imports = graph.resolve_imports().unwrap();
    assert!(imports.pending.is_empty() && imports.missing.is_empty());
    let views = [&source];
    let resolver = CallResolver::new(&graph, &views).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, definition, "take")
    else {
        panic!()
    };
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
        panic!()
    };
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(
        index.as_ref().clone(),
        index.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    for _ in 0..128 {
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!()
        };
        let slot = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::CallPrepare { arguments, .. } => Some(arguments[0]),
            _ => None,
        });
        if let Some(slot) = slot {
            let QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(mut hints))) =
                prepare_argument_hint(&mut db, &callee, &graph, &[&source], 0)
            else {
                panic!()
            };
            assert_eq!(hints.len(), 1);
            let hint = hints.pop().unwrap();
            return Caller {
                db,
                source,
                callee,
                frame,
                store,
                recipe: recipe.as_ref().clone(),
                slot,
                hint,
            };
        }
        assert!(matches!(
            step(&mut frame, &mut store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("actual ordinary caller did not reach call prepare")
}
fn borrow_argument(c: &mut Caller) -> Option<TyTerm> {
    let (descriptor, source) = runtime_hint(
        &mut c.db,
        &c.frame,
        &mut c.store,
        &c.callee,
        &c.hint,
        0,
        c.slot,
    )
    .unwrap();
    let installed = source.clone();
    let site = NodeId(c.frame.pc().node);
    let ordinary = c
        .callee
        .ordinary_call_witness(&mut c.db, c.frame.body.body().clone(), site)
        .unwrap();
    c.frame.call_start = Some(CallStartFacts {
        ordinary,
        site: site.0,
        hints: vec![(c.slot, descriptor, source)],
        partial: Vec::new(),
    });
    assert!(matches!(
        step(&mut c.frame, &mut c.store, &c.recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    for _ in 0..128 {
        let fetched =
            c.db.fetch(&SyntaxProvider, QueryKey::Thunk(c.frame.pc().clone()))
                .into_ready()
                .unwrap()
                .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!()
        };
        let is_borrow = recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op,Op::Borrow{destination,..}if *destination==c.slot));
        assert!(matches!(
            step(&mut c.frame, &mut c.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        if is_borrow {
            return installed;
        }
    }
    panic!("actual ordinary argument did not execute its borrow")
}
#[test]
fn genuine_source_present_local_and_literal_temporary_keep_caller_region_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, temporary, primitive) in [
            (
                "fn take(x:&bool){}fn main(){let x=true;take(&x);}",
                false,
                crate::ir::op::PrimitiveType::Bool,
            ),
            (
                "fn take(x:&isize){}fn main(){take(&20);}",
                true,
                crate::ir::op::PrimitiveType::Isize,
            ),
        ] {
            let mut c = reached_call(text, edition);
            assert_eq!(c.source.selection().outline.parse.profile.edition, edition);
            let installed = borrow_argument(&mut c);
            assert_eq!(installed.is_some(), temporary);
            let source = c.frame.source_view_ref(c.slot).unwrap().unwrap();
            assert!(
                matches!(source,TyTerm::Ref(RegionTerm::Local{definition,..},false,target)
                if *definition==c.frame.body.body().definition && target.as_ref()==&TyTerm::Primitive(primitive))
            );
            ClosedReferenceArgument::capture(&c.frame, c.slot, &c.store).unwrap();
        }
    }
}
#[test]
fn actual_primitive_local_with_missing_source_cannot_gain_callee_reference_identity() {
    let mut c = reached_call("fn take(x:&bool){}fn main(){let x=true;take(&x);}", 2024);
    let binding = c
        .frame
        .body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    let cell = c.frame.binding_cell_for_test(binding).unwrap();
    let physical = c.store.admit_move(cell).unwrap();
    c.frame.clear_binding_source_for_test(binding).unwrap();
    assert!(borrow_argument(&mut c).is_none());
    assert_eq!(c.store.admit_move(cell).unwrap(), physical);
    let reference = c.frame.cell(c.slot).unwrap();
    c.store.reference_from_cell(reference).unwrap();
    assert!(c.frame.source_view_ref(c.slot).unwrap().is_none());
    let before = c.store.owner_inventory_for_test();
    assert_eq!(
        ClosedReferenceArgument::capture(&c.frame, c.slot, &c.store)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    assert_eq!(c.store.owner_inventory_for_test(), before);
    c.store.reference_from_cell(reference).unwrap();
}
