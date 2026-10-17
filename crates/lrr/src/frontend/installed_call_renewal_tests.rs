//! Actual installed-call source authority only. No VM frame, argument/effect
//! execution or full-fixture pass is manufactured by these component controls.
use super::*;
use crate::{
    db::memo::{BodyExecutionLease, RetainedCallBodyTransition},
    engine::{install::InstalledRecipes, telemetry::INSTALLED_LRIR_BUDGET},
    frontend::explicit_type_arguments::PreparedExplicitTypeArguments,
    ir::{pc::Phase, recipe::Recipe},
    syntax::body_index::BodyIndex,
};

struct ActualCall {
    db: Database,
    source: PreparedSourceView,
    discovery: SourceDiscovery,
    index: Arc<BodyIndex>,
    site: PcKey,
    other: PcKey,
    before: PreparedExplicitTypeArguments,
    definition: Digest,
    bar: Digest,
    old_body: BodyKey,
}
fn actual(edition: u16) -> ActualCall {
    let (mut db, source, discovery, parent) = actual_world(edition);
    let index = Arc::clone(parent.context().source().index());
    let sites = calls(&parent);
    assert_eq!(sites.len(), 3);
    let site = index.pc(sites[0], Phase::Entry).unwrap();
    let other = index.pc(sites[1], Phase::Entry).unwrap();
    let value = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedExplicitTypeArguments(site.clone()),
    ));
    let QueryValue::PreparedExplicitTypeArguments(before) = value.value.as_ref() else {
        panic!("actual Q47 differs")
    };
    let before = before.as_ref().clone();
    let callee = before.callable(&mut db).unwrap();
    assert_eq!(callee.definition().declared_name, "static_assert_exists");
    assert!(callee.signature().parameters.is_empty());
    let definition = callee.identity();
    let old_body = actual_locator(&mut db, definition);
    let mut bars = parent
        .context()
        .facts()
        .items
        .iter()
        .filter(|row| row.item.def_key.declared_name == "Bar");
    let bar = bars.next().unwrap().item.def_key.fingerprint();
    assert!(bars.next().is_none());
    assert_eq!(before.rows().len(), 1);
    assert_eq!(
        before.rows()[0].source().source(),
        &crate::traits::term::TyTerm::Nominal(bar, vec![])
    );
    ActualCall {
        db,
        source,
        discovery,
        index,
        site,
        other,
        before,
        definition,
        bar,
        old_body,
    }
}
fn publish(call: &mut ActualCall, lease: &BodyExecutionLease) -> RetainedCallBodyTransition {
    let prepared = call
        .discovery
        .prepare_body_syntax(call.source.selection(), call.definition)
        .unwrap();
    assert!(prepared.permits_body(call.definition));
    assert!(!prepared.permits_body(call.index.body().definition));
    let input = ready(RetainedSyntaxInput::prepare(
        &mut call.db,
        prepared,
        call.discovery.key().clone(),
    ));
    ready(call.db.publish_retained_syntax(Arc::clone(&input)));
    ready(
        call.db
            .publish_retained_call_body(&call.before, &call.index, &call.site, lease, input),
    )
}
fn snapshot(table: &InstalledRecipes) -> (usize, usize, Vec<Recipe>) {
    (
        table.len(),
        table.metadata_bytes(),
        table
            .recipes()
            .map(|recipe| recipe.recipe().clone())
            .collect(),
    )
}
fn rejected<T>(poll: QueryPoll<T>, code: &str, message: &str) {
    match poll {
        QueryPoll::Ready(Err(error)) => {
            assert_eq!(error.code, code);
            assert_eq!(error.message, message);
        }
        _ => panic!("actual renewal must refuse with {code}: {message}"),
    }
}
fn stale<T>(result: Result<T, Diagnostic>) {
    match result {
        Err(error) => {
            assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
            assert_eq!(error.message, "prepared operation input changed");
        }
        Ok(_) => panic!("actual old source guard must remain stale"),
    }
}
fn only_selected_entry_was_prepared(call: &ActualCall) {
    assert_eq!(call.db.trace.truncated, 0);
    assert!(
        call.db.trace.events.iter().all(|event| match &event.key {
            QueryKey::Thunk(pc) => pc == &call.site,
            QueryKey::BodyStructure(body) => body == call.index.body(),
            _ => true,
        }),
        "other call nodes/phases and actual target executable body stay cold"
    );
}

#[test]
fn actual_second_call_observer_keeps_its_own_type_and_recipe_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut call = actual(edition);
        let mut table =
            InstalledRecipes::new(call.db.source_revision(), INSTALLED_LRIR_BUDGET).unwrap();
        let publisher_id = ready(table.resolve(&mut call.db, call.site.clone()));
        let observer_id = ready(table.resolve(&mut call.db, call.other.clone()));
        let old = table.get_current(&call.db, observer_id).unwrap().clone();
        let observer = old.source_call().unwrap().clone();
        assert_eq!(observer.definition(), call.before.definition());
        assert_ne!(observer.key(), call.before.key());
        assert_eq!(observer.rows().len(), 1);
        assert_ne!(observer.rows()[0].source(), call.before.rows()[0].source());
        let lease = ready(call.db.pin_guest_body(call.index.body()));
        let transition = publish(&mut call, &lease);
        stale(table.get_current(&call.db, observer_id));
        stale(observer.require_current(&call.db, &call.index, &call.other));
        assert!(transition.require_before_plan(&call.db, &observer).is_err());

        let fresh = ready(table.renew_observed_source_call(
            &mut call.db,
            &transition,
            &observer,
            &call.index,
            &call.other,
        ));
        assert_eq!(fresh.key(), observer.key());
        assert_eq!(fresh.rows()[0].source(), observer.rows()[0].source());
        fresh
            .require_current(&call.db, &call.index, &call.other)
            .unwrap();
        assert_eq!(
            fresh.callable(&mut call.db).unwrap().body().unwrap().key(),
            transition.body()
        );
        let renewed = table.get_current(&call.db, observer_id).unwrap();
        assert_eq!(renewed.recipe().key, old.recipe().key);
        assert_eq!(renewed.recipe().origin, old.recipe().origin);
        assert_eq!(renewed.recipe().operations, old.recipe().operations);
        assert_eq!(renewed.recipe().exit, old.recipe().exit);
        assert_eq!(table.len(), 2);
        assert_eq!(
            ready(table.resolve(&mut call.db, call.other.clone())),
            observer_id
        );
        stale(old.revalidate_namespace(&call.db));
        stale(observer.require_current(&call.db, &call.index, &call.other));
        stale(table.get_current(&call.db, publisher_id));
        ready(table.renew_source_call(
            &mut call.db,
            &transition,
            &call.before,
            &call.index,
            &call.site,
        ));
        assert_eq!(
            ready(table.resolve(&mut call.db, call.site.clone())),
            publisher_id
        );
        ready(lease.require_current(&mut call.db));
        let events = call.db.trace.events.len();
        call.db.begin_expansion_epoch(2).unwrap();
        assert!(
            transition
                .require_observer_plan(&call.db, &observer)
                .is_err()
        );
        assert_eq!(call.db.trace.events.len(), events);
    }
}

#[test]
fn actual_installed_call_renews_same_id_and_executable_with_real_bar_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut call = actual(edition);
        let mut table =
            InstalledRecipes::new(call.db.source_revision(), INSTALLED_LRIR_BUDGET).unwrap();
        let id = ready(table.resolve(&mut call.db, call.site.clone()));
        let old = table.get_current(&call.db, id).unwrap().clone();
        assert_eq!(table.len(), 1);
        assert_eq!(
            old.source_call().unwrap().fingerprint(),
            call.before.fingerprint()
        );
        let lease = ready(call.db.pin_guest_body(call.index.body()));
        let transition = publish(&mut call, &lease);
        let new_body = transition.body();
        assert_ne!(new_body, call.old_body);
        stale(table.get_current(&call.db, id));
        stale(old.revalidate_namespace(&call.db));
        stale(
            call.before
                .require_current(&call.db, &call.index, &call.site),
        );

        let fresh = ready(table.renew_source_call(
            &mut call.db,
            &transition,
            &call.before,
            &call.index,
            &call.site,
        ));
        assert_eq!(table.len(), 1);
        assert_eq!(ready(table.resolve(&mut call.db, call.site.clone())), id);
        let renewed = table.get_current(&call.db, id).unwrap();
        assert_eq!(renewed.recipe().key, old.recipe().key);
        assert_eq!(renewed.recipe().origin, old.recipe().origin);
        assert_eq!(renewed.recipe().operations, old.recipe().operations);
        assert_eq!(renewed.recipe().exit, old.recipe().exit);
        assert_eq!(
            renewed.source_call().unwrap().fingerprint(),
            fresh.fingerprint()
        );
        assert_eq!(
            renewed.prepared_inputs_fingerprint(),
            Some(fresh.guard().fingerprint())
        );
        fresh
            .require_current(&call.db, &call.index, &call.site)
            .unwrap();
        assert_eq!(fresh.rows().len(), 1);
        assert_eq!(
            fresh.rows()[0].source().source(),
            &crate::traits::term::TyTerm::Nominal(call.bar, vec![])
        );
        let callee = fresh.callable(&mut call.db).unwrap();
        assert_eq!(callee.identity(), call.definition);
        assert_eq!(callee.body().unwrap().key(), new_body);
        stale(old.revalidate_namespace(&call.db));
        stale(
            call.before
                .require_current(&call.db, &call.index, &call.site),
        );
        ready(lease.require_current(&mut call.db));
        only_selected_entry_was_prepared(&call);
    }
}

#[test]
fn renewal_quota_wrong_pc_and_epoch_failure_leave_installed_table_unchanged() {
    let mut call = actual(2024);
    // Meter the genuine ordinary recipe before publication. Reusing its memo
    // in a second local table does not fabricate or execute an instruction.
    let mut meter =
        InstalledRecipes::new(call.db.source_revision(), INSTALLED_LRIR_BUDGET).unwrap();
    ready(meter.resolve(&mut call.db, call.site.clone()));
    let limit = meter.metadata_bytes();
    let mut table = InstalledRecipes::new(call.db.source_revision(), limit).unwrap();
    let id = ready(table.resolve(&mut call.db, call.site.clone()));
    assert_eq!(table.metadata_bytes(), limit);
    let before = snapshot(&table);
    let old = table.get(id).unwrap().clone();
    let lease = ready(call.db.pin_guest_body(call.index.body()));
    let transition = publish(&mut call, &lease);
    let trace = call.db.trace.events.len();
    rejected(
        table.renew_source_call(
            &mut call.db,
            &transition,
            &call.before,
            &call.index,
            &call.other,
        ),
        "LRR-E-INTERNAL-SNAPSHOT",
        "call renewal has no current installed site",
    );
    assert_eq!(
        call.db.trace.events.len(),
        trace,
        "wrong PC must not query another call"
    );
    assert_eq!(snapshot(&table), before);
    rejected(
        table.renew_source_call(
            &mut call.db,
            &transition,
            &call.before,
            &call.index,
            &call.site,
        ),
        "LRR-E-RUNTIME-RESOURCE",
        "guest arena handle/memory reservation exhausted",
    );
    assert_eq!(snapshot(&table), before);
    assert_eq!(table.get(id).unwrap().recipe(), old.recipe());
    stale(table.get_current(&call.db, id));
    stale(
        call.before
            .require_current(&call.db, &call.index, &call.site),
    );
    only_selected_entry_was_prepared(&call);
    drop(lease);
    call.db.begin_expansion_epoch(2).unwrap();
    let trace = call.db.trace.events.len();
    rejected(
        table.renew_source_call(
            &mut call.db,
            &transition,
            &call.before,
            &call.index,
            &call.site,
        ),
        "LRR-E-INTERNAL-SNAPSHOT",
        "prepared operation input changed",
    );
    assert_eq!(
        call.db.trace.events.len(),
        trace,
        "revocation must not replan"
    );
    assert_eq!(snapshot(&table), before);
}
