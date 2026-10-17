//! Ordered derive path facts from original owned tokens and the pinned type parser.
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Key, TokenForest},
};
use syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivePath {
    pub absolute: bool,
    pub segments: Vec<String>,
    pub first_span: Key,
    pub last_span: Key,
    /// Original token occurrences, including contexts and token editions.
    pub tokens: TokenForest,
}
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
pub fn parse(input: &TokenForest) -> Result<Vec<DerivePath>, AdapterError> {
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    if tree.top_subtree().delimiter.kind != tt::DelimiterKind::Parenthesis {
        return Err(fault("derive arguments require parentheses"));
    }
    let parts: Vec<_> = tree
        .token_trees()
        .split(|t| matches!(t,tt::TtElement::Leaf(tt::Leaf::Punct(p)) if p.char==','))
        .collect();
    let mut result = Vec::new();
    let mut bytes = 0usize;
    for (index, part) in parts.iter().enumerate() {
        let Some(first) = part.first_span() else {
            if index + 1 == parts.len() {
                continue;
            }
            return Err(fault("empty derive path"));
        };
        let last = part
            .last_span()
            .ok_or_else(|| fault("missing derive path span"))?;
        let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
            kind: tt::DelimiterKind::Invisible,
            open: first,
            close: last,
        });
        builder.extend_with_tt(*part);
        let path_tokens = builder.build();
        let (parsed, _) = syntax_bridge::token_tree_to_syntax_node(
            &path_tokens,
            parser::TopEntryPoint::Type,
            &mut |ctx| ctx.edition(store.database()),
        );
        if !parsed.errors().is_empty() {
            return Err(fault("invalid derive path syntax"));
        }
        let ty = ast::PathType::cast(parsed.syntax_node())
            .ok_or_else(|| fault("derive requires a path, not another type"))?;
        let mut path = ty.path().ok_or_else(|| fault("missing derive path"))?;
        let mut segments = Vec::new();
        let absolute;
        loop {
            let segment = path
                .segment()
                .ok_or_else(|| fault("missing derive path segment"))?;
            if segment.generic_arg_list().is_some()
                || segment.type_anchor().is_some()
                || segment.parenthesized_arg_list().is_some()
            {
                return Err(fault("derive paths cannot contain type arguments/anchors"));
            }
            let name = segment
                .name_ref()
                .ok_or_else(|| fault("missing derive path name"))?
                .text()
                .to_string();
            bytes = bytes
                .checked_add(name.len() + std::mem::size_of::<String>())
                .ok_or_else(|| fault("derive path metadata limit"))?;
            segments.push(name);
            match path.qualifier() {
                Some(parent) => path = parent,
                None => {
                    absolute =
                        path.coloncolon_token().is_some() || segment.coloncolon_token().is_some();
                    break;
                }
            }
        }
        segments.reverse();
        let tokens = store.export(&path_tokens)?;
        bytes = bytes
            .checked_add(tokens.allocated_bytes()?)
            .ok_or_else(|| fault("derive path token retention limit"))?;
        result.push(DerivePath {
            absolute,
            segments,
            first_span: store.owned_span_key(first)?,
            last_span: store.owned_span_key(last)?,
            tokens,
        });
    }
    Ok(result)
}
