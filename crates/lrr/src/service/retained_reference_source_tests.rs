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
                    RunMachine::current_field_source_for_test(&mut db, &sources, indexed, &pc),
                    QueryPoll::Ready(Err(_))
                ));
            }
        }
    }
}
