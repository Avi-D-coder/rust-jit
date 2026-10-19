//! Declaration-only trait membership; no self Type, arguments or applicability.
use super::impl_head_types::ImplDeclarationContext;
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    traits::header_syntax::ImplHeaderSyntaxKey,
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImplTraitIdentityResult {
    Inherent,
    Trait(Digest),
    Pending,
}
#[derive(Debug)]
pub struct PreparedImplTraitIdentity {
    key: ImplHeaderSyntaxKey,
    syntax: Arc<QueryValue>,
    declaration: Option<ImplDeclarationContext>,
    result: ImplTraitIdentityResult,
}
macro_rules! ready {
    ($value:expr) => { match $value {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    } };
}
impl PreparedImplTraitIdentity {
    pub fn key(&self) -> &ImplHeaderSyntaxKey { &self.key }
    pub fn result(&self) -> ImplTraitIdentityResult { self.result }
    pub(crate) fn syntax(&self) -> &crate::traits::header_syntax::ImplHeaderSyntax {
        let QueryValue::ImplHeaderSyntax(syntax) = self.syntax.as_ref() else { unreachable!("retained impl syntax") };
        syntax
    }
    pub(crate) fn matches_metadata(&self, parse: &crate::syntax::parse::ParseKey,
        recipe: &crate::syntax::outline::HeaderRecipe,
        context: &crate::syntax::outline::DeclarationContext,
    ) -> bool { self.declaration.as_ref().is_some_and(|source| source.matches_metadata(parse, recipe, context)) }
    pub(crate) fn prepare(db: &mut Database, key: &ImplHeaderSyntaxKey) -> QueryPoll<Self> {
        let syntax_owner = ready!(db.fetch(&SyntaxProvider, QueryKey::ImplHeaderSyntax(Box::new(key.clone())))).value;
        let QueryValue::ImplHeaderSyntax(syntax) = syntax_owner.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl membership syntax variant differs")));
        };
        let mut result = Self { key: key.clone(), syntax: Arc::clone(&syntax_owner),
            declaration: None, result: ImplTraitIdentityResult::Pending };
        let Some(owner) = syntax.canonical_owner() else { return QueryPoll::Ready(Ok(result)); };
        let inventory = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.inventory.clone()))).value;
        let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl membership inventory variant differs")));
        };
        let Some(raw) = inventory.header(key.implementation) else {
            return QueryPoll::Ready(Err(fault("impl membership active header absent")));
        };
        let Some(declaration) = ready!(ImplDeclarationContext::observe(db, syntax, raw)) else {
            return QueryPoll::Ready(Ok(result));
        };
        result.declaration = Some(declaration);
        if syntax.trait_type().is_none() {
            result.result = ImplTraitIdentityResult::Inherent;
            return QueryPoll::Ready(Ok(result));
        }
        let Some(environment) = inventory.inventory_root().environment() else { return QueryPoll::Ready(Ok(result)); };
        let declaration = result.declaration.as_ref().expect("observed source metadata");
        let node = match syntax.parsed().syntax_at(declaration.range(), |node| ast::Impl::can_cast(node.kind())) {
            Ok(Some(node)) => node,
            Ok(None) => return QueryPoll::Ready(Err(fault("impl membership actual syntax absent"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let implementation = ast::Impl::cast(node.clone()).expect("checked impl syntax");
        let Some(ast::Type::PathType(ty)) = implementation.trait_() else { return QueryPoll::Ready(Ok(result)); };
        let Some(path) = ty.path() else { return QueryPoll::Ready(Ok(result)); };
        let written = super::source_types::WrittenTypeDeclaration::implementation(
            declaration.owner(owner.identity()), syntax.generics(), &node, None);
        let sources: Vec<_> = environment.sources().iter().collect();
        if let Some(trait_id) = ready!(super::source_types::projection_trait_identity(
            db, environment.names(), &sources, &written, &path)) {
            result.result = ImplTraitIdentityResult::Trait(trait_id);
        }
        QueryPoll::Ready(Ok(result))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-IMPL-HEADER", message.into()) }
impl StableEncode for PreparedImplTraitIdentity {
    const DOMAIN: &'static str = "lrr/v1/prepared-impl-trait-identity";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key); e.record(self.syntax());
        match self.result {
            ImplTraitIdentityResult::Inherent => e.u32(0),
            ImplTraitIdentityResult::Trait(id) => { e.u32(1); e.digest(id); }
            ImplTraitIdentityResult::Pending => e.u32(2),
        }
    }
}
