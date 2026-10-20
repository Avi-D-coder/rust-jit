//! Upstream source field edge cases through the pinned variance producer.
use super::upstream_generated_projection_fields::{actual_rows, assert_cold, named};
use super::{configured_impl_catalog_world, temporary_source};
use crate::{
    db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    source::manifest::ItemKind,
    traits::upstream::{
        source_definition_headers::DefinitionHeaderKey,
        source_variance_queries::{CurrentSourceVarianceQueries, VarianceQueryOutcome},
    },
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual upstream variance field: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual upstream variance input: {input:?}"),
    }
}

#[test]
fn unchanged_mutable_pointer_field_is_invariant_all_editions_and_origins() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/variance/variance-use-invariant-struct-1.rs");
    let original = std::fs::read_to_string(&path).unwrap();
    let offset = original.find("struct SomeStruct").unwrap();
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let temporary = if origin == 0 {
                None
            } else {
                let macro_source = if origin == 1 {
                    "macro_rules! emit {($($t:tt)*)=>{$($t)*}} emit!"
                } else {
                    "macro_rules! emit {($($t:tt)*)=>{$($t)*}} macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!"
                };
                Some(temporary_source(
                    &format!("upstream-invariant-{edition}-{origin}"),
                    &format!(
                        "{}{}{{{}}}",
                        &original[..offset],
                        macro_source,
                        &original[offset..]
                    ),
                ))
            };
            let (mut db, seal, root) =
                configured_impl_catalog_world(temporary.as_ref().unwrap_or(&path), edition)
                    .unwrap();
            let rows = actual_rows(&mut db, &seal);
            let definition = named(&rows, root.crate_key, &[], "SomeStruct", ItemKind::Struct);
            let key = DefinitionHeaderKey {
                inventory: seal,
                definition,
            };
            let query = QueryKey::SourceVarianceQueries(Box::new(key.clone()));
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let QueryValue::SourceVarianceQueries(plan) = first.value.as_ref() else {
                panic!("actual variance memo kind")
            };
            assert_eq!(
                plan.outcome(),
                Ok(VarianceQueryOutcome::Known(&[
                    rustc_type_ir::Variance::Invariant
                ]))
            );
            let current = ready(CurrentSourceVarianceQueries::capture(&mut db, &key));
            current.require_current(&db).unwrap();
            let again = ready(db.fetch(&SyntaxProvider, query));
            assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
            assert_eq!(first.fingerprint, again.fingerprint);
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
                "actual upstream invariant source field,edition{edition},origin{origin}; exact pinned result/cold/memo/epoch; compiler diagnostic/root/runtime unexecuted"
            );
        }
    }
    assert_eq!(worlds, 12);
}
