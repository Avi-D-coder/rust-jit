//! Effective attributes assembled from original token subtrees, never display text.
use super::attributes::{fault, prune_tree};
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{
        attributes::ItemAttributeInputs,
        spans::SourceRange,
        token_wire::{Delimiter, Key, SourceIdentity, SpanRecord, TokenForest, TokenTree},
    },
};
use std::collections::BTreeMap;
pub(crate) fn token_range(
    token: &TokenTree,
    source: &SourceIdentity,
    spans: &BTreeMap<Key, &SpanRecord>,
) -> Result<SourceRange, Diagnostic> {
    let (first, last) = match token {
        TokenTree::Group { open, close, .. } => (*open, *close),
        TokenTree::Ident { span, .. }
        | TokenTree::Punct { span, .. }
        | TokenTree::Literal { span, .. } => (*span, *span),
    };
    let first = spans
        .get(&first)
        .ok_or_else(|| fault("missing materialization span"))?;
    let last = spans
        .get(&last)
        .ok_or_else(|| fault("missing materialization span"))?;
    if &first.source != source || &last.source != source {
        return Err(fault("materialization contains foreign source"));
    }
    SourceRange::checked(first.start, last.end)
}
pub(crate) fn meta_tokens(
    tokens: &[TokenTree],
    selected: SourceRange,
    source: &SourceIdentity,
    spans: &BTreeMap<Key, &SpanRecord>,
) -> Result<Vec<TokenTree>, Diagnostic> {
    let mut result = Vec::new();
    for token in tokens {
        let range = token_range(token, source, spans)?;
        if selected.contains(range) {
            result.push(token.clone());
        } else if range.contains(selected)
            && let TokenTree::Group { children, .. } = token
        {
            result.extend(meta_tokens(children, selected, source, spans)?);
        }
    }
    if result.is_empty() {
        return Err(fault("effective meta tokens unavailable"));
    }
    Ok(result)
}
/// Produce native item input after cfg filtering and removal of the applied attribute.
pub fn materialize(
    inputs: &ItemAttributeInputs,
    applied: usize,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    materialize_for(inputs, Some(applied), &[], source, forest)
}
/// Remove only preceding annotations certified by the source resolver as
/// consumed compiler transformations. This never classifies by spelling.
pub(crate) fn after_compiler_prefix(
    inputs: &ItemAttributeInputs,
    applied: usize,
    consumed: &[usize],
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    if consumed.iter().any(|index| *index >= applied)
        || consumed.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(fault("consumed compiler prefix indexes are invalid"));
    }
    materialize_for(inputs, Some(applied), consumed, source, forest)
}
/// Keep raw member tokens for a replacing attribute after completed derives.
/// Only the selected attribute and certified preceding derive groups are removed.
pub(crate) fn after_derives(
    inputs: &ItemAttributeInputs,
    applied: usize,
    derives: &[usize],
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    if derives.iter().any(|index| *index >= applied)
        || derives
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != derives.len()
    {
        return Err(fault("preceding derive annotation indexes are invalid"));
    }
    materialize_for(inputs, Some(applied), derives, source, forest)
}
/// Derive input retains the item and inert/helper attributes, with all derives removed.
/// The returned macro output is appended by the expansion coordinator.
pub fn derive_input(
    inputs: &ItemAttributeInputs,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    let indices: Vec<_> = inputs
        .attributes
        .iter()
        .enumerate()
        .filter_map(|(i, a)| (a.name == "derive").then_some(i))
        .collect();
    derive_input_resolved(inputs, &indices, source, forest)
}
/// Remove exactly the annotations certified as compiler-owned derive identities.
pub fn derive_input_resolved(
    inputs: &ItemAttributeInputs,
    indices: &[usize],
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    if indices.iter().any(|i| *i >= inputs.attributes.len())
        || indices
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != indices.len()
    {
        return Err(fault("resolved derive annotation indexes are invalid"));
    }
    materialize_for(inputs, None, indices, source, forest)
}
fn materialize_for(
    inputs: &ItemAttributeInputs,
    applied: Option<usize>,
    derives: &[usize],
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    forest.validate().map_err(|e| fault(&e.to_string()))?;
    if !inputs.enabled || source.content != Key(inputs.parse.content.0) {
        return Err(fault("effective item/source is inactive or stale"));
    }
    if applied.is_some_and(|index| index >= inputs.attributes.len()) {
        return Err(fault("applied attribute index out of range"));
    }
    let spans: BTreeMap<_, _> = forest.span_keys().iter().copied().zip(forest.spans()).collect();
    let mut pending: Vec<_> = forest.roots().iter().collect();
    let mut candidate = None;
    while let Some(token) = pending.pop() {
        if let TokenTree::Group {
            open,
            close,
            children,
            ..
        } = token
        {
            let mut wrappers = BTreeMap::new();
            for pair in children.windows(2) {
                if let TokenTree::Punct { scalar: '#', .. } = &pair[0]
                    && let TokenTree::Group {
                        delimiter: Delimiter::Bracket,
                        ..
                    } = &pair[1]
                {
                    let range = SourceRange {
                        start: token_range(&pair[0], source, &spans)?.start,
                        end: token_range(&pair[1], source, &spans)?.end,
                    };
                    if inputs.original_attribute_ranges.contains(&range) {
                        wrappers.insert((range.start, range.end), (&pair[0], &pair[1]));
                    }
                }
            }
            if !inputs.original_attribute_ranges.is_empty()
                && wrappers.len() == inputs.original_attribute_ranges.len()
            {
                // Reconstruct each explicit source wrapper at its original position.
                // Desugared doc comments are not ast::Attr rows, so prepending the
                // entire effective attribute list would move them past later macros.
                let mut effective_wrappers = BTreeMap::<(u32, u32), Vec<TokenTree>>::new();
                for (index, attribute) in inputs.attributes.iter().enumerate() {
                    if Some(index) == applied || attribute.name == "cfg" || derives.contains(&index)
                    {
                        continue;
                    }
                    let wrapper = attribute.enclosing_attribute_range;
                    let (hash, group) = wrappers
                        .get(&(wrapper.start, wrapper.end))
                        .ok_or_else(|| fault("effective attribute wrapper missing"))?;
                    let TokenTree::Group {
                        open,
                        close,
                        children,
                        ..
                    } = group
                    else {
                        return Err(fault("effective attribute wrapper is not a group"));
                    };
                    let content = meta_tokens(children, attribute.range, source, &spans)?;
                    let effective = effective_wrappers
                        .entry((wrapper.start, wrapper.end))
                        .or_default();
                    effective
                        .try_reserve(2)
                        .map_err(|_| fault("effective attribute allocation"))?;
                    effective.push((*hash).clone());
                    effective.push(TokenTree::Group {
                        delimiter: Delimiter::Bracket,
                        open: *open,
                        close: *close,
                        children: content,
                    });
                }
                let mut effective = Vec::new();
                let mut cursor = 0usize;
                while let Some(child) = children.get(cursor) {
                    let range = token_range(child, source, &spans)?;
                    if inputs.item.contains(range) {
                        if matches!(child, TokenTree::Punct { scalar: '#', .. })
                            && let Some(
                                next @ TokenTree::Group {
                                    delimiter: Delimiter::Bracket,
                                    ..
                                },
                            ) = children.get(
                                cursor
                                    .checked_add(1)
                                    .ok_or_else(|| fault("effective item cursor overflow"))?,
                            )
                        {
                            let wrapper = (range.start, token_range(next, source, &spans)?.end);
                            if wrappers.contains_key(&wrapper) {
                                if let Some(tokens) = effective_wrappers.remove(&wrapper) {
                                    effective
                                        .try_reserve(tokens.len())
                                        .map_err(|_| fault("effective item allocation"))?;
                                    effective.extend(tokens);
                                }
                                cursor = cursor
                                    .checked_add(2)
                                    .ok_or_else(|| fault("effective item cursor overflow"))?;
                                continue;
                            }
                        }
                        if !inputs
                            .original_attribute_ranges
                            .iter()
                            .any(|attribute| attribute.contains(range))
                        {
                            effective
                                .try_reserve(1)
                                .map_err(|_| fault("effective item allocation"))?;
                            effective.push(child.clone());
                        }
                    }
                    cursor = cursor
                        .checked_add(1)
                        .ok_or_else(|| fault("effective item cursor overflow"))?;
                }
                if !effective_wrappers.is_empty() {
                    return Err(fault("effective source wrappers were not replayed"));
                }
                let root = TokenTree::Group {
                    delimiter: Delimiter::Invisible,
                    open: *open,
                    close: *close,
                    children: effective,
                };
                if candidate.replace(root).is_some() {
                    return Err(fault("ambiguous effective attribute item"));
                }
            }
            pending.extend(children);
        }
    }
    prune_tree(
        forest,
        &candidate.ok_or_else(|| fault("original effective attribute item unavailable"))?,
    )
}

#[cfg(test)]
mod doc_order_tests {
    use super::*;
    use crate::{
        core::{digest::Digest, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::parse::ParsedFile,
    };
    use lrr_expand_adapter::token_wire::{ContextRecord, Edition, Transparency};

    const UPSTREAM: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/attributes-included.rs"
    );

    fn original_component() -> &'static str {
        &UPSTREAM[UPSTREAM.find("#[bar]").unwrap()..UPSTREAM.find("\nfn main()").unwrap()]
    }

    fn materialized_names(source: &str, cfg: &CfgSet) -> Vec<String> {
        let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest::of_bytes(b"doc-order-unit"), vec![], cfg)
            .unwrap();
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "foo")
            .unwrap();
        let inputs = parsed
            .attribute_inputs_at(item.declaration_span, cfg)
            .unwrap();
        let applied = inputs
            .attributes
            .iter()
            .position(|attribute| attribute.name == "bar")
            .unwrap();
        let original = lrr_expand_adapter::token_wire::parse_source(
            source,
            lrr_expand_adapter::token_wire::SourceIdentity {
                unit: lrr_expand_adapter::token_wire::Key(outline.root_scope.crate_key.0),
                content: lrr_expand_adapter::token_wire::Key(blob.content().0),
                path: "attributes-included.rs".into(),
            },
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: lrr_expand_adapter::token_wire::Key(outline.root_scope.crate_key.0),
                edition: Edition::E2024,
            },
            lrr_expand_adapter::token_wire::Key(Digest::of_bytes(b"doc-order-origin").0),
        )
        .unwrap();
        let forest = TokenForest::decode(&original.encode().unwrap()).unwrap();
        let output = materialize(&inputs, applied, &forest.spans()[0].source, &forest).unwrap();
        let [
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            },
        ] = output.roots()
        else {
            panic!("materialized item root absent")
        };
        let mut names = Vec::new();
        for token in children {
            if let TokenTree::Group {
                delimiter: Delimiter::Bracket,
                children,
                ..
            } = token
            {
                let TokenTree::Ident { text, .. } = &children[0] else {
                    panic!("attribute name absent")
                };
                if text == "doc" {
                    assert_eq!(children.len(), 3);
                    assert!(matches!(&children[1], TokenTree::Punct { scalar: '=', .. }));
                    assert!(
                        matches!(&children[2], TokenTree::Literal { token_text, .. } if token_text == "\" doc\"")
                    );
                }
                names.push(text.clone());
            }
        }
        for span in output.spans() {
            assert!(forest.spans().contains(span));
        }
        for context in output.contexts() {
            assert!(forest.contexts().contains(context));
        }
        names
    }

    #[test]
    fn upstream_doc_comments_keep_their_positions_between_explicit_attributes() {
        let source = original_component();
        assert!(source.contains("let a: i32 = \"foo\";"));
        assert_eq!(source.matches("/// doc").count(), 2);
        assert_eq!(
            materialized_names(source, &CfgSet::default()),
            ["inline", "doc", "foo", "inline", "doc"]
        );
    }

    #[test]
    fn cfg_attribute_replacement_preserves_interleaved_original_doc_positions() {
        let source = original_component().replacen("#[inline]", "#[cfg_attr(on, inline, cold)]", 1);
        let mut cfg = CfgSet::default();
        assert_eq!(
            materialized_names(&source, &cfg),
            ["doc", "foo", "inline", "doc"]
        );
        cfg.atoms.insert("on".into());
        assert_eq!(
            materialized_names(&source, &cfg),
            ["inline", "cold", "doc", "foo", "inline", "doc"]
        );
    }
}
