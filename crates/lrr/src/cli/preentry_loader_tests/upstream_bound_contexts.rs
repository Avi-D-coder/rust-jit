//! Actual bound provenance and immediate filtered clauses; no root or guest run.
use super::upstream_generated_projection_fields::{actual_rows, assert_cold, named};
use super::upstream_projection_types::fixture_world;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::SealKey,
        value::QueryValue,
    },
    frontend::declaration_source_predicates::{
        DeclarationSourcePredicatesKey, DeclarationSourcePredicatesPending,
        PreparedDeclarationSourcePredicates,
    },
    source::manifest::ItemKind,
    syntax::{ast_adapter::range, outline::ScopeKey, spans::SourceRange},
    traits::{
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::{ParamKey, TyTerm},
        upstream::{
            source_default_bounds::DefaultBoundsPending,
            source_definition_headers::PreparedDefinitionHeader,
            source_trait_clauses::{
                CurrentSourceTraitClauses, SourceTraitClauseFilter, SourceTraitClausesKey,
                SourceTraitClausesPending, check_actual_filtered_trait_encoding,
            },
        },
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams, HasTypeBounds},
};
use std::{collections::BTreeMap, sync::Arc};

const INJECT: &str = "tests/ui/higher-ranked/trait-bounds/normalize-under-binder/issue-62529-1.rs";
const FILTERED: &str = "tests/ui/implied-bounds/implied-bounds-unconstrained-1.rs";

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual bound context source: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual bound context input: {input:?}"),
    }
}
fn trait_ast(header: &PreparedDefinitionHeader) -> ast::Trait {
    let (_, _, context, parsed) = header.declaration().source_parts().unwrap();
    assert!(
        parsed.errors.is_empty(),
        "actual unchanged grammar: {:?}",
        parsed.errors
    );
    let node = parsed
        .syntax_at(context.range, |node| ast::Trait::can_cast(node.kind()))
        .unwrap()
        .expect("actual retained trait node");
    ast::Trait::cast(node).unwrap()
}
fn self_type(header: &PreparedDefinitionHeader) -> TyTerm {
    let declaration = header.declaration();
    assert!(declaration.has_trait_self());
    TyTerm::Param(ParamKey {
        definition: header.key().definition,
        ordinal: u32::try_from(declaration.parameters().len()).unwrap(),
    })
}
fn own_plan(value: &Arc<QueryValue>) -> &PreparedDeclarationSourcePredicates {
    let QueryValue::DeclarationSourcePredicates(plan) = value.as_ref() else {
        panic!("actual own source query differs")
    };
    plan
}
fn own(
    db: &mut Database,
    seal: &SealKey,
    definition: Digest,
) -> (Arc<QueryValue>, PreparedInputGuard) {
    let query = QueryKey::DeclarationSourcePredicates(Box::new(DeclarationSourcePredicatesKey {
        inventory: seal.clone(),
        definition,
    }));
    let first = ready(db.fetch(&SyntaxProvider, query.clone()));
    let again = ready(db.fetch(&SyntaxProvider, query.clone()));
    assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
    assert_eq!(first.fingerprint, again.fingerprint);
    let guard = ready(db.prepare_input_guard(&SyntaxProvider, &query, first.fingerprint));
    guard.require_current(db).unwrap();
    (first.value, guard)
}
fn filtered(
    db: &mut Database,
    seal: &SealKey,
    definition: Digest,
    filter: SourceTraitClauseFilter,
) -> CurrentSourceTraitClauses {
    let key = SourceTraitClausesKey {
        inventory: seal.clone(),
        definition,
        filter,
    };
    let query = QueryKey::SourceTraitClauses(Box::new(key.clone()));
    let first = ready(db.fetch(&SyntaxProvider, query.clone()));
    let again = ready(db.fetch(&SyntaxProvider, query));
    assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
    assert_eq!(first.fingerprint, again.fingerprint);
    let (current, encoded) = check_actual_filtered_trait_encoding(db, &key).unwrap();
    assert_eq!(current.plan().key(), &key);
    match current.plan().pending() {
        None => assert_eq!(encoded, Ok(current.plan().recipes().len())),
        Some(why) => assert_eq!(encoded, Err(why), "actual production encoder pending gate"),
    }
    current.require_current(db).unwrap();
    current
}
fn assert_row(
    recipe: &SourcePredicateRecipe,
    subject: &TyTerm,
    trait_id: Digest,
    expected_span: SourceRange,
) {
    let SourcePredicateRecipe::Explicit {
        source,
        result: PredicateResult::Implemented(reference),
    } = recipe
    else {
        panic!("actual positive bound row differs: {recipe:?}")
    };
    assert_eq!(*source, expected_span);
    assert_eq!(reference.trait_key, trait_id);
    assert_eq!(&reference.self_ty, subject);
    assert!(reference.args.is_empty());
}
fn retire(db: &mut Database, filtered: &[CurrentSourceTraitClauses], own: &PreparedInputGuard) {
    for current in filtered {
        current.require_current(db).unwrap();
    }
    own.require_current(db).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert!(
        filtered
            .iter()
            .all(|current| current.require_current(db).is_err())
    );
    assert!(own.require_current(db).is_err());
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(
        filtered
            .iter()
            .all(|current| current.require_current(db).is_err())
    );
    assert!(own.require_current(db).is_err());
}
fn worlds(
    case: &str,
    first_item: &str,
    mut check: impl FnMut(&mut Database, &SealKey, &ScopeKey, u16, u8),
) {
    let mut completed = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(case, first_item, edition, origin);
            check(&mut db, &seal, &root, edition, origin);
            completed += 1;
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
        }
    }
    assert_eq!(completed, 12);
}

#[test]
fn unchanged_where_self_sized_retains_item_and_bound_provenance_all_editions_and_origins() {
    worlds(
        INJECT,
        "pub trait FamilyLt",
        |db, seal, root, edition, origin| {
            let rows = actual_rows(db, seal);
            let owner = named(&rows, root.crate_key, &[], "Inject", ItemKind::Trait);
            assert_eq!(rows[&owner].generated, origin != 0);
            let core = crate::facade::unit(
                &ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
                "core",
            );
            let meta = named(&rows, core, &["marker"], "MetaSized", ItemKind::Trait);
            let sized = named(&rows, core, &["marker"], "Sized", ItemKind::Trait);
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let (own, guard) = own(db, seal, owner);
            let plan = own_plan(&own);
            assert_eq!(plan.pending(), None);
            let (_, _, context, parsed) = plan.declaration().source_parts().unwrap();
            assert!(parsed.errors.is_empty());
            let trait_node = ast::Trait::cast(
                parsed
                    .syntax_at(context.range, |node| ast::Trait::can_cast(node.kind()))
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
            assert!(
                trait_node.type_bound_list().is_none(),
                "actual Inject has no colon list"
            );
            let predicates = trait_node
                .where_clause()
                .unwrap()
                .predicates()
                .collect::<Vec<_>>();
            assert_eq!(predicates.len(), 1);
            assert_eq!(
                predicates[0].ty().unwrap().syntax().text().to_string(),
                "Self"
            );
            let bounds = predicates[0]
                .type_bound_list()
                .unwrap()
                .bounds()
                .collect::<Vec<_>>();
            assert_eq!(bounds.len(), 1);
            assert_eq!(bounds[0].syntax().text().to_string(), "Sized");
            let bound_span = range(bounds[0].syntax().text_range());
            let subject = plan.self_type().unwrap().clone();
            let recipes = plan.recipes().unwrap();
            assert_eq!(recipes.len(), 2);
            assert_row(&recipes[0], &subject, meta, context.range);
            assert_row(&recipes[1], &subject, sized, bound_span);
            assert_ne!(context.range, bound_span);
            let mut currents = Vec::new();
            for filter in [
                SourceTraitClauseFilter::Super,
                SourceTraitClauseFilter::Implied,
            ] {
                let current = filtered(db, seal, owner, filter);
                let plan = current.plan();
                assert_eq!(plan.pending(), None, "actual Inject {filter:?}");
                assert_eq!(self_type(plan.header()), subject);
                assert_eq!(plan.recipes(), recipes);
                let dependencies = plan.positive_dependencies();
                assert_eq!(dependencies.len(), 2);
                for (dependency, expected, span) in [
                    (dependencies[0], meta, context.range),
                    (dependencies[1], sized, bound_span),
                ] {
                    assert_eq!(dependency.definition, expected);
                    assert_eq!(dependency.filter, filter);
                    assert_eq!(dependency.origin, rows[&owner].parse.content);
                    assert_eq!(dependency.source, span);
                }
                currents.push(current);
            }
            assert_cold(db, before);
            retire(db, &currents, &guard);
            eprintln!(
                "actual trait Self bound provenance edition{edition},origin{origin}: 2own/4filtered/4encoded, MetaSized item span and Sized where span, authentic dependencies/memo/cold/epoch; root held"
            );
        },
    );
}

#[test]
fn unchanged_filtered_trait_clauses_leave_unrelated_hrtb_unforced_all_editions_and_origins() {
    worlds(
        FILTERED,
        "pub trait QueryDb",
        |db, seal, root, edition, origin| {
            let rows = actual_rows(db, seal);
            let owner = named(
                &rows,
                root.crate_key,
                &[],
                "QueryStorageOpsAsync",
                ItemKind::Trait,
            );
            assert_eq!(rows[&owner].generated, origin != 0);
            let core = crate::facade::unit(
                &ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
                "core",
            );
            let meta = named(&rows, core, &["marker"], "MetaSized", ItemKind::Trait);
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let mut currents = Vec::new();
            for filter in [
                SourceTraitClauseFilter::Super,
                SourceTraitClauseFilter::Implied,
            ] {
                let current = filtered(db, seal, owner, filter);
                let plan = current.plan();
                assert_eq!(
                    plan.pending(),
                    None,
                    "unrelated HRTB must remain cold: {filter:?}"
                );
                let syntax = trait_ast(plan.header());
                let bound = syntax
                    .where_clause()
                    .unwrap()
                    .predicates()
                    .next()
                    .unwrap()
                    .type_bound_list()
                    .unwrap()
                    .bounds()
                    .next()
                    .unwrap();
                assert!(bound.for_binder().is_some(), "unchanged generic Q HRTB");
                let (_, _, context, _) = plan.header().declaration().source_parts().unwrap();
                assert_eq!(plan.recipes().len(), 1);
                assert_row(
                    &plan.recipes()[0],
                    &self_type(plan.header()),
                    meta,
                    context.range,
                );
                assert_eq!(plan.positive_dependencies().len(), 1);
                currents.push(current);
            }
            assert!(
                db.trace.events[before..]
                    .iter()
                    .all(|event| !matches!(event.key, QueryKey::DeclarationSourcePredicates(_))),
                "filtered preparation must not demand the full own source query"
            );
            let (own, guard) = own(db, seal, owner);
            assert_eq!(
                own_plan(&own).pending(),
                Some(DeclarationSourcePredicatesPending::DefaultBounds(
                    DefaultBoundsPending::WrittenRecipe
                )),
                "actual full unrelated HRTB remains pending"
            );
            assert_cold(db, before);
            retire(db, &currents, &guard);
            eprintln!(
                "actual filtered unrelated HRTB edition{edition},origin{origin}: 2ready filtered/2Self defaults/2encoded, full own pending, no premature Q78/body demand, memo/epoch; root held"
            );
        },
    );
}

#[test]
fn unchanged_selected_nested_associated_trait_bounds_remain_pending_all_editions_and_origins() {
    worlds(
        FILTERED,
        "pub trait QueryDb",
        |db, seal, root, edition, origin| {
            let rows = actual_rows(db, seal);
            let owner = named(
                &rows,
                root.crate_key,
                &[],
                "AsyncQueryFunction",
                ItemKind::Trait,
            );
            assert_eq!(rows[&owner].generated, origin != 0);
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let mut currents = Vec::new();
            for filter in [
                SourceTraitClauseFilter::Super,
                SourceTraitClauseFilter::Implied,
            ] {
                let current = filtered(db, seal, owner, filter);
                let plan = current.plan();
                assert_eq!(plan.source_pending(), None);
                assert_eq!(
                    plan.default_pending(),
                    Some(DefaultBoundsPending::WrittenRecipe)
                );
                assert_eq!(
                    plan.pending(),
                    Some(SourceTraitClausesPending::Defaults(
                        DefaultBoundsPending::WrittenRecipe
                    )),
                    "selected associated binding is not a complete answer"
                );
                let syntax = trait_ast(plan.header());
                let bound = syntax.type_bound_list().unwrap().bounds().next().unwrap();
                let expected_span = range(bound.syntax().text_range());
                let bindings = bound
                    .syntax()
                    .descendants()
                    .filter_map(ast::AssocTypeArg::cast)
                    .collect::<Vec<_>>();
                assert_eq!(bindings.len(), 1, "actual QueryDb associated equality");
                assert!(bindings[0].eq_token().is_some());
                assert_eq!(
                    bindings[0].name_ref().unwrap().syntax().text().to_string(),
                    "Db"
                );
                let pending = plan
                    .recipes()
                    .iter()
                    .filter(|recipe| {
                        matches!(recipe,
                SourcePredicateRecipe::Explicit { source, result: PredicateResult::Pending(
                    PredicatePending::AssociatedIdentity) } if *source == expected_span)
                    })
                    .count();
                assert_eq!(
                    pending,
                    1,
                    "selected nested associated row retained: {:?}",
                    plan.recipes()
                );
                let pending_dependencies = plan.pending_dependencies();
                assert_eq!(pending_dependencies.len(), 1);
                assert_eq!(pending_dependencies[0].origin, rows[&owner].parse.content);
                assert_eq!(pending_dependencies[0].source, expected_span);
                assert_eq!(
                    pending_dependencies[0].pending,
                    PredicatePending::AssociatedIdentity
                );
                assert!(
                    plan.positive_dependencies()
                        .iter()
                        .all(|dependency| dependency.source != expected_span),
                    "pending row must not certify positive recursion"
                );
                currents.push(current);
            }
            let (own, guard) = own(db, seal, owner);
            assert!(
                own_plan(&own).pending().is_some(),
                "full source stays typed pending too"
            );
            assert_cold(db, before);
            retire(db, &currents, &guard);
            eprintln!(
                "actual selected nested associated bound edition{edition},origin{origin}: 2pending filtered/2retained AssociatedIdentity rows/2typed encoding refusals, actual source spans, memo/cold/epoch; root held"
            );
        },
    );
}

enum ExpectedTrait {
    Local(&'static str),
    CoreMarker(&'static str),
}
struct ExpectedClause {
    identity: ExpectedTrait,
    written_bound: Option<&'static str>,
}
fn ordering(case: &str, first_item: &str, owner_name: &str, expected: [ExpectedClause; 2]) {
    worlds(case, first_item, |db, seal, root, edition, origin| {
        let rows = actual_rows(db, seal);
        let owner = named(&rows, root.crate_key, &[], owner_name, ItemKind::Trait);
        assert_eq!(rows[&owner].generated, origin != 0);
        let core = crate::facade::unit(
            &ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
            "core",
        );
        db.trace.detailed = true;
        let before = db.trace.events.len();
        let mut currents = Vec::new();
        let mut expected_rows = Vec::new();
        for filter in [
            SourceTraitClauseFilter::Super,
            SourceTraitClauseFilter::Implied,
        ] {
            let current = filtered(db, seal, owner, filter);
            let plan = current.plan();
            assert_eq!(plan.pending(), None, "actual {owner_name} {filter:?} order");
            let syntax = trait_ast(plan.header());
            let (_, _, context, _) = plan.header().declaration().source_parts().unwrap();
            let bounds = syntax
                .type_bound_list()
                .unwrap()
                .bounds()
                .collect::<Vec<_>>();
            assert_eq!(plan.recipes().len(), expected.len());
            for (recipe, expected) in plan.recipes().iter().zip(&expected) {
                let trait_id = match expected.identity {
                    ExpectedTrait::Local(name) => {
                        named(&rows, root.crate_key, &[], name, ItemKind::Trait)
                    }
                    ExpectedTrait::CoreMarker(name) => {
                        named(&rows, core, &["marker"], name, ItemKind::Trait)
                    }
                };
                let source = expected.written_bound.map_or(context.range, |text| {
                    let found = bounds
                        .iter()
                        .filter(|bound| bound.syntax().text().to_string() == text)
                        .collect::<Vec<_>>();
                    assert_eq!(found.len(), 1, "actual unique written {text}");
                    range(found[0].syntax().text_range())
                });
                assert_row(recipe, &self_type(plan.header()), trait_id, source);
            }
            if expected_rows.is_empty() {
                expected_rows.extend_from_slice(plan.recipes());
            } else {
                assert_eq!(plan.recipes(), expected_rows.as_slice());
            }
            currents.push(current);
        }
        let (own, guard) = own(db, seal, owner);
        assert_eq!(own_plan(&own).pending(), None);
        assert_eq!(own_plan(&own).recipes().unwrap(), expected_rows.as_slice());
        assert_cold(db, before);
        retire(db, &currents, &guard);
        eprintln!(
            "actual trait clause ordering {owner_name},edition{edition},origin{origin}: 2own/4filtered/4encoded, authentic compiler order/spans, memo/cold/epoch; root held"
        );
    });
}

#[test]
fn unchanged_explicit_sized_precedes_other_supertraits_all_editions_and_origins() {
    ordering(
        "tests/ui/traits/default-method/supervtable.rs",
        "fn require_y",
        "Z",
        [
            ExpectedClause {
                identity: ExpectedTrait::CoreMarker("Sized"),
                written_bound: Some("Sized"),
            },
            ExpectedClause {
                identity: ExpectedTrait::Local("Y"),
                written_bound: Some("Y"),
            },
        ],
    );
}

#[test]
fn unchanged_implicit_metasized_precedes_copy_supertrait_all_editions_and_origins() {
    ordering(
        "tests/ui/union/union-manuallydrop-rpass.rs",
        "use std::mem",
        "ImpliesCopy",
        [
            ExpectedClause {
                identity: ExpectedTrait::CoreMarker("MetaSized"),
                written_bound: None,
            },
            ExpectedClause {
                identity: ExpectedTrait::CoreMarker("Copy"),
                written_bound: Some("Copy"),
            },
        ],
    );
}
