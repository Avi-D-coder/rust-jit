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
