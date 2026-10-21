//! Actual source projection metadata controls; no solver root or guest main.
use super::{configured_impl_catalog_world, temporary_source};
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, seals::SealKey,
        value::QueryValue,
    },
    source::manifest::ItemKind,
    syntax::outline::{ItemOutline, ScopeKey},
};
use std::sync::Arc;
use std::{collections::BTreeMap, path::PathBuf};

fn check_actual_clause_emission_all_editions_and_origins() {
    use crate::{
        frontend::declaration_source_predicates::DeclarationSourcePredicatesKey,
        traits::{
            predicate::{PredicateResult, SourcePredicateRecipe},
            term::{ParamKey, TyTerm},
            upstream::{
                source_declaration_clauses::{
                    CurrentSourceDeclarationClauses, check_actual_declaration_clause_emission,
                },
                source_default_bounds::DefaultBoundsContext,
                source_definition_headers::DefinitionHeaderKey,
                source_lang_identity::CurrentSourceLangIdentity,
                source_trait_clauses::{
                    SourceTraitClauseFilter, SourceTraitClausesKey,
                    check_actual_filtered_trait_encoding,
                },
            },
        },
    };
    use ra_ap_syntax::{
        AstNode,
        ast::{self, HasTypeBounds},
    };
    let mut worlds = 0;
    let mut written_rows = 0;
    let mut transient_rows = 0;
    let mut shadow_rows = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, _, temporary) = fixture_world(
                "tests/ui/enum-discriminant/repr128.rs",
                "use std::intrinsics",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let core = crate::facade::unit(
                &ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
                "core",
            );
            let partial_eq = actual_named(&rows, core, &["cmp"], "PartialEq", ItemKind::Trait);
            let pointee = actual_named(&rows, core, &["marker"], "PointeeSized", ItemKind::Trait);
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let language = ready(CurrentSourceLangIdentity::capture(
                &mut db,
                &DefinitionHeaderKey {
                    inventory: seal.clone(),
                    definition: pointee,
                },
            ));
            assert!(matches!(
                language.plan().classification(),
                Ok(Some(("pointee_sized", _)))
            ));
            let key = DeclarationSourcePredicatesKey {
                inventory: seal.clone(),
                definition: partial_eq,
            };
            let own_query = QueryKey::DeclarationSourcePredicates(Box::new(key.clone()));
            let own = ready(db.fetch(&SyntaxProvider, own_query.clone()));
            let QueryValue::DeclarationSourcePredicates(own_plan) = own.value.as_ref() else {
                unreachable!()
            };
            assert_eq!(own_plan.pending(), None);
            let declaration = own_plan.declaration();
            assert!(declaration.has_trait_self());
            assert_eq!(declaration.parameters().len(), 1);
            assert_eq!(declaration.parameters()[0].name, "Rhs");
            let self_key = ParamKey {
                definition: partial_eq,
                ordinal: u32::try_from(declaration.parameters().len()).unwrap(),
            };
            let rhs_key = ParamKey {
                definition: partial_eq,
                ordinal: declaration.parameters()[0].ordinal,
            };
            let raw = own_plan.recipes().unwrap();
            assert_eq!(
                raw.len(),
                2,
                "actual PartialEq Self and Rhs source bounds stay retained"
            );
            let mut subjects = Vec::new();
            for recipe in raw {
                let SourcePredicateRecipe::Explicit {
                    result: PredicateResult::Implemented(reference),
                    ..
                } = recipe
                else {
                    panic!("actual written PointeeSized row changed: {recipe:?}")
                };
                assert_eq!(reference.trait_key, pointee);
                let TyTerm::Param(subject) = reference.self_ty else {
                    panic!("actual source parameter")
                };
                assert!(reference.args.is_empty());
                subjects.push(subject);
            }
            subjects.sort();
            let mut expected = vec![self_key, rhs_key];
            expected.sort();
            assert_eq!(subjects, expected);
            let own_guard =
                ready(db.prepare_input_guard(&SyntaxProvider, &own_query, own.fingerprint));
            let default_query = QueryKey::SourceDefaultBounds(Box::new(key.clone()));
            let defaults = ready(db.fetch(&SyntaxProvider, default_query.clone()));
            let QueryValue::SourceDefaultBounds(policy) = defaults.value.as_ref() else {
                unreachable!()
            };
            assert_eq!(policy.sizedness_identity("pointee_sized"), Some(pointee));
            let default_guard = ready(db.prepare_input_guard(
                &SyntaxProvider,
                &default_query,
                defaults.fingerprint,
            ));
            let q80 = ready(CurrentSourceDeclarationClauses::capture(&mut db, &key));
            assert!(
                check_actual_declaration_clause_emission(&mut db, &key)
                    .unwrap()
                    .is_empty(),
                "ordinary declaration emission also discards both actual transient bounds"
            );
            let mut filtered = Vec::new();
            for filter in [
                SourceTraitClauseFilter::Super,
                SourceTraitClauseFilter::Implied,
            ] {
                let (current, encoded) = check_actual_filtered_trait_encoding(
                    &mut db,
                    &SourceTraitClausesKey {
                        inventory: seal.clone(),
                        definition: partial_eq,
                        filter,
                    },
                )
                .unwrap();
                assert_eq!(encoded, Ok(0));
                let plan = current.plan();
                assert_eq!(plan.pending(), None);
                assert!(
                    plan.positive_dependencies().is_empty(),
                    "discarded clauses are not semantic graph edges"
                );
                let [
                    SourcePredicateRecipe::Explicit {
                        source,
                        result: PredicateResult::Implemented(reference),
                    },
                ] = plan.recipes()
                else {
                    panic!("actual filtered written Self bound stays present")
                };
                assert_eq!(reference.trait_key, pointee);
                assert_eq!(reference.self_ty, TyTerm::Param(self_key));
                let header = plan.header();
                let (parse, _, context, parsed) = header.declaration().source_parts().unwrap();
                let syntax = ast::Trait::cast(
                    parsed
                        .syntax_at(context.range, |node| ast::Trait::can_cast(node.kind()))
                        .unwrap()
                        .unwrap(),
                )
                .unwrap();
                let bound = syntax.type_bound_list().unwrap().bounds().next().unwrap();
                assert_eq!(*source, crate::syntax::ast_adapter::range(bound.syntax().text_range()));
                let inputs = plan
                    .written()
                    .unwrap()
                    .default_inputs(DefaultBoundsContext::TraitSelf);
                assert_eq!(
                    inputs.recipes().collect::<Vec<_>>(),
                    plan.recipes().iter().collect::<Vec<_>>(),
                    "default policy still sees the real colon bound"
                );
                assert!(
                    policy
                        .apply(
                            &TyTerm::Param(self_key),
                            (parse.content, context.range),
                            DefaultBoundsContext::TraitSelf,
                            inputs
                        )
                        .unwrap()
                        .is_empty(),
                    "actual PointeeSized still suppresses implicit sizedness"
                );
                current.require_current(&db).unwrap();
                transient_rows += plan.recipes().len();
                filtered.push(current);
            }
            let again = ready(db.fetch(&SyntaxProvider, own_query));
            assert!(own.cacheable && Arc::ptr_eq(&own.value, &again.value));
            assert_eq!(own.fingerprint, again.fingerprint);
            assert_cold(&db, before);
            own_guard.require_current(&db).unwrap();
            default_guard.require_current(&db).unwrap();
            q80.require_current(&db).unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(
                filtered
                    .iter()
                    .all(|query| query.require_current(&db).is_err())
            );
            assert!(own_guard.require_current(&db).is_err());
            assert!(default_guard.require_current(&db).is_err());
            assert!(language.require_current(&db).is_err());
            assert!(q80.require_current(&db).is_err());
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(
                filtered
                    .iter()
                    .all(|query| query.require_current(&db).is_err())
            );
            worlds += 1;
            written_rows += raw.len();
            eprintln!(
                "actual transient emission edition{edition},origin{origin}: 2written rows,2filtered transient rows,0emitted clauses; actual role/header/defaults retained, Q80/Q88 consumers, no semantic edge, memo/epoch revocation, bodies cold; root/runtime held"
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }

            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/traits/core-marker-name-shadowing-issue-2284.rs",
                "trait Send",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let send = actual_named(&rows, root.crate_key, &[], "Send", ItemKind::Trait);
            let functions = rows
                .iter()
                .filter(|(_, item)| {
                    item.scope == root
                        && item.def_key.declared_name == "f"
                        && item.def_key.item_kind == ItemKind::Function
                })
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            assert_eq!(functions.len(), 1, "actual free f is distinct from Send::f");
            let function = functions[0];
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let language = ready(CurrentSourceLangIdentity::capture(
                &mut db,
                &DefinitionHeaderKey {
                    inventory: seal.clone(),
                    definition: send,
                },
            ));
            assert_eq!(
                language.plan().classification(),
                Ok(None),
                "ordinary Send has genuine non-language identity"
            );
            let key = DeclarationSourcePredicatesKey {
                inventory: seal.clone(),
                definition: function,
            };
            let current = ready(CurrentSourceDeclarationClauses::capture(&mut db, &key));
            assert_eq!(current.plan().pending(), None);
            let raw = current.plan().source().recipes().unwrap();
            assert_eq!(raw.iter().filter(|recipe| matches!(recipe,
                SourcePredicateRecipe::Explicit { result: PredicateResult::Implemented(reference), .. }
                    if reference.trait_key == send)).count(), 1);
            let emitted = check_actual_declaration_clause_emission(&mut db, &key).unwrap();
            assert_eq!(
                emitted.iter().filter(|&&trait_id| trait_id == send).count(),
                1,
                "ordinary user trait remains emitted through the same encoder"
            );
            let query = QueryKey::SourceDeclarationClauses(Box::new(key));
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let again = ready(db.fetch(&SyntaxProvider, query));
            assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
            assert_eq!(first.fingerprint, again.fingerprint);
            assert_cold(&db, before);
            current.require_current(&db).unwrap();
            language.require_current(&db).unwrap();
            db.begin_expansion_epoch(2).unwrap();
            assert!(current.require_current(&db).is_err());
            assert!(language.require_current(&db).is_err());
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(current.require_current(&db).is_err());
            worlds += 1;
            shadow_rows += 1;
            eprintln!(
                "actual Send shadow emission edition{edition},origin{origin}: 1ordinary bound retained and emitted; actual Q76 non-language identity, Q80 consumer, memo/epoch revocation, bodies cold; root/runtime held"
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
        }
    }
    assert_eq!(
        (worlds, written_rows, transient_rows, shadow_rows),
        (24, 24, 24, 12)
    );
}

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual projection source preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual projection source needs input: {input:?}"),
    }
}

/// Select from independent captured Outline declarations, including the actual
/// generated publication. The production projection plan supplies no expected ID.
fn actual_rows(db: &mut Database, seal: &SealKey) -> BTreeMap<Digest, ItemOutline> {
    let inventory = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedScopeImplHeaders(seal.clone()),
    ))
    .value;
    let QueryValue::PreparedScopeImplHeaders(headers) = inventory.as_ref() else {
        panic!("actual projection inventory differs")
    };
    let environment = headers
        .inventory_root()
        .environment()
        .expect("actual completed source environment");
    let mut rows = BTreeMap::new();
    for source in environment.sources() {
        for item in &source.outline().items {
            rows.insert(item.def_key.fingerprint(), item.clone());
        }
    }
    for unit in environment.generated_publication_for_control().0.iter() {
        for &definition in unit.definitions.keys() {
            let item = unit
                .declaration(definition)
                .expect("actual generated projection declaration");
            rows.insert(definition, item.clone());
        }
    }
    rows
}

fn actual_named(
    rows: &BTreeMap<Digest, ItemOutline>,
    unit: Digest,
    module: &[&str],
    name: &str,
    kind: ItemKind,
) -> Digest {
    let found = rows
        .iter()
        .filter(|(_, item)| {
            item.scope.crate_key == unit
                && item
                    .scope
                    .module_path
                    .iter()
                    .map(String::as_str)
                    .eq(module.iter().copied())
                && item.def_key.declared_name == name
                && item.def_key.item_kind == kind
        })
        .map(|(identity, _)| *identity)
        .collect::<Vec<_>>();
    assert_eq!(
        found.len(),
        1,
        "actual declaration {module:?}::{name} must have one source identity"
    );
    found[0]
}

fn actual_trait_item(rows: &BTreeMap<Digest, ItemOutline>, parent: Digest, name: &str) -> Digest {
    let owner = rows.get(&parent).expect("actual trait owner");
    assert_eq!(owner.def_key.item_kind, ItemKind::Trait);
    let mut member_scope = owner.scope.clone();
    // Generated publication IDs differ from the Outline's actual member-scope
    // owner attachment. Use that attachment for selection and return the real
    // publication ID retained as the map key.
    member_scope.owner = Some(owner.def_key.fingerprint());
    let found = rows
        .iter()
        .filter(|(_, item)| {
            item.scope == member_scope
                && item.def_key.declared_name == name
                && item.def_key.item_kind == ItemKind::TypeAlias
                && owner.declaration_span.start <= item.declaration_span.start
                && item.declaration_span.end <= owner.declaration_span.end
        })
        .map(|(identity, _)| *identity)
        .collect::<Vec<_>>();
    assert_eq!(
        found.len(),
        1,
        "actual trait member {name} must retain one authentic owner attachment"
    );
    found[0]
}

pub(super) fn fixture_world(
    case: &str,
    first_item: &str,
    edition: u16,
    origin: u8,
) -> (Database, SealKey, ScopeKey, Option<PathBuf>) {
    let original_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout")
        .join(case);
    let original = std::fs::read_to_string(&original_path).unwrap();
    let temporary = if origin == 0 {
        None
    } else {
        let offset = original
            .find(first_item)
            .expect("actual unchanged item boundary");
        let macros = if origin == 1 {
            "macro_rules! emit {($($t:tt)*)=>{$($t)*}} emit!"
        } else {
            "macro_rules! emit {($($t:tt)*)=>{$($t)*}} macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!"
        };
        Some(temporary_source(
            &format!("projection-{edition}-{origin}"),
            &format!(
                "{}{}{{{}}}",
                &original[..offset],
                macros,
                &original[offset..]
            ),
        ))
    };
    let path = temporary.as_ref().unwrap_or(&original_path);
    let (db, seal, root) = configured_impl_catalog_world(path, edition).unwrap_or_else(|error| {
        panic!("actual projection world {case}, edition{edition}, origin{origin}: {error:?}")
    });
    (db, seal, root, temporary)
}

fn assert_cold(db: &Database, before: usize) {
    assert!(
        db.trace.events[before..].iter().all(|event| !matches!(
            event.key,
            QueryKey::FunctionSignature(_)
                | QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::PatternPlan(_)
        )),
        "projection metadata demand must leave executable bodies cold"
    );
}

#[test]
fn unchanged_iterator_projection_fields_encode_actual_associated_declaration_all_editions_and_origins()
 {
    use crate::traits::upstream::source_adt_fields::{
        AdtSourceFieldsKey, check_actual_projected_source_fields,
    };
    let mut worlds = 0;
    let mut encoded = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/union/union-generic-rpass.rs",
                "use std::mem",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let owner = actual_named(&rows, root.crate_key, &[], "MaybeItem", ItemKind::Union);
            let core = crate::facade::unit(
                &ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
                "core",
            );
            let iterator = actual_named(&rows, core, &["iter"], "Iterator", ItemKind::Trait);
            let item = actual_trait_item(&rows, iterator, "Item");
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let query = QueryKey::AdtSourceFields(Box::new(AdtSourceFieldsKey {
                inventory: seal.clone(),
                definition: owner,
            }));
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let (current, count) =
                check_actual_projected_source_fields(&mut db, &seal, owner, iterator, item)
                    .unwrap();
            assert_eq!(count, 2, "actual MaybeItem contains elem and none");
            let again = ready(db.fetch(&SyntaxProvider, query));
            assert!(
                first.cacheable && Arc::ptr_eq(&first.value, &again.value),
                "projected fields reuse one retained source plan"
            );
            assert_eq!(first.fingerprint, again.fingerprint);
            current.require_current(&db).unwrap();
            assert_cold(&db, before);
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(
                current.require_current(&db).is_err(),
                "source withdrawal revokes projected field metadata"
            );
            worlds += 1;
            encoded += count;
            eprintln!(
                "actual Iterator::Item source projection edition{edition},origin{origin}: {count}fields; canonical trait/item IDs, memo/currentness/revocation; semantics/root/main held"
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
        }
    }
    assert_eq!((worlds, encoded), (12, 24));
}

#[test]
fn unchanged_repr128_source_factory_publishes_actual_query_domains() {
    use crate::traits::{
        term::TyTerm,
        upstream::{check_source_factory, source_definition_headers::DefinitionHeaderKey},
    };

    // One unchanged owning-edition source world exercises actual factory
    // publication. Broader source-origin/edition and runtime matrices remain
    // separate; this control does not evaluate a replacement solver root.
    let (mut db, seal, root, temporary) = fixture_world(
        "tests/ui/enum-discriminant/repr128.rs",
        "use std::intrinsics",
        2015,
        0,
    );
    assert!(temporary.is_none());
    let rows = actual_rows(&mut db, &seal);
    let main = actual_named(&rows, root.crate_key, &[], "main", ItemKind::Function);
    let discr = actual_named(&rows, root.crate_key, &[], "discr", ItemKind::Function);
    let signed = actual_named(&rows, root.crate_key, &[], "Signed", ItemKind::Enum);
    let unsigned = actual_named(&rows, root.crate_key, &[], "Unsigned", ItemKind::Enum);
    let core = crate::facade::unit(
        &ProfileIdentity {
            edition: 2015,
            ..ProfileIdentity::current()
        },
        "core",
    );
    let partial_eq = actual_named(&rows, core, &["cmp"], "PartialEq", ItemKind::Trait);
    let membership_key = QueryKey::ImplTraitCatalog(seal.clone());
    let membership = ready(db.fetch(&SyntaxProvider, membership_key.clone()));
    let QueryValue::ImplTraitCatalog(catalog) = membership.value.as_ref() else {
        unreachable!("actual source candidate catalog")
    };
    let expected_candidates = catalog
        .candidates(partial_eq)
        .expect("complete actual PartialEq source membership")
        .len();
    assert!(expected_candidates >= 2);
    let header_key = QueryKey::DefinitionHeaderPlan(Box::new(DefinitionHeaderKey {
        inventory: seal.clone(),
        definition: main,
    }));
    let header = ready(db.fetch(&SyntaxProvider, header_key.clone()));
    db.trace.detailed = true;
    let before = db.trace.events.len();
    let input_types = [
        TyTerm::Nominal(signed, Vec::new()),
        TyTerm::Nominal(unsigned, Vec::new()),
    ];
    let (current, observed) = check_source_factory(
        &mut db,
        &seal,
        main,
        discr,
        partial_eq,
        &input_types,
    )
    .unwrap();
    eprintln!("actual repr128 source factory observation before assertions: {observed:?}");
    assert!(observed.definitions >= 5);
    assert!(observed.traits >= 1);
    assert_eq!(observed.implementations, expected_candidates);
    assert_eq!(observed.covered_buckets, 1);
    assert!(observed.impl_empty_clauses >= 2);
    assert!(observed.impl_super_outlives >= 2);
    assert!(observed.adts >= 2);
    assert!(observed.enum_reprs >= 2);
    let mut input_enum_reprs = observed.input_enum_reprs.clone();
    input_enum_reprs.sort_unstable();
    let mut expected_enum_reprs = vec![signed, unsigned];
    expected_enum_reprs.sort_unstable();
    assert_eq!(input_enum_reprs, expected_enum_reprs);
    assert!(
        observed.root_held,
        "coarse readiness stays held for unprepared query domains"
    );
    current.require_current(&db).unwrap();
    let membership_again = ready(db.fetch(&SyntaxProvider, membership_key));
    let header_again = ready(db.fetch(&SyntaxProvider, header_key));
    assert!(membership.cacheable && Arc::ptr_eq(&membership.value, &membership_again.value));
    assert_eq!(membership.fingerprint, membership_again.fingerprint);
    assert!(header.cacheable && Arc::ptr_eq(&header.value, &header_again.value));
    assert_eq!(header.fingerprint, header_again.fingerprint);
    assert_cold(&db, before);
    // This is fresh withdrawal, independently of expansion-epoch revocation.
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(current.require_current(&db).is_err());
    eprintln!(
        "actual repr128 source factory edition2015,origin0: {}definitions,{}traits,{}implementations,{}complete bucket,{}null impl clauses,{}derived impl outlives,{}ADTs,{}enum reprs,{}encoding holds; same-context publication/index, actual candidate coverage, memo/cold/fresh-source-withdrawal; root/activation/response/runtime held",
        observed.definitions,
        observed.traits,
        observed.implementations,
        observed.covered_buckets,
        observed.impl_empty_clauses,
        observed.impl_super_outlives,
        observed.adts,
        observed.enum_reprs,
        observed.encoding_holds,
    );
}

#[test]
fn unchanged_discriminant_projection_bound_encodes_actual_partial_eq_all_editions_and_origins() {
    use crate::traits::upstream::source_declaration_clauses::SourceDeclarationClausesKey;
    use crate::traits::upstream::source_predicates::check_actual_projected_source_predicates;
    use crate::traits::upstream::source_trait_semantics::{
        CurrentSourceTraitSemantics, TraitBooleanQuery as Flag, TraitSemantic,
        TraitSemanticsPending,
    };
    use crate::traits::upstream::source_impl_semantics::{
        CurrentSourceImplSemantics, ImplSemantic, ImplSemanticsPending,
    };
    let mut worlds = 0;
    let mut clauses = 0;
    let mut projected = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let (mut db, seal, root, temporary) = fixture_world(
                "tests/ui/enum-discriminant/repr128.rs",
                "use std::intrinsics",
                edition,
                origin,
            );
            let rows = actual_rows(&mut db, &seal);
            let owner = actual_named(&rows, root.crate_key, &[], "discr", ItemKind::Function);
            let core = crate::facade::unit(
                &ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
                "core",
            );
            let discriminant = actual_named(
                &rows,
                core,
                &["marker"],
                "DiscriminantKind",
                ItemKind::Trait,
            );
            let item = actual_trait_item(&rows, discriminant, "Discriminant");
            let partial_eq = actual_named(&rows, core, &["cmp"], "PartialEq", ItemKind::Trait);
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let mut trait_semantics = Vec::new();
            // The real upstream builtin and impl search distinguish these
            // actual declarations. A spelling alone cannot supply their flags.
            for (name, auto, safety, coinductive, deny, force_dyn) in [
                ("DiscriminantKind", false, false, false, true, true),
                ("Sized", false, false, true, true, true),
                ("MetaSized", false, false, true, true, false),
                ("PointeeSized", false, false, true, true, false),
                ("Send", true, true, true, false, false),
                ("Sync", true, true, true, false, false),
                ("Unpin", true, false, true, false, false),
            ] {
                let id = actual_named(&rows, core, &["marker"], name, ItemKind::Trait);
                let key = crate::traits::upstream::source_definition_headers::DefinitionHeaderKey {
                    inventory: seal.clone(), definition: id,
                };
                let current = ready(CurrentSourceTraitSemantics::capture(&mut db, &key));
                let plan = current.plan();
                assert_eq!(plan.pending(), None, "actual {name} semantic capture");
                for (flag, expected) in [(Flag::Auto,auto),(Flag::Unsafe,safety),
                    (Flag::Coinductive,coinductive),(Flag::DenyExplicitImpl,deny),
                    (Flag::Alias,false),(Flag::Marker,false),
                    (Flag::DefaultTrait,false),
                    (Flag::SizednessTrait,matches!(name,"Sized"|"MetaSized")),
                    (Flag::Fundamental,matches!(name,"Sized"|"MetaSized"|"PointeeSized"))] {
                    assert_eq!(plan.boolean(flag), TraitSemantic::Known(expected),
                        "actual {name} {flag:?}");
                }
                if force_dyn {
                    assert!(matches!(plan.force_dyn_incompatible(), TraitSemantic::Known(Some(_))));
                    assert_eq!(plan.dyn_compatible(), TraitSemantic::Known(false));
                } else {
                    assert_eq!(plan.force_dyn_incompatible(), TraitSemantic::Known(None));
                    assert_eq!(plan.dyn_compatible(),
                        TraitSemantic::Pending(TraitSemanticsPending::DynCompatibility));
                }
                let query = QueryKey::SourceTraitSemantics(Box::new(key));
                let first = ready(db.fetch(&SyntaxProvider, query.clone()));
                let again = ready(db.fetch(&SyntaxProvider, query));
                assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                assert_eq!(first.fingerprint, again.fingerprint);
                current.require_current(&db).unwrap();
                trait_semantics.push(current);
            }
            let output = ready(crate::frontend::source_item_bounds::CurrentSourceItemBounds::capture(
                &mut db, &crate::frontend::source_item_bounds::SourceItemBoundsKey {
                    inventory: seal.clone(), definition: item,
                }));
            let written = output.plan().written_recipes().unwrap();
            let expected_output_traits = [
                (&["clone"][..], "Clone"), (&["marker"][..], "Copy"),
                (&["fmt"][..], "Debug"), (&["cmp"][..], "Eq"),
                (&["cmp"][..], "PartialEq"), (&["hash"][..], "Hash"),
                (&["marker"][..], "Send"), (&["marker"][..], "Sync"),
                (&["marker"][..], "Unpin"),
            ].map(|(module,name)| actual_named(&rows,core,module,name,ItemKind::Trait));
            assert_eq!(written.len(), expected_output_traits.len());
            for (recipe, expected) in written.iter().zip(expected_output_traits) {
                assert!(matches!(recipe,
                    crate::traits::predicate::SourcePredicateRecipe::Explicit {
                        result: crate::traits::predicate::PredicateResult::Implemented(reference), ..
                    } if reference.trait_key == expected), "actual associated output bound: {recipe:?}");
            }
            // Observe genuine candidate metadata, not a filtered applicability
            // certificate: the complete bucket remains owned by the catalog.
            let catalog = ready(db.fetch(&SyntaxProvider, QueryKey::ImplTraitCatalog(seal.clone()))).value;
            let QueryValue::ImplTraitCatalog(catalog) = catalog.as_ref() else { unreachable!() };
            let mut primitive_impls = Vec::new();
            for &(raw, canonical) in catalog.candidates(partial_eq).unwrap() {
                let key = crate::traits::header_syntax::ImplHeaderSyntaxKey {
                    inventory: seal.clone(), implementation: raw,
                };
                let head = ready(db.fetch(&SyntaxProvider, QueryKey::ImplHeadTypes(Box::new(key.clone())))).value;
                let QueryValue::ImplHeadTypes(head) = head.as_ref() else { unreachable!() };
                if !matches!(head.result(), crate::frontend::impl_head_types::ImplHeadTypeResult::Resolved {
                    self_type: crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::I128 | crate::ir::op::PrimitiveType::U128), ..
                }) { continue; }
                let current = ready(CurrentSourceImplSemantics::capture(&mut db, &key));
                let plan = current.plan();
                assert_eq!(plan.pending(), None);
                assert_eq!(plan.impl_definition(), ImplSemantic::Known(canonical));
                assert_eq!(plan.trait_definition(), ImplSemantic::Known(partial_eq));
                assert_eq!(plan.polarity(), ImplSemantic::Known(rustc_type_ir::ImplPolarity::Positive));
                assert_eq!(plan.is_default(), ImplSemantic::Known(false));
                assert_eq!(plan.is_const(), ImplSemantic::Known(false));
                assert!(matches!(plan.inferred_outlives_of(), ImplSemantic::Known(rows) if rows.is_empty()));
                match plan.complete_clauses_of() {
                    ImplSemantic::Known(clauses) => {
                        assert_eq!(clauses.parent(), None);
                        assert!(clauses.recipes().is_empty());
                    }
                    other => panic!("actual nongeneric primitive impl clauses: {other:?}"),
                }
                assert_eq!(plan.impl_super_outlives(), ImplSemantic::Pending(ImplSemanticsPending::ElaborationClosure));
                let query = QueryKey::SourceImplSemantics(Box::new(key));
                let first = ready(db.fetch(&SyntaxProvider, query.clone()));
                let again = ready(db.fetch(&SyntaxProvider, query));
                assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                assert_eq!(first.fingerprint, again.fingerprint);
                current.require_current(&db).unwrap();
                primitive_impls.push(current);
            }
            assert_eq!(primitive_impls.len(), 2, "genuine i128/u128 PartialEq declarations");
            let query =
                QueryKey::SourceDeclarationClauses(Box::new(SourceDeclarationClausesKey {
                    inventory: seal.clone(),
                    definition: owner,
                }));
            let first = ready(db.fetch(&SyntaxProvider, query.clone()));
            let (current, count, projections) = check_actual_projected_source_predicates(
                &mut db,
                &seal,
                owner,
                discriminant,
                item,
                partial_eq,
            )
            .unwrap();
            assert_eq!(
                (count, projections),
                (3, 1),
                "actual discr owns Sized(T), Sized(U) and PartialEq<U> on its projected subject"
            );
            let again = ready(db.fetch(&SyntaxProvider, query));
            assert!(
                first.cacheable && Arc::ptr_eq(&first.value, &again.value),
                "projected own clauses reuse one production source bundle"
            );
            assert_eq!(first.fingerprint, again.fingerprint);
            current.require_current(&db).unwrap();
            assert_cold(&db, before);
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(
                current.require_current(&db).is_err(),
                "source withdrawal revokes projected predicate metadata"
            );
            assert!(trait_semantics.iter().all(|query| query.require_current(&db).is_err()));
            assert!(primitive_impls.iter().all(|query| query.require_current(&db).is_err()));
            assert!(output.require_current(&db).is_err());
            worlds += 1;
            clauses += count;
            projected += projections;
            eprintln!(
                "actual DiscriminantKind::Discriminant:PartialEq<U> source projection edition{edition},origin{origin}: {count}own clauses,{projections}projected subjects; canonical trait/item IDs, shared upstream Integer root variable, rigid/foreign-owner/unknown-key refusals, memo/currentness/revocation; root evaluation/main held"
            );
            if let Some(path) = temporary {
                std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
            }
        }
    }
    assert_eq!((worlds, clauses, projected), (12, 36, 12));
}

#[test]
fn unchanged_projection_fixture_trait_members_keep_actual_source_identity_all_editions_and_origins()
{
    use crate::frontend::{
        member_index::associated_name_key,
        trait_associated_types::{TraitAssociatedTypePending, TraitAssociatedTypesKey},
    };
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, first_item, local, trait_name, item_name) in [
            (
                "tests/ui/associated-types/associated-types-conditional-dispatch.rs",
                "use std::marker",
                false,
                "Deref",
                "Target",
            ),
            (
                "tests/ui/associated-types/associated-types-basic.rs",
                "trait Foo",
                true,
                "Foo",
                "T",
            ),
        ] {
            for origin in 0..3 {
                let (mut db, seal, root, temporary) =
                    fixture_world(case, first_item, edition, origin);
                let rows = actual_rows(&mut db, &seal);
                let core = crate::facade::unit(
                    &ProfileIdentity {
                        edition,
                        ..ProfileIdentity::current()
                    },
                    "core",
                );
                let (unit, module): (Digest, &[&str]) = if local {
                    (root.crate_key, &[])
                } else {
                    (core, &["ops"])
                };
                let parent = actual_named(&rows, unit, module, trait_name, ItemKind::Trait);
                let target = actual_trait_item(&rows, parent, item_name);
                let local_impl_targets = rows
                    .iter()
                    .filter(|(_, item)| {
                        item.scope.crate_key == root.crate_key
                            && item.def_key.declared_name == item_name
                            && item.def_key.item_kind == ItemKind::TypeAlias
                            && item.def_key.fingerprint() != rows[&target].def_key.fingerprint()
                    })
                    .map(|(identity, _)| *identity)
                    .collect::<Vec<_>>();
                assert_eq!(
                    local_impl_targets.len(),
                    1,
                    "the unchanged source has one real impl associated declaration"
                );
                assert_ne!(
                    target, local_impl_targets[0],
                    "trait and impl associated declarations retain distinct identities"
                );
                let key = TraitAssociatedTypesKey {
                    inventory: seal,
                    trait_definition: parent,
                };
                let query = QueryKey::TraitAssociatedTypes(Box::new(key.clone()));
                db.trace.detailed = true;
                let before = db.trace.events.len();
                let captured = ready(db.fetch(&SyntaxProvider, query.clone()));
                assert!(captured.cacheable);
                let QueryValue::TraitAssociatedTypes(plan) = captured.value.as_ref() else {
                    panic!("actual associated Type query differs")
                };
                assert_eq!(plan.key(), &key);
                assert_eq!(plan.pending(), None);
                assert_eq!(plan.lookup(associated_name_key(item_name)), Ok(target));
                assert_eq!(
                    plan.lookup(associated_name_key("LrrAbsentAssociatedType")),
                    Err(TraitAssociatedTypePending::ItemUnavailable),
                    "a missing member remains source input pending, never a negative solver answer"
                );
                assert_ne!(
                    target,
                    associated_name_key(item_name),
                    "a legacy name lookup key is never the actual declaration ID"
                );
                let again = ready(db.fetch(&SyntaxProvider, query.clone()));
                assert!(
                    Arc::ptr_eq(&captured.value, &again.value),
                    "the demanded source query retains one immutable member plan"
                );
                assert_eq!(captured.fingerprint, again.fingerprint);
                let guard =
                    ready(db.prepare_input_guard(&SyntaxProvider, &query, captured.fingerprint));
                guard.require_current(&db).unwrap();
                assert_cold(&db, before);
                db.publish_inputs(BTreeMap::new()).unwrap();
                assert!(
                    guard.require_current(&db).is_err(),
                    "source withdrawal revokes associated declaration metadata"
                );
                worlds += 1;
                eprintln!(
                    "actual {trait_name}::{item_name} source identity {case},edition{edition},origin{origin}: distinct trait/impl IDs, missing-item pending, memo/currentness/revocation; equality solving/root/main held"
                );
                if let Some(path) = temporary {
                    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
                }
            }
        }
    }
    assert_eq!(worlds, 24);
}

#[test]
fn unchanged_impl_super_outlives_uses_pinned_elaborator_all_editions_and_origins() {
    use crate::traits::{
        header_syntax::ImplHeaderSyntaxKey,
        term::{ParamKey, TyTerm},
        upstream::source_impl_outlives::{
            CurrentSourceImplOutlives, ImplOutlivesDomain, check_actual_impl_elaboration,
        },
    };
    let mut worlds = 0;
    let mut elaborations = 0;
    let mut outlives = 0;
    let mut typed_holds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            for (case, first_item, kind) in [
                (
                    "tests/ui/enum-discriminant/repr128.rs",
                    "use std::intrinsics",
                    0,
                ),
                (
                    "tests/ui/self/builtin-superkinds-self-type.rs",
                    "use std::sync",
                    1,
                ),
                (
                    "tests/ui/generic-associated-types/streaming_iterator.rs",
                    "use std::fmt",
                    2,
                ),
            ] {
                let (mut db, seal, root, temporary) =
                    fixture_world(case, first_item, edition, origin);
                let rows = actual_rows(&mut db, &seal);
                db.trace.detailed = true;
                let before = db.trace.events.len();
                let inventory = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::PreparedScopeImplHeaders(seal.clone()),
                ))
                .value;
                let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.as_ref() else {
                    unreachable!()
                };
                let mut selected = Vec::new();
                let expected_trait =
                    if kind == 2 {
                        // Select the actual unchanged inherent impl through its
                        // independently captured new member, never a fabricated key.
                        let new =
                            actual_named(&rows, root.crate_key, &[], "new", ItemKind::Function);
                        let attached = super::upstream_generated_projection_fields::actual_rows(
                            &mut db, &seal,
                        );
                        let parent = super::upstream_associated_predicates::parent(&attached, new);
                        for raw in inventory.inventory_root().headers() {
                            if inventory
                                .canonical_owner(raw.identity)
                                .is_some_and(|owner| owner.identity() == parent)
                            {
                                selected.push((raw.identity, parent));
                            }
                        }
                        assert_eq!(selected.len(), 1, "actual StreamEnumerate inherent impl");
                        None
                    } else {
                        let target = if kind == 0 {
                            let core = crate::facade::unit(
                                &ProfileIdentity {
                                    edition,
                                    ..ProfileIdentity::current()
                                },
                                "core",
                            );
                            actual_named(&rows, core, &["cmp"], "PartialEq", ItemKind::Trait)
                        } else {
                            actual_named(&rows, root.crate_key, &[], "Foo", ItemKind::Trait)
                        };
                        let catalog = ready(
                            db.fetch(&SyntaxProvider, QueryKey::ImplTraitCatalog(seal.clone())),
                        )
                        .value;
                        let QueryValue::ImplTraitCatalog(catalog) = catalog.as_ref() else {
                            unreachable!()
                        };
                        for &(raw, canonical) in catalog.candidates(target).unwrap() {
                            if kind == 0 {
                                let head = ready(db.fetch(
                                    &SyntaxProvider,
                                    QueryKey::ImplHeadTypes(Box::new(ImplHeaderSyntaxKey {
                                        inventory: seal.clone(),
                                        implementation: raw,
                                    })),
                                ))
                                .value;
                                let QueryValue::ImplHeadTypes(head) = head.as_ref() else {
                                    unreachable!()
                                };
                                if !matches!(head.result(),
                                crate::frontend::impl_head_types::ImplHeadTypeResult::Resolved {
                                    self_type: TyTerm::Primitive(crate::ir::op::PrimitiveType::I128
                                        | crate::ir::op::PrimitiveType::U128), ..
                                }) {
                                    continue;
                                }
                            } else if rows
                                .get(&canonical)
                                .is_none_or(|item| item.scope.crate_key != root.crate_key)
                            {
                                continue;
                            }
                            selected.push((raw, canonical));
                        }
                        assert_eq!(selected.len(), if kind == 0 { 2 } else { 1 });
                        Some(target)
                    };
                let mut held = Vec::new();
                let mut local_elaborations = 0;
                let mut local_outlives = 0;
                let mut local_holds = 0;
                for (raw, canonical) in selected {
                    let key = ImplHeaderSyntaxKey {
                        inventory: seal.clone(),
                        implementation: raw,
                    };
                    let query = QueryKey::SourceImplOutlives(Box::new(key.clone()));
                    let first = ready(db.fetch(&SyntaxProvider, query.clone()));
                    let current = ready(CurrentSourceImplOutlives::capture(&mut db, &key));
                    let plan = current.plan();
                    assert_eq!(plan.key(), &key);
                    if let Some(expected_trait) = expected_trait {
                        assert_eq!(
                            plan.domain(),
                            ImplOutlivesDomain::TraitImpl {
                                implementation: canonical,
                                trait_definition: expected_trait,
                            }
                        );
                        assert!(
                            plan.requirements().is_empty(),
                            "actual source requirements: {:?}",
                            plan.requirements()
                        );
                        let observed = check_actual_impl_elaboration(&mut db, &key).unwrap();
                        assert_eq!(observed.implementation, canonical);
                        assert_eq!(observed.trait_definition, expected_trait);
                        assert_eq!(observed.source, plan.fingerprint());
                        if kind == 0 {
                            assert_eq!(
                                observed.outlives, 0,
                                "pinned filtering of primitive PartialEq graph"
                            );
                            assert!(observed.static_type_parameters.is_empty());
                        } else {
                            let generics = ready(
                                db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(canonical)),
                            )
                            .value;
                            let QueryValue::DeclarationGenerics(generics) = generics.as_ref()
                            else {
                                unreachable!()
                            };
                            assert_eq!(generics.parameters().len(), 1);
                            assert_eq!(generics.parameters()[0].name, "T");
                            assert_eq!(observed.outlives, 1, "actual Foo requires T:'static");
                            assert_eq!(
                                observed.static_type_parameters,
                                vec![ParamKey {
                                    definition: canonical,
                                    ordinal: generics.parameters()[0].ordinal,
                                }]
                            );
                        }
                        local_elaborations += 1;
                        local_outlives += observed.outlives;
                    } else {
                        assert_eq!(
                            plan.domain(),
                            ImplOutlivesDomain::NotApplicable,
                            "inherent impl is not a trait-impl elaboration input"
                        );
                        assert_eq!(plan.pending(), None);
                        local_holds += 1;
                    }
                    let again = ready(db.fetch(&SyntaxProvider, query));
                    assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                    assert_eq!(first.fingerprint, again.fingerprint);
                    current.require_current(&db).unwrap();
                    held.push(current);
                }
                assert_cold(&db, before);
                db.begin_expansion_epoch(2).unwrap();
                assert!(held.iter().all(|plan| plan.require_current(&db).is_err()));
                db.publish_inputs(BTreeMap::new()).unwrap();
                assert!(held.iter().all(|plan| plan.require_current(&db).is_err()));
                worlds += 1;
                elaborations += local_elaborations;
                outlives += local_outlives;
                typed_holds += local_holds;
                eprintln!(
                    "actual pinned impl-super-outlives {case},edition{edition},origin{origin}: {local_elaborations}elaborations,{local_outlives}outlives,{local_holds}typed-domain-holds; canonical source IDs, actual static parameter, memo/currentness/epoch revocation, bodies cold; root/runtime held"
                );
                if let Some(path) = temporary {
                    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
                }
            }
        }
    }
    assert_eq!(
        (worlds, elaborations, outlives, typed_holds),
        (36, 36, 12, 12)
    );
    check_actual_clause_emission_all_editions_and_origins();
}

#[test]
fn unchanged_impl_anonymous_regions_retain_sites_order_and_associated_inheritance() {
    use crate::traits::{term::BoundKind, upstream::source_terms::check_anonymous_impl_regions};
    // All three original files have the default owning Rust 2015 edition. The
    // last is a diagnostic fixture: only actual source metadata is observed.
    for (case, module, written, anonymous, upstream_ordinals, method) in [
        ("tests/ui/impl-header-lifetime-elision/trait-underscore.rs", &[][..], &[][..], 2, &[0, 1][..], None),
        ("tests/ui/impl-header-lifetime-elision/inherent-impl.rs", &[][..], &[][..], 1, &[0][..], Some("x")),
        ("tests/ui/delegation/generics/infer-defaults.rs", &["free_to_trait"][..],
            &[("XX", BoundKind::Type), ("Y", BoundKind::Type), ("T", BoundKind::Type), ("N", BoundKind::Const)][..],
            1, &[4, 0, 1, 2, 3][..], None),
    ] {
        let (mut db, seal, scope, temporary) = fixture_world(case, "", 2015, 0);
        assert!(temporary.is_none(), "unchanged original source path");
        let member = method.map(|name| actual_named(&actual_rows(&mut db, &seal), scope.crate_key,
            module, name, ItemKind::Function));
        db.trace.detailed = true;
        let before = db.trace.events.len();
        let count = check_anonymous_impl_regions(&mut db, &seal, scope.crate_key, module,
            written, anonymous, upstream_ordinals, member)
            .unwrap_or_else(|error| panic!("actual anonymous impl source {case}: {error:?}"));
        assert_eq!(count, anonymous);
        assert_cold(&db, before);
        eprintln!("unchanged anonymous impl source {case},edition2015,origin0: {count} actual anonymous sites; original main/compiler diagnostics unexecuted");
    }
}
