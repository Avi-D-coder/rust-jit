//! Checked reached floating paths and structural continuations. Upstream AST
//! handles remain here; this adapter performs no queries or guest execution.
use super::{
    body_index::{BodyIndex, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::{NodeId, PcKey, Phase},
};
use ra_ap_syntax::{AstNode, SyntaxNode, TextRange, TextSize, ast, ast::HasGenericArgs};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
#[derive(Debug)]
pub(crate) struct WrittenFloatingPath {
    pub(crate) qualifier: String,
    pub(crate) member: String,
}
fn path_expression(
    parsed: &ParsedFile,
    owner: &BodyHandle,
    start: u32,
    end: u32,
) -> Result<Option<ast::PathExpr>, Diagnostic> {
    if start > end {
        return Err(fault("floating dispatch range is reversed"));
    }
    let root = parsed.entered_body_root(owner)?;
    let range = TextRange::new(TextSize::from(start), TextSize::from(end));
    if !root.text_range().contains_range(range) {
        return Err(fault("floating dispatch range outside entered body"));
    }
    Ok(root
        .covering_element(range)
        .ancestors()
        .find_map(|node| {
            (node.text_range() == range)
                .then(|| ast::PathExpr::cast(node))
                .flatten()
        }))
}
pub(crate) fn written_two_segment_path(
    parsed: &ParsedFile,
    owner: &BodyHandle,
    start: u32,
    end: u32,
) -> Result<Option<WrittenFloatingPath>, Diagnostic> {
    let Some(path) = path_expression(parsed, owner, start, end)?.and_then(|expr| expr.path())
    else {
        return Ok(None);
    };
    let Some(qualifier) = path
        .qualifier()
        .filter(|q| q.qualifier().is_none() && q.coloncolon_token().is_none())
    else {
        return Ok(None);
    };
    let segment_name = |path: &ast::Path| {
        path.segment()
            .filter(|s| {
                s.coloncolon_token().is_none()
                    && s.generic_arg_list().is_none()
                    && s.parenthesized_arg_list().is_none()
                    && s.type_anchor().is_none()
            })
            .and_then(|s| s.name_ref())
    };
    let (Some(member), Some(qualifier)) = (segment_name(&path), segment_name(&qualifier)) else {
        return Ok(None);
    };
    let (member, qualifier) = (member.text(), qualifier.text());
    if member.len() > 64 {
        return Err(Diagnostic::new(
            "LRR-E-METADATA-LIMIT",
            "floating dispatch name budget".into(),
        ));
    }
    Ok(Some(WrittenFloatingPath {
        qualifier: qualifier.into(),
        member: member.into(),
    }))
}
pub(crate) fn qualified_floating_path(
    parsed: &ParsedFile,
    owner: &BodyHandle,
    start: u32,
    end: u32,
) -> Result<Option<WrittenFloatingPath>, Diagnostic> {
    Ok(
        written_two_segment_path(parsed, owner, start, end)?.filter(|path| {
            matches!(
                path.member.as_str(),
                "NAN" | "INFINITY" | "NEG_INFINITY" | "MIN" | "MAX" | "MIN_POSITIVE" | "EPSILON"
            )
        }),
    )
}
pub(crate) fn floating_path_successor(
    parsed: &ParsedFile,
    owner: &BodyHandle,
    index: &BodyIndex,
    start: u32,
    end: u32,
) -> Result<PcKey, Diagnostic> {
    if index.body() != &owner.key() {
        return Err(fault("floating continuation body identity differs"));
    }
    let expression = path_expression(parsed, owner, start, end)?
        .ok_or_else(|| fault("floating recipe exact path expression absent"))?;
    successor(owner, index, expression.syntax())
}
fn indexed_node(
    index: &BodyIndex,
    body: &BodyHandle,
    syntax: &SyntaxNode,
) -> Result<NodeId, Diagnostic> {
    let start = u32::from(syntax.text_range().start())
        .checked_sub(body.range.start)
        .ok_or_else(|| fault("floating continuation precedes selected body"))?;
    let end = u32::from(syntax.text_range().end())
        .checked_sub(body.range.start)
        .ok_or_else(|| fault("floating continuation exceeds selected body"))?;
    index
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == start
                && node.range.end == end
                && if ast::Expr::can_cast(syntax.kind()) {
                    matches!(node.role, NodeRole::Expression(_))
                } else {
                    node.role == NodeRole::Statement
                }
        })
        .map(|node| node.id)
        .ok_or_else(|| fault("floating continuation lacks its structural node"))
}
fn successor(
    body: &BodyHandle,
    index: &BodyIndex,
    syntax: &SyntaxNode,
) -> Result<PcKey, Diagnostic> {
    if u32::from(syntax.text_range().start()) == body.range.start
        && u32::from(syntax.text_range().end()) == body.range.end
        && let Some(next) = index.initializer_return_pc()?
    {
        return Ok(next);
    }
    let parent = syntax
        .parent()
        .ok_or_else(|| fault("floating path structural parent absent"))?;
    if ast::LetStmt::can_cast(parent.kind()) || ast::ExprStmt::can_cast(parent.kind()) {
        return index.pc(indexed_node(index, body, &parent)?, Phase::Commit);
    }
    if let Some(list) = ast::StmtList::cast(parent) {
        let block = list
            .syntax()
            .parent()
            .ok_or_else(|| fault("floating tail block absent"))?;
        return index.pc(indexed_node(index, body, &block)?, Phase::Commit);
    }
    let child = indexed_node(index, body, syntax)?;
    let parent = index
        .nodes()
        .iter()
        .find(|node| node.operands.contains(&child))
        .ok_or_else(|| fault("floating operand continuation absent"))?;
    let ordinal = parent
        .operands
        .iter()
        .position(|node| *node == child)
        .and_then(|ordinal| u16::try_from(ordinal).ok())
        .ok_or_else(|| fault("floating operand continuation overflow"))?;
    index.pc(parent.id, Phase::Operand(ordinal))
}
