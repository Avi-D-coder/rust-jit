//! Actual source/caller borrow admission; authority mutations follow positives.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        call_contracts::{
            ArgumentType, ClosedReferenceArgument, ContractPreparation, ReferenceCallArguments,
            prepare_contracts_with_references,
        },
        callables::{CallResolver, CallSelection},
    },
    ir::{
        op::{Op, PrimitiveType},
        pc::{NodeId, Phase, SlotId},
    },
    runtime::cell::GuestStore,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
    vm::{
        frame::Frame,
        step::{StepOutcome, step},
    },
};
struct World {
    db: Database,
    source: PreparedSourceView,
    graph: crate::frontend::names::NameNamespaces,
    callee: Box<SourceCallable>,
    packet: PreparedCallContracts,
    frame: Frame,
    store: GuestStore,
    slot: SlotId,
}
fn callable(
    db: &mut Database,
    source: &PreparedSourceView,
    graph: &crate::frontend::names::NameNamespaces,
    name: &str,
) -> Box<SourceCallable> {
    let main = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let views = [source];
    let resolver = CallResolver::new(graph, &views).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(db, main, name)
    else {
        panic!("actual source callable absent")
    };
    callee
}
fn world(edition: u16) -> World {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let unit = SourceUnitInput {
        path: SourcePath::new("context-closed-reference.rs").unwrap(),
        crate_key: Digest([197; 32]),
        module_path: vec![],
        profile: profile.clone(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(
            b"fn take(x:&bool,y:bool){}fn other(x:&bool,y:bool){}fn main(){&true;}".to_vec(),
        )
        .unwrap(),
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
    let mut graph =
        crate::frontend::names::NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let imports = graph.resolve_imports().unwrap();
    assert!(imports.pending.is_empty() && imports.missing.is_empty());
    let callee = callable(&mut db, &source, &graph, "take");
    let body = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
        panic!()
    };
    let mut store = GuestStore::new(1, 10_000, 4_000_000).unwrap();
    let mut frame = Frame::new(
        index.as_ref().clone(),
        index.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    frame.set_fuel_limit(100_000).unwrap();
    let mut reached = None;
    for _ in 0..128 {
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!()
        };
        let reference = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::Borrow { destination, .. } => Some(*destination),
            _ => None,
        });
        assert!(matches!(
            step(&mut frame, &mut store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        if reference.is_some() {
            reached = reference;
            break;
        }
    }
    let slot = reached.expect("actual ordinary caller borrow was not reached");
    let reference = ClosedReferenceArgument::capture(&frame, slot, &store).unwrap();
    let arguments = [
        ArgumentType {
            source: frame.source_view_ref(slot).unwrap().unwrap().clone(),
            descriptor: TypeId(store.admit_move(frame.cell(slot).unwrap()).unwrap().type_id),
        },
        ArgumentType {
            source: TyTerm::Primitive(PrimitiveType::Bool),
            descriptor: primitive_runtime_id(PrimitiveType::Bool).unwrap(),
        },
    ];
    let references = [Some(reference), None];
    let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) =
        prepare_contracts_with_references(
            &mut db,
            &callee,
            &graph,
            &[&source],
            &[],
            None,
            ReferenceCallArguments {
                store: &store,
                arguments: &arguments,
                references: &references,
            },
        )
    else {
        panic!("actual source reference producer failed")
    };
    World {
        db,
        source,
        graph,
        callee,
        packet,
        frame,
        store,
        slot,
    }
}
#[test]
fn actual_current_reference_environment_owns_checked_certificate_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut w = world(edition);
        ClosedReferenceArgument::capture(&w.frame, w.slot, &w.store).unwrap();
        let proof = w.packet.closed_references.as_ref().unwrap();
        let expected = w
            .callee
            .signature()
            .estimated_bytes()
            .checked_add(std::mem::size_of::<GenericEnv>())
            .unwrap()
            .checked_add(proof.allocated_bytes().unwrap())
            .unwrap();
        let checked = GenericEnv::checked_bytes(w.callee.signature(), &w.packet).unwrap();
        assert_eq!(checked, expected);
        let env = GenericEnv::capture_current(&mut w.db, &w.callee, &w.packet).unwrap();
        assert_eq!(env.retained_bytes(), checked);
        assert_eq!(env.definition(), w.callee.signature());
        let retained = env.closed_reference_contracts().unwrap();
        assert!(!std::ptr::eq(retained, proof.as_ref()));
        retained
            .validate_packet(w.callee.signature(), &w.packet)
            .unwrap();
        assert_eq!(
            GenericEnv::capture(w.callee.signature(), &w.packet)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
    }
}
#[test]
fn current_reference_context_rejects_missing_proof_extra_ref_and_wrong_descriptor() {
    let mut w = world(2024);
    let proof = w.packet.closed_references.take().unwrap();
    assert_eq!(
        GenericEnv::checked_bytes(w.callee.signature(), &w.packet)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    assert!(GenericEnv::capture_current(&mut w.db, &w.callee, &w.packet).is_err());
    w.packet.closed_references = Some(proof);
    let original = w.packet.source_parameters[1].clone();
    let descriptor = w.packet.parameter_descriptors[1];
    w.packet.source_parameters[1] = w.packet.source_parameters[0].clone();
    w.packet.parameter_descriptors[1] = w.packet.parameter_descriptors[0];
    assert_eq!(
        GenericEnv::capture_current(&mut w.db, &w.callee, &w.packet)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    w.packet.source_parameters[1] = original;
    w.packet.parameter_descriptors[1] = descriptor;
    w.packet.parameter_descriptors[0] = primitive_runtime_id(PrimitiveType::Bool).unwrap();
    assert_eq!(
        GenericEnv::capture_current(&mut w.db, &w.callee, &w.packet)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
}
#[test]
fn reference_environment_replays_current_source_and_selected_signature() {
    let mut w = world(2024);
    GenericEnv::capture_current(&mut w.db, &w.callee, &w.packet).unwrap();
    let other = callable(&mut w.db, &w.source, &w.graph, "other");
    assert_eq!(
        GenericEnv::capture_current(&mut w.db, &other, &w.packet)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    w.db.begin_expansion_epoch(1).unwrap();
    assert_eq!(
        GenericEnv::capture_current(&mut w.db, &w.callee, &w.packet)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
}
#[test]
fn reference_certificate_does_not_exempt_existing_substitution_or_result_guards() {
    let mut w = world(2024);
    let key = ParamKey {
        definition: Digest([198; 32]),
        ordinal: 0,
    };
    w.packet
        .source_substitution
        .insert(key, TyTerm::Primitive(PrimitiveType::Bool));
    w.packet
        .runtime_substitution
        .insert(key, primitive_runtime_id(PrimitiveType::Bool).unwrap());
    assert_eq!(
        GenericEnv::checked_bytes(w.callee.signature(), &w.packet)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    w.packet.source_substitution.clear();
    w.packet.runtime_substitution.clear();
    w.packet.source_result = Some(TyTerm::Primitive(PrimitiveType::Bool));
    w.packet.result_descriptor = Some(primitive_runtime_id(PrimitiveType::Bool).unwrap());
    assert_eq!(
        GenericEnv::capture_current(&mut w.db, &w.callee, &w.packet)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
}

#[test]
fn reference_certificate_refuses_a_distinct_owning_copy_cell_without_transport() {
    use crate::runtime::{
        cell::{CellOwner, StorageContract},
        value::ValueRepr,
    };
    let mut w = world(2024);
    let certified = w.frame.cell(w.slot).unwrap();
    let frame = w.store.new_frame_anchor().unwrap();
    let copied = w
        .store
        .new_cell(
            StorageContract::Exact(w.packet.parameter_descriptors[0]),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    let copied_ticket = w.store.copy_from(certified).unwrap();
    w.store.initialize(copied, copied_ticket).unwrap();
    let boolean = w
        .store
        .new_cell(
            StorageContract::Exact(primitive_runtime_id(PrimitiveType::Bool).unwrap()),
            true,
            CellOwner::FrameSlot { frame, slot: 1 },
        )
        .unwrap();
    let boolean_ticket = w.store.ticket(ValueRepr::boolean(true)).unwrap();
    w.store.initialize(boolean, boolean_ticket).unwrap();
    assert_ne!(certified, copied);
    assert_eq!(
        w.store.admit_move(certified).unwrap(),
        w.store.admit_move(copied).unwrap()
    );
    w.store.reference_from_cell(certified).unwrap();
    w.store.reference_from_cell(copied).unwrap();
    let actuals = [
        ArgumentType {
            source: w.frame.source_view_ref(w.slot).unwrap().unwrap().clone(),
            descriptor: w.packet.parameter_descriptors[0],
        },
        ArgumentType {
            source: TyTerm::Primitive(PrimitiveType::Bool),
            descriptor: primitive_runtime_id(PrimitiveType::Bool).unwrap(),
        },
    ];
    let certificate = w.packet.closed_references.as_ref().unwrap();
    let before = w.store.owner_inventory_for_test();
    certificate
        .require_argument_cells(&w.store, &[certified, boolean], &actuals)
        .unwrap();
    assert_eq!(
        certificate
            .require_argument_cells(&w.store, &[copied, boolean], &actuals)
            .unwrap_err()
            .code,
        "LRR-E-CALL-CONTEXT"
    );
    assert_eq!(w.store.owner_inventory_for_test(), before);
    w.store.reference_from_cell(certified).unwrap();
    w.store.reference_from_cell(copied).unwrap();
    // This is a read-only admission check. No packet is sealed or cell moved.
}
