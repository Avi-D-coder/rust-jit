use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{
        key::{ConstantKey, QueryKey},
        poll::{ExpansionRequest, InputRequest},
    },
    engine::dispatch::{ResumePoll, SavedPhase},
    expand::identity::{ExpansionKey, GrammarSlot, MacroIdentity},
    service::expansion::{
        DemandContext, ExpansionCoordinator, PhaseIdentity, Reply, StartDecision,
    },
    source::manifest::BodyKey,
};
fn context() -> DemandContext {
    DemandContext {
        request: Digest([1; 32]),
        session: Digest([2; 32]),
        unit: Digest([3; 32]),
        revision: lrr::db::memo::Database::default().source_revision(),
        epoch: 11,
        source_generation: Digest([4; 32]),
        native_generation: None,
        compile_environment: Digest([5; 32]),
    }
}
fn query() -> QueryKey {
    QueryKey::Constant(ConstantKey {
        definition: Digest([6; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    })
}
fn phase(index: u32) -> PhaseIdentity {
    PhaseIdentity {
        frame: Digest([7; 32]),
        body: BodyKey {
            definition: Digest([8; 32]),
            exact_body_bytes: Digest([9; 32]),
        },
        index,
    }
}
fn input() -> InputRequest {
    InputRequest::Expand(Box::new(ExpansionRequest {
        key: ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: Digest([10; 32]),
            },
            invocation: Digest([11; 32]),
            input_tokens: Digest([12; 32]),
            consumer_compile_env: context().compile_environment,
            parent_context: Digest([13; 32]),
            definition_edition: 2024,
            call_edition: 2024,
            grammar: GrammarSlot::Expression,
            engine_version: Digest([14; 32]),
            procedural_epoch: None,
        },
    }))
}
fn output_record() -> lrr::expand::record::ExpansionRecord {
    let InputRequest::Expand(input) = input() else {
        panic!("wrong input")
    };
    let tokens = lrr::syntax::token_wire::TokenForest::new(vec![], vec![], vec![]).unwrap();
    lrr::expand::record::ExpansionRecord::from_tokens(input.key, &tokens).unwrap()
}
fn adopt(
    coordinator: &mut ExpansionCoordinator,
    c: &DemandContext,
    token: &lrr::service::expansion::DemandToken,
) {
    coordinator
        .adopt(
            c,
            token,
            &mut lrr::db::memo::Database::default(),
            output_record(),
        )
        .unwrap();
}

#[test]
fn demand_tokens_share_one_operation_and_distinguish_query_phase_waiters() {
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let first = coordinator.issue(query(), phase(3), input()).unwrap();
    let duplicate = coordinator.issue(query(), phase(3), input()).unwrap();
    assert_eq!(first, duplicate);
    let other = coordinator.issue(query(), phase(4), input()).unwrap();
    assert_ne!(first, other);
    assert!(matches!(
        coordinator.start(&c, &first).unwrap(),
        StartDecision::Start
    ));
    assert!(matches!(
        coordinator.start(&c, &other).unwrap(),
        StartDecision::InFlight
    ));
    let reply = Reply::success(&first, output_record().output());
    coordinator.accept(&c, &first, reply.clone()).unwrap();
    assert!(coordinator.accept(&c, &first, reply).is_err());
    assert!(
        matches!(coordinator.start(&c,&other).unwrap(),StartDecision::Reuse(d) if d==output_record().output())
    );
    assert_eq!(
        coordinator.ready(&c, &other).unwrap().unwrap().unwrap(),
        output_record().output()
    );
}
#[test]
fn mismatched_reply_context_and_input_refuse_without_clearing_running_operation() {
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator.issue(query(), phase(3), input()).unwrap();
    coordinator.start(&c, &token).unwrap();
    let good = Reply::success(&token, output_record().output());
    let mut wrong = good.clone();
    wrong.input = Digest([16; 32]);
    assert!(coordinator.accept(&c, &token, wrong).is_err());
    for field in 0..8 {
        let mut wrong = good.clone();
        match field {
            0 => wrong.context.revision += 1,
            1 => wrong.context.epoch += 1,
            2 => wrong.context.unit = Digest([16; 32]),
            3 => wrong.context.session = Digest([16; 32]),
            4 => wrong.context.request = Digest([16; 32]),
            5 => wrong.context.source_generation = Digest([16; 32]),
            6 => wrong.context.compile_environment = Digest([16; 32]),
            _ => wrong.context.native_generation = Some(Digest([16; 32])),
        };
        assert!(coordinator.accept(&c, &token, wrong).is_err());
    }
    assert!(coordinator.ready(&c, &token).unwrap().is_none());
    coordinator.accept(&c, &token, good).unwrap();
}
#[test]
fn cancelled_and_superseded_demands_never_resume_saved_phase() {
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator.issue(query(), phase(3), input()).unwrap();
    coordinator.start(&c, &token).unwrap();
    let reply = Reply::success(&token, output_record().output());
    let saved = SavedPhase::new(token.clone(), vec![17, 31]);
    coordinator.cancel();
    assert!(coordinator.accept(&c, &token, reply).is_err());
    let ResumePoll::Rejected { saved, error } = saved.resume(&mut coordinator, &c) else {
        panic!("cancelled continuation resumed")
    };
    assert!(error.message.contains("closed"));
    assert_eq!(saved.state(), &vec![17, 31]);
    assert!(coordinator.issue(query(), phase(4), input()).is_err());
}
#[test]
fn saved_phase_retains_owned_operands_until_input_is_ready_and_rejects_wrong_revision() {
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator.issue(query(), phase(3), input()).unwrap();
    let saved = SavedPhase::new(token.clone(), String::from("already evaluated"));
    let ResumePoll::Waiting(saved) = saved.resume(&mut coordinator, &c) else {
        panic!("missing input resumed")
    };
    coordinator.start(&c, &token).unwrap();
    let mut stale = c.clone();
    stale.revision += 1;
    let ResumePoll::Rejected { saved, .. } = saved.resume(&mut coordinator, &stale) else {
        panic!("wrong revision resumed")
    };
    coordinator
        .accept(&c, &token, Reply::success(&token, output_record().output()))
        .unwrap();
    adopt(&mut coordinator, &c, &token);
    let ResumePoll::Ready { state, output } = saved.resume(&mut coordinator, &c) else {
        panic!("ready phase missing")
    };
    assert_eq!(state, "already evaluated");
    assert_eq!(output, output_record().output());
}
#[test]
fn failed_operation_is_shared_in_epoch_and_not_retried() {
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator.issue(query(), phase(3), input()).unwrap();
    coordinator.start(&c, &token).unwrap();
    let mut reply = Reply::success(&token, output_record().output());
    reply.output = Err(lrr::core::diagnostic::Diagnostic::new(
        "LRR-E-MACRO-FAILED",
        "fixture failure".into(),
    ));
    coordinator.accept(&c, &token, reply).unwrap();
    let other = coordinator.issue(query(), phase(4), input()).unwrap();
    assert!(coordinator.start(&c, &other).is_err());
    assert_eq!(
        coordinator
            .ready(&c, &other)
            .unwrap()
            .unwrap()
            .unwrap_err()
            .code,
        "LRR-E-MACRO-FAILED"
    );
    let saved = SavedPhase::new(other, vec![17]);
    assert!(matches!(
        saved.resume(&mut coordinator, &c),
        ResumePoll::Rejected { .. }
    ));
}

#[test]
fn a_consumed_phase_token_cannot_resume_another_copy_of_the_continuation() {
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let token = coordinator.issue(query(), phase(3), input()).unwrap();
    coordinator.start(&c, &token).unwrap();
    coordinator
        .accept(&c, &token, Reply::success(&token, output_record().output()))
        .unwrap();
    adopt(&mut coordinator, &c, &token);
    let first = SavedPhase::new(token.clone(), vec![17]);
    let duplicate = SavedPhase::new(token, vec![31]);
    assert!(matches!(
        first.resume(&mut coordinator, &c),
        ResumePoll::Ready { .. }
    ));
    let ResumePoll::Rejected { saved, error } = duplicate.resume(&mut coordinator, &c) else {
        panic!("duplicate phase resumed")
    };
    assert_eq!(saved.state(), &vec![31]);
    assert!(error.message.contains("consumed"));
}

#[test]
fn issue_rejects_wrong_environment_epoch_unit_and_excess_metadata_atomically() {
    use lrr::{
        db::poll::GeneratedSourceRequest, expand::identity::ProceduralKind,
        source::snapshot::SourcePath,
    };
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let InputRequest::Expand(mut wrong) = input() else {
        panic!("wrong input")
    };
    wrong.key.consumer_compile_env = Digest([16; 32]);
    assert!(
        coordinator
            .issue(query(), phase(3), InputRequest::Expand(wrong))
            .is_err()
    );
    let InputRequest::Expand(mut wrong) = input() else {
        panic!("wrong input")
    };
    wrong.key.macro_identity = MacroIdentity::Procedural {
        artifact: Digest([16; 32]),
        export: "echo".into(),
        kind: ProceduralKind::Function,
    };
    wrong.key.procedural_epoch = Some(c.epoch + 1);
    assert!(
        coordinator
            .issue(query(), phase(3), InputRequest::Expand(wrong))
            .is_err()
    );
    let wrong = InputRequest::CaptureGeneratedSource(GeneratedSourceRequest {
        consumer_unit: Digest([16; 32]),
        compile_environment: c.compile_environment,
        path: SourcePath::new("generated.rs").unwrap(),
        expected_content: None,
    });
    assert!(coordinator.issue(query(), phase(3), wrong).is_err());
    let QueryKey::Constant(mut excessive) = query() else {
        panic!("wrong query")
    };
    excessive.arguments = Vec::with_capacity(1_048_577);
    assert!(
        coordinator
            .issue(QueryKey::Constant(excessive), phase(3), input())
            .is_err()
    );
    let good = coordinator.issue(query(), phase(3), input()).unwrap();
    assert_eq!(good, coordinator.issue(query(), phase(3), input()).unwrap());
    coordinator.start(&c, &good).unwrap();
    let mut too_large = Reply::success(&good, output_record().output());
    too_large.output = Err(lrr::core::diagnostic::Diagnostic::new(
        "LRR-E-MACRO-FAILED",
        "x".repeat(65_537),
    ));
    assert!(coordinator.accept(&c, &good, too_large).is_err());
    assert!(coordinator.ready(&c, &good).unwrap().is_none());
    coordinator
        .accept(&c, &good, Reply::success(&good, output_record().output()))
        .unwrap();
}

#[test]
fn new_request_epoch_cannot_adopt_an_old_reply_even_for_identical_input() {
    let c = context();
    let mut first = ExpansionCoordinator::new(c.clone());
    let old = first.issue(query(), phase(3), input()).unwrap();
    first.start(&c, &old).unwrap();
    let reply = Reply::success(&old, output_record().output());
    let mut next = c.clone();
    next.epoch += 1;
    next.request = Digest([16; 32]);
    let mut second = ExpansionCoordinator::new(next.clone());
    let token = second.issue(query(), phase(3), input()).unwrap();
    second.start(&next, &token).unwrap();
    assert!(second.accept(&next, &token, reply).is_err());
    assert!(second.ready(&next, &token).unwrap().is_none());
    second
        .accept(&next, &token, Reply::success(&token, Digest([17; 32])))
        .unwrap();
    assert_eq!(
        second.ready(&next, &token).unwrap().unwrap().unwrap(),
        Digest([17; 32])
    );
}

#[test]
fn unstarted_or_wrong_waiter_reply_cannot_publish_shared_operation() {
    let c = context();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let first = coordinator.issue(query(), phase(3), input()).unwrap();
    let other = coordinator.issue(query(), phase(4), input()).unwrap();
    let reply = Reply::success(&first, output_record().output());
    assert!(coordinator.accept(&c, &first, reply.clone()).is_err());
    coordinator.start(&c, &first).unwrap();
    assert!(coordinator.accept(&c, &other, reply.clone()).is_err());
    assert!(
        coordinator
            .accept(&c, &other, Reply::success(&other, output_record().output()))
            .is_err()
    );
    assert!(coordinator.ready(&c, &first).unwrap().is_none());
    coordinator.accept(&c, &first, reply).unwrap();
    assert!(coordinator.ready(&c, &other).unwrap().is_some());
}

#[test]
fn include_capture_demand_binds_authority_context_reply_and_defers_resumption() {
    include_capture_fixture(false);
}

#[test]
fn include_capture_adoption_publishes_immutable_query_input_and_invalidates_on_removal() {
    include_capture_fixture(true);
}

fn include_capture_fixture(adopt: bool) {
    use lrr::syntax::token_wire::Key;
    use lrr::{
        expand::compile_input_identity::descriptor,
        service::source_capture::SourceCapture,
        source::{
            include_paths::{IncludeAuthorities, IncludeLocation, IncludeRoot, IncludeRootKind},
            snapshot::SourcePath,
        },
    };
    use std::{
        collections::BTreeMap,
        time::{SystemTime, UNIX_EPOCH},
    };
    static DIRECTORY_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = DIRECTORY_SEQUENCE
        .try_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .unwrap();
    let temp = std::env::temp_dir().join(format!(
        "lrr-include-demand-{sequence}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&temp).unwrap();
    let temp = std::fs::canonicalize(temp).unwrap();
    std::fs::write(temp.join("one.rs"), b"31").unwrap();
    std::fs::write(temp.join("two.rs"), b"73").unwrap();
    let c = context();
    let root = Digest([41; 32]);
    let authorities = IncludeAuthorities::new(
        c.unit,
        c.source_generation,
        vec![IncludeRoot {
            identity: root,
            path: temp.clone(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let from = IncludeLocation {
        unit: c.unit,
        root,
        path: SourcePath::new("main.rs").unwrap(),
        origin: Key([42; 32]),
    };
    let definition = descriptor("include").unwrap().unwrap();
    let one = authorities
        .request(&definition, &from, "one.rs", Key([43; 32]))
        .unwrap();
    let two = authorities
        .request(&definition, &from, "two.rs", Key([44; 32]))
        .unwrap();
    let input = InputRequest::CaptureInclude {
        request: Box::new(one.clone()),
        compile_environment: c.compile_environment,
    };
    let mut wrong_generation = c.clone();
    wrong_generation.source_generation = Digest([45; 32]);
    assert!(
        ExpansionCoordinator::new(wrong_generation)
            .issue(query(), phase(3), input.clone())
            .is_err()
    );
    let mut wrong_env = input.clone();
    if let InputRequest::CaptureInclude {
        compile_environment,
        ..
    } = &mut wrong_env
    {
        *compile_environment = Digest([46; 32]);
    }
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    assert!(coordinator.issue(query(), phase(3), wrong_env).is_err());
    let token = coordinator.issue(query(), phase(3), input.clone()).unwrap();
    let other = coordinator.issue(query(), phase(4), input).unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let first = capture.capture(&one).unwrap();
    let second = capture.capture(&two).unwrap();
    assert!(coordinator.accept_include(&c, &token, &first).is_err());
    assert_eq!(coordinator.start(&c, &token).unwrap(), StartDecision::Start);
    assert_eq!(
        coordinator.start(&c, &other).unwrap(),
        StartDecision::InFlight
    );
    assert!(coordinator.accept_include(&c, &token, &second).is_err());
    assert!(coordinator.ready(&c, &token).unwrap().is_none());
    coordinator.accept_include(&c, &token, &first).unwrap();
    assert_eq!(
        coordinator.ready(&c, &other).unwrap().unwrap().unwrap(),
        first.identity()
    );
    assert!(coordinator.accept_include(&c, &token, &first).is_err());
    assert!(coordinator.take_ready(&c, &token).unwrap().is_none());
    let mut stale = c.clone();
    stale.revision += 1;
    assert!(coordinator.ready(&stale, &token).is_err());
    if adopt {
        use lrr::{
            core::digest::StableEncode,
            db::{
                dependency::{InputKey, InputValue},
                memo::Database,
                poll::QueryPoll,
                provider::{Provided, Provider},
                value::QueryValue,
            },
        };
        struct ReadInclude {
            input: InputRequest,
        }
        impl Provider for ReadInclude {
            fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
                match db
                    .input(&InputKey::IncludeRecord(self.input.fingerprint()))
                    .as_deref()
                {
                    Some(InputValue::IncludeRecord { captured, .. }) => {
                        let text = std::str::from_utf8(captured.blob().bytes()).unwrap();
                        Provided::pure(QueryValue::Constant(text.parse().unwrap()))
                    }
                    None => QueryPoll::NeedInput(self.input.clone()),
                    _ => Provided::error("LRR-E-INTERNAL-SNAPSHOT", "wrong include record"),
                }
            }
        }
        let provider = ReadInclude {
            input: token.input().clone(),
        };
        let mut db = Database::default();
        assert!(matches!(
            db.fetch(&provider, query()),
            QueryPoll::NeedInput(_)
        ));
        assert!(db.memo(&query()).is_none());
        assert!(
            coordinator
                .adopt_include(&c, &token, &mut db, second.clone())
                .is_err()
        );
        assert!(coordinator.take_ready(&c, &token).unwrap().is_none());
        let before = db.revision();
        db.begin_run_with_budgets(lrr::engine::telemetry::CacheBudgets {
            source_input_bytes: 1,
            ..Default::default()
        })
        .unwrap();
        assert!(
            coordinator
                .adopt_include(&c, &token, &mut db, first.clone())
                .is_err()
        );
        assert_eq!(db.revision(), before);
        assert!(
            db.input(&InputKey::IncludeRecord(token.input().fingerprint()))
                .is_none()
        );
        assert!(coordinator.take_ready(&c, &token).unwrap().is_none());
        db.finish_run().unwrap();
        coordinator
            .adopt_include(&c, &token, &mut db, first.clone())
            .unwrap();
        assert_eq!(db.source_revision(), c.revision);
        assert!(db.revision() > before);
        let published = db.revision();
        coordinator
            .adopt_include(&c, &other, &mut db, first.clone())
            .unwrap();
        assert_eq!(db.revision(), published);
        assert_eq!(
            coordinator
                .take_ready(&c, &token)
                .unwrap()
                .unwrap()
                .unwrap(),
            first.identity()
        );
        assert!(coordinator.take_ready(&c, &token).is_err());
        assert_eq!(db.cache_usage().unwrap().source_input_bytes, 2);
        std::fs::write(temp.join("one.rs"), b"999").unwrap();
        let result = db.fetch(&provider, query()).into_ready().unwrap().unwrap();
        assert!(matches!(result.value.as_ref(), QueryValue::Constant(31)));
        assert!(db.memo(&query()).unwrap().dependencies.iter().any(|dep| matches!(dep, lrr::db::dependency::DependencyStamp::Input {key:InputKey::IncludeRecord(key),..} if *key==token.input().fingerprint())));
        let lease = db
            .input(&InputKey::IncludeRecord(token.input().fingerprint()))
            .unwrap();
        db.publish_inputs(BTreeMap::new()).unwrap();
        assert_eq!(db.cache_usage().unwrap().source_input_bytes, 2);
        assert!(matches!(
            db.fetch(&provider, query()),
            QueryPoll::NeedInput(_)
        ));
        assert!(
            coordinator
                .adopt_include(&c, &other, &mut db, first.clone())
                .is_err()
        );
        drop(lease);
        assert_eq!(db.cache_usage().unwrap().source_input_bytes, 0);
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn adopted_item_include_uses_pinned_parser_caller_scope_cfg_and_origin_errors() {
    adopted_item_include_edition_fixture(2024);
}
#[test]
fn dependency_item_include_preserves_older_edition_keywords_cfg_and_origin_errors() {
    for edition in [2015, 2018, 2021] {
        adopted_item_include_edition_fixture(edition);
    }
}
fn adopted_item_include_edition_fixture(edition: u16) {
    use lrr::core::digest::StableEncode;
    use lrr::{
        db::{memo::Database, poll::QueryPoll, provider::SyntaxProvider},
        expand::compile_input_identity::descriptor,
        service::source_capture::SourceCapture,
        source::{
            include_paths::{IncludeAuthorities, IncludeLocation, IncludeRoot, IncludeRootKind},
            included::IncludeItemsContext,
            snapshot::{CfgSet, SourcePath},
        },
        syntax::token_wire::Key,
    };
    use std::{
        collections::BTreeMap,
        time::{SystemTime, UNIX_EPOCH},
    };
    let temp = std::env::temp_dir().join(format!(
        "lrr-include-items-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&temp).unwrap();
    let temp = std::fs::canonicalize(temp).unwrap();
    std::fs::write(temp.join("items.rs"),if edition==2024 { "fn live()->i64{31} fn cold(){absent();} #[cfg(disabled)] fn hidden(){} include!(\"nested.rs\");" } else { "fn gen()->i64{31} fn cold(){absent();} #[cfg(disabled)] fn hidden(){} include!(\"nested.rs\");" }).unwrap();
    std::fs::write(temp.join("bad.rs"), b"fn broken( {").unwrap();
    let c = context();
    let root = Digest([51; 32]);
    let authorities = IncludeAuthorities::new(
        c.unit,
        c.source_generation,
        vec![IncludeRoot {
            identity: root,
            path: temp.clone(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap();
    let from = IncludeLocation {
        unit: c.unit,
        root,
        path: SourcePath::new("main.rs").unwrap(),
        origin: Key([52; 32]),
    };
    let definition = descriptor("include").unwrap().unwrap();
    let request = authorities
        .request(&definition, &from, "items.rs", Key([53; 32]))
        .unwrap();
    let bad = authorities
        .request(&definition, &from, "bad.rs", Key([54; 32]))
        .unwrap();
    let capture = SourceCapture::open(authorities, BTreeMap::new()).unwrap();
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let mut db = Database::default();
    let mut lexical = IncludeItemsContext {
        unit: c.unit,
        crate_key: Digest([55; 32]),
        module_path: vec!["caller".into()],
        cfg: CfgSet::default(),
        profile: ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    };
    let input = InputRequest::CaptureInclude {
        request: Box::new(request.clone()),
        compile_environment: c.compile_environment,
    };
    for (invalid, code) in [
        (
            ProfileIdentity {
                edition: 2000,
                ..ProfileIdentity::current()
            },
            "LRR-E-TOOLCHAIN-CAPABILITY",
        ),
        (
            ProfileIdentity {
                edition,
                semantics: "lrr-2024-v1".into(),
            },
            "LRR-E-SEMANTICS-PROFILE",
        ),
    ] {
        lexical.profile = invalid;
        let error = SyntaxProvider::included_items(&mut db, &input, &lexical)
            .into_ready()
            .unwrap()
            .err()
            .unwrap();
        assert_eq!(error.code, code);
    }
    lexical.profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    assert!(matches!(
        SyntaxProvider::included_items(&mut db, &input, &lexical),
        QueryPoll::NeedInput(_)
    ));
    for (index, request) in [request, bad].into_iter().enumerate() {
        let input = InputRequest::CaptureInclude {
            request: Box::new(request.clone()),
            compile_environment: c.compile_environment,
        };
        let token = coordinator
            .issue(query(), phase(30 + index as u32), input.clone())
            .unwrap();
        coordinator.start(&c, &token).unwrap();
        let captured = capture.capture(&request).unwrap();
        coordinator.accept_include(&c, &token, &captured).unwrap();
        coordinator
            .adopt_include(&c, &token, &mut db, captured)
            .unwrap();
        if index == 0 {
            let items = SyntaxProvider::included_items(&mut db, &input, &lexical)
                .into_ready()
                .unwrap()
                .unwrap();
            assert!(items.parsed.errors.is_empty());
            assert_eq!(items.parsed.profile.edition, edition);
            assert_eq!(items.outline.profile.edition, edition);
            assert_eq!(items.outline.root_scope.crate_key, lexical.crate_key);
            assert_eq!(items.outline.root_scope.module_path, lexical.module_path);
            assert_eq!(items.outline.cfg, lexical.cfg.fingerprint());
            assert_eq!(items.outline.item_invocations.len(), 1);
            assert_eq!(
                items
                    .outline
                    .items
                    .iter()
                    .map(|item| item.def_key.declared_name.as_str())
                    .collect::<Vec<_>>(),
                vec![if edition == 2024 { "live" } else { "gen" }, "cold"]
            );
            assert!(
                matches!(items.input.as_ref(),lrr::db::dependency::InputValue::IncludeRecord{captured,..} if captured.location().path.as_str()=="items.rs")
            );
            lexical.unit = Digest([56; 32]);
            assert!(
                SyntaxProvider::included_items(&mut db, &input, &lexical)
                    .into_ready()
                    .unwrap()
                    .is_err()
            );
            lexical.unit = c.unit;
        } else {
            let error = SyntaxProvider::included_items(&mut db, &input, &lexical)
                .into_ready()
                .unwrap()
                .err()
                .unwrap();
            assert_eq!(error.code, "LRR-E-INCLUDE-PARSE");
            assert!(error.message.contains("bad.rs"));
            assert!(error.message.contains(&Digest([52; 32]).hex()));
            assert!(error.message.contains(&Digest([54; 32]).hex()));
        }
    }
    std::fs::remove_dir_all(temp).unwrap();
}
