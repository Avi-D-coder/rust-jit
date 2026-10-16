//! Retention/currentness controls over unchanged official qualified source.
//! This checks shared preparation ownership, not guest fixture execution.
use super::*;
use crate::db::poll::{InputRequest, QueryPoll};
const QUALIFIED: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/module-qualified-struct-destructure.rs"
);
fn completed(d: &SourceDiscovery, db: &mut Database) -> Arc<CompletedSourceEnvironment> {
    match d.completed_source_environment(db) {
        QueryPoll::Ready(Ok(root)) => root,
        other => panic!("actual completed source environment unavailable: {other:?}"),
    }
}
#[test]
fn qualified_source_uses_one_actual_completed_graph_and_source_vector() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source) = prepare(QUALIFIED, edition, CfgSet::guest_default(), &[]);
        let mut d = discovery(&source, Namespace::Type, false);
        assert!(matches!(
            d.completed_source_environment(&mut db),
            QueryPoll::Ready(Err(_))
        ));
        seal(&mut d);
        assert!(matches!(
            d.completed_source_environment(&mut db),
            QueryPoll::NeedInput(InputRequest::Discover { .. })
        ));
        d.publish(&mut db).unwrap();
        let first = completed(&d, &mut db);
        let revision = db.revision();
        let second = completed(&d, &mut db);
        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(first.names_root(), second.names_root()));
        assert!(Arc::ptr_eq(first.sources_root(), second.sources_root()));
        assert_eq!(
            db.revision(),
            revision,
            "sharing must not publish source inputs"
        );
        db.begin_run().unwrap();
        assert!(
            matches!(
                d.completed_source_environment(&mut db),
                QueryPoll::Ready(Err(_))
            ),
            "source retention publication remains preparation-only"
        );
        db.finish_run().unwrap();
        assert_eq!(db.revision(), revision);
        assert_eq!(first.sources().len(), 1);
        assert!(first.sources()[0].same_publication(&source));
        for scope in source
            .outline()
            .scopes
            .keys()
            .filter(|scope| scope.owner.is_none())
        {
            match first.scope_owner(scope) {
                SourceScopeOwner::Selected(owner) => assert!(owner.same_publication(&source)),
                SourceScopeOwner::Ambiguous(owners) => {
                    panic!("unchanged inline source has duplicate owners: {owners:?}")
                }
                SourceScopeOwner::Missing => panic!("actual original/inline scope owner missing"),
            }
        }
        let mut absent = source.outline().root_scope.clone();
        absent.crate_key = Digest([231; 32]);
        assert!(matches!(
            first.scope_owner(&absent),
            SourceScopeOwner::Missing
        ));
        first
            .names()
            .require_source(source.selection(), source.outline())
            .unwrap();
        let parts = [
            crate::syntax::imports::ImportSegment::Name("m".into()),
            crate::syntax::imports::ImportSegment::Name("S".into()),
        ];
        let actual = first
            .names()
            .lookup_source_path(
                Namespace::Type,
                &parts,
                false,
                &source.outline().root_scope,
                edition,
            )
            .unwrap();
        assert!(
            matches!(actual, NameLookup::Defined(_)),
            "ordinary completed module kernel: {actual:?}"
        );
        assert!(
            unique_graph(&mut d.graph).is_err(),
            "completed graph cannot become mutable or deep-cloned"
        );
        let selected = match d.sealed_body_macro_environment(&mut db, source.selection()) {
            QueryPoll::Ready(Ok(selected)) => selected,
            other => panic!("selected source receipt: {other:?}"),
        };
        assert!(Arc::ptr_eq(selected.source_environment(), &first));
        db.begin_expansion_epoch(2).unwrap();
        assert!(matches!(
            d.completed_source_environment(&mut db),
            QueryPoll::Ready(Err(_))
        ));
        assert!(matches!(
            selected.require_current(&mut db),
            QueryPoll::Ready(Err(_))
        ));
    }
}
#[test]
fn naked_graph_handle_retains_charged_source_owner_until_between_run_release() {
    let (mut db, source) = prepare(QUALIFIED, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&source, Namespace::Type, false);
    seal(&mut d);
    d.publish(&mut db).unwrap();
    let environment = completed(&d, &mut db);
    let graph = Arc::clone(environment.names_root());
    let weak = Arc::downgrade(&environment);
    drop(environment);
    drop(d);
    drop(source);
    db.publish_inputs(Default::default()).unwrap();
    db.evict_between_runs(crate::engine::telemetry::CacheBudgets::default())
        .unwrap();
    assert!(
        weak.upgrade().is_some(),
        "raw graph still owns a charged retirement lease"
    );
    let retained = weak.upgrade().unwrap().retained_metadata_bytes().unwrap();
    assert!(retained > 0);
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= retained);
    drop(graph);
    db.evict_between_runs(crate::engine::telemetry::CacheBudgets::default())
        .unwrap();
    assert!(
        weak.upgrade().is_none(),
        "last destructor belongs to coordinator release"
    );
}
