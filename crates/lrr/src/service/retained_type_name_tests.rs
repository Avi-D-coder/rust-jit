//! LRR-specific source authority controls supplement the unchanged stock cast
//! witness in retained_callable_tests. They do not define alternate cast behavior.
use super::retained_body_tests::prepare_expanded_source;
use crate::{
    core::digest::StableEncode,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    engine::run::{RunMachine, RunPoll, ValueSummary},
    frontend::primitive_type_names::PrimitiveTypeNameAdmission,
    ir::pc::{NodeId, Phase},
    source::manifest::ItemKind,
};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("Type source setup failed: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("Type source setup suspended: {input:?}"),
    }
}
fn setup(
    text: &str,
    edition: u16,
) -> (
    Database,
    crate::source::manifest::BodyKey,
    crate::expand::source_discovery::SourceDiscovery,
) {
    let mut db = Database::default();
    let (input, discovery) = prepare_expanded_source(&mut db, text, edition, 1, true);
    let source = db
        .prepared_source(input.prepared().source())
        .unwrap()
        .unwrap();
    let definition = source
        .outline()
        .items
        .iter()
        .find(|item| {
            item.def_key.item_kind == ItemKind::Function && item.def_key.declared_name == "main"
        })
        .unwrap()
        .def_key
        .fingerprint();
    ready(db.publish_retained_syntax(input.clone()));
    let body = ready(db.publish_retained_body(input, definition));
    (db, body, discovery)
}
const ORIGINAL_TARGET: &str =
    "macro_rules! emit { () => { struct Marker; } } emit!(); fn main() { let x = 7_u8; x as u8; }";

#[test]
fn ordinary_original_type_name_uses_expanded_namespace_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, body, discovery) = setup(ORIGINAL_TARGET, edition);
        let mut run = ready(RunMachine::start_with_discovery(
            &mut db, body, 100_000, discovery,
        ));
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("ordinary original Type name did not complete");
        };
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[12], 1);
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
    }
}
#[test]
fn expanded_type_item_cannot_be_bypassed_by_primitive_spelling_all_editions() {
    const SHADOW: &str = "macro_rules! emit { () => { type u8 = i32; } } emit!(); fn main() { let x = 7_u8; x as u8; }";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, body, discovery) = setup(SHADOW, edition);
        let mut run = ready(RunMachine::start_with_discovery(
            &mut db, body, 100_000, discovery,
        ));
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("shadowed Type name was admitted");
        };
        assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
        assert_eq!(run.opcode_counters().unwrap().started_by_opcode[12], 0);
    }
}
#[test]
fn definition_site_type_name_requires_its_actual_hygienic_scope_all_editions() {
    const MARKED: &str = "macro_rules! value { () => { 7_u8 as u8 } } fn main() { value!(); }";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, body, discovery) = setup(MARKED, edition);
        let mut run = ready(RunMachine::start_with_discovery(
            &mut db, body, 100_000, discovery,
        ));
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("definition-site Type name borrowed caller authority");
        };
        assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
        assert!(
            error
                .message
                .contains("actual original source scope and hygiene")
        );
        assert_eq!(run.opcode_counters().unwrap().started_by_opcode[0], 0);
        assert_eq!(run.opcode_counters().unwrap().started_by_opcode[12], 0);
    }
}
#[test]
fn primitive_name_proof_cannot_move_between_nodes_or_refresh_after_epoch_change() {
    const TWO: &str = "macro_rules! emit { () => { struct Marker; } } emit!(); fn main() { let x = 7_u8; x as u8; x as u8; }";
    let (mut db, body, _) = setup(TWO, 2024);
    let handle = db
        .input(&InputKey::DefinitionBody(body.definition))
        .unwrap();
    let InputValue::DefinitionBody(handle) = handle.as_ref() else {
        panic!("body absent");
    };
    let locator = db
        .input(&InputKey::DefinitionSource(body.definition))
        .unwrap();
    let InputValue::DefinitionSource(parse) = locator.as_ref() else {
        panic!("parse absent");
    };
    let parsed = ready(db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())));
    let index = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body)));
    let (QueryValue::ParseFile(parsed), QueryValue::BodyStructure(index)) =
        (parsed.value.as_ref(), index.value.as_ref())
    else {
        panic!("ordinary parse/index absent");
    };
    let mut sites = Vec::new();
    for n in 0..index.nodes().len() {
        let pc = index.pc(NodeId(n as u32), Phase::Entry).unwrap();
        if let Some(site) = parsed
            .retained_primitive_type_name_site(handle, index, &pc)
            .unwrap()
        {
            sites.push((pc, site));
        }
    }
    assert_eq!(sites.len(), 2);
    let (second, _) = sites.pop().unwrap();
    let (first, site) = sites.pop().unwrap();
    let proof = ready(PrimitiveTypeNameAdmission::capture(
        &mut db, parsed, handle, &first, site,
    ));
    proof
        .lower_current(&mut db, parsed, handle, index, &first)
        .unwrap();
    assert_eq!(
        proof
            .lower_current(&mut db, parsed, handle, index, &second)
            .unwrap_err()
            .code,
        "LRR-E-TYPE-CONTEXT"
    );
    db.begin_expansion_epoch(2).unwrap();
    let error = proof
        .lower_current(&mut db, parsed, handle, index, &first)
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
    assert!(error.message.contains("source or namespace changed"));
}
