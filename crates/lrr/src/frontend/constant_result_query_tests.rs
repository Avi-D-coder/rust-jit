//! Root-hook acceptance only: genuine public query must refuse before fetch.
//! Pending the parent's result-type readiness integration; not a provider edit.
use super::*;
use crate::db::poll::QueryPoll;
#[test]
fn official_faulting_initializer_is_not_entered_for_known_wrong_expected_type() {
    for (source, name, editions) in [
        (DIV_ZERO, "X", &[2015, 2018, 2021, 2024][..]),
        (PANIC, "Z", &[2015, 2018, 2021][..]),
    ] {
        for edition in editions {
            let (mut db, view, definition) = world(source, name, *edition);
            let counts = db.trace.counts.clone();
            let events = db.trace.events.len();
            let result = crate::frontend::constants::query::constant(
                &mut db,
                &view,
                definition,
                &ConstantWireType::Scalar(PrimitiveType::Bool),
                crate::frontend::constants::ConstLimits::default(),
                1024,
            );
            let QueryPoll::Ready(Err(error)) = result else {
                panic!("known source result mismatch must refuse before query entry")
            };
            assert_eq!(error.code, "LRR-E-CONST-TYPE");
            assert_eq!(db.trace.counts, counts);
            assert_eq!(db.trace.events.len(), events);
            assert_eq!(
                db.trace
                    .counts
                    .get("Constant:ProviderRun")
                    .copied()
                    .unwrap_or(0),
                0
            );
        }
    }
}
#[test]
fn known_const_fn_result_mismatch_does_not_enter_current_query_provider() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(FUNCTIONS, "add", edition);
        let counts = db.trace.counts.clone();
        let events = db.trace.events.len();
        let result = crate::frontend::constants::query::const_call(
            &mut db,
            &view,
            definition,
            vec![arg(44), arg(22)],
            &ConstantWireType::Scalar(PrimitiveType::Bool),
            crate::frontend::constants::ConstLimits::default(),
            1024,
        );
        let QueryPoll::Ready(Err(error)) = result else {
            panic!("known const fn result mismatch entered its provider")
        };
        assert_eq!(error.code, "LRR-E-CONST-TYPE");
        assert_eq!(db.trace.counts, counts);
        assert_eq!(db.trace.events.len(), events);
    }
}
