use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{
        dependency::{InputKey, InputValue},
        key::{ConstantKey, QueryKey},
        memo::Database,
        poll::{ExpansionRequest, InputRequest, QueryPoll},
        provider::{Provided, Provider},
        value::QueryValue,
    },
    expand::identity::{ExpansionKey, GrammarSlot, MacroIdentity},
};
use std::{cell::Cell, collections::BTreeMap};
fn key(n: u8) -> QueryKey {
    QueryKey::Constant(ConstantKey {
        definition: Digest([n; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    })
}
fn request() -> InputRequest {
    InputRequest::Expand(Box::new(ExpansionRequest {
        key: ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: Digest([3; 32]),
            },
            invocation: Digest([4; 32]),
            input_tokens: Digest([5; 32]),
            consumer_compile_env: Digest([6; 32]),
            parent_context: Digest([7; 32]),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: Digest([8; 32]),
            procedural_epoch: None,
        },
    }))
}
struct Missing {
    runs: Cell<usize>,
    ignore_child: bool,
}
impl Provider for Missing {
    fn execute(&self, db: &mut Database, k: &QueryKey) -> QueryPoll<Provided> {
        self.runs.set(self.runs.get() + 1);
        if *k == key(2) {
            let child = db.fetch(self, key(1));
            if self.ignore_child {
                return Provided::pure(QueryValue::Constant(-1));
            }
            return child.map(|o| Provided {
                value: o.value.as_ref().clone(),
                cacheable: o.cacheable,
            });
        }
        match db
            .input(&InputKey::CompileEnv("expansion".into()))
            .as_deref()
        {
            Some(InputValue::CompileEnv(v)) => {
                Provided::pure(QueryValue::Constant(v.parse().unwrap()))
            }
            _ => {
                let _ = db.input(&InputKey::CompileEnv("incomplete-only".into()));
                QueryPoll::NeedInput(request())
            }
        }
    }
}
fn inputs(v: Option<&str>) -> BTreeMap<InputKey, InputValue> {
    v.into_iter()
        .map(|v| {
            (
                InputKey::CompileEnv("expansion".into()),
                InputValue::CompileEnv(v.into()),
            )
        })
        .collect()
}
fn number(p: QueryPoll<lrr::db::memo::Outcome>) -> i128 {
    let QueryValue::Constant(v) = p
        .into_ready()
        .expect("completed fixture query")
        .unwrap()
        .value
        .as_ref()
        .clone()
    else {
        panic!("wrong result")
    };
    v
}
#[test]
fn missing_input_unwinds_nested_collectors_without_memoizing_absence() {
    let provider = Missing {
        runs: Cell::new(0),
        ignore_child: false,
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    for _ in 0..2 {
        assert!(matches!(db.fetch(&provider,key(2)),QueryPoll::NeedInput(r) if r==request()));
        assert!(db.memo(&key(1)).is_none());
        assert!(db.memo(&key(2)).is_none());
    }
    assert_eq!(provider.runs.get(), 4);
    assert_eq!(db.trace.counts["Constant:ProviderRun"], 4);
    db.publish_inputs(inputs(Some("43"))).unwrap();
    assert_eq!(number(db.fetch(&provider, key(2))), 43);
    let runs = provider.runs.get();
    assert_eq!(number(db.fetch(&provider, key(2))), 43);
    assert_eq!(provider.runs.get(), runs);
    assert!(db.memo(&key(1)).unwrap().dependencies.iter().all(|s|!matches!(s,lrr::db::dependency::DependencyStamp::Input{key:InputKey::CompileEnv(n),..} if n=="incomplete-only")));
    assert!(
        db.trace
            .events
            .iter()
            .any(|e| e.action == lrr::db::trace::Action::Suspended)
    );
}
#[test]
fn suspended_child_cannot_be_ignored_and_turned_into_completed_negative_value() {
    let provider = Missing {
        runs: Cell::new(0),
        ignore_child: true,
    };
    let mut db = Database::default();
    assert!(matches!(
        db.fetch(&provider, key(2)),
        QueryPoll::NeedInput(_)
    ));
    assert!(db.memo(&key(2)).is_none());
    db.publish_inputs(inputs(Some("17"))).unwrap();
    assert_eq!(number(db.fetch(&provider, key(1))), 17);
}
#[test]
fn memo_revalidation_suspends_and_rejects_old_value_until_new_input_is_published() {
    let provider = Missing {
        runs: Cell::new(0),
        ignore_child: false,
    };
    let mut db = Database::default();
    db.publish_inputs(inputs(Some("7"))).unwrap();
    assert_eq!(number(db.fetch(&provider, key(2))), 7);
    db.publish_inputs(inputs(None)).unwrap();
    assert!(matches!(
        db.fetch(&provider, key(2)),
        QueryPoll::NeedInput(_)
    ));
    assert!(db.memo(&key(1)).is_none());
    assert!(db.memo(&key(2)).is_none());
    db.publish_inputs(inputs(Some("31"))).unwrap();
    assert_eq!(number(db.fetch(&provider, key(2))), 31);
}

#[test]
fn invalid_input_request_is_a_completed_protocol_error_and_leaves_no_memo() {
    struct Invalid;
    impl Provider for Invalid {
        fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
            if db
                .input(&InputKey::CompileEnv("expansion".into()))
                .is_some()
            {
                return Provided::pure(QueryValue::Constant(41));
            }
            let InputRequest::Expand(mut r) = request() else {
                panic!("wrong fixture request")
            };
            r.key.call_edition = 2016;
            QueryPoll::NeedInput(InputRequest::Expand(r))
        }
    }
    let mut db = Database::default();
    db.publish_inputs(inputs(Some("41"))).unwrap();
    assert_eq!(number(db.fetch(&Invalid, key(1))), 41);
    assert!(db.memo(&key(1)).is_some());
    db.publish_inputs(inputs(None)).unwrap();
    let error = db
        .fetch(&Invalid, key(1))
        .into_ready()
        .expect("invalid request is terminal")
        .unwrap_err();
    assert!(error.message.contains("invalid expansion edition/epoch"));
    assert!(db.memo(&key(1)).is_none());
    db.publish_inputs(inputs(Some("41"))).unwrap();
    let provider = Missing {
        runs: Cell::new(0),
        ignore_child: false,
    };
    assert_eq!(number(db.fetch(&provider, key(1))), 41);
}

#[test]
fn input_request_encoding_binds_compile_context_and_capture_expectation() {
    use lrr::{
        core::digest::StableEncode, db::poll::GeneratedSourceRequest, source::snapshot::SourcePath,
    };
    let original = request();
    assert_eq!(original.fingerprint(), original.clone().fingerprint());
    let InputRequest::Expand(mut changed) = original.clone() else {
        panic!("wrong request")
    };
    changed.key.consumer_compile_env = Digest([9; 32]);
    assert_ne!(
        original.fingerprint(),
        InputRequest::Expand(changed).fingerprint()
    );
    let capture = GeneratedSourceRequest {
        consumer_unit: Digest([1; 32]),
        compile_environment: Digest([2; 32]),
        path: SourcePath::new("generated.rs").unwrap(),
        expected_content: None,
    };
    let request = InputRequest::CaptureGeneratedSource(capture.clone());
    request.validate().unwrap();
    assert_ne!(
        request.fingerprint(),
        InputRequest::CaptureGeneratedSource(GeneratedSourceRequest {
            expected_content: Some(Digest([3; 32])),
            ..capture
        })
        .fingerprint()
    );
    assert_ne!(request.fingerprint(), original.fingerprint());
}
