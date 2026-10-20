//! Actual own GAT arguments from unchanged upstream declarations; no root/main.
use super::upstream_generated_projection_fields::{actual_rows, assert_cold, associated, named};
use super::{configured_impl_catalog_world, temporary_source};
use crate::{
    db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider},
    source::manifest::ItemKind,
    traits::{
        term::ParamKey,
        upstream::{
            model::AdtKind,
            source_adt_fields::{
                AdtSourceFieldsKey, DeclaredProjectionFieldExpectation,
                ProjectionOwnArgumentExpectation, ProjectionSelfExpectation,
                check_actual_declared_projection_field,
            },
        },
    },
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual GAT source preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual GAT source input: {input:?}"),
    }
}

fn check_fixture(
    case: &str,
    first_item: &str,
    owner_name: &str,
    kind: ItemKind,
    trait_name: &str,
    item_name: &str,
    field_name: &str,
    parameter: u32,
    own_static_region: bool,
) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout")
        .join(case);
    let original = std::fs::read_to_string(&path).unwrap();
    let offset = original.find(first_item).unwrap();
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let temporary = if origin == 0 {
                None
            } else {
                let emitter = "macro_rules! emit {($($t:tt)*)=>{$($t)*}}";
                let call = if origin == 1 {
                    "emit!"
                } else {
                    "macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!"
                };
                Some(temporary_source(
                    &format!("gat-{owner_name}-{edition}-{origin}"),
                    &format!(
                        "{}{emitter}{call}{{{}}}",
                        &original[..offset],
                        &original[offset..]
                    ),
                ))
            };
            let (mut db, seal, root) =
                configured_impl_catalog_world(temporary.as_ref().unwrap_or(&path), edition)
                    .unwrap();
            let rows = actual_rows(&mut db, &seal);
            let owner = named(&rows, root.crate_key, &[], owner_name, kind);
            let parent = named(&rows, root.crate_key, &[], trait_name, ItemKind::Trait);
            let item = associated(&rows, parent, item_name);
            for identity in [owner, parent, item] {
                assert_eq!(rows[&identity].generated, origin != 0);
            }
            let own = if own_static_region {
                vec![ProjectionOwnArgumentExpectation::StaticRegion]
            } else {
                vec![ProjectionOwnArgumentExpectation::NominalParameters {
                    definition: owner,
                    parameters: (0..2)
                        .map(|ordinal| ParamKey {
                            definition: owner,
                            ordinal,
                        })
                        .collect(),
                }]
            };
            let expected = DeclaredProjectionFieldExpectation {
                kind: if kind == ItemKind::Enum {
                    AdtKind::Enum
                } else {
                    AdtKind::Struct
                },
                field_name,
                wrappers: &[],
                trait_definition: parent,
                associated_definition: item,
                self_type: ProjectionSelfExpectation::Parameter(ParamKey {
                    definition: owner,
                    ordinal: parameter,
                }),
                unsigned_trait_arguments: &[],
                own_arguments: &own,
                parent_count: 1,
            };
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let query = QueryKey::AdtSourceFields(Box::new(AdtSourceFieldsKey {
                inventory: seal.clone(),
                definition: owner,
            }));
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let (current, count) =
                check_actual_declared_projection_field(&mut db, &seal, owner, &expected)
                    .unwrap_or_else(|error| {
                        panic!("{case},edition{edition},origin{origin}: {error:?}")
                    });
            assert_eq!(count, 1);
            let again = ready(db.fetch(&SyntaxProvider, query));
            assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
            assert_eq!(first.fingerprint, again.fingerprint);
            current.require_current(&db).unwrap();
            assert_cold(&db, before);
            db.begin_expansion_epoch(2).unwrap();
            assert!(current.require_current(&db).is_err());
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(current.require_current(&db).is_err());
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
            worlds += 1;
            eprintln!(
                "actual GAT field {case},edition{edition},origin{origin}: actual alias/ownargument/parent, memo/cold/epoch; full fixture/root/runtime held"
            );
        }
    }
    assert_eq!(worlds, 12);
}

#[test]
fn unchanged_streaming_iterator_gat_retains_own_static_region_all_editions_and_origins() {
    check_fixture(
        "tests/ui/generic-associated-types/streaming_iterator.rs",
        "use std::fmt",
        "Foo",
        ItemKind::Struct,
        "StreamingIterator",
        "Item",
        "bar",
        0,
        true,
    );
}

#[test]
fn unchanged_recursive_gat_field_retains_own_nominal_type_all_editions_and_origins() {
    check_fixture(
        "tests/ui/generic-associated-types/issue-87750.rs",
        "trait PointerFamily",
        "Node",
        ItemKind::Enum,
        "PointerFamily",
        "Pointer",
        "0",
        1,
        false,
    );
}
