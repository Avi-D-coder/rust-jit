//! Real associated output-bound source recipes; no full compiler query/root.
use super::{
    upstream_generated_projection_fields::{actual_rows, assert_cold, associated, named},
    upstream_projection_types::fixture_world,
};
use crate::{
    db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    frontend::{
        member_index::associated_name_key,
        source_item_bounds::{
            CurrentSourceItemBounds, ItemBoundsDefaultDependency, SizednessRole,
            SourceItemBoundsKey,
        },
    },
    source::manifest::ItemKind,
    syntax::ast_adapter::range,
    traits::{
        predicate::{PredicateResult, SourcePredicateRecipe},
        term::{ParamKey, TyTerm},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasTypeBounds},
};
use std::sync::Arc;
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual output-bound source: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual output-bound source needs input: {input:?}"),
    }
}

#[test]
fn unchanged_associated_output_bound_uses_actual_projection_span_and_typed_default_holds() {
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/associated-types/associated-types-bound.rs",
                "pub trait ToI32",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let parent = named(&rows, root.crate_key, &[], "GetToI32", ItemKind::Trait);
            let bound_trait = named(&rows, root.crate_key, &[], "ToI32", ItemKind::Trait);
            let item = associated(&rows, parent, "R");
            assert_eq!(rows[&item].generated, origin != 0);
            let parsed = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::ParseFile(rows[&item].parse.clone()),
            ))
            .value;
            let QueryValue::ParseFile(parsed) = parsed.as_ref() else {
                panic!()
            };
            let alias = parsed
                .syntax_at(rows[&item].item.declaration_span, |node| {
                    ast::TypeAlias::can_cast(node.kind())
                })
                .unwrap()
                .and_then(ast::TypeAlias::cast)
                .expect("actual associated output AST");
            let bounds = alias
                .type_bound_list()
                .unwrap()
                .bounds()
                .collect::<Vec<_>>();
            assert_eq!(bounds.len(), 1);
            let expected_span = range(bounds[0].syntax().text_range());
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let key = SourceItemBoundsKey {
                inventory: seal,
                definition: item,
            };
            let query = QueryKey::SourceItemBounds(Box::new(key.clone()));
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let current = ready(CurrentSourceItemBounds::capture(&mut db, &key));
            let plan = current.plan();
            assert_eq!(plan.source_pending(), None);
            assert!(
                plan.source().recipes().unwrap().is_empty(),
                "written output bound must not become declaration own requirements"
            );
            let Some(TyTerm::Projection(projection)) = plan.projection_subject() else {
                panic!("output bound needs genuine source projection subject")
            };
            assert_eq!(projection.trait_ref.trait_key, parent);
            assert_eq!(
                projection.trait_ref.self_ty,
                TyTerm::Param(ParamKey {
                    definition: item,
                    ordinal: 0
                })
            );
            assert!(projection.trait_ref.args.is_empty() && projection.item_args.is_empty());
            assert_eq!(projection.item, associated_name_key("R"));
            assert_eq!(
                plan.output_site(),
                Some((
                    rows[&item].parse.content,
                    range(alias.syntax().text_range())
                ))
            );
            let recipes = plan.written_recipes().unwrap();
            assert_eq!(recipes.len(), 1);
            assert!(matches!(&recipes[0], SourcePredicateRecipe::Explicit {
                source, result: PredicateResult::Implemented(reference),
            } if *source == expected_span && reference.trait_key == bound_trait
                && Some(&reference.self_ty) == plan.projection_subject() && reference.args.is_empty()));
            assert!(
                plan.parent_candidates().unwrap().is_empty(),
                "this actual trait declares no parent where candidates"
            );
            assert!(
                plan.default_dependencies()
                    .contains(&ItemBoundsDefaultDependency::Configuration)
            );
            for role in [
                SizednessRole::Sized,
                SizednessRole::MetaSized,
                SizednessRole::PointeeSized,
            ] {
                assert_eq!(
                    plan.sizedness_identity(role).is_none(),
                    plan.default_dependencies()
                        .contains(&ItemBoundsDefaultDependency::LanguageIdentity(role)),
                    "missing source lang role must remain a typed dependency"
                );
            }
            let again = ready(db.fetch(&SyntaxProvider, query));
            assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
            assert_eq!(first.fingerprint, again.fingerprint);
            assert_cold(&db, before);
            current.require_current(&db).unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(current.require_current(&db).is_err());
            worlds += 1;
            eprintln!(
                "actual associated output recipe edition{edition},origin{origin}: real R/ToI32/projection/span, own0/output1, defaults explicit, memo/cold/epoch; full item queries/root/runtime held"
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
        }
    }
    assert_eq!(worlds, 12);
}
