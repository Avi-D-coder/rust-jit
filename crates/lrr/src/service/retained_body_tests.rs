//! Common body publication/execution controls. Upstream originals stay unchanged;
//! supplemental programs exercise LRR currentness and representation invariants.
use super::retained_syntax_tests::{admit, fixture, seal};
use crate::{
    core::digest::Digest,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        retained_syntax::RetainedSyntaxInput,
        value::QueryValue,
    },
    engine::run::{RunMachine, RunPoll, ValueSummary},
    expand::{
        source_declarative::{
            self,
            assembly::{PreparedSourceSyntax, SourceSyntaxAssembly, tests as assembly},
        },
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::manifest::{BodyKey, Namespace},
    syntax::body_index::{ExpressionKind, NodeRole},
};
use std::sync::Arc;

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("unexpected component failure: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("unexpected component suspension: {input:?}"),
    }
}
fn original(db: &Database, prepared: &PreparedSourceSyntax, name: &str) -> BodyKey {
    db.prepared_source(prepared.source())
        .unwrap()
        .unwrap()
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
fn unchanged_stock_body_publishes_atomically_only_before_frame_entry_in_every_edition() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut f = fixture(edition, true);
        seal(&mut f);
        let input = admit(&mut f);
        ready(f.db.publish_retained_syntax(input.clone()));
        let body = original(&f.db, &f.prepared, "main");
        let old = ready(RunMachine::start(&mut f.db, body.clone(), 100_000));
        let revision = f.db.revision();
        let source_revision = f.db.source_revision();
        assert!(matches!(
            f.db.publish_retained_body(input.clone(), body.definition),
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-BODY-ACTIVE"
        ));
        assert_eq!(f.db.revision(), revision);
        let Some((InputValue::DefinitionSource(parse), _)) =
            f.db.published_input(&InputKey::DefinitionSource(body.definition))
        else {
            panic!("original source row disappeared")
        };
        assert_eq!(parse, &f.prepared.source().outline.parse);
        drop(old);
        let retained = ready(f.db.publish_retained_body(input.clone(), body.definition));
        assert_eq!(retained.definition, body.definition);
        assert_ne!(retained, body);
        assert_eq!(f.db.source_revision(), source_revision);
        let revision = f.db.revision();
        assert_eq!(
            ready(f.db.publish_retained_body(input.clone(), body.definition)),
            retained
        );
        assert_eq!(f.db.revision(), revision);
        let Some((InputValue::DefinitionSource(parse), _)) =
            f.db.published_input(&InputKey::DefinitionSource(body.definition))
        else {
            panic!("retained source row absent")
        };
        assert_eq!(parse, input.key());
        let Some((InputValue::DefinitionBody(handle), _)) =
            f.db.published_input(&InputKey::DefinitionBody(body.definition))
        else {
            panic!("retained actual body attachment absent")
        };
        let anchor = f
            .prepared
            .original_declarations()
            .iter()
            .find(|anchor| anchor.original_definition() == body.definition)
            .unwrap();
        assert_eq!(Some(handle.range), anchor.body_range());
        assert!(
            f.db.published_input(&InputKey::SourceBlob(input.key().content))
                .is_none()
        );
        let indexed = ready(f.db.fetch(&SyntaxProvider, QueryKey::BodyStructure(retained)));
        let QueryValue::BodyStructure(indexed) = indexed.value.as_ref() else {
            panic!("ordinary body structure absent")
        };
        assert!(
            !indexed
                .nodes()
                .iter()
                .any(|node| matches!(node.role, NodeRole::Expression(ExpressionKind::Macro)))
        );
    }
    // The stock main's generic member calls are not executed by this control.
}

const ARITHMETIC: &str =
    "macro_rules! compute { () => { 2_i32 + 5_i32 } } fn value()->i32 { compute!() } fn main() {}";

// Actual source/request/record/discovery preparation, never a synthetic callee
// inserted by production lowering. `value` belongs to the original program.
fn prepared_arithmetic(
    db: &mut Database,
    edition: u16,
    epoch: u64,
    publish_source: bool,
) -> Arc<RetainedSyntaxInput> {
    prepare_single_expansion(db, ARITHMETIC, edition, epoch, publish_source).0
}
pub(super) fn prepare_single_expansion(
    db: &mut Database,
    text: &str,
    edition: u16,
    epoch: u64,
    publish_source: bool,
) -> (Arc<RetainedSyntaxInput>, SourceDiscovery) {
    let (provider, index, inputs) = assembly::world(text, edition);
    // Reuse the actual ordinary-item request path for module producers. Its
    // invocation ID is intentionally different from declaration-scan identity.
    let (request, output) =
        if let Some(call) = provider.module_source().outline().item_invocations.first() {
            let source_declarative::SourceDeclarativeDemand::Ready(request) = provider
                .demand(
                    call.item_input,
                    Digest([17; 32]),
                    source_declarative::engine_version(),
                )
                .unwrap()
            else {
                panic!("actual ordinary item request absent")
            };
            let output = Arc::new(
                crate::expand::generated_declarations::GeneratedDeclarations::admit_item_output(
                    Arc::new(request.expand().unwrap()),
                    epoch,
                    provider.module_source(),
                    provider.module_source().captured_macro_cfg().unwrap(),
                    Default::default(),
                    &request.source_base().unwrap(),
                )
                .unwrap(),
            );
            (request, output)
        } else {
            let request = assembly::request(&provider, &index);
            let output = assembly::output(&provider, index.scan(), &request, epoch);
            (request, output)
        };
    let prepared = Arc::new(
        SourceSyntaxAssembly::new(&provider, &index, epoch)
            .unwrap()
            .attach(&request, output)
            .unwrap()
            .finish()
            .unwrap(),
    );
    let source = provider.module_source();
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest([242; 32]),
        epoch,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([17; 32]),
        source_declarative::engine_version(),
        Arc::new(
            CompileInputCatalog::standalone(&source.selection().outline.parse.profile).unwrap(),
        ),
    )
    .unwrap();
    if publish_source {
        inputs.publish(db).unwrap();
    }
    db.begin_expansion_epoch(epoch).unwrap();
    for _ in 0..16 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(db).unwrap();
                let input = ready(RetainedSyntaxInput::prepare(
                    db,
                    prepared,
                    discovery.key().clone(),
                ));
                return (input, discovery);
            }
            _ => panic!("actual arithmetic discovery requires an unhandled phase"),
        }
    }
    panic!("actual arithmetic discovery did not close")
}

#[test]
fn user_macro_nonliteral_output_executes_ordinary_numeric_lowering_in_every_edition() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut db = Database::default();
        db.trace.detailed = true;
        let input = prepared_arithmetic(&mut db, edition, 1, true);
        let original = original(&db, input.prepared(), "value");
        ready(db.publish_retained_syntax(input.clone()));
        let body = ready(db.publish_retained_body(input, original.definition));
        let mut run = ready(RunMachine::start(&mut db, body.clone(), 100_000));
        let RunPoll::Completed(report) = run.poll(&mut db) else {
            panic!("ordinary expanded arithmetic did not execute")
        };
        assert!(matches!(report.value, ValueSummary::Signed(ref value) if value == "7"));
        assert_eq!(report.opcode_counters.started_by_opcode[10], 1);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
        assert!(db.trace.events.iter().all(|event| match &event.key {
            QueryKey::Thunk(pc) => pc.body == body,
            _ => true,
        }));
    }
}

#[test]
fn unowned_body_can_advance_epoch_but_an_old_frame_cannot_execute_or_be_retargeted() {
    let mut db = Database::default();
    let first = prepared_arithmetic(&mut db, 2024, 1, true);
    let original = original(&db, first.prepared(), "value");
    ready(db.publish_retained_syntax(first.clone()));
    let body = ready(db.publish_retained_body(first, original.definition));
    let mut run = ready(RunMachine::start(&mut db, body.clone(), 100_000));
    let source_revision = db.source_revision();
    let second = prepared_arithmetic(&mut db, 2024, 2, false);
    ready(db.publish_retained_syntax(second.clone()));
    assert!(
        matches!(db.publish_retained_body(second.clone(), body.definition),
        QueryPoll::Ready(Err(error)) if error.code == "LRR-E-BODY-ACTIVE")
    );
    assert!(matches!(run.poll(&mut db), RunPoll::Fault(error)
        if error.code == "LRR-E-EXPANSION-INPUT"));
    assert_eq!(db.source_revision(), source_revision);
    // Failed revalidation must run no guest arithmetic.
    let facts = run.terminal_facts();
    assert!(facts.is_some());
    drop(run);
    let body = ready(db.publish_retained_body(second.clone(), body.definition));
    let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(body.definition))
    else {
        panic!("new retained source absent")
    };
    assert_eq!(parse, second.key());
    let mut run = ready(RunMachine::start(&mut db, body, 100_000));
    assert!(matches!(run.poll(&mut db), RunPoll::Completed(_)));
}

#[test]
fn body_publication_requires_the_actual_typed_source_and_original_anchor() {
    let mut db = Database::default();
    let input = prepared_arithmetic(&mut db, 2024, 1, true);
    let body = original(&db, input.prepared(), "value");
    let revision = db.revision();
    assert!(
        matches!(db.publish_retained_body(input.clone(), body.definition),
        QueryPoll::Ready(Err(error)) if error.code == "LRR-E-EXPANSION-INPUT")
    );
    assert_eq!(db.revision(), revision);
    ready(db.publish_retained_syntax(input.clone()));
    let revision = db.revision();
    assert!(
        matches!(db.publish_retained_body(input, Digest::of_bytes(b"foreign declaration")),
        QueryPoll::Ready(Err(error)) if error.code == "LRR-E-EXPANSION-INPUT")
    );
    assert_eq!(db.revision(), revision);
}
