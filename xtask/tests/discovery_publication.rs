use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> (PreparedSourceView, PreparedSourceInputs) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([241; 32]);
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
    (view, inputs)
}

fn discover(v: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}

fn finish(w: &mut SourceDiscovery) {
    loop {
        match w.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. }
            | SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::Incomplete { .. } => {
                panic!("fixture failed to complete discovery")
            }
        }
    }
}
#[test]
fn discovery_query_suspends_without_memo_and_publishes_record_bound_seal_atomically() {
    use lrr::db::{
        dependency::{DependencyStamp, InputKey},
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
        value::QueryValue,
    };
    let (v, inputs) = source(
        "macro_rules! leaf{()=>{pub fn made(){7_i64;}}}macro_rules! seed{()=>{leaf!();}}seed!();",
    );
    let mut w = discover(&v);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let key = QueryKey::DiscoverySeal(w.key().clone());
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::NeedInput(InputRequest::Discover { .. })
    ));
    assert!(db.memo(&key).is_none());
    finish(&mut w);
    let before = db.revision();
    let base = db.source_revision();
    w.publish(&mut db).unwrap();
    assert_eq!(db.revision(), before + 1);
    assert_eq!(db.source_revision(), base);
    let QueryPoll::Ready(Ok(outcome)) = db.fetch(&SyntaxProvider, key.clone()) else {
        panic!("seal query not ready")
    };
    let QueryValue::DiscoverySeal(seal) = outcome.value.as_ref() else {
        panic!("wrong query value")
    };
    assert_eq!(seal.key(), *w.key());
    assert_eq!(seal.unresolved_frontiers(), 0);
    let dependencies = db.memo(&key).unwrap().dependencies.clone();
    for record in w.sealed_expansions().unwrap() {
        assert!(dependencies.iter().any(|d|matches!(d,DependencyStamp::Input{key:InputKey::ExpansionRecord(id),..} if *id==record.key().fingerprint())));
        assert!(matches!(
            db.fetch(
                &SyntaxProvider,
                QueryKey::ExpansionInput(Box::new(record.key().clone()))
            ),
            QueryPoll::Ready(Ok(_))
        ));
    }
    let revision = db.revision();
    w.publish(&mut db).unwrap();
    assert_eq!(db.revision(), revision);
}
#[test]
fn incomplete_discovery_cannot_publish_any_record_or_seal_input() {
    use lrr::db::{dependency::InputKey, memo::Database};
    let (v, inputs) = source("macro_rules! make{()=>{}}make!();unknown!();");
    let mut w = discover(&v);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    w.admit_expansion(record.clone()).unwrap();
    assert!(matches!(
        w.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    let revision = db.revision();
    assert_eq!(
        w.publish(&mut db).unwrap_err().code,
        "LRR-E-CATALOG-INCOMPLETE"
    );
    assert_eq!(db.revision(), revision);
    assert!(
        db.input(&InputKey::ExpansionRecord(record.key().fingerprint()))
            .is_none()
    );
    assert!(
        db.input(&InputKey::DiscoverySeal(w.key().clone()))
            .is_none()
    );
}
#[test]
fn stale_captured_source_refuses_discovery_publication_and_invalidates_ready_seal() {
    use lrr::db::{
        dependency::InputKey, key::QueryKey, memo::Database, poll::QueryPoll,
        provider::SyntaxProvider,
    };
    let (v, inputs) = source("macro_rules! make{()=>{pub fn made(){}}}make!();");
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    let key = QueryKey::DiscoverySeal(w.key().clone());
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    let (_, changed) = source("pub fn replacement(){}");
    changed.publish(&mut db).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, key),
        QueryPoll::NeedInput(_)
    ));
    let revision = db.revision();
    assert_eq!(
        w.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.revision(), revision);
    assert!(
        db.input(&InputKey::DiscoverySeal(w.key().clone()))
            .is_none()
    );
}
#[test]
fn discovery_publication_budget_refusal_has_no_partial_inputs() {
    use lrr::db::{dependency::InputKey, memo::Database};
    let (v, inputs) = source("macro_rules! make{()=>{pub fn made(){7_i64;}}}make!();");
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let bytes = db.cache_usage().unwrap().memo_bytes;
    db.begin_run_with_budgets(lrr::engine::telemetry::CacheBudgets {
        query_bytes: bytes + 1,
        ..Default::default()
    })
    .unwrap();
    let revision = db.revision();
    assert_eq!(w.publish(&mut db).unwrap_err().code, "LRR-E-METADATA-LIMIT");
    assert_eq!(db.revision(), revision);
    for record in w.sealed_expansions().unwrap() {
        assert!(
            db.input(&InputKey::ExpansionRecord(record.key().fingerprint()))
                .is_none()
        );
    }
    assert!(
        db.input(&InputKey::DiscoverySeal(w.key().clone()))
            .is_none()
    );
    db.finish_run().unwrap();
}
#[test]
fn pure_provider_cannot_publish_completed_discovery() {
    use lrr::db::{
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::{Provided, Provider},
    };
    struct Mutator<'a>(&'a SourceDiscovery);
    impl Provider for Mutator<'_> {
        fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
            QueryPoll::Ready(Err(self.0.publish(db).unwrap_err()))
        }
    }
    let (v, inputs) = source("pub fn ordinary(){}");
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let before = db.revision();
    let key = QueryKey::Constant(fixture_key(
        Digest([250; 32]),
        vec![],
        Digest([251; 32]),
        ProfileIdentity::current(),
    ));
    let QueryPoll::Ready(Err(error)) = db.fetch(&Mutator(&w), key) else {
        panic!("pure provider mutated inputs")
    };
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(db.revision(), before);
}

fn adopt(
    db: &mut lrr::db::memo::Database,
    record: &lrr::expand::record::ExpansionRecord,
) -> Result<(), lrr::core::diagnostic::Diagnostic> {
    use lrr::{
        db::{key::QueryKey, poll::InputRequest},
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        source::manifest::BodyKey,
    };
    let context = DemandContext {
        request: Digest([10; 32]),
        session: Digest([11; 32]),
        unit: Digest([1; 32]),
        revision: db.source_revision(),
        epoch: 1,
        source_generation: Digest([12; 32]),
        native_generation: None,
        compile_environment: record.key().consumer_compile_env,
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator.issue(
        QueryKey::ExpansionInput(Box::new(record.key().clone())),
        PhaseIdentity {
            frame: Digest([13; 32]),
            body: BodyKey {
                definition: Digest([14; 32]),
                exact_body_bytes: Digest([15; 32]),
            },
            index: 0,
        },
        InputRequest::Expand(Box::new(lrr::db::poll::ExpansionRequest {
            key: record.key().clone(),
        })),
    )?;
    coordinator.start(&context, &token)?;
    coordinator.accept(&context, &token, Reply::success(&token, record.output()))?;
    coordinator.adopt(&context, &token, db, record.clone())
}

#[test]
fn discovery_record_conflict_rejects_entire_batch_before_first_append() {
    use lrr::db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    };
    let (v, inputs) = source("macro_rules! noop{()=>{}}noop!();noop!();");
    let mut w = discover(&v);
    finish(&mut w);
    let records = w.sealed_expansions().unwrap().cloned().collect::<Vec<_>>();
    assert_eq!(records.len(), 2);
    let bad = lrr::expand::record::ExpansionRecord::from_tokens(
        records[1].key().clone(),
        &records[0].tokens().unwrap(),
    )
    .unwrap();
    assert_ne!(bad.output(), records[1].output());
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    adopt(&mut db, &bad).unwrap();
    let revision = db.revision();
    assert_eq!(
        w.publish(&mut db).unwrap_err().code,
        "LRR-E-EXPANSION-CONFLICT"
    );
    assert_eq!(db.revision(), revision);
    assert!(
        db.input(&InputKey::ExpansionRecord(records[0].key().fingerprint()))
            .is_none()
    );
    let input = db
        .input(&InputKey::ExpansionRecord(records[1].key().fingerprint()))
        .unwrap();
    let InputValue::ExpansionRecord(actual) = input.as_ref() else {
        panic!("existing record changed kind")
    };
    assert_eq!(actual.output(), bad.output());
    assert!(
        db.input(&InputKey::DiscoverySeal(w.key().clone()))
            .is_none()
    );
}
#[test]
fn deleted_capture_cannot_reuse_identical_content_from_another_location() {
    use lrr::db::{dependency::InputKey, memo::Database};
    let text = "pub fn ordinary(){}";
    let (v, inputs) = source(text);
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let other = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path: SourcePath::new("other.rs").unwrap(),
            crate_key: v.outline().root_scope.crate_key,
            module_path: vec!["other".into()],
            profile: ProfileIdentity::current(),
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        }],
        Default::default(),
    )
    .unwrap();
    other.publish(&mut db).unwrap();
    assert!(
        db.input(&InputKey::SourceBlob(v.selection().outline.parse.content))
            .is_some()
    );
    let before = db.revision();
    assert_eq!(
        w.publish(&mut db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.revision(), before);
    assert!(
        db.input(&InputKey::DiscoverySeal(w.key().clone()))
            .is_none()
    );
}

include!("support/constant_key.rs");
