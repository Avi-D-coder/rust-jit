//! Owned selected field spelling from the exact captured expression boundary.
use super::{ast_adapter::range, body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile};
use crate::{core::diagnostic::Diagnostic, ir::pc::NodeId};
use ra_ap_syntax::{AstNode, ast};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
impl ParsedFile {
    pub(crate) fn named_field_member(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        node: NodeId,
    ) -> Result<String, Diagnostic> {
        body.require_entry()?;
        if index.body() != &body.key() || index.profile() != &self.profile {
            return Err(fault("field source body/index identity differs"));
        }
        let span = index.span(node)?;
        let start = body
            .range
            .start
            .checked_add(span.relative_start)
            .ok_or_else(|| fault("field start overflow"))?;
        let end = body
            .range
            .start
            .checked_add(span.relative_end)
            .ok_or_else(|| fault("field end overflow"))?;
        let root = self.entered_body_root(body)?;
        let field = root
            .descendants()
            .filter_map(ast::FieldExpr::cast)
            .find(|field| {
                range(field.syntax().text_range()) == (super::spans::SourceRange { start, end })
            })
            .ok_or_else(|| fault("exact named field expression absent"))?;
        let member = field
            .name_ref()
            .ok_or_else(|| fault("named field selector absent"))?;
        if member.text().bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(fault(
                "named field preparation cannot select a tuple ordinal",
            ));
        }
        let text = member.text();
        if text.len() > 32 * 1024 * 1024 {
            return Err(crate::frontend::types::schema_limit());
        }
        let mut owned = String::new();
        owned
            .try_reserve_exact(text.len())
            .map_err(|_| crate::frontend::types::schema_limit())?;
        owned.push_str(text);
        Ok(owned)
    }
}
