//! Host metadata controls around unchanged imported source declarations.
//! These do not enter main, publish complete solver facts or prove a runtime.
use super::{configured_impl_catalog_world, temporary_source};
use crate::{
    core::{digest::{Digest, StableEncode}, profile::ProfileIdentity},
    db::{key::QueryKey, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    frontend::names::NameLookup,
    source::manifest::{ItemKind, Namespace},
    syntax::{imports::ImportSegment, outline::ItemOutline, parse::ParseKey},
    traits::upstream::{
        source_definition_headers::{DefinitionHeaderKey, DefinitionHeaderRole as Role,
            check_actual_definition_headers},
        source_lang_identity::{CurrentSourceLangIdentity, LangIdentityOrigin},
    },
};
use std::{collections::BTreeMap, path::PathBuf};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual header metadata preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual header metadata needs source input: {input:?}"),
    }
}
#[derive(Clone)]
struct SourceRow { identity: Digest, item: ItemOutline, parse: ParseKey }

/// Expectations come from independent actual Outline owner/scope attachments;
/// the production DefinitionHeaderPlan is never consulted to choose a parent.
fn associated_parent(row: &SourceRow, rows: &BTreeMap<Digest, SourceRow>) -> Option<Digest> {
    let owners = rows.values().filter(|owner| {
        if owner.parse != row.parse || !matches!(owner.item.def_key.item_kind, ItemKind::Trait | ItemKind::Impl) {
            return false;
        }
        let mut scope = owner.item.scope.clone();
        scope.owner = Some(if owner.item.def_key.item_kind == ItemKind::Impl {
            owner.item.def_key.enclosing_impl.expect("actual impl member owner")
        } else { owner.item.def_key.fingerprint() });
        scope == row.item.scope
            && owner.item.declaration_span.start <= row.item.declaration_span.start
            && row.item.declaration_span.end <= owner.item.declaration_span.end
    }).map(|owner| owner.identity).collect::<Vec<_>>();
    assert!(owners.len() <= 1, "actual source member has ambiguous owner attachments");
    owners.first().copied()
}

#[test]
fn unchanged_declarations_prepare_actual_header_inputs_all_editions_and_origins() {
    let originals = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout");
    let mut worlds = 0;
    let mut headers_checked = 0;
    let mut facets_checked = 0;
    let mut facet_gaps = Vec::new();
    for edition in [2015, 2018, 2021, 2024] {
        for (case, first_item, expected_count) in [
            ("tests/ui/generics/generic-derived-type.rs", "fn g", 4),
            ("tests/ui/methods/method-self-arg-trait.rs", "static mut COUNT", 16),
            ("tests/ui/const-generics/array-wrapper-struct-ctor.rs", "struct ArrayStruct", 3),
        ] {
            let original_path = originals.join(case);
            let original = std::fs::read_to_string(&original_path).unwrap();
            let offset = original.find(first_item).expect("actual unchanged first item");
            for origin in 0..3 {
                // Original files are never edited. Wrappers retain their exact
                // item bytes after the original inner attributes/directives.
                let path = if origin == 0 { original_path.clone() } else {
                    let macros = if origin == 1 {
                        "macro_rules! emit {($($t:tt)*)=>{$($t)*}} emit!"
                    } else {
                        "macro_rules! emit {($($t:tt)*)=>{$($t)*}} macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!"
                    };
                    temporary_source(&format!("definition-header-{edition}-{origin}-{expected_count}"),
                        &format!("{}{}{{{}}}", &original[..offset], macros, &original[offset..]))
                };
                let (mut db, seal, scope) = configured_impl_catalog_world(&path, edition)
                    .unwrap_or_else(|error| panic!("actual header world {case}, edition{edition}, origin{origin}: {error:?}"));
                let headers = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone()))).value;
                let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { panic!("actual header inventory differs") };
                let environment = headers.inventory_root().environment().expect("actual completed source environment");
                let mut rows = BTreeMap::new();
                for source in environment.sources().iter().filter(|source| source.outline().root_scope.crate_key == scope.crate_key) {
                    for item in &source.outline().items {
                        let identity = item.def_key.fingerprint();
                        rows.insert(identity, SourceRow { identity, item: item.clone(), parse: source.selection().outline.parse.clone() });
                    }
                }
                if origin != 0 {
                    for unit in environment.generated_publication_for_control().0.iter().filter(|unit| unit.scope.crate_key == scope.crate_key) {
                        for &identity in unit.definitions.keys() {
                            let item = unit.declaration(identity).expect("actual generated declaration location");
                            rows.insert(identity, SourceRow { identity, item: item.clone(), parse: unit.key.clone() });
                        }
                    }
                }
                let wanted: &[(&str, Role)] = if case.contains("generic-derived-type") {
                    &[("Pair", Role::Struct), ("g", Role::Function), ("f", Role::Function), ("main", Role::Function)]
                } else if case.contains("array-wrapper") {
                    &[("ArrayStruct", Role::Struct), ("ArrayTuple", Role::Struct), ("main", Role::Function)]
                } else {
                    &[("Foo", Role::Struct), ("Bar", Role::Trait), ("COUNT", Role::Static), ("main", Role::Function),
                        ("foo1", Role::Function), ("foo2", Role::Function), ("foo3", Role::Function),
                        ("bar1", Role::Function), ("bar2", Role::Function), ("bar3", Role::Function), ("baz", Role::Function)]
                };
                let mut selected = BTreeMap::new();
                for &(name, role) in wanted {
                    let matches = rows.values().filter(|row| row.item.def_key.declared_name == name).collect::<Vec<_>>();
                    assert!(!matches.is_empty(), "unchanged source declaration {name} missing in {case}, edition{edition}, origin{origin}");
                    for row in matches {
                        let parent = associated_parent(row, &rows);
                        selected.insert(row.identity, (row.identity, role, Some(name), parent));
                        if let Some(parent) = parent {
                            let owner = &rows[&parent];
                            match owner.item.def_key.item_kind {
                                ItemKind::Trait => {}, // Selected Bar already has its named expectation.
                                ItemKind::Impl => { selected.insert(parent, (parent, Role::Impl, None, None)); },
                                _ => panic!("actual associated source owner is not a trait/impl"),
                            }
                        }
                    }
                }
                assert_eq!(selected.len(), expected_count, "actual source declaration coverage, {case}, edition{edition}, origin{origin}");
                db.trace.detailed = true;
                let before = db.trace.events.len();
                let expected = selected.values().copied().collect::<Vec<_>>();
                let (mut held, count) = check_actual_definition_headers(&mut db, &seal, &expected)
                    .unwrap_or_else(|error| panic!("actual source header controls {case}, edition{edition}, origin{origin}: {error:?}"));
                assert_eq!(count, expected_count);
                headers_checked += count;
                let mut lang_guards = Vec::new();
                if case.contains("generic-derived-type") {
                    let pair = selected.values().find(|row| row.2 == Some("Pair")).unwrap().0;
                    let ordinary = ready(CurrentSourceLangIdentity::capture(&mut db, &DefinitionHeaderKey { inventory: seal.clone(), definition: pair }));
                    assert_eq!(ordinary.plan().classification(), Ok(None), "an ordinary actual user struct has no lang role");
                    lang_guards.push(ordinary);
                }
                // Facade source ownership does not change with the root's macro
                // origin, so observe it once per edition rather than repeating
                // the same six dependency records in every fixture world.
                if case.contains("generic-derived-type") && origin == 0 {
                    let core_unit = crate::facade::unit(&ProfileIdentity { edition, ..ProfileIdentity::current() }, "core");
                    for (module, name, role, compiler_name) in [
                    ("marker", "Copy", Role::Trait, "copy"), ("marker", "Sized", Role::Trait, "sized"),
                    ("clone", "Clone", Role::Trait, "clone"), ("ops", "Fn", Role::Trait, "fn"),
                    ("marker", "FnPtr", Role::Trait, "fn_ptr_trait"), ("pin", "Pin", Role::Struct, "pin"),
                ] {
                    let found = environment.sources().iter().filter(|source| source.outline().root_scope.crate_key == core_unit)
                        .flat_map(|source| &source.outline().items)
                        .filter(|item| item.scope.module_path.as_slice() == [module] && item.def_key.declared_name == name)
                        .collect::<Vec<_>>();
                    if found.is_empty() {
                        let gap = format!("edition{edition},origin{origin},{case}:missing core::{module}::{name}");
                        eprintln!("source lang-identity GAP {gap}"); facet_gaps.push(gap); continue;
                    }
                    assert_eq!(found.len(), 1, "actual facade declaration must be unambiguous");
                    let definition = found[0].def_key.fingerprint();
                    let key = DefinitionHeaderKey { inventory: seal.clone(), definition };
                    let classified = ready(CurrentSourceLangIdentity::capture(&mut db, &key));
                    let query = QueryKey::SourceLangIdentity(Box::new(key.clone()));
                    let first = ready(db.fetch(&SyntaxProvider, query.clone())).value;
                    let again = ready(db.fetch(&SyntaxProvider, query)).value;
                    assert!(std::sync::Arc::ptr_eq(&first, &again), "per-definition lang identity shares its immutable source memo");
                    classified.require_current(&db).unwrap();
                    assert_eq!(classified.plan().classification(), Ok(Some((compiler_name, LangIdentityOrigin::EmbeddedFacet))),
                        "actual core::{module}::{name} lang identity");
                    let (facet_held, count) = check_actual_definition_headers(&mut db, &seal, &[(definition, role, Some(name), None)]).unwrap();
                    assert_eq!(count, 1); held.extend(facet_held); facets_checked += 1; lang_guards.push(classified);
                    }
                }
                assert!(db.trace.events[before..].iter().all(|event| !matches!(event.key,
                    QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_))),
                    "source header/lang demand must leave executable code cold");
                db.publish_inputs(BTreeMap::new()).unwrap();
                assert!(held.iter().all(|header| header.require_current(&db).is_err()));
                assert!(lang_guards.iter().all(|lang| lang.require_current(&db).is_err()));
                worlds += 1;
                eprintln!("actual declaration headers {case},edition{edition},origin{origin}: {count}source rows, memo/currentness/revocation, semantics/root/main held");
                if origin != 0 { std::fs::remove_dir_all(path.parent().unwrap()).unwrap(); }
            }
        }
    }
    assert_eq!((worlds, headers_checked), (36, 276));
    assert!(facets_checked > 0, "some real facade role evidence is required");
    eprintln!("actual header source matrix: {worlds}worlds,{headers_checked}root headers,{facets_checked}facet headers,{}explicit facade gaps", facet_gaps.len());
}

#[test]
fn source_lang_attributes_and_aliases_preserve_actual_definition_identity() {
    let original = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-derived-type.rs")).unwrap();
    for edition in [2015, 2018, 2021, 2024] {
        // Supplemental source metadata control; the imported original stays
        // unchanged. No rustc diagnostics or guest runtime are asserted here.
        let path = temporary_source(&format!("definition-lang-alias-{edition}"),
            &format!("{original}\nextern crate core as LrrMetadataCore;\nuse LrrMetadataCore::marker::Copy as LrrCopyAlias;\n#[lang = \"copy\"] trait LrrSourceCopy {{}}\n"));
        let (mut db, seal, scope) = configured_impl_catalog_world(&path, edition).unwrap();
        let headers = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone()))).value;
        let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { panic!() };
        let environment = headers.inventory_root().environment().unwrap();
        let select = |name: &str| {
            let found = environment.names().lookup_source_path(Namespace::Type,
                &[ImportSegment::Name(name.into())], false, &scope, edition).unwrap();
            let NameLookup::Defined(found) = found else { panic!("actual alias/source lang declaration {name}: {found:?}") };
            found.identity
        };
        let alias = select("LrrCopyAlias");
        let attribute = select("LrrSourceCopy");
        let core_unit = crate::facade::unit(&ProfileIdentity { edition, ..ProfileIdentity::current() }, "core");
        let actual_copy = environment.sources().iter().filter(|source| source.outline().root_scope.crate_key == core_unit)
            .flat_map(|source| &source.outline().items)
            .find(|item| item.scope.module_path.as_slice() == ["marker"] && item.def_key.declared_name == "Copy")
            .expect("actual embedded Copy source declaration").def_key.fingerprint();
        assert_eq!(alias, actual_copy, "the ordinary import alias must retain the target declaration ID");
        assert_ne!(attribute, alias, "source #[lang] classification must not merge declaration identities");
        db.trace.detailed = true; let before = db.trace.events.len();
        let alias = ready(CurrentSourceLangIdentity::capture(&mut db, &DefinitionHeaderKey { inventory: seal.clone(), definition: alias }));
        assert_eq!(alias.plan().classification(), Ok(Some(("copy", LangIdentityOrigin::EmbeddedFacet))));
        let attribute = ready(CurrentSourceLangIdentity::capture(&mut db, &DefinitionHeaderKey { inventory: seal, definition: attribute }));
        assert_eq!(attribute.plan().classification(), Ok(Some(("copy", LangIdentityOrigin::SourceAttribute))));
        alias.require_current(&db).unwrap(); attribute.require_current(&db).unwrap();
        assert!(db.trace.events[before..].iter().all(|event| !matches!(event.key,
            QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_))));
        db.publish_inputs(BTreeMap::new()).unwrap();
        assert!(alias.require_current(&db).is_err()); assert!(attribute.require_current(&db).is_err());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[test]
fn nominal_declared_types_do_not_demand_projected_fields_or_initializers() {
    let originals = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/upstream/rustc-checkout");
    let mut checked = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, names, role) in [
            ("tests/ui/enum-discriminant/repr128.rs", ["Signed", "Unsigned"], Role::Enum),
            ("tests/ui/union/union-generic-rpass.rs", ["MaybeItem", "U"], Role::Union),
        ] {
            let original_path = originals.join(case);
            let text = std::fs::read_to_string(&original_path).unwrap();
            let offset = text.find("use std").unwrap();
            for origin in 0..3 {
                let path = if origin == 0 { original_path.clone() } else {
                    let macros = if origin == 1 {
                        "macro_rules! emit {($($t:tt)*)=>{$($t)*}} emit!"
                    } else {
                        "macro_rules! emit {($($t:tt)*)=>{$($t)*}} macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!"
                    };
                    temporary_source(&format!("nominal-declared-{edition}-{origin}"),
                        &format!("{}{}{{{}}}", &text[..offset], macros, &text[offset..]))
                };
                let (mut db, seal, scope) = configured_impl_catalog_world(&path, edition).unwrap();
                let source = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone()))).value;
                let QueryValue::PreparedScopeImplHeaders(headers) = source.as_ref() else { panic!() };
                let environment = headers.inventory_root().environment().unwrap();
                let mut rows = BTreeMap::new();
                for source in environment.sources().iter().filter(|s| s.outline().root_scope.crate_key == scope.crate_key) {
                    for item in &source.outline().items { rows.insert(item.def_key.fingerprint(), item.clone()); }
                }
                if origin != 0 {
                    for unit in environment.generated_publication_for_control().0.iter().filter(|unit| unit.scope.crate_key == scope.crate_key) {
                        for &identity in unit.definitions.keys() { rows.insert(identity, unit.declaration(identity).unwrap().clone()); }
                    }
                }
                let expected = names.iter().map(|name| {
                    let found = rows.iter().filter(|(_, item)| item.def_key.declared_name == *name
                        && item.def_key.namespace == Namespace::Type).collect::<Vec<_>>();
                    assert_eq!(found.len(), 1, "actual nominal header source {name}");
                    (*found[0].0, role, Some(*name), None)
                }).collect::<Vec<_>>();
                db.trace.detailed = true; let before = db.trace.events.len();
                let (held, count) = check_actual_definition_headers(&mut db, &seal, &expected).unwrap();
                assert_eq!(count, 2);
                assert!(db.trace.events[before..].iter().all(|event| !matches!(event.key,
                    QueryKey::AdtSourceFields(_) | QueryKey::TypeSchema(_) | QueryKey::BodyStructure(_)
                    | QueryKey::FunctionSignature(_) | QueryKey::Thunk(_))),
                    "nominal identity must not demand field projections or discriminant initializers");
                db.publish_inputs(BTreeMap::new()).unwrap();
                assert!(held.iter().all(|plan| plan.require_current(&db).is_err()));
                checked += count;
                if origin != 0 { std::fs::remove_dir_all(path.parent().unwrap()).unwrap(); }
            }
        }
    }
    assert_eq!(checked, 48);
}
