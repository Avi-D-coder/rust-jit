//! Exact source parameter RefType capture; names carry no type authority.
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
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};
#[derive(Debug, Clone)]
pub(crate) struct ClosedReferenceSyntax {
    parse: ParseKey,
    signature: Digest,
    ordinal: u32,
    range: SourceRange,
    mutable: bool,
    lifetime: Option<String>,
    target: String,
}
impl ClosedReferenceSyntax {
    pub(crate) fn parse(&self) -> &ParseKey {
        &self.parse
    }
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
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
    pub(crate) fn require(&self, signature: &FunctionSignature) -> Result<(), Diagnostic> {
        if self.signature != signature.fingerprint()
            || self.parse.profile != signature.header.profile
            || self.ordinal as usize >= signature.parameters.len()
            || self.range.start >= self.range.end
        {
            return Err(fault("closed reference parameter source identity differs"));
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
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
impl ParsedFile {
    pub(crate) fn closed_reference_parameter(
        &self,
        signature: &FunctionSignature,
        context: &DeclarationContext,
        body: Option<&BodyHandle>,
        ordinal: usize,
    ) -> Result<Option<ClosedReferenceSyntax>, Diagnostic> {
        if ordinal >= 100_000
            || self.function_signature(signature.definition, &signature.header, context, body)?
                != *signature
        {
            return Err(fault("closed reference source signature differs"));
        }
        let Some(parameter) = signature.parameters.get(ordinal) else {
            return Ok(None);
        };
        if parameter.receiver || parameter.variadic || context.scope.owner.is_some() {
            return Ok(None);
        }
        let function = self
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Fn::cast)
            .find(|function| {
                super::ast_adapter::range(function.syntax().text_range()) == context.range
            })
            .ok_or_else(|| fault("closed reference owning function absent"))?;
        let params = function
            .param_list()
            .ok_or_else(|| fault("closed reference function parameters absent"))?;
        if params.self_param().is_some() {
            return Ok(None);
        }
        let Some(ast::Type::RefType(reference)) = params
            .params()
            .nth(ordinal)
            .and_then(|parameter| parameter.ty())
        else {
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
        let proof = ClosedReferenceSyntax {
            parse: self.key(),
            signature: signature.fingerprint(),
            ordinal: u32::try_from(ordinal)
                .map_err(|_| fault("closed reference ordinal overflow"))?,
            range: super::ast_adapter::range(reference.syntax().text_range()),
            mutable: reference.mut_token().is_some(),
            lifetime,
            target,
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
