//! Target-only derived publication authority on unchanged official source.
//! This does not execute the fixture or count a runtime/whole-fixture pass.
use super::*;

#[path = "installed_call_renewal_tests.rs"]
mod installed_call_renewal_tests;
use crate::{
    db::retained_syntax::RetainedSyntaxInput,
    frontend::entered_call_goals::EnteredCallGoal,
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        manifest::BodyKey,
        snapshot::SourceBlob,
    },
};
const NESTED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);

fn actual_world(
    edition: u16,
) -> (
    Database,
    PreparedSourceView,
    SourceDiscovery,
    Arc<PreparedSourceLocalPlan>,
) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(NESTED, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"actual nested target-only retained publication"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"actual nested target-only compiler"),
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
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => {
                panic!(
                    "actual nested declaration discovery incomplete: {diagnostic:?}; {blocked:?}"
                )
            }
            _ => panic!("actual nested declaration discovery requires an unsupported stage"),
        }
    }
    discovery.publish(&mut db).unwrap();
    let CallSelection::Source(main) = ready(discovery.entry_function(&mut db, "main")) else {
        panic!("actual main is not a source function")
    };
    let main =
        crate::cli::retained_entry::prepare(&mut db, &discovery, &source, "main", main).unwrap();
    let parent =
        ready(db.prepare_source_local_plan(&discovery, &[&source], &main.body().unwrap().key()));
    (db, source, discovery, parent)
}
fn actual_call(db: &mut Database, site: &PcKey) -> Arc<QueryValue> {
    let value = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredCall(site.clone())));
    assert!(matches!(value.value.as_ref(), QueryValue::EnteredCall(_)));
    value.value
}
fn goal(value: &QueryValue) -> &EnteredCallGoal {
    let QueryValue::EnteredCall(call) = value else {
        panic!("actual Q44 variant differs")
    };
    call
}
fn actual_locator(db: &mut Database, definition: Digest) -> BodyKey {
    let input = db.input(&InputKey::DefinitionBody(definition)).unwrap();
    let InputValue::DefinitionBody(body) = input.as_ref() else {
        panic!("actual body locator differs")
    };
    body.key()
}

#[test]
fn target_only_retained_publication_revokes_old_calls_preserves_live_caller_and_keeps_other_bodies_cold()
 {
    for edition in [2015, 2018, 2021, 2024] {
        for revocation in 0..3 {
            let (mut db, source, discovery, parent) = actual_world(edition);
            let caller = parent.context().source().index().body().clone();
            let index = parent.context().source().index();
            let mut target = source
                .outline()
                .items
                .iter()
                .filter(|item| item.def_key.declared_name == "static_assert_exists");
            let target = target.next().unwrap();
            let definition = target.def_key.fingerprint();
            let written_definition = target.def_key.clone();
            let sites = calls(&parent);
            assert_eq!(sites.len(), 3);
            let site = sites
                .iter()
                .map(|id| index.pc(*id, crate::ir::pc::Phase::Entry).unwrap())
                .find(|site| {
                    goal(&actual_call(&mut db, site))
                        .signature()
                        .unwrap()
                        .definition
                        == definition
                })
                .unwrap();
            let other_site = sites
                .iter()
                .filter(|id| id.0 != site.node)
                .map(|id| index.pc(*id, crate::ir::pc::Phase::Entry).unwrap())
                .next()
                .unwrap();
            let old_call = actual_call(&mut db, &site);
            let old_proof = admit(&mut db, &parent, NodeId(site.node));
            assert_eq!(old_proof.canonical.definition(), &written_definition);
            let old_body = actual_locator(&mut db, definition);
            let old_callee = goal(&old_call).module_callable(&mut db).unwrap();
            assert_eq!(old_callee.body().unwrap().key(), old_body);
            let value = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::PreparedExplicitTypeArguments(site.clone()),
            ));
            let QueryValue::PreparedExplicitTypeArguments(old_explicit) = value.value.as_ref()
            else {
                panic!("actual Q47 variant differs")
            };
            let old_explicit = old_explicit.as_ref().clone();
            let mut bars = parent
                .context()
                .facts()
                .items
                .iter()
                .filter(|row| row.item.def_key.declared_name == "Bar");
            let bar = bars.next().unwrap().item.def_key.fingerprint();
            assert!(bars.next().is_none());
            assert_eq!(old_explicit.rows().len(), 1);
            assert_eq!(
                old_explicit.rows()[0].source().source(),
                &crate::traits::term::TyTerm::Nominal(bar, vec![])
            );
            old_explicit.require_current(&db, index, &site).unwrap();
            let caller_rows = [
                InputKey::DefinitionSource(caller.definition),
                InputKey::DefinitionBody(caller.definition),
                InputKey::DefinitionHeader(caller.definition),
                InputKey::DeclarationContext(caller.definition),
            ]
            .map(|key| {
                let value = db.input(&key).unwrap();
                (key, value)
            });
            let lease = ready(db.pin_guest_body(&caller));
            assert_eq!(lease.body(), &caller);
            assert_eq!(
                db.require_body_unowned(caller.definition).unwrap_err().code,
                "LRR-E-BODY-ACTIVE"
            );

            // The same production assembly/admission/publication services are used;
            // only this genuine original target's definition rows are replaced.
            let prepared = discovery
                .prepare_body_syntax(source.selection(), definition)
                .unwrap();
            assert!(prepared.permits_body(definition));
            assert!(
                !prepared.permits_body(caller.definition),
                "target readiness remains genuinely body scoped"
            );
            let input = ready(RetainedSyntaxInput::prepare(
                &mut db,
                prepared,
                discovery.key().clone(),
            ));
            ready(db.publish_retained_syntax(Arc::clone(&input)));
            match db.publish_retained_call_body(
                &old_explicit,
                index,
                &other_site,
                &lease,
                Arc::clone(&input),
            ) {
                QueryPoll::Ready(Err(error)) => {
                    assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
                    assert_eq!(error.message, "explicit Type plan caller use differs");
                }
                _ => panic!("wrong actual callsite acquired target transition authority"),
            }
            assert_eq!(actual_locator(&mut db, definition), old_body);
            let main_input = db
                .input(&InputKey::RetainedSyntax(
                    parent.context().source().parsed().key().content,
                ))
                .unwrap();
            let InputValue::RetainedSyntax(main_input) = main_input.as_ref() else {
                panic!("genuine main retained input absent")
            };
            match db.publish_retained_call_body(
                &old_explicit,
                index,
                &site,
                &lease,
                Arc::clone(main_input),
            ) {
                QueryPoll::Ready(Err(error)) => {
                    assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
                    assert_eq!(
                        error.message,
                        "retained syntax readiness belongs to another original body"
                    );
                }
                _ => panic!("main-only retained permit authorized another target body"),
            }
            assert_eq!(actual_locator(&mut db, definition), old_body);
            let transition =
                ready(db.publish_retained_call_body(&old_explicit, index, &site, &lease, input));
            assert!(transition.metadata_bytes() > 0);
            transition.require_current(&db).unwrap();
            let new_body = transition.body();
            assert_eq!(new_body.definition, definition);
            assert_ne!(new_body, old_body);
            assert_eq!(actual_locator(&mut db, definition), new_body);
            let events = db.trace.events.len();
            let error = old_proof.canonical.revalidate(&db).unwrap_err();
            assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
            assert_eq!(error.message, "selected module declaration row changed");
            for error in [
                goal(&old_call).require_current(&db).unwrap_err(),
                old_explicit.require_current(&db, index, &site).unwrap_err(),
                old_callee.revalidate(&mut db).unwrap_err(),
            ] {
                assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
                assert_eq!(error.message, "prepared operation input changed");
            }
            for (key, before) in &caller_rows {
                let after = db.input(key).unwrap();
                assert!(Arc::ptr_eq(before, &after));
                assert_eq!(before.fingerprint(), after.fingerprint());
            }
            ready(parent.require_current(&mut db));
            ready(lease.require_current(&mut db));
            // Existing source/lease currentness validates the actual seal
            // memo. Query trace events alone are not source replanning.
            let validation = &db.trace.events[events..];
            eprintln!(
                "caller currentness: {:?}",
                validation
                    .iter()
                    .map(|event| (event.key.kind(), event.action))
                    .collect::<Vec<_>>()
            );
            assert_eq!(db.trace.truncated, 0, "currentness trace must be complete");
            assert!(
                validation.iter().all(|event| {
                    matches!(event.action, Action::MemoHit | Action::ValidatedReuse)
                        && (event.key == QueryKey::DiscoverySeal(discovery.key().clone())
                            || event.key == QueryKey::Outline(source.selection().outline.clone())
                            || event.key
                                == QueryKey::ParseFile(source.selection().outline.parse.clone()))
                }),
                "caller currentness may only reuse its actual seal and captured source dependencies"
            );
            let fresh_call = actual_call(&mut db, &site);
            goal(&fresh_call).require_current(&db).unwrap();
            let fresh_callee = goal(&fresh_call).module_callable(&mut db).unwrap();
            assert_eq!(fresh_callee.definition(), &written_definition);
            assert_eq!(fresh_callee.identity(), definition);
            assert_eq!(fresh_callee.body().unwrap().key(), new_body);
            let value = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::PreparedExplicitTypeArguments(site.clone()),
            ));
            let QueryValue::PreparedExplicitTypeArguments(fresh_explicit) = value.value.as_ref()
            else {
                panic!("fresh actual Q47 variant differs")
            };
            fresh_explicit.require_current(&db, index, &site).unwrap();
            assert_eq!(fresh_explicit.rows().len(), 1);
            assert_eq!(
                fresh_explicit.rows()[0].source().source(),
                &crate::traits::term::TyTerm::Nominal(bar, vec![])
            );
            let fresh_explicit_callee = fresh_explicit.callable(&mut db).unwrap();
            assert_eq!(fresh_explicit_callee.identity(), definition);
            assert_eq!(fresh_explicit_callee.body().unwrap().key(), new_body);
            let value = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::PreparedExplicitTypeArguments(other_site.clone()),
            ));
            let QueryValue::PreparedExplicitTypeArguments(other_explicit) = value.value.as_ref()
            else {
                panic!("actual other Q47 absent")
            };
            let events = db.trace.events.len();
            transition
                .require_rebound(&mut db, &old_explicit, fresh_explicit, index, &site)
                .unwrap();
            for (fresh, pc) in [
                (other_explicit.as_ref(), &site),
                (fresh_explicit.as_ref(), &other_site),
            ] {
                let error = transition
                    .require_rebound(&mut db, &old_explicit, fresh, index, pc)
                    .unwrap_err();
                assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
                assert_eq!(
                    error.message,
                    "retained call renewal changed the selected contract"
                );
            }
            assert_eq!(db.trace.events.len(), events);
            assert!(
                old_explicit.require_current(&db, index, &site).is_err(),
                "transition must not refresh the old Q47 guard"
            );

            assert!(
                db.trace.events.iter().all(|event| match &event.key {
                    QueryKey::BodyStructure(body) => body == &caller,
                    QueryKey::Thunk(_) => false,
                    _ => true,
                }),
                "target publication/canonical selection must not prepare any child executable body"
            );
            drop(lease);
            db.require_body_unowned(caller.definition).unwrap();
            if revocation == 0 {
                db.begin_expansion_epoch(2).unwrap();
            } else {
                let mut cfg = source.captured_macro_cfg().unwrap().clone();
                let blob = if revocation == 1 {
                    cfg.atoms.insert("lrr_call_transition_cfg_change".into());
                    source.captured_macro_blob().unwrap().clone()
                } else {
                    SourceBlob::new(
                        format!("{NESTED}\n// actual source revision control\n").into_bytes(),
                    )
                    .unwrap()
                };
                let changed = PreparedSourceInputs::refresh(
                    None,
                    vec![SourceUnitInput {
                        path: source.selection().path.clone(),
                        crate_key: source.selection().outline.crate_key,
                        module_path: source.selection().outline.module_path.clone(),
                        profile: source.selection().outline.parse.profile.clone(),
                        cfg,
                        blob,
                    }],
                    Default::default(),
                )
                .unwrap();
                changed.publish(&mut db).unwrap();
            }
            let events = db.trace.events.len();
            let error = transition.require_current(&db).unwrap_err();
            assert_eq!(
                error.code, "LRR-E-INTERNAL-SNAPSHOT",
                "edition={edition} revocation={revocation}: {error:?}"
            );
            assert_eq!(
                error.message,
                if revocation == 0 {
                    "prepared operation input changed"
                } else {
                    "retained call target transition publication changed"
                }
            );
            let error = transition
                .require_rebound(&mut db, &old_explicit, fresh_explicit, index, &site)
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
            assert_eq!(
                db.trace.events.len(),
                events,
                "revocation must not prepare source or reacquire a callable"
            );
        }
    }
}
