//! Request/record/occurrence controls. Stock originals are not guest passes.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    expand::source_declarative::SourceDeclarativeDemand,
    frontend::impl_index::{DeclarationMacroParent, GeneratedDeclarationMacroIndex},
    source::{
        changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::BodyIndexLimits,
        declaration_scan::DeclarationScanLimits,
        parse::{ParseKey, ParsedFile},
    },
};

fn provider(text: &str, edition: u16) -> (SourceDeclarative, Arc<DeclarationMacroIndex>) {
    let (provider, index, _) = world(text, edition);
    (provider, index)
}
pub(crate) fn world(
    text: &str,
    edition: u16,
) -> (
    SourceDeclarative,
    Arc<DeclarationMacroIndex>,
    PreparedSourceInputs,
) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let mut profile = ProfileIdentity::current();
    profile.edition = edition;
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("assembly.rs").unwrap();
    let unit = Digest([241; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let scan = view
        .declaration_inventory(DeclarationScanLimits::default())
        .unwrap();
    let mut provider = SourceDeclarative::new(view).unwrap();
    let compiler = Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(provider.profile())
            .unwrap(),
    );
    provider.compiler = Some(compiler.clone());
    let mut index = DeclarationMacroIndex::new(scan.clone()).unwrap();
    // Preserve stock crate attributes. Resolve their actual Attribute facet and
    // retain the same inert proof used by ordinary source discovery, rather
    // than removing annotations or treating every attribute as completed.
    for identity in
        crate::syntax::original_inert::capture(&provider.source, &scan, 32 * 1024 * 1024).unwrap()
    {
        let proof = crate::expand::nested_inert::NestedInertProof::select(scan.clone(), identity)
            .unwrap()
            .expect("stock original inert prefix");
        let lookup =
            crate::service::source_attribute_requests::SourceAttributeSelection::cold_lookup(
                &provider.source,
                &scan,
                identity,
                &crate::frontend::macro_names::MacroNamespaces::default(),
            )
            .unwrap();
        let binding = crate::expand::builtin_attributes::certify_inert_binding(
            proof.name().unwrap(),
            &lookup,
            None,
            compiler.attribute_catalog(),
        )
        .unwrap()
        .expect("matched compiler inert Attribute facet");
        let proof = proof.with_attribute_facet(binding);
        proof
            .revalidate(
                provider.source.captured_macro_blob().unwrap(),
                provider.source.captured_macro_cfg().unwrap(),
            )
            .unwrap();
        index
            .accept_inert_attribute(identity, proof.name().unwrap())
            .unwrap();
    }
    (provider, Arc::new(index), inputs)
}
pub(crate) fn request(
    provider: &SourceDeclarative,
    index: &DeclarationMacroIndex,
) -> Box<SourceDeclarativeRequest> {
    let invocation = index
        .scan()
        .producers()
        .iter()
        .find(|p| p.kind == DeclarationProducerKind::Macro)
        .unwrap()
        .identity;
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(
            index,
            invocation,
            Digest([17; 32]),
            super::super::engine_version(),
        )
        .unwrap()
    else {
        panic!("demand incomplete")
    };
    request
}
pub(crate) fn output(
    provider: &SourceDeclarative,
    parent: &DeclarationScan,
    request: &SourceDeclarativeRequest,
    epoch: u64,
) -> Arc<GeneratedDeclarations> {
    Arc::new(
        GeneratedDeclarations::admit(
            request.key(),
            Arc::new(request.expand().unwrap()),
            epoch,
            parent,
            provider.source.captured_macro_cfg().unwrap(),
            DeclarationScanLimits::default(),
        )
        .unwrap(),
    )
}
const NESTED: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
const EXPRESSION: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-stmt_macro_in_expr_macro.rs"
);
const FORWARDED: &str =
    include_str!("../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-stmt-2.rs");

#[test]
fn rustc_local_struct_source_preserves_attribute_semantics_across_normalized_anchors() {
    const ORIGINAL: &str = include_str!(
        "../../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, index) = provider(ORIGINAL, edition);
        let original = provider
            .module_source()
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        let prepared = SourceSyntaxAssembly::new(&provider, &index, 1)
            .unwrap()
            .finish()
            .unwrap();
        let parsed =
            ParsedFile::from_syntax_snapshot(prepared.syntax().clone(), provider.profile().clone())
                .unwrap();
        let normalized = parsed
            .outline(
                original.def_key.crate_key,
                vec![],
                provider.module_source().captured_macro_cfg().unwrap(),
            )
            .unwrap();
        let actual = normalized
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == original.def_key.fingerprint())
            .unwrap();
        assert_eq!(original.header.attributes.len(), 1);
        assert_ne!(
            original.header.attributes[0].source_meta, actual.header.attributes[0].source_meta,
            "this unchanged fixture must exercise coordinate normalization"
        );
        assert!(original.header.same_semantics(&actual.header));
        assert_eq!(original.header.fingerprint(), actual.header.fingerprint());
        let anchor = prepared
            .original_declarations()
            .iter()
            .find(|anchor| anchor.original_definition() == original.def_key.fingerprint())
            .unwrap();
        assert_eq!(anchor.original_range(), original.declaration_span);
        assert_eq!(anchor.declaration_range(), actual.declaration_span);
        assert_eq!(
            anchor.body_range(),
            actual.body.as_ref().map(|body| body.range)
        );
        let mut changed = actual.header.clone();
        changed.attributes[0].name.push_str("_different");
        assert!(!original.header.same_semantics(&changed));
        let mut changed = actual.header.clone();
        changed.attributes[0].tokens[0].text.push_str("_different");
        assert!(!original.header.same_semantics(&changed));
    }
}

#[test]
fn original_declaration_anchor_survives_a_copied_header_before_it() {
    let (provider, index) = provider(
        "macro_rules! emit{($item:item)=>{$item};}emit!(fn original(){});fn original(){}",
        2024,
    );
    let original = provider
        .module_source()
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "original")
        .unwrap();
    let request = request(&provider, &index);
    let generated = output(&provider, index.scan(), &request, 1);
    let prepared = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .attach(&request, generated)
        .unwrap()
        .finish()
        .unwrap();
    let anchor = prepared
        .original_declarations()
        .iter()
        .find(|anchor| anchor.original_definition() == original.def_key.fingerprint())
        .unwrap();
    let parsed =
        ParsedFile::from_syntax_snapshot(prepared.syntax().clone(), provider.profile().clone())
            .unwrap();
    let outline = parsed
        .outline(
            original.def_key.crate_key,
            vec![],
            provider.module_source().captured_macro_cfg().unwrap(),
        )
        .unwrap();
    let copies = outline
        .items
        .iter()
        .filter(|item| item.def_key.declared_name == "original")
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2);
    let actual = copies
        .iter()
        .find(|item| item.declaration_span == anchor.declaration_range())
        .unwrap();
    assert_eq!(actual.def_key.duplicate_ordinal, 1);
    assert_ne!(actual.def_key.fingerprint(), anchor.original_definition());
    assert_ne!(copies[0].declaration_span, anchor.declaration_range());
    assert_eq!(anchor.original_range(), original.declaration_span);
}

#[test]
fn original_initializer_anchor_tracks_a_completely_replaced_expression() {
    let (provider, index) = provider(
        "macro_rules! value{()=>{1_i32+2_i32};}const X:i32=value!();fn main(){}",
        2024,
    );
    let original = provider
        .module_source()
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "X")
        .unwrap();
    let request = request(&provider, &index);
    let generated = output(&provider, index.scan(), &request, 1);
    let prepared = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .attach(&request, generated)
        .unwrap()
        .finish()
        .unwrap();
    let anchor = prepared
        .original_declarations()
        .iter()
        .find(|anchor| anchor.original_definition() == original.def_key.fingerprint())
        .unwrap();
    let parsed =
        ParsedFile::from_syntax_snapshot(prepared.syntax().clone(), provider.profile().clone())
            .unwrap();
    let outline = parsed
        .outline(
            original.def_key.crate_key,
            vec![],
            provider.module_source().captured_macro_cfg().unwrap(),
        )
        .unwrap();
    let actual = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "X")
        .unwrap();
    assert_eq!(
        anchor.body_range(),
        actual.body.as_ref().map(|body| body.range)
    );
    let index = parsed
        .index_body(actual.body.as_ref().unwrap(), BodyIndexLimits::default())
        .unwrap();
    assert_eq!(
        index
            .nodes()
            .iter()
            .filter(|node| matches!(
                node.role,
                crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Literal
                )
            ))
            .count(),
        2
    );
}

#[test]
fn upstream_nested_statement_and_expression_requests_attach_to_ordinary_bodies() {
    for edition in [2015, 2018, 2021, 2024] {
        for (stock, remaining) in [(NESTED, 0), (EXPRESSION, 1)] {
            let (provider, index) = provider(stock, edition);
            let outer = request(&provider, &index);
            let generated = output(&provider, index.scan(), &outer, 1);
            let assembly = SourceSyntaxAssembly::new(&provider, &index, 1)
                .unwrap()
                .attach(&outer, generated.clone())
                .unwrap();
            let nested = GeneratedDeclarationMacroIndex::new(
                generated.clone(),
                DeclarationMacroParent::Source(index.clone()),
            )
            .unwrap();
            let invocation = generated
                .scan()
                .producers()
                .iter()
                .find(|p| p.kind == DeclarationProducerKind::Macro)
                .unwrap()
                .identity;
            let SourceDeclarativeDemand::Ready(inner) = provider
                .generated_declaration_demand(&nested, invocation)
                .unwrap()
            else {
                panic!("nested demand incomplete")
            };
            let child = output(&provider, generated.scan(), &inner, 1);
            let prepared = assembly.attach(&inner, child).unwrap().finish().unwrap();
            assert_eq!(prepared.source(), provider.source.selection());
            assert_eq!(prepared.epoch(), 1);
            assert_eq!(prepared.outputs().len(), 2);
            assert_eq!(prepared.pending_calls().len(), remaining);
            assert_eq!(
                prepared.macro_definitions().len(),
                index.scan().macro_definitions().len()
            );
            for header in index.scan().macro_definitions() {
                let anchor = prepared
                    .macro_definitions()
                    .iter()
                    .find(|anchor| anchor.definition().identity == header.identity)
                    .expect("actual original macro declaration occurrence");
                assert_eq!(
                    anchor.definition().defining_unit,
                    index.scan().scope().crate_key
                );
                assert_eq!(anchor.definition().definition_edition, edition);
            }
            for issued in [&outer, &inner] {
                let definition = prepared
                    .expansion_definitions()
                    .get(&issued.key().fingerprint())
                    .expect("actual issued expansion definition");
                assert_eq!(
                    Some(definition),
                    receipt::SyntaxRequestReceipt::capture(issued)
                        .unwrap()
                        .definition
                        .as_ref()
                );
            }
            for original in &provider.module_source().outline().items {
                let anchor = prepared
                    .original_declarations()
                    .iter()
                    .find(|anchor| anchor.original_definition() == original.def_key.fingerprint())
                    .expect("original declaration anchor");
                assert_eq!(anchor.original_range(), original.declaration_span);
            }
            let parsed = ParsedFile::from_syntax_snapshot(
                prepared.syntax().clone(),
                provider.profile().clone(),
            )
            .unwrap();
            let outline = parsed
                .outline(
                    prepared.source().outline.crate_key,
                    vec![],
                    &CfgSet::guest_default(),
                )
                .unwrap();
            let body = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "main")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            parsed.index_body(body, BodyIndexLimits::default()).unwrap();
            let facts = parsed
                .entered_macro_facts(body, &CfgSet::guest_default(), &outline.root_scope)
                .unwrap();
            assert_eq!(
                facts
                    .items
                    .iter()
                    .map(|i| i.item.def_key.declared_name.as_str())
                    .collect::<Vec<_>>(),
                ["Bar", "Baz", "Xyzzy"]
            );
        }
    }
}

#[test]
fn copied_macro_definition_origins_keep_distinct_source_occurrences() {
    // LRR source-identity regression, supplementing the unchanged upstream
    // nested/wrapped attachment witnesses above. Both emitted definitions have
    // the same token origins and body, but distinct admitted scan identities.
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, index) = provider(
            "macro_rules! duplicate{($item:item)=>{$item $item}} duplicate!(macro_rules! inner{()=>{1_i32}});fn main(){}",
            edition,
        );
        let issued = request(&provider, &index);
        let generated = output(&provider, index.scan(), &issued, 1);
        assert_eq!(generated.scan().macro_definitions().len(), 2);
        let prepared = SourceSyntaxAssembly::new(&provider, &index, 1)
            .unwrap()
            .attach(&issued, generated.clone())
            .unwrap()
            .finish()
            .unwrap();
        let selected: Vec<_> = generated
            .scan()
            .macro_definitions()
            .iter()
            .map(|header| {
                prepared
                    .macro_definitions()
                    .iter()
                    .find(|anchor| anchor.definition().identity == header.identity)
                    .unwrap()
            })
            .collect();
        assert_ne!(
            selected[0].definition().identity,
            selected[1].definition().identity
        );
        assert_ne!(selected[0].range(), selected[1].range());
        let actual = prepared.syntax().syntax();
        macro_anchors::validate(prepared.macro_definitions(), actual).unwrap();
        let mut duplicated = prepared.macro_definitions().to_vec();
        duplicated.push(selected[0].clone());
        assert!(macro_anchors::validate(&duplicated, actual).is_err());
    }
}

#[test]
fn upstream_forwarded_statement_keeps_the_issued_call_and_generated_let() {
    let (provider, index) = provider(FORWARDED, 2024);
    let outer = request(&provider, &index);
    let generated = output(&provider, index.scan(), &outer, 3);
    let assembly = SourceSyntaxAssembly::new(&provider, &index, 3)
        .unwrap()
        .attach(&outer, generated.clone())
        .unwrap();
    let nested = GeneratedDeclarationMacroIndex::new(
        generated.clone(),
        DeclarationMacroParent::Source(index),
    )
    .unwrap();
    let invocation = generated
        .scan()
        .producers()
        .iter()
        .find(|p| p.kind == DeclarationProducerKind::Macro)
        .unwrap()
        .identity;
    let SourceDeclarativeDemand::Ready(inner) = provider
        .generated_declaration_demand(&nested, invocation)
        .unwrap()
    else {
        panic!("nested statement demand incomplete")
    };
    let child = output(&provider, generated.scan(), &inner, 3);
    let prepared = assembly.attach(&inner, child).unwrap().finish().unwrap();
    assert!(prepared.pending_calls().is_empty());
    let parsed =
        ParsedFile::from_syntax_snapshot(prepared.syntax().clone(), provider.profile().clone())
            .unwrap();
    let outline = parsed
        .outline(
            prepared.source().outline.crate_key,
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    parsed.index_body(body, BodyIndexLimits::default()).unwrap();
    assert!(prepared.syntax().syntax().text.contains("_x"));
}

#[test]
fn rejects_another_issued_record_without_disguising_it_as_source() {
    let (provider, index) = provider(NESTED, 2024);
    let first = request(&provider, &index);
    let mut other = request(&provider, &index);
    other.key.consumer_compile_env = Digest([29; 32]);
    let generated = output(&provider, index.scan(), &other, 1);
    let error = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .attach(&first, generated)
        .err()
        .unwrap();
    assert!(error.message.contains("request/record/epoch"));
    assert_eq!(
        provider.source.selection().outline.parse.content,
        provider.source.captured_macro_blob().unwrap().content()
    );
}

#[test]
fn rejects_wrong_source_scan_zero_epoch_and_wrong_output_epoch() {
    let (provider, index) = provider(NESTED, 2024);
    let (_, foreign) = self::provider(EXPRESSION, 2024);
    assert!(SourceSyntaxAssembly::new(&provider, &foreign, 1).is_err());
    assert!(SourceSyntaxAssembly::new(&provider, &index, 0).is_err());
    let issued = request(&provider, &index);
    let generated = output(&provider, index.scan(), &issued, 2);
    assert!(
        SourceSyntaxAssembly::new(&provider, &index, 1)
            .unwrap()
            .attach(&issued, generated)
            .is_err()
    );
}

#[test]
fn already_attached_request_cannot_be_applied_twice() {
    let (provider, index) = provider(NESTED, 2024);
    let issued = request(&provider, &index);
    let generated = output(&provider, index.scan(), &issued, 1);
    let assembly = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .attach(&issued, generated.clone())
        .unwrap();
    let error = assembly.attach(&issued, generated).err().unwrap();
    assert!(error.message.contains("not pending"));
}

#[test]
fn repeated_origin_keys_keep_distinct_generated_invocation_occurrences() {
    // LRR identity regression: both copies have identical argument-origin keys,
    // but must consume different genuine generated-scan invocation identities.
    let text = "macro_rules! duplicate{($call:stmt)=>{$call $call}} macro_rules! leaf{()=>{}} fn main(){duplicate!(leaf!());}";
    let (provider, index) = provider(text, 2024);
    let issued = request(&provider, &index);
    let generated = output(&provider, index.scan(), &issued, 1);
    let mut assembly = SourceSyntaxAssembly::new(&provider, &index, 1)
        .unwrap()
        .attach(&issued, generated.clone())
        .unwrap();
    let ids = generated
        .scan()
        .producers()
        .iter()
        .filter(|p| p.kind == DeclarationProducerKind::Macro)
        .map(|p| p.identity)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    let mut nested = GeneratedDeclarationMacroIndex::new(
        generated.clone(),
        DeclarationMacroParent::Source(index),
    )
    .unwrap();
    let mut sites = Vec::new();
    for invocation in ids {
        let SourceDeclarativeDemand::Ready(issued) = provider
            .generated_declaration_demand(&nested, invocation)
            .unwrap()
        else {
            panic!("repeated demand incomplete")
        };
        sites.push(issued.site);
        let child = output(&provider, generated.scan(), &issued, 1);
        let child_index = GeneratedDeclarationMacroIndex::new(
            child.clone(),
            DeclarationMacroParent::Generated(Arc::new(nested.clone())),
        )
        .unwrap();
        assembly = assembly.attach(&issued, child).unwrap();
        nested
            .adopt_completed_child(issued.key(), &child_index)
            .unwrap();
    }
    assert_eq!(sites[0], sites[1]);
    let prepared = assembly.finish().unwrap();
    assert!(prepared.pending_calls().is_empty());
    assert_eq!(prepared.outputs().len(), 3);
}

#[test]
fn issued_attachment_receipt_cannot_authorize_other_token_roots_or_sites() {
    let (provider, index, _) = world(
        "macro_rules! value { () => { 1_i32 + 2_i32 } } fn main() { value!(); }",
        2024,
    );
    let mut request = request(&provider, &index);
    let output = output(&provider, index.scan(), &request, 1);
    let receipt = SyntaxRequestReceipt::capture(&request).unwrap();
    for alter_site in [false, true] {
        let mut wrong = receipt.clone();
        if alter_site {
            wrong.site = lrr_expand_adapter::token_wire::Key([0; 32]);
        } else {
            wrong.roots = Digest([0; 32]);
        }
        let error = match SourceSyntaxAssembly::new(&provider, &index, 1)
            .unwrap()
            .attach_receipt(&wrong, output.clone())
        {
            Ok(_) => panic!("wrong issued attachment was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.code, "LRR-E-EXPANSION-INPUT");
        assert!(error.message.contains("tokens/site differ"));
    }
    request.key.input_tokens = Digest([0; 32]);
    assert!(
        SyntaxRequestReceipt::capture(&request)
            .unwrap_err()
            .message
            .contains("input differs from its key")
    );
}

#[test]
fn unowned_structural_calls_cannot_disappear_from_the_pending_result() {
    // Declaration discovery correctly excludes this cfg-disabled body. The raw
    // retained syntax still contains a call, so it cannot be represented as a
    // completely attached source. A selective/cfg-aware preparation may leave
    // the disabled body cold; dropping the pending marker is not that proof.
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, index) = provider(
            "#[cfg(any())] fn cold() { unknown!(); } fn main() {}",
            edition,
        );
        assert!(
            !index
                .scan()
                .producers()
                .iter()
                .any(|p| p.kind == DeclarationProducerKind::Macro)
        );
        let assembly = SourceSyntaxAssembly::new(&provider, &index, 1).unwrap();
        assert_eq!(assembly.calls.len(), 1);
        assert!(assembly.calls[0].invocation.is_none());
        assert!(assembly.next_pending_invocation().is_err());
        let expected = assembly.calls[0].anchor;
        let prepared = assembly.finish().unwrap();
        assert_eq!(prepared.pending_calls(), &[expected]);
        assert!(prepared.outputs().is_empty());
    }
}
