//! LRR route/currentness controls. Stock main and standard-macro expansion are
//! separate acceptance requirements; these execute ordinary reference calls.
use super::retained_body_tests::prepare_expanded_source;
use crate::{
    core::digest::StableEncode,
    db::{key::QueryKey, memo::Database, poll::QueryPoll},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    frontend::{
        call_contracts::{ArgumentHintPreparation, prepare_argument_hint},
        callables::{CallResolver, CallSelection},
        names::NameNamespaces,
    },
    source::{changes::PreparedSourceView, manifest::BodyKey},
};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("reference source preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("reference source suspended: {input:?}"),
    }
}
fn body(source: &PreparedSourceView, name: &str) -> BodyKey {
    source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key()
}

#[test]
fn retained_reference_calls_share_ordinary_source_entry_all_editions() {
    // Same primitive reference operation as regions-infer-call.rs, moved
    // across ordinary source, arbitrary user output and a macro wrapper.
    for edition in [2015, 2018, 2021, 2024] {
        for declarations in [
            "fn take(x:&i32)->i32{*x}",
            "macro_rules! emit{()=>{fn take(x:&i32)->i32{*x}}}emit!();",
            "macro_rules! emit{()=>{fn take(x:&i32)->i32{*x}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
        ] {
            let text = format!(
                "{declarations}fn main()->i32{{let value=22_i32;take(&value)}}fn cold<T:Missing>(){{unknown();}}"
            );
            let mut db = Database::default();
            db.trace.detailed = true;
            let (input, discovery) = prepare_expanded_source(&mut db, &text, edition, 1, true);
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let original_main = body(&source, "main");
            ready(db.publish_retained_syntax(input.clone()));
            // Only original members are reconciled here. Generated members
            // retain their genuine discovery-published GeneratedSyntax source.
            if let Some(take) = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "take")
            {
                ready(db.publish_retained_body(input.clone(), take.def_key.fingerprint()));
            }
            let main = ready(db.publish_retained_body(input, original_main.definition));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, main, 100_000, discovery,
            ));
            let report = match run.poll(&mut db) {
                RunPoll::Completed(report) => report,
                RunPoll::Fault(error) => panic!("ordinary reference call faulted: {error:?}"),
                RunPoll::NeedInput(input) => panic!("ordinary reference call suspended: {input:?}"),
                RunPoll::Panicked(report) => panic!("ordinary reference call panicked: {report:?}"),
            };
            assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "22"));
            assert_eq!(run.peak_guest_frames(), 2);
            assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::FunctionSignature(_)))
            );
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::BodyStructure(_)))
            );
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::Thunk(_)))
            );
        }
    }
}

#[test]
fn retained_reference_parameter_proof_refuses_changed_expansion_epoch() {
    let mut db = Database::default();
    let (input, _) = prepare_expanded_source(
        &mut db,
        "macro_rules! id{($x:expr)=>{$x}}fn take<'a>(x:&'a i32)->i32{id!(*x)}fn main(){}",
        2024,
        1,
        true,
    );
    let source = db
        .prepared_source(input.prepared().source())
        .unwrap()
        .unwrap();
    let original = body(&source, "take");
    let main = body(&source, "main");
    ready(db.publish_retained_syntax(input.clone()));
    let retained = ready(db.publish_retained_body(input, original.definition));
    assert_ne!(retained, original);
    let graph = NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    let views = [&source];
    let resolver = CallResolver::new(&graph, &views).unwrap();
    let CallSelection::Source(callee) =
        ready(resolver.module_function(&mut db, main.definition, "take"))
    else {
        panic!()
    };
    let ArgumentHintPreparation::Ready(hints) =
        ready(prepare_argument_hint(&mut db, &callee, &graph, &views, 0))
    else {
        panic!("retained parameter lost its actual source authority")
    };
    let proof = hints[0].closed_reference.as_ref().unwrap();
    proof.require_current(&mut db, &callee).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert!(proof.require_current(&mut db, &callee).is_err());
}

#[test]
fn common_reference_results_forward_real_caller_provenance_after_cleanup_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (declarations, binding, argument) in [
            ("fn take(x:&isize)->&isize{return &*x;}", "value", "&value"),
            ("fn take<'a>(x:&'a isize)->&'a isize{x}", "value", "&value"),
            (
                "macro_rules! emit{()=>{fn take(x:&isize)->&isize{return &*x;}}}emit!();",
                "value",
                "&value",
            ),
            (
                "macro_rules! emit{()=>{fn take(x:&isize)->&isize{return &*x;}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
                "value",
                "&value",
            ),
            (
                "fn take(x:&mut isize)->&isize{&*x}",
                "mut value",
                "&mut value",
            ),
        ] {
            let text = format!(
                "{declarations}fn read(x:&isize)->isize{{*x}}fn main()->isize{{let {binding}:isize=22;read(take({argument}))}}fn cold<T:Missing>(){{unknown();}}"
            );
            let mut db = Database::default();
            db.trace.detailed = true;
            let (input, discovery) = prepare_expanded_source(&mut db, &text, edition, 1, true);
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let original_main = body(&source, "main");
            let cold = body(&source, "cold");
            ready(db.publish_retained_syntax(input.clone()));
            for name in ["take", "read"] {
                if let Some(item) = source
                    .outline()
                    .items
                    .iter()
                    .find(|item| item.def_key.declared_name == name)
                {
                    ready(db.publish_retained_body(input.clone(), item.def_key.fingerprint()));
                }
            }
            let main = ready(db.publish_retained_body(input, original_main.definition));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, main, 100_000, discovery,
            ));
            let report = match run.poll(&mut db) {
                RunPoll::Completed(report) => report,
                RunPoll::Fault(error) => panic!("common reference result faulted: {error:?}"),
                RunPoll::NeedInput(input) => panic!("common reference result suspended: {input:?}"),
                RunPoll::Panicked(report) => panic!("common reference result panicked: {report:?}"),
            };
            assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "22"));
            assert_eq!(report.opcode_counters.started_by_opcode[19], 2);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert!(report.opcode_counters.return_scopes_cleaned >= 3);
            assert_eq!(run.peak_guest_frames(), 2);
            assert!(!db.trace.events.iter().any(|event| matches!(&event.key,
                QueryKey::BodyStructure(body) if body == &cold)));
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::FunctionSignature(_)))
            );
        }
    }
    // Forwarding to a second real source call exercises the restored caller
    // reference source; a matching scalar value alone cannot admit that call.
}

#[test]
fn retained_caller_and_generated_named_reference_identity_use_the_ordinary_source_engine() {
    const POINT: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-borrow-scope.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for declaration in [
            "fn lrr_identity(p:&Point)->&Point{p}",
            "macro_rules! emit{()=>{fn lrr_identity(p:&Point)->&Point{p}}}emit!();",
            "macro_rules! emit{()=>{fn lrr_identity(p:&Point)->&Point{p}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
        ] {
            let text = format!(
                "{POINT}\n{declaration}\nfn lrr_probe()->isize{{let p=Point{{x:3_isize,y:4_isize}};let q=lrr_identity(&p);q.x}}"
            );
            let mut db = Database::default();
            db.trace.detailed = true;
            let (input, discovery) = super::retained_body_tests::prepare_expanded_body_source(
                &mut db,
                &text,
                edition,
                1,
                "lrr_probe",
            );
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let original_probe = body(&source, "lrr_probe");
            let cold_main = body(&source, "main");
            ready(db.publish_retained_syntax(input.clone()));
            let probe = ready(db.publish_retained_body(input, original_probe.definition));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, probe, 100_000, discovery,
            ));
            let report = match run.poll(&mut db) {
                RunPoll::Completed(report) => report,
                RunPoll::Fault(error) => panic!("common named reference identity: {error:?}"),
                RunPoll::NeedInput(input) => panic!("common named reference suspension: {input:?}"),
                RunPoll::Panicked(report) => panic!("common named reference panic: {report:?}"),
            };
            assert!(matches!(report.value,ValueSummary::Signed(ref value) if value=="3"));
            assert_eq!(run.peak_guest_frames(), 2);
            assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert!(!db.trace.events.iter().any(
                |event| matches!(&event.key,QueryKey::BodyStructure(body) if body==&cold_main)
            ));
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::ReferenceHeader(_)))
            );
            assert!(
                db.trace
                    .events
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::TypeSchema(_)))
            );
        }
    }
    // Stock x_coord/Box/assert main and nested standard-macro execution remain
    // separate acceptance gaps. This external probe does not rewrite originals.
}

#[test]
fn unchanged_point_coordinate_and_emitted_interior_returns_share_field_provenance() {
    use crate::db::retained_syntax::RetainedSyntaxInput;
    const POINT: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-borrow-scope.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for (declaration, callee, retain_callee) in [
            ("", "x_coord", false),
            ("", "x_coord", true),
            (
                "macro_rules! emit{()=>{fn lrr_coord(p:&Point)->&isize{return &p.x;}}}emit!();",
                "lrr_coord",
                false,
            ),
            (
                "macro_rules! emit{()=>{fn lrr_coord(p:&Point)->&isize{return &p.x;}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
                "lrr_coord",
                false,
            ),
        ] {
            // Preserve the complete official fixture. Its real x_coord body is
            // entered by the first two rows; the stock Box/assert main stays cold.
            let text = format!(
                "{POINT}\n{declaration}\nfn lrr_probe()->isize{{let point=Point{{x:3_isize,y:4_isize}};let selected={callee}(&point);*selected}}"
            );
            let mut db = Database::default();
            db.trace.detailed = true;
            let (input, discovery) = super::retained_body_tests::prepare_expanded_body_source(
                &mut db,
                &text,
                edition,
                1,
                "lrr_probe",
            );
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            let original_probe = body(&source, "lrr_probe");
            let cold_main = body(&source, "main");
            ready(db.publish_retained_syntax(input.clone()));
            if retain_callee {
                let original = body(&source, callee);
                let prepared = discovery
                    .prepare_body_syntax(source.selection(), original.definition)
                    .unwrap();
                let retained = ready(RetainedSyntaxInput::prepare(
                    &mut db,
                    prepared,
                    discovery.key().clone(),
                ));
                ready(db.publish_retained_syntax(retained.clone()));
                ready(db.publish_retained_body(retained, original.definition));
            }
            let probe = ready(db.publish_retained_body(input, original_probe.definition));
            let mut run = ready(RunMachine::start_with_discovery(
                &mut db, probe, 100_000, discovery,
            ));
            let report = match run.poll(&mut db) {
                RunPoll::Completed(report) => report,
                RunPoll::Fault(error) => panic!(
                    "edition {edition}, {callee}, retained={retain_callee}: interior source return faulted {error:?}"
                ),
                RunPoll::NeedInput(input) => panic!("interior source return suspended {input:?}"),
                RunPoll::Panicked(report) => panic!("interior source return panicked {report:?}"),
            };
            assert!(matches!(report.value,ValueSummary::Signed(ref value) if value=="3"));
            assert_eq!(run.peak_guest_frames(), 2);
            assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[37], 1);
            assert_eq!(report.opcode_counters.started_by_opcode[31], 2);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert!(report.opcode_counters.return_scopes_cleaned >= 2);
            assert!(!db.trace.events.iter().any(|event| matches!(&event.key,
                QueryKey::BodyStructure(body) if body==&cold_main)));
            for required in ["ReferenceHeader", "TypeSchema", "BodyStructure", "Thunk"] {
                assert!(
                    db.trace.counts.keys().any(|key| key.starts_with(required)),
                    "missing ordinary {required} query evidence"
                );
            }
        }
    }
    // This control requires the unchanged x_coord body and LRR route checks;
    // stock Box/standard-library macro main remains a separate requirement.
}

#[test]
fn actual_field_occurrence_source_admission_refuses_foreign_pc_and_stale_epoch() {
    use crate::{
        core::digest::Digest,
        db::{
            dependency::{InputKey, InputValue},
            provider::SyntaxProvider,
            retained_syntax::RetainedSyntaxInput,
            value::QueryValue,
        },
        ir::pc::Phase,
    };
    use ra_ap_syntax::{AstNode, ast};
    for edition in [2015, 2018, 2021, 2024] {
        for (declaration, retain_callee) in [
            ("fn coordinate(p:&Point)->&isize{&p.x}", false),
            ("fn coordinate(p:&Point)->&isize{&p.x}", true),
            (
                "macro_rules! emit{()=>{fn coordinate(p:&Point)->&isize{&p.x}}}emit!();",
                false,
            ),
            (
                "macro_rules! emit{()=>{fn coordinate(p:&Point)->&isize{&p.x}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
                false,
            ),
        ] {
            let text = format!("struct Point{{x:isize}}{declaration}fn lrr_probe(){{}}");
            let mut db = Database::default();
            let (input, discovery) = super::retained_body_tests::prepare_expanded_body_source(
                &mut db,
                &text,
                edition,
                1,
                "lrr_probe",
            );
            let source = db
                .prepared_source(input.prepared().source())
                .unwrap()
                .unwrap();
            ready(db.publish_retained_syntax(input));
            let original = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "coordinate");
            let (definition, generated) = if let Some(original) = original {
                (original.def_key.fingerprint(), false)
            } else {
                let stamps = db.lexical_namespace_stamps().unwrap();
                let definitions = stamps
                    .keys()
                    .filter_map(|key| {
                        let Some((InputValue::GeneratedSyntax(unit), _)) = db.published_input(key)
                        else {
                            return None;
                        };
                        unit.definitions
                            .iter()
                            .find(|(_, key)| key.declared_name == "coordinate")
                            .map(|(identity, _)| *identity)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(definitions.len(), 1);
                (definitions[0], true)
            };
            if retain_callee {
                let prepared = discovery
                    .prepare_body_syntax(source.selection(), definition)
                    .unwrap();
                let retained = ready(RetainedSyntaxInput::prepare(
                    &mut db,
                    prepared,
                    discovery.key().clone(),
                ));
                ready(db.publish_retained_syntax(retained.clone()));
                ready(db.publish_retained_body(retained, definition));
            }
            let body = db.input(&InputKey::DefinitionBody(definition)).unwrap();
            let InputValue::DefinitionBody(body) = body.as_ref() else {
                panic!()
            };
            let parse = db.input(&InputKey::DefinitionSource(definition)).unwrap();
            let InputValue::DefinitionSource(parse) = parse.as_ref() else {
                panic!()
            };
            let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
            let QueryValue::BodyStructure(indexed) = indexed.value.as_ref() else {
                panic!()
            };
            let parsed = ready(db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())));
            let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
                panic!()
            };
            // Test inventory walks the one retained index once; production uses
            // the exact entered PC/direct syntax location selected below.
            let field = indexed
                .nodes()
                .iter()
                .find(|node| {
                    parsed
                        .entered_node_syntax(body, indexed, node.id)
                        .unwrap()
                        .is_some_and(|node| ast::FieldExpr::can_cast(node.kind()))
                })
                .unwrap();
            let pc = indexed.pc(field.id, Phase::Entry).unwrap();
            let sources = [source.share()];
            let owner = std::sync::Arc::new(ready(crate::frontend::source_body_context::capture(
                &mut db,
                &[&sources[0]],
                indexed,
                &pc,
            )));
            let owning_plan = ready(std::sync::Arc::clone(&owner).prepare(&mut db));
            assert_eq!(owning_plan.source().body(), body);
            assert!(
                owning_plan
                    .facts()
                    .bindings
                    .iter()
                    .any(|binding| binding.name == "p")
            );
            assert!(std::sync::Arc::ptr_eq(
                owning_plan.source().index(),
                owner.index()
            ));
            ready(owning_plan.require_current(&mut db));
            let (member, caller) = ready(RunMachine::current_field_source_for_test(
                &mut db, &sources, indexed, &pc,
            ));
            assert_eq!(member, "x");
            assert_eq!(caller, source.outline().root_scope);
            let mut wrong_pc = pc.clone();
            wrong_pc.node = 0;
            assert!(matches!(
                RunMachine::current_field_source_for_test(&mut db, &sources, indexed, &wrong_pc),
                QueryPoll::Ready(Err(_))
            ));
            wrong_pc = pc.clone();
            wrong_pc.body.exact_body_bytes = Digest([219; 32]);
            assert!(matches!(
                RunMachine::current_field_source_for_test(&mut db, &sources, indexed, &wrong_pc),
                QueryPoll::Ready(Err(_))
            ));
            if generated || retain_callee {
                db.begin_expansion_epoch(2).unwrap();
                assert!(matches!(
                    owning_plan.require_current(&mut db),
                    QueryPoll::Ready(Err(_))
                ));
                assert!(matches!(
                    RunMachine::current_field_source_for_test(&mut db, &sources, indexed, &pc),
                    QueryPoll::Ready(Err(_))
                ));
            }
        }
    }
}

#[test]
fn generated_owning_macro_boundaries_retain_actual_insertion_ancestry_all_editions() {
    use crate::{
        db::dependency::{InputKey, InputValue},
        expand::scopes::MacroName,
        frontend::{
            macro_origins::{ExpansionAncestry, MacroSelection},
            macro_scopes::{HygienicLookup, MacroReference},
        },
    };
    for edition in [2015, 2018, 2021, 2024] {
        for (declaration, local, associated) in [
            (
                "macro_rules! emit{()=>{fn answer(){earlier!();}}}emit!();",
                false,
                false,
            ),
            (
                "macro_rules! emit{()=>{fn answer(){earlier!();}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
                false,
                false,
            ),
            (
                "macro_rules! emit{()=>{mod nested{pub fn answer(){earlier!();}}}}emit!();",
                false,
                false,
            ),
            (
                "macro_rules! emit{()=>{macro_rules! local{()=>{}}fn answer(){earlier!();}}}emit!();",
                true,
                false,
            ),
            (
                "macro_rules! emit{()=>{mod nested{macro_rules! local{()=>{}}pub fn answer(){earlier!();}}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
                true,
                false,
            ),
            (
                "macro_rules! emit{()=>{macro_rules! local{()=>{}}struct Holder;impl Holder{fn answer(){earlier!();}}}}emit!();",
                true,
                true,
            ),
            (
                "macro_rules! emit{()=>{macro_rules! local{()=>{}}trait Holder{fn answer(){earlier!();}}}}emit!();",
                true,
                true,
            ),
            (
                "macro_rules! emit{()=>{fn answer(){use earlier as selected;selected!();}}}emit!();",
                false,
                false,
            ),
            (
                "macro_rules! emit{()=>{fn answer(){use earlier as selected;selected!();}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
                false,
                false,
            ),
            (
                "macro_rules! emit{()=>{fn answer(){use import_module::{self as module_alias};earlier!();}}}macro_rules! wrap{()=>{emit!();}}wrap!();",
                false,
                false,
            ),
        ] {
            let text = format!(
                "mod import_module{{}}macro_rules! earlier{{()=>{{}}}}{declaration}macro_rules! later{{()=>{{}}}}fn lrr_probe(){{}}"
            );
            let mut db = Database::default();
            let (_, discovery) = super::retained_body_tests::prepare_expanded_body_source(
                &mut db,
                &text,
                edition,
                1,
                "lrr_probe",
            );
            // Test-only inventory selects the actual generated publication.
            let stamps = db.lexical_namespace_stamps().unwrap();
            let candidates = stamps
                .keys()
                .filter_map(|key| match db.published_input(key) {
                    Some((InputValue::GeneratedSyntax(unit), _))
                        if unit
                            .definitions
                            .values()
                            .any(|key| key.declared_name == "answer") =>
                    {
                        Some(std::sync::Arc::clone(unit))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(candidates.len(), 1);
            let unit = &candidates[0];
            let definition = *unit
                .definitions
                .iter()
                .find(|(_, key)| key.declared_name == "answer")
                .unwrap()
                .0;
            let context = db.input(&InputKey::DeclarationContext(definition)).unwrap();
            let InputValue::DeclarationContext(context) = context.as_ref() else {
                panic!()
            };
            assert_eq!(context.scope.owner.is_some(), associated);
            let boundary = discovery.generated_body_macro_boundary(unit).unwrap();
            if associated {
                assert!(boundary.selected_root().is_some());
            }
            // The same actual output cannot acquire a boundary in another
            // source world merely because its completion epoch matches.
            let mut foreign_seal = unit.seal.clone();
            foreign_seal.unit_graph =
                crate::core::digest::Digest::of_bytes(b"foreign generated source world");
            let (foreign, _) = crate::expand::generated_syntax::GeneratedSyntaxInput::prepare(
                unit.items.record_input(),
                unit.scope.clone(),
                unit.cfg.clone(),
                foreign_seal,
                unit.producer_origin.clone(),
                unit.included_source.clone(),
            )
            .unwrap();
            assert!(discovery.generated_body_macro_boundary(&foreign).is_err());
            let body_input = db.input(&InputKey::DefinitionBody(definition)).unwrap();
            let InputValue::DefinitionBody(body) = body_input.as_ref() else {
                panic!()
            };
            let indexed = ready(db.fetch(
                &crate::db::provider::SyntaxProvider,
                QueryKey::BodyStructure(body.key()),
            ));
            let crate::db::value::QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
                panic!()
            };
            let owner_source = boundary.source_roots()[0].share();
            let pc = index
                .pc(crate::ir::pc::NodeId(0), crate::ir::pc::Phase::Entry)
                .unwrap();
            let admitted =
                std::sync::Arc::new(ready(crate::frontend::source_body_context::capture(
                    &mut db,
                    &[&owner_source],
                    index,
                    &pc,
                )));
            let unbound_identity = admitted.identity();
            let unbound_source = std::sync::Arc::clone(&admitted);
            let admitted = ready(
                admitted.attach_generated_boundary(&mut db, std::sync::Arc::clone(&boundary)),
            );
            assert_ne!(admitted.identity(), unbound_identity);
            let owning = ready(std::sync::Arc::clone(&admitted).prepare(&mut db));
            assert!(std::sync::Arc::ptr_eq(owning.source(), &admitted));
            let (module_context, module_facts) = crate::syntax::floating_fixture_hygiene::root(
                owner_source.selection().outline.crate_key,
                edition,
            );
            let published = crate::frontend::source_macros::PublishedMacroIndex::from_view(
                owner_source.share(),
                module_context,
                module_facts,
            )
            .unwrap();
            let entered = published
                .enter_prepared_body(std::sync::Arc::clone(&owning))
                .unwrap();
            for import in &owning.facts().imports {
                let crate::syntax::imports::ImportSegment::Name(target) = &import.source.path[0]
                else {
                    panic!()
                };
                let occurrence = owning.macro_import_occurrence(import, target).unwrap();
                assert!(
                    occurrence.context(&unbound_source, module_context).is_err(),
                    "an unattached owning source cannot consume a boundary-bound occurrence"
                );
                occurrence.context(owning.source(), module_context).unwrap();
                let mut wrong = import.clone();
                wrong.source.leaf_ordinal = wrong.source.leaf_ordinal.checked_add(1).unwrap();
                assert!(owning.macro_import_occurrence(&wrong, target).is_err());
                assert!(
                    owning
                        .macro_import_occurrence(import, "different_target")
                        .is_err()
                );
            }
            let invocation = owning.facts().invocations.first().unwrap();
            assert!(matches!(
                entered
                    .lookup_invocation(
                        owner_source.selection(),
                        &body.key(),
                        invocation.range,
                        &Default::default()
                    )
                    .unwrap(),
                crate::frontend::macro_names::PathLookup::Defined(_)
            ));
            ready(owning.require_current(&mut db));

            assert!(std::sync::Arc::ptr_eq(boundary.output(), &unit.items));
            assert_eq!(boundary.seal(), &unit.seal);
            assert!(boundary.exclusive_metadata_bytes() > 0);
            assert!(!boundary.source_roots().is_empty());
            let selected = boundary.declaration_outer(definition, context).unwrap();
            assert_eq!(selected.is_some(), boundary.selected_root().is_some());
            let mut roots = Vec::new();
            if let Some(root) = &selected {
                roots.push(root);
            }
            roots.extend(boundary.scopes());
            let mut facts = unit.items.hygiene_facts().as_ref().clone();
            let mut ancestry = ExpansionAncestry::default();
            for root in &roots {
                facts.extend(root.facts()).unwrap();
                ancestry.extend(root.ancestry()).unwrap();
            }
            let (root_context, _) =
                crate::syntax::floating_fixture_hygiene::root(unit.scope.crate_key, edition);
            let first = roots[0].outer();
            let outers = roots[1..]
                .iter()
                .map(|root| root.outer())
                .collect::<Vec<_>>();
            for (name, present) in [("earlier", true), ("later", false), ("local", local)] {
                let name = MacroName::parse(name, edition).unwrap();
                let lookup = first
                    .declarations
                    .resolve_with_outers(
                        first.scopes,
                        MacroReference {
                            scope: first.scope,
                            before: first.before,
                            name: &name,
                            context: root_context,
                            origin: None,
                        },
                        &outers,
                        &facts,
                        &ancestry,
                    )
                    .unwrap();
                assert!(matches!(
                    (lookup, present),
                    (HygienicLookup::Selection(MacroSelection::Defined(_)), true)
                        | (HygienicLookup::Selection(MacroSelection::Absent), false)
                ));
            }
            assert!(
                boundary
                    .declaration_outer(crate::core::digest::Digest([233; 32]), context)
                    .is_err()
            );
            // Catalogue-backed boundaries retain the actual associated
            // declaration's lexical owner; a module-scope substitution fails.
            // Completed-empty boundaries have a separately held proof gap.
            if boundary.selected_root().is_some() {
                let mut wrong_owner = context.clone();
                wrong_owner.scope.owner = Some(crate::core::digest::Digest([234; 32]));
                assert!(
                    boundary
                        .declaration_outer(definition, &wrong_owner)
                        .is_err()
                );
                if associated {
                    wrong_owner.scope.owner = None;
                    assert!(
                        boundary
                            .declaration_outer(definition, &wrong_owner)
                            .is_err()
                    );
                }
            }
            let mut wrong = context.clone();
            wrong.range.end -= 1;
            assert!(boundary.declaration_outer(definition, &wrong).is_err());
            ready(boundary.require_current(&mut db, unit));
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                boundary.require_current(&mut db, unit),
                QueryPoll::Ready(Err(_))
            ));
        }
    }
    // Pure scope/source controls do not execute answer or stock Point/Box main.
}
