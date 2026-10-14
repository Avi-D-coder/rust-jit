//! Source-minted deferred return admission and printed-hint preservation.
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
        call_contracts::{ContractPreparation, prepare_contracts},
        callables::{CallResolver, CallSelection, SourceCallable},
        names::NameNamespaces,
    },
    ir::{
        op::PrimitiveType,
        pc::{NodeId, Phase},
    },
    runtime::{cell::GuestStore, value::ValueRepr},
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
const WITNESS: &str = "type i64=bool;fn main(){chosen();}fn chosen()->i64{true}fn cold(){}";
const ORIGINAL_SHADOW: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/shadowed/primitive-type-shadowing.rs"
);
const ORIGINAL_USIZE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/usize-base.rs"
);
fn source(text: &str, edition: u16) -> (Database, PreparedSourceView, NameNamespaces) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("return-context.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let unit = Digest::of_bytes(b"LRR deferred return source world");
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
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    (db, view, graph)
}
fn contracts(
    db: &mut Database,
    view: &PreparedSourceView,
    graph: &NameNamespaces,
    name: &str,
) -> (Box<SourceCallable>, PreparedCallContracts) {
    let caller = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let sources = [view];
    let resolver = CallResolver::new(graph, &sources).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(db, caller, name)
    else {
        panic!("callee unavailable")
    };
    let QueryPoll::Ready(Ok(ContractPreparation::Ready(contracts))) =
        prepare_contracts(db, &callee, graph, &sources, &[], None, &[])
    else {
        panic!("contract unresolved")
    };
    (callee, contracts)
}
#[test]
fn genuine_primitive_shadow_alias_return_keeps_bool_through_root_set_hint_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, graph) = source(WITNESS, edition);
        let (callee, contracts) = contracts(&mut db, &view, &graph, "chosen");
        let env = GenericEnv::capture_current(&mut db, &callee, &contracts).unwrap();
        let indexed = db
            .fetch(
                &SyntaxProvider,
                QueryKey::BodyStructure(callee.body().unwrap().key()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
            panic!()
        };
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut frame = Frame::new(
            index.as_ref().clone(),
            index.pc(NodeId(0), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        frame.install_context(env).unwrap();
        frame.set_fuel_limit(100).unwrap();
        let mut returned = None;
        for _ in 0..32 {
            let outcome = db
                .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!()
            };
            match step(&mut frame, &mut store, recipe).unwrap() {
                StepOutcome::Next(_) => {}
                StepOutcome::Return(ticket) => {
                    returned = Some(ticket);
                    break;
                }
                other => panic!("unexpected step {other:?}"),
            }
        }
        let value = store
            .peek_ticket(returned.expect("chosen did not return"))
            .unwrap();
        assert_eq!(value, ValueRepr::boolean(true));
        assert_eq!(
            frame
                .source_view(frame.body.nodes()[0].value.unwrap())
                .unwrap(),
            Some(TyTerm::Primitive(PrimitiveType::Bool))
        );
    }
}
#[test]
fn normalized_return_requires_private_mint_and_exact_source_descriptor_owner() {
    let (mut db, view, graph) = source(
        "type i64=bool;fn main(){}fn chosen()->i64{true}fn other()->i64{false}",
        2024,
    );
    let (callee, mut accepted) = contracts(&mut db, &view, &graph, "chosen");
    assert!(accepted.normalized_deferred_return.is_some());
    assert!(GenericEnv::capture(callee.signature(), &accepted).is_err());
    let proof = accepted.normalized_deferred_return.clone();
    accepted.normalized_deferred_return = None;
    assert!(GenericEnv::capture_current(&mut db, &callee, &accepted).is_err());
    accepted.normalized_deferred_return = proof.clone();
    accepted.source_result = Some(TyTerm::Primitive(PrimitiveType::I64));
    accepted.result_descriptor = Some(TypeId(7));
    assert!(GenericEnv::capture_current(&mut db, &callee, &accepted).is_err());
    let (other, mut wrong) = contracts(&mut db, &view, &graph, "other");
    wrong.normalized_deferred_return = proof;
    assert!(GenericEnv::capture_current(&mut db, &other, &wrong).is_err());
}
#[test]
fn copied_same_header_proof_refuses_changed_alias_source_and_derived_revision() {
    let (mut db, view, graph) = source(WITNESS, 2024);
    let (callee, accepted) = contracts(&mut db, &view, &graph, "chosen");
    let proof = accepted.normalized_deferred_return.clone();
    let (mut newer, new_view, new_graph) = source(&WITNESS.replace("i64=bool", "i64=u64"), 2024);
    let (new_callee, mut forged) = contracts(&mut newer, &new_view, &new_graph, "chosen");
    assert_eq!(
        new_callee.signature().fingerprint(),
        callee.signature().fingerprint()
    );
    forged.source_result = accepted.source_result.clone();
    forged.result_descriptor = accepted.result_descriptor;
    forged.normalized_deferred_return = proof;
    assert!(GenericEnv::capture_current(&mut newer, &new_callee, &forged).is_err());
    let old = db.revision();
    let source_revision = db.source_revision();
    let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        crate::source::manifest::Namespace::Type,
        Digest::of_bytes(b"derived-return-proof-epoch"),
        1,
    )
    .unwrap();
    assert!(matches!(
        discovery.poll().unwrap(),
        crate::expand::source_discovery::SourceDiscoveryStep::Sealed(_)
    ));
    discovery.publish(&mut db).unwrap();
    assert!(db.revision() > old);
    assert_eq!(db.source_revision(), source_revision);
    assert!(GenericEnv::capture_current(&mut db, &callee, &accepted).is_err());
}
#[test]
fn official_shadowed_nominal_and_named_primitive_controls_are_not_normalized_aliases() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, graph) = source(ORIGINAL_USIZE, edition);
        let (callee, prepared) = contracts(&mut db, &view, &graph, "return_val");
        assert!(prepared.normalized_deferred_return.is_none());
        assert!(GenericEnv::capture_current(&mut db, &callee, &prepared).is_ok());
        let (_, view, _) = source(ORIGINAL_SHADOW, edition);
        // The original struct has an allow attribute. Complete its actual
        // compiler-inert declaration preparation before requesting its name.
        let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            crate::source::manifest::Namespace::Type,
            Digest::of_bytes(b"original nominal shadow declaration preparation"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"original nominal shadow macro indexes"),
            Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
            std::sync::Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &view.selection().outline.parse.profile,
                )
                .unwrap(),
            ),
        )
        .unwrap();
        assert!(matches!(
            discovery.poll().unwrap(),
            crate::expand::source_discovery::SourceDiscoveryStep::Sealed(_)
        ));
        let (_, mut graph) = discovery.into_runtime_parts().unwrap();
        let imports = graph.resolve_imports().unwrap();
        assert!(imports.pending.is_empty());
        // Official bool is nominal; this is header/name admission, not impl execution.
        assert!(matches!(
            graph
                .lookup_unqualified(
                    crate::source::manifest::Namespace::Type,
                    "bool",
                    &view.outline().root_scope,
                    edition,
                    None
                )
                .unwrap(),
            crate::frontend::names::NameLookup::Defined(_)
        ));
    }
}

#[test]
fn complete_minimal_shadow_alias_call_uses_current_owned_return_certificate() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, _) = source(WITNESS, edition);
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let mut run = crate::engine::run::RunMachine::start_with_sources(
            &mut db,
            body,
            1000,
            vec![view.share()],
        )
        .into_ready()
        .unwrap()
        .unwrap();
        let crate::engine::run::RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("minimal source alias witness did not complete")
        };
        assert!(matches!(
            report.value,
            crate::engine::run::ValueSummary::Unit
        ));
        assert!(report.opcode_counters.started_by_opcode[0] > 0);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        db.finish_run().unwrap();
    }
}

#[test]
fn normalized_return_does_not_erase_a_rigid_actual_source_at_equal_descriptor() {
    let (mut db, view, graph) = source(WITNESS, 2024);
    let (callee, contracts) = contracts(&mut db, &view, &graph, "chosen");
    let context = GenericEnv::capture_current(&mut db, &callee, &contracts).unwrap();
    let indexed = db
        .fetch(
            &SyntaxProvider,
            QueryKey::BodyStructure(callee.body().unwrap().key()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!()
    };
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(
        index.as_ref().clone(),
        index.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    frame.install_context(context).unwrap();
    frame.set_fuel_limit(100).unwrap();
    for _ in 0..32 {
        let outcome = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!()
        };
        let transfer = recipe.recipe().operations.iter().find_map(|op| match op {
            crate::ir::op::Op::ResultTransfer {
                source,
                destination,
            } if Some(*destination) == frame.body.nodes()[0].value => Some((*source, *destination)),
            _ => None,
        });
        if let Some((source, _)) = transfer {
            let cell = frame.cell(source).unwrap();
            let value = store.read_scalar(cell).unwrap();
            assert_eq!(value.type_id, 2);
            let rigid = TyTerm::Param(ParamKey {
                definition: frame.body.body().definition,
                ordinal: 0,
            });
            frame.set_source_view(source, Some(rigid.clone())).unwrap();
            let before = (frame.fuel_snapshot(), store.budget().handles());
            assert_eq!(
                step(&mut frame, &mut store, recipe).unwrap_err().code,
                "LRR-E-CALL-CONTEXT"
            );
            assert_eq!((frame.fuel_snapshot(), store.budget().handles()), before);
            assert_eq!(frame.source_view(source).unwrap(), Some(rigid));
            assert_eq!(store.read_scalar(cell).unwrap(), value);
            assert_eq!(frame.counters.started_by_opcode[3], 0);
            return;
        }
        assert!(matches!(
            step(&mut frame, &mut store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("certified root transfer was not reached")
}

#[test]
fn normalized_return_refuses_missing_variable_identity_instead_of_descriptor_recovery() {
    let text = "type i64=bool;fn main(){}fn chosen()->i64{let value=true;value}";
    let (mut db, view, graph) = source(text, 2024);
    let (callee, contracts) = contracts(&mut db, &view, &graph, "chosen");
    let context = GenericEnv::capture_current(&mut db, &callee, &contracts).unwrap();
    let indexed = db
        .fetch(
            &SyntaxProvider,
            QueryKey::BodyStructure(callee.body().unwrap().key()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!()
    };
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(
        index.as_ref().clone(),
        index.pc(NodeId(0), Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    frame.install_context(context).unwrap();
    frame.set_fuel_limit(100).unwrap();
    for _ in 0..64 {
        let outcome = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!()
        };
        let transfer = recipe.recipe().operations.iter().find_map(|op| match op {
            crate::ir::op::Op::ResultTransfer {
                source,
                destination,
            } if Some(*destination) == frame.body.nodes()[0].value => Some(*source),
            _ => None,
        });
        if let Some(source) = transfer {
            let producer = frame
                .body
                .nodes()
                .iter()
                .find(|node| node.value == Some(source))
                .unwrap();
            assert_eq!(
                producer.role,
                crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Path
                )
            );
            let cell = frame.cell(source).unwrap();
            let value = store.read_scalar(cell).unwrap();
            assert_eq!(value.type_id, 2);
            frame.set_source_view(source, None).unwrap();
            let before = (frame.fuel_snapshot(), store.budget().handles());
            assert_eq!(
                step(&mut frame, &mut store, recipe).unwrap_err().code,
                "LRR-E-CALL-CONTEXT"
            );
            assert_eq!((frame.fuel_snapshot(), store.budget().handles()), before);
            assert!(frame.source_view(source).unwrap().is_none());
            assert_eq!(store.read_scalar(cell).unwrap(), value);
            assert_eq!(frame.counters.started_by_opcode[3], 0);
            return;
        }
        assert!(matches!(
            step(&mut frame, &mut store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
    }
    panic!("variable result transfer was not reached")
}

// Source-only preregistered regression: counters are database-local; an
// immediate alias recipe does not cover a changed alias in another module file.
fn transitive_return_world(terminal: &str) -> (Database, Vec<PreparedSourceView>, NameNamespaces) {
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let owner = Digest::of_bytes(b"LRR transitive normalized return proof world");
    let root_text = "mod types;use types::B;type Return=B;fn main(){}fn chosen()->Return{true}";
    let child_text = format!("pub type B={terminal};");
    let mut units = Vec::new();
    let mut selections = Vec::new();
    for (path, module_path, text) in [
        ("root.rs", vec![], root_text),
        ("types.rs", vec!["types".to_owned()], child_text.as_str()),
    ] {
        let path = SourcePath::new(path).unwrap();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        selections.push(SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: owner,
                module_path: module_path.clone(),
                cfg: cfg.fingerprint(),
            },
        });
        units.push(SourceUnitInput {
            path,
            crate_key: owner,
            module_path,
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh(None, units, Default::default()).unwrap();
    let views = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("transitive source world unavailable")
            };
            view
        })
        .collect::<Vec<_>>();
    let outlines = views.iter().map(|view| view.outline()).collect::<Vec<_>>();
    let mut graph = NameNamespaces::from_outlines(&outlines).unwrap();
    let imports = graph.resolve_imports().unwrap();
    assert!(
        imports.pending.is_empty(),
        "plain owning alias import remained unresolved"
    );
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    (db, views, graph)
}
fn transitive_return_contracts(
    db: &mut Database,
    views: &[PreparedSourceView],
    graph: &NameNamespaces,
) -> (Box<SourceCallable>, PreparedCallContracts) {
    let caller = views[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let sources = views.iter().collect::<Vec<_>>();
    let resolver = CallResolver::new(graph, &sources).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(db, caller, "chosen")
    else {
        panic!("transitive return callee unavailable")
    };
    let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) =
        prepare_contracts(db, &callee, graph, &sources, &[], None, &[])
    else {
        panic!("transitive primitive alias remained unresolved")
    };
    (callee, packet)
}
#[test]
fn copied_normalized_return_proof_refuses_changed_transitive_alias_at_equal_database_epochs() {
    let (mut old_db, old_views, old_graph) = transitive_return_world("bool");
    let (old_callee, accepted) = transitive_return_contracts(&mut old_db, &old_views, &old_graph);
    assert_eq!(
        accepted.source_result,
        Some(TyTerm::Primitive(PrimitiveType::Bool))
    );
    assert!(GenericEnv::capture_current(&mut old_db, &old_callee, &accepted).is_ok());
    let (mut new_db, new_views, new_graph) = transitive_return_world("u64");
    let (new_callee, mut packet) = transitive_return_contracts(&mut new_db, &new_views, &new_graph);
    assert_eq!(
        packet.source_result,
        Some(TyTerm::Primitive(PrimitiveType::U64))
    );
    assert!(GenericEnv::capture_current(&mut new_db, &new_callee, &packet).is_ok());
    assert_eq!(old_db.source_revision(), new_db.source_revision());
    assert_eq!(old_db.revision(), new_db.revision());
    assert_eq!(
        old_callee.signature().fingerprint(),
        new_callee.signature().fingerprint()
    );
    assert_eq!(
        old_views[0].selection(),
        new_views[0].selection(),
        "root source is byte-exact"
    );
    assert_ne!(
        old_views[1].selection(),
        new_views[1].selection(),
        "only transitive module source changed"
    );
    use crate::db::dependency::InputKey;
    let old_root = old_views[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "Return")
        .unwrap()
        .def_key
        .fingerprint();
    let new_root = new_views[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "Return")
        .unwrap()
        .def_key
        .fingerprint();
    assert_eq!(old_root, new_root);
    assert_eq!(
        old_db
            .input(&InputKey::SourceDependencies)
            .unwrap()
            .fingerprint(),
        new_db
            .input(&InputKey::SourceDependencies)
            .unwrap()
            .fingerprint()
    );
    let old_alias = old_db
        .fetch(&SyntaxProvider, QueryKey::TypeAlias(old_root))
        .into_ready()
        .unwrap()
        .unwrap();
    let new_alias = new_db
        .fetch(&SyntaxProvider, QueryKey::TypeAlias(new_root))
        .into_ready()
        .unwrap()
        .unwrap();
    assert_eq!(
        old_alias.fingerprint, new_alias.fingerprint,
        "top alias recipe is not its transitive normalization"
    );
    // Public packet values are mutable; the cloned private certificate must not
    // let them claim an old source result in the new immutable source world.
    packet.source_result = accepted.source_result.clone();
    packet.result_descriptor = accepted.result_descriptor;
    packet.normalized_deferred_return = accepted.normalized_deferred_return.clone();
    let error = GenericEnv::capture_current(&mut new_db, &new_callee, &packet)
        .expect_err("old transitive primitive meaning must not enter a current frame owner");
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    assert!(
        new_db
            .trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
}

// Authentic immutable worlds differ only in an intermediate import/reexport
// file. Never stamp the current bytes as a proof of a supplied stale graph.
fn bridge_return_world(route: &str) -> (Database, Vec<PreparedSourceView>, NameNamespaces) {
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let owner = Digest::of_bytes(b"LRR alias namespace catalogue proof world");
    let root_text = "mod left;mod right;mod bridge;use bridge::Selected;type Return=Selected;fn main(){}fn chosen()->Return{true}";
    let bridge_text = format!("pub use crate::{route}::B as Selected;");
    let mut units = Vec::new();
    let mut selections = Vec::new();
    for (path, module_path, text) in [
        ("root.rs", vec![], root_text),
        ("left.rs", vec!["left".to_owned()], "pub type B=bool;"),
        ("right.rs", vec!["right".to_owned()], "pub type B=u64;"),
        ("bridge.rs", vec!["bridge".to_owned()], bridge_text.as_str()),
    ] {
        let path = SourcePath::new(path).unwrap();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        selections.push(SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: owner,
                module_path: module_path.clone(),
                cfg: cfg.fingerprint(),
            },
        });
        units.push(SourceUnitInput {
            path,
            crate_key: owner,
            module_path,
            profile: profile.clone(),
            cfg: cfg.clone(),
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh(None, units, Default::default()).unwrap();
    let views = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("bridge source world unavailable")
            };
            view
        })
        .collect::<Vec<_>>();
    let outlines = views.iter().map(|view| view.outline()).collect::<Vec<_>>();
    let mut graph = NameNamespaces::from_outlines(&outlines).unwrap();
    assert!(
        graph.resolve_imports().unwrap().pending.is_empty(),
        "plain alias reexport routing remained unresolved"
    );
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    (db, views, graph)
}
#[test]
fn primitive_alias_return_refuses_stale_supplied_namespace_catalogue_before_mint() {
    let (mut old_db, old_views, old_graph) = bridge_return_world("left");
    let (old_callee, old_packet) = transitive_return_contracts(&mut old_db, &old_views, &old_graph);
    assert_eq!(
        old_packet.source_result,
        Some(TyTerm::Primitive(PrimitiveType::Bool))
    );
    assert!(GenericEnv::capture_current(&mut old_db, &old_callee, &old_packet).is_ok());
    let (mut new_db, new_views, new_graph) = bridge_return_world("right");
    let (new_callee, current) = transitive_return_contracts(&mut new_db, &new_views, &new_graph);
    assert_eq!(
        current.source_result,
        Some(TyTerm::Primitive(PrimitiveType::U64))
    );
    assert!(GenericEnv::capture_current(&mut new_db, &new_callee, &current).is_ok());
    assert_eq!(
        old_callee.signature().fingerprint(),
        new_callee.signature().fingerprint()
    );
    for index in 0..3 {
        assert_eq!(
            old_views[index].selection(),
            new_views[index].selection(),
            "root and both terminal aliases must stay byte-exact"
        );
    }
    assert_ne!(
        old_views[3].selection(),
        new_views[3].selection(),
        "only bridge import routing changed"
    );
    use crate::db::dependency::{InputKey, InputValue};
    let location = old_views[3].selection().location_identity();
    assert_eq!(location, new_views[3].selection().location_identity());
    let captured = new_db.input(&InputKey::CapturedSource(location)).unwrap();
    let InputValue::CapturedSource(selection) = captured.as_ref() else {
        panic!("captured source input variant differs")
    };
    assert_eq!(selection, new_views[3].selection());
    assert_ne!(selection, old_views[3].selection());
    // The actual current SourceCallable is genuine; only the supplied immutable
    // namespace graph/catalogue is stale. No public packet or publication is forged.
    let old_sources = old_views.iter().collect::<Vec<_>>();
    match prepare_contracts(
        &mut new_db,
        &new_callee,
        &old_graph,
        &old_sources,
        &[],
        None,
        &[],
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-TYPE-CONTEXT"),
        QueryPoll::Ready(Ok(_)) => panic!(
            "stale catalogue must not normalize using old reexports and stamp current source bytes"
        ),
        QueryPoll::NeedInput(input) => {
            panic!("captured stale catalogue must refuse explicitly, not suspend: {input:?}")
        }
    }
    assert!(
        new_db
            .trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
}
