//! Actual associated explicit-own recipe controls; no solver root or guest run.
use super::upstream_generated_projection_fields::{SourceRow, actual_rows, assert_cold, named};
use super::{configured_impl_catalog_world, temporary_source};
use crate::{
    core::digest::{Digest, StableEncode},
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::declaration_source_predicates::{
        DeclarationSourcePredicatesKey, PreparedDeclarationSourcePredicates,
    },
    ir::op::PrimitiveType,
    source::manifest::ItemKind,
    traits::{
        predicate::{PredicateResult, SourcePredicateRecipe},
        term::{GenericArg, ParamKey, RegionTerm, TyTerm},
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual associated recipe source: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual associated recipe input: {input:?}"),
    }
}
pub(super) fn parent(rows: &BTreeMap<Digest, SourceRow>, item: Digest) -> Digest {
    let child = &rows[&item];
    let found = rows
        .iter()
        .filter(|(_, owner)| {
            if owner.parse != child.parse
                || !matches!(
                    owner.item.def_key.item_kind,
                    ItemKind::Trait | ItemKind::Impl
                )
            {
                return false;
            }
            let mut scope = owner.item.scope.clone();
            scope.owner = Some(if owner.item.def_key.item_kind == ItemKind::Impl {
                owner
                    .item
                    .def_key
                    .enclosing_impl
                    .expect("actual impl member scope")
            } else {
                owner.item.def_key.fingerprint()
            });
            scope == child.item.scope
                && owner.item.declaration_span.start <= child.item.declaration_span.start
                && child.item.declaration_span.end <= owner.item.declaration_span.end
        })
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    assert_eq!(
        found.len(),
        1,
        "actual associated declaration must have one authentic parent attachment"
    );
    found[0]
}
fn plan(value: &Arc<QueryValue>) -> &PreparedDeclarationSourcePredicates {
    let QueryValue::DeclarationSourcePredicates(plan) = value.as_ref() else {
        panic!("actual recipe query differs")
    };
    plan
}
fn capture(
    db: &mut Database,
    key: DeclarationSourcePredicatesKey,
) -> (Arc<QueryValue>, PreparedInputGuard) {
    let query = QueryKey::DeclarationSourcePredicates(Box::new(key));
    let first = ready(db.fetch(&SyntaxProvider, query.clone()));
    let again = ready(db.fetch(&SyntaxProvider, query.clone()));
    assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
    assert_eq!(first.fingerprint, again.fingerprint);
    let guard = ready(db.prepare_input_guard(&SyntaxProvider, &query, first.fingerprint));
    guard.require_current(db).unwrap();
    (first.value, guard)
}
fn own_ranges(plan: &PreparedDeclarationSourcePredicates) {
    let (_, _, context, parsed) = plan
        .declaration()
        .source_parts()
        .expect("actual recipe declaration snapshot");
    let layout = plan.declaration().layout().expect("actual recipe layout");
    for parameter in plan.declaration().parameters() {
        let key = ParamKey {
            definition: plan.key().definition,
            ordinal: parameter.ordinal,
        };
        if parameter.ordinal < layout.own {
            let range = plan
                .parameter_range(key)
                .expect("actual own generic source range");
            assert!(context.range.start <= range.start && range.end <= context.range.end);
            assert!(
                parsed
                    .syntax_at(range, |node| ast::GenericParam::can_cast(node.kind()))
                    .unwrap()
                    .is_some()
            );
        } else {
            assert!(
                plan.parameter_range(key).is_none(),
                "inherited/default Self spans are not child own parameter spans"
            );
        }
    }
    for recipe in plan
        .recipes()
        .expect("actual explicit-own recipe preparation remains pending")
    {
        match recipe {
            SourcePredicateRecipe::Explicit { source, result } => {
                assert!(context.range.start <= source.start && source.end <= context.range.end);
                assert!(
                    !matches!(result, PredicateResult::Pending(_)),
                    "actual supported own bound remains pending: {result:?}"
                );
            }
            SourcePredicateRecipe::ResolvedSizedDefault(reference) => {
                let TyTerm::Param(key) = &reference.self_ty else {
                    panic!("actual Sized default is not an own source parameter")
                };
                assert!(plan.parameter_range(*key).is_some());
            }
            SourcePredicateRecipe::ImplicitSizedDefault(_) => {
                panic!("actual Sized lang identity missing")
            }
        }
    }
}

#[test]
fn unchanged_associated_explicit_own_recipes_keep_real_parent_self_and_sized_ownership() {
    let originals =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut worlds = 0;
    let mut associated = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, first, aliases) in [
            (
                "tests/ui/associated-types/assoc-type-in-struct-constructor.rs",
                "trait Tr",
                2,
            ),
            (
                "tests/ui/associated-types/associated-types-basic.rs",
                "trait Foo",
                2,
            ),
            (
                "tests/ui/generic-associated-types/streaming_iterator.rs",
                "use std::fmt::Display;",
                3,
            ),
            (
                "tests/ui/generic-associated-types/issue-87750.rs",
                "trait PointerFamily",
                2,
            ),
        ] {
            let original_path = originals.join(case);
            let original = std::fs::read_to_string(&original_path).unwrap();
            let offset = original.find(first).expect("unchanged first source item");
            for origin in 0..3 {
                let temporary = (origin != 0).then(|| {
                    let emitter = "macro_rules! emit {($($t:tt)*)=>{$($t)*}}";
                    let call = if origin == 1 {
                        "emit!"
                    } else {
                        "macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!"
                    };
                    temporary_source(
                        &format!("associated-recipes-{aliases}-{first}-{edition}-{origin}"),
                        &format!(
                            "{}{}{}{{{}}}",
                            &original[..offset],
                            emitter,
                            call,
                            &original[offset..]
                        ),
                    )
                });
                let (mut db, seal, root) = configured_impl_catalog_world(
                    temporary.as_ref().unwrap_or(&original_path),
                    edition,
                )
                .unwrap_or_else(|error| {
                    panic!("{case},edition{edition},origin{origin}: {error:?}")
                });
                let rows = actual_rows(&mut db, &seal);
                let items = rows
                    .iter()
                    .filter(|(_, row)| {
                        row.item.scope.crate_key == root.crate_key
                            && row.item.def_key.item_kind == ItemKind::TypeAlias
                    })
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>();
                assert_eq!(items.len(), aliases, "actual associated alias coverage");
                db.trace.detailed = true;
                let before = db.trace.events.len();
                let mut guards = Vec::new();
                for item in items {
                    let owner = parent(&rows, item);
                    assert_eq!(rows[&item].generated, origin != 0);
                    assert_eq!(rows[&owner].generated, origin != 0);
                    let (value, guard) = capture(
                        &mut db,
                        DeclarationSourcePredicatesKey {
                            inventory: seal.clone(),
                            definition: item,
                        },
                    );
                    let prepared = plan(&value);
                    assert_eq!(
                        prepared.pending(),
                        None,
                        "{case},edition{edition},origin{origin},item{item:?}"
                    );
                    assert_eq!(prepared.parent_definition(), Some(owner));
                    assert_eq!(prepared.parent_header().unwrap().key().definition, owner);
                    assert!(
                        prepared.source_header_root().is_some()
                            && prepared.parent_header_root().is_some()
                    );
                    let child_layout = prepared.declaration().layout().unwrap();
                    let expected_self = if rows[&owner].item.def_key.item_kind == ItemKind::Trait {
                        TyTerm::Param(ParamKey {
                            definition: item,
                            ordinal: child_layout.own + child_layout.inherited,
                        })
                    } else {
                        let parsed = ready(db.fetch(
                            &SyntaxProvider,
                            QueryKey::ParseFile(rows[&owner].parse.clone()),
                        ))
                        .value;
                        let QueryValue::ParseFile(parsed) = parsed.as_ref() else {
                            panic!()
                        };
                        let node = parsed
                            .syntax_at(rows[&owner].item.declaration_span, |node| {
                                ast::Impl::can_cast(node.kind())
                            })
                            .unwrap()
                            .and_then(ast::Impl::cast)
                            .expect("actual impl parent AST");
                        let ast::Type::PathType(ty) = node.self_ty().unwrap() else {
                            panic!("actual witness self shape")
                        };
                        let name = ty
                            .path()
                            .unwrap()
                            .segment()
                            .unwrap()
                            .name_ref()
                            .unwrap()
                            .text()
                            .to_string();
                        match name.as_str() {
                            "u8" => TyTerm::Primitive(PrimitiveType::U8),
                            "i32" => TyTerm::Primitive(PrimitiveType::I32),
                            "I" => TyTerm::Param(ParamKey {
                                definition: item,
                                ordinal: child_layout.own,
                            }),
                            "StreamEnumerate" => TyTerm::Nominal(
                                named(
                                    &rows,
                                    root.crate_key,
                                    &[],
                                    "StreamEnumerate",
                                    ItemKind::Struct,
                                ),
                                vec![GenericArg::Type(TyTerm::Param(ParamKey {
                                    definition: item,
                                    ordinal: child_layout.own,
                                }))],
                            ),
                            "RcFamily" => TyTerm::Nominal(
                                named(&rows, root.crate_key, &[], "RcFamily", ItemKind::Struct),
                                vec![],
                            ),
                            other => panic!("unexpected actual fixture parent Self {other}"),
                        }
                    };
                    assert_eq!(prepared.self_type(), Some(&expected_self));
                    own_ranges(prepared);
                    let recipes = prepared.recipes().unwrap();
                    if case.contains("streaming_iterator") {
                        assert_eq!(
                            recipes.len(),
                            1,
                            "parent trait/impl clauses and Sized defaults must not be merged into own recipes"
                        );
                        assert!(
                            matches!(&recipes[0], SourcePredicateRecipe::Explicit { result: PredicateResult::TypeOutlives { ty, region: RegionTerm::Param(region) }, .. }
                            if *ty == expected_self && *region == (ParamKey { definition: item, ordinal: 0 }))
                        );
                        if rows[&owner].item.def_key.item_kind == ItemKind::Impl {
                            let (parent_value, parent_guard) = capture(
                                &mut db,
                                DeclarationSourcePredicatesKey {
                                    inventory: seal.clone(),
                                    definition: owner,
                                },
                            );
                            assert_eq!(
                                plan(&parent_value).recipes().unwrap().len(),
                                2,
                                "real inherited impl I:Sized+Iterator/StreamingIterator stay on parent"
                            );
                            guards.push(parent_guard);
                        }
                    } else if case.contains("issue-87750") {
                        assert_eq!(recipes.len(), 1);
                        assert!(
                            matches!(&recipes[0], SourcePredicateRecipe::ResolvedSizedDefault(reference)
                            if reference.self_ty == TyTerm::Param(ParamKey { definition: item, ordinal: 0 }))
                        );
                    } else {
                        assert!(
                            recipes.is_empty(),
                            "associated output type implicit Sized is item-bound metadata, not own source-generic default"
                        );
                    }
                    guards.push(guard);
                    associated += 1;
                }
                if case.contains("assoc-type-in-struct-constructor") {
                    let tr = named(&rows, root.crate_key, &[], "Tr", ItemKind::Trait);
                    for (name, kind) in [("S", ItemKind::Struct), ("foo", ItemKind::Function)] {
                        let owner = named(&rows, root.crate_key, &[], name, kind);
                        let (value, guard) = capture(
                            &mut db,
                            DeclarationSourcePredicatesKey {
                                inventory: seal.clone(),
                                definition: owner,
                            },
                        );
                        let prepared = plan(&value);
                        own_ranges(prepared);
                        assert_eq!(prepared.parent_definition(), None);
                        assert_eq!(prepared.recipes().unwrap().len(), 2);
                        assert!(prepared.recipes().unwrap().iter().any(|recipe| matches!(recipe,
                            SourcePredicateRecipe::Explicit { result: PredicateResult::Implemented(reference), .. }
                            if reference.trait_key == tr && reference.self_ty == TyTerm::Param(ParamKey { definition: owner, ordinal: 0 }))));
                        guards.push(guard);
                    }
                }
                assert_cold(&db, before);
                assert!(
                    guards
                        .iter()
                        .all(|guard| guard.require_current(&db).is_ok())
                );
                db.begin_expansion_epoch(2).unwrap();
                assert!(
                    guards
                        .iter()
                        .all(|guard| guard.require_current(&db).is_err())
                );
                worlds += 1;
                eprintln!(
                    "actual explicit-own associated recipes {case},edition{edition},origin{origin}: parent/Self/span/own-vs-inherited/memo/cold/revocation; full clauses/item bounds/root/main held"
                );
                if let Some(path) = temporary {
                    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
                }
            }
        }
    }
    assert_eq!((worlds, associated), (48, 108));
}

#[test]
fn unchanged_associated_own_recipes_leave_parent_hrtb_unforced() {
    use crate::traits::predicate::PredicatePending;
    let original_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/higher-ranked/trait-bounds/assoc-type-projection-method.rs");
    let original = std::fs::read_to_string(&original_path).unwrap();
    let offset = original
        .find("trait Collection")
        .expect("unchanged first item");
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let temporary = (origin != 0).then(|| {
                let emitter = "macro_rules! emit {($($t:tt)*)=>{$($t)*}}";
                let call = if origin == 1 {
                    "emit!"
                } else {
                    "macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!"
                };
                temporary_source(
                    &format!("associated-parent-hrtb-{edition}-{origin}"),
                    &format!(
                        "{}{}{}{{{}}}",
                        &original[..offset],
                        emitter,
                        call,
                        &original[offset..]
                    ),
                )
            });
            let (mut db, seal, root) = configured_impl_catalog_world(
                temporary.as_ref().unwrap_or(&original_path),
                edition,
            )
            .unwrap_or_else(|error| {
                panic!("actual HRTB owner world edition{edition},origin{origin}: {error:?}")
            });
            let rows = actual_rows(&mut db, &seal);
            let collection = named(&rows, root.crate_key, &[], "Collection", ItemKind::Trait);
            let method = named(&rows, root.crate_key, &[], "my_iter", ItemKind::Function);
            assert_eq!(parent(&rows, method), collection);
            assert_eq!(rows[&method].generated, origin != 0);
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let (method_value, method_guard) = capture(
                &mut db,
                DeclarationSourcePredicatesKey {
                    inventory: seal.clone(),
                    definition: method,
                },
            );
            let own = plan(&method_value);
            assert_eq!(own.pending(), None);
            assert_eq!(own.parent_definition(), Some(collection));
            assert!(
                own.recipes().unwrap().is_empty(),
                "parent HRTB must not leak into method explicit-own transport"
            );
            let child_self = ParamKey {
                definition: method,
                ordinal: 0,
            };
            assert_eq!(own.self_type(), Some(&TyTerm::Param(child_self)));
            assert_eq!(
                own.rebase_parent_parameter(ParamKey {
                    definition: collection,
                    ordinal: 0
                }),
                Some(child_self)
            );
            assert!(own.parameter_range(child_self).is_none());
            assert!(
                db.trace.events[before..]
                    .iter()
                    .all(|event| !matches!(&event.key,
                QueryKey::DeclarationSourcePredicates(key) if key.definition == collection)),
                "child own demand reads parent source/generic metadata, never parent predicates"
            );
            own_ranges(own);
            let (parent_value, parent_guard) = capture(
                &mut db,
                DeclarationSourcePredicatesKey {
                    inventory: seal,
                    definition: collection,
                },
            );
            let parent_recipes = plan(&parent_value).recipes().unwrap();
            assert!(
                parent_recipes.iter().any(|recipe| matches!(
                    recipe,
                    SourcePredicateRecipe::Explicit {
                        result: PredicateResult::Pending(PredicatePending::HigherRanked),
                        ..
                    }
                )),
                "the actual parent HRTB remains an explicit pending recipe"
            );
            assert_cold(&db, before);
            method_guard.require_current(&db).unwrap();
            parent_guard.require_current(&db).unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(method_guard.require_current(&db).is_err());
            assert!(parent_guard.require_current(&db).is_err());
            worlds += 1;
            eprintln!(
                "actual associated own-versus-parent demand edition{edition},origin{origin}: method own ready/empty, parent HRTB pending, memo/cold/epoch; root/main held"
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
        }
    }
    assert_eq!(worlds, 12);
}
