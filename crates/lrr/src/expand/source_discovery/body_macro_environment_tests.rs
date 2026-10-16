//! Authority/currentness controls over unchanged official exported macro source;
//! no guest execution or complete upstream runtime contract is claimed.
use super::*;
use crate::db::poll::{InputRequest, QueryPoll};
const EXPORTED: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/auxiliary/foreign-crate-macro-pat.rs"
);
#[test]
fn selected_environment_requires_actual_completed_and_published_discovery() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source) = prepare(EXPORTED, edition, CfgSet::guest_default(), &[]);
        let mut d = discovery(&source, Namespace::Macro, false);
        assert!(matches!(
            d.sealed_body_macro_environment(&mut db, source.selection()),
            QueryPoll::Ready(Err(_))
        ));
        seal(&mut d);
        match d.sealed_body_macro_environment(&mut db, source.selection()) {
            QueryPoll::NeedInput(InputRequest::Discover { key }) => assert_eq!(&key, d.key()),
            other => panic!("unpublished source environment acquired authority: {other:?}"),
        }
        d.publish(&mut db).unwrap();
        let environment = match d.sealed_body_macro_environment(&mut db, source.selection()) {
            QueryPoll::Ready(Ok(environment)) => environment,
            other => panic!("genuine selected source environment unavailable: {other:?}"),
        };
        assert!(environment.source().same_publication(&source));
        assert_eq!(environment.index().source(), source.selection());
        assert_eq!(environment.seal(), d.key());
        assert!(
            environment.exclusive_metadata_bytes() >= std::mem::size_of_val(environment.as_ref())
        );
        assert!(matches!(
            environment.require_current(&mut db),
            QueryPoll::Ready(Ok(()))
        ));
        let second = match d.sealed_body_macro_environment(&mut db, source.selection()) {
            QueryPoll::Ready(Ok(environment)) => environment,
            other => panic!("same genuine source environment unavailable: {other:?}"),
        };
        assert_eq!(environment.identity(), second.identity());
        assert_eq!(environment.dependency(), second.dependency());
        // Owning query/publication reuses the retained receipt; currentness
        // checks do not remint or encode the actual index again.
        let identity = environment.identity();
        db.begin_expansion_epoch(2).unwrap();
        match environment.require_current(&mut db) {
            QueryPoll::Ready(Err(error)) => {
                assert_eq!(error.code, "LRR-E-QUERY-REQUEST");
                assert_eq!(error.message, "discovery query has stale epoch");
            }
            other => panic!("stale owning environment lost its typed refusal: {other:?}"),
        }
        assert_eq!(environment.identity(), identity);
    }
}
#[test]
fn source_or_body_spelling_cannot_select_an_unadmitted_macro_environment() {
    let (mut db, source) = prepare(EXPORTED, 2018, CfgSet::guest_default(), &[]);
    let mut d = discovery(&source, Namespace::Macro, false);
    seal(&mut d);
    d.publish(&mut db).unwrap();
    let mut wrong = source.selection().clone();
    wrong.path = SourcePath::new("same-content-other-owner.rs").unwrap();
    assert!(matches!(
        d.sealed_body_macro_environment(&mut db, &wrong),
        QueryPoll::Ready(Err(_))
    ));
    wrong = source.selection().clone();
    wrong.outline.crate_key = Digest([232; 32]);
    assert!(matches!(
        d.sealed_body_macro_environment(&mut db, &wrong),
        QueryPoll::Ready(Err(_))
    ));
    wrong = source.selection().clone();
    wrong.outline.cfg = Digest([233; 32]);
    assert!(matches!(
        d.sealed_body_macro_environment(&mut db, &wrong),
        QueryPoll::Ready(Err(_))
    ));
}
