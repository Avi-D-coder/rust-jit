//! Exact metadata-to-executable authority joins over an unchanged rustc source.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{key::OutlineKey, provider::SyntaxProvider, value::QueryValue},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        callables::admit_lexical_function,
        source_body_context::{self, PreparedBodyContext},
    },
    ir::pc::{NodeId, Phase},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{lexical_numeric_callees::LexicalFunctionPlan, parse::ParseKey},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("lexical header authority: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("lexical header demand: {input:?}"),
    }
}
fn setup(text: &str, edition: u16) -> (Database, PreparedSourceView, SourceDiscovery) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("lexical-numeric-original.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let unit = Digest::of_bytes(b"LRR lexical numeric source");
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
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!()
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"LRR lexical numeric discovery"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR lexical numeric declarative"),
        Digest::of_bytes(b"LRR lexical numeric compiler"),
        std::sync::Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                .unwrap(),
        ),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("lexical discovery incomplete: {diagnostic:?} {blocked:?}"),
            _ => panic!("lexical source discovery requires additional actual authority"),
        }
    }
    (db, view, discovery)
}

fn world(edition: u16) -> (Database, PreparedSourceView, Arc<PreparedBodyContext>) {
    let (mut db, view, discovery) = setup(ORIGINAL, edition);
    discovery.publish(&mut db).unwrap();
    let main = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap();
    let body = main.body.as_ref().unwrap();
    let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!("actual main index absent")
    };
    let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
    let admitted = Arc::new(ready(source_body_context::capture(
        &mut db,
        &[&view],
        index,
        &pc,
    )));
    let context = ready(admitted.prepare(&mut db));
    (db, view, context)
}
fn definition(context: &PreparedBodyContext) -> Digest {
    context
        .facts()
        .items
        .iter()
        .find(|row| row.item.def_key.declared_name == "id_i8")
        .unwrap()
        .item
        .def_key
        .fingerprint()
}
fn plan(db: &Database, context: &PreparedBodyContext) -> LexicalFunctionPlan {
    let source = context.source();
    let call = "id_i8(j)";
    let at = u32::try_from(ORIGINAL.find(call).unwrap()).unwrap() - source.body().range.start;
    let index = source.index();
    let node = index
        .nodes()
        .iter()
        .find(|node| node.range.start == at && node.range.end == at + 8)
        .unwrap();
    source
        .parsed()
        .lexical_numeric_function_plan(
            source.body(),
            index,
            &index.pc(node.id, Phase::Entry).unwrap(),
            source.header(),
            source.context(),
            definition(context),
            "id_i8",
            db.source_revision(),
            db.revision(),
        )
        .unwrap()
}
fn publish_header(db: &mut Database, context: &PreparedBodyContext) {
    let id = definition(context);
    assert!(matches!(ready(db.publish_local_declaration(context, id)),
        crate::db::memo::local_declaration_inputs::LocalDeclarationPublication::Published(actual) if actual == id));
    assert!(!db.inputs.contains_key(&InputKey::DefinitionBody(id)));
}
fn stamps(db: &Database) -> BTreeMap<InputKey, Digest> {
    db.inputs
        .iter()
        .map(|(key, row)| (key.clone(), row.fingerprint))
        .collect()
}
fn assert_no_child_preparation(db: &Database, context: &PreparedBodyContext) {
    assert!(db.trace.events.iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => body == context.source().index().body(),
        QueryKey::Thunk(_) => false,
        _ => true,
    }));
}
#[test]
fn exact_cold_local_function_headers_admit_only_the_reached_body_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, context) = world(edition);
        publish_header(&mut db, &context);
        let id = definition(&context);
        let metadata = stamps(&db);
        let namespace = db.lexical_namespace_stamps().unwrap();
        let source_revision = db.source_revision();
        db.begin_run().unwrap();
        let selected = plan(&db, &context);
        let revision = db.revision();
        let callee = ready(admit_lexical_function(&mut db, &view, &selected));
        assert_eq!(db.revision(), revision + 1);
        assert_eq!(db.source_revision(), source_revision);
        assert_eq!(db.lexical_namespace_stamps().unwrap(), namespace);
        for (key, fingerprint) in metadata {
            assert_eq!(db.inputs[&key].fingerprint, fingerprint);
        }
        assert_eq!(callee.body(), selected.item().body.as_ref());
        assert!(
            matches!(db.inputs[&InputKey::DefinitionBody(id)].value.as_ref(),
            InputValue::DefinitionBody(body) if Some(body) == selected.item().body.as_ref())
        );
        let revision = db.revision();
        let selected = plan(&db, &context);
        ready(admit_lexical_function(&mut db, &view, &selected));
        assert_eq!(db.revision(), revision);
        assert_no_child_preparation(&db, &context);
        db.finish_run().unwrap();
    }
}
#[test]
fn partial_lexical_metadata_requires_its_actual_cold_owner_and_all_three_exact_rows() {
    // These mutations are adversarial DB controls, never production publication.
    for variant in 0..6 {
        let (mut db, view, context) = world(2024);
        publish_header(&mut db, &context);
        let id = definition(&context);
        let owner_key = InputKey::LocalDeclarationOwner(id);
        match variant {
            0 => {
                db.inputs.remove(&owner_key);
            }
            1 => {
                let mut owner = context.source().index().body().clone();
                owner.definition = Digest([239; 32]);
                let value = InputValue::LocalDeclarationOwner(owner);
                db.inputs.insert(
                    owner_key,
                    InputRecord {
                        fingerprint: value.fingerprint(),
                        value: Arc::new(value),
                    },
                );
            }
            2 => {
                db.inputs.remove(&InputKey::DefinitionHeader(id));
            }
            3 => {
                db.inputs.remove(&InputKey::DeclarationContext(id));
            }
            4 | 5 => {
                let key = InputKey::DeclarationContext(id);
                let old_fingerprint = db.inputs[&key].fingerprint;
                let InputValue::DeclarationContext(prior) = db.inputs[&key].value.as_ref() else {
                    panic!()
                };
                let mut changed = prior.clone();
                changed.range.start += 1;
                let value = InputValue::DeclarationContext(changed);
                db.inputs.insert(
                    key,
                    InputRecord {
                        fingerprint: if variant == 5 {
                            old_fingerprint
                        } else {
                            value.fingerprint()
                        },
                        value: Arc::new(value),
                    },
                );
            }
            _ => unreachable!(),
        }
        db.begin_run().unwrap();
        let selected = plan(&db, &context);
        let before = stamps(&db);
        let revision = db.revision();
        match admit_lexical_function(&mut db, &view, &selected) {
            QueryPoll::Ready(Err(error)) => {
                let (code, message) = match variant {
                    0 | 2 | 3 => (
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "lexical definition has a partial prior publication",
                    ),
                    1 => (
                        "LRR-E-CALL-CONTEXT",
                        "lexical cold declaration belongs to another owning body",
                    ),
                    4 | 5 => (
                        "LRR-E-CALL-CONTEXT",
                        "lexical immutable definition row conflict",
                    ),
                    _ => unreachable!(),
                };
                assert_eq!(error.code, code, "{error:?}");
                assert_eq!(error.message, message, "{error:?}");
            }
            _ => panic!("unproved partial metadata admitted, variant {variant}"),
        }
        assert_eq!(stamps(&db), before);
        assert_eq!(db.revision(), revision);
        assert!(!db.inputs.contains_key(&InputKey::DefinitionBody(id)));
        assert_no_child_preparation(&db, &context);
        db.finish_run().unwrap();
    }
}
#[test]
fn exact_cold_headers_body_publication_respects_the_existing_metadata_cap() {
    let (mut db, view, context) = world(2024);
    publish_header(&mut db, &context);
    let selected = plan(&db, &context);
    let budget = db.cache_usage().unwrap().memo_bytes;
    db.begin_run_with_budgets(crate::engine::telemetry::CacheBudgets {
        query_bytes: budget,
        ..Default::default()
    })
    .unwrap();
    let before = stamps(&db);
    let revision = db.revision();
    match admit_lexical_function(&mut db, &view, &selected) {
        QueryPoll::Ready(Err(error)) => {
            assert_eq!(error.code, "LRR-E-METADATA-LIMIT", "{error:?}");
            assert_eq!(
                error.message, "lexical definition publication exceeds cache cap",
                "{error:?}"
            );
        }
        _ => panic!("cold headers bypassed reached-body metadata cap"),
    }
    assert_eq!(stamps(&db), before);
    assert_eq!(db.revision(), revision);
    assert!(
        !db.inputs
            .contains_key(&InputKey::DefinitionBody(definition(&context)))
    );
    assert_no_child_preparation(&db, &context);
    db.finish_run().unwrap();
}

fn owning_world(
    edition: u16,
) -> (
    Database,
    PreparedSourceView,
    Arc<crate::frontend::source_locals::PreparedSourceLocalPlan>,
) {
    let (mut db, view, discovery) = setup(ORIGINAL, edition);
    db.begin_expansion_epoch(1).unwrap();
    discovery.publish(&mut db).unwrap();
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
    let parent = ready(db.prepare_source_local_plan(&discovery, &[&view], &body));
    assert_no_child_preparation(&db, parent.context());
    (db, view, parent)
}
#[test]
fn entered_local_function_source_uses_actual_parent_plan_and_shared_nested_scope_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, parent) = owning_world(edition);
        let selected = plan(&db, parent.context());
        let child = selected.item().body.as_ref().unwrap().key();
        // Preparing the parent and all cold header facts did not index this
        // child's executable syntax. Only the actual selected call below does.
        assert!(db.trace.events.iter().all(|event| !matches!(&event.key,
            QueryKey::BodyStructure(body) if body == &child)));
        db.begin_run().unwrap();
        let callee = ready(admit_lexical_function(&mut db, &view, &selected));
        let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(child.clone())));
        let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
            panic!("actual entered child index absent")
        };
        let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
        let admitted = Arc::new(ready(source_body_context::capture_local(
            &mut db,
            Arc::clone(&parent),
            &callee,
            index,
            &entry,
        )));
        // Allocation equality is evidence of sharing only, never source authority.
        assert!(Arc::ptr_eq(admitted.index(), index));
        assert!(Arc::ptr_eq(admitted.local_parent().unwrap(), &parent));
        assert_eq!(admitted.body().key(), child);
        ready(admitted.require_current(&mut db));
        let selected_again = plan(&db, parent.context());
        assert!(selected_again.query_revision() > selected.query_revision());
        let callee_again = ready(admit_lexical_function(&mut db, &view, &selected_again));
        let admitted_again = ready(source_body_context::capture_local(
            &mut db,
            Arc::clone(&parent),
            &callee_again,
            index,
            &entry,
        ));
        assert_eq!(admitted_again.identity(), admitted.identity());
        let context = ready(Arc::clone(&admitted).prepare(&mut db));
        assert!(context.facts().items.is_empty());
        let prepared = ready(
            crate::frontend::source_locals::PreparedSourceLocalPlan::prepare(
                &mut db,
                parent.environment().unwrap().macros().index(),
                context,
            ),
        );
        assert!(prepared.allocation_roots().namespace().is_some());
        assert_eq!(prepared.module_scope(), parent.module_scope());
        assert!(Arc::ptr_eq(
            prepared.lexical_outline_root(),
            parent.lexical_outline_root()
        ));
        assert!(db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) =>
                body == &child || body == parent.context().source().index().body(),
            QueryKey::Thunk(_) => false,
            _ => true,
        }));
        // This source component prepares no published child environment and
        // performs no child interpreter operation. The engine owns that join.
        assert!(!db.inputs.contains_key(&InputKey::BodyEnvironment(child)));
        db.finish_run().unwrap();
    }
}
#[test]
fn entered_local_source_rejects_wrong_entry_owner_and_revoked_parent_without_publication() {
    let (mut db, view, parent) = owning_world(2024);
    let selected = plan(&db, parent.context());
    db.begin_run().unwrap();
    let callee = ready(admit_lexical_function(&mut db, &view, &selected));
    let child = callee.body().unwrap().key();
    let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(child.clone())));
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!()
    };
    let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
    let mut wrong_entry = entry.clone();
    wrong_entry.phase = 1;
    let revision = db.revision();
    let before = stamps(&db);
    match source_body_context::capture_local(
        &mut db,
        Arc::clone(&parent),
        &callee,
        index,
        &wrong_entry,
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(
            error.message, "local body requires its actual callee entry PC",
            "{error:?}"
        ),
        _ => panic!("local body accepted a non-entry PC"),
    }
    assert_eq!(db.revision(), revision);
    assert_eq!(stamps(&db), before);
    let owner_key = InputKey::LocalDeclarationOwner(callee.identity());
    let old_owner = db.inputs[&owner_key].clone();
    // Adversarial private DB control: production cannot publish this owner row.
    let mut wrong = parent.context().source().index().body().clone();
    wrong.definition = Digest([238; 32]);
    let value = InputValue::LocalDeclarationOwner(wrong);
    db.inputs.insert(
        owner_key.clone(),
        InputRecord {
            fingerprint: value.fingerprint(),
            value: Arc::new(value),
        },
    );
    match source_body_context::capture_local(&mut db, Arc::clone(&parent), &callee, index, &entry) {
        QueryPoll::Ready(Err(error)) => assert_eq!(
            error.message, "local body actual owning declaration differs",
            "{error:?}"
        ),
        _ => panic!("local body accepted a manufactured parent"),
    }
    db.inputs.insert(owner_key, old_owner);
    let admitted = ready(source_body_context::capture_local(
        &mut db,
        Arc::clone(&parent),
        &callee,
        index,
        &entry,
    ));
    db.inputs.remove(&InputKey::BodyEnvironment(
        parent.context().source().index().body().clone(),
    ));
    match admitted.require_current(&mut db) {
        QueryPoll::Ready(Err(error)) => assert_eq!(
            error.message, "published local environment was replaced",
            "{error:?}"
        ),
        _ => panic!("local body outlived its actual parent environment"),
    }
    assert!(!db.inputs.contains_key(&InputKey::BodyEnvironment(child)));
    db.finish_run().unwrap();
}
