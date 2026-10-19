//! Namespace/authority controls on unchanged official macro definitions.
//! No guest execution or whole stock fixture acceptance is claimed.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    },
    expand::{
        generated_items::GeneratedItems,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{
        compile_input_catalog::CompileInputCatalog,
        names::{NameLookup, NameNamespaces},
    },
    syntax::parse::ParsedFile,
};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/auxiliary/intercrate.rs"
);

fn world(text: &str, edition: u16) -> (Database, SourceDiscovery) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(text, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Type,
        Digest::of_bytes(b"actual contextual official declaration control"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"actual contextual declaration compiler"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    let mut published = false;
    for _ in 0..100_000 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(&mut db).unwrap();
                published = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("actual contextual declaration Incomplete: {diagnostic:?} {blocked:?}"),
            SourceDiscoveryStep::NeedSourceDeriveSelection { .. } => {
                panic!("actual NeedSourceDeriveSelection")
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { .. } => {
                panic!("actual NeedSourceDeriveExpansion")
            }
            SourceDiscoveryStep::NeedSourceDeriveAppend { .. } => {
                panic!("actual NeedSourceDeriveAppend")
            }
            SourceDiscoveryStep::NeedDeriveAppend { .. } => panic!("actual NeedDeriveAppend"),
            SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("actual NeedDeriveDeclaration")
            }
            SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("actual NeedAttributeSelection")
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. } => {
                panic!("actual NeedProceduralSelection")
            }
        }
    }
    assert!(published, "actual contextual declaration never sealed");
    (db, discovery)
}

#[test]
fn unchanged_official_opaque_aliases_share_the_namespace_kernel_without_root_leakage() {
    for edition in [2015, 2018, 2021, 2024] {
        // Entire original file/definitions are unchanged. Appended controls
        // compare actual hidden bindings with an ordinary same-spelled glob.
        let source = format!(
            "{ORIGINAL}\nuses_dollar_crate_modern!();uses_dollar_crate_modern!();\nmod root_side{{pub struct Alias;pub struct RootOnly;}}use self::root_side::*;use self::SomeType as Imported;"
        );
        let (mut db, discovery) = world(&source, edition);
        let mut units = Vec::new();
        for record in discovery.sealed_expansions().unwrap() {
            let items = GeneratedItems::admit(record.key(), Arc::clone(record), 1).unwrap();
            if !items.items().iter().any(|header| {
                header
                    .name
                    .as_ref()
                    .is_some_and(|name| name.text == "Alias")
            }) {
                continue;
            }
            // External test-only inspection of actual published structural input.
            let key = ParsedFile::from_generated_items(&items).unwrap().key();
            let input = db.input(&InputKey::GeneratedSyntax(key.content)).unwrap();
            let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
                panic!("actual contextual source variant differs")
            };
            units.push(Arc::clone(unit));
        }
        assert_eq!(units.len(), 2);
        let (sources, graph) = discovery.into_runtime_parts().unwrap();
        let scope = &sources[0].outline().root_scope;
        let opaque = graph
            .declared_item_names()
            .filter(|name| name.name() == "Alias")
            .collect::<Vec<_>>();
        assert_eq!(opaque.len(), 2);
        assert_ne!(opaque[0].class(), opaque[1].class());
        let root = graph
            .lookup_unqualified(Namespace::Type, "Alias", scope, edition, None)
            .unwrap();
        let NameLookup::Defined(root) = root else {
            panic!("ordinary glob alias must retain its actual selected source: {root:?}")
        };
        assert!(
            opaque
                .iter()
                .all(|fact| fact.definition().identity != root.identity)
        );
        for fact in &opaque {
            assert!(matches!(fact.class(), NameBindingClass::Item(_)));
            assert_eq!(fact.definition().definition_edition, edition);
            assert_eq!(fact.epoch(), 1);
            assert!(fact.range().start < fact.range().end);
            assert_eq!(fact.fingerprint(), StableEncode::fingerprint(fact.as_ref()));
            assert_eq!(fact.definition().identity, fact.source().fingerprint());
            assert!(fact.metadata_bytes().unwrap() > 0);
            assert!(Arc::ptr_eq(
                graph
                    .declared_item_name(Namespace::Type, fact.definition().identity)
                    .unwrap(),
                fact,
            ));
            let matching = units
                .iter()
                .filter(|unit| fact.require_generated_source(unit).is_ok())
                .count();
            assert_eq!(
                matching, 1,
                "actual source record/canonical map must match uniquely"
            );
            assert_eq!(
                graph
                    .lookup_unqualified_in_class(
                        Namespace::Type,
                        fact.name(),
                        scope,
                        edition,
                        None,
                        fact.class(),
                    )
                    .unwrap(),
                NameLookup::Defined(fact.definition().clone())
            );
            for needs_adjustment in ["SomeType", "Imported", "RootOnly"] {
                match graph.lookup_unqualified_in_class(
                    Namespace::Type,
                    needs_adjustment,
                    scope,
                    edition,
                    None,
                    fact.class(),
                ) {
                    Err(error) if error.code == "LRR-E-NAME-CONTEXT" => {}
                    other => panic!(
                        "actual unmatched declaration/import/glob requires pinned adjustment, got {other:?}"
                    ),
                }
            }
            let path = NameNamespaces::path(scope.crate_key, &["Alias"], edition).unwrap();
            let foreign = ScopeKey {
                crate_key: Digest([203; 32]),
                module_path: vec![],
                owner: None,
            };
            // A foreign caller cannot reuse a root glob in an opaque class.
            // The actual binding adjustment is required before this lookup
            // can establish an access result. Keep that precise refusal.
            let error = graph
                .lookup_in_class(Namespace::Type, &path, &foreign, fact.class())
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-NAME-CONTEXT");
            assert_eq!(
                error.message,
                "actual glob binding name adjustment is required"
            );
        }
        // Completeness still requires the actual owning discovery/query/source
        // authority. These metadata facts do not grant a constructor/Type use.
    }
}

#[test]
fn upstream_enum_members_keep_actual_generated_owner_and_token_occurrences() {
    const ENUM_SOURCE: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/issue-8851.rs");
    for edition in [2015, 2018, 2021, 2024] {
        // Both complete official programs are unchanged. Extra modules and
        // passthrough expansion belong to this source-identity control only.
        let text = format!(
            "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} mod left {{emit! {{{ENUM_SOURCE}}}}} mod right {{emit! {{{ENUM_SOURCE}}}}}"
        );
        let (mut db, discovery) = world(&text, edition);
        let mut units = Vec::new();
        for record in discovery.sealed_expansions().unwrap() {
            let items = GeneratedItems::admit(record.key(), Arc::clone(record), 1).unwrap();
            let key = ParsedFile::from_generated_items(&items).unwrap().key();
            if let Some(input) = db.input(&InputKey::GeneratedSyntax(key.content)) {
                if let InputValue::GeneratedSyntax(unit) = input.as_ref() {
                    units.push(Arc::clone(unit));
                }
            }
        }
        let (_, graph) = discovery.into_runtime_parts().unwrap();
        let facts: Vec<_> = graph
            .declared_item_names()
            .filter(|fact| fact.enum_owner().is_some())
            .collect();
        assert_eq!(facts.len(), 8, "two variants, two namespaces, two owners");
        let mut owners = std::collections::BTreeSet::new();
        for fact in facts {
            let matches: Vec<_> = units
                .iter()
                .filter(|unit| fact.require_generated_source(unit).is_ok())
                .collect();
            assert_eq!(matches.len(), 1);
            let unit = matches[0];
            let owner = fact.enum_owner().unwrap();
            owners.insert(owner);
            assert_eq!(
                fact.definition().identity,
                crate::syntax::outline::variant_identity(owner, fact.name())
            );
            assert_eq!(fact.definition().definition_edition, edition);
            assert_eq!(fact.class(), NameBindingClass::Root);
            assert_eq!(fact.fingerprint(), StableEncode::fingerprint(fact.as_ref()));
            let variant = graph.enum_variant(fact.definition().identity).unwrap();
            assert_eq!(variant.enumeration, owner);
            assert_eq!(variant.name, fact.name());
            assert!(graph.is_enum_scope(fact.scope()));
            let mut path = fact
                .scope()
                .module_path
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            path.push(fact.name());
            let path = NameNamespaces::path(fact.scope().crate_key, &path, edition).unwrap();
            let mut caller = fact.scope().clone();
            caller.module_path.pop();
            assert_eq!(
                graph
                    .lookup(fact.definition().namespace, &path, &caller)
                    .unwrap(),
                NameLookup::Defined(fact.definition().clone())
            );
            let mut wrong = fact.as_ref().clone();
            wrong.epoch += 1;
            assert!(wrong.require_generated_source(unit).is_err());
            let mut wrong = fact.as_ref().clone();
            wrong.member.as_mut().unwrap().1 = u32::MAX;
            assert!(wrong.require_generated_source(unit).is_err());
            let mut wrong = fact.as_ref().clone();
            wrong.range.end += 1;
            assert!(wrong.require_generated_source(unit).is_err());
            assert!(
                units
                    .iter()
                    .filter(|other| !Arc::ptr_eq(unit, other))
                    .all(|other| fact.require_generated_source(other).is_err())
            );
        }
        assert_eq!(owners.len(), 2);
    }
}
