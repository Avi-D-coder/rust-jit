//! LRR currentness/ownership controls over unchanged upstream source. These
//! prepare real module answers; they do not execute or pass the stock program.
use super::*;
use crate::{
    db::trace::Action,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::compile_input_catalog::CompileInputCatalog,
};

const SOURCE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
);

fn world(edition: u16) -> (Database, PreparedSourceView) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(SOURCE, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    (db, source)
}

fn publish(db: &mut Database, source: &PreparedSourceView, namespace: Namespace, epoch: u64) {
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        namespace,
        Digest::of_bytes(b"current module-name actual source discovery"),
        epoch,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"current module-name actual compile environment"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    for _ in 0..100_000 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(db).unwrap();
                return;
            }
            _ => panic!("actual upstream source discovery requires another authorized producer"),
        }
    }
    panic!("actual upstream source discovery exceeded its control budget")
}

fn key(source: &PreparedSourceView, namespace: Namespace, name: &str) -> QueryKey {
    QueryKey::CurrentModuleName(Box::new(CurrentModuleNameKey {
        source: source.selection().outline.clone(),
        path: source.selection().path.as_str().to_owned(),
        bucket: BucketKey {
            scope: source.outline().root_scope.clone(),
            namespace,
            name: name.to_owned(),
        },
    }))
}

fn observe(db: &mut Database, key: &QueryKey) -> Arc<CurrentModuleNameObservation> {
    let value = match db.fetch(&SyntaxProvider, key.clone()) {
        QueryPoll::Ready(Ok(value)) => value,
        other => panic!("actual module-name observation failed: {other:?}"),
    };
    assert!(value.cacheable);
    let QueryValue::CurrentModuleName(observation) = value.value.as_ref() else {
        panic!("actual module-name query returned another variant")
    };
    Arc::clone(observation)
}

#[test]
fn upstream_module_answers_distinguish_unsealed_absent_and_defined_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source) = world(edition);
        let search = key(&source, Namespace::Value, "search");
        let pending = observe(&mut db, &search);
        assert_eq!(pending.completion(), ModuleNameCompletion::Unsealed);
        assert!(pending.lookup().is_none());
        assert!(pending.answer_dependency().is_none());

        publish(&mut db, &source, Namespace::Value, 1);
        let selected = observe(&mut db, &search);
        assert_eq!(selected.completion(), ModuleNameCompletion::Complete);
        assert!(matches!(selected.lookup(), Some(NameLookup::Defined(_))));
        assert!(matches!(
            selected.answer_dependency(),
            Some(DependencyStamp::Query {
                key: QueryKey::SealedName(_),
                ..
            })
        ));
        assert!(matches!(
            selected.certificate_dependency(),
            Some(DependencyStamp::Input {
                key: InputKey::DiscoverySeal(_),
                ..
            })
        ));
        for expected in [
            InputKey::CapturedSource(source.selection().location_identity()),
            InputKey::CrateCfg(source.selection().outline.crate_key),
            InputKey::ExpansionEpoch,
        ] {
            assert!(db.memo(&search).unwrap().dependencies.iter().any(|dependency|
                matches!(dependency, DependencyStamp::Input { key, .. } if key == &expected)));
        }
        let absent = observe(
            &mut db,
            &key(&source, Namespace::Value, "missing_module_member"),
        );
        assert_eq!(absent.completion(), ModuleNameCompletion::Complete);
        assert!(matches!(absent.lookup(), Some(NameLookup::Absent)));

        let type_key = key(&source, Namespace::Type, "search");
        // The actual Value parent retains a separately completed Type facet;
        // its existence is no longer hidden behind the primary namespace.
        let type_answer = observe(&mut db, &type_key);
        assert_eq!(type_answer.completion(), ModuleNameCompletion::Complete);
        assert!(matches!(type_answer.lookup(), Some(NameLookup::Absent)));
    }
}

#[test]
fn upstream_module_answer_semantics_survive_certificate_epoch_refresh_with_real_dependencies() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source) = world(edition);
        publish(&mut db, &source, Namespace::Value, 1);
        let request = key(&source, Namespace::Value, "search");
        let first = observe(&mut db, &request);
        let same = observe(&mut db, &request);
        assert!(Arc::ptr_eq(&first, &same));
        drop(same);
        publish(&mut db, &source, Namespace::Type, 1);
        assert!(Arc::ptr_eq(&first, &observe(&mut db, &request)));
        assert!(
            db.trace
                .events
                .iter()
                .any(|event| event.key == request && event.action == Action::ValidatedReuse)
        );

        db.begin_expansion_epoch(2).unwrap();
        let unsealed = observe(&mut db, &request);
        assert_eq!(unsealed.completion(), ModuleNameCompletion::Unsealed);
        assert!(unsealed.lookup().is_none());
        assert_ne!(unsealed.semantic_identity(), first.semantic_identity());
        publish(&mut db, &source, Namespace::Value, 2);
        let refreshed = observe(&mut db, &request);
        assert!(!Arc::ptr_eq(&first, &refreshed));
        assert_eq!(first.lookup(), refreshed.lookup());
        assert_eq!(first.semantic_identity(), refreshed.semantic_identity());
        assert_ne!(first.answer_dependency(), refreshed.answer_dependency());

        // Source removal cannot reuse the still-held old immutable answer as
        // current authority. Existing consumers retain its allocation only.
        db.publish_inputs(Default::default()).unwrap();
        assert!(matches!(
            db.fetch(&SyntaxProvider, request),
            QueryPoll::Ready(Err(_))
        ));
    }
}

#[test]
fn inner_module_observation_remains_charged_after_outer_memo_is_replaced() {
    let (mut db, source) = world(2024);
    publish(&mut db, &source, Namespace::Value, 1);
    let request = key(&source, Namespace::Value, "search");
    let retained = observe(&mut db, &request);
    let before = db.cache_usage().unwrap();
    let clone = Arc::clone(&retained);
    assert_eq!(before.memo_bytes, db.cache_usage().unwrap().memo_bytes);
    drop(clone);
    let bytes = retained.exclusive_metadata_bytes() + 2 * std::mem::size_of::<usize>();
    assert!(before.pinned_memo_bytes >= bytes);
    db.publish_inputs(Default::default()).unwrap();
    assert!(matches!(
        db.fetch(&SyntaxProvider, request),
        QueryPoll::Ready(Err(_))
    ));
    drop(source);
    let retired = db.cache_usage().unwrap();
    assert!(retired.pinned_memo_bytes >= bytes);
    let weak = Arc::downgrade(&retained);
    drop(retained);
    assert!(weak.upgrade().is_none());
    let released = db.cache_usage().unwrap();
    assert!(retired.memo_bytes >= released.memo_bytes + bytes);
}

#[test]
fn cache_only_module_answer_does_not_pin_the_retired_input_universe() {
    let (mut db, source) = world(2024);
    publish(&mut db, &source, Namespace::Value, 1);
    let request = key(&source, Namespace::Value, "search");
    let observation = observe(&mut db, &request);
    let weak = Arc::downgrade(&observation);
    drop(observation);
    drop(source);
    db.publish_inputs(Default::default()).unwrap();
    let eviction = db
        .evict_between_runs(crate::engine::telemetry::CacheBudgets {
            query_bytes: 0,
            parse_bytes: usize::MAX,
            source_input_bytes: usize::MAX,
        })
        .unwrap();
    assert!(eviction.victims.contains(&request));
    assert!(weak.upgrade().is_none());
    assert_eq!(eviction.after.memo_bytes, 0);
}

#[path = "current_type_scope_tests.rs"]
mod type_scope_tests;
