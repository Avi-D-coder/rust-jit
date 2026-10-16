//! Authority/lexical-plan controls only; no guest or upstream main is executed.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    frontend::{local_scopes::LocalLookup, source_macros::PublishedMacroIndex},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("owning context: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("owning context suspended: {input:?}"),
    }
}

#[test]
fn actual_body_context_retains_full_local_facts_and_producer_frontier() {
    for edition in [2015, 2018, 2021, 2024] {
        for pending_producer in [false, true] {
            let text = format!(
                "struct Point{{x:isize}}fn coordinate(p:&Point){{type Local=i32;use self::Point as Alias;let mut value=1;value;macro_rules! pending{{()=>{{}}}}{}value;}}fn cold(){{unknown();}}",
                if pending_producer { "pending!();" } else { "" }
            );
            let unit = Digest([231; 32]);
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let cfg = CfgSet::guest_default();
            let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
            let path = SourcePath::new("owning-context.rs").unwrap();
            let selection = SourceSelection {
                path: path.clone(),
                outline: OutlineKey {
                    parse: ParseKey {
                        content: blob.content(),
                        profile: profile.clone(),
                    },
                    crate_key: unit,
                    module_path: vec![],
                    cfg: cfg.fingerprint(),
                },
            };
            let inputs = PreparedSourceInputs::refresh(
                None,
                vec![SourceUnitInput {
                    path,
                    crate_key: unit,
                    module_path: vec![],
                    profile,
                    cfg,
                    blob,
                }],
                Default::default(),
            )
            .unwrap();
            let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
                panic!()
            };
            let mut db = Database::default();
            inputs.publish(&mut db).unwrap();
            let item = view
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "coordinate")
                .unwrap();
            let body = item.body.as_ref().unwrap();
            let index = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
            let QueryValue::BodyStructure(index) = index.value.as_ref() else {
                panic!()
            };
            let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
            let admitted = Arc::new(ready(capture(&mut db, &[&view], index, &pc)));
            let context = ready(Arc::clone(&admitted).prepare(&mut db));
            assert!(Arc::ptr_eq(admitted.parsed(), context.source().parsed()));
            assert!(Arc::ptr_eq(admitted.index(), context.source().index()));
            assert!(
                context
                    .facts()
                    .items
                    .iter()
                    .any(|row| row.item.def_key.declared_name == "Local")
            );
            assert_eq!(context.facts().imports.len(), 1);
            assert_eq!(
                context.facts().invocations.len(),
                usize::from(pending_producer)
            );
            assert!(context.facts().events.len() >= 1 + usize::from(pending_producer));
            assert!(
                context.exclusive_metadata_bytes() >= context.facts().metadata_bytes().unwrap()
            );
            let (root, facts) = crate::syntax::floating_fixture_hygiene::root(unit, edition);
            let published = PublishedMacroIndex::from_view(view.share(), root, facts).unwrap();
            let entered = published.enter_prepared_body(Arc::clone(&context)).unwrap();
            let locals = entered
                .local_index_with_pattern_index(
                    view.selection(),
                    body,
                    &Default::default(),
                    &[],
                    index,
                )
                .unwrap();
            let value = context
                .facts()
                .bindings
                .iter()
                .find(|binding| binding.name == "value")
                .unwrap();
            let actual_context = locals.context_at(value.name_range).unwrap();
            let names =
                crate::frontend::names::NameNamespaces::from_outlines(&[view.outline()]).unwrap();
            // The declaration visibility boundary is also the next item start in
            // compact source. Query an actual entered expression, never that opaque item.
            let ordinary_at = text.find("value;macro_rules!").unwrap() as u64;
            let pending = context
                .facts()
                .events
                .iter()
                .find_map(|event| match event.kind {
                    crate::syntax::entered_macros::BodyMacroEventKind::Producer {
                        identity,
                        ..
                    } => Some(identity),
                    _ => None,
                });
            assert_eq!(pending.is_some(), pending_producer);
            for name in ["Local", "Alias"] {
                let actual = locals
                    .lookup_ordinary(
                        crate::frontend::source_locals::OrdinaryNameSite {
                            source: view.selection(),
                            body: &body.key(),
                            at: ordinary_at,
                            name,
                            context: actual_context,
                            namespace: crate::source::manifest::Namespace::Type,
                        },
                        &names,
                        None,
                    )
                    .unwrap();
                match pending {
                    Some(expected) => assert_eq!(
                        actual,
                        crate::frontend::names::NameLookup::NeedInput(expected),
                        "unordered Type items require the exact outstanding producer"
                    ),
                    None => assert!(
                        matches!(actual, crate::frontend::names::NameLookup::Defined(_)),
                        "completed local/import Type lookup failed: {actual:?}"
                    ),
                }
            }
            assert!(matches!(
                locals
                    .lookup(
                        view.selection(),
                        &body.key(),
                        ordinary_at,
                        "value",
                        actual_context
                    )
                    .unwrap(),
                LocalLookup::Binding(_)
            ));
            let after = locals
                .lookup(
                    view.selection(),
                    &body.key(),
                    u64::from(body.range.end - 2),
                    "value",
                    actual_context,
                )
                .unwrap();
            match pending {
                Some(expected) => assert_eq!(after, LocalLookup::NeedInput(expected)),
                None => assert!(matches!(after, LocalLookup::Binding(_))),
            }
            // Owned preparation retains the actual complete entered environment.
            // These remain metadata observations with explicit authority debts.
            use crate::frontend::source_locals::PreparedSourceLocalPlan;
            let owned = ready(PreparedSourceLocalPlan::prepare(
                &mut db,
                &published,
                Arc::clone(&context),
            ));
            let independently_prepared = ready(PreparedSourceLocalPlan::prepare(
                &mut db,
                &published,
                Arc::clone(&context),
            ));
            assert!(owned.same_environment(&owned));
            assert!(!owned.same_environment(&independently_prepared));
            assert_eq!(
                owned.source_identity(),
                independently_prepared.source_identity()
            );
            assert!(Arc::ptr_eq(owned.context(), &context));
            assert!(owned.exclusive_metadata_bytes() > 0);
            assert!(owned.tables_root().metadata_bytes().unwrap() > 0);
            assert!(Arc::ptr_eq(
                owned.tables_root().hygiene_root(),
                owned.allocation_roots().hygiene()
            ));
            let source_index = Arc::clone(owned.allocation_roots().source_index());
            let runtime_outers = Arc::clone(owned.allocation_roots().runtime_outers());
            drop(locals);
            drop(entered);
            drop(published);
            let body_key = body.key();
            let site = || crate::frontend::source_locals::OrdinaryNameSite {
                source: view.selection(),
                body: &body_key,
                at: ordinary_at,
                name: "Local",
                context: actual_context,
                namespace: crate::source::manifest::Namespace::Type,
            };
            for _ in 0..2 {
                let observation = ready(owned.inspect_prefix_metadata(&mut db, site(), &names));
                use crate::frontend::source_locals::owned_plan::LocalPlanAuthorityDemand;
                assert!(
                    observation
                        .demands()
                        .contains(&LocalPlanAuthorityDemand::CurrentEnvironmentPublication)
                );
                assert!(
                    observation
                        .demands()
                        .contains(&LocalPlanAuthorityDemand::DeclarationNameContext)
                );
                assert!(
                    observation
                        .demands()
                        .contains(&LocalPlanAuthorityDemand::ExactEnteredOccurrence)
                );
                assert!(
                    observation
                        .demands()
                        .contains(&LocalPlanAuthorityDemand::ModuleNamespacePublication)
                );
                match pending {
                    Some(expected) => assert_eq!(
                        observation.lookup(),
                        Some(&crate::frontend::names::NameLookup::NeedInput(expected))
                    ),
                    None => assert!(matches!(
                        observation.lookup(),
                        Some(crate::frontend::names::NameLookup::Defined(_))
                    )),
                }
                assert!(Arc::ptr_eq(
                    &source_index,
                    owned.allocation_roots().source_index()
                ));
                assert!(Arc::ptr_eq(
                    &runtime_outers,
                    owned.allocation_roots().runtime_outers()
                ));
            }
            let mut foreign = pc.clone();
            foreign.body.exact_body_bytes = Digest([232; 32]);
            assert!(matches!(
                capture(&mut db, &[&view], index, &foreign),
                QueryPoll::Ready(Err(_))
            ));
            ready(context.require_current(&mut db));
            let mut header = admitted.header().clone();
            header.profile.edition = if edition == 2024 { 2021 } else { 2024 };
            let mut replacement = view.prepared_inputs().clone();
            replacement.insert(
                InputKey::DefinitionHeader(body.def_key),
                InputValue::DefinitionHeader(header),
            );
            db.publish_inputs(replacement).unwrap();
            assert!(matches!(
                context.require_current(&mut db),
                QueryPoll::Ready(Err(_))
            ));
            assert!(matches!(
                owned.inspect_prefix_metadata(&mut db, site(), &names),
                QueryPoll::Ready(Err(_))
            ));
        }
    }
}
