//! Actual declaration own-bound recipes through the ordinary source Type walk.
//! No body preparation, inherited predicate closure, elaboration or solving.
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider,
        seals::SealKey, value::QueryValue},
    traits::{predicate::SourcePredicateRecipe, term::{ParamKey, TyTerm}},
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;
use super::{declaration_generics::DeclarationGenerics,
    declaration_generic_types::{DeclarationGenericTypes, DeclarationGenericTypesKey, GenericTypesPending},
    source_types::WrittenTypeDeclaration, type_name_namespace::SourceTypeOwner};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct DeclarationSourcePredicatesKey {
    pub inventory: SealKey,
    pub definition: Digest,
}
impl StableEncode for DeclarationSourcePredicatesKey {
    const DOMAIN: &'static str = "lrr/v1/declaration-source-predicates-key";
    fn encode_fields(&self, e: &mut StableEncoder) { e.record(&self.inventory); e.digest(self.definition); }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeclarationSourcePredicatesPending {
    GenericTypes(GenericTypesPending), Source, Environment, AssociatedContext,
    SelfType, UnsupportedDeclaration,
}
#[derive(Debug)]
pub struct PreparedDeclarationSourcePredicates {
    key: DeclarationSourcePredicatesKey,
    generic_types: Arc<QueryValue>,
    environment: Arc<QueryValue>,
    recipes: Vec<SourcePredicateRecipe>,
    pending: Option<DeclarationSourcePredicatesPending>,
}
macro_rules! ready {
    ($value:expr) => { match $value {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    } };
}
impl PreparedDeclarationSourcePredicates {
    pub fn key(&self) -> &DeclarationSourcePredicatesKey { &self.key }
    pub fn generic_types(&self) -> &DeclarationGenericTypes {
        let QueryValue::DeclarationGenericTypes(generics) = self.generic_types.as_ref() else { unreachable!("actual generic Type query") };
        generics
    }
    pub fn declaration(&self) -> &DeclarationGenerics { self.generic_types().declaration() }
    pub fn pending(&self) -> Option<DeclarationSourcePredicatesPending> { self.pending }
    /// Recipe-level pending remains explicit in each source requirement. This
    /// slice never turns an incomplete preparation into an empty environment.
    pub fn recipes(&self) -> Result<&[SourcePredicateRecipe], DeclarationSourcePredicatesPending> {
        if let Some(why) = self.pending { return Err(why); }
        Ok(&self.recipes)
    }
    pub(crate) fn prepare(db: &mut Database, key: &DeclarationSourcePredicatesKey) -> QueryPoll<Self> {
        let generic_types = ready!(db.fetch(&SyntaxProvider,
            QueryKey::DeclarationGenericTypes(Box::new(DeclarationGenericTypesKey {
                inventory: key.inventory.clone(), definition: key.definition,
            })))).value;
        let environment = ready!(db.fetch(&SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone()))).value;
        let mut result = Self { key: key.clone(), generic_types, environment,
            recipes: Vec::new(), pending: None };
        macro_rules! pending { ($why:expr) => {{
            result.pending = Some($why); return QueryPoll::Ready(Ok(result));
        }}; }
        if let Some(why) = result.generic_types().pending() {
            pending!(DeclarationSourcePredicatesPending::GenericTypes(why));
        }
        let declaration = result.declaration();
        let Some(layout) = declaration.layout() else { pending!(DeclarationSourcePredicatesPending::Source); };
        let Some((_, header, context, parsed)) = declaration.source_parts() else { pending!(DeclarationSourcePredicatesPending::Source); };
        let syntax = match parsed.syntax_at(context.range, |node| ast::Item::can_cast(node.kind())) {
            Ok(Some(node)) => node,
            Ok(None) => pending!(DeclarationSourcePredicatesPending::Source),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if !matches!(ast::Item::cast(syntax.clone()), Some(ast::Item::Struct(_) | ast::Item::Enum(_)
            | ast::Item::Union(_) | ast::Item::Trait(_) | ast::Item::Impl(_) | ast::Item::Fn(_)
            | ast::Item::Const(_) | ast::Item::Static(_) | ast::Item::TypeAlias(_))) {
            pending!(DeclarationSourcePredicatesPending::UnsupportedDeclaration);
        }
        // Own source ordinals are represented. An associated declaration's
        // inherited binder/Self context still needs its exact parent rebasing
        // producer, not a manufactured function signature or parent predicate.
        let associated = syntax.parent().and_then(|list| list.parent())
            .is_some_and(|parent| ast::Impl::can_cast(parent.kind()) || ast::Trait::can_cast(parent.kind()));
        if layout.inherited != 0 || layout.implicit_self || associated || !header.context.is_empty() {
            pending!(DeclarationSourcePredicatesPending::AssociatedContext);
        }
        let QueryValue::PreparedScopeImplHeaders(inventory) = result.environment.as_ref() else {
            return QueryPoll::Ready(Err(fault("declaration predicate inventory query differs")));
        };
        if inventory.key() != &key.inventory { return QueryPoll::Ready(Err(fault("declaration predicate inventory differs"))); }
        let Some(environment) = inventory.inventory_root().environment() else { pending!(DeclarationSourcePredicatesPending::Environment); };
        let owner = SourceTypeOwner { definition: key.definition, scope: &context.scope, header };
        let sources = environment.sources().iter().collect::<Vec<_>>();
        let self_type = if declaration.has_trait_self() {
            let ordinal = match u32::try_from(declaration.parameters().len()) {
                Ok(ordinal) => ordinal,
                Err(_) => return QueryPoll::Ready(Err(fault("declaration Self ordinal overflow"))),
            };
            Some(TyTerm::Param(ParamKey { definition: key.definition, ordinal }))
        } else if let Some(implementation) = ast::Impl::cast(syntax.clone()) {
            let Some(ty) = implementation.self_ty() else { pending!(DeclarationSourcePredicatesPending::SelfType); };
            if super::source_types::symbolic_type_pending(&ty).is_some() { pending!(DeclarationSourcePredicatesPending::SelfType); }
            let frame = WrittenTypeDeclaration::declaration(owner, declaration.parameters(), &syntax, None);
            let Some(ty) = ready!(super::source_types::resolve_actual_type(db,
                environment.names(), &sources, &frame, &ty)) else { pending!(DeclarationSourcePredicatesPending::SelfType); };
            Some(ty)
        } else { None };
        let frame = WrittenTypeDeclaration::declaration(owner, declaration.parameters(), &syntax, self_type.as_ref());
        let recipes = ready!(super::source_predicates::prepare_own(db, environment.names(), &sources,
            &frame, &syntax, environment.language_item_core(frame.profile()), layout.own, self_type.as_ref()));
        result.recipes = recipes;
        QueryPoll::Ready(Ok(result))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-DECLARATION-SOURCE-PREDICATES", message.into()) }
impl StableEncode for PreparedDeclarationSourcePredicates {
    const DOMAIN: &'static str = "lrr/v1/actual-declaration-source-predicates";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key); e.record(self.generic_types());
        let QueryValue::PreparedScopeImplHeaders(inventory) = self.environment.as_ref() else { unreachable!("captured environment") };
        e.digest(inventory.fingerprint());
        e.sequence(&self.recipes, |e, recipe| e.record(recipe));
        e.option(&self.pending, |e, why| match why {
            DeclarationSourcePredicatesPending::GenericTypes(why) => { e.u32(0); e.u32(*why as u32); }
            DeclarationSourcePredicatesPending::Source => e.u32(1),
            DeclarationSourcePredicatesPending::Environment => e.u32(2),
            DeclarationSourcePredicatesPending::AssociatedContext => e.u32(3),
            DeclarationSourcePredicatesPending::SelfType => e.u32(4),
            DeclarationSourcePredicatesPending::UnsupportedDeclaration => e.u32(5),
        });
    }
}
