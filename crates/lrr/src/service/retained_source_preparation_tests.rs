//! Production coordinator assembly, driven by actual discovery requests and
//! records from unchanged upstream source. No guest execution is claimed here.
use super::retained_syntax_tests::{fixture, seal};
#[test]
fn sealed_source_preparation_replays_actual_nested_upstream_requests_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut f = fixture(edition, true);
        seal(&mut f);
        let prepared = f
            .discovery
            .prepare_source_syntax(f.prepared.source())
            .unwrap();
        assert_eq!(prepared.identity(), f.prepared.identity());
        assert_eq!(prepared.syntax().identity(), f.prepared.syntax().identity());
        assert_eq!(prepared.outputs().len(), 2);
        assert!(prepared.pending_calls().is_empty());
        let again = f
            .discovery
            .prepare_source_syntax(f.prepared.source())
            .unwrap();
        assert_eq!(again.identity(), prepared.identity());
    }
}
#[test]
fn source_preparation_requires_the_real_completed_discovery() {
    let f = fixture(2024, true);
    let error = match f.discovery.prepare_source_syntax(f.prepared.source()) {
        Ok(_) => panic!("unsealed preparation was admitted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(error.message.contains("completed discovery"));
}
#[test]
fn sealed_preparation_cannot_borrow_another_source_owner() {
    let mut f = fixture(2024, true);
    seal(&mut f);
    let mut other = f.prepared.source().clone();
    other.path = crate::source::snapshot::SourcePath::new("other.rs").unwrap();
    let error = match f.discovery.prepare_source_syntax(&other) {
        Ok(_) => panic!("another source owner was admitted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(error.message.contains("captured owner absent"));
}
