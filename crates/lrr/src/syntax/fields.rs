//! One selected field occurrence from the ordinary entered source/index.
use super::{
    ast_adapter::range, body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile,
    spans::SourceRange, token_wire::Key,
};
use crate::{core::diagnostic::Diagnostic, ir::pc::NodeId};
use ra_ap_syntax::{AstNode, ast};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}

/// Parser-owned member occurrence. Context comparison is not source, module or
/// privacy authority; the owning source admission service supplies those facts.
pub(crate) struct NamedFieldSite {
    member: String,
    range: SourceRange,
    root_context: Option<Key>,
}
impl NamedFieldSite {
    pub(crate) fn member(&self) -> &str {
        &self.member
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn root_context(&self) -> Option<Key> {
        self.root_context
    }
}
impl ParsedFile {
    pub(crate) fn named_field_site(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        node: NodeId,
    ) -> Result<NamedFieldSite, Diagnostic> {
        body.require_entry()?;
        if index.body() != &body.key() || index.profile() != &self.profile {
            return Err(fault("field source body/index identity differs"));
        }
        let field = self
            .entered_node_syntax(body, index, node)?
            .and_then(ast::FieldExpr::cast)
            .ok_or_else(|| fault("exact named field expression absent"))?;
        let member = field
            .name_ref()
            .ok_or_else(|| fault("named field selector absent"))?;
        if member.text().bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(fault(
                "named field preparation cannot select a tuple ordinal",
            ));
        }
        let selected = range(member.syntax().text_range());
        let text = member.text();
        // Resolve raw identifier validity in its actual token edition. The
        // selected nominal member service still chooses the semantic field.
        crate::frontend::names::owned_name(text, self.token_edition(selected)?)?;
        if text.len() > 64 * 1024 {
            return Err(crate::frontend::types::schema_limit());
        }
        let root_context = self.item_name_context(selected)?;
        if let Some(context) = root_context {
            let facts = self
                .hygiene_facts()
                .ok_or_else(|| fault("field token hygiene facts absent"))?;
            if facts.context(context)?.parent.is_some() {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "field member requires pinned opaque context adjustment".into(),
                ));
            }
        }
        let mut owned = String::new();
        owned
            .try_reserve_exact(text.len())
            .map_err(|_| crate::frontend::types::schema_limit())?;
        owned.push_str(text);
        Ok(NamedFieldSite {
            member: owned,
            range: selected,
            root_context,
        })
    }
    #[cfg(test)]
    pub(crate) fn named_field_member(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        node: NodeId,
    ) -> Result<String, Diagnostic> {
        Ok(self.named_field_site(body, index, node)?.member)
    }
}
