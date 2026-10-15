//! Parameter-place component controls. The original fixture is never rewritten;
//! actual caller borrows, frontend contracts and packet delivery supply sources.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
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
        names::NameNamespaces,
    },
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    runtime::place::PlaceRoot,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParseKey,
    },
    traits::term::TyTerm,
    vm::{
        call::ArgumentPacket,
        context::GenericEnv,
        step::{StepOutcome, step},
    },
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs"
);
struct Entered {
    db: Database,
    views: Vec<PreparedSourceView>,
    // Retain the actual caller and its borrowed locals for the entire entry.
    _caller: Frame,
    _other_callers: Vec<Frame>,
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    destination: SlotId,
}
fn indexed(db: &mut Database, source: &PreparedSourceView, name: &str) -> BodyIndex {
    let key = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(key))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
        panic!("body kind")
    };
    index.as_ref().clone()
}
fn thunk(db: &mut Database, pc: &PcKey) -> VerifiedRecipe {
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
        panic!("thunk kind")
    };
    recipe.as_ref().clone()
}
fn entered(edition: u16, mutable: bool) -> Entered {
    // The primary official callee is selected without changing its main or
    // bodies. The second source is an LRR physical-entry harness, not stock.
    let text = if mutable {
        "fn twice<'a>(x:&'a mut isize)->isize{*x+*x}fn main(){}"
    } else {
        ORIGINAL
    };
    let caller = if mutable {
        "fn main(){let mut value:isize=20; &mut value;}"
    } else {
        "fn main(){let value:isize=20;&value;}"
    };
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let mut units = Vec::new();
    let mut selections = Vec::new();
    for (ordinal, content) in [text, caller].into_iter().enumerate() {
        let cfg = CfgSet::guest_default();
        let path = SourcePath::new(if ordinal == 0 {
            "regions-infer-call.rs"
        } else {
            "entry-caller.rs"
        })
        .unwrap();
        let blob = SourceBlob::new(content.as_bytes().to_vec()).unwrap();
        let unit = Digest::of_bytes(if ordinal == 0 {
            b"parameter-place-original"
        } else {
            b"parameter-place-caller"
        });
        selections.push(SourceSelection {
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
        });
        units.push(SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile: profile.clone(),
            cfg,
            blob,
        });
    }
    let inputs =
        PreparedSourceInputs::refresh_with_dependencies(None, units, Default::default(), vec![])
            .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let views: Vec<_> = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("captured source absent")
            };
            view
        })
        .collect();
    let outlines: Vec<_> = views.iter().map(PreparedSourceView::outline).collect();
    let mut graph = NameNamespaces::from_outlines(&outlines).unwrap();
    let imports = graph.resolve_imports().unwrap();
    assert!(imports.pending.is_empty() && imports.missing.is_empty());
    let main = views[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let source_refs: Vec<_> = views.iter().collect();
    let resolver = CallResolver::new(&graph, &source_refs).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(&mut db, main, if mutable { "twice" } else { "takes_two" })
    else {
        panic!("selected source callable absent")
    };
    let body = indexed(&mut db, &views[1], "main");
    let mut store = GuestStore::new(1, 10_000, 4_000_000).unwrap();
    // Each component argument is captured at its real borrow boundary in a
    // retained caller activation. Running a later expression statement in one
    // activation would correctly discard the earlier temporary reference.
    // The complete upstream-main adapter separately exercises the actual
    // single-caller evaluation and transport of both arguments.
    let mut callers = Vec::new();
    let mut slots = Vec::new();
    for _ in 0..callee.signature().parameters.len() {
        let mut caller = Frame::new(
            body.clone(),
            body.pc(NodeId(0), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        let mut reached = None;
        for _ in 0..128 {
            let recipe = thunk(&mut db, caller.pc());
            let borrowed = recipe.recipe().operations.iter().find_map(|op| match op {
                Op::Borrow { destination, .. } => Some(*destination),
                _ => None,
            });
            assert!(matches!(
                step(&mut caller, &mut store, &recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if borrowed.is_some() {
                reached = borrowed;
                break;
            }
        }
        slots.push(reached.expect("actual caller borrow was not reached"));
        callers.push(caller);
    }
    assert_eq!(slots.len(), callee.signature().parameters.len());
    let arguments: Vec<_> = callers
        .iter()
        .zip(&slots)
        .map(|(caller, slot)| ArgumentType {
            source: caller.source_view_ref(*slot).unwrap().unwrap().clone(),
            descriptor: TypeId(
                store
                    .admit_move(caller.cell(*slot).unwrap())
                    .unwrap()
                    .type_id,
            ),
        })
        .collect();
    let references: Vec<_> = callers
        .iter()
        .zip(&slots)
        .map(|(caller, slot)| {
            Some(ClosedReferenceArgument::capture(caller, *slot, &store).unwrap())
        })
        .collect();
    let QueryPoll::Ready(Ok(ContractPreparation::Ready(contracts))) =
        prepare_contracts_with_references(
            &mut db,
            &callee,
            &graph,
            &source_refs,
            &[],
            None,
            ReferenceCallArguments {
                store: &store,
                arguments: &arguments,
                references: &references,
            },
        )
    else {
        panic!("genuine closed reference contracts absent")
    };
    let cells: Vec<_> = callers
        .iter()
        .zip(&slots)
        .map(|(caller, slot)| caller.cell(*slot).unwrap())
        .collect();
    contracts
        .closed_references
        .as_ref()
        .unwrap()
        .require_argument_cells(&store, &cells, &arguments)
        .unwrap();
    let context = GenericEnv::capture_current(&mut db, &callee, &contracts).unwrap();
    let body = indexed(
        &mut db,
        &views[0],
        if mutable { "twice" } else { "takes_two" },
    );
    let first = body
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Path))
        .min_by_key(|node| node.range.start)
        .unwrap();
    let pc = body.pc(first.id, Phase::Entry).unwrap();
    let scope = first.scope;
    let recipe = thunk(&mut db, &pc);
    let [
        Op::ParameterPlace {
            ordinal: 0,
            destination,
        },
    ] = recipe.recipe().operations.as_ref()
    else {
        panic!("actual parameter path must lower to its sole address opcode")
    };
    let destination = *destination;
    let packet =
        ArgumentPacket::seal(&mut store, &cells, &contracts.parameter_descriptors).unwrap();
    let mut frame = Frame::new(body, pc, &mut store).unwrap();
    frame.install_context(context).unwrap();
    frame
        .install_parameter_sources(contracts.source_parameters.clone())
        .unwrap();
    let physical: Vec<_> = contracts
        .parameter_descriptors
        .iter()
        .map(|ty| (*ty, false))
        .collect();
    frame
        .initialize_parameters(&packet, &physical, &mut store)
        .unwrap();
    let mut scopes = Vec::new();
    let mut current = Some(scope);
    while let Some(scope) = current {
        scopes.push(scope);
        current = frame.body.scopes()[scope.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    Entered {
        db,
        views,
        _caller: callers.remove(0),
        _other_callers: callers,
        frame,
        store,
        recipe,
        destination,
    }
}
fn physical(c: &Entered) -> (crate::runtime::cell::OwnerInventory, u32, usize) {
    (
        c.store.owner_inventory_for_test(),
        c.store.budget().handles(),
        c.store.budget().bytes(),
    )
}
fn assert_unentered(c: &Entered, before: &(crate::runtime::cell::OwnerInventory, u32, usize)) {
    assert_eq!(&physical(c), before);
    assert!(c.frame.executed_place(c.destination).is_err());
    assert_eq!(c.frame.source_view(c.destination).unwrap(), None);
    assert_eq!(c.frame.counters.started_by_opcode[44], 0);
    assert_eq!(c.frame.fuel_snapshot().used, 0);
}
#[test]
fn original_shared_parameter_path_retains_actual_entry_cell_source_and_lease_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut c = entered(edition, false);
        let cell = c.frame.parameter(0).unwrap();
        let reference = c.store.reference_from_cell(cell).unwrap();
        let source = c.frame.parameter_sources[0].clone();
        assert!(
            matches!(&source, TyTerm::Ref(crate::traits::term::RegionTerm::Local { definition, .. }, false, _) if *definition == c._caller.body.body().definition)
        );
        assert_ne!(
            c._caller.body.body().definition,
            c.frame.body.body().definition
        );
        let before = physical(&c);
        c.frame
            .preflight_parameter_place(0, c.destination, &c.store)
            .unwrap();
        assert_eq!(physical(&c), before);
        assert!(matches!(
            step(&mut c.frame, &mut c.store, &c.recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        assert_eq!(
            c.frame.executed_place(c.destination).unwrap().root,
            PlaceRoot::Cell(cell)
        );
        assert_eq!(c.frame.source_view(c.destination).unwrap(), Some(source));
        assert_eq!(c.store.reference_from_cell(cell).unwrap(), reference);
        assert_eq!(physical(&c), before);
        assert_eq!(c.frame.counters.started_by_opcode[44], 1);
        assert_eq!(c.frame.counters.operations_completed, 1);
    }
}
#[test]
fn repeated_mutable_parameter_dereferences_preserve_unique_owner_and_exact_cell_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut c = entered(edition, true);
        let cell = c.frame.parameter(0).unwrap();
        let reference = c.store.reference_from_cell(cell).unwrap();
        let initial_state = c.store.state(cell).unwrap();
        let mut dereferences = 0;
        let mut addresses = 0;
        for _ in 0..32 {
            let recipe = thunk(&mut c.db, c.frame.pc());
            let parameter = recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::ParameterPlace { .. }));
            let dereference = recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::DerefPlace { .. }));
            let before = physical(&c);
            assert!(matches!(
                step(&mut c.frame, &mut c.store, &recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if parameter {
                addresses += 1;
                assert_eq!(physical(&c), before);
            }
            if dereference {
                dereferences += 1;
            }
            assert_eq!(c.store.state(cell).unwrap(), initial_state);
            assert_eq!(c.store.reference_from_cell(cell).unwrap(), reference);
            if dereferences == 2 {
                break;
            }
        }
        assert_eq!(dereferences, 2);
        assert_eq!(addresses, 2);
        assert_eq!(c.frame.counters.started_by_opcode[44], 2);
        assert_eq!(c.frame.parameter(0).unwrap(), cell);
        assert_eq!(c.views[0].outline().profile.edition, edition);
    }
}
#[test]
fn rustc_original_reference_parameter_scalar_guard_refuses_before_owning_read_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for mutable in [false, true] {
            // Shared entry selects the unchanged regions-infer-call callee;
            // mutable entry is the disclosed source-backed lease supplement.
            // Both own actual current contracts and delivered physical cells.
            let c = entered(edition, mutable);
            let parameter = c.frame.parameter(0).unwrap();
            let before = (
                physical(&c),
                c.store.state(parameter).unwrap(),
                c.store.location_epoch(parameter).unwrap(),
                c.store.reference_from_cell(parameter).unwrap(),
                c.frame.fuel_snapshot().used,
                c.frame.counters.operations_started,
                c.frame.active_scopes.clone(),
            );
            let error = c.frame.definition_parameter_primitive(0).unwrap_err();
            assert_eq!(error.code, "LRR-E-UNSUPPORTED");
            assert_eq!(
                error.message,
                "scalar parameter context requires an admitted primitive source"
            );
            assert_eq!(
                before,
                (
                    physical(&c),
                    c.store.state(parameter).unwrap(),
                    c.store.location_epoch(parameter).unwrap(),
                    c.store.reference_from_cell(parameter).unwrap(),
                    c.frame.fuel_snapshot().used,
                    c.frame.counters.operations_started,
                    c.frame.active_scopes.clone(),
                )
            );
            assert_eq!(c.frame.counters.started_by_opcode[21], 0);
        }
    }
}
#[test]
fn parameter_place_uses_no_guest_handles_or_bytes_even_at_handle_quota() {
    let mut c = entered(2024, true);
    while c.store.budget().handles() < c.store.budget().max_handles() {
        c.store.ticket(ValueRepr::unit()).unwrap();
    }
    let before = physical(&c);
    c.frame.parameter_place(0, c.destination, &c.store).unwrap();
    assert_eq!(physical(&c), before);
}
#[test]
fn missing_wrong_rigid_and_prepared_source_refuse_before_fuel_or_place_effect() {
    for bad in 0..6 {
        let mut c = entered(2024, false);
        match bad {
            0 => c.frame.parameter_sources.clear(),
            1 => {
                c.frame.parameter_sources[0] =
                    TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize)
            }
            2 => {
                if let TyTerm::Ref(_, access, _) = &mut c.frame.parameter_sources[0] {
                    *access = true;
                }
            }
            3 => {
                c.frame.parameter_sources[0] = TyTerm::Param(crate::traits::term::ParamKey {
                    definition: c.frame.body.body().definition,
                    ordinal: 0,
                })
            }
            4 => {
                c.frame.slots[c.destination.0 as usize].source =
                    Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Isize))
            }
            5 => {
                if let TyTerm::Ref(_, _, target) = &mut c.frame.parameter_sources[0] {
                    **target = TyTerm::Primitive(crate::ir::op::PrimitiveType::Usize);
                }
            }
            _ => unreachable!(),
        }
        let before = physical(&c);
        let prior_source = c.frame.source_view(c.destination).unwrap();
        let code = match bad {
            0 | 3 => "LRR-E-UNSUPPORTED",
            1 | 2 | 5 => "LRR-E-OWNERSHIP",
            4 => "LRR-E-INTERNAL-IR",
            _ => unreachable!(),
        };
        assert_eq!(
            step(&mut c.frame, &mut c.store, &c.recipe)
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(physical(&c), before);
        assert!(c.frame.executed_place(c.destination).is_err());
        assert_eq!(c.frame.source_view(c.destination).unwrap(), prior_source);
        assert_eq!(c.frame.counters.started_by_opcode[44], 0);
        assert_eq!(c.frame.fuel_snapshot().used, 0);
    }
}
#[test]
fn retired_uninitialized_foreign_owner_and_inactive_parameters_refuse_without_effect() {
    for bad in 0..5 {
        let mut c = entered(2024, true);
        let parameter = c.frame.parameter(0).unwrap();
        match bad {
            0 | 1 => {
                let value = c.store.move_from(parameter).unwrap();
                c.store.discard(value).unwrap();
                if bad == 1 {
                    c.store.retire_cell(parameter).unwrap();
                }
            }
            2 => {
                // A real current cell in this same store, owned by the actual
                // caller, cannot stand in for the callee's parameter cell.
                let foreign = c
                    .store
                    .new_cell(
                        StorageContract::Exact(TypeId(8)),
                        false,
                        CellOwner::FrameSlot {
                            frame: c._caller.anchor,
                            slot: 100_000,
                        },
                    )
                    .unwrap();
                c.frame.parameters[0] = Some(foreign);
            }
            3 => c.frame.parameters_initialized = false,
            4 => c.frame.active_scopes.clear(),
            _ => unreachable!(),
        }
        let before = physical(&c);
        let code = if bad <= 1 {
            "LRR-E-STALE-HANDLE"
        } else {
            "LRR-E-INTERNAL-IR"
        };
        assert_eq!(
            step(&mut c.frame, &mut c.store, &c.recipe)
                .unwrap_err()
                .code,
            code
        );
        assert_unentered(&c, &before);
    }
}
#[test]
fn ordinal_foreign_store_and_metadata_overflow_admission_are_pure() {
    let mut c = entered(2024, true);
    let before = physical(&c);
    for ordinal in [1, 100_000, u32::MAX] {
        assert_eq!(
            c.frame
                .preflight_parameter_place(ordinal, c.destination, &c.store)
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_unentered(&c, &before);
    }
    let foreign = GuestStore::new(1, 10_000, 4_000_000).unwrap();
    assert_eq!(
        c.frame
            .preflight_parameter_place(0, c.destination, &foreign)
            .unwrap_err()
            .code,
        "LRR-E-STALE-HANDLE"
    );
    assert_unentered(&c, &before);
    c.frame.constructor_mapping_bytes = usize::MAX;
    assert_eq!(
        c.frame
            .preflight_parameter_place(0, c.destination, &c.store)
            .unwrap_err()
            .code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_unentered(&c, &before);
}
#[test]
fn zero_fuel_refuses_before_opcode_entry_and_successful_place_is_not_replayed() {
    let mut c = entered(2024, true);
    c.frame.set_fuel_limit(0).unwrap();
    let before = physical(&c);
    assert_eq!(
        step(&mut c.frame, &mut c.store, &c.recipe)
            .unwrap_err()
            .code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_unentered(&c, &before);
    let mut c = entered(2024, true);
    c.frame.set_fuel_limit(1).unwrap();
    let before = physical(&c);
    // One operation is admitted; the exit's independent fuel refusal must not
    // undo or replay it. This matches the existing single-prefix step contract.
    assert_eq!(
        step(&mut c.frame, &mut c.store, &c.recipe)
            .unwrap_err()
            .code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_eq!(physical(&c), before);
    assert_eq!(c.frame.counters.started_by_opcode[44], 1);
    assert_eq!(c.frame.counters.operations_completed, 1);
    let destination = c.frame.executed_place(c.destination).unwrap().root;
    assert!(step(&mut c.frame, &mut c.store, &c.recipe).is_err());
    assert_eq!(
        c.frame.executed_place(c.destination).unwrap().root,
        destination
    );
    assert_eq!(c.frame.counters.started_by_opcode[44], 1);
    assert_eq!(physical(&c), before);
}
