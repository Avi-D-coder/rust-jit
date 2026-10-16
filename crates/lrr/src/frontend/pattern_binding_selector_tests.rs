// LRR dependency/authority controls over unchanged official source worlds.
// This does not run the stock program or classify a refusal as its runtime pass.
use super::*;
use crate::{
    core::digest::StableEncoder,
    db::{
        dependency::missing_fingerprint,
        key::QueryKey,
        memo::MemoEntry,
        poll::QueryPoll,
        provider::{Provided, Provider},
        trace::Action,
        value::QueryValue,
    },
    syntax::outline::ScopeKey,
};
const SEARCH: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/search-via-bindings.rs"
);
const MODULE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/blind-item-mixed-use-item.rs"
);

fn selector_key(source: &SourceSelection, scope: &ScopeKey) -> InputKey {
    // Independent encoding oracle for the fixed selector identity, not a name
    // resolver or permission to manufacture a derived publication.
    let mut source_key = StableEncoder::new("lrr/v1/scope-certificate-source-selection");
    source_key.string(source.path.as_str());
    source_key.record(&source.outline);
    let mut selection = StableEncoder::new("lrr/v1/scope-certificate-selection");
    selection.digest(source_key.finish());
    selection.digest(scope.fingerprint());
    selection.u32(Namespace::Value.tag());
    selection.u16(source.outline.parse.profile.edition);
    InputKey::ScopeCertificateSelection(selection.finish())
}
fn publish_scope(
    db: &mut Database,
    source: &PreparedSourceView,
    scope: &ScopeKey,
    namespace: Namespace,
    graph: &'static [u8],
    epoch: u64,
) {
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        scope.clone(),
        namespace,
        Digest::of_bytes(graph),
        epoch,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"scalar binding actual compile environment"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    let mut complete = false;
    for _ in 0..100_000 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => {
                complete = true;
                break;
            }
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!(
                "actual official source selector discovery incomplete: {diagnostic}; {blocked:?}"
            ),
            _ => panic!("actual source selector discovery requires another authorized producer"),
        }
    }
    assert!(complete);
    discovery.publish(db).unwrap();
}
struct SelectorQuery<'a> {
    source: &'a SourceSelection,
    scope: &'a ScopeKey,
}
impl Provider for SelectorQuery<'_> {
    fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
        match db.current_lexical_scope_certificate(self.source, self.scope) {
            Ok(selected) => Provided::pure(QueryValue::Error(Diagnostic::new(
                "LRR-TEST-SCOPE-OBSERVATION",
                if selected.is_some() {
                    "selected"
                } else {
                    "absent"
                }
                .into(),
            ))),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
fn fetch<'a>(
    db: &'a mut Database,
    query: &SelectorQuery<'_>,
    key: &QueryKey,
    expected: &str,
) -> &'a MemoEntry {
    // Error is intentionally transported as ReadyErr by Database::returned.
    // The memo retains the pure observation and its actual dependency stamps.
    match db.fetch(query, key.clone()) {
        QueryPoll::Ready(Err(observation)) => {
            assert_eq!(observation.code, "LRR-TEST-SCOPE-OBSERVATION");
            assert_eq!(observation.message, expected);
        }
        other => panic!("actual selector observation did not preserve Error transport: {other:?}"),
    }
    let memo = db
        .memo(key)
        .expect("pure selector observation was not retained");
    let QueryValue::Error(observation) = memo.value.as_ref() else {
        panic!("selector memo variant differs")
    };
    assert_eq!(observation.code, "LRR-TEST-SCOPE-OBSERVATION");
    assert_eq!(observation.message, expected);
    assert_eq!(memo.fingerprint, memo.value.fingerprint());
    memo
}
fn input_stamp(db: &Database, query: &QueryKey, selector: &InputKey) -> Digest {
    db.memo(query)
        .expect("selector memo absent")
        .dependencies
        .iter()
        .find_map(|dependency| match dependency {
            DependencyStamp::Input { key, fingerprint } if key == selector => Some(*fingerprint),
            _ => None,
        })
        .expect("query did not retain relevant missing/present selector dependency")
}

#[test]
fn official_pattern_selector_tracks_relevant_same_epoch_publication_without_unrelated_namespace_invalidation_all_editions()
 {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, _, _) = world(SEARCH, "search", edition);
        db.trace.detailed = true;
        let scope = source.outline().root_scope.clone();
        let selector = selector_key(source.selection(), &scope);
        let before = db.input(&selector).unwrap().fingerprint();
        let query = SelectorQuery {
            source: source.selection(),
            scope: &scope,
        };
        let key = QueryKey::TypeAlias(Digest::of_bytes(
            b"scope-selector-positive-dependency-control",
        ));
        fetch(&mut db, &query, &key, "selected");
        assert_eq!(input_stamp(&db, &key, &selector), before);
        assert!(db.trace.events.iter().any(|event| event.key == key
            && event.dependencies.iter().any(|dependency| matches!(
                dependency,
                DependencyStamp::Input {
                    key: InputKey::ExpansionEpoch,
                    ..
                }
            ))));
        publish_scope(
            &mut db,
            &source,
            &scope,
            Namespace::Type,
            b"actual unrelated Type scope certificate",
            1,
        );
        assert_eq!(db.input(&selector).unwrap().fingerprint(), before);
        let provider_runs = db
            .trace
            .counts
            .get("TypeAlias:ProviderRun")
            .copied()
            .unwrap_or(0);
        fetch(&mut db, &query, &key, "selected");
        assert_eq!(
            db.trace
                .counts
                .get("TypeAlias:ProviderRun")
                .copied()
                .unwrap_or(0),
            provider_runs
        );
        assert!(
            db.trace
                .events
                .iter()
                .rev()
                .any(|event| event.key == key && event.action == Action::ValidatedReuse)
        );
        publish_scope(
            &mut db,
            &source,
            &scope,
            Namespace::Value,
            b"actual second same-epoch Value certificate",
            1,
        );
        assert_ne!(db.input(&selector).unwrap().fingerprint(), before);
        assert_eq!(
            db.current_lexical_scope_certificate(source.selection(), &scope)
                .unwrap_err()
                .code,
            "LRR-E-CALL-CONTEXT"
        );
        match db.fetch(&query, key.clone()) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-CALL-CONTEXT"),
            other => {
                panic!("ambiguous selector did not report actual authority refusal: {other:?}")
            }
        }
        assert!(
            db.trace
                .events
                .iter()
                .any(|event| event.key == key && event.action == Action::Invalidated)
        );
    }
}

#[test]
fn official_nested_scope_missing_selector_becomes_present_and_epoch_reset_or_forged_base_cannot_preserve_authority()
 {
    let (mut db, source, _, _) = world(MODULE, "main", 2024);
    db.trace.detailed = true;
    let root = source.outline().root_scope.clone();
    let nested = source
        .outline()
        .scopes
        .keys()
        .find(|scope| scope.owner.is_none() && scope.module_path == ["m"])
        .unwrap()
        .clone();
    let root_selector = selector_key(source.selection(), &root);
    let root_before = db.input(&root_selector).unwrap().fingerprint();
    let nested_selector = selector_key(source.selection(), &nested);
    assert!(db.input(&nested_selector).is_none());
    let key = QueryKey::TypeAlias(Digest::of_bytes(
        b"scope-selector-negative-dependency-control",
    ));
    let query = SelectorQuery {
        source: source.selection(),
        scope: &nested,
    };
    fetch(&mut db, &query, &key, "absent");
    assert_eq!(
        input_stamp(&db, &key, &nested_selector),
        missing_fingerprint()
    );
    publish_scope(
        &mut db,
        &source,
        &nested,
        Namespace::Value,
        b"actual original nested module Value scope",
        1,
    );
    let nested_value = db.input(&nested_selector).unwrap();
    assert_ne!(nested_value.fingerprint(), missing_fingerprint());
    assert_eq!(db.input(&root_selector).unwrap().fingerprint(), root_before);
    fetch(&mut db, &query, &key, "selected");
    assert_eq!(
        input_stamp(&db, &key, &nested_selector),
        nested_value.fingerprint()
    );
    assert!(
        db.trace
            .events
            .iter()
            .any(|event| event.key == key && event.action == Action::Invalidated)
    );
    // A caller can construct the public input value, but only genuine discovery
    // may publish its derived locator. Rejection must leave revision/state intact.
    let mut forged = source.prepared_inputs().clone();
    forged.insert(nested_selector.clone(), nested_value.as_ref().clone());
    let revision = db.revision();
    assert_eq!(
        db.publish_inputs(forged).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.revision(), revision);
    assert_eq!(
        db.input(&nested_selector).unwrap().fingerprint(),
        nested_value.fingerprint()
    );
    db.begin_expansion_epoch(2).unwrap();
    fetch(&mut db, &query, &key, "absent");
    assert!(
        db.current_lexical_scope_certificate(source.selection(), &root)
            .unwrap()
            .is_none()
    );
    // The immutable old selector row is retained, but epoch makes it unusable.
    assert_eq!(
        db.input(&nested_selector).unwrap().fingerprint(),
        nested_value.fingerprint()
    );
    db.publish_inputs(source.prepared_inputs().clone()).unwrap();
    assert!(db.input(&nested_selector).is_none());
    assert!(db.input(&root_selector).is_none());
    fetch(&mut db, &query, &key, "absent");
    assert_eq!(
        input_stamp(&db, &key, &nested_selector),
        missing_fingerprint()
    );
}
