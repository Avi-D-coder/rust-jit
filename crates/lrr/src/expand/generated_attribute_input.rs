//! Effective generated attribute input assembled from the pinned syntax map.
use crate::{core::diagnostic::Diagnostic, syntax::token_wire::TokenForest};
pub(crate) struct GeneratedAttributeInput {
    pub item: TokenForest,
    pub arguments: Option<TokenForest>,
    pub path: TokenForest,
    pub site: crate::syntax::token_wire::Key,
    pub syntax: crate::syntax::macro_calls::SourceAttributePath,
    pub consumed_tests: std::collections::BTreeSet<usize>,
}
pub(crate) fn prepare_remaining(
    items: &super::generated_items::GeneratedItems,
    index: usize,
    cfg: &crate::source::snapshot::CfgSet,
    skipped: &std::collections::BTreeSet<usize>,
    inert: &std::collections::BTreeSet<usize>,
    compiler_tests: &std::collections::BTreeSet<usize>,
) -> Result<Option<GeneratedAttributeInput>, Diagnostic> {
    let fault = |message: &str| Diagnostic::new("LRR-E-MACRO-OUTPUT", message.into());
    let header = items
        .items()
        .get(index)
        .ok_or_else(|| fault("generated attribute header absent"))?;
    let parsed = crate::syntax::parse::ParsedFile::from_generated_items(items)?;
    let facts = parsed.attribute_inputs_at(
        crate::syntax::spans::SourceRange {
            start: header.start,
            end: header.end,
        },
        cfg,
    )?;
    if !facts.enabled {
        return Err(fault("generated attribute item became disabled"));
    }
    if !compiler_tests.is_subset(inert)
        || skipped
            .iter()
            .chain(inert)
            .any(|at| *at >= facts.attributes.len())
    {
        return Err(fault("generated consumed attribute index differs"));
    }
    let mut selected_source = None;
    for (at, attribute) in facts.attributes.iter().enumerate() {
        if skipped.contains(&at)
            || inert.contains(&at)
            || attribute.name == "cfg"
            || crate::expand::builtin_attributes::inert_descriptor(&attribute.name)?.is_some()
        {
            continue;
        }
        if attribute.name != "test"
            && crate::expand::builtin_attributes::descriptor(&attribute.name)?.is_some()
        {
            return Ok(None);
        }
        selected_source = Some(at);
        break;
    }
    let Some(selected_source) = selected_source else {
        return Ok(None);
    };
    let consumed_tests: std::collections::BTreeSet<_> =
        compiler_tests.range(..selected_source).copied().collect();
    let mut effective = Vec::new();
    effective
        .try_reserve_exact(facts.attributes.len())
        .map_err(|_| fault("generated effective attribute range allocation"))?;
    let mut selected = None;
    for (at, attribute) in facts.attributes.iter().enumerate() {
        if skipped.contains(&at) || consumed_tests.contains(&at) {
            continue;
        }
        if attribute.name == "cfg" {
            continue;
        }
        let index = effective.len();
        effective.push(lrr_expand_adapter::generated_attribute::AttributeRange {
            start: attribute.range.start,
            end: attribute.range.end,
            wrapper_start: attribute.enclosing_attribute_range.start,
            wrapper_end: attribute.enclosing_attribute_range.end,
        });
        if at == selected_source {
            selected = Some((index, parsed.attribute_path_at(attribute.range)?));
        }
    }
    let Some((index, syntax)) = selected else {
        return Ok(None);
    };
    let adapter = lrr_expand_adapter::token_wire::TokenForest::decode(items.record().wire())
        .map_err(|e| fault(&e.message))?;
    let input = lrr_expand_adapter::generated_attribute::materialize(
        &adapter,
        header.start,
        header.end,
        index,
        &effective,
    )
    .map_err(|e| Diagnostic::new(e.code, e.message))?;
    let decode = |tokens: &lrr_expand_adapter::token_wire::TokenForest| {
        TokenForest::decode(&tokens.encode().map_err(|e| fault(&e.message))?)
            .map_err(|e| fault(&e.message))
    };
    Ok(Some(GeneratedAttributeInput {
        item: decode(&input.item)?,
        arguments: input.arguments.as_ref().map(decode).transpose()?,
        path: decode(&input.path)?,
        site: crate::syntax::token_wire::Key(input.site.0),
        syntax,
        consumed_tests,
    }))
}
