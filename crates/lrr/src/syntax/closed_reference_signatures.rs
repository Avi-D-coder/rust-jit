//! Exact source RefType capture; names carry no type authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    syntax::{
        outline::{BodyHandle, DeclarationContext},
        parse::{ParseKey, ParsedFile},
        signatures::FunctionSignature,
        spans::SourceRange,
    },
};
#[path = "reference_header_plan.rs"]
mod plan;
pub use plan::ReferenceHeaderPlan;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ReferenceRole {
    Parameter,
    Result,
}

/// Ordinary parser-owned reference type. The role cannot be supplied by callers.
#[derive(Debug, Clone)]
pub(crate) struct ClosedReferenceTypeSyntax {
    parse: ParseKey,
    signature: Digest,
    role: ReferenceRole,
    range: SourceRange,
    mutable: bool,
    lifetime: Option<String>,
    target: String,
    target_range: SourceRange,
}
impl ClosedReferenceTypeSyntax {
    pub(crate) fn parse(&self) -> &ParseKey {
        &self.parse
    }
    pub(crate) fn mutable(&self) -> bool {
        self.mutable
    }
    pub(crate) fn lifetime(&self) -> Option<&str> {
        self.lifetime.as_deref()
    }
    pub(crate) fn target(&self) -> &str {
        &self.target
    }
    pub(crate) fn target_range(&self) -> SourceRange {
        self.target_range
    }
    pub(crate) fn require(&self, signature: &FunctionSignature) -> Result<(), Diagnostic> {
        if self.signature != signature.fingerprint()
            || self.parse.profile != signature.header.profile
            || self.range.is_empty()
            || self.target_range.is_empty()
            || !self.range.contains(self.target_range)
        {
            return Err(fault("closed reference type source identity differs"));
        }
        Ok(())
    }
    pub(crate) fn require_result(&self, signature: &FunctionSignature) -> Result<(), Diagnostic> {
        self.require(signature)?;
        if self.role != ReferenceRole::Result || signature.result.is_empty() {
            return Err(fault("closed reference source is not the selected result"));
        }
        Ok(())
    }
    pub(crate) fn allocated_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.parse.profile.semantics.capacity())?
            .checked_add(self.target.capacity())?
            .checked_add(self.lifetime.as_ref().map_or(0, String::capacity))
    }
}
#[derive(Debug, Clone)]
pub(crate) struct ClosedReferenceSyntax {
    reference: ClosedReferenceTypeSyntax,
    ordinal: u32,
}
impl ClosedReferenceSyntax {
    pub(crate) fn reference(&self) -> &ClosedReferenceTypeSyntax {
        &self.reference
    }
    pub(crate) fn parse(&self) -> &ParseKey {
        self.reference.parse()
    }
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn mutable(&self) -> bool {
        self.reference.mutable()
    }
    pub(crate) fn lifetime(&self) -> Option<&str> {
        self.reference.lifetime()
    }
    pub(crate) fn target(&self) -> &str {
        self.reference.target()
    }
    pub(crate) fn require(&self, signature: &FunctionSignature) -> Result<(), Diagnostic> {
        self.reference.require(signature)?;
        if self.reference.role != ReferenceRole::Parameter
            || self.ordinal as usize >= signature.parameters.len()
        {
            return Err(fault("closed reference parameter source identity differs"));
        }
        Ok(())
    }
    pub(crate) fn allocated_bytes(&self) -> Option<usize> {
        self.reference
            .allocated_bytes()?
            .checked_sub(std::mem::size_of::<ClosedReferenceTypeSyntax>())?
            .checked_add(std::mem::size_of::<Self>())
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
impl ParsedFile {
    fn closed_reference_function(
        &self,
        signature: &FunctionSignature,
        context: &DeclarationContext,
        body: Option<&BodyHandle>,
    ) -> Result<ast::Fn, Diagnostic> {
        if self.function_signature(signature.definition, &signature.header, context, body)?
            != *signature
        {
            return Err(fault("closed reference source signature differs"));
        }
        self.syntax_at(context.range, |node| ast::Fn::can_cast(node.kind()))?
            .and_then(ast::Fn::cast)
            .ok_or_else(|| fault("closed reference owning function absent"))
    }
    fn closed_reference_type(
        &self,
        ty: Option<ast::Type>,
        signature: Digest,
        role: ReferenceRole,
    ) -> Result<Option<ClosedReferenceTypeSyntax>, Diagnostic> {
        let Some(ast::Type::RefType(reference)) = ty else {
            return Ok(None);
        };
        let Some(ast::Type::PathType(target)) = reference.ty() else {
            return Ok(None);
        };
        let Some(path) = target.path().filter(|path| path.qualifier().is_none()) else {
            return Ok(None);
        };
        let Some(segment) = path.segment() else {
            return Ok(None);
        };
        if segment.coloncolon_token().is_some()
            || segment.generic_arg_list().is_some()
            || segment.parenthesized_arg_list().is_some()
            || segment.type_anchor().is_some()
        {
            return Ok(None);
        }
        let Some(name) = segment.name_ref() else {
            return Ok(None);
        };
        let target = crate::expand::scopes::MacroName::parse(name.text(), self.edition)?
            .as_str()
            .to_owned();
        let lifetime = reference
            .lifetime()
            .map(|lifetime| {
                super::signatures::canonical_lifetime(&lifetime.syntax().text().to_string())
                    .ok_or_else(|| fault("closed reference lifetime syntax absent"))
            })
            .transpose()?;
        let proof = ClosedReferenceTypeSyntax {
            parse: self.key(),
            signature,
            role,
            range: super::ast_adapter::range(reference.syntax().text_range()),
            mutable: reference.mut_token().is_some(),
            lifetime,
            target,
            target_range: super::ast_adapter::range(name.syntax().text_range()),
        };
        if proof.allocated_bytes().is_none_or(|n| n > 128 * 1024) {
            return Err(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "closed reference syntax budget".into(),
            ));
        }
        Ok(Some(proof))
    }
}
