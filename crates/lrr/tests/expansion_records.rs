use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::{ExpansionRequest, InputRequest, QueryPoll},
        provider::{Provided, Provider, SyntaxProvider},
    },
    expand::{
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
    },
    service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
    source::manifest::BodyKey,
    syntax::token_wire::{
        ContextRecord, Edition, Key, SourceIdentity, SpanRecord, TokenForest, TokenTree,
        Transparency,
    },
};
fn key() -> ExpansionKey {
    ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: Digest([2; 32]),
        input_tokens: Digest([3; 32]),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest([5; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    }
}
fn record(value: &str) -> ExpansionRecord {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([7; 32]),
        edition: Edition::E2024,
    };
    let span = SpanRecord {
        source: SourceIdentity {
            unit: Key([7; 32]),
            content: Key(Digest::of_bytes(value.as_bytes()).0),
            path: "macro.rs".into(),
        },
        start: 0,
        end: value.len() as u32,
        context: context.key(),
        edition: Edition::E2024,
        origin: Key([8; 32]),
    };
    let tokens = TokenForest::new(
        vec![TokenTree::Literal {
            token_text: value.into(),
            span: span.key(),
        }],
        vec![context],
        vec![span],
    )
    .unwrap();
    ExpansionRecord::from_tokens(key(), &tokens).unwrap()
}
fn context(db: &Database) -> DemandContext {
    DemandContext {
        request: Digest([9; 32]),
        session: Digest([10; 32]),
        unit: Digest([7; 32]),
        revision: db.source_revision(),
        epoch: 11,
        source_generation: Digest([12; 32]),
        native_generation: None,
        compile_environment: key().consumer_compile_env,
    }
}
fn phase() -> PhaseIdentity {
    PhaseIdentity {
        frame: Digest([13; 32]),
        body: BodyKey {
            definition: Digest([14; 32]),
            exact_body_bytes: Digest([15; 32]),
        },
        index: 3,
    }
}
fn adopt(
    coordinator: &mut ExpansionCoordinator,
    context: &DemandContext,
    db: &mut Database,
    record: ExpansionRecord,
) {
    let request = InputRequest::Expand(Box::new(ExpansionRequest { key: key() }));
    let token = coordinator
        .issue(QueryKey::ExpansionInput(Box::new(key())), phase(), request)
        .unwrap();
    if matches!(
        coordinator.start(context, &token).unwrap(),
        lrr::service::expansion::StartDecision::Start
    ) {
        coordinator
            .accept(context, &token, Reply::success(&token, record.output()))
            .unwrap();
    }
    coordinator.adopt(context, &token, db, record).unwrap();
}
#[test]
fn absent_expansion_is_unmemoized_and_admitted_record_resolves_pure_query() {
    let mut db = Database::default();
    let query = QueryKey::ExpansionInput(Box::new(key()));
    assert!(
        matches!(db.fetch(&SyntaxProvider,query.clone()),QueryPoll::NeedInput(InputRequest::Expand(r)) if r.key==key())
    );
    assert!(db.memo(&query).is_none());
    let c = context(&db);
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let output = record("43");
    let hash = output.output();
    db.begin_run().unwrap();
    adopt(&mut coordinator, &c, &mut db, output);
    assert_eq!(db.source_revision(), c.revision);
    let outcome = db
        .fetch(&SyntaxProvider, query.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    let lrr::db::value::QueryValue::ExpansionInput(descriptor) = outcome.value.as_ref() else {
        panic!("wrong query value")
    };
    assert_eq!(descriptor.output, hash);
    assert!(db.memo(&query).is_some());
    let revision = db.revision();
    adopt(&mut coordinator, &c, &mut db, record("43"));
    assert_eq!(db.revision(), revision);
    db.finish_run().unwrap();
}
#[test]
fn invalid_wire_and_unadmitted_or_changed_records_fail_without_mutating_database() {
    assert!(ExpansionRecord::from_wire(key(), vec![0, 1, 2]).is_err());
    let mut db = Database::default();
    let c = context(&db);
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator
        .issue(
            QueryKey::ExpansionInput(Box::new(key())),
            phase(),
            InputRequest::Expand(Box::new(ExpansionRequest { key: key() })),
        )
        .unwrap();
    assert!(
        coordinator
            .adopt(&c, &token, &mut db, record("43"))
            .is_err()
    );
    adopt(&mut coordinator, &c, &mut db, record("43"));
    let revision = db.revision();
    assert!(
        coordinator
            .adopt(&c, &token, &mut db, record("17"))
            .is_err()
    );
    assert_eq!(db.revision(), revision);
    let mut second = ExpansionCoordinator::new(c.clone());
    let other = second
        .issue(
            QueryKey::ExpansionInput(Box::new(key())),
            phase(),
            token.input().clone(),
        )
        .unwrap();
    second.start(&c, &other).unwrap();
    let changed = record("17");
    second
        .accept(&c, &other, Reply::success(&other, changed.output()))
        .unwrap();
    assert_eq!(
        second.adopt(&c, &other, &mut db, changed).unwrap_err().code,
        "LRR-E-EXPANSION-CONFLICT"
    );
    assert_eq!(db.revision(), revision);
}
#[test]
fn record_adoption_refuses_changed_base_revision_and_pure_provider_publication() {
    let mut db = Database::default();
    let c = context(&db);
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator
        .issue(
            QueryKey::ExpansionInput(Box::new(key())),
            phase(),
            InputRequest::Expand(Box::new(ExpansionRequest { key: key() })),
        )
        .unwrap();
    coordinator.start(&c, &token).unwrap();
    let output = record("43");
    coordinator
        .accept(&c, &token, Reply::success(&token, output.output()))
        .unwrap();
    db.publish_inputs(Default::default()).unwrap();
    assert!(coordinator.adopt(&c, &token, &mut db, output).is_err());
    struct Bad;
    impl Provider for Bad {
        fn execute(&self, db: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
            let c = context(db);
            let mut coordinator = ExpansionCoordinator::new(c.clone());
            let token = coordinator
                .issue(
                    QueryKey::ExpansionInput(Box::new(key())),
                    phase(),
                    InputRequest::Expand(Box::new(ExpansionRequest { key: key() })),
                )
                .unwrap();
            coordinator.start(&c, &token).unwrap();
            let output = record("43");
            coordinator
                .accept(&c, &token, Reply::success(&token, output.output()))
                .unwrap();
            assert_eq!(
                coordinator.adopt(&c, &token, db, output).unwrap_err().code,
                "LRR-E-INTERNAL-SNAPSHOT"
            );
            Provided::pure(fixture_value(requested, 17))
        }
    }
    let q = QueryKey::Constant(fixture_key(
        Digest([16; 32]),
        vec![],
        Digest::default(),
        ProfileIdentity::current(),
    ));
    db.fetch(&Bad, q).into_ready().unwrap().unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, QueryKey::ExpansionInput(Box::new(key()))),
        QueryPoll::NeedInput(_)
    ));
}
#[test]
fn base_publication_cannot_bypass_append_only_expansion_records() {
    let mut db = Database::default();
    let revision = db.revision();
    let record = record("43");
    let input = InputKey::ExpansionRecord(record.key().fingerprint());
    let values = std::collections::BTreeMap::from([(input, InputValue::ExpansionRecord(record))]);
    assert!(db.publish_inputs(values).is_err());
    assert_eq!(db.revision(), revision);
}

#[test]
fn admitted_reply_does_not_release_saved_phase_until_checked_record_is_adopted() {
    use lrr::engine::dispatch::{ResumePoll, SavedPhase};
    let mut db = Database::default();
    let c = context(&db);
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator
        .issue(
            QueryKey::ExpansionInput(Box::new(key())),
            phase(),
            InputRequest::Expand(Box::new(ExpansionRequest { key: key() })),
        )
        .unwrap();
    coordinator.start(&c, &token).unwrap();
    let output = record("43");
    let digest = output.output();
    coordinator
        .accept(&c, &token, Reply::success(&token, digest))
        .unwrap();
    let saved = SavedPhase::new(token.clone(), vec![17, 31]);
    let ResumePoll::Waiting(saved) = saved.resume(&mut coordinator, &c) else {
        panic!("metadata-only reply resumed phase")
    };
    coordinator.adopt(&c, &token, &mut db, output).unwrap();
    let ResumePoll::Ready { state, output } = saved.resume(&mut coordinator, &c) else {
        panic!("installed record did not release phase")
    };
    assert_eq!(state, vec![17, 31]);
    assert_eq!(output, digest);
}

#[test]
fn expansion_payload_metadata_and_retired_input_handles_remain_charged() {
    let mut db = Database::default();
    let c = context(&db);
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let output = record("43");
    let wire_bytes = output.descriptor().wire_bytes;
    let input = InputKey::ExpansionRecord(output.key().fingerprint());
    adopt(&mut coordinator, &c, &mut db, output);
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= wire_bytes);
    let retained = db.input(&input).unwrap();
    db.publish_inputs(Default::default()).unwrap();
    let usage = db.cache_usage().unwrap();
    assert!(usage.pinned_memo_bytes >= wire_bytes);
    drop(retained);
    db.evict_between_runs(lrr::engine::telemetry::CacheBudgets::default())
        .unwrap();
    assert!(db.cache_usage().unwrap().pinned_memo_bytes < usage.pinned_memo_bytes);
}

#[test]
fn derived_record_adoption_obeys_active_run_metadata_budget_without_partial_release() {
    let mut db = Database::default();
    let c = context(&db);
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator
        .issue(
            QueryKey::ExpansionInput(Box::new(key())),
            phase(),
            InputRequest::Expand(Box::new(ExpansionRequest { key: key() })),
        )
        .unwrap();
    coordinator.start(&c, &token).unwrap();
    let output = record("43");
    coordinator
        .accept(&c, &token, Reply::success(&token, output.output()))
        .unwrap();
    db.begin_run_with_budgets(lrr::engine::telemetry::CacheBudgets {
        query_bytes: 0,
        ..Default::default()
    })
    .unwrap();
    let revision = db.revision();
    assert_eq!(
        coordinator
            .adopt(&c, &token, &mut db, output)
            .unwrap_err()
            .code,
        "LRR-E-METADATA-LIMIT"
    );
    assert_eq!(db.revision(), revision);
    assert!(coordinator.take_ready(&c, &token).unwrap().is_none());
    db.finish_run().unwrap();
    coordinator
        .adopt(&c, &token, &mut db, record("43"))
        .unwrap();
    assert!(coordinator.take_ready(&c, &token).unwrap().is_some());
}

include!("../../../xtask/tests/support/constant_key.rs");
include!("../../../xtask/tests/support/constant_value.rs");
