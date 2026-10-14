//! Exact original-source token selection. No printing, expansion or effect.
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{
        attributes::ItemAttributeInputs,
        token_wire::{Key, SourceIdentity, TokenForest, TokenTree},
    },
};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-OUTPUT", message.into())
}
pub fn argument_forest(
    inputs: &ItemAttributeInputs,
    index: usize,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<Option<TokenForest>, Diagnostic> {
    forest.validate().map_err(|e| fault(&e.to_string()))?;
    if !inputs.enabled {
        return Err(fault("inactive item has no attribute arguments"));
    }
    if source.content != Key(inputs.parse.content.0) {
        return Err(fault("attribute source differs from frozen parse"));
    }
    let attribute = inputs
        .attributes
        .get(index)
        .ok_or_else(|| fault("attribute index out of range"))?;
    let Some(selected) = attribute.argument_range else {
        return Ok(None);
    };
    if selected.start >= selected.end
        || !attribute.range.contains(selected)
        || !inputs.item.contains(attribute.range)
    {
        return Err(fault(
            "attribute argument range is outside its owning item/meta",
        ));
    }
    let spans: BTreeMap<_, _> = forest.spans().iter().map(|s| (s.key(), s)).collect();
    let mut pending: Vec<_> = forest.roots().iter().collect();
    let mut found = None;
    while let Some(token) = pending.pop() {
        if let TokenTree::Group {
            open,
            close,
            children,
            ..
        } = token
        {
            let first = spans
                .get(open)
                .ok_or_else(|| fault("missing argument open span"))?;
            let last = spans
                .get(close)
                .ok_or_else(|| fault("missing argument close span"))?;
            if &first.source == source
                && &last.source == source
                && first.start == selected.start
                && last.end == selected.end
            {
                if edition(first.edition) != inputs.parse.profile.edition
                    || edition(last.edition) != inputs.parse.profile.edition
                {
                    return Err(fault("attribute span edition differs from parse"));
                }
                if found.replace(token).is_some() {
                    return Err(fault("ambiguous original argument group"));
                }
            }
            pending.extend(children);
        }
    }
    let root = found.ok_or_else(|| fault("original attribute argument group is unavailable"))?;
    prune_tree(forest, root).map(Some)
}
pub(crate) fn prune_tree(
    forest: &TokenForest,
    root: &TokenTree,
) -> Result<TokenForest, Diagnostic> {
    let spans: BTreeMap<_, _> = forest.spans().iter().map(|s| (s.key(), s)).collect();
    let mut needed = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Group {
                open,
                close,
                children,
                ..
            } => {
                needed.insert(*open);
                needed.insert(*close);
                pending.extend(children);
            }
            TokenTree::Ident { span, .. }
            | TokenTree::Punct { span, .. }
            | TokenTree::Literal { span, .. } => {
                needed.insert(*span);
            }
        }
    }
    let selected_spans: Vec<_> = needed.into_iter().map(|key| spans[&key].clone()).collect();
    let contexts: BTreeMap<_, _> = forest.contexts().iter().map(|c| (c.key(), c)).collect();
    let mut needed: BTreeSet<_> = selected_spans.iter().map(|s| s.context).collect();
    let mut pending: Vec<_> = needed.iter().copied().collect();
    while let Some(key) = pending.pop() {
        let record = contexts
            .get(&key)
            .ok_or_else(|| fault("missing argument context"))?;
        if let Some(parent) = record.parent
            && needed.insert(parent)
        {
            pending.push(parent);
        }
    }
    let selected_contexts = needed
        .into_iter()
        .map(|key| contexts[&key].clone())
        .collect();
    TokenForest::new(vec![root.clone()], selected_contexts, selected_spans)
        .map_err(|e| fault(&e.to_string()))
}

fn edition(value: crate::syntax::token_wire::Edition) -> u16 {
    use crate::syntax::token_wire::Edition::*;
    match value {
        E2015 => 2015,
        E2018 => 2018,
        E2021 => 2021,
        E2024 => 2024,
    }
}

/// Attribute macro arguments omit only the outer attribute delimiter.
/// Interior groups, span keys and context rows are unchanged.
pub fn macro_argument_contents(delimited: &TokenForest) -> Result<TokenForest, Diagnostic> {
    delimited.validate().map_err(|e| fault(&e.to_string()))?;
    let [
        TokenTree::Group {
            open,
            close,
            children,
            ..
        },
    ] = delimited.roots()
    else {
        return Err(fault("attribute arguments require one group"));
    };
    TokenForest::new(
        vec![TokenTree::Group {
            delimiter: crate::syntax::token_wire::Delimiter::Invisible,
            open: *open,
            close: *close,
            children: children.clone(),
        }],
        delimited.contexts().to_vec(),
        delimited.spans().to_vec(),
    )
    .map_err(|e| fault(&e.to_string()))
}

#[derive(Debug, Clone)]
pub enum AttributeItemInput {
    Ready(TokenForest),
    NeedCfgMaterialization,
}
/// Select one original item and remove exactly the currently applied outer attribute.
/// cfg_attr-produced attributes require effective-attribute reconstruction first.
pub fn item_without_applied_attribute(
    inputs: &ItemAttributeInputs,
    index: usize,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<AttributeItemInput, Diagnostic> {
    forest.validate().map_err(|e| fault(&e.to_string()))?;
    if !inputs.enabled || source.content != Key(inputs.parse.content.0) {
        return Err(fault("attribute item/source is not active and current"));
    }
    let selected = inputs
        .attributes
        .get(index)
        .ok_or_else(|| fault("attribute index out of range"))?;
    if inputs.requires_cfg_materialization {
        return Ok(AttributeItemInput::NeedCfgMaterialization);
    }
    let Some(attribute) = selected.original_attribute_range else {
        return Ok(AttributeItemInput::NeedCfgMaterialization);
    };
    if !inputs.item.contains(attribute) || !attribute.contains(selected.range) {
        return Err(fault("applied attribute range is outside item"));
    }
    let spans: BTreeMap<_, _> = forest.spans().iter().map(|s| (s.key(), s)).collect();
    let token_range = |token: &TokenTree| -> Result<crate::syntax::spans::SourceRange, Diagnostic> {
        let (first, last) = match token {
            TokenTree::Group { open, close, .. } => (*open, *close),
            TokenTree::Ident { span, .. }
            | TokenTree::Punct { span, .. }
            | TokenTree::Literal { span, .. } => (*span, *span),
        };
        let first = spans
            .get(&first)
            .ok_or_else(|| fault("missing item token span"))?;
        let last = spans
            .get(&last)
            .ok_or_else(|| fault("missing item token span"))?;
        if &first.source != source || &last.source != source {
            return Err(fault("original item contains a foreign source"));
        }
        crate::syntax::spans::SourceRange::checked(first.start, last.end)
    };
    let mut pending: Vec<_> = forest.roots().iter().collect();
    let mut selected_tree = None;
    while let Some(token) = pending.pop() {
        if let TokenTree::Group {
            open,
            close,
            children,
            ..
        } = token
        {
            for (position, first) in children.iter().enumerate() {
                if matches!(first, TokenTree::Punct { scalar: '#', .. })
                    && token_range(first)?.start == attribute.start
                {
                    let second = children
                        .get(position + 1)
                        .ok_or_else(|| fault("attribute delimiter missing"))?;
                    if !matches!(
                        second,
                        TokenTree::Group {
                            delimiter: crate::syntax::token_wire::Delimiter::Bracket,
                            ..
                        }
                    ) || token_range(second)?.end != attribute.end
                    {
                        return Err(fault("applied attribute token bounds mismatch"));
                    }
                    let mut kept = Vec::new();
                    for (i, child) in children.iter().enumerate() {
                        let range = token_range(child)?;
                        if inputs.item.contains(range) && i != position && i != position + 1 {
                            kept.push(child.clone());
                        }
                    }
                    if kept.is_empty() {
                        return Err(fault("attribute removal produced an empty item"));
                    }
                    let candidate = TokenTree::Group {
                        delimiter: crate::syntax::token_wire::Delimiter::Invisible,
                        open: *open,
                        close: *close,
                        children: kept,
                    };
                    if selected_tree.replace(candidate).is_some() {
                        return Err(fault("ambiguous applied attribute item"));
                    }
                }
            }
            pending.extend(children);
        }
    }
    let root =
        selected_tree.ok_or_else(|| fault("original applied attribute tokens unavailable"))?;
    prune_tree(forest, &root).map(AttributeItemInput::Ready)
}
