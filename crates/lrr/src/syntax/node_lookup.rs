//! Direct source attachment through the existing indexed ranges. The pinned
//! tree's binary child lookup avoids a complete-file walk for each node phase.
use super::{
    body_index::{BodyIndex, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
    spans::SourceRange,
};
use crate::{core::diagnostic::Diagnostic, ir::pc::NodeId};
use ra_ap_syntax::{AstNode, NodeOrToken, SyntaxKind, SyntaxNode, TextRange, ast};
const MAX_DEPTH: usize = 4096;
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-BODY-INDEX", message.into())
}
fn role_matches(node: &SyntaxNode, role: NodeRole) -> bool {
    match role {
        NodeRole::Expression(_) => ast::Expr::can_cast(node.kind()),
        NodeRole::Statement => ast::Stmt::can_cast(node.kind()),
        NodeRole::MatchArm => ast::MatchArm::can_cast(node.kind()),
        NodeRole::Pattern => {
            matches!(
                node.kind(),
                SyntaxKind::LITERAL_PAT
                    | SyntaxKind::WILDCARD_PAT
                    | SyntaxKind::IDENT_PAT
                    | SyntaxKind::RANGE_PAT
            ) && node
                .ancestors()
                .take(MAX_DEPTH)
                .any(|parent| ast::MatchArm::can_cast(parent.kind()))
        }
        NodeRole::OpaqueItem => ast::Item::can_cast(node.kind()),
        _ => false,
    }
}
impl ParsedFile {
    /// The indexed interval chooses a tree path; the role disambiguates nodes
    /// sharing that interval (e.g. a path, a path expression and its name).
    pub(crate) fn entered_node_syntax(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        id: NodeId,
    ) -> Result<Option<SyntaxNode>, Diagnostic> {
        if body.key() != *index.body() {
            return Err(invalid("entered node owning body differs"));
        }
        let node = index
            .nodes()
            .get(id.0 as usize)
            .ok_or_else(|| invalid("entered node ID absent"))?;
        let start = body
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(|| invalid("entered node range overflow"))?;
        let end = body
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(|| invalid("entered node range overflow"))?;
        let range = SourceRange::checked(start, end)?;
        if !body.range.contains(range) {
            return Err(invalid("entered node lies outside its body"));
        }
        self.syntax_at(range, |syntax| role_matches(syntax, node.role))
    }

    pub(crate) fn syntax_at(
        &self,
        source: SourceRange,
        mut selected: impl FnMut(&SyntaxNode) -> bool,
    ) -> Result<Option<SyntaxNode>, Diagnostic> {
        if source.start > source.end {
            return Err(invalid("reversed entered syntax range"));
        }
        let root = self.parsed.tree();
        let target = TextRange::new(source.start.into(), source.end.into());
        if source.is_empty() || !root.syntax().text_range().contains_range(target) {
            return Ok(None);
        }
        let mut node = root.syntax().clone();
        // Return the outermost matching node at the exact indexed range, as
        // ordinary source preorder did, without visiting unrelated siblings.
        for _ in 0..MAX_DEPTH {
            if node.text_range() == target && selected(&node) {
                return Ok(Some(node));
            }
            match node.child_or_token_at_range(target) {
                Some(NodeOrToken::Node(child)) => {
                    node = child;
                }
                Some(NodeOrToken::Token(_)) | None => return Ok(None),
            }
        }
        Err(invalid("entered syntax attachment depth budget"))
    }
}

#[cfg(test)]
#[path = "node_lookup_tests.rs"]
mod tests;
