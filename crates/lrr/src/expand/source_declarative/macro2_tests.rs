//! Actual original declaration and invocation components; no full-fixture pass.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    frontend::{impl_index::DeclarationMacroIndex, source_macro_routes::SourceMacroRoutes},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        declaration_scan::DeclarationScanLimits,
        parse::{ParseKey, ParsedFile},
    },
};
use std::sync::Arc;
const REEXPORT: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/reexport-from-multiple-namespaces.rs"
);
const PATTERN: &str =
    include_str!("../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/issue-44127.rs");
const MODERN: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/dollar-crate-modern.rs"
);
const AUX: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/auxiliary/intercrate.rs"
);
const RULES: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/lint/unused/unused-macro-rules-decl.rs"
);
const NO_ARGS: &str =
    include_str!("../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/issue-63102.rs");

fn source(text: &str, path: &str, unit: Digest, edition: u16) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key: unit,
        module_path: vec![],
        profile: ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    }
}
fn providers(units: Vec<SourceUnitInput>, edges: Vec<SourceDependency>) -> Vec<SourceDeclarative> {
    let selections: Vec<_> = units
        .iter()
        .map(|unit| SourceSelection {
            path: unit.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile: unit.profile.clone(),
                },
                crate_key: unit.crate_key,
                module_path: unit.module_path.clone(),
                cfg: unit.cfg.fingerprint(),
            },
        })
        .collect();
    let inputs =
        PreparedSourceInputs::refresh_with_dependencies(None, units, Default::default(), edges)
            .unwrap();
    selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("actual original source absent")
            };
            SourceDeclarative::new(view).unwrap()
        })
        .collect()
}
fn graph(providers: &[SourceDeclarative]) -> crate::frontend::macro_names::MacroNamespaces {
    let routes = providers
        .iter()
        .flat_map(|p| p.routes_in(providers).unwrap())
        .collect::<Vec<_>>();
    // This API already runs the bounded module/import worklist; dynamic macro
    // reexports are selected by lookup, not NameNamespaces::resolve_imports.
    // The result remains a candidate graph, not a published scope seal.
    SourceMacroRoutes::discovery_graph(&routes).unwrap()
}
fn declaration_index(provider: &SourceDeclarative) -> Arc<DeclarationMacroIndex> {
    let source = &provider.source;
    let parsed = ParsedFile::parse(
        source.captured_macro_blob().unwrap(),
        source.selection().outline.parse.profile.clone(),
    )
    .unwrap();
    Arc::new(
        DeclarationMacroIndex::new(Arc::new(
            parsed
                .scan_declarations(
                    source.selection().path.clone(),
                    source.outline().root_scope.clone(),
                    &CfgSet::guest_default(),
                    DeclarationScanLimits::default(),
                )
                .unwrap(),
        ))
        .unwrap(),
    )
}
fn body_request(
    providers: &[SourceDeclarative],
    owner: usize,
    path_tail: &str,
) -> SourceDeclarativeRequest {
    let provider = &providers[owner];
    let index = declaration_index(provider);
    let producer = index
        .scan()
        .producers()
        .iter()
        .find(|p| p.path.last().is_some_and(|s| s.text == path_tail))
        .unwrap();
    let names = graph(providers);
    // The raw declaration index still contains actual preceding builtin inert
    // attribute producers. Retire only a source-replayed, facet-selected proof;
    // this preparation is not a scope seal or retirement of arbitrary macros.
    let mut prepared = (*index).clone();
    for row in index.scan().producers().iter().filter(|row| {
        row.kind == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            && row.range.start < producer.range.start
    }) {
        let Some(proof) = crate::expand::nested_inert::NestedInertProof::select(
            index.scan_handle().clone(),
            row.identity,
        )
        .unwrap() else {
            continue;
        };
        let lookup =
            crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(
                &provider.source,
                index.scan(),
                row.identity,
                &names,
            )
            .unwrap();
        let binding = crate::expand::builtin_attributes::certify_inert_binding(
            proof.name().unwrap(),
            &lookup,
            provider.procedural.as_deref(),
            provider
                .compiler
                .as_ref()
                .and_then(|catalog| catalog.attribute_catalog()),
        )
        .unwrap()
        .expect("original inert Attribute facet remains unresolved");
        let proof = proof.with_attribute_facet(binding);
        proof
            .revalidate(
                provider.source.captured_macro_blob().unwrap(),
                provider.source.captured_macro_cfg().unwrap(),
            )
            .unwrap();
        prepared
            .accept_inert_attribute(row.identity, proof.name().unwrap())
            .unwrap();
    }
    let index = Arc::new(prepared);
    let compile_env = Digest::of_bytes(b"macro2 actual source compile environment");
    let engine = Digest::of_bytes(b"macro2 source producer acceptance");
    let first = provider
        .declaration_demand(&index, producer.identity, compile_env, engine)
        .unwrap();
    let request = match first {
        SourceDeclarativeDemand::Ready(request) => request,
        SourceDeclarativeDemand::NeedInput(input) => {
            assert_eq!(
                input, producer.identity,
                "a different pending producer cannot be retired by module lookup"
            );
            let SourceDeclarativeDemand::Ready(request) = provider
                .declaration_import_request(
                    &index,
                    producer.identity,
                    &names,
                    providers,
                    compile_env,
                    engine,
                )
                .unwrap()
            else {
                panic!("actual original module definition not selected")
            };
            request
        }
    };
    *request
}
fn module_request(
    providers: &[SourceDeclarative],
    owner: usize,
    path_tail: &str,
) -> SourceDeclarativeRequest {
    let provider = &providers[owner];
    let call = provider.source.outline().item_invocations.iter().find(|c| {
        provider.source.macro_call_at(c.range).unwrap().segments().last().is_some_and(|s| matches!(s, crate::syntax::imports::ImportSegment::Name(n) if n == path_tail))
    }).unwrap();
    let SourceDeclarativeDemand::Ready(request) = provider
        .demand_for_discovery(
            call.item_input,
            Digest::of_bytes(b"macro2 actual source compile environment"),
            Digest::of_bytes(b"macro2 source producer acceptance"),
            &graph(providers),
            providers,
        )
        .unwrap()
    else {
        panic!("actual original module invocation not selected")
    };
    *request
}
fn expanded_tokens(request: &SourceDeclarativeRequest) -> TokenForest {
    let output = request.expand().unwrap().tokens().unwrap();
    // Both crates expose the same stable wire schema as distinct Rust types.
    // Replay its checked encoding; do not cast away span/context ownership.
    TokenForest::decode(&output.encode().unwrap()).unwrap()
}
fn token_span<'a>(forest: &'a TokenForest, name: &str) -> &'a wire::SpanRecord {
    let mut stack: Vec<_> = forest.roots().iter().collect();
    while let Some(token) = stack.pop() {
        let key = match token {
            TokenTree::Group { children, .. } => {
                stack.extend(children);
                continue;
            }
            TokenTree::Ident { text, span, .. } if text == name => *span,
            TokenTree::Literal {
                token_text, span, ..
            } if token_text == name => *span,
            _ => continue,
        };
        return forest.spans().iter().find(|s| s.key() == key).unwrap();
    }
    panic!("missing actual output token {name}")
}
fn marked_context<'a>(forest: &'a TokenForest, name: &str) -> &'a wire::ContextRecord {
    let span = token_span(forest, name);
    forest
        .contexts()
        .iter()
        .find(|c| c.key() == span.context)
        .unwrap()
}
fn adapter_for(request: &SourceDeclarativeRequest) -> DeclarativeMacro {
    let DeclarativeBody::Macro2 { arguments, body } = &request.body else {
        panic!("actual macro2 dialect absent")
    };
    DeclarativeMacro::new_macro2(arguments.clone(), body.clone()).unwrap()
}
fn site(request: &SourceDeclarativeRequest) -> DeclarativeSite {
    DeclarativeSite {
        expansion: Key(request.key.fingerprint().0),
        defining_unit: Key(request.owner.0),
        definition_edition: edition(request.key.definition_edition).unwrap(),
    }
}

#[test]
fn unchanged_reexport_main_call_uses_exact_macro_member_all_editions() {
    for owning in [2015, 2018, 2021, 2024] {
        let p = providers(
            vec![source(REEXPORT, "reexport.rs", Digest([71; 32]), owning)],
            vec![],
        );
        let request = body_request(&p, 0, "A");
        let selected = p[0]
            .source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.item_kind == ItemKind::Macro && i.def_key.declared_name == "A")
            .unwrap();
        assert_eq!(
            request.key.macro_identity,
            MacroIdentity::Declaration {
                definition: selected.def_key.fingerprint()
            }
        );
        assert_eq!(request.key.call_edition, owning);
        assert_eq!(request.key.definition_edition, owning);
        let shape = p[0]
            .source
            .macro_definition_shape_at(selected.declaration_span)
            .unwrap();
        assert_eq!(
            shape.dialect(),
            crate::syntax::macro_calls::MacroDefinitionDialect::Macro2
        );
        let DeclarativeBody::Macro2 {
            arguments: Some(arguments),
            body,
        } = &request.body
        else {
            panic!("exact original simple declaration groups absent")
        };
        assert_eq!(
            arguments.encode().unwrap(),
            group(&p[0].owned_forest().unwrap(), shape.arguments().unwrap())
                .unwrap()
                .encode()
                .unwrap()
        );
        assert_eq!(
            body.encode().unwrap(),
            group(&p[0].owned_forest().unwrap(), shape.body())
                .unwrap()
                .encode()
                .unwrap()
        );
        let output = request.expand().unwrap();
        assert_eq!(output.key(), request.key());
        let tokens = output.tokens().unwrap();
        assert!(
            matches!(tokens.roots(), [crate::syntax::token_wire::TokenTree::Group { children, .. }] if children.is_empty()),
            "the real empty macro body retains its root delimiter node"
        );
        assert_eq!(
            p[0].source
                .outline()
                .items
                .iter()
                .filter(|i| i.def_key.declared_name == "A")
                .count(),
            3
        );
    }
}
#[test]
fn unchanged_pattern_macro_retains_opaque_definition_and_captured_call_spans_all_editions() {
    for owning in [2015, 2018, 2021, 2024] {
        let p = providers(
            vec![source(PATTERN, "issue-44127.rs", Digest([72; 32]), owning)],
            vec![],
        );
        let request = body_request(&p, 0, "pattern");
        assert_eq!(request.key.grammar, GrammarSlot::Pattern);
        let original = token_span(&request.input, "3").clone();
        let output = expanded_tokens(&request);
        assert_eq!(token_span(&output, "3"), &original);
        for template in ["Foo", "bar"] {
            let context = marked_context(&output, template);
            assert_eq!(context.transparency, wire::Transparency::Opaque);
            assert_eq!(context.expansion, Some(Key(request.key.fingerprint().0)));
            assert_eq!(context.defining_unit, Key(request.owner.0));
            assert_eq!(context.edition, edition(owning).unwrap());
            assert_eq!(
                token_span(&output, template).source.content,
                Key(p[0].source.selection().outline.parse.content.0)
            );
        }
    }
}
#[test]
fn unchanged_cross_crate_dollar_crate_retains_actual_provider_and_owning_editions() {
    for (caller, owner) in [(2015, 2024), (2018, 2021), (2021, 2018), (2024, 2015)] {
        let root = Digest([73; 32]);
        let aux = Digest([74; 32]);
        let p = providers(
            vec![
                source(MODERN, "dollar-crate-modern.rs", root, caller),
                source(AUX, "intercrate.rs", aux, owner),
            ],
            vec![SourceDependency {
                consumer: root,
                alias: "intercrate".into(),
                provider: aux,
            }],
        );
        let request = module_request(&p, 0, "uses_dollar_crate_modern");
        assert_eq!(request.owner, aux);
        assert_eq!(request.key.call_edition, caller);
        assert_eq!(request.key.definition_edition, owner);
        let output = expanded_tokens(&request);
        for name in ["$crate", "SomeType"] {
            let context = marked_context(&output, name);
            assert_eq!(context.transparency, wire::Transparency::Opaque);
            assert_eq!(context.defining_unit, Key(aux.0));
            assert_eq!(context.edition, edition(owner).unwrap());
            assert_eq!(token_span(&output, name).source.unit, Key(aux.0));
        }
    }
}
#[test]
fn unchanged_rules_syntax_macro_uses_actual_rule_body_without_argument_group() {
    for owning in [2015, 2018, 2021, 2024] {
        let p = providers(
            vec![source(
                RULES,
                "unused-macro-rules-decl.rs",
                Digest([75; 32]),
                owning,
            )],
            vec![],
        );
        let request = body_request(&p, 0, "num");
        assert!(matches!(
            request.body,
            DeclarativeBody::Macro2 {
                arguments: None,
                ..
            }
        ));
        let output = expanded_tokens(&request);
        assert_eq!(token_span(&output, "1").edition, edition(owning).unwrap());
        let p = providers(
            vec![source(NO_ARGS, "issue-63102.rs", Digest([76; 32]), owning)],
            vec![],
        );
        let item = p[0]
            .source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.item_kind == ItemKind::Macro)
            .unwrap();
        let shape = p[0]
            .source
            .macro_definition_shape_at(item.declaration_span)
            .unwrap();
        assert_eq!(shape.arguments(), None);
        assert!(matches!(
            p[0].original_definition_body(item.declaration_span)
                .unwrap(),
            DeclarativeBody::Macro2 {
                arguments: None,
                ..
            }
        ));
    }
}
#[test]
fn macro2_source_request_exact_byte_limit_refuses_before_materialization() {
    let p = providers(
        vec![source(PATTERN, "issue-44127.rs", Digest([77; 32]), 2024)],
        vec![],
    );
    let request = body_request(&p, 0, "pattern");
    let parser = adapter_for(&request);
    let (result, stats) = parser
        .expand_hygienic_with_limits(&request.input, request.site, site(&request), 100, 32768)
        .unwrap();
    let bytes = result.unwrap().tokens.encoded_len().unwrap();
    assert_eq!(stats.materializations, 1);
    assert_eq!(stats.admitted_bytes, bytes);
    let (result, stats) = parser
        .expand_hygienic_with_limits(&request.input, request.site, site(&request), 100, bytes - 1)
        .unwrap();
    assert_eq!(
        result.err().unwrap().kind,
        lrr_expand_adapter::declarative::FailureKind::Limit
    );
    assert_eq!(stats.materializations, 0);
    assert_eq!(
        parser
            .expand_hygienic_with_limits(&request.input, request.site, site(&request), 100, bytes)
            .unwrap()
            .1
            .materializations,
        1
    );
}
#[test]
fn macro2_exact_range_and_changed_arguments_body_edition_are_not_interchangeable() {
    let unit = Digest([78; 32]);
    let before = providers(
        vec![source(
            "pub macro m($v:literal){$v}fn main(){m!(1);}",
            "owner.rs",
            unit,
            2024,
        )],
        vec![],
    );
    let request = body_request(&before, 0, "m");
    let item = before[0]
        .source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.item_kind == ItemKind::Macro)
        .unwrap();
    assert!(
        before[0]
            .original_definition_body(SourceRange {
                start: item.declaration_span.start + 1,
                end: item.declaration_span.end
            })
            .is_err()
    );
    assert!(
        before[0]
            .original_definition_body(SourceRange {
                start: item.declaration_span.end,
                end: item.declaration_span.start
            })
            .is_err()
    );
    for (text, owning) in [
        ("pub macro m($v:literal){2}fn main(){m!(1);}", 2024),
        ("pub macro m($v:expr){$v}fn main(){m!(1);}", 2024),
        ("pub macro m($v:literal){$v}fn main(){m!(1);}", 2021),
    ] {
        let after = providers(vec![source(text, "owner.rs", unit, owning)], vec![]);
        let changed = body_request(&after, 0, "m");
        assert_ne!(request.key.fingerprint(), changed.key.fingerprint());
        assert_ne!(
            request.input.encode().unwrap(),
            changed.input.encode().unwrap()
        );
    }
}
#[test]
fn original_module_macro2_forward_binding_does_not_become_textual_macro_rules() {
    let p = providers(
        vec![source(
            "fn main(){m!();}pub macro m(){1}",
            "forward.rs",
            Digest([79; 32]),
            2024,
        )],
        vec![],
    );
    let index = declaration_index(&p[0]);
    let producer = index
        .scan()
        .producers()
        .iter()
        .find(|p| p.path[0].text == "m")
        .unwrap();
    assert!(matches!(
        index.lookup(producer.identity).unwrap(),
        HygienicLookup::Selection(crate::frontend::macro_origins::MacroSelection::Absent)
    ));
    let request = body_request(&p, 0, "m");
    assert!(matches!(request.body, DeclarativeBody::Macro2 { .. }));
    assert_eq!(
        token_span(&expanded_tokens(&request), "1").source.unit,
        Key([79; 32])
    );
}
#[test]
fn original_wrong_definition_owner_cannot_use_private_source_groups() {
    use crate::frontend::macro_names::{MacroNamespaces, MacroPath};
    let p = providers(
        vec![source(
            "pub macro m(){1}fn main(){m!();}",
            "owner.rs",
            Digest([80; 32]),
            2024,
        )],
        vec![],
    );
    let index = declaration_index(&p[0]);
    let producer = index
        .scan()
        .producers()
        .iter()
        .find(|p| p.path[0].text == "m")
        .unwrap();
    let item = p[0]
        .source
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.item_kind == ItemKind::Macro)
        .unwrap();
    let mut bad = MacroNamespaces::default();
    bad.define(
        MacroPath::new(
            Digest([80; 32]),
            vec![crate::expand::scopes::MacroName::parse("m", 2024).unwrap()],
        )
        .unwrap(),
        crate::expand::scopes::MacroDefinition {
            identity: item.def_key.fingerprint(),
            defining_unit: Digest([81; 32]),
            definition_edition: 2024,
        },
    )
    .unwrap();
    let result = p[0].declaration_import_request(
        &index,
        producer.identity,
        &bad,
        &p,
        Digest([82; 32]),
        Digest([83; 32]),
    );
    assert!(matches!(result,Err(ref d) if d.code=="LRR-E-EXPANSION-INPUT"));
}

#[test]
fn genuine_original_macro2_output_cannot_publish_generated_macro2_as_rules() {
    let root = Digest([84; 32]);
    let aux = Digest([85; 32]);
    let p = providers(
        vec![
            source(MODERN, "dollar-crate-modern.rs", root, 2024),
            source(AUX, "intercrate.rs", aux, 2018),
        ],
        vec![SourceDependency {
            consumer: root,
            alias: "intercrate".into(),
            provider: aux,
        }],
    );
    let request = module_request(&p, 0, "define_uses_dollar_crate_modern_nested");
    let output = request.expand().unwrap();
    let items =
        crate::expand::generated_items::GeneratedItems::admit(request.key(), Arc::new(output), 1)
            .unwrap();
    let parsed = ParsedFile::from_generated_items(&items).unwrap();
    // Generated output may contain macro 2.0 items at outline time; the
    // generated-dialect refusal belongs to definition capture when selected.
    let outline = parsed.outline(root, vec![], &CfgSet::guest_default()).unwrap();
    let macro2 = outline
        .items
        .iter()
        .find(|item| item.def_key.item_kind == crate::source::manifest::ItemKind::Macro)
        .expect("generated macro 2.0 item retained by the outline");
    let refusal = parsed
        .macro_definition_shape_at(macro2.declaration_span)
        .unwrap_err();
    assert_eq!(refusal.code, "LRR-E-MACRO2-SOURCE");
    assert!(refusal.message.contains("generated macro2"));
}

#[test]
fn private_module_macro2_is_not_a_public_definition_route() {
    let p = providers(
        vec![source(
            "mod hidden{macro m(){1}}fn main(){hidden::m!();}",
            "private.rs",
            Digest([86; 32]),
            2024,
        )],
        vec![],
    );
    let index = declaration_index(&p[0]);
    let producer = index
        .scan()
        .producers()
        .iter()
        .find(|p| p.path.last().is_some_and(|p| p.text == "m"))
        .unwrap();
    let result = p[0].declaration_import_request(
        &index,
        producer.identity,
        &graph(&p),
        &p,
        Digest([87; 32]),
        Digest([88; 32]),
    );
    assert!(matches!(result,Err(ref d) if d.code=="LRR-E-MACRO-INACCESSIBLE"));
}

#[test]
fn original_cfg_disabled_macro2_does_not_mint_a_module_definition() {
    let p = providers(
        vec![source(
            "#[cfg(any())]pub macro m(){1}fn main(){m!();}",
            "inactive.rs",
            Digest([89; 32]),
            2024,
        )],
        vec![],
    );
    assert!(
        p[0].source
            .outline()
            .items
            .iter()
            .all(|i| i.def_key.item_kind != ItemKind::Macro)
    );
    let index = declaration_index(&p[0]);
    let producer = index
        .scan()
        .producers()
        .iter()
        .find(|p| p.path[0].text == "m")
        .unwrap();
    let result = p[0]
        .declaration_import_request(
            &index,
            producer.identity,
            &graph(&p),
            &p,
            Digest([90; 32]),
            Digest([91; 32]),
        )
        .unwrap();
    assert!(matches!(result,SourceDeclarativeDemand::NeedInput(id) if id==producer.identity));
    assert_eq!(
        <crate::syntax::outline::Outline as StableEncode>::DOMAIN,
        "lrr/v3/outline"
    );
}

#[test]
fn exact_macro2_invalid_matcher_is_cached_definition_failure_before_transcription() {
    let p = providers(
        vec![source(
            "pub macro m($v){$v}fn main(){m!(1);}",
            "invalid.rs",
            Digest([92; 32]),
            2024,
        )],
        vec![],
    );
    let request = body_request(&p, 0, "m");
    let parser = adapter_for(&request);
    for _ in 0..2 {
        let (result, stats) = parser
            .expand_hygienic_with_limits(&request.input, request.site, site(&request), 100, 32768)
            .unwrap();
        assert_eq!(
            result.err().unwrap().kind,
            lrr_expand_adapter::declarative::FailureKind::DefinitionSyntax
        );
        assert_eq!(stats.preflight_calls, 0);
        assert_eq!(stats.materializations, 0);
    }
}
