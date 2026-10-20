//! Declared const parameter Types through the ordinary retained Type service.
use crate::{core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, seals::SealKey, value::QueryValue},
    syntax::signatures::GenericKind, traits::term::{TyTerm, ParamKey}};
use ra_ap_syntax::{AstNode, ast};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct DeclarationGenericTypesKey { pub inventory: SealKey, pub definition: Digest }
impl StableEncode for DeclarationGenericTypesKey {
    const DOMAIN: &'static str = "lrr/v1/declaration-generic-types-key";
    fn encode_fields(&self, e: &mut StableEncoder) { e.record(&self.inventory); e.digest(self.definition); }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub enum GenericTypesPending { Parameters = 0, Environment = 1, ConstTypeSyntax = 2, ConstType = 3, SymbolicIdentity = 4 }
#[derive(Debug)]
pub struct DeclarationGenericTypes {
    key: DeclarationGenericTypesKey,
    declaration: Arc<QueryValue>,
    environment: Arc<QueryValue>,
    constants: BTreeMap<u32, TyTerm>,
    pending: Option<GenericTypesPending>,
}
impl DeclarationGenericTypes {
    pub fn key(&self) -> &DeclarationGenericTypesKey { &self.key }
    pub fn declaration(&self) -> &super::declaration_generics::DeclarationGenerics {
        let QueryValue::DeclarationGenerics(declaration) = self.declaration.as_ref() else { unreachable!("actual generic query") }; declaration
    }
    pub(crate) fn declaration_root(&self) -> &Arc<QueryValue> { &self.declaration }
    pub fn const_types(&self) -> &BTreeMap<u32, TyTerm> { &self.constants }
    pub fn pending(&self) -> Option<GenericTypesPending> { self.pending }
    pub(crate) fn prepare(db: &mut Database, key: &DeclarationGenericTypesKey) -> QueryPoll<Self> {
        macro_rules! ready { ($value:expr) => { match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        } }; }
        let declaration = ready!(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(key.definition))).value;
        let environment = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.inventory.clone()))).value;
        let mut result = Self { key: key.clone(), declaration, environment, constants: BTreeMap::new(), pending: None };
        macro_rules! pending { ($why:ident) => {{ result.pending = Some(GenericTypesPending::$why); return QueryPoll::Ready(Ok(result)); }}; }
        let QueryValue::PreparedScopeImplHeaders(inventory) = result.environment.as_ref() else { return QueryPoll::Ready(Err(fault("generic Type environment query differs"))); };
        let Some(environment) = inventory.inventory_root().environment() else { pending!(Environment); };
        let declaration = result.declaration();
        if declaration.has_unproduced_parameter_identities() { pending!(Parameters); }
        let Some(layout) = declaration.layout() else { pending!(Parameters); };
        let Some((source, header, context, parsed)) = declaration.source_parts() else { pending!(Parameters); };
        let syntax = match parsed.syntax_at(context.range, |node| ast::Item::can_cast(node.kind())) {
            Ok(Some(node)) => node, Ok(None) => pending!(Parameters), Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let own_self = (declaration.has_trait_self() || layout.implicit_self).then(|| TyTerm::Param(ParamKey {
            definition: key.definition,
            ordinal: u32::try_from(declaration.parameters().len()).expect("captured source ordinals") - u32::from(layout.implicit_self),
        }));
        // An inherited parameter Type belongs to the parent's header/source,
        // not to the child function's declaration range. Keep that exact owner
        // for name lookup, rebasing only its captured parameter identities.
        let inherited = if declaration.parameters().iter().any(|p| p.kind == GenericKind::Const && p.ordinal >= layout.own) {
            let parent = if let Some(input) = db.input(&crate::db::dependency::InputKey::GeneratedSyntax(source.content)) {
                let crate::db::dependency::InputValue::GeneratedSyntax(unit) = input.as_ref() else { return QueryPoll::Ready(Err(fault("generic parent generated input differs"))); };
                if unit.key != *source { return QueryPoll::Ready(Err(fault("generic parent generated source differs"))); }
                unit.declaration_for_member_scope(&context.scope)
            } else {
                inventory.declaration_for_member_scope(&context.scope).or(context.scope.owner)
            };
            let Some(parent) = parent else { pending!(Parameters); };
            Some(ready!(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(parent))).value)
        } else { None };
        let mut rebase = BTreeMap::new();
        let parent_syntax = if let Some(parent) = &inherited {
            let QueryValue::DeclarationGenerics(parent) = parent.as_ref() else { return QueryPoll::Ready(Err(fault("inherited generic source query differs"))); };
            let Some((_, parent_header, parent_context, parent_parsed)) = parent.source_parts() else { pending!(Parameters); };
            if parent.parameters().len() != layout.inherited as usize || parent.has_trait_self() != layout.implicit_self
                || parent_header.tokens != header.context { pending!(Parameters); }
            for parameter in parent.parameters() {
                let Some(child) = declaration.parameters().get((layout.own + parameter.ordinal) as usize) else { pending!(Parameters); };
                if child.kind != parameter.kind || child.name != parameter.name { pending!(Parameters); }
                rebase.insert(ParamKey { definition: parent.definition(), ordinal: parameter.ordinal }, ParamKey { definition: key.definition, ordinal: child.ordinal });
            }
            if parent.has_trait_self() {
                rebase.insert(ParamKey { definition: parent.definition(), ordinal: layout.inherited },
                    ParamKey { definition: key.definition, ordinal: layout.own + layout.inherited });
            }
            Some(match parent_parsed.syntax_at(parent_context.range, |node| ast::Impl::can_cast(node.kind()) || ast::Trait::can_cast(node.kind())) {
                Ok(Some(node)) => node, Ok(None) => pending!(Parameters), Err(error) => return QueryPoll::Ready(Err(error)),
            })
        } else { None };
        let owner = super::type_name_namespace::SourceTypeOwner { definition: key.definition, scope: &context.scope, header };
        let frame = super::source_types::WrittenTypeDeclaration::declaration(owner, declaration.parameters(), &syntax, own_self.as_ref());
        let sources = environment.sources().iter().collect::<Vec<_>>();
        let mut constants = BTreeMap::new();
        for parameter in declaration.parameters().iter().filter(|parameter| parameter.kind == GenericKind::Const) {
            let (ty, inherited_frame) = if parameter.ordinal >= layout.own {
                let Some(parent) = &inherited else { pending!(Parameters); };
                let QueryValue::DeclarationGenerics(parent) = parent.as_ref() else { unreachable!() };
                let Some((_, header, context, _)) = parent.source_parts() else { pending!(Parameters); };
                let owner = super::type_name_namespace::SourceTypeOwner { definition: parent.definition(), scope: &context.scope, header };
                let frame = super::source_types::WrittenTypeDeclaration::declaration(owner, parent.parameters(), parent_syntax.as_ref().expect("actual parent syntax"), own_self.as_ref())
                    .rebase_parameters(&rebase);
                (parent.const_type(parameter.ordinal - layout.own), Some(frame))
            } else { (declaration.const_type(parameter.ordinal), None) };
            let ty = match ty {
                Ok(Some(ty)) => ty, Ok(None) => pending!(ConstTypeSyntax), Err(error) => return QueryPoll::Ready(Err(error)),
            };
            if super::source_types::symbolic_type_pending(&ty).is_some() { pending!(SymbolicIdentity); }
            let source_frame = inherited_frame.as_ref().unwrap_or(&frame);
            let Some(term) = ready!(super::source_types::resolve_actual_type(db, environment.names(), &sources, source_frame, &ty)) else { pending!(ConstType); };
            constants.insert(parameter.ordinal, term);
        }
        result.constants = constants;
        QueryPoll::Ready(Ok(result))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-DECLARATION-GENERICS", message.into()) }
impl StableEncode for DeclarationGenericTypes {
    const DOMAIN: &'static str = "lrr/v1/actual-declaration-generic-types";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key); e.record(self.declaration());
        e.map(&self.constants, |e, ordinal, ty| { e.u32(*ordinal); e.record(ty); });
        e.option(&self.pending, |e, pending| e.u32(*pending as u32));
    }
}
