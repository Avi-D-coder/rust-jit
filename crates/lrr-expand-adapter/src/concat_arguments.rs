//! Pinned expression classification of original concat operands.
use crate::{AdapterError, span_store::SpanStore, token_wire::TokenForest};
use syntax::{AstNode, ast};
pub struct ConcatOperand {
    pub tokens: TokenForest,
    pub nested: Option<crate::nested_string::NestedStringInvocation>,
}
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-COMPILE-INPUT",
        message: message.into(),
    }
}
pub fn parse(input: &TokenForest) -> Result<Vec<ConcatOperand>, AdapterError> {
    if input.allocated_bytes().map_err(|e| fault(&e.to_string()))? > 64 * 1024 * 1024 {
        return Err(fault("concat operand input metadata limit"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let view = tree.token_trees();
    if view.is_empty() {
        return Ok(vec![]);
    }
    let mut parts =
        view.split(|t| matches!(t,tt::TtElement::Leaf(tt::Leaf::Punct(p)) if p.char==','));
    let mut operands = Vec::new();
    let mut metadata = 0usize;
    while let Some(part) = parts.next() {
        if part.is_empty() {
            if operands.is_empty() || parts.next().is_some() {
                return Err(fault("empty concat operand"));
            }
            break;
        }
        if operands.len() >= 100_000 {
            return Err(fault("concat operand count limit"));
        }
        let first = part
            .first_span()
            .ok_or_else(|| fault("concat operand first span absent"))?;
        let last = part
            .last_span()
            .ok_or_else(|| fault("concat operand last span absent"))?;
        let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
            kind: tt::DelimiterKind::Invisible,
            open: first,
            close: last,
        });
        builder.extend_with_tt(part);
        let tokens = builder.build();
        let (parsed, _) = syntax_bridge::token_tree_to_syntax_node(
            &tokens,
            parser::TopEntryPoint::Expr,
            &mut |ctx| ctx.edition(store.database()),
        );
        if !parsed.errors().is_empty() {
            return Err(fault("invalid concat operand expression"));
        }
        let owned = store.export(&tokens)?;
        let nested = if ast::MacroExpr::cast(parsed.syntax_node()).is_some() {
            Some(crate::nested_string::parse(&owned)?)
        } else {
            crate::concat::literal_value(
                &owned,
                store.owned_span_key(first)?,
                64 * 1024 * 1024,
                32 * 1024 * 1024,
            )?;
            None
        };
        let bytes = owned.allocated_bytes().map_err(|e| fault(&e.to_string()))?;
        let nested_bytes = match &nested {
            Some(n) => n
                .arguments
                .allocated_bytes()
                .map_err(|e| fault(&e.to_string()))?
                .checked_add(n.segments.iter().map(|s| s.len()).sum::<usize>())
                .ok_or_else(|| fault("concat nested metadata overflow"))?,
            None => 0,
        };
        metadata = metadata
            .checked_add(bytes)
            .and_then(|n| n.checked_add(nested_bytes))
            .and_then(|n| n.checked_add(std::mem::size_of::<ConcatOperand>()))
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fault("concat operand metadata limit"))?;
        operands.push(ConcatOperand {
            tokens: owned,
            nested,
        });
    }
    Ok(operands)
}

/// Join already selected operand outputs through pinned token storage, preserving marks.
pub fn join(original: &TokenForest, operands: &[TokenForest]) -> Result<TokenForest, AdapterError> {
    use std::collections::BTreeMap;
    let mut bytes = original
        .allocated_bytes()
        .map_err(|e| fault(&e.to_string()))?;
    if operands.len() > 100_000 {
        return Err(fault("concat join operand count limit"));
    }
    for operand in operands {
        bytes = bytes
            .checked_add(
                operand
                    .allocated_bytes()
                    .map_err(|e| fault(&e.to_string()))?,
            )
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| fault("concat join metadata limit"))?;
    }
    if bytes > 64 * 1024 * 1024 {
        return Err(fault("concat join metadata limit"));
    }
    let mut store = SpanStore::default();
    let root = store.import(original)?;
    let delimiter = root.top_subtree().delimiter;
    let mut builder = tt::TopSubtreeBuilder::new(delimiter);
    for (index, operand) in operands.iter().enumerate() {
        if index != 0 {
            builder.push(tt::Leaf::Punct(tt::Punct {
                char: ',',
                spacing: tt::Spacing::Alone,
                span: delimiter.open,
            }));
        }
        let tree = store.import(operand)?;
        builder.extend_with_tt(tree.token_trees());
    }
    let result = store.export(&builder.build())?;
    // Preserve the original invocation site, including registered spans not used by an operand.
    let mut contexts = original
        .contexts()
        .iter()
        .chain(result.contexts())
        .map(|r| (r.key(), r.clone()))
        .collect::<BTreeMap<_, _>>();
    let spans = original
        .spans()
        .iter()
        .chain(result.spans())
        .map(|r| (r.key(), r.clone()))
        .collect::<BTreeMap<_, _>>();
    let output = TokenForest::new(
        result.roots().to_vec(),
        std::mem::take(&mut contexts).into_values().collect(),
        spans.into_values().collect(),
    )
    .map_err(|e| fault(&e.to_string()))?;
    if output
        .allocated_bytes()
        .map_err(|e| fault(&e.to_string()))?
        > 64 * 1024 * 1024
    {
        return Err(fault("concat joined output metadata limit"));
    }
    Ok(output)
}
