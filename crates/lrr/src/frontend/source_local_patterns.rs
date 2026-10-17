//! Exact retained pattern equivalence for the existing source-local builder.
//! Canonical syntax does not resolve a name or grant namespace authority.
use super::error;
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    syntax::{
        body_index::{BodyIndex, NodeRole},
        local_bindings::LocalBindingRecipe,
        outline::BodyHandle,
        spans::SourceRange,
    },
};
use std::collections::BTreeMap;

fn canonical_name(
    binding: &LocalBindingRecipe,
    handle: &BodyHandle,
    index: &BodyIndex,
) -> Result<Option<SourceRange>, Diagnostic> {
    let Some(start) = binding.name_range.start.checked_sub(handle.range.start) else {
        return Ok(None); // Parameters/receivers are outside the entered body.
    };
    let Some(end) = binding.name_range.end.checked_sub(handle.range.start) else {
        return Ok(None);
    };
    if binding.name_range.end > handle.range.end {
        return Ok(None);
    }
    for candidate in index.nodes_at_range(SourceRange { start, end }) {
        let mut node = candidate;
        for _ in 0usize.. {
            if node.role == NodeRole::Pattern {
                if let Some(group) = index.pattern_bindings().group_for(node.id)
                    && group.name() == binding.name
                    && group
                        .occurrence(node.id)
                        .is_some_and(|occurrence| occurrence.name_range == binding.name_range)
                    && group.canonical_candidate().is_some()
                {
                    return Ok(Some(group.canonical_name_range()));
                }
                break;
            }
            if node.role != NodeRole::Syntax {
                break;
            }
            let Some(parent) = node.parent else { break };
            node = index
                .nodes()
                .get(parent.0 as usize)
                .ok_or_else(|| error("local binding pattern parent absent"))?;
        }
    }
    Ok(None)
}

pub(super) fn prepare(
    bindings: &[LocalBindingRecipe],
    handle: &BodyHandle,
    index: &BodyIndex,
) -> Result<BTreeMap<Digest, Digest>, Diagnostic> {
    // Three temporary maps and their tree nodes, bounded before allocating.
    bindings
        .len()
        .checked_mul(768)
        .ok_or_else(|| error("local binding equivalence metadata limit"))?;
    let mut names = BTreeMap::new();
    for binding in bindings {
        if names
            .insert((binding.name_range.start, binding.name_range.end), binding)
            .is_some()
        {
            return Err(error("local binding source name range is ambiguous"));
        }
    }
    let mut equivalence = BTreeMap::new();
    for binding in bindings {
        let Some(range) = canonical_name(binding, handle, index)? else {
            continue;
        };
        let canonical = names
            .get(&(range.start, range.end))
            .ok_or_else(|| error("canonical pattern source binding absent"))?;
        if canonical.name != binding.name
            || canonical.scope != binding.scope
            || canonical.visible_from != binding.visible_from
            || canonical.form != binding.form
        {
            return Err(error("canonical pattern source scope/visibility differs"));
        }
        equivalence.insert(binding.identity, canonical.identity);
    }
    Ok(equivalence)
}
