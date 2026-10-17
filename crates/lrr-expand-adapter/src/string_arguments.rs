//! Shared pinned string operand parsing; no expansion execution or source reads.
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Key, TokenForest},
};
use syntax::{AstNode, ast};
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum StringOperand {
    Literal { value: String, span: Key },
    NeedExpansion { first_span: Key, last_span: Key },
}
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-COMPILE-INPUT",
        message: message.into(),
    }
}
pub(crate) fn parse_operands(
    input: &TokenForest,
    max_operands: usize,
) -> Result<Vec<StringOperand>, AdapterError> {
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let mut parts = tree
        .token_trees()
        .split(|t| matches!(t,tt::TtElement::Leaf(tt::Leaf::Punct(p)) if p.char==','));
    let mut parsed = Vec::new();
    while let Some(part) = parts.next() {
        if part.is_empty() {
            if parsed.is_empty() || parts.next().is_some() {
                return Err(fault("empty compile-time string operand"));
            }
            break;
        }
        if parsed.len() >= max_operands {
            return Err(fault("excess compile-time string operands"));
        }
        let first = part
            .first_span()
            .ok_or_else(|| fault("missing compile-time string operand span"))?;
        let last = part
            .last_span()
            .ok_or_else(|| fault("missing compile-time string operand span"))?;
        let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
            kind: tt::DelimiterKind::Invisible,
            open: first,
            close: last,
        });
        builder.extend_with_tt(part);
        let tokens = builder.build();
        let (syntax, map) = syntax_bridge::token_tree_to_syntax_node(
            &tokens,
            parser::TopEntryPoint::Expr,
            &mut |ctx| ctx.edition(store.database()),
        );
        if !syntax.errors().is_empty() {
            return Err(fault("invalid compile-time string operand syntax"));
        }
        let node = syntax.syntax_node();
        let operand = if let Some(literal) = ast::Literal::cast(node.clone()) {
            let ast::LiteralKind::String(string) = literal.kind() else {
                return Err(fault(
                    "compile-time string operand requires a string literal",
                ));
            };
            let token = literal.token();
            let span = map.span_at(token.text_range().start());
            if tt::token_to_literal(token.text(), span).suffix_len != 0 {
                return Err(fault(
                    "compile-time string string literal suffix is invalid",
                ));
            }
            let value = string
                .value()
                .map_err(|_| fault("invalid compile-time string string escape"))?;
            StringOperand::Literal {
                value: value.into_owned(),
                span: store.owned_span_key(span)?,
            }
        } else if ast::MacroExpr::cast(node).is_some() {
            StringOperand::NeedExpansion {
                first_span: store.owned_span_key(first)?,
                last_span: store.owned_span_key(last)?,
            }
        } else {
            return Err(fault(
                "compile-time string operand requires a string or nested macro expansion",
            ));
        };
        parsed.push(operand);
    }
    if parsed.is_empty() {
        return Err(fault("compile-time string operand is missing"));
    }
    Ok(parsed)
}
/// Exactly one string literal or one suspended nested macro operand.
pub fn parse_single(input: &TokenForest) -> Result<StringOperand, AdapterError> {
    parse_operands(input, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| fault("compile-time string operand is missing"))
}

/// Original owned operand slices for eager string-only macro arguments.
pub struct StringArgumentTokens {
    pub tokens: TokenForest,
    pub nested: Option<crate::nested_string::NestedStringInvocation>,
}
pub fn owned_operands(
    input: &TokenForest,
    max_operands: usize,
) -> Result<Vec<StringArgumentTokens>, AdapterError> {
    let parsed = parse_operands(input, max_operands)?;
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let mut parts = tree
        .token_trees()
        .split(|t| matches!(t,tt::TtElement::Leaf(tt::Leaf::Punct(p)) if p.char==','));
    let mut result = Vec::with_capacity(parsed.len());
    let mut bytes = 0usize;
    for operand in parsed {
        let part = parts
            .next()
            .ok_or_else(|| fault("string operand slice is absent"))?;
        let first = part
            .first_span()
            .ok_or_else(|| fault("string operand first span absent"))?;
        let last = part
            .last_span()
            .ok_or_else(|| fault("string operand last span absent"))?;
        let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
            kind: tt::DelimiterKind::Invisible,
            open: first,
            close: last,
        });
        builder.extend_with_tt(part);
        let tokens = store.export(&builder.build())?;
        let nested = match operand {
            StringOperand::Literal { .. } => None,
            StringOperand::NeedExpansion { .. } => Some(crate::nested_string::parse(&tokens)?),
        };
        let extra = match &nested {
            Some(n) => n
                .arguments
                .allocated_bytes()
                .map_err(|e| fault(&e.to_string()))?
                .checked_add(n.segments.iter().map(|s| s.len()).sum::<usize>())
                .ok_or_else(|| fault("string operand metadata overflow"))?,
            None => 0,
        };
        bytes = bytes
            .checked_add(
                tokens
                    .allocated_bytes()
                    .map_err(|e| fault(&e.to_string()))?,
            )
            .and_then(|n| n.checked_add(extra))
            .and_then(|n| n.checked_add(std::mem::size_of::<StringArgumentTokens>()))
            .ok_or_else(|| fault("string operand aggregate metadata limit"))?;
        result.push(StringArgumentTokens { tokens, nested });
    }
    Ok(result)
}
