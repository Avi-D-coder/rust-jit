//! Real issued-output source controls, never guest/member compatibility passes.
use super::*;
use crate::{
    expand::{
        generated_items::GeneratedItems,
        source_declarative::{SourceDeclarative, SourceDeclarativeDemand},
    },
    frontend::impl_index::{
        DeclarationMacroIndex, DeclarationMacroParent, GeneratedDeclarationMacroIndex,
    },
};
const STOCK: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-as-fn-body.rs"
);
fn original(source: &PreparedSourceView) -> PreparedOriginalAssociatedDeclarations<'_> {
    let AssociatedDeclarationsPreparation::Original(value) =
        PreparedOriginalAssociatedDeclarations::prepare(source).unwrap()
    else {
        panic!("actual original source absent")
    };
    value
}
fn request(
    provider: &SourceDeclarative,
    index: &Arc<DeclarationMacroIndex>,
    invocation: Digest,
) -> Box<SourceDeclarativeRequest> {
    let SourceDeclarativeDemand::Ready(request) = provider
        .declaration_demand(
            index,
            invocation,
            Digest([208; 32]),
            crate::expand::source_declarative::engine_version(),
        )
        .unwrap()
    else {
        panic!("actual associated source demand unfinished")
    };
    request
}
fn output(
    provider: &SourceDeclarative,
    parent: &DeclarationScan,
    request: &SourceDeclarativeRequest,
) -> Arc<GeneratedDeclarations> {
    Arc::new(
        GeneratedDeclarations::admit(
            request.key(),
            Arc::new(request.expand().unwrap()),
            1,
            parent,
            provider.module_source().captured_macro_cfg().unwrap(),
            Default::default(),
        )
        .unwrap(),
    )
}
#[test]
fn actual_stock_trait_and_inherent_outputs_keep_real_canonical_owner_and_generated_identity() {
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, index, _) =
            crate::expand::source_declarative::assembly::tests::world(STOCK, edition);
        let root = original(provider.module_source());
        let mut identities = Vec::new();
        for producer in index.scan().producers().iter().filter(|p| {
            matches!(p.grammar, GrammarSlot::TraitItems | GrammarSlot::ImplItems)
                && p.kind == DeclarationProducerKind::Macro
        }) {
            let request = request(&provider, &index, producer.identity);
            let output = output(&provider, index.scan(), &request);
            let chain = [IssuedAssociatedOutput {
                request: &request,
                output: Arc::clone(&output),
            }];
            let prepared = PreparedGeneratedAssociatedDeclarations::prepare(
                &root,
                producer.identity,
                &chain,
                1,
            )
            .unwrap();
            assert!(std::ptr::eq(prepared.source(), provider.module_source()));
            assert_eq!(prepared.len(), 1);
            assert!(Arc::ptr_eq(&prepared.outputs()[0], &output));
            assert_eq!(prepared.receipts()[0].key(), request.key());
            let header = prepared.header(0).unwrap();
            assert_eq!(header.source.kind, AssociatedDeclarationKind::Function);
            assert_eq!(header.source.name.as_ref().unwrap().spelling, "bar");
            assert!(matches!(
                header.source.name.as_ref().unwrap().context,
                DeclarationNameContext::Captured(_)
            ));
            assert_eq!(header.key.expansion, request.key().fingerprint());
            assert_eq!(header.key, &output.headers()[0].key);
            let actual = header.declaration.unwrap();
            assert_eq!(actual.def_key.item_kind, ItemKind::Function);
            assert_eq!(actual.header.context, header.owner.header.tokens);
            assert_eq!(actual.scope.owner, Some(prepared.owner_identity()));
            assert_eq!(header.body.unwrap().def_key, header.key.fingerprint());
            assert_eq!(header.body.unwrap().range, header.source.body.unwrap());
            assert_ne!(header.key.fingerprint(), actual.def_key.fingerprint());
            assert!(
                !actual
                    .header
                    .tokens
                    .iter()
                    .any(|token| token.text == "println")
            );
            identities.push(header.key.fingerprint());
            let before = prepared.fingerprint();
            for _ in 0..8 {
                assert!(std::ptr::eq(
                    prepared.header(0).unwrap().source,
                    header.source
                ));
                assert_eq!(prepared.fingerprint(), before);
            }
            assert!(prepared.metadata_bytes() > std::mem::size_of_val(&prepared));
        }
        assert_eq!(identities.len(), 2);
        assert_ne!(identities[0], identities[1]);
    }
}
#[test]
fn exact_items_header_keys_are_shared_with_existing_generated_items_transport() {
    let (provider, index, _) =
        crate::expand::source_declarative::assembly::tests::world(STOCK, 2015);
    let producer = index
        .scan()
        .producers()
        .iter()
        .find(|p| p.grammar == GrammarSlot::Items && p.kind == DeclarationProducerKind::Macro)
        .unwrap();
    let request = request(&provider, &index, producer.identity);
    let record = Arc::new(request.expand().unwrap());
    let items = GeneratedItems::admit(request.key(), Arc::clone(&record), 1).unwrap();
    let declarations = GeneratedDeclarations::admit(
        request.key(),
        record,
        1,
        index.scan(),
        provider.module_source().captured_macro_cfg().unwrap(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(items.items(), declarations.headers());
}
#[test]
fn wrong_actual_owner_request_epoch_and_missing_chain_refuse_without_semantic_answer() {
    let (provider, index, _) =
        crate::expand::source_declarative::assembly::tests::world(STOCK, 2015);
    let root = original(provider.module_source());
    let producers = index
        .scan()
        .producers()
        .iter()
        .filter(|p| {
            matches!(p.grammar, GrammarSlot::TraitItems | GrammarSlot::ImplItems)
                && p.kind == DeclarationProducerKind::Macro
        })
        .collect::<Vec<_>>();
    assert_eq!(producers.len(), 2);
    let first = request(&provider, &index, producers[0].identity);
    let second = request(&provider, &index, producers[1].identity);
    let first_output = output(&provider, index.scan(), &first);
    let second_output = output(&provider, index.scan(), &second);
    let wrong = [IssuedAssociatedOutput {
        request: &first,
        output: Arc::clone(&second_output),
    }];
    let error =
        PreparedGeneratedAssociatedDeclarations::prepare(&root, producers[0].identity, &wrong, 1)
            .err()
            .unwrap();
    assert_eq!(error.code, "LRR-E-ASSOCIATED-DECLARATION");
    assert_eq!(
        error.message,
        "associated issued output source/record/parent/epoch differs"
    );
    let valid = [IssuedAssociatedOutput {
        request: &first,
        output: Arc::clone(&first_output),
    }];
    assert_eq!(
        PreparedGeneratedAssociatedDeclarations::prepare(&root, producers[0].identity, &valid, 2)
            .err()
            .unwrap()
            .message,
        "associated issued output source/record/parent/epoch differs"
    );
    assert_eq!(
        PreparedGeneratedAssociatedDeclarations::prepare(&root, producers[1].identity, &valid, 1)
            .err()
            .unwrap()
            .message,
        "associated issued output source/record/parent/epoch differs"
    );
    assert_eq!(
        PreparedGeneratedAssociatedDeclarations::prepare(&root, producers[0].identity, &[], 1)
            .err()
            .unwrap()
            .message,
        "associated issued chain count/epoch differs"
    );
    let swapped = [
        IssuedAssociatedOutput {
            request: &second,
            output: second_output,
        },
        IssuedAssociatedOutput {
            request: &first,
            output: first_output,
        },
    ];
    assert_eq!(
        PreparedGeneratedAssociatedDeclarations::prepare(&root, producers[1].identity, &swapped, 1)
            .err()
            .unwrap()
            .message,
        "associated issued parent producer absent"
    );
}
#[test]
fn genuine_wrapper_chain_keeps_parent_attachment_and_equal_outputs_distinct() {
    let supplement = "\nmacro_rules! lrr_wrap_assoc { () => { def_fn!({}); }; }\ntrait LrrWrappedOne { lrr_wrap_assoc!(); }\ntrait LrrWrappedTwo { lrr_wrap_assoc!(); }\n";
    for edition in [2015, 2018, 2021, 2024] {
        let text = format!("{STOCK}{supplement}");
        assert!(text.starts_with(STOCK));
        let (provider, index, _) =
            crate::expand::source_declarative::assembly::tests::world(&text, edition);
        let root = original(provider.module_source());
        let mut equal_outputs = Vec::new();
        let mut keys = Vec::new();
        let mut owners = Vec::new();
        for producer in index.scan().producers().iter().filter(|p| {
            p.path.iter().any(|t| t.text == "lrr_wrap_assoc")
                && p.kind == DeclarationProducerKind::Macro
        }) {
            let issued = request(&provider, &index, producer.identity);
            let parent = output(&provider, index.scan(), &issued);
            let parent_index = GeneratedDeclarationMacroIndex::new(
                Arc::clone(&parent),
                DeclarationMacroParent::Source(Arc::clone(&index)),
            )
            .unwrap();
            let child_invocation = parent
                .scan()
                .associated_fragments()
                .iter()
                .find_map(|r| r.producer)
                .unwrap();
            let SourceDeclarativeDemand::Ready(child_request) = provider
                .generated_declaration_demand(&parent_index, child_invocation, None)
                .unwrap()
            else {
                panic!("actual wrapper child demand absent")
            };
            let child = output(&provider, parent.scan(), &child_request);
            let chain = [
                IssuedAssociatedOutput {
                    request: &issued,
                    output: Arc::clone(&parent),
                },
                IssuedAssociatedOutput {
                    request: &child_request,
                    output: Arc::clone(&child),
                },
            ];
            let prepared = PreparedGeneratedAssociatedDeclarations::prepare(
                &root,
                producer.identity,
                &chain,
                1,
            )
            .unwrap();
            assert_eq!(prepared.outputs().len(), 2);
            assert_eq!(prepared.len(), 1);
            let header = prepared.header(0).unwrap();
            assert_eq!(header.declaration.unwrap().def_key.declared_name, "bar");
            assert_eq!(child.parent_inventory(), parent.scan().fingerprint());
            assert_eq!(child.parent_invocation(), child_invocation);
            equal_outputs.push(child.text().to_owned());
            keys.push(header.key.fingerprint());
            owners.push(prepared.owner_identity());
            // The real wrapper output alone is an unfinished producer fact,
            // never a manufactured function or declaration-closure result.
            let prefix = PreparedGeneratedAssociatedDeclarations::prepare(
                &root,
                producer.identity,
                &chain[..1],
                1,
            )
            .unwrap();
            assert!(prefix.header(0).unwrap().declaration.is_none());
            assert!(prefix.header(0).unwrap().body.is_none());
            let missing_parent = [IssuedAssociatedOutput {
                request: &child_request,
                output: child,
            }];
            assert_eq!(
                PreparedGeneratedAssociatedDeclarations::prepare(
                    &root,
                    producer.identity,
                    &missing_parent,
                    1
                )
                .err()
                .unwrap()
                .message,
                "associated issued parent producer absent"
            );
        }
        assert_eq!(equal_outputs.len(), 2);
        assert_eq!(equal_outputs[0], equal_outputs[1]);
        assert_ne!(keys[0], keys[1]);
        assert_ne!(owners[0], owners[1]);
    }
}

#[test]
fn generated_cfg_active_rows_join_actual_header_paths_without_reviving_disabled_members() {
    let supplement = "\nmacro_rules! lrr_cfg_assoc { () => { #[cfg(any())] fn off() { missing(); } fn on() {} }; }\ntrait LrrCfg { lrr_cfg_assoc!(); }\n";
    for edition in [2015, 2018, 2021, 2024] {
        let text = format!("{STOCK}{supplement}");
        assert!(text.starts_with(STOCK));
        let (provider, index, _) =
            crate::expand::source_declarative::assembly::tests::world(&text, edition);
        let original = original(provider.module_source());
        let producer = index
            .scan()
            .producers()
            .iter()
            .find(|row| {
                row.path.iter().any(|token| token.text == "lrr_cfg_assoc")
                    && row.kind == DeclarationProducerKind::Macro
            })
            .unwrap();
        let issued = request(&provider, &index, producer.identity);
        let output = output(&provider, index.scan(), &issued);
        assert_eq!(output.headers().len(), 2);
        assert_eq!(output.scan().associated_fragments().len(), 1);
        let chain = [IssuedAssociatedOutput {
            request: &issued,
            output: Arc::clone(&output),
        }];
        let prepared = PreparedGeneratedAssociatedDeclarations::prepare(
            &original,
            producer.identity,
            &chain,
            1,
        )
        .unwrap();
        assert_eq!(prepared.len(), 1);
        let member = prepared.header(0).unwrap();
        assert_eq!(member.source.name.as_ref().unwrap().spelling, "on");
        assert_eq!(member.declaration.unwrap().def_key.declared_name, "on");
        assert_eq!(member.key, &output.headers()[1].key);
        assert_eq!(member.key.structural_path, vec![1]);
        assert!(
            !member
                .declaration
                .unwrap()
                .header
                .tokens
                .iter()
                .any(|token| token.text == "missing")
        );
    }
}
