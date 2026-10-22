//! Derive member cfg filtering from pinned AST facts and original token ranges.
use super::{
    attribute_materialize::{meta_tokens, token_range},
    attributes::{fault, prune_tree},
};
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{
        attributes::DeriveItemInputs,
        token_wire::{Delimiter, Key, SourceIdentity, SpanRecord, TokenForest, TokenTree},
    },
};
use std::collections::{BTreeMap, BTreeSet};
/// Native derive conventions retain positive cfg attrs on members, expand cfg_attr,
/// and remove disabled fields/variants before execution. No member body is resolved.
pub fn derive_input(
    inputs: &DeriveItemInputs,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    let outer = super::attribute_materialize::derive_input(&inputs.item, source, forest)?;
    filter_members(inputs, source, outer)
}
pub fn derive_input_resolved(
    inputs: &DeriveItemInputs,
    indices: &[usize],
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<TokenForest, Diagnostic> {
    let outer =
        super::attribute_materialize::derive_input_resolved(&inputs.item, indices, source, forest)?;
    filter_members(inputs, source, outer)
}
fn filter_members(
    inputs: &DeriveItemInputs,
    source: &SourceIdentity,
    outer: TokenForest,
) -> Result<TokenForest, Diagnostic> {
    let mut members_seen = BTreeSet::new();
    let mut disabled = BTreeMap::new();
    let mut wrappers = BTreeMap::new();
    for member in &inputs.members {
        if !members_seen.insert((member.item.start, member.item.end))
            || member.item == inputs.item.item
        {
            return Err(fault("duplicate or non-member derive range"));
        }
        if member.parse != inputs.item.parse
            || member.cfg != inputs.item.cfg
            || !inputs.item.item.contains(member.item)
        {
            return Err(fault("derive member source/cfg/owner mismatch"));
        }
        if !member.enabled {
            disabled.insert(member.item.start, member.item);
            continue;
        }
        for attr in &member.original_attribute_ranges {
            if !member.item.contains(*attr)
                || wrappers.insert((attr.start, attr.end), member).is_some()
            {
                return Err(fault("invalid or duplicate member attribute wrapper"));
            }
        }
    }
    let spans: BTreeMap<_, _> = outer.span_keys().iter().copied().zip(outer.spans()).collect();
    let mut seen = BTreeSet::new();
    let roots = rewrite(
        outer.roots(),
        source,
        &spans,
        &disabled,
        &wrappers,
        &mut seen,
    )?;
    if seen.len() != wrappers.len() {
        return Err(fault("original member attributes unavailable"));
    }
    if roots.len() != 1 {
        return Err(fault("derive input needs one source root"));
    }
    let rebuilt = outer
        .with_roots(roots)
        .map_err(|e| fault(&e.to_string()))?;
    prune_tree(&rebuilt, &rebuilt.roots()[0])
}
fn rewrite(
    tokens: &[TokenTree],
    source: &SourceIdentity,
    spans: &BTreeMap<Key, &SpanRecord>,
    disabled: &BTreeMap<u32, crate::syntax::spans::SourceRange>,
    wrappers: &BTreeMap<(u32, u32), &crate::syntax::attributes::ItemAttributeInputs>,
    seen: &mut BTreeSet<(u32, u32)>,
) -> Result<Vec<TokenTree>, Diagnostic> {
    let mut result = Vec::new();
    let mut index = 0;
    let mut remove_separator = false;
    while index < tokens.len() {
        let token = &tokens[index];
        let range = token_range(token, source, spans)?;
        if disabled
            .range(..=range.start)
            .next_back()
            .is_some_and(|(_, member)| member.contains(range))
        {
            remove_separator = true;
            index += 1;
            continue;
        }
        if remove_separator {
            remove_separator = false;
            if matches!(token, TokenTree::Punct { scalar: ',', .. }) {
                index += 1;
                continue;
            }
        }
        if matches!(token, TokenTree::Punct { scalar: '#', .. })
            && let Some(TokenTree::Group {
                delimiter: Delimiter::Bracket,
                open,
                close,
                children,
            }) = tokens.get(index + 1)
        {
            let end = token_range(&tokens[index + 1], source, spans)?.end;
            if let Some(member) = wrappers.get(&(range.start, end)) {
                seen.insert((range.start, end));
                for attr in member.attributes.iter().filter(|a| {
                    a.enclosing_attribute_range.start == range.start
                        && a.enclosing_attribute_range.end == end
                }) {
                    result.push(token.clone());
                    result.push(TokenTree::Group {
                        delimiter: Delimiter::Bracket,
                        open: *open,
                        close: *close,
                        children: meta_tokens(children, attr.range, source, spans)?,
                    });
                }
                index += 2;
                continue;
            }
        }
        result.push(match token {
            TokenTree::Group {
                delimiter,
                open,
                close,
                children,
            } => TokenTree::Group {
                delimiter: *delimiter,
                open: *open,
                close: *close,
                children: rewrite(children, source, spans, disabled, wrappers, seen)?,
            },
            _ => token.clone(),
        });
        index += 1;
    }
    Ok(result)
}
