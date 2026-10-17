//! Source-authority/atomicity controls over the complete unchanged rustc input.
//! Publishing a body locator is not executing a guest call or fixture.
use super::*;
use crate::{
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        callables::{CallSelection, EnteredLocalCall, prepare_entered_local_function},
        compile_input_catalog::CompileInputCatalog,
        source_locals::PreparedSourceLocalPlan,
    },
    ir::pc::{PcKey, Phase},
    source::manifest::Namespace,
    syntax::body_index::{ExpressionKind, NodeRole},
};
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("ordinary publication source: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("ordinary publication demand: {input:?}"),
    }
}
fn world(text: &str, edition: u16) -> (Database, Arc<PreparedSourceLocalPlan>, PcKey) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(text, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"ordinary local publication control"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"ordinary local publication compiler"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(&mut db).unwrap();
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("actual discovery incomplete: {diagnostic:?} {blocked:?}"),
            SourceDiscoveryStep::NeedSourceDeriveSelection { .. } => {
                panic!("actual NeedSourceDeriveSelection")
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { .. } => {
                panic!("actual NeedSourceDeriveExpansion")
            }
            SourceDiscoveryStep::NeedSourceDeriveAppend { .. } => {
                panic!("actual NeedSourceDeriveAppend")
            }
            SourceDiscoveryStep::NeedDeriveAppend { .. } => panic!("actual NeedDeriveAppend"),
            SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("actual NeedDeriveDeclaration")
            }
            SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("actual NeedAttributeSelection")
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. } => {
                panic!("actual NeedProceduralSelection")
            }
        }
    }
    let CallSelection::Source(main) = ready(discovery.entry_function(&mut db, "main")) else {
        panic!("actual selected main is not a source function")
    };
    let parent =
        ready(db.prepare_source_local_plan(&discovery, &[&source], &main.body().unwrap().key()));
    let index = parent.context().source().index();
    let call = index
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .unwrap();
    let site = index.pc(call.id, Phase::Entry).unwrap();
    (db, parent, site)
}
fn proof(db: &mut Database, site: &PcKey) -> Box<OrdinaryLocalAdmission> {
    match ready(prepare_entered_local_function(db, site)) {
        EnteredLocalCall::Local(proof) => proof,
        other => panic!("actual selected local source: {other:?}"),
    }
}
fn stamps(db: &Database) -> BTreeMap<InputKey, Digest> {
    db.inputs
        .iter()
        .map(|(key, row)| (key.clone(), row.fingerprint))
        .collect()
}
fn no_executable_work(db: &Database, start: usize) {
    assert!(
        !db.trace.events[start..].iter().any(|event| matches!(
            &event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::Constant(_)
        )),
        "publication prepared executable code"
    );
}
#[test]
fn two_cached_ordinary_proofs_publish_independent_cold_bodies_without_replanning() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, parent, _) = world(SUFFIX, edition);
        let index = parent.context().source().index();
        let calls = index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
            .take(3)
            .map(|node| index.pc(node.id, Phase::Entry).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 3);
        db.begin_run().unwrap();
        // These are actual first/third calls of the unchanged official program;
        // the same common Q39 service must select two distinct canonical items.
        let first = proof(&mut db, &calls[0]);
        let second = proof(&mut db, &calls[2]);
        assert_eq!(first.item().def_key.declared_name, "id_i8");
        assert_eq!(second.item().def_key.declared_name, "id_i16");
        let first_id = first.item().def_key.fingerprint();
        let second_id = second.item().def_key.fingerprint();
        assert_ne!(first_id, second_id);
        for selected in [&first, &second] {
            let body = selected.item().body.as_ref().unwrap().key();
            assert!(
                !db.inputs
                    .contains_key(&InputKey::DefinitionBody(body.definition))
            );
            assert!(db.memo(&QueryKey::BodyStructure(body)).is_none());
        }
        let source_revision = db.source_revision();
        let revision = db.revision();
        let inputs = db.inputs.len();
        let trace = db.trace.events.len();
        db.publish_ordinary_local_function(&first).unwrap();
        assert_eq!(db.revision(), revision + 1);
        assert_eq!(db.inputs.len(), inputs + 1);
        // Same cached proof remains idempotent after its own real publication.
        let after_first = stamps(&db);
        db.publish_ordinary_local_function(&first).unwrap();
        assert_eq!(stamps(&db), after_first);
        assert_eq!(db.revision(), revision + 1);
        // First body is unrelated to the second Q39 input closure. Do not
        // reconstruct its proof/guard just because global revision advanced.
        db.publish_ordinary_local_function(&second).unwrap();
        assert_eq!(db.revision(), revision + 2);
        assert_eq!(db.inputs.len(), inputs + 2);
        let after_second = stamps(&db);
        db.publish_ordinary_local_function(&second).unwrap();
        assert_eq!(stamps(&db), after_second);
        assert_eq!(db.revision(), revision + 2);
        assert_eq!(db.source_revision(), source_revision);
        assert_eq!(
            db.trace.events.len(),
            trace,
            "cached publication performed Q39/provider/guard planning: edition={edition}"
        );
        for selected in [&first, &second] {
            selected.revalidate(&db).unwrap();
            let body = selected.item().body.as_ref().unwrap().key();
            assert!(db.memo(&QueryKey::BodyStructure(body)).is_none());
        }
        db.finish_run().unwrap();
    }
}
#[test]
fn ordinary_local_body_publication_uses_same_source_rows_across_editions_and_origins() {
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..3 {
            let text = match mode {
                0 => SUFFIX.to_owned(),
                1 => format!("macro_rules! emit{{()=>{{{SUFFIX}}}}}emit!();"),
                _ => format!(
                    "macro_rules! emit{{()=>{{{SUFFIX}}}}}macro_rules! wrap{{()=>{{emit!();}}}}wrap!();"
                ),
            };
            let (mut db, parent, site) = world(&text, edition);
            db.begin_run().unwrap();
            let selected = proof(&mut db, &site);
            let id = selected.item().def_key.fingerprint();
            let source_revision = db.source_revision();
            let revision = db.revision();
            let count = db.inputs.len();
            let start = db.trace.events.len();
            db.publish_ordinary_local_function(&selected).unwrap();
            assert_eq!(db.inputs.len(), count + 1);
            assert_eq!(db.revision(), revision + 1);
            assert_eq!(db.source_revision(), source_revision);
            assert!(db.run_active);
            assert!(
                matches!(db.inputs[&InputKey::DefinitionBody(id)].value.as_ref(), InputValue::DefinitionBody(body) if Some(body) == selected.item().body.as_ref())
            );
            selected.revalidate(&db).unwrap();
            let callee = selected.callable(&mut db).unwrap();
            assert!(callee.lexical_contract().is_none());
            assert!(Arc::ptr_eq(
                callee.ordinary_local_contract().unwrap().parent(),
                &parent
            ));
            callee.revalidate(&mut db).unwrap();
            no_executable_work(&db, start);
            let current = proof(&mut db, &site);
            let before = stamps(&db);
            let revision = db.revision();
            db.publish_ordinary_local_function(&current).unwrap();
            assert_eq!(stamps(&db), before);
            assert_eq!(db.revision(), revision);
            db.finish_run().unwrap();
        }
    }
}
#[test]
fn ordinary_local_publication_requires_active_run_and_query_coordinator_atomically() {
    let (mut db, _, site) = world(SUFFIX, 2024);
    let selected = proof(&mut db, &site);
    let before = stamps(&db);
    let error = db.publish_ordinary_local_function(&selected).unwrap_err();
    assert_eq!(
        error.message,
        "ordinary local admission requires its active run"
    );
    assert_eq!(stamps(&db), before);
    db.begin_run().unwrap();
    let selected = proof(&mut db, &site);
    db.active_queries
        .push(QueryKey::SourceLocalPlan(site.body.clone()));
    let error = db.publish_ordinary_local_function(&selected).unwrap_err();
    assert_eq!(error.message, "coordinator work during pure query");
    db.active_queries.pop();
    assert_eq!(stamps(&db), before);
    db.finish_run().unwrap();
}
#[test]
fn ordinary_local_publication_checks_actual_owner_header_and_parent_before_mutation() {
    for variant in 0..17 {
        let (mut db, parent, site) = world(SUFFIX, 2024);
        db.begin_run().unwrap();
        let selected = proof(&mut db, &site);
        let id = selected.item().def_key.fingerprint();
        match variant {
            0 => {
                let mut owner = site.body.clone();
                owner.definition = Digest([61; 32]);
                // Preserve the old cached stamp to distinguish actual-value
                // checks from the already mandatory semantic input guard.
                db.inputs
                    .get_mut(&InputKey::LocalDeclarationOwner(id))
                    .unwrap()
                    .value = Arc::new(InputValue::LocalDeclarationOwner(owner));
            }
            1 => {
                let mut header = selected.item().header.clone();
                header.tokens[0].text.push_str("_wrong");
                db.inputs
                    .get_mut(&InputKey::DefinitionHeader(id))
                    .unwrap()
                    .value = Arc::new(InputValue::DefinitionHeader(header));
            }
            2 => {
                let mut context = parent.context().source().context().clone();
                context.range.start += 1;
                db.inputs
                    .get_mut(&InputKey::DeclarationContext(site.body.definition))
                    .unwrap()
                    .value = Arc::new(InputValue::DeclarationContext(context));
            }
            // E-R5-006: the cache-budget refusal variant was removed with the budget.
            3 => continue,
            4 => {
                let mut parse = selected.source().clone();
                parse.content = Digest([62; 32]);
                db.inputs
                    .get_mut(&InputKey::DefinitionSource(site.body.definition))
                    .unwrap()
                    .value = Arc::new(InputValue::DefinitionSource(parse));
            }
            5 => {
                let mut header = parent.context().source().header().clone();
                header.tokens[0].text.push_str("_wrong");
                db.inputs
                    .get_mut(&InputKey::DefinitionHeader(site.body.definition))
                    .unwrap()
                    .value = Arc::new(InputValue::DefinitionHeader(header));
            }
            6 => {
                let mut body = parent.context().source().body().clone();
                body.range.end += 1;
                db.inputs
                    .get_mut(&InputKey::DefinitionBody(site.body.definition))
                    .unwrap()
                    .value = Arc::new(InputValue::DefinitionBody(body));
            }
            7 => {
                let item = selected.item();
                let mut context = crate::syntax::outline::DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                };
                context.range.end += 1;
                db.inputs
                    .get_mut(&InputKey::DeclarationContext(id))
                    .unwrap()
                    .value = Arc::new(InputValue::DeclarationContext(context));
            }
            8 => {
                let mut parse = selected.source().clone();
                parse.content = Digest([63; 32]);
                db.inputs
                    .get_mut(&InputKey::DefinitionSource(id))
                    .unwrap()
                    .value = Arc::new(InputValue::DefinitionSource(parse));
            }
            9 => {
                let mut body = selected.item().body.clone().unwrap();
                body.range.end += 1;
                let value = InputValue::DefinitionBody(body);
                db.inputs.insert(
                    InputKey::DefinitionBody(id),
                    InputRecord {
                        fingerprint: value.fingerprint(),
                        value: Arc::new(value),
                    },
                );
            }
            10 => {
                // Reissue an ACTUAL opaque envelope in its own coordinator;
                // no caller struct/digest constructs a body environment.
                let (mut other, other_parent, _) = world(SUFFIX, 2024);
                let environment = other_parent.environment().unwrap();
                let key = InputKey::BodyEnvironment(environment.body().clone());
                other.inputs.remove(&key).unwrap();
                ready(other.publish_body_environment(
                    Arc::clone(environment.source()),
                    Arc::clone(environment.macros()),
                ));
                let actual = other.input(&key).unwrap();
                let InputValue::BodyEnvironment(reissued) = actual.as_ref() else {
                    panic!()
                };
                assert_ne!(
                    reissued.identity(),
                    parent.environment().unwrap().identity()
                );
                // Keep old stamp: publisher must check actual opaque identity.
                db.inputs.get_mut(&key).unwrap().value = actual;
            }
            11 => {
                db.finish_run().unwrap();
                db.begin_expansion_epoch(2).unwrap();
                db.begin_run().unwrap();
            }
            12 => {
                db.finish_run().unwrap();
                db.publish_inputs(BTreeMap::new()).unwrap();
                db.begin_run().unwrap();
            }
            // E-R5-006: the source-input budget variant was removed with the budget.
            13 => continue,
            14 => {
                let original = parent.context().source().original_source().unwrap();
                db.inputs
                    .remove(&InputKey::CapturedSource(
                        original.selection().location_identity(),
                    ))
                    .unwrap();
            }
            15 => {
                db.inputs
                    .remove(&InputKey::SourceBlob(selected.source().content))
                    .unwrap();
            }
            _ => {
                db.inputs
                    .remove(&InputKey::DiscoverySeal(
                        parent.environment().unwrap().macros().seal().clone(),
                    ))
                    .unwrap();
            }
        }
        if matches!(variant, 11 | 12 | 14 | 15 | 16) {
            // These revoke genuine source/epoch/seal inputs themselves, without
            // changing the selected parent/header rows to force another check.
            assert!(
                selected.revalidate(&db).is_err(),
                "guard admitted revoked input: variant={variant}"
            );
        }
        let before = stamps(&db);
        let actual_before = db
            .inputs
            .iter()
            .map(|(key, row)| (key.clone(), row.value.fingerprint()))
            .collect::<BTreeMap<_, _>>();
        let revision = db.revision();
        let start = db.trace.events.len();
        let error = db.publish_ordinary_local_function(&selected).unwrap_err();
        let expected = match variant {
            0 => "ordinary local admission actual declaration owner differs",
            1 => "ordinary local admission immutable declaration row conflict",
            2 => "ordinary local admission owning parent changed",
            4 | 5 | 6 | 10 => "ordinary local admission owning parent changed",
            7 | 8 | 9 => "ordinary local admission immutable declaration row conflict",
            11 | 12 | 14 | 15 | 16 => "prepared operation input changed",
            _ => unreachable!(),
        };
        assert_eq!(error.message, expected, "{error:?}");
        assert_eq!(stamps(&db), before);
        assert_eq!(
            db.inputs
                .iter()
                .map(|(key, row)| (key.clone(), row.value.fingerprint()))
                .collect::<BTreeMap<_, _>>(),
            actual_before
        );
        assert_eq!(db.revision(), revision);
        if variant != 9 {
            assert!(!db.inputs.contains_key(&InputKey::DefinitionBody(id)));
        }
        assert_eq!(
            db.trace.events.len(),
            start,
            "publication queried: variant={variant}"
        );
        no_executable_work(&db, start);
        db.finish_run().unwrap();
    }
}

#[test]
fn ordinary_local_publication_revision_overflow_refuses_before_mutation() {
    let (mut db, _, site) = world(SUFFIX, 2024);
    db.begin_run().unwrap();
    let selected = proof(&mut db, &site);
    db.revision = u64::MAX;
    let before = stamps(&db);
    let trace = db.trace.events.len();
    let error = db.publish_ordinary_local_function(&selected).unwrap_err();
    assert_eq!(error.code, "LRR-E-ID-EXHAUSTED");
    assert_eq!(
        error.message,
        "ordinary local publication revision exhausted"
    );
    assert_eq!(stamps(&db), before);
    assert_eq!(db.revision(), u64::MAX);
    assert_eq!(db.trace.events.len(), trace);
    db.finish_run().unwrap();
}

#[test]
fn ordinary_callable_parent_and_guard_remain_accounted_after_source_memos_retire() {
    let (mut db, parent, site) = world(SUFFIX, 2024);
    db.begin_run().unwrap();
    let selected = proof(&mut db, &site);
    let guard = Arc::clone(selected.guard());
    let weak_guard = Arc::downgrade(&guard);
    let weak_parent = Arc::downgrade(&parent);
    let parent_bytes = parent.exclusive_metadata_bytes();
    let guard_bytes = guard.metadata_bytes() + 2 * std::mem::size_of::<usize>();
    assert!(selected.metadata_bytes() >= guard_bytes);
    db.publish_ordinary_local_function(&selected).unwrap();
    let callee = selected.callable(&mut db).unwrap();
    let retained = callee.ordinary_local_contract().unwrap();
    assert_eq!(
        retained.metadata_bytes(),
        std::mem::size_of::<OrdinaryLocalAdmission>()
            + 2 * std::mem::size_of::<usize>()
            + callee.signature().estimated_bytes()
            + guard_bytes
            + retained.source().profile.semantics.capacity()
    );
    assert!(callee.retained_lexical_metadata_bytes() >= guard_bytes);
    drop(guard);
    drop(parent);
    db.finish_run().unwrap();

    // Revoke real authority and all cached outer query values. Only the actual
    // callable owns the external parent/guard handles; no synthetic pin is added.
    db.publish_inputs(BTreeMap::new()).unwrap();
    db.memos.clear();
    db.evict_between_runs(crate::engine::telemetry::CacheBudgets::default())
        .unwrap();
    assert!(weak_parent.upgrade().is_some());
    assert!(weak_guard.upgrade().is_some());
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= parent_bytes);
    assert!(db.body_environment_root_bytes(None).unwrap() > 0);
    assert!(callee.revalidate(&mut db).is_err());

    db.begin_run().unwrap();
    drop(callee);
    // Guard owns cached stamps only and can retire immediately. The parent
    // graph's existing registry wrapper defers its last drop to quiescence.
    assert!(weak_guard.upgrade().is_none());
    assert!(weak_parent.upgrade().is_some());
    db.finish_run().unwrap();
    db.evict_between_runs(crate::engine::telemetry::CacheBudgets::default())
        .unwrap();
    assert!(weak_parent.upgrade().is_none());
    assert_eq!(db.body_environment_root_bytes(None).unwrap(), 0);
}
