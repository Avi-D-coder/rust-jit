//! Source metadata controls around unchanged local associated field fixtures.
//! Generated canonical IDs come from publication, never from a name digest.
use super::{configured_impl_catalog_world, temporary_source};
use crate::{
    core::{digest::{Digest, StableEncode}, profile::ProfileIdentity},
    db::{dependency::{InputKey, InputValue}, key::QueryKey, memo::Database,
        poll::QueryPoll, provider::SyntaxProvider, seals::SealKey, value::QueryValue},
    frontend::{member_index::associated_name_key,
        trait_associated_types::{TraitAssociatedTypesKey, TraitAssociatedTypePending}},
    source::manifest::ItemKind,
    syntax::{outline::ItemOutline, parse::ParseKey},
    traits::{term::ParamKey, upstream::source_adt_fields::{
        AdtSourceFieldsKey, DeclaredProjectionFieldExpectation, ProjectionSelfExpectation,
        check_actual_declared_projection_field,
    }},
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual local projected field source: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual local projected field input: {input:?}"),
    }
}
pub(super) struct SourceRow { pub(super) item: ItemOutline, pub(super) parse: ParseKey, pub(super) generated: bool }
pub(super) fn actual_rows(db: &mut Database, seal: &SealKey) -> BTreeMap<Digest, SourceRow> {
    let inventory = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone()))).value;
    let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.as_ref() else { panic!("actual source inventory differs") };
    let environment = inventory.inventory_root().environment().expect("completed actual environment");
    let mut rows = BTreeMap::new();
    for source in environment.sources() {
        for item in &source.outline().items {
            rows.insert(item.def_key.fingerprint(), SourceRow {
                item: item.clone(), parse: source.selection().outline.parse.clone(), generated: false,
            });
        }
    }
    for unit in environment.generated_publication_for_control().0.iter() {
        for &identity in unit.definitions.keys() {
            rows.insert(identity, SourceRow {
                item: unit.declaration(identity).expect("actual generated declaration").clone(),
                parse: unit.key.clone(), generated: true,
            });
        }
    }
    rows
}
pub(super) fn named(rows: &BTreeMap<Digest, SourceRow>, unit: Digest, module: &[&str], name: &str, kind: ItemKind) -> Digest {
    let found = rows.iter().filter(|(_, row)| row.item.scope.crate_key == unit
        && row.item.scope.module_path.iter().map(String::as_str).eq(module.iter().copied())
        && row.item.def_key.declared_name == name && row.item.def_key.item_kind == kind)
        .map(|(identity, _)| *identity).collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "actual source {module:?}::{name} must be unique");
    found[0]
}
pub(super) fn associated(rows: &BTreeMap<Digest, SourceRow>, parent: Digest, name: &str) -> Digest {
    let owner = &rows[&parent];
    assert_eq!(owner.item.def_key.item_kind, ItemKind::Trait);
    let mut scope = owner.item.scope.clone();
    // Outline's auxiliary scope attachment is not a generated canonical ID.
    scope.owner = Some(owner.item.def_key.fingerprint());
    let found = rows.iter().filter(|(_, row)| row.parse == owner.parse
        && row.item.scope == scope && row.item.def_key.item_kind == ItemKind::TypeAlias
        && row.item.def_key.declared_name == name
        && owner.item.declaration_span.start <= row.item.declaration_span.start
        && row.item.declaration_span.end <= owner.item.declaration_span.end)
        .map(|(identity, _)| *identity).collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "actual trait associated declaration must be unique");
    found[0]
}
pub(super) fn assert_cold(db: &Database, before: usize) {
    assert!(db.trace.events[before..].iter().all(|event| !matches!(event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_)
        | QueryKey::Thunk(_) | QueryKey::PatternPlan(_))),
        "declared projected fields must leave executable preparation cold");
}

fn check_fixture(case: &str, first_item: &str, owner_name: &str, trait_name: &str,
    item_name: &str, field_name: &str, nominal_self: Option<&str>, boxed: bool) {
    let original_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout").join(case);
    let original = std::fs::read_to_string(&original_path).unwrap();
    let offset = original.find(first_item).expect("unchanged actual first item");
    let mut worlds = 0;
    let mut fields = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for origin in 0..3 {
            let temporary = if origin == 0 { None } else {
                let macros = if origin == 1 { "macro_rules! emit {($($t:tt)*)=>{$($t)*}} emit!" }
                    else { "macro_rules! emit {($($t:tt)*)=>{$($t)*}} macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!" };
                Some(temporary_source(&format!("local-projection-{owner_name}-{edition}-{origin}"),
                    &format!("{}{}{{{}}}", &original[..offset], macros, &original[offset..])))
            };
            let (mut db, seal, root) = configured_impl_catalog_world(
                temporary.as_ref().unwrap_or(&original_path), edition).unwrap_or_else(|error|
                panic!("actual projected field world {case},edition{edition},origin{origin}: {error:?}"));
            let rows = actual_rows(&mut db, &seal);
            let owner = named(&rows, root.crate_key, &[], owner_name, ItemKind::Struct);
            let parent = named(&rows, root.crate_key, &[], trait_name, ItemKind::Trait);
            let item = associated(&rows, parent, item_name);
            for identity in [owner, parent, item] {
                assert_eq!(rows[&identity].generated, origin != 0);
                assert!(matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),
                    Some(InputValue::DefinitionSource(parse)) if parse == &rows[&identity].parse));
            }
            let impl_members = rows.iter().filter(|(_, row)| row.item.scope.crate_key == root.crate_key
                && row.item.def_key.item_kind == ItemKind::TypeAlias
                && row.item.def_key.declared_name == item_name
                && row.item.def_key.fingerprint() != rows[&item].item.def_key.fingerprint())
                .map(|(identity, _)| *identity).collect::<Vec<_>>();
            assert_eq!(impl_members.len(), 1);
            assert_ne!(item, impl_members[0], "actual trait and impl associated IDs must differ");
            assert_ne!(item, associated_name_key(item_name));
            let self_type = nominal_self.map_or(ProjectionSelfExpectation::Parameter(ParamKey {
                definition: owner, ordinal: 0,
            }), |name| ProjectionSelfExpectation::Nominal(named(&rows, root.crate_key, &[], name, ItemKind::Struct)));
            let wrappers = if boxed {
                let alloc = crate::facade::unit(&ProfileIdentity { edition, ..ProfileIdentity::current() }, "alloc");
                vec![named(&rows, alloc, &["boxed"], "Box", ItemKind::Struct)]
            } else { Vec::new() };
            let arguments = if boxed { vec![rustc_type_ir::UintTy::Usize] } else { Vec::new() };
            let expected = DeclaredProjectionFieldExpectation {
                kind: crate::traits::upstream::model::AdtKind::Struct,
                field_name, wrappers: &wrappers, trait_definition: parent, associated_definition: item,
                self_type, unsigned_trait_arguments: &arguments, own_arguments: &[], parent_count: 1 + arguments.len(),
            };
            db.trace.detailed = true;
            let before = db.trace.events.len();
            let member_query = QueryKey::TraitAssociatedTypes(Box::new(TraitAssociatedTypesKey {
                inventory: seal.clone(), trait_definition: parent,
            }));
            let member = ready(db.fetch(&SyntaxProvider, member_query.clone()));
            let QueryValue::TraitAssociatedTypes(plan) = member.value.as_ref() else { panic!("actual associated member query differs") };
            assert_eq!(plan.pending(), None);
            assert_eq!(plan.lookup(associated_name_key(item_name)), Ok(item));
            assert_eq!(plan.lookup(associated_name_key("LrrAbsentAssociatedType")), Err(TraitAssociatedTypePending::ItemUnavailable));
            let trait_query = QueryKey::TraitDeclaration(parent);
            let trait_receipt = ready(db.fetch(&SyntaxProvider, trait_query.clone()));
            let trait_guard = ready(db.prepare_input_guard(&SyntaxProvider, &trait_query, trait_receipt.fingerprint));
            let field_query = QueryKey::AdtSourceFields(Box::new(AdtSourceFieldsKey {
                inventory: seal.clone(), definition: owner,
            }));
            let first = ready(db.fetch(&SyntaxProvider, field_query.clone()));
            let (current, count) = check_actual_declared_projection_field(&mut db, &seal, owner, &expected)
                .unwrap_or_else(|error| panic!("actual projected field {case},edition{edition},origin{origin}: {error:?}"));
            assert_eq!(count, 1);
            let again = ready(db.fetch(&SyntaxProvider, field_query));
            assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
            assert_eq!(first.fingerprint, again.fingerprint);
            let member_again = ready(db.fetch(&SyntaxProvider, member_query.clone()));
            assert!(member.cacheable && Arc::ptr_eq(&member.value, &member_again.value));
            assert_eq!(member.fingerprint, member_again.fingerprint);
            let member_guard = ready(db.prepare_input_guard(&SyntaxProvider, &member_query, member.fingerprint));
            current.require_current(&db).unwrap();
            member_guard.require_current(&db).unwrap();
            assert_cold(&db, before);
            db.begin_expansion_epoch(2).unwrap();
            assert!(current.require_current(&db).is_err());
            assert!(member_guard.require_current(&db).is_err());
            if rows[&parent].generated {
                assert!(trait_guard.require_current(&db).is_err(),
                    "the retained generated trait member-scope receipt must retain its real epoch dependencies");
            }
            db.publish_inputs(BTreeMap::new()).unwrap();
            assert!(current.require_current(&db).is_err());
            assert!(member_guard.require_current(&db).is_err());
            worlds += 1;
            fields += count;
            eprintln!("actual local {trait_name}::{item_name} field {case},edition{edition},origin{origin}: {count}field; actual alias/parent/Self/arguments, memo/currentness/withdrawal/cold; solver/runtime held");
            if let Some(path) = temporary { std::fs::remove_dir_all(path.parent().unwrap()).unwrap(); }
        }
    }
    assert_eq!((worlds, fields), (12, 12));
}

#[test]
fn unchanged_local_associated_field_encodes_actual_generated_declaration_all_editions_and_origins() {
    check_fixture("tests/ui/associated-types/assoc-type-in-struct-constructor.rs", "trait Tr",
        "S", "Tr", "T", "0", None, false);
}
#[test]
fn unchanged_generic_trait_associated_field_encodes_actual_generated_parent_arguments_all_editions_and_origins() {
    check_fixture("tests/ui/packed/packed-with-inference-vars-issue-61402.rs", "pub struct S",
        "PackedAssocSized", "Trait", "Assoc", "pos", Some("S"), true);
}
