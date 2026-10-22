//! The spelling of a qualified callee's anchor Type (`<Type as Trait>::f`):
//! a module path, or a (mutable) reference to one (`<&i32 as Mul<i32>>::mul`).
use super::ParsedFile;
use crate::core::diagnostic::Diagnostic;
use ra_ap_syntax::ast;

impl ParsedFile {
    /// `None` for any other Type (a generic application, tuple, slice, ...),
    /// which the callee path then refuses as it refused every non-path anchor.
    pub(super) fn anchor_type_spelling(
        &self,
        ty: &ast::Type,
        anchor: &ra_ap_syntax::SyntaxNode,
        cfg: &crate::source::snapshot::CfgSet,
        generic: bool,
    ) -> Result<Option<String>, Diagnostic> {
        let spell = |path: Option<ast::Path>| match path {
            Some(path) => self.anchor_spelling(&path, anchor, cfg, generic),
            None => Ok(None),
        };
        match ty {
            ast::Type::PathType(path) => spell(path.path()),
            ast::Type::RefType(reference) => {
                let Some(ast::Type::PathType(inner)) = reference.ty() else {
                    return Ok(None);
                };
                Ok(spell(inner.path())?.map(|spelled| {
                    let qualifier = if reference.mut_token().is_some() { "mut " } else { "" };
                    format!("&{qualifier}{spelled}")
                }))
            }
            _ => Ok(None),
        }
    }
}
