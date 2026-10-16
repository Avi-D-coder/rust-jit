//! Accounting of a genuine installed source query handle after its memo retires.
//! This is a quiescent authority/lifetime control, not guest runtime acceptance.
use super::*;
use crate::{
    engine::telemetry::CacheBudgets,
    frontend::prepared_constructors::PreparedRecordConstructorHandle,
};

#[test]
fn retained_prepared_query_handle_is_charged_after_real_memo_replacement_until_release() {
    let (mut db, plan) = world(ORIGINAL, 2024, false);
    let key = {
        let index = plan.context().source().index();
        let node = index
            .nodes()
            .iter()
            .find(|node| node.constructor_guard.is_some())
            .expect("unchanged source has an actual record constructor");
        index.pc(node.id, Phase::Entry).unwrap()
    };
    let demand = QueryKey::BoundRecordConstructor(key.clone());
    let retained: PreparedRecordConstructorHandle = {
        let selected = ready(db.fetch(&SyntaxProvider, demand.clone()));
        let QueryValue::BoundRecordConstructor(prepared) = selected.value.as_ref() else {
            panic!("actual record binding query variant differs")
        };
        let mut installed = InstalledRecipes::new(
            db.source_revision(),
            crate::engine::telemetry::INSTALLED_LRIR_BUDGET,
        )
        .unwrap();
        let id = ready(installed.resolve(&mut db, key.clone()));
        let handle = installed
            .prepared_constructor(&db, id)
            .unwrap()
            .expect("installed record retains its actual query allocation");
        assert!(std::ptr::eq(handle.prepared(), prepared.as_ref()));
        // Both the Outcome outer owner and installed table are dropped here.
        // The one typed handle retains the original Arc<QueryValue>, never a
        // detached clone of the bound schema or a fabricated source proof.
        handle
    };
    let payload_bytes = retained.prepared().metadata_bytes().unwrap();
    assert!(payload_bytes > 0);
    drop(plan);

    // Public complete-universe revocation: no private input/memo mutation and
    // no test-only freshness escape. The real Signature provider requires the
    // owning DefinitionHeader before ConstructorPlan can be reconstructed.
    db.publish_inputs(std::collections::BTreeMap::new())
        .unwrap();
    match db.fetch(&SyntaxProvider, demand.clone()) {
        QueryPoll::Ready(Err(error)) => {
            assert_eq!(error.code, "LRR-E-NAME-MISSING", "{error:?}");
            assert_eq!(error.message, "definition header not found", "{error:?}");
        }
        QueryPoll::Ready(Ok(_)) => panic!("revoked source still admitted a record binding"),
        QueryPoll::NeedInput(input) => {
            panic!("revoked record binding demanded another input: {input:?}")
        }
    }
    assert!(matches!(
        db.memo(&demand).unwrap().value.as_ref(),
        QueryValue::Error(error)
            if error.code == "LRR-E-NAME-MISSING"
                && error.message == "definition header not found"
    ));
    // The cached success is gone, while its immutable data remains readable.
    // This is lifetime, not permission to execute the revoked constructor.
    assert_eq!(retained.prepared().key(), &key);
    let pinned = db.cache_usage().unwrap();
    assert!(pinned.memo_bytes >= payload_bytes);
    assert!(pinned.pinned_memo_bytes >= payload_bytes);
    let zero = CacheBudgets {
        query_bytes: 0,
        parse_bytes: 0,
        source_input_bytes: 0,
    };
    let error = db.evict_between_runs(zero).unwrap_err();
    assert_eq!(error.code, "LRR-E-METADATA-LIMIT", "{error:?}");
    assert_eq!(
        error.message, "pinned cache/source inputs exceed configured cap",
        "{error:?}"
    );
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= payload_bytes);

    drop(retained);
    let released = db.evict_between_runs(zero).unwrap();
    assert_eq!(released.after.memo_bytes, 0);
    assert_eq!(released.after.parse_bytes, 0);
    assert_eq!(released.after.source_input_bytes, 0);
}
