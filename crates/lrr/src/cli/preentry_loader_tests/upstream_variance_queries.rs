//! Actual source query-domain controls, separate from compiler/guest execution.
use super::{configured_impl_catalog_world, temporary_source};
use crate::{
    core::digest::{Digest, StableEncode},
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, seals::SealKey,
        value::QueryValue,
    },
    source::manifest::ItemKind,
    syntax::outline::ItemOutline,
    traits::upstream::{
        source_definition_headers::DefinitionHeaderKey,
        source_variance_queries::{
            VarianceQueryDomain as Domain, VarianceQueryOutcome, check_actual_variance_query,
        },
    },
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual source variance preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual source variance needs input: {input:?}"),
    }
}
fn actual_rows(db: &mut Database, seal: &SealKey, unit: Digest) -> BTreeMap<Digest, ItemOutline> {
    let inventory = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedScopeImplHeaders(seal.clone()),
    ))
    .value;
    let QueryValue::PreparedScopeImplHeaders(headers) = inventory.as_ref() else {
        panic!("actual inventory differs")
    };
    let environment = headers
        .inventory_root()
        .environment()
        .expect("actual completed source environment");
    let mut rows = BTreeMap::new();
    for source in environment
        .sources()
        .iter()
        .filter(|source| source.outline().root_scope.crate_key == unit)
    {
        for item in &source.outline().items {
            rows.insert(item.def_key.fingerprint(), item.clone());
        }
    }
    for generated in environment
        .generated_publication_for_control()
        .0
        .iter()
        .filter(|generated| generated.scope.crate_key == unit)
    {
        for &id in generated.definitions.keys() {
            rows.insert(
                id,
                generated
                    .declaration(id)
                    .expect("actual generated declaration")
                    .clone(),
            );
        }
    }
    rows
}
/// Independent Outline attachments select actual associated parents; generated
/// publication IDs are retained separately from the Outline member-scope ID.
fn parent_kind(row: &ItemOutline, rows: &BTreeMap<Digest, ItemOutline>) -> Option<ItemKind> {
    let found = rows
        .values()
        .filter(|owner| {
            if !matches!(owner.def_key.item_kind, ItemKind::Trait | ItemKind::Impl) {
                return false;
            }
            let mut members = owner.scope.clone();
            members.owner = Some(if owner.def_key.item_kind == ItemKind::Impl {
                owner.def_key.enclosing_impl.expect("actual impl scope")
            } else {
                owner.def_key.fingerprint()
            });
            members == row.scope
                && owner.declaration_span.start <= row.declaration_span.start
                && row.declaration_span.end <= owner.declaration_span.end
        })
        .collect::<Vec<_>>();
    assert!(
        found.len() <= 1,
        "actual associated owner must be unambiguous"
    );
    found.first().map(|owner| owner.def_key.item_kind)
}

#[test]
fn unchanged_source_variance_query_domains_all_editions_and_origins() {
    let originals =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut worlds = 0;
    let mut checks = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, first_item) in [
            ("tests/ui/generics/generic-derived-type.rs", "fn g"),
            (
                "tests/ui/associated-types/associated-types-basic.rs",
                "trait Foo",
            ),
        ] {
            let original_path = originals.join(case);
            let original = std::fs::read_to_string(&original_path).unwrap();
            let offset = original.find(first_item).expect("unchanged item boundary");
            for origin in 0..3 {
                let temporary = (origin != 0).then(|| {
                    let macros = if origin == 1 { "macro_rules! emit {($($t:tt)*)=>{$($t)*}} emit!" }
                    else { "macro_rules! emit {($($t:tt)*)=>{$($t)*}} macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!" };
                    temporary_source(&format!("variance-query-{edition}-{origin}"),
                        &format!("{}{}{{{}}}", &original[..offset], macros, &original[offset..]))
                });
                let path = temporary.as_ref().unwrap_or(&original_path);
                let (mut db, seal, root) = configured_impl_catalog_world(path, edition)
                    .unwrap_or_else(|error| panic!("actual variance world {case},edition{edition},origin{origin}: {error:?}"));
                let rows = actual_rows(&mut db, &seal, root.crate_key);
                let expected = rows
                    .iter()
                    .filter_map(|(&id, row)| {
                        let (count, domain, associated) =
                            match (row.def_key.declared_name.as_str(), row.def_key.item_kind) {
                                ("Pair", ItemKind::Struct) => (1, Domain::NominalInference, false),
                                ("g" | "f", ItemKind::Function) => {
                                    (1, Domain::FunctionInference, false)
                                }
                                ("main", ItemKind::Function) => (0, Domain::ZeroGenerics, false),
                                ("Foo", ItemKind::Trait) => (1, Domain::NotApplicable, false),
                                ("T", ItemKind::TypeAlias) => match parent_kind(row, &rows) {
                                    Some(ItemKind::Trait) => (1, Domain::NotApplicable, true),
                                    Some(ItemKind::Impl) => (0, Domain::ZeroGenerics, false),
                                    other => {
                                        panic!("actual associated T parent missing: {other:?}")
                                    }
                                },
                                _ => return None,
                            };
                        Some((id, count, domain, associated))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    expected.len(),
                    4,
                    "actual source declaration coverage {case},edition{edition},origin{origin}"
                );
                db.trace.detailed = true;
                let before = db.trace.events.len();
                let mut held = Vec::new();
                for (definition, count, domain, associated) in expected {
                    let key = DefinitionHeaderKey {
                        inventory: seal.clone(),
                        definition,
                    };
                    let query = QueryKey::SourceVarianceQueries(Box::new(key.clone()));
                    let first = ready(db.fetch(&SyntaxProvider, query.clone()));
                    let current = check_actual_variance_query(&mut db, &key, count, domain, associated)
                        .unwrap_or_else(|error| panic!("actual variance metadata {case},edition{edition},origin{origin}: {error:?}"));
                    if domain == Domain::NominalInference {
                        // The unchanged Pair fields both contain T directly.
                        // This expectation lives solely in the host control.
                        assert_eq!(
                            current.plan().outcome(),
                            Ok(VarianceQueryOutcome::Known(&[
                                rustc_type_ir::Variance::Covariant
                            ]))
                        );
                    }
                    held.push(current);
                    let again = ready(db.fetch(&SyntaxProvider, query));
                    assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                    assert_eq!(first.fingerprint, again.fingerprint);
                    checks += 1;
                }
                assert!(
                    db.trace.events[before..].iter().all(|event| !matches!(
                        event.key,
                        QueryKey::FunctionSignature(_)
                            | QueryKey::BodyStructure(_)
                            | QueryKey::Thunk(_)
                            | QueryKey::PatternPlan(_)
                    )),
                    "variance metadata must leave executable bodies cold"
                );
                assert!(
                    held.iter()
                        .all(|current| current.require_current(&db).is_ok())
                );
                db.publish_inputs(BTreeMap::new()).unwrap();
                assert!(
                    held.iter()
                        .all(|current| current.require_current(&db).is_err())
                );
                worlds += 1;
                eprintln!(
                    "actual variance query metadata {case},edition{edition},origin{origin}: memo/currentness/revocation; function inference, opaque/RPITIT, root/main held"
                );
                if let Some(path) = temporary {
                    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
                }
            }
        }
    }
    assert_eq!((worlds, checks), (24, 96));
}

#[test]
fn unchanged_argument_impl_trait_never_claims_zero_upstream_generics_all_editions_and_origins() {
    use crate::traits::upstream::{
        parameters::ParameterPending,
        source_variance_queries::{CurrentSourceVarianceQueries, VarianceQueryPending},
    };
    use ra_ap_syntax::{AstNode, ast};
    let originals =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/upstream/rustc-checkout");
    let mut worlds = 0;
    for edition in [2015, 2018, 2021, 2024] {
        for (case, name) in [
            ("tests/ui/impl-trait/universal_multiple_bounds.rs", "foo"),
            (
                "tests/ui/impl-trait/universal_in_adt_in_parameters.rs",
                "check_display_eq",
            ),
        ] {
            let original_path = originals.join(case);
            let original = std::fs::read_to_string(&original_path).unwrap();
            let offset = original
                .find("use std::fmt::Display;")
                .expect("unchanged first item");
            for origin in 0..3 {
                let temporary = (origin != 0).then(|| {
                    let macros = if origin == 1 { "macro_rules! emit {($($t:tt)*)=>{$($t)*}} emit!" }
                    else { "macro_rules! emit {($($t:tt)*)=>{$($t)*}} macro_rules! wrap {($($t:tt)*)=>{emit!{$($t)*}}} wrap!" };
                    temporary_source(&format!("variance-apit-{name}-{edition}-{origin}"),
                        &format!("{}{}{{{}}}", &original[..offset], macros, &original[offset..]))
                });
                let path = temporary.as_ref().unwrap_or(&original_path);
                let (mut db, seal, root) = configured_impl_catalog_world(path, edition)
                    .unwrap_or_else(|error| {
                        panic!(
                            "actual APIT world {case},edition{edition},origin{origin}: {error:?}"
                        )
                    });
                let rows = actual_rows(&mut db, &seal, root.crate_key);
                let select = |wanted: &str| {
                    let found = rows
                        .iter()
                        .filter(|(_, item)| {
                            item.def_key.item_kind == ItemKind::Function
                                && item.def_key.declared_name == wanted
                        })
                        .map(|(id, _)| *id)
                        .collect::<Vec<_>>();
                    assert_eq!(found.len(), 1, "actual function identity {wanted}");
                    found[0]
                };
                let definition = select(name);
                let main = select("main");
                db.trace.detailed = true;
                let before = db.trace.events.len();
                let declaration =
                    ready(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(definition)))
                        .value;
                let QueryValue::DeclarationGenerics(generics) = declaration.as_ref() else {
                    panic!("actual generic declaration differs")
                };
                assert_eq!(
                    generics.parameters().len(),
                    0,
                    "written generic syntax remains unchanged"
                );
                assert_eq!(generics.layout().and_then(|layout| layout.count()), Some(0));
                assert_eq!(generics.argument_impl_traits().len(), 1);
                assert!(generics.parameter_type_macros().is_empty());
                let (_, _, context, parsed) = generics
                    .source_parts()
                    .expect("actual declaration parser snapshot");
                for &site in generics.argument_impl_traits() {
                    assert!(context.range.start <= site.start && site.end <= context.range.end);
                    assert!(
                        parsed
                            .syntax_at(site, |node| ast::ImplTraitType::can_cast(node.kind()))
                            .unwrap()
                            .and_then(ast::ImplTraitType::cast)
                            .is_some()
                    );
                }
                let typed = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::DeclarationGenericTypes(Box::new(
                        crate::frontend::declaration_generic_types::DeclarationGenericTypesKey {
                            inventory: seal.clone(),
                            definition,
                        },
                    )),
                ))
                .value;
                let QueryValue::DeclarationGenericTypes(typed) = typed.as_ref() else {
                    panic!("actual typed generic query differs")
                };
                assert_eq!(
                    typed.pending(),
                    Some(
                        crate::frontend::declaration_generic_types::GenericTypesPending::Parameters
                    ),
                    "incomplete identities must remain a typed source preparation dependency"
                );
                let key = DefinitionHeaderKey {
                    inventory: seal.clone(),
                    definition,
                };
                let query = QueryKey::SourceVarianceQueries(Box::new(key.clone()));
                let first = ready(db.fetch(&SyntaxProvider, query.clone()));
                let current = ready(CurrentSourceVarianceQueries::capture(&mut db, &key));
                assert_eq!(
                    current.plan().generic_count(),
                    None,
                    "anonymous upstream identities are missing, not zero"
                );
                assert_eq!(current.plan().domain(), None);
                assert_eq!(
                    current.plan().outcome(),
                    Err(VarianceQueryPending::Parameters(
                        ParameterPending::AnonymousFunctionParameters
                    ))
                );
                let again = ready(db.fetch(&SyntaxProvider, query));
                assert!(first.cacheable && Arc::ptr_eq(&first.value, &again.value));
                assert_eq!(first.fingerprint, again.fingerprint);
                let ordinary = check_actual_variance_query(
                    &mut db,
                    &DefinitionHeaderKey {
                        inventory: seal,
                        definition: main,
                    },
                    0,
                    Domain::ZeroGenerics,
                    false,
                )
                .unwrap();
                assert!(
                    db.trace.events[before..].iter().all(|event| !matches!(
                        event.key,
                        QueryKey::FunctionSignature(_)
                            | QueryKey::BodyStructure(_)
                            | QueryKey::Thunk(_)
                            | QueryKey::PatternPlan(_)
                    )),
                    "APIT completeness must not prepare executable code"
                );
                current.require_current(&db).unwrap();
                ordinary.require_current(&db).unwrap();
                db.publish_inputs(BTreeMap::new()).unwrap();
                assert!(current.require_current(&db).is_err());
                assert!(ordinary.require_current(&db).is_err());
                worlds += 1;
                eprintln!(
                    "actual APIT parameter identities {case},edition{edition},origin{origin}: explicit pending, written layout preserved, memo/cold/revocation; root/runtime held"
                );
                if let Some(path) = temporary {
                    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
                }
            }
        }
    }
    assert_eq!(worlds, 24);
}
