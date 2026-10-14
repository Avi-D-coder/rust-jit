//! Pure cold declaration discovery of unchanged official check_async bodies.
//! Full entry attempts and their actual unsupported runtime frontier are retained
//! separately in readiness-full-entry-witness.rs and readiness-repaired.log.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const ROOT2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2015-2015.rs"
);
const ROOT2018: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2018-2015.rs"
);
const AUX2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/auxiliary/edition-kw-macro-2015.rs"
);
fn prepare(
    year: u16,
    text: &str,
) -> (
    Vec<lrr::source::changes::PreparedSourceView>,
    SourceDiscovery,
) {
    let mut root_profile = ProfileIdentity::current();
    root_profile.edition = year;
    let mut aux_profile = ProfileIdentity::current();
    aux_profile.edition = 2015;
    let units = vec![
        SourceUnitInput {
            path: SourcePath::new("caller.rs").unwrap(),
            crate_key: Digest([2; 32]),
            module_path: vec![],
            profile: root_profile,
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        },
        SourceUnitInput {
            path: SourcePath::new("edition-kw-macro-2015.rs").unwrap(),
            crate_key: Digest([1; 32]),
            module_path: vec![],
            profile: aux_profile,
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(AUX2015.as_bytes().to_vec()).unwrap(),
        },
    ];
    let selections = units
        .iter()
        .map(|unit| SourceSelection {
            path: unit.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile: unit.profile.clone(),
                },
                crate_key: unit.crate_key,
                module_path: vec![],
                cfg: unit.cfg.fingerprint(),
            },
        })
        .collect::<Vec<_>>();
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        vec![SourceDependency {
            consumer: Digest([2; 32]),
            alias: "edition_kw_macro_2015".into(),
            provider: Digest([1; 32]),
        }],
    )
    .unwrap();
    let views = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("source absent")
            };
            view
        })
        .collect::<Vec<_>>();
    let discovery = SourceDiscovery::new(
        views.iter().map(|view| view.share()).collect(),
        views[0].outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([5; 32]),
        1,
    )
    .unwrap()
    .with_declarative(
        Digest([6; 32]),
        Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
    )
    .unwrap();
    (views, discovery)
}
fn exercise(year: u16, text: &str) {
    let (views, mut discovery) = prepare(year, text);
    let mut sealed = false;
    for _ in 0..1000 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                sealed = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!(
                "unchanged caller{year} incomplete discovery: {diagnostic:?}; blocked={blocked:?}"
            ),
            _ => {
                panic!("unchanged caller{year} requested unexpected procedural/compiler authority")
            }
        }
    }
    assert!(
        sealed,
        "stock caller{year} did not seal within bounded work"
    );
    let records = discovery.sealed_declaration_expansions().unwrap();
    for name in [
        "consumes_async",
        "consumes_async_raw",
        "passes_ident",
        "passes_tt",
    ] {
        let id = views[1]
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == name)
            .unwrap()
            .def_key
            .fingerprint();
        let matching = records.iter().filter(|record| matches!(record.key().macro_identity, lrr::expand::identity::MacroIdentity::Declaration {definition} if definition == id)).collect::<Vec<_>>();
        let expected = if name.starts_with("passes_") && year == 2015 {
            2
        } else {
            1
        };
        assert_eq!(
            matching.len(),
            expected,
            "unchanged caller{year} macro {name}"
        );
        for record in matching {
            assert_eq!(record.key().definition_edition, 2015);
            assert_eq!(record.key().call_edition, year);
            assert_eq!(
                record.key().grammar,
                lrr::expand::identity::GrammarSlot::Expression
            );
            record.tokens().unwrap().validate().unwrap();
        }
    }
}
#[test]
fn unchanged_stock_2015_check_async_declarations_resolve_2015_macro_origin() {
    exercise(2015, ROOT2015)
}
#[test]
fn unchanged_stock_2018_check_async_declarations_resolve_2015_macro_origin() {
    exercise(2018, ROOT2018)
}

#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
#[test]
fn unchanged_keyword_header_keeps_caller_profile_and_definition_token_context_separate() {
    use lrr::{
        expand::{
            generated_items::GeneratedItems,
            identity::{ExpansionKey, GrammarSlot, MacroIdentity},
            record::ExpansionRecord,
        },
        frontend::{
            generated_headers::GeneratedHeaderPlan,
            names::{NameLookup, NameNamespaces},
        },
        source::manifest::Namespace,
        syntax::{outline::ScopeKey, token_wire as runtime},
    };
    use lrr_expand_adapter::{
        declarative::{DeclarativeMacro, DeclarativeSite},
        token_wire::{Edition, Key},
    };
    use std::sync::Arc;
    for (producer, name) in [
        ("produces_async", "async"),
        ("produces_async_raw", "r#async"),
    ] {
        let rules = AUX2015
            .split_once(&format!("macro_rules! {producer} {{\n"))
            .unwrap()
            .1
            .split_once("\n}\n")
            .unwrap()
            .0;
        let definition = DeclarativeMacro::new(fixture::definition(rules, Edition::E2015)).unwrap();
        let input = fixture::input("");
        let site = fixture::call_site(&input);
        let context = input
            .spans()
            .iter()
            .find(|s| s.key() == site)
            .unwrap()
            .context;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: Digest::of_bytes(&definition.definition().encode().unwrap()),
            },
            invocation: Digest([9; 32]),
            input_tokens: Digest::of_bytes(&input.encode().unwrap()),
            consumer_compile_env: Digest([6; 32]),
            parent_context: Digest(context.0),
            definition_edition: 2015,
            call_edition: 2024,
            grammar: GrammarSlot::Items,
            engine_version: Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
            procedural_epoch: None,
        };
        let (output, _) = definition
            .expand_hygienic_with_limits(
                &input,
                site,
                DeclarativeSite {
                    expansion: Key(key.fingerprint().0),
                    defining_unit: Key([1; 32]),
                    definition_edition: Edition::E2015,
                },
                2_000_000,
                32 * 1024 * 1024,
            )
            .unwrap();
        let output = output.unwrap();
        assert!(fixture::literal_texts(&output.tokens).is_empty());
        let record = Arc::new(
            ExpansionRecord::from_wire(key.clone(), output.tokens.encode().unwrap()).unwrap(),
        );
        let items = GeneratedItems::admit(&key, record, 1).unwrap();
        let header = &items.items()[0];
        assert_eq!(header.name.as_ref().unwrap().text, name);
        let span = items
            .tokens()
            .spans()
            .iter()
            .find(|s| s.key().0 == header.name.as_ref().unwrap().span.0)
            .unwrap();
        let name_context = items
            .tokens()
            .contexts()
            .iter()
            .find(|c| c.key() == span.context)
            .unwrap();
        assert_eq!(name_context.edition, runtime::Edition::E2015);
        assert_eq!(span.edition, runtime::Edition::E2015);
        let scope = ScopeKey {
            crate_key: Digest([2; 32]),
            module_path: vec![],
            owner: None,
        };
        let plan = GeneratedHeaderPlan::prepare(
            &items,
            scope.clone(),
            runtime::Key(context.0),
            &CfgSet::guest_default(),
            1,
        )
        .unwrap();
        let mut graph = NameNamespaces::default();
        graph
            .declare_module(
                scope.crate_key,
                &[],
                2024,
                lrr::syntax::visibility::VisibilityScope::Public,
            )
            .unwrap();
        plan.publish(&mut graph).unwrap();
        let NameLookup::Defined(selected) = graph
            .lookup_unqualified(Namespace::Value, "r#async", &scope, 2024, None)
            .unwrap()
        else {
            panic!("unchanged2015 keyword header unavailable from caller2024")
        };
        assert_eq!(selected.identity, header.key.fingerprint());
        assert_eq!(selected.defining_unit, scope.crate_key);
        // NameDefinition stores the generated owner's parse profile. This
        // is distinct from the2015 name-token resolution context above; the
        // metadata alone does not establish a semantic edition defect.
        assert_eq!(selected.definition_edition, key.call_edition);
    }
}

#[test]
fn captured_extern_fallback_keeps_relevant_lexical_imports_pending() {
    for import in [
        "use edition_kw_macro_2015::passes_ident as consumes_async;",
        "use edition_kw_macro_2015::*;",
    ] {
        let text = format!(
            "#[macro_use] extern crate edition_kw_macro_2015; fn case(){{{import} let _=consumes_async!(async);}}"
        );
        let (_, mut discovery) = prepare(2018, &text);
        let mut stopped = false;
        for _ in 0..1000 {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => {
                    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
                    assert!(!blocked.is_empty());
                    stopped = true;
                    break;
                }
                SourceDiscoveryStep::Sealed(_) => panic!(
                    "unresolved relevant lexical import bypassed by extern fallback: {import}"
                ),
                _ => panic!("unexpected authority during lexical import guard"),
            }
        }
        assert!(stopped, "lexical import did not retain a typed frontier");
    }
}
#[test]
fn captured_extern_fallback_ignores_unrelated_and_sibling_lexical_imports() {
    for body in [
        "use edition_kw_macro_2015::passes_ident as unrelated; let _=consumes_async!(async);",
        "{use edition_kw_macro_2015::passes_ident as consumes_async;} let _=consumes_async!(async);",
    ] {
        let text = format!("#[macro_use] extern crate edition_kw_macro_2015; fn case(){{{body}}}");
        let (views, mut discovery) = prepare(2018, &text);
        let mut sealed = false;
        for _ in 0..1000 {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Sealed(_) => {
                    sealed = true;
                    break;
                }
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => {
                    panic!("irrelevant lexical import blocked extern fallback: {diagnostic:?}")
                }
                _ => panic!("unexpected authority during lexical import supplement"),
            }
        }
        assert!(sealed);
        let id = views[1]
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "consumes_async")
            .unwrap()
            .def_key
            .fingerprint();
        let records = discovery.sealed_declaration_expansions().unwrap();
        assert_eq!(records.len(), 1);
        assert!(
            matches!(records[0].key().macro_identity,lrr::expand::identity::MacroIdentity::Declaration{definition}if definition==id)
        );
    }
}
