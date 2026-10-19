//! Actual declaration-header Type name locations from the retained parse.
//! This receipt identifies syntax, not a namespace answer or trait proof.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    syntax::{
        outline::{DeclarationContext, HeaderRecipe, ScopeKey},
        parse::{ParseKey, ParsedFile},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{AstNode, ast};

pub(crate) struct DeclarationTypeName {
    parse: ParseKey,
    definition: Digest,
    scope: ScopeKey,
    declaration: SourceRange,
    header: Digest,
    range: SourceRange,
    name: String,
}
fn fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-TYPE-CONTEXT",
        "declaration Type name occurrence differs from its actual source".into(),
    )
}
impl DeclarationTypeName {
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn require(
        &self,
        parse: &ParseKey,
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
        name: &str,
    ) -> Result<(), Diagnostic> {
        if &self.parse != parse
            || self.definition != definition
            || self.scope != context.scope
            || self.declaration != context.range
            || self.header != header.fingerprint()
            || self.name != name
        {
            return Err(fault());
        }
        Ok(())
    }
}
impl ParsedFile {
    pub(crate) fn declaration_type_name(
        &self,
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
        candidate: &ast::NameRef,
    ) -> Result<DeclarationTypeName, Diagnostic> {
        let range = super::ast_adapter::range(candidate.syntax().text_range());
        if header.profile != self.key().profile || !context.range.contains(range) {
            return Err(fault());
        }
        let selected = self
            .syntax_at(range, |node| ast::NameRef::can_cast(node.kind()))?
            .and_then(ast::NameRef::cast)
            .ok_or_else(fault)?;
        if selected.text() != candidate.text()
            || !selected
                .syntax()
                .parent()
                .is_some_and(|node| ast::PathSegment::can_cast(node.kind()))
        {
            return Err(fault());
        }
        let mut type_path = false;
        let mut owner = false;
        for ancestor in selected.syntax().ancestors().skip(1) {
            type_path |= ast::PathType::can_cast(ancestor.kind());
            if ast::Item::can_cast(ancestor.kind()) {
                owner = super::ast_adapter::range(ancestor.text_range()) == context.range;
                break;
            }
        }
        if !type_path || !owner {
            return Err(fault());
        }
        Ok(DeclarationTypeName {
            parse: self.key(),
            definition,
            scope: context.scope.clone(),
            declaration: context.range,
            header: header.fingerprint(),
            range,
            name: selected.text().to_owned(),
        })
    }
}
