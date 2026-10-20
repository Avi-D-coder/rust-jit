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
        upstream::{source_definition_semantics::{
            CurrentSourceDefinitionSemantics, DefinitionBooleanQuery as Boolean,
            DefinitionSemanticPending as SemanticPending, SourceSemantic,
            SourceDefinitionSemanticsKey, SourceAliasDomain,
        }, source_declaration_clauses::{
            CurrentSourceDeclarationClauseHierarchy, CurrentSourceDeclarationClauses,
            SourceDeclarationClausesKey, SourceDeclarationClausesPending,
            check_actual_parent_source_clauses,
        },
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
            // Observe source-proven query domains for both actual associated
            // declarations; missing elaboration/body producers stay pending.
            let streaming = named(&rows, root.crate_key, &[], "StreamingIterator", ItemKind::Trait);
            let declaration_item = rows.iter().find_map(|(&id, row)| {
                (row.item.scope.crate_key == root.crate_key
                    && row.item.def_key.item_kind == ItemKind::TypeAlias
                    && row.item.def_key.declared_name == "Item"
                    && parent(&rows, id) == streaming).then_some(id)
            }).expect("actual trait associated Item declaration");
            let mut semantics = Vec::new();
            for (item, owner, has_definition, alias) in [
                (definition, expected_parent, true, SourceSemantic::NotApplicable),
                (declaration_item, streaming, false, SourceSemantic::Known(SourceAliasDomain::ProjectionType)),
            ] {
                let key = SourceDefinitionSemanticsKey { inventory: seal.clone(), definition: item };
                let query = QueryKey::SourceDefinitionSemantics(Box::new(key.clone()));
                let first = ready(db.fetch(&SyntaxProvider, query.clone()));
                let current = ready(CurrentSourceDefinitionSemantics::capture(&mut db, &key));
                let semantic = current.plan();
                assert_eq!(semantic.parent(), SourceSemantic::Known(owner));
                assert_eq!(semantic.boolean(Boolean::HasItemDefinition), SourceSemantic::Known(has_definition));
                assert_eq!(semantic.boolean(Boolean::IsConditionallyConst), SourceSemantic::Known(false));
                assert_eq!(semantic.boolean(Boolean::RequiresSizedSelf), SourceSemantic::Pending(SemanticPending::ClauseElaboration));
                assert_eq!(semantic.boolean(Boolean::HasTargetFeatures), SourceSemantic::NotApplicable);
                assert_eq!(semantic.alias_domain(), alias);
                assert!(matches!(semantic.opaque_types_defined_by(), SourceSemantic::Pending(SemanticPending::OpaqueTypeWalk)));
                let again = ready(db.fetch(&SyntaxProvider, query));
                assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                assert_eq!(first.fingerprint, again.fingerprint);
                current.require_current(&db).unwrap();
                semantics.push(current);
            }
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
            assert!(semantics.iter().all(|semantic| semantic.require_current(&db).is_err()));
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(current.require_current(&db).is_err());
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
            worlds += 1;
            eprintln!(
                "actual source clause parent instantiation edition{edition},origin{origin}: 2parent/1own, actual owner rebasing/spans, pinned methods, memo/cold/withdrawal; 2source semantic declarations with actual domains/pending/currentness; full queries/root/runtime held"
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
            // The actual parent now demands its trait-Self default policy.
            // Its unsupported written HRTB row prevents that policy's complete
            // source input before TypeIR clause encoding. The child own bundle
            // above must still leave this entire parent demand cold.
            assert_eq!(
                parent.pending(),
                Some(SourceDeclarationClausesPending::Source(
                    crate::frontend::declaration_source_predicates::DeclarationSourcePredicatesPending::DefaultBounds(
                        crate::traits::upstream::source_default_bounds::DefaultBoundsPending::WrittenRecipe
                    )
                )),
                "actual parent default-bound input must remain explicitly pending"
            );
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

#[test]
fn unchanged_negative_supertrait_bounds_remain_typed_pending_all_editions_and_origins() {
    use crate::{
        db::dependency::{InputKey, InputValue},
        frontend::declaration_source_predicates::DeclarationSourcePredicatesPending,
        syntax::ast_adapter::range,
        traits::upstream::source_default_bounds::DefaultBoundsPending,
    };
    use ra_ap_syntax::{
        AstNode, SyntaxKind,
        ast::{self, HasTypeBounds},
    };
    let expected =
        DeclarationSourcePredicatesPending::DefaultBounds(DefaultBoundsPending::WrittenRecipe);
    let mut worlds = 0;
    let mut declarations = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/traits/negative-bounds/supertrait.rs",
                "trait A",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let a = named(&rows, root.crate_key, &[], "A", ItemKind::Trait);
            let b = named(&rows, root.crate_key, &[], "B", ItemKind::Trait);
            let mut held = Vec::new();
            let mut source_guards = Vec::new();
            let mut trait_guards = Vec::new();
            db.trace.detailed = true;
            let before = db.trace.events.len();
            for (definition, target, target_name) in [(a, b, "B"), (b, a, "A")] {
                let row = &rows[&definition];
                assert_eq!(row.generated, origin != 0);
                assert_ne!(definition, target);
                assert!(
                    matches!(db.input(&InputKey::DefinitionSource(definition)).as_deref(),
                    Some(InputValue::DefinitionSource(source)) if *source == row.parse)
                );
                let parsed =
                    ready(db.fetch(&SyntaxProvider, QueryKey::ParseFile(row.parse.clone()))).value;
                let QueryValue::ParseFile(parsed) = parsed.as_ref() else {
                    panic!("actual negative-bound parser differs")
                };
                assert!(
                    parsed.errors.is_empty(),
                    "negative bound must be admitted as ordinary syntax: {:?}",
                    parsed.errors
                );
                let syntax = parsed
                    .syntax_at(row.item.declaration_span, |node| {
                        ast::Trait::can_cast(node.kind())
                    })
                    .unwrap()
                    .and_then(ast::Trait::cast)
                    .expect("actual negative-bound trait syntax");
                let bounds = syntax
                    .type_bound_list()
                    .expect("actual negative supertrait list")
                    .bounds()
                    .collect::<Vec<_>>();
                assert_eq!(bounds.len(), 1);
                let bound = &bounds[0];
                let bangs = bound
                    .syntax()
                    .children_with_tokens()
                    .filter(|token| token.kind() == SyntaxKind::BANG)
                    .count();
                assert_eq!(
                    bangs, 1,
                    "negative polarity belongs to the exact retained TypeBound"
                );
                assert!(bound.question_mark_token().is_none());
                let Some(ast::Type::PathType(path)) = bound.ty() else {
                    panic!("actual negative target must remain a path Type")
                };
                assert_eq!(path.syntax().text().to_string(), target_name);
                let bound_range = range(bound.syntax().text_range());
                assert!(
                    row.item.declaration_span.start <= bound_range.start
                        && bound_range.end <= row.item.declaration_span.end
                );
                let trait_query = QueryKey::TraitDeclaration(definition);
                let actual_trait = ready(db.fetch(&SyntaxProvider, trait_query.clone()));
                let QueryValue::TraitDeclaration(trait_plan) = actual_trait.value.as_ref() else {
                    panic!("actual trait declaration differs")
                };
                assert_eq!(trait_plan.definition(), definition);
                assert!(trait_plan.is_trait());
                assert_eq!(trait_plan.source(), Some(&row.parse));
                trait_guards.push(ready(db.prepare_input_guard(
                    &SyntaxProvider,
                    &trait_query,
                    actual_trait.fingerprint,
                )));
                let key = SourceDeclarationClausesKey {
                    inventory: seal.clone(),
                    definition,
                };
                let source_query = QueryKey::DeclarationSourcePredicates(Box::new(key.clone()));
                let source_first = ready(db.fetch(&SyntaxProvider, source_query.clone()));
                let QueryValue::DeclarationSourcePredicates(source) = source_first.value.as_ref()
                else {
                    panic!("actual negative source query differs")
                };
                assert_eq!(source.key(), &key);
                assert_eq!(source.declaration().definition(), definition);
                let (actual_parse, _, context, _) = source
                    .declaration()
                    .source_parts()
                    .expect("actual source receipt");
                assert_eq!(actual_parse, &row.parse);
                assert_eq!(context.scope, row.item.scope);
                assert_eq!(
                    source.pending(),
                    Some(expected),
                    "negative written bounds must block default input before encoding"
                );
                assert_eq!(
                    source.recipes().unwrap_err(),
                    expected,
                    "pending source cannot expose a positive or empty recipe environment"
                );
                source_guards.push(ready(db.prepare_input_guard(
                    &SyntaxProvider,
                    &source_query,
                    source_first.fingerprint,
                )));
                let source_again = ready(db.fetch(&SyntaxProvider, source_query));
                assert!(
                    source_first.cacheable && Arc::ptr_eq(&source_first.value, &source_again.value)
                );
                assert_eq!(source_first.fingerprint, source_again.fingerprint);
                let own_query = QueryKey::SourceDeclarationClauses(Box::new(key.clone()));
                let first = ready(db.fetch(&SyntaxProvider, own_query.clone()));
                let current = ready(CurrentSourceDeclarationClauses::capture(&mut db, &key));
                assert_eq!(
                    current.plan().pending(),
                    Some(SourceDeclarationClausesPending::Source(expected))
                );
                assert_eq!(current.plan().source().pending(), Some(expected));
                let again = ready(db.fetch(&SyntaxProvider, own_query));
                assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                assert_eq!(first.fingerprint, again.fingerprint);
                current.require_current(&db).unwrap();
                held.push(current);
                declarations += 1;
            }
            assert_cold(&db, before);
            assert!(
                source_guards
                    .iter()
                    .all(|guard| guard.require_current(&db).is_ok())
            );
            assert!(
                trait_guards
                    .iter()
                    .all(|guard| guard.require_current(&db).is_ok())
            );
            db.begin_expansion_epoch(2).unwrap();
            assert!(
                held.iter()
                    .all(|current| current.require_current(&db).is_err())
            );
            assert!(
                source_guards
                    .iter()
                    .all(|guard| guard.require_current(&db).is_err())
            );
            if origin != 0 {
                assert!(
                    trait_guards
                        .iter()
                        .all(|guard| guard.require_current(&db).is_err()),
                    "actual generated trait source receipts depend on their publication epoch"
                );
            }
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(
                held.iter()
                    .all(|current| current.require_current(&db).is_err())
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
            worlds += 1;
            eprintln!(
                "actual negative supertrait source edition{edition},origin{origin}: 2 authentic TypeBound+BANG rows, Q78/Q80 typed written-input pending, memo/currentness/withdrawal/cold; negative solver/static/runtime acceptance held"
            );
        }
    }
    assert_eq!((worlds, declarations), (12, 24));
}

#[test]
fn unchanged_negative_associated_constraints_keep_signature_transport_pending_all_editions_and_origins()
 {
    use crate::{
        core::{diagnostic::Diagnostic, digest::Digest},
        db::{
            dependency::{InputKey, InputValue},
            memo::Database,
            trace::Action,
        },
        syntax::{
            ast_adapter::range,
            source_bound_polarity::{self, SourceBoundPolarity},
        },
        traits::upstream::source_definition_headers::{
            CurrentDefinitionHeader, DefinitionHeaderKey, DefinitionHeaderRole,
        },
    };
    use ra_ap_syntax::{AstNode, SyntaxKind, ast};
    fn signature_error(db: &mut Database, definition: Digest) -> Diagnostic {
        match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
            QueryPoll::Ready(Err(error)) => error,
            QueryPoll::Ready(Ok(_)) => {
                panic!("negative header cannot publish a positive legacy signature")
            }
            QueryPoll::NeedInput(input) => {
                panic!("actual negative signature needs input: {input:?}")
            }
        }
    }
    let mut worlds = 0;
    let mut demanded = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/traits/negative-bounds/associated-constraints.rs",
                "trait Trait",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let definitions = ["test", "test2", "test3", "test4", "test5"].map(|name| {
                (
                    name,
                    named(&rows, root.crate_key, &[], name, ItemKind::Function),
                )
            });
            let main = named(&rows, root.crate_key, &[], "main", ItemKind::Function);
            let mut headers = Vec::new();
            db.trace.detailed = true;
            let before = db.trace.events.len();
            for (name, definition) in definitions {
                let row = &rows[&definition];
                assert_eq!(row.generated, origin != 0);
                assert!(
                    matches!(db.input(&InputKey::DefinitionSource(definition)).as_deref(),
                    Some(InputValue::DefinitionSource(source)) if *source==row.parse)
                );
                let parsed =
                    ready(db.fetch(&SyntaxProvider, QueryKey::ParseFile(row.parse.clone()))).value;
                let QueryValue::ParseFile(parsed) = parsed.as_ref() else {
                    panic!("actual negative function parser differs")
                };
                assert!(
                    parsed.errors.is_empty(),
                    "ordinary negative header syntax must be admitted: {:?}",
                    parsed.errors
                );
                let function = parsed
                    .syntax_at(row.item.declaration_span, |node| {
                        ast::Fn::can_cast(node.kind())
                    })
                    .unwrap()
                    .and_then(ast::Fn::cast)
                    .expect("actual negative-bound function AST");
                let negative = function
                    .syntax()
                    .descendants()
                    .filter_map(ast::TypeBound::cast)
                    .filter(|bound| {
                        source_bound_polarity::polarity(bound) == SourceBoundPolarity::Negative
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    negative.len(),
                    1,
                    "each unchanged function has exactly one actual negative bound"
                );
                assert_eq!(
                    negative[0]
                        .syntax()
                        .children_with_tokens()
                        .filter(|token| token.kind() == SyntaxKind::BANG)
                        .count(),
                    1
                );
                let negative_span = range(negative[0].syntax().text_range());
                assert!(
                    row.item.declaration_span.start <= negative_span.start
                        && negative_span.end <= row.item.declaration_span.end
                );
                assert!(
                    source_bound_polarity::negative_in_header(function.syntax()),
                    "shared retained-header detector must see the real negative bound"
                );
                let key = DefinitionHeaderKey {
                    inventory: seal.clone(),
                    definition,
                };
                let query = QueryKey::DefinitionHeaderPlan(Box::new(key.clone()));
                let first = ready(db.fetch(&SyntaxProvider, query.clone()));
                let current = ready(CurrentDefinitionHeader::capture(&mut db, &key));
                let header = current.plan();
                assert_eq!(
                    header.pending(),
                    None,
                    "source header metadata remains independently available"
                );
                assert_eq!(header.role(), DefinitionHeaderRole::Function);
                assert_eq!(header.name(), Some(name));
                assert!(header.source_function().is_some());
                let (actual_parse, _, context, _) = header
                    .declaration()
                    .source_parts()
                    .expect("actual function header receipt");
                assert_eq!(actual_parse, &row.parse);
                assert_eq!(context.range, row.item.declaration_span);
                assert_eq!(context.scope, row.item.scope);
                let again = ready(db.fetch(&SyntaxProvider, query));
                assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                assert_eq!(first.fingerprint, again.fingerprint);
                let error = signature_error(&mut db, definition);
                assert_eq!(error.code, "LRR-E-UNSUPPORTED");
                assert_eq!(
                    error.message,
                    "negative source bounds require semantic signature transport"
                );
                let repeated_before = db.trace.events.len();
                let repeated = signature_error(&mut db, definition);
                assert_eq!(
                    (repeated.code, repeated.message.as_str()),
                    (error.code, error.message.as_str())
                );
                assert!(
                    db.trace.events[repeated_before..]
                        .iter()
                        .any(|event| event.key == QueryKey::FunctionSignature(definition)
                            && event.action == Action::MemoHit),
                    "the actual failed signature query reuses its retained memo"
                );
                current.require_current(&db).unwrap();
                headers.push(current);
                demanded += 1;
            }
            assert!(
                db.trace.events[before..].iter().all(|event| !matches!(
                    &event.key,
                    QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_)
                )),
                "signature metadata refusal must leave executable preparation cold"
            );
            assert!(
                db.trace.events[before..]
                    .iter()
                    .all(|event| match &event.key {
                        QueryKey::FunctionSignature(id) =>
                            *id != main && definitions.iter().any(|(_, actual)| actual == id),
                        _ => true,
                    }),
                "main and all unselected function signatures remain cold"
            );
            db.begin_expansion_epoch(2).unwrap();
            assert!(
                headers
                    .iter()
                    .all(|current| current.require_current(&db).is_err())
            );
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(
                headers
                    .iter()
                    .all(|current| current.require_current(&db).is_err())
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
            worlds += 1;
            eprintln!(
                "actual negative associated constraint signatures edition{edition},origin{origin}: 5 real headers/TypeBound+BANG, exact semantic-transport unsupported, memo/currentness/withdrawal, main/bodies cold; static diagnostics/root/runtime held"
            );
        }
    }
    assert_eq!((worlds, demanded), (12, 60));
}
