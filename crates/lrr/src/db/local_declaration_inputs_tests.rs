//! Publication/authority controls. Language source is the unchanged rustc case;
//! the small attribute program below only exercises a pending producer barrier.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{
        key::OutlineKey,
        provider::{Provided, SyntaxProvider},
        value::QueryValue,
    },
    frontend::source_body_context,
    ir::pc::{NodeId, Phase},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};

const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("local declaration: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("local declaration demand: {input:?}"),
    }
}
fn setup(text: &str, edition: u16) -> (Database, Arc<PreparedBodyContext>) {
    let unit = Digest([242; 32]);
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("local-declaration-authority.rs").unwrap();
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
        BTreeMap::new(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("actual source selection absent")
    };
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.trace.detailed = true;
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!()
    };
    let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
    let admitted = Arc::new(ready(source_body_context::capture(
        &mut db,
        &[&view],
        index,
        &pc,
    )));
    let context = ready(admitted.prepare(&mut db));
    (db, context)
}
fn local(context: &PreparedBodyContext, name: &str) -> Digest {
    context
        .facts()
        .items
        .iter()
        .find(|row| row.item.def_key.declared_name == name)
        .unwrap()
        .item
        .def_key
        .fingerprint()
}
fn stamps(db: &Database) -> BTreeMap<InputKey, Digest> {
    db.inputs
        .iter()
        .map(|(key, row)| (key.clone(), row.fingerprint))
        .collect()
}
fn source_rows(db: &Database) -> BTreeMap<InputKey, InputValue> {
    db.inputs
        .iter()
        // publish_inputs derives these rows from the actual ScopeMembers. A
        // base-universe replacement supplies those original members once,
        // rather than submitting the derived projections a second time.
        .filter(|(key, _)| {
            !matches!(
                key,
                InputKey::ScopeBlockers(_)
                    | InputKey::NameCandidates(_)
                    | InputKey::LocalDeclarationOwner(_)
            )
        })
        .map(|(key, row)| (key.clone(), row.value.as_ref().clone()))
        .collect()
}
fn absent(db: &Database, definition: Digest) {
    for key in [
        InputKey::DefinitionSource(definition),
        InputKey::LocalDeclarationOwner(definition),
        InputKey::DefinitionHeader(definition),
        InputKey::DeclarationContext(definition),
        InputKey::DefinitionBody(definition),
    ] {
        assert!(
            !db.inputs.contains_key(&key),
            "unexpected declaration grant: {key:?}"
        );
    }
}
#[test]
fn unchanged_cold_impl_scopes_publish_headers_without_entering_enclosing_functions() {
    use crate::frontend::associated_declarations::{
        AssociatedDeclarationsPreparation, PreparedOriginalAssociatedDeclarations,
    };
    use crate::source::manifest::ItemKind;
    const COLD: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/privacy/required-pub-in-blocks.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, _, inputs) =
            crate::expand::source_declarative::assembly::tests::world(COLD, edition);
        let view = provider.module_source();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.trace.detailed = true;
        let AssociatedDeclarationsPreparation::Original(joined) =
            PreparedOriginalAssociatedDeclarations::prepare(view).unwrap()
        else {
            panic!("upstream original requires genuine canonical owner");
        };
        assert_eq!(joined.scan().impls().len(), 3);
        let mut signatures = 0;
        for implementation in joined.scan().impls() {
            let scope = joined
                .declaration_scope(implementation.identity)
                .expect("unchanged cold impl has its genuine enclosing function");
            let owner = scope.body().key();
            assert!(
                !db.inputs
                    .contains_key(&InputKey::BodyEnvironment(owner.clone()))
            );
            for row in scope.facts().declarations() {
                let definition = row.item.def_key.fingerprint();
                assert_eq!(
                    ready(db.publish_captured_declaration(&scope, definition)),
                    LocalDeclarationPublication::Published(definition)
                );
                assert!(
                    matches!(db.input(&InputKey::LocalDeclarationOwner(definition)).as_deref(),
                    Some(InputValue::LocalDeclarationOwner(actual)) if actual == &owner)
                );
            }
            for member in &scope.facts().members {
                if member.item.def_key.item_kind != ItemKind::Function {
                    continue;
                }
                let definition = member.item.def_key.fingerprint();
                let selected =
                    ready(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
                assert!(matches!(selected.value.as_ref(),
                    QueryValue::FunctionSignature(signature) if signature.definition == definition));
                signatures += 1;
            }
            assert!(!db.inputs.contains_key(&InputKey::BodyEnvironment(owner)));
        }
        assert_eq!(signatures, 2);
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::SourceLocalPlan(_)
        )));
    }
}
#[test]
fn cold_declaration_publication_rejects_changed_parent_before_any_child_input() {
    use crate::frontend::associated_declarations::{
        AssociatedDeclarationsPreparation, PreparedOriginalAssociatedDeclarations,
    };
    const COLD: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/privacy/required-pub-in-blocks.rs"
    );
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(COLD, 2024);
    let view = provider.module_source();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let AssociatedDeclarationsPreparation::Original(joined) =
        PreparedOriginalAssociatedDeclarations::prepare(view).unwrap()
    else {
        panic!("upstream original owner absent");
    };
    let scope = joined
        .declaration_scope(joined.scan().impls()[0].identity)
        .unwrap();
    let parent = scope.body().def_key;
    let member = scope
        .facts()
        .members
        .first()
        .unwrap()
        .item
        .def_key
        .fingerprint();
    let Some(InputValue::DefinitionHeader(mut changed)) = db
        .input(&InputKey::DefinitionHeader(parent))
        .as_deref()
        .cloned()
    else {
        panic!("actual parent header absent");
    };
    changed.tokens[0].text.push('_');
    let mut rows = source_rows(&db);
    rows.insert(
        InputKey::DefinitionHeader(parent),
        InputValue::DefinitionHeader(changed),
    );
    db.publish_inputs(rows).unwrap();
    let before = stamps(&db);
    let revision = db.revision;
    assert!(matches!(
        db.publish_captured_declaration(&scope, member),
        QueryPoll::Ready(Err(_))
    ));
    assert_eq!(db.revision, revision);
    assert_eq!(stamps(&db), before);
    absent(&db, member);
}
#[test]
fn unchanged_local_structs_publish_selected_headers_for_the_shared_schema_query() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, context) = setup(ORIGINAL, edition);
        for row in &context.facts().items {
            let item = &row.item;
            let definition = item.def_key.fingerprint();
            absent(&db, definition);
            let keys: Vec<_> = db.inputs.keys().cloned().collect();
            let from = db.trace.events.len();
            assert_eq!(
                ready(db.publish_local_declaration(&context, definition)),
                LocalDeclarationPublication::Published(definition)
            );
            assert_eq!(db.inputs.len(), keys.len() + 4);
            assert!(matches!(
                db.input(&InputKey::LocalDeclarationOwner(definition)).as_deref(),
                Some(InputValue::LocalDeclarationOwner(owner))
                    if owner == context.source().index().body()
            ));
            assert_ne!(
                item.scope.owner,
                Some(context.source().index().body().definition)
            );
            assert!(keys.iter().all(|key| db.inputs.contains_key(key)));
            assert!(
                !db.inputs
                    .contains_key(&InputKey::DefinitionBody(definition))
            );
            assert!(
                !db.trace.events[from..]
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::Thunk(_) | QueryKey::TypeSchema(_)))
            );
            let revision = db.revision;
            let before = stamps(&db);
            assert_eq!(
                ready(db.publish_local_declaration(&context, definition)),
                LocalDeclarationPublication::Published(definition)
            );
            assert_eq!(db.revision, revision);
            assert_eq!(stamps(&db), before);
            let schema = ready(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(definition)));
            let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
                panic!()
            };
            assert_eq!(schema.definition, definition);
            assert_eq!(schema.header, item.header);
            assert_eq!(schema.scope, item.scope);
        }
    }
}
#[test]
fn local_header_publication_conflict_refusal_is_atomic() {
    let (mut db, context) = setup(ORIGINAL, 2024);
    let definition = local(&context, "A");
    absent(&db, definition);
    let mut rows = source_rows(&db);
    let mut wrong = context.source().parsed().key();
    wrong.content = Digest([243; 32]);
    rows.insert(
        InputKey::DefinitionSource(definition),
        InputValue::DefinitionSource(wrong),
    );
    db.publish_inputs(rows).unwrap();
    let before = stamps(&db);
    let revision = db.revision;
    assert!(matches!(db.publish_local_declaration(&context, definition),
        QueryPoll::Ready(Err(error)) if error.code == "LRR-E-EXPANSION-CONFLICT"));
    assert_eq!(stamps(&db), before);
    assert_eq!(db.revision, revision);
    assert!(
        !db.inputs
            .contains_key(&InputKey::DefinitionHeader(definition))
    );
    assert!(
        !db.inputs
            .contains_key(&InputKey::DeclarationContext(definition))
    );
}
#[test]
fn local_owner_locator_is_derived_and_conflicts_publish_no_partial_headers() {
    let (mut db, context) = setup(ORIGINAL, 2024);
    let definition = local(&context, "A");
    let mut wrong = context.source().index().body().clone();
    wrong.definition = Digest([249; 32]);
    let key = InputKey::LocalDeclarationOwner(definition);
    let value = InputValue::LocalDeclarationOwner(wrong);
    let mut base = source_rows(&db);
    base.insert(key.clone(), value.clone());
    let before = stamps(&db);
    assert!(db.publish_inputs(base).is_err());
    assert_eq!(stamps(&db), before);
    // Only this adversarial DB control can install an invalid derived locator.
    // The production service must reject it before any other header row appears.
    db.inputs.insert(
        key,
        InputRecord {
            fingerprint: value.fingerprint(),
            value: Arc::new(value),
        },
    );
    let before = stamps(&db);
    let revision = db.revision;
    assert!(matches!(db.publish_local_declaration(&context, definition),
        QueryPoll::Ready(Err(error)) if error.code == "LRR-E-EXPANSION-CONFLICT"));
    assert_eq!(stamps(&db), before);
    assert_eq!(db.revision, revision);
    assert!(
        !db.inputs
            .contains_key(&InputKey::DefinitionHeader(definition))
    );
    assert!(
        !db.inputs
            .contains_key(&InputKey::DeclarationContext(definition))
    );
}
#[test]
fn local_header_publication_requires_current_owner_and_coordinator() {
    let (mut db, context) = setup(ORIGINAL, 2024);
    let definition = local(&context, "A");
    let before = stamps(&db);
    assert!(matches!(
        db.publish_local_declaration(&context, Digest([244; 32])),
        QueryPoll::Ready(Err(_))
    ));
    assert_eq!(stamps(&db), before);
    db.begin_run().unwrap();
    assert!(matches!(db.publish_local_declaration(&context, definition),
        QueryPoll::Ready(Err(error)) if error.code == "LRR-E-INTERNAL-SNAPSHOT"));
    assert_eq!(stamps(&db), before);
    db.finish_run().unwrap();
    struct DuringQuery {
        context: Arc<PreparedBodyContext>,
        definition: Digest,
    }
    impl crate::db::provider::Provider for DuringQuery {
        fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
            match db.publish_local_declaration(&self.context, self.definition) {
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                _ => panic!("pure query unexpectedly published declaration"),
            }
        }
    }
    assert!(
        matches!(db.fetch(&DuringQuery { context: Arc::clone(&context), definition },
        QueryKey::Signature(definition)), QueryPoll::Ready(Err(error))
        if error.code == "LRR-E-INTERNAL-SNAPSHOT")
    );
    assert_eq!(stamps(&db), before);
    let mut rows = source_rows(&db);
    rows.remove(&InputKey::DefinitionHeader(context.source().body().def_key));
    db.publish_inputs(rows).unwrap();
    let before = stamps(&db);
    assert!(matches!(
        db.publish_local_declaration(&context, definition),
        QueryPoll::Ready(Err(_))
    ));
    assert_eq!(stamps(&db), before);
    absent(&db, definition);
}
#[test]
fn local_attribute_producer_does_not_publish_unexpanded_header() {
    let (mut db, context) = setup("fn main() { #[pending_attribute] struct Subject; }", 2024);
    let row = context
        .facts()
        .items
        .iter()
        .find(|row| row.item.def_key.declared_name == "Subject")
        .unwrap();
    let definition = row.item.def_key.fingerprint();
    let producer = row
        .attribute_input
        .expect("actual attribute producer absent");
    let before = stamps(&db);
    let revision = db.revision;
    assert_eq!(
        ready(db.publish_local_declaration(&context, definition)),
        LocalDeclarationPublication::NeedsProducer(producer)
    );
    assert_eq!(stamps(&db), before);
    assert_eq!(db.revision, revision);
    absent(&db, definition);
}
