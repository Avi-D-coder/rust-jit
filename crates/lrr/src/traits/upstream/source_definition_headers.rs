//! Retained actual declaration headers, before full semantic fact production.
//! Source syntax facts are not compiler-computed Types, flags or solver facts.
use super::{
    identity::DefinitionId,
    model::{Generics, Span, Symbol},
    prepared::{self, Cx, DefinitionReceipt},
    source_bindings::{DefinitionKind, SourceBindings},
    source_generics::SourceGenericLayouts,
    source_terms::SourceTypePending,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{dependency::{InputKey, InputValue}, key::QueryKey,
        memo::{Database, PreparedInputGuard}, poll::QueryPoll,
        provider::SyntaxProvider, seals::SealKey, value::QueryValue},
    frontend::{declaration_generics::DeclarationGenerics,
        impl_trait_identity::ImplTraitIdentityResult, trait_declarations::TraitHeader},
    syntax::{ast_adapter::range, outline::{AttributeRecipe, HeaderRecipe, ScopeKey},
        signatures::GenericSourceLayout, spans::SourceRange},
    traits::header_syntax::ImplHeaderSyntaxKey,
};
use ra_ap_syntax::{AstNode, ast::{self, HasName}};
use rustc_type_ir::Interner;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct DefinitionHeaderKey {
    pub inventory: SealKey,
    pub definition: Digest,
}
impl StableEncode for DefinitionHeaderKey {
    const DOMAIN: &'static str = "lrr/v1/source-definition-header-key";
    fn encode_fields(&self, e: &mut StableEncoder) { e.record(&self.inventory); e.digest(self.definition); }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum DefinitionHeaderRole {
    Struct = 0, Enum = 1, Union = 2, Trait = 3, Impl = 4, Function = 5,
    Constant = 6, Static = 7, TypeAlias = 8, Other = 9,
}
/// Source ownership, deliberately distinct from a compiler-computed DefId parent.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SourceHeaderParent {
    Associated(Digest),
    Lexical(ScopeKey),
    Foreign(ScopeKey),
    Pending,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DefinitionHeaderPending {
    ParentJoin,
    ImplMembership,
    Source(SourceTypePending),
    UnsupportedRole,
}
/// Missing semantic producers are explicit holds, never successful absence.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum DefinitionSemanticHold {
    Variance, AliasVariance, DeclaredType, TypedSignature, PredicateClosure,
    OpaqueHiddenType, OpaqueDefinitionIdentity, ConstValue, Coroutine,
    SizedSelf, CompilerConstness, ConstConditions, TargetFeatureSemantics,
    ImplTraitInTraitIdentity, HasDefinitionSemantics, AliasArgumentLayouts,
}
const SEMANTIC_HOLDS: &[DefinitionSemanticHold] = &[
    DefinitionSemanticHold::Variance, DefinitionSemanticHold::AliasVariance,
    DefinitionSemanticHold::DeclaredType, DefinitionSemanticHold::TypedSignature,
    DefinitionSemanticHold::PredicateClosure, DefinitionSemanticHold::OpaqueHiddenType,
    DefinitionSemanticHold::OpaqueDefinitionIdentity, DefinitionSemanticHold::ConstValue,
    DefinitionSemanticHold::Coroutine, DefinitionSemanticHold::SizedSelf,
    DefinitionSemanticHold::CompilerConstness, DefinitionSemanticHold::ConstConditions,
    DefinitionSemanticHold::TargetFeatureSemantics, DefinitionSemanticHold::ImplTraitInTraitIdentity,
    DefinitionSemanticHold::HasDefinitionSemantics, DefinitionSemanticHold::AliasArgumentLayouts,
];
/// Exact source header locations and keywords, not a typed fn signature.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SourceFunctionHeader {
    pub parameters: SourceRange,
    pub result: Option<SourceRange>,
    pub abi: Option<SourceRange>,
    pub body: Option<SourceRange>,
    pub constant_keyword: bool,
    pub asynchronous_keyword: bool,
    pub unsafe_keyword: bool,
    pub default_keyword: bool,
}
#[derive(Debug)]
pub struct PreparedDefinitionHeader {
    key: DefinitionHeaderKey,
    receipt: DefinitionReceipt,
    declaration: Arc<QueryValue>,
    inventory: Arc<QueryValue>,
    trait_declaration: Option<Arc<QueryValue>>,
    implementation: Option<Arc<QueryValue>>,
    associated_parent: Option<Arc<QueryValue>>,
    bindings: SourceBindings,
    role: DefinitionHeaderRole,
    name: Option<String>,
    span: Span,
    edition: u16,
    scope: ScopeKey,
    parent: SourceHeaderParent,
    function: Option<SourceFunctionHeader>,
    declared_type: Option<SourceRange>,
    initializer_equals: Option<bool>,
    pending: Option<DefinitionHeaderPending>,
}
macro_rules! ready {
    ($value:expr) => { match $value {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    } };
}
impl PreparedDefinitionHeader {
    pub fn key(&self) -> &DefinitionHeaderKey { &self.key }
    pub fn pending(&self) -> Option<DefinitionHeaderPending> { self.pending }
    pub fn role(&self) -> DefinitionHeaderRole { self.role }
    pub fn name(&self) -> Option<&str> { self.name.as_deref() }
    pub(crate) fn span(&self) -> Span { self.span }
    pub fn edition(&self) -> u16 { self.edition }
    pub fn scope(&self) -> &ScopeKey { &self.scope }
    pub fn source_parent(&self) -> &SourceHeaderParent { &self.parent }
    pub fn source_function(&self) -> Option<SourceFunctionHeader> { self.function }
    pub fn declared_type_range(&self) -> Option<SourceRange> { self.declared_type }
    pub fn initializer_equals(&self) -> Option<bool> { self.initializer_equals }
    pub fn generic_owner(&self) -> Digest { self.declaration().definition() }
    pub fn source_generic_layout(&self) -> Option<GenericSourceLayout> { self.declaration().layout() }
    pub fn semantic_holds(&self) -> &[DefinitionSemanticHold] { SEMANTIC_HOLDS }
    pub(crate) fn declaration(&self) -> &DeclarationGenerics {
        let QueryValue::DeclarationGenerics(declaration) = self.declaration.as_ref() else { unreachable!("captured declaration generics") };
        declaration
    }
    pub(crate) fn header(&self) -> &HeaderRecipe {
        self.declaration().source_parts().expect("captured actual source header").1
    }
    pub(crate) fn attributes(&self) -> &[AttributeRecipe] { &self.header().attributes }
    pub(crate) fn opaque_returns(&self) -> Option<&crate::frontend::declaration_generics::OpaqueReturnHeaders> {
        self.declaration().opaque_returns()
    }
    pub(crate) fn trait_header(&self) -> Option<&TraitHeader> {
        match self.trait_declaration.as_deref() {
            Some(QueryValue::TraitDeclaration(declaration)) => declaration.header(),
            None => None,
            _ => unreachable!("captured trait source query"),
        }
    }
    pub(crate) fn impl_membership(&self) -> Option<ImplTraitIdentityResult> {
        match self.implementation.as_deref() {
            Some(QueryValue::ImplTraitIdentity(identity)) => Some(identity.result()),
            None => None,
            _ => unreachable!("captured actual impl membership"),
        }
    }
    pub(super) fn receipt(&self) -> &DefinitionReceipt { &self.receipt }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        if self.bindings.pending.is_some() { return Err(fault("definition header generic source closure remains pending")); }
        SourceGenericLayouts::from_bindings(Arc::clone(&self.bindings.catalog), [&self.bindings])
    }
    pub(crate) fn prepare(db: &mut Database, key: &DefinitionHeaderKey) -> QueryPoll<Self> {
        let declaration = ready!(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(key.definition))).value;
        let inventory = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.inventory.clone()))).value;
        let Some(receipt) = ready!(DefinitionReceipt::observe(db, key.definition)) else {
            return QueryPoll::Ready(Err(fault("definition header actual source receipt absent")));
        };
        let QueryValue::DeclarationGenerics(generics) = declaration.as_ref() else {
            return QueryPoll::Ready(Err(fault("definition header generic query differs")));
        };
        if !receipt.matches_generics(generics) { return QueryPoll::Ready(Err(fault("definition header generic receipt differs"))); }
        let Some((parse, header, context, parsed)) = generics.source_parts() else {
            return QueryPoll::Ready(Err(fault("definition header retained source parts absent")));
        };
        let item = match parsed.syntax_at(context.range, |node| ast::Item::can_cast(node.kind())) {
            Ok(Some(node)) => ast::Item::cast(node).expect("selected item syntax"),
            Ok(None) => return QueryPoll::Ready(Err(fault("definition header actual item absent"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (role, name) = match &item {
            ast::Item::Struct(item) => (DefinitionHeaderRole::Struct, item.name()),
            ast::Item::Enum(item) => (DefinitionHeaderRole::Enum, item.name()),
            ast::Item::Union(item) => (DefinitionHeaderRole::Union, item.name()),
            ast::Item::Trait(item) => (DefinitionHeaderRole::Trait, item.name()),
            ast::Item::Impl(_) => (DefinitionHeaderRole::Impl, None),
            ast::Item::Fn(item) => (DefinitionHeaderRole::Function, item.name()),
            ast::Item::Const(item) => (DefinitionHeaderRole::Constant, item.name()),
            ast::Item::Static(item) => (DefinitionHeaderRole::Static, item.name()),
            ast::Item::TypeAlias(item) => (DefinitionHeaderRole::TypeAlias, item.name()),
            _ => (DefinitionHeaderRole::Other, None),
        };
        let edition_range = name.as_ref().map(|name| range(name.syntax().text_range()))
            .or_else(|| item.syntax().first_token().map(|token| range(token.text_range())))
            .ok_or_else(|| fault("definition header has no source token"));
        let edition = match edition_range.and_then(|range| parsed.token_edition(range)) {
            Ok(edition) => edition, Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let function = if let ast::Item::Fn(_) = &item {
            let function = match parsed.function_header(key.definition, header, context, None) {
                Ok(function) => function, Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let Some(parameters) = function.param_list() else { return QueryPoll::Ready(Err(fault("actual function header parameter list absent"))); };
            Some(SourceFunctionHeader {
                parameters: range(parameters.syntax().text_range()),
                result: function.ret_type().and_then(|result| result.ty()).map(|ty| range(ty.syntax().text_range())),
                abi: function.abi().map(|abi| range(abi.syntax().text_range())),
                body: function.syntax().children().find_map(ast::BlockExpr::cast).map(|body| range(body.syntax().text_range())),
                constant_keyword: function.const_token().is_some(), asynchronous_keyword: function.async_token().is_some(),
                unsafe_keyword: function.unsafe_token().is_some(), default_keyword: function.default_token().is_some(),
            })
        } else { None };
        let (declared_type, initializer_equals) = match &item {
            ast::Item::Const(item) => (item.ty().map(|ty| range(ty.syntax().text_range())), Some(item.eq_token().is_some())),
            ast::Item::Static(item) => (item.ty().map(|ty| range(ty.syntax().text_range())), Some(item.eq_token().is_some())),
            ast::Item::TypeAlias(item) => (item.ty().map(|ty| range(ty.syntax().text_range())), Some(item.eq_token().is_some())),
            _ => (None, None),
        };
        let QueryValue::PreparedScopeImplHeaders(headers) = inventory.as_ref() else { return QueryPoll::Ready(Err(fault("definition header inventory query differs"))); };
        if headers.key() != &key.inventory { return QueryPoll::Ready(Err(fault("definition header inventory differs"))); }
        let Some(catalog_input) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else { return QueryPoll::Ready(Err(fault("definition header source catalog absent"))); };
        let InputValue::DiscoverySeal(catalog) = catalog_input.as_ref() else { return QueryPoll::Ready(Err(fault("definition header source catalog differs"))); };
        if let Err(error) = prepared::catalog_world(catalog) { return QueryPoll::Ready(Err(error)); }
        let mut pending = (role == DefinitionHeaderRole::Other).then_some(DefinitionHeaderPending::UnsupportedRole);
        let parent_owner = if let Some(input) = db.input(&InputKey::GeneratedSyntax(parse.content)) {
            let InputValue::GeneratedSyntax(unit) = input.as_ref() else { return QueryPoll::Ready(Err(fault("definition header generated source variant differs"))); };
            if unit.key != *parse { return QueryPoll::Ready(Err(fault("definition header generated parser identity differs"))); }
            unit.declaration_for_member_scope(&context.scope)
        } else { headers.declaration_for_member_scope(&context.scope) };
        let syntax_parent = item.syntax().ancestors().skip(1).find_map(ast::Item::cast);
        let associated_syntax = matches!(syntax_parent, Some(ast::Item::Trait(_) | ast::Item::Impl(_)));
        let parent = match parent_owner {
            Some(owner) => SourceHeaderParent::Associated(owner),
            None if associated_syntax => { pending = Some(DefinitionHeaderPending::ParentJoin); SourceHeaderParent::Pending },
            None if matches!(syntax_parent, Some(ast::Item::ExternBlock(_))) => SourceHeaderParent::Foreign(context.scope.clone()),
            None => SourceHeaderParent::Lexical(context.scope.clone()),
        };
        let associated_parent = if let SourceHeaderParent::Associated(parent) = parent {
            let actual = ready!(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(parent))).value;
            let QueryValue::DeclarationGenerics(parent) = actual.as_ref() else { return QueryPoll::Ready(Err(fault("associated parent source query differs"))); };
            if parent.source_parts().is_none() { pending = Some(DefinitionHeaderPending::ParentJoin); }
            Some(actual)
        } else { None };
        let trait_declaration = if role == DefinitionHeaderRole::Trait {
            let actual = ready!(db.fetch(&SyntaxProvider, QueryKey::TraitDeclaration(key.definition))).value;
            let QueryValue::TraitDeclaration(trait_) = actual.as_ref() else { return QueryPoll::Ready(Err(fault("definition trait source query differs"))); };
            if !trait_.matches_owner(key.definition, parse, header, &context.scope) { return QueryPoll::Ready(Err(fault("definition trait header source differs"))); }
            Some(actual)
        } else { None };
        let implementation = if role == DefinitionHeaderRole::Impl {
            if let Some(raw) = headers.header_by_owner(key.definition) {
                let actual = ready!(db.fetch(&SyntaxProvider, QueryKey::ImplTraitIdentity(Box::new(ImplHeaderSyntaxKey { inventory: key.inventory.clone(), implementation: raw.identity() })))).value;
                let QueryValue::ImplTraitIdentity(identity) = actual.as_ref() else { return QueryPoll::Ready(Err(fault("definition impl membership query differs"))); };
                if identity.syntax().canonical_owner().map(|owner| owner.identity()) != Some(key.definition)
                    || !identity.matches_metadata(parse, header, context) { return QueryPoll::Ready(Err(fault("definition impl membership source differs"))); }
                if identity.result() == ImplTraitIdentityResult::Pending { pending = Some(DefinitionHeaderPending::ImplMembership); }
                Some(actual)
            } else { pending = Some(DefinitionHeaderPending::ImplMembership); None }
        } else { None };
        let mut required = BTreeMap::from([(key.definition, DefinitionKind::Declaration)]);
        if let SourceHeaderParent::Associated(parent) = parent { required.insert(parent, DefinitionKind::Declaration); }
        let bindings = ready!(SourceBindings::prepare(db, catalog_input, required));
        if let Some(why) = bindings.pending { pending = Some(DefinitionHeaderPending::Source(why)); }
        let span = Span { origin: parse.content, start: context.range.start, end: context.range.end };
        let scope = context.scope.clone();
        let name = name.map(|name| name.text().to_string());
        QueryPoll::Ready(Ok(Self { key: key.clone(), receipt, declaration, inventory, trait_declaration,
            implementation, associated_parent, bindings, role, name, span, edition, scope, parent,
            function, declared_type, initializer_equals, pending }))
    }

    /// Established inputs for the complete DefinitionFacts factory. Every
    /// compiler-computed payload in semantic_holds still requires a producer.
    pub(super) fn encode_header<'cx>(&self, cx: Cx<'cx>) -> Result<DefinitionHeaderInputs<'cx>, DefinitionHeaderPending> {
        if let Some(pending) = self.pending { return Err(pending); }
        let Some(&definition) = self.bindings.definitions.get(&self.key.definition) else { return Err(DefinitionHeaderPending::Source(SourceTypePending::SourceDefinition)); };
        if definition.world() != cx.services().source_world() { return Err(DefinitionHeaderPending::Source(SourceTypePending::SourceDefinition)); }
        let generics = cx.generics_of(definition);
        if cx.services().require_generic_value(definition, generics).is_err() { return Err(DefinitionHeaderPending::Source(SourceTypePending::ParameterLayout)); }
        let associated_parent = match self.parent {
            SourceHeaderParent::Associated(parent) => Some(*self.bindings.definitions.get(&parent).ok_or(DefinitionHeaderPending::ParentJoin)?),
            SourceHeaderParent::Pending => return Err(DefinitionHeaderPending::ParentJoin),
            SourceHeaderParent::Lexical(_) | SourceHeaderParent::Foreign(_) => None,
        };
        Ok(DefinitionHeaderInputs { definition, name: self.name.as_deref().map(|name| cx.retain_adt_symbol(name)),
            span: self.span, generics, associated_parent, role: self.role, function_syntax: self.function })
    }
}
pub(super) struct DefinitionHeaderInputs<'cx> {
    pub(super) definition: DefinitionId,
    pub(super) name: Option<Symbol<'cx>>,
    pub(super) span: Span,
    pub(super) generics: Generics<'cx, prepared::PreparedProvider<'cx>>,
    pub(super) associated_parent: Option<DefinitionId>,
    pub(super) role: DefinitionHeaderRole,
    pub(super) function_syntax: Option<SourceFunctionHeader>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum DefinitionTypePending { NonNominal, SourceWorld, SourceGenerics }
impl<'cx> DefinitionHeaderInputs<'cx> {
    pub(super) fn semantic_holds(&self) -> &[DefinitionSemanticHold] { SEMANTIC_HOLDS }
    /// Exact pinned nominal type_of construction, without demanding its fields.
    /// Other declared Type kinds and complete DefinitionFacts remain held.
    pub(super) fn nominal_type(&self, cx: Cx<'cx>)
        -> Result<rustc_type_ir::EarlyBinder<Cx<'cx>, super::model::Ty<'cx, prepared::PreparedProvider<'cx>>>, DefinitionTypePending>
    {
        use rustc_type_ir::inherent::*;
        if !matches!(self.role, DefinitionHeaderRole::Struct | DefinitionHeaderRole::Enum | DefinitionHeaderRole::Union) {
            return Err(DefinitionTypePending::NonNominal);
        }
        if self.definition.world() != cx.services().source_world() { return Err(DefinitionTypePending::SourceWorld); }
        if cx.services().require_generic_value(self.definition, self.generics).is_err() { return Err(DefinitionTypePending::SourceGenerics); }
        let def = cx.adt_def(self.definition);
        let args = super::model::Args::identity_for_item(cx, self.definition);
        Ok(rustc_type_ir::EarlyBinder::bind(cx, super::model::Ty::new_adt(cx, def, args)))
    }
}
#[derive(Debug)]
pub struct CurrentDefinitionHeader { value: Arc<QueryValue>, guard: PreparedInputGuard }
impl CurrentDefinitionHeader {
    pub(crate) fn capture(db: &mut Database, key: &DefinitionHeaderKey) -> QueryPoll<Self> {
        let query = QueryKey::DefinitionHeaderPlan(Box::new(key.clone()));
        let value = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !value.cacheable || !matches!(value.value.as_ref(), QueryValue::DefinitionHeaderPlan(plan) if plan.key() == key) { return QueryPoll::Ready(Err(fault("definition header memo is not retained source preparation"))); }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, value.fingerprint));
        QueryPoll::Ready(Ok(Self { value: value.value, guard }))
    }
    pub(crate) fn plan(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(plan) = self.value.as_ref() else { unreachable!("captured definition header memo") }; plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> { self.guard.require_current(db) }
    pub(crate) fn fingerprint(&self) -> Digest { self.guard.fingerprint() }
}
impl StableEncode for PreparedDefinitionHeader {
    const DOMAIN: &'static str = "lrr/v1/actual-definition-header";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key); e.record(self.declaration()); e.record(&self.bindings);
        let QueryValue::PreparedScopeImplHeaders(inventory) = self.inventory.as_ref() else { unreachable!("captured inventory") };
        e.digest(inventory.fingerprint()); e.u8(self.role as u8); e.option(&self.name, |e, name| e.string(name));
        e.digest(self.span.origin); e.u32(self.span.start); e.u32(self.span.end); e.u16(self.edition); e.record(&self.scope);
        match &self.parent {
            SourceHeaderParent::Associated(owner) => { e.u8(0); e.digest(*owner); },
            SourceHeaderParent::Lexical(scope) => { e.u8(1); e.record(scope); },
            SourceHeaderParent::Foreign(scope) => { e.u8(2); e.record(scope); },
            SourceHeaderParent::Pending => e.u8(3),
        }
        let span = |e: &mut StableEncoder, range: &SourceRange| { e.u32(range.start); e.u32(range.end); };
        e.option(&self.function, |e, function| {
            span(e, &function.parameters); e.option(&function.result, span); e.option(&function.abi, span); e.option(&function.body, span);
            e.bool(function.constant_keyword); e.bool(function.asynchronous_keyword); e.bool(function.unsafe_keyword); e.bool(function.default_keyword);
        });
        e.option(&self.declared_type, span); e.option(&self.initializer_equals, |e, value| e.bool(*value));
        e.option(&self.trait_declaration, |e, value| { let QueryValue::TraitDeclaration(trait_) = value.as_ref() else { unreachable!() }; e.record(trait_.as_ref()); });
        e.option(&self.implementation, |e, value| { let QueryValue::ImplTraitIdentity(impl_) = value.as_ref() else { unreachable!() }; e.record(impl_.as_ref()); });
        e.option(&self.associated_parent, |e, value| { let QueryValue::DeclarationGenerics(parent) = value.as_ref() else { unreachable!() }; e.record(parent.as_ref()); });
        e.option(&self.pending, |e, pending| match pending {
            DefinitionHeaderPending::ParentJoin => e.u8(0), DefinitionHeaderPending::ImplMembership => e.u8(1),
            DefinitionHeaderPending::Source(why) => { e.u8(2); e.u32(*why as u32); }, DefinitionHeaderPending::UnsupportedRole => e.u8(3),
        });
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }

/// Metadata-only controls for actual IDs chosen by the existing fixture harness.
/// Expected tuple: declaration, source role, actual name, associated parent.
/// No fixture spelling is used by production header preparation.
#[cfg(test)]
pub(crate) fn check_actual_definition_headers(
    db: &mut Database,
    inventory: &SealKey,
    expected: &[(Digest, DefinitionHeaderRole, Option<&str>, Option<Digest>)],
) -> Result<(Vec<CurrentDefinitionHeader>, usize), Diagnostic> {
    use super::{configuration::SolverConfigurationKey, model::{Args, GenericParameter, Arg}};
    use rustc_type_ir::inherent::*;

    let before_capture = db.trace.events.len();
    let input = db.input(&InputKey::DiscoverySeal(inventory.clone()))
        .ok_or_else(|| fault("actual definition control catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        return Err(fault("actual definition control catalog differs"));
    };
    let root = catalog.impl_catalog.as_ref().ok_or_else(|| fault("actual definition control root absent"))?.root();
    let configuration = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(
        SolverConfigurationKey::from_source(root),
    ))) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual definition control configuration needs input")),
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        return Err(fault("actual definition control configuration query differs"));
    };
    let mut held = Vec::new();
    let mut count = 0;
    for &(definition, role, name, parent) in expected {
        let key = DefinitionHeaderKey { inventory: inventory.clone(), definition };
        let current = match CurrentDefinitionHeader::capture(db, &key) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("actual definition control header needs input")),
        };
        current.require_current(db)?;
        let again = match CurrentDefinitionHeader::capture(db, &key) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("actual repeated definition header needs input")),
        };
        assert!(Arc::ptr_eq(&current.value, &again.value), "a declaration reuses one immutable header plan");
        assert_eq!(current.fingerprint(), again.fingerprint());
        let plan = current.plan();
        assert_eq!(plan.key(), &key);
        assert_eq!((plan.role(), plan.name()), (role, name));
        assert_eq!(plan.generic_owner(), definition);
        assert_eq!(plan.pending(), None, "actual selected header metadata remains incomplete: definition {definition:?}, name {name:?}, role {role:?}");
        match (plan.source_parent(), parent) {
            (SourceHeaderParent::Associated(actual), Some(expected)) => assert_eq!(*actual, expected),
            (SourceHeaderParent::Lexical(_) | SourceHeaderParent::Foreign(_), None) => {},
            (actual, expected) => panic!("actual associated source parent differs: {actual:?}, expected {expected:?}"),
        }
        let (parse, _, context, _) = plan.declaration().source_parts().expect("actual source declaration");
        assert_eq!(plan.span(), super::model::Span { origin: parse.content, start: context.range.start, end: context.range.end });
        assert_eq!(plan.scope(), &context.scope);
        assert!(matches!(plan.edition(), 2015 | 2018 | 2021 | 2024));
        assert!(!plan.semantic_holds().is_empty());
        assert!(plan.semantic_holds().contains(&DefinitionSemanticHold::PredicateClosure));
        assert!(plan.semantic_holds().contains(&DefinitionSemanticHold::Variance));
        assert!(plan.semantic_holds().contains(&DefinitionSemanticHold::CompilerConstness));
        assert_eq!(plan.source_function().is_some(), role == DefinitionHeaderRole::Function);
        if role == DefinitionHeaderRole::Trait { assert!(plan.trait_header().is_some()); }
        if role == DefinitionHeaderRole::Impl {
            assert!(matches!(plan.impl_membership(), Some(ImplTraitIdentityResult::Trait(_) | ImplTraitIdentityResult::Inherent)));
            assert_eq!(plan.name(), None, "an unnamed actual impl must not acquire a fabricated source name");
        }
        let layouts = plan.generic_layouts()?;
        let before_encode = db.trace.events.len();
        configuration.with_source_context(db, Arc::clone(&input), Some(&layouts), |cx, _| {
            assert!(cx.services().ready(cx).is_err(), "source headers cannot grant complete solver readiness");
            let encoded = plan.encode_header(cx).unwrap_or_else(|pending| panic!("actual source header conversion pending: {pending:?}"));
            assert_eq!(encoded.definition.source(), definition);
            assert_eq!(encoded.definition.world(), catalog.seal.fingerprint());
            assert_eq!(encoded.name.map(|name| name.0), name);
            assert_eq!(encoded.span, plan.span());
            assert_eq!(encoded.role, role);
            assert_eq!(encoded.associated_parent.map(|parent| parent.source()), parent);
            assert_eq!(encoded.function_syntax, plan.source_function());
            if matches!(role, DefinitionHeaderRole::Struct | DefinitionHeaderRole::Enum | DefinitionHeaderRole::Union) {
                let nominal = encoded.nominal_type(cx).expect("actual nominal declared Type payload");
                let rustc_type_ir::TyKind::Adt(adt, args) = nominal.skip_binder().kind() else { panic!("nominal header must construct the actual shared ADT Type"); };
                assert_eq!(adt.definition, encoded.definition);
                assert_eq!(args, Args::identity_for_item(cx, encoded.definition));
            } else {
                assert_eq!(encoded.nominal_type(cx).unwrap_err(), DefinitionTypePending::NonNominal,
                    "other declaration kinds need their actual semantic Type producer");
            }
            assert_eq!(encoded.semantic_holds(), plan.semantic_holds());
            assert!(!encoded.semantic_holds().is_empty());
            cx.services().require_generic_value(encoded.definition, encoded.generics).unwrap();
            let args = Args::identity_for_item(cx, encoded.definition);
            assert_eq!(encoded.generics.count(), args.items().len());
            for (parameter, argument) in encoded.generics.parameters.iter().zip(args.items()) {
                match (parameter, argument) {
                    (GenericParameter::Type(parameter), Arg::Type(value)) =>
                        assert!(matches!(value.kind(), rustc_type_ir::TyKind::Param(actual) if actual==parameter)),
                    (GenericParameter::Region(parameter), Arg::Region(value)) =>
                        assert!(matches!(value.kind(), rustc_type_ir::RegionKind::ReEarlyParam(actual) if actual==parameter)),
                    (GenericParameter::Const(parameter, _), Arg::Const(value)) =>
                        assert!(matches!(value.kind(), rustc_type_ir::ConstKind::Param(actual) if actual==parameter)),
                    _ => panic!("actual declaration's identity argument changed its source namespace"),
                }
            }
            assert!(cx.services().ready(cx).is_err(), "encoded header inputs are still incomplete DefinitionFacts");
        })?;
        assert_eq!(before_encode, db.trace.events.len(), "header input conversion must not reenter source preparation");
        current.require_current(db)?;
        held.push(current);
        count += 1;
    }
    assert!(db.trace.events[before_capture..].iter().all(|event| !matches!(event.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )), "actual header demand must leave function signatures and executable bodies cold");
    Ok((held, count))
}
