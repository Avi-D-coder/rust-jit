//! Actual own-source clause bundles and explicit parent demand boundaries.
use super::{
    upstream_associated_predicates::parent,
    upstream_generated_projection_fields::{actual_rows, assert_cold, named},
    upstream_projection_types::fixture_world,
};
use crate::{
    db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    source::manifest::ItemKind,
    traits::{
        term::TyTerm,
        upstream::source_declaration_clauses::{
            CurrentSourceDeclarationClauseHierarchy, CurrentSourceDeclarationClauses,
            SourceDeclarationClausesKey, SourceDeclarationClausesPending,
            check_actual_parent_source_clauses,
        },
    },
};
use std::{collections::BTreeMap, sync::Arc};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual source clauses: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual source clause input: {input:?}"),
    }
}

#[test]
fn unchanged_streaming_gat_clauses_use_pinned_parent_instantiation_all_editions_and_origins() {
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/generic-associated-types/streaming_iterator.rs",
                "use std::fmt",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let nominal = named(
                &rows,
                root.crate_key,
                &[],
                "StreamEnumerate",
                ItemKind::Struct,
            );
            let mut found = Vec::new();
            for (&id, row) in rows.iter().filter(|(_, row)| {
                row.item.scope.crate_key == root.crate_key
                    && row.item.def_key.item_kind == ItemKind::TypeAlias
                    && row.item.def_key.declared_name == "Item"
            }) {
                let own = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::DeclarationSourcePredicates(Box::new(SourceDeclarationClausesKey {
                        inventory: seal.clone(),
                        definition: id,
                    })),
                ));
                let QueryValue::DeclarationSourcePredicates(plan) = own.value.as_ref() else {
                    unreachable!()
                };
                if matches!(plan.self_type(), Some(TyTerm::Nominal(id, _)) if *id == nominal) {
                    let actual_parent = parent(&rows, id);
                    assert_eq!(plan.parent_definition(), Some(actual_parent));
                    assert_eq!(row.generated, origin != 0);
                    found.push((id, actual_parent));
                }
            }
            assert_eq!(
                found.len(),
                1,
                "actual impl GAT source identity must be unique"
            );
            let (definition, expected_parent) = found[0];
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let key = SourceDeclarationClausesKey {
                inventory: seal.clone(),
                definition,
            };
            let query = QueryKey::SourceDeclarationClauseHierarchy(Box::new(key.clone()));
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let (current, parent_count, own_count) =
                check_actual_parent_source_clauses(&mut db, &key, expected_parent).unwrap_or_else(
                    |why| panic!("actual GAT hierarchy edition{edition},origin{origin}: {why:?}"),
                );
            assert_eq!((parent_count, own_count), (2, 1));
            let again = ready(db.fetch(&SyntaxProvider, query));
            assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
            assert_eq!(first.fingerprint, again.fingerprint);
            assert_cold(&db, before);
            current.require_current(&db).unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(current.require_current(&db).is_err());
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(current.require_current(&db).is_err());
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
            worlds += 1;
            eprintln!(
                "actual source clause parent instantiation edition{edition},origin{origin}: 2parent/1own, actual owner rebasing/spans, pinned methods, memo/cold/withdrawal; full queries/root/runtime held"
            );
        }
    }
    assert_eq!(worlds, 12);
}

#[test]
fn unchanged_hrtb_parent_is_unforced_for_own_bundle_and_pending_for_hierarchy() {
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/higher-ranked/trait-bounds/assoc-type-projection-method.rs",
                "trait Collection",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let collection = named(&rows, root.crate_key, &[], "Collection", ItemKind::Trait);
            let methods = rows
                .iter()
                .filter(|(_, row)| {
                    row.item.scope.crate_key == root.crate_key
                        && row.item.def_key.item_kind == ItemKind::Function
                        && row.item.def_key.declared_name == "my_iter"
                })
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            assert_eq!(methods.len(), 1);
            let definition = methods[0];
            assert_eq!(parent(&rows, definition), collection);
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let key = SourceDeclarationClausesKey {
                inventory: seal.clone(),
                definition,
            };
            let own_query = QueryKey::SourceDeclarationClauses(Box::new(key.clone()));
            let first = ready(db.fetch(&SyntaxProvider, own_query.clone()));
            let own = ready(CurrentSourceDeclarationClauses::capture(&mut db, &key));
            assert_eq!(own.plan().pending(), None);
            assert!(own.plan().source().recipes().unwrap().is_empty());
            assert_eq!(own.plan().source().parent_definition(), Some(collection));
            let again = ready(db.fetch(&SyntaxProvider, own_query));
            assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
            assert_eq!(first.fingerprint, again.fingerprint);
            assert!(db.trace.events[before..].iter().all(|event| !matches!(&event.key,
                QueryKey::DeclarationSourcePredicates(key) | QueryKey::SourceDeclarationClauses(key)
                    | QueryKey::SourceDeclarationClauseHierarchy(key) if key.definition == collection)),
                "own source bundle must not demand any parent requirement recipes");
            let hierarchy = ready(CurrentSourceDeclarationClauseHierarchy::capture(
                &mut db, &key,
            ));
            assert_eq!(
                hierarchy.plan().pending(),
                Some(SourceDeclarationClausesPending::Parent)
            );
            let parent = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceDeclarationClauses(Box::new(SourceDeclarationClausesKey {
                    inventory: seal.clone(),
                    definition: collection,
                })),
            ));
            let QueryValue::SourceDeclarationClauses(parent) = parent.value.as_ref() else {
                unreachable!()
            };
            assert!(matches!(
                parent.pending(),
                Some(SourceDeclarationClausesPending::Recipe(
                    crate::traits::upstream::source_predicates::SourcePredicatePending::Bound(
                        crate::traits::predicate::PredicatePending::HigherRanked
                    )
                ))
            ));
            assert_cold(&db, before);
            own.require_current(&db).unwrap();
            hierarchy.require_current(&db).unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(own.require_current(&db).is_err());
            assert!(hierarchy.require_current(&db).is_err());
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(own.require_current(&db).is_err());
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
            worlds += 1;
            eprintln!(
                "actual source own/hierarchy demand edition{edition},origin{origin}: own ready0, parent HRTB explicitly pending, no body/signature, memo/withdrawal; full queries/root/runtime held"
            );
        }
    }
    assert_eq!(worlds, 12);
}
