//! M012 active-epoch query eligibility, distinct from immutable historical reads.
use lrr::{
    core::digest::{Digest, StableEncode},
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
    },
    expand::{
        identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        record::ExpansionRecord,
    },
};
use std::collections::BTreeMap;

#[test]
fn current_expansion_queries_depend_on_epoch_and_refuse_stale_record_adoption() {
    let wire = lrr_expand_adapter::token_wire::parse_source(
        "11_u64",
        lrr_expand_adapter::token_wire::SourceIdentity {
            unit: lrr_expand_adapter::token_wire::Key([1; 32]),
            content: lrr_expand_adapter::token_wire::Key([2; 32]),
            path: "fixture.rs".into(),
        },
        lrr_expand_adapter::token_wire::ContextRecord {
            parent: None,
            expansion: None,
            transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
            defining_unit: lrr_expand_adapter::token_wire::Key([1; 32]),
            edition: lrr_expand_adapter::token_wire::Edition::E2024,
        },
        lrr_expand_adapter::token_wire::Key([3; 32]),
    )
    .unwrap()
    .encode()
    .unwrap();
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Procedural {
            artifact: Digest([4; 32]),
            export: "controlled".into(),
            kind: ProceduralKind::Function,
        },
        invocation: Digest([5; 32]),
        input_tokens: Digest([6; 32]),
        consumer_compile_env: Digest([7; 32]),
        parent_context: Digest([8; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([9; 32]),
        procedural_epoch: Some(1),
    };
    let record = ExpansionRecord::from_wire(key.clone(), wire).unwrap();
    let current = QueryKey::CurrentExpansionInput(Box::new(key.clone()));
    let historical = QueryKey::ExpansionInput(Box::new(key.clone()));
    assert_ne!(current.fingerprint(), historical.fingerprint());
    let mut db = Database::default();
    let mut inputs = BTreeMap::new();
    adopt(&mut db, &record).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, current.clone()),
        QueryPoll::Ready(Err(_))
    ));
    assert!(matches!(
        db.fetch(&SyntaxProvider, historical),
        QueryPoll::Ready(Ok(_))
    ));
    inputs.insert(InputKey::ExpansionEpoch, InputValue::ExpansionEpoch(1));
    db.publish_inputs(inputs.clone()).unwrap();
    adopt(&mut db, &record).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, current.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    assert!(
        db.memo(&current)
            .unwrap()
            .dependencies
            .iter()
            .any(|d| matches!(
                d,
                lrr::db::dependency::DependencyStamp::Input {
                    key: InputKey::ExpansionEpoch,
                    ..
                }
            ))
    );
    inputs.insert(InputKey::ExpansionEpoch, InputValue::ExpansionEpoch(2));
    db.publish_inputs(inputs.clone()).unwrap();
    assert!(adopt(&mut db, &record).is_err());
    assert!(matches!(
        db.fetch(&SyntaxProvider, current.clone()),
        QueryPoll::Ready(Err(_))
    ));
    let mut new_key = key.clone();
    new_key.procedural_epoch = Some(2);
    let new_query = QueryKey::CurrentExpansionInput(Box::new(new_key.clone()));
    assert!(
        matches!(db.fetch(&SyntaxProvider,new_query.clone()),QueryPoll::NeedInput(InputRequest::Expand(r)) if r.key==new_key)
    );
    assert!(db.memo(&new_query).is_none());
    inputs.remove(&InputKey::ExpansionEpoch);
    db.publish_inputs(inputs).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, new_query),
        QueryPoll::Ready(Err(_))
    ));
    assert_eq!(record.key(), &key);
    assert!(!InputValue::ExpansionEpoch(1).matches(&InputKey::CompileEnv("epoch".into())));
    assert_ne!(
        InputValue::ExpansionEpoch(1).fingerprint(),
        InputValue::ExpansionEpoch(2).fingerprint()
    );
}

fn adopt(
    db: &mut Database,
    record: &ExpansionRecord,
) -> Result<(), lrr::core::diagnostic::Diagnostic> {
    use lrr::{
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
        source::manifest::BodyKey,
    };
    let context = DemandContext {
        request: Digest([10; 32]),
        session: Digest([11; 32]),
        unit: Digest([1; 32]),
        revision: db.source_revision(),
        epoch: record.key().procedural_epoch.unwrap(),
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
