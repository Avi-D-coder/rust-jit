//! Cache/installation controls over the same unchanged upstream local records.
//! Query and installation evidence is not a completed interpreter fixture.
use super::*;
use crate::{engine::install::InstalledRecipes, ir::pc::Phase};

#[path = "prepared_constructor_retention_tests.rs"]
mod retention_tests;

#[test]
fn stock_records_share_one_binding_between_query_and_repeated_installed_lookup_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, retained) in [
            (ORIGINAL.to_owned(), false),
            (
                format!("macro_rules! emit{{()=>{{{ORIGINAL}}}}}emit!();"),
                false,
            ),
            (
                format!(
                    "macro_rules! emit{{()=>{{{ORIGINAL}}}}}macro_rules! wrap{{()=>{{emit!();}}}}wrap!();"
                ),
                false,
            ),
            (ORIGINAL.to_owned(), true),
        ] {
            let (mut db, plan) = world(&text, edition, retained);
            let index = plan.context().source().index();
            let keys: Vec<_> = index
                .nodes()
                .iter()
                .filter(|node| node.constructor_guard.is_some())
                .map(|node| index.pc(node.id, Phase::Entry).unwrap())
                .collect();
            assert_eq!(
                keys.len(),
                2,
                "unchanged source contains exactly A and B constructions"
            );
            let mut installed = InstalledRecipes::new(
                db.source_revision(),
                crate::engine::telemetry::INSTALLED_LRIR_BUDGET,
            )
            .unwrap();
            for key in keys {
                let from = db.trace.events.len();
                let bound = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::BoundRecordConstructor(key.clone()),
                ));
                let QueryValue::BoundRecordConstructor(prepared) = bound.value.as_ref() else {
                    panic!("record query variant differs")
                };
                assert_eq!(prepared.key(), &key);
                assert_eq!(prepared.plan_identity(), prepared.plan().fingerprint());
                assert_eq!(
                    prepared.bound().mapping().constructor(),
                    prepared.plan_identity()
                );
                assert!(
                    !db.trace.events[from..]
                        .iter()
                        .any(|event| matches!(event.key, QueryKey::Thunk(_))),
                    "binding must prepare no executable recipe"
                );
                let id = ready(installed.resolve(&mut db, key.clone()));
                let handle = installed.prepared_constructor(&db, id).unwrap().unwrap();
                assert!(
                    std::ptr::eq(handle.prepared(), prepared.as_ref()),
                    "installed recipe must retain the actual query allocation"
                );
                let events = db.trace.events.len();
                for _ in 0..3 {
                    assert_eq!(ready(installed.resolve(&mut db, key.clone())), id);
                    let reused = installed.prepared_constructor(&db, id).unwrap().unwrap();
                    assert!(std::ptr::eq(handle.prepared(), reused.prepared()));
                }
                assert_eq!(
                    db.trace.events.len(),
                    events,
                    "installed lookup may check stamps but must not reenter a provider"
                );
            }
        }
    }
}

#[test]
fn installed_record_rejects_revoked_owning_epoch_without_replanning() {
    let (mut db, plan) = world(ORIGINAL, 2024, false);
    let index = plan.context().source().index();
    let node = index
        .nodes()
        .iter()
        .find(|node| node.constructor_guard.is_some())
        .unwrap();
    let key = index.pc(node.id, Phase::Entry).unwrap();
    let mut installed = InstalledRecipes::new(
        db.source_revision(),
        crate::engine::telemetry::INSTALLED_LRIR_BUDGET,
    )
    .unwrap();
    let id = ready(installed.resolve(&mut db, key.clone()));
    let retained = installed.prepared_constructor(&db, id).unwrap().unwrap();
    db.begin_expansion_epoch(2).unwrap();
    let events = db.trace.events.len();
    let error = installed.prepared_constructor(&db, id).unwrap_err();
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert!(matches!(
        installed.resolve(&mut db, key),
        QueryPoll::Ready(Err(_))
    ));
    assert_eq!(db.trace.events.len(), events);
    assert_eq!(
        retained.prepared().key().phase,
        0,
        "retaining old immutable facts cannot refresh their authority"
    );
}
