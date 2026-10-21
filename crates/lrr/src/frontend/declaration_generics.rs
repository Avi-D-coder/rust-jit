//! Immutable generic declaration metadata shared by ordinary source clients.
//! Namespace/arity metadata is not typed facts, applicability or a body check.
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{dependency::{InputKey, InputValue}, key::QueryKey, memo::Database,
        poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    syntax::{signatures::{DefinitionParameter, GenericKind, GenericSourceLayout},
        outline::{HeaderRecipe, DeclarationContext}, parse::ParseKey},
};
use ra_ap_syntax::{AstNode, WalkEvent, ast};
use std::sync::Arc;

#[path = "opaque_return_headers.rs"]
mod opaque_return_headers;
pub(crate) use opaque_return_headers::OpaqueReturnHeaders;

#[derive(Debug)]
pub struct DeclarationGenerics {
    definition: Digest,
    metadata: Option<[Arc<InputValue>; 3]>,
    parsed: Option<Arc<QueryValue>>,
    parameters: Vec<DefinitionParameter>,
    layout: Option<GenericSourceLayout>,
    /// A trait declaration's own implicit Self differs from an associated
    /// function's inherited Self. Neither changes ordinary source ordinals.
    trait_self: bool,
    function: bool,
    /// Syntax evidence only: anonymous compiler parameter identities have not
    /// been produced. Neither location is a DefinitionId or a guessed count.
    argument_impl_traits: Vec<crate::syntax::spans::SourceRange>,
    parameter_type_macros: Vec<crate::syntax::spans::SourceRange>,
    opaque_returns: Option<OpaqueReturnHeaders>,
    const_types: std::collections::BTreeMap<u32, crate::syntax::spans::SourceRange>,
    impl_regions_pending: bool,
}
impl DeclarationGenerics {
    pub fn definition(&self) -> Digest { self.definition }
    pub fn parameters(&self) -> &[DefinitionParameter] { &self.parameters }
    pub fn layout(&self) -> Option<GenericSourceLayout> { self.layout }
    pub(crate) fn has_trait_self(&self) -> bool { self.trait_self }
    pub(crate) fn is_function(&self) -> bool { self.function }
    pub(crate) fn argument_impl_traits(&self) -> &[crate::syntax::spans::SourceRange] { &self.argument_impl_traits }
    pub(crate) fn parameter_type_macros(&self) -> &[crate::syntax::spans::SourceRange] { &self.parameter_type_macros }
    pub(crate) fn has_unproduced_parameter_identities(&self) -> bool {
        !self.argument_impl_traits.is_empty() || !self.parameter_type_macros.is_empty() || self.impl_regions_pending
    }
    pub(crate) fn impl_regions_pending(&self) -> bool { self.impl_regions_pending }
    pub(crate) fn opaque_returns(&self) -> Option<&OpaqueReturnHeaders> { self.opaque_returns.as_ref() }
    pub(crate) fn opaque_return_syntax(&self, position: u32) -> Result<Option<ast::ImplTraitType>, Diagnostic> {
        match (self.opaque_returns.as_ref(), self.source_parts()) {
            (Some(plan), Some((_, _, _, parsed))) => plan.syntax(parsed, position),
            _ => Ok(None),
        }
    }
    pub(crate) fn source_parts(&self) -> Option<(&ParseKey, &HeaderRecipe, &DeclarationContext, &crate::syntax::parse::ParsedFile)> {
        let [source, header, context] = self.metadata.as_ref()?;
        let (InputValue::DefinitionSource(source), InputValue::DefinitionHeader(header), InputValue::DeclarationContext(context), Some(QueryValue::ParseFile(parsed))) =
            (source.as_ref(), header.as_ref(), context.as_ref(), self.parsed.as_deref()) else { unreachable!("captured declaration inputs") };
        Some((source, header, context, parsed))
    }
    pub(crate) fn const_type(&self, ordinal: u32) -> Result<Option<ast::Type>, Diagnostic> {
        let Some(range) = self.const_types.get(&ordinal) else { return Ok(None); };
        let Some((_, _, _, parsed)) = self.source_parts() else { return Ok(None); };
        parsed.syntax_at(*range, |node| ast::Type::can_cast(node.kind())).map(|node| node.and_then(ast::Type::cast))
    }
    pub(crate) fn matches_metadata(&self, source: &ParseKey, header: &HeaderRecipe,
        context: &DeclarationContext) -> bool {
        let Some([actual_source, actual_header, actual_context]) = &self.metadata else { return false; };
        matches!((actual_source.as_ref(), actual_header.as_ref(), actual_context.as_ref()),
            (InputValue::DefinitionSource(parse), InputValue::DefinitionHeader(recipe), InputValue::DeclarationContext(declaration))
                if parse == source && recipe == header && declaration == context)
    }
    pub(crate) fn prepare(db: &mut Database, definition: Digest) -> QueryPoll<Self> {
        let mut result = Self { definition, metadata: None, parsed: None, parameters: Vec::new(),
            layout: None, trait_self: false, function: false, argument_impl_traits: Vec::new(),
            parameter_type_macros: Vec::new(), opaque_returns: None, const_types: std::collections::BTreeMap::new(), impl_regions_pending: false };
        let source = db.input(&InputKey::DefinitionSource(definition));
        let header = db.input(&InputKey::DefinitionHeader(definition));
        let context = db.input(&InputKey::DeclarationContext(definition));
        let (Some(source), Some(header), Some(context)) = (source, header, context) else {
            return QueryPoll::Ready(Ok(result));
        };
        let (InputValue::DefinitionSource(parse), InputValue::DefinitionHeader(recipe),
            InputValue::DeclarationContext(declaration)) = (source.as_ref(), header.as_ref(), context.as_ref())
            else { return QueryPoll::Ready(Err(fault("generic declaration input variants differ"))); };
        if parse.profile != recipe.profile { return QueryPoll::Ready(Err(fault("generic declaration source/profile differs"))); }
        let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(tree) = parsed.as_ref() else { return QueryPoll::Ready(Err(fault("generic declaration parser differs"))); };
        let node = match tree.syntax_at(declaration.range, |node| ast::Item::can_cast(node.kind())) {
            Ok(Some(node)) => node, Ok(None) => return QueryPoll::Ready(Err(fault("actual generic declaration absent"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (parameters, layout) = match tree.declaration_generics(&node) {
            Ok(value) => value, Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if layout.count().and_then(|count| usize::try_from(count).ok()) != Some(parameters.len()) {
            return QueryPoll::Ready(Err(fault("actual generic declaration count differs")));
        }
        result.trait_self = ast::Trait::can_cast(node.kind());
        result.function = ast::Fn::can_cast(node.kind());
        result.impl_regions_pending = crate::syntax::signatures::impl_region_binders_pending(&node);
        if result.function {
            let function = match tree.function_header(definition, recipe, declaration, None) {
                Ok(function) => function, Err(error) => return QueryPoll::Ready(Err(error)),
            };
            // Inspect only retained parameter Types. An executable const
            // expression can contain declarations belonging to another demand;
            // those must not change this function's parameter completeness.
            if let Some(parameters) = function.param_list() {
                for ty in parameters.params().filter_map(|parameter| parameter.ty())
                    .chain(parameters.self_param().and_then(|parameter| parameter.ty())) {
                    let mut walk = ty.syntax().preorder();
                    while let Some(event) = walk.next() {
                        let WalkEvent::Enter(node) = event else { continue; };
                        if ast::Expr::can_cast(node.kind()) || ast::Item::can_cast(node.kind()) {
                            walk.skip_subtree(); continue;
                        }
                        if ast::MacroType::can_cast(node.kind()) {
                            result.parameter_type_macros.push(crate::syntax::ast_adapter::range(node.text_range()));
                            walk.skip_subtree(); continue;
                        }
                        if ast::ImplTraitType::can_cast(node.kind()) {
                            result.argument_impl_traits.push(crate::syntax::ast_adapter::range(node.text_range()));
                        }
                    }
                }
            }
            result.opaque_returns = match OpaqueReturnHeaders::capture(tree, definition, &function) {
                Ok(plan) => plan, Err(error) => return QueryPoll::Ready(Err(error)),
            };
        }
        result.const_types = match tree.declaration_const_type_ranges(&node) {
            Ok(ranges) => ranges, Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // Function early/late lifetime classification must come from the real
        // binder service. Never expose all written function lifetimes as early.
        if result.impl_regions_pending || result.function && parameters[..layout.own as usize].iter().any(|p| p.kind == GenericKind::Lifetime) {
            result.parameters = parameters; result.metadata = Some([source, header, context]);
            result.parsed = Some(parsed); return QueryPoll::Ready(Ok(result));
        }
        result.parameters = parameters; result.layout = Some(layout);
        result.metadata = Some([source, header, context]); result.parsed = Some(parsed);
        QueryPoll::Ready(Ok(result))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-DECLARATION-GENERICS", message.into()) }
impl StableEncode for DeclarationGenerics {
    const DOMAIN: &'static str = "lrr/v5/actual-declaration-generics";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition); e.option(&self.layout, |e, layout| e.record(layout));
        e.bool(self.trait_self); e.bool(self.function);
        e.bool(self.impl_regions_pending);
        for sites in [&self.argument_impl_traits, &self.parameter_type_macros] {
            e.sequence(sites, |e, range| { e.u32(range.start); e.u32(range.end); });
        }
        e.option(&self.opaque_returns,|e,plan| e.record(plan));
        e.map(&self.const_types, |e, ordinal, range| { e.u32(*ordinal); e.u32(range.start); e.u32(range.end); });
        e.sequence(&self.parameters, |e, p| {
            e.u32(p.ordinal); e.string(&p.name);
            e.u32(match p.kind { GenericKind::Type => 0, GenericKind::Lifetime => 1, GenericKind::Const => 2 });
            for tokens in [&p.bounds, &p.default, &p.const_type] { e.sequence(tokens, |e, token| e.record(token)); }
            e.option(&p.anonymous_region, |e, range| { e.u32(range.start); e.u32(range.end); });
        });
        e.option(&self.metadata, |e, inputs| { for input in inputs { e.digest(input.fingerprint()); } });
        e.option(&self.parsed, |e, parsed| { let QueryValue::ParseFile(tree) = parsed.as_ref() else { unreachable!() }; e.record(&tree.key()); });
    }
}
