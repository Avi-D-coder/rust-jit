//! Owned, demand-captured function declarations; no bound solving or body analysis.
use super::{
    ast_adapter::{semantic_name, tokens, TokenRecipe},
    outline::{BodyHandle, DeclarationContext, HeaderRecipe, ScopeKey},
    parse::ParsedFile,
};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncode, StableEncoder},
};
use ra_ap_syntax::{
    ast::{self, HasAttrs, HasGenericParams, HasName, HasTypeBounds},
    AstNode, SyntaxNode,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum GenericKind {
    Lifetime,
    Type,
    Const,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DefinitionParameter {
    pub ordinal: u32,
    pub kind: GenericKind,
    pub name: String,
    pub bounds: Vec<TokenRecipe>,
    pub default: Vec<TokenRecipe>,
    pub const_type: Vec<TokenRecipe>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParameterRecipe {
    pub pattern: Vec<TokenRecipe>,
    pub ty: Vec<TokenRecipe>,
    pub symbolic: Option<crate::traits::term::TyTerm>,
    pub receiver: bool,
    pub variadic: bool,
}
/// Real declaration ownership, captured with its generic header. LRR keeps
/// own parameters first so written call arguments retain their source ordinals.
/// A backend may use a different order without changing these identities.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct GenericSourceLayout {
    pub own: u32,
    pub inherited: u32,
    pub implicit_self: bool,
}
impl GenericSourceLayout {
    pub fn count(self) -> Option<u32> {
        self.own
            .checked_add(self.inherited)?
            .checked_add(u32::from(self.implicit_self))
    }
}
impl StableEncode for GenericSourceLayout {
    const DOMAIN: &'static str = "lrr/v1/generic-source-layout";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.own);
        e.u32(self.inherited);
        e.bool(self.implicit_self);
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionSignature {
    pub definition: Digest,
    pub scope: ScopeKey,
    pub header: HeaderRecipe,
    pub generics: Vec<DefinitionParameter>,
    /// None identifies a legacy synthetic signature, not an empty source binder.
    pub generic_source_layout: Option<GenericSourceLayout>,
    pub parameters: Vec<ParameterRecipe>,
    /// Empty means the declared unit result, not a fresh inference variable.
    pub result: Vec<TokenRecipe>,
    pub result_symbolic: Option<crate::traits::term::TyTerm>,
    pub predicates: Vec<TokenRecipe>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-SIGNATURE", message.into())
}
fn owned(node: Option<ra_ap_syntax::SyntaxNode>) -> Vec<TokenRecipe> {
    node.map_or_else(Vec::new, |n| tokens(&n, None))
}
impl ParsedFile {
    /// Written Type tokens of a signature. A generated `$crate` root is
    /// spelled with the crate its definition-site hygiene selects, as the
    /// value-path lowering spells it, so the ordinary source Type service
    /// resolves it in that crate (`&mut $crate::fmt::Formatter` of a
    /// builtin derive names core's `Formatter`).
    pub(crate) fn type_tokens(
        &self,
        node: Option<ra_ap_syntax::SyntaxNode>,
    ) -> Result<Vec<TokenRecipe>, Diagnostic> {
        let Some(node) = node else {
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        for token in node
            .descendants_with_tokens()
            .filter_map(|item| item.into_token())
            .filter(|token| !token.kind().is_trivia())
        {
            let text = if token.text() == "$crate" {
                match self.dollar_crate_root(super::ast_adapter::range(token.text_range()))? {
                    Some((unit, _)) => crate::frontend::callables::dollar_crate_type_segment(unit),
                    None => token.text().to_owned(),
                }
            } else {
                token.text().to_owned()
            };
            result.push(TokenRecipe {
                kind: token.kind() as u16,
                text,
            });
        }
        Ok(result)
    }
    /// Generic bounds are Type syntax too: a generated `$crate` root in a
    /// bound (`T: $crate::clone::Clone` of a builtin derive) is spelled with
    /// the crate its hygiene selects, as `type_tokens` spells parameter and
    /// result types, so the ordinary bound lookup resolves it in that crate.
    /// Own parameters come first, then the owner impl's or trait's, in
    /// `owner_generics` order.
    fn spell_generic_bounds(
        &self,
        declaration: &SyntaxNode,
        generics: &mut [DefinitionParameter],
    ) -> Result<(), Diagnostic> {
        let owner = associated_owner(declaration)
            .and_then(|owner| owner.children().find_map(ast::GenericParamList::cast));
        let written = declaration
            .children()
            .find_map(ast::GenericParamList::cast)
            .into_iter()
            .chain(owner)
            .flat_map(|list| list.generic_params());
        for (parameter, written) in generics.iter_mut().zip(written) {
            let bounds = match written {
                ast::GenericParam::TypeParam(p) => p.type_bound_list(),
                ast::GenericParam::LifetimeParam(p) => p.type_bound_list(),
                ast::GenericParam::ConstParam(_) => None,
            };
            if parameter.bounds.iter().any(|token| token.text == "$crate") {
                parameter.bounds = self.type_tokens(bounds.map(|b| b.syntax().clone()))?;
            }
        }
        Ok(())
    }
    /// The selected declaration's own and immediate associated owner's
    /// generic syntax, using the same source binder as ordinary functions.
    pub(crate) fn declaration_generics(
        &self,
        declaration: &SyntaxNode,
    ) -> Result<(Vec<DefinitionParameter>, GenericSourceLayout), Diagnostic> {
        let own = capture_generics(declaration.children().find_map(ast::GenericParamList::cast))?;
        let (mut generics, layout) = associated_generics_with_layout(declaration, own)?;
        self.spell_generic_bounds(declaration, &mut generics)?;
        Ok((generics, layout))
    }
    /// Direct retained locations of const parameter Types in the same own/owner
    /// declaration order as declaration_generics. No token-text parser.
    pub(crate) fn declaration_const_type_ranges(&self, declaration: &SyntaxNode)
        -> Result<std::collections::BTreeMap<u32, super::spans::SourceRange>, Diagnostic> {
        let mut result = std::collections::BTreeMap::new();
        let mut ordinal = 0u32;
        for owner in std::iter::once(declaration.clone()).chain(associated_owner(declaration)) {
            for parameter in owner.children().find_map(ast::GenericParamList::cast).into_iter().flat_map(|list| list.generic_params()) {
                if let ast::GenericParam::ConstParam(parameter) = parameter {
                    if let Some(ty) = parameter.ty() { result.insert(ordinal, super::ast_adapter::range(ty.syntax().text_range())); }
                }
                ordinal = ordinal.checked_add(1).ok_or_else(|| fault("generic parameter ordinal overflow"))?;
            }
        }
        Ok(result)
    }
    /// Own and immediate impl/trait where clauses belong to the same
    /// declared Type scope. This prepares signature facts, never a body.
    pub(crate) fn declaration_predicates(
        &self,
        declaration: &SyntaxNode,
    ) -> Result<Vec<TokenRecipe>, Diagnostic> {
        let own = declaration.children().find_map(ast::WhereClause::cast);
        let mut predicates = self.type_tokens(own.map(|clause| clause.syntax().clone()))?;
        let inherited = associated_owner(declaration)
            .and_then(|owner| owner.children().find_map(ast::WhereClause::cast));
        let inherited = self.type_tokens(inherited.map(|clause| clause.syntax().clone()))?;
        if predicates.is_empty() {
            predicates = inherited;
        } else if !inherited.is_empty() {
            if !predicates.last().is_some_and(|token| token.text == ",") {
                predicates.push(TokenRecipe {
                    kind: ra_ap_syntax::SyntaxKind::COMMA as u16,
                    text: ",".into(),
                });
            }
            predicates.extend(inherited.into_iter().skip(1));
        }
        Ok(predicates)
    }
    /// Capture the selected initializer's real associated Type binder.
    /// It remains an initializer declaration, never a synthetic function.
    pub(crate) fn initializer_parameters(
        &self,
        header: &HeaderRecipe,
        context: &DeclarationContext,
        body: &BodyHandle,
    ) -> Result<Vec<DefinitionParameter>, Diagnostic> {
        if header.profile != self.profile {
            return Err(fault("initializer/source profile mismatch"));
        }
        let root = self.entered_body_root(body)?;
        let item = root
            .parent()
            .ok_or_else(|| fault("initializer declaration absent"))?;
        // An enum variant's discriminant constant declares no generics of its
        // own (E-R5-013); its declaration is the variant.
        if let Some(variant) = item.parent().filter(|variant| {
            super::body_index::is_initializer_owner(&item) && ast::Variant::can_cast(variant.kind())
        }) {
            if super::ast_adapter::range(variant.text_range()) != context.range
                || tokens(&variant, Some(item.text_range())) != header.tokens
            {
                return Err(fault("initializer declaration/header/source mismatch"));
            }
            return Ok(Vec::new());
        }
        if !(ast::Const::can_cast(item.kind()) || ast::Static::can_cast(item.kind()))
            || super::ast_adapter::range(item.text_range()) != context.range
            || tokens(&item, Some(root.text_range())) != header.tokens
        {
            return Err(fault("initializer declaration/header/source mismatch"));
        }
        self.declaration_generics(&item)
            .map(|(generics, _)| generics)
    }
    /// A `const`/`static` item's initializer as a call signature: no
    /// parameters, its declared type the result (E-R5-007). `None` when the
    /// declaration is not a constant or static.
    fn initializer_signature(
        &self,
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
    ) -> Result<Option<FunctionSignature>, Diagnostic> {
        // An enum variant's `= expr` is an anonymous constant of the enum's
        // representation type, declared in the enum's scope (E-R5-013).
        if let Ok(Some(variant)) =
            self.syntax_at(context.range, |node| ast::Variant::can_cast(node.kind()))
        {
            return Ok(Some(Self::discriminant_signature(
                definition, header, context, &variant,
            )?));
        }
        let Ok(Some(item)) = self.syntax_at(context.range, |node| {
            ast::Const::can_cast(node.kind()) || ast::Static::can_cast(node.kind())
        }) else {
            return Ok(None);
        };
        let ty = ast::Const::cast(item.clone())
            .and_then(|item| item.ty())
            .or_else(|| ast::Static::cast(item.clone()).and_then(|item| item.ty()));
        let Some(ty) = ty else {
            return Err(fault("constant declaration lacks its declared type"));
        };
        let (generics, layout) = self.declaration_generics(&item)?;
        Ok(Some(FunctionSignature {
            definition,
            scope: context.scope.clone(),
            header: header.clone(),
            result_symbolic: lower_type(&ty, definition, &generics, 0)?,
            result: self.type_tokens(Some(ty.syntax().clone()))?,
            generics,
            generic_source_layout: Some(layout),
            parameters: Vec::new(),
            predicates: self.declaration_predicates(&item)?,
        }))
    }
    /// The zero-parameter signature of an enum variant's discriminant
    /// constant: its result is the enum's `#[repr(<integer>)]` type, `isize`
    /// without one (E-R5-013).
    fn discriminant_signature(
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
        variant: &SyntaxNode,
    ) -> Result<FunctionSignature, Diagnostic> {
        use crate::ir::op::PrimitiveType as P;
        let enumeration = variant
            .parent()
            .and_then(|list| list.parent())
            .and_then(ast::Enum::cast)
            .ok_or_else(|| fault("discriminant variant lacks its enum"))?;
        let mut primitive = P::Isize;
        let mut spelling = "isize";
        for attribute in enumeration.attrs() {
            let Some(meta) = attribute.meta() else {
                continue;
            };
            if meta
                .path()
                .is_none_or(|path| path.syntax().text().to_string() != "repr")
            {
                continue;
            }
            for token in meta
                .syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
            {
                let (name, found) = match token.text() {
                    "i8" => ("i8", P::I8),
                    "i16" => ("i16", P::I16),
                    "i32" => ("i32", P::I32),
                    "i64" => ("i64", P::I64),
                    "i128" => ("i128", P::I128),
                    "isize" => ("isize", P::Isize),
                    "u8" => ("u8", P::U8),
                    "u16" => ("u16", P::U16),
                    "u32" => ("u32", P::U32),
                    "u64" => ("u64", P::U64),
                    "u128" => ("u128", P::U128),
                    "usize" => ("usize", P::Usize),
                    _ => continue,
                };
                (spelling, primitive) = (name, found);
            }
        }
        Ok(FunctionSignature {
            definition,
            scope: context.scope.clone(),
            header: header.clone(),
            result_symbolic: Some(crate::traits::term::TyTerm::Primitive(primitive)),
            result: vec![TokenRecipe {
                kind: ra_ap_syntax::SyntaxKind::IDENT as u16,
                text: spelling.into(),
            }],
            generics: Vec::new(),
            generic_source_layout: Some(GenericSourceLayout {
                own: 0,
                inherited: 0,
                implicit_self: false,
            }),
            parameters: Vec::new(),
            predicates: Vec::new(),
        })
    }
    pub fn function_signature(
        &self,
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
        body: Option<&BodyHandle>,
    ) -> Result<FunctionSignature, Diagnostic> {
        if header.profile != self.profile {
            return Err(fault("signature/source profile mismatch"));
        }
        if body.is_some_and(|b| b.def_key != definition) {
            return Err(fault("signature/body definition mismatch"));
        }
        if let Some(signature) = self.initializer_signature(definition, header, context)? {
            return Ok(signature);
        }
        let function = self
            .syntax_at(context.range, |node| ast::Fn::can_cast(node.kind()))
            .map_err(|_| fault("signature declaration is not in its immutable source"))?
            .and_then(ast::Fn::cast)
            .ok_or_else(|| fault("signature declaration is not in its immutable source"))?;
        let block = function.syntax().children().find_map(ast::BlockExpr::cast);
        let excluded = block.as_ref().map(|b| b.syntax().text_range());
        if body.is_some_and(|handle| {
            excluded.is_none_or(|r| super::ast_adapter::range(r) != handle.block_range())
        }) || tokens(function.syntax(), excluded) != header.tokens
        {
            return Err(fault(
                "signature declaration is not in its immutable source",
            ));
        }
        let (generics, layout) = self.declaration_generics(function.syntax())?;
        let list = function
            .param_list()
            .ok_or_else(|| fault("function lacks its parameter list"))?;
        let mut parameters = Vec::new();
        if let Some(receiver) = list.self_param() {
            // The receiver's type as rustc reads it: `self`/`mut self` is
            // `Self`, `&'a mut self` is `&'a mut Self`, `self: T` is `T`.
            let ty = match receiver.ty() {
                Some(ty) => self.type_tokens(Some(ty.syntax().clone()))?,
                None => {
                    let token = |kind: ra_ap_syntax::SyntaxKind, text: &str| {
                        super::ast_adapter::TokenRecipe {
                            kind: kind as u16,
                            text: text.into(),
                        }
                    };
                    let mut ty = Vec::new();
                    if receiver.amp_token().is_some() {
                        ty.push(token(ra_ap_syntax::SyntaxKind::AMP, "&"));
                        if let Some(lifetime) = receiver.lifetime() {
                            ty.push(token(
                                ra_ap_syntax::SyntaxKind::LIFETIME_IDENT,
                                &lifetime.syntax().text().to_string(),
                            ));
                        }
                        if receiver.mut_token().is_some() {
                            ty.push(token(ra_ap_syntax::SyntaxKind::MUT_KW, "mut"));
                        }
                    }
                    ty.push(token(ra_ap_syntax::SyntaxKind::SELF_TYPE_KW, "Self"));
                    ty
                }
            };
            parameters.push(ParameterRecipe {
                pattern: tokens(receiver.syntax(), None),
                ty,
                symbolic: None,
                receiver: true,
                variadic: false,
            });
        }
        for parameter in list.params() {
            let variadic = parameter.dotdotdot_token().is_some();
            let ty = self.type_tokens(parameter.ty().map(|n| n.syntax().clone()))?;
            if ty.is_empty() && !variadic {
                return Err(fault("function parameter lacks a declared type"));
            }
            parameters.push(ParameterRecipe {
                pattern: owned(parameter.pat().map(|n| n.syntax().clone())),
                ty,
                symbolic: parameter
                    .ty()
                    .map(|ty| lower_type(&ty, definition, &generics, 0))
                    .transpose()?
                    .flatten(),
                receiver: false,
                variadic,
            });
        }
        if function.ret_type().is_some_and(|ret| ret.ty().is_none()) {
            return Err(fault("explicit function result lacks its declared type"));
        }
        // The associated item's Type scope includes predicates of its
        // actual impl/trait owner. Keep them with the same captured source
        // signature, not a separate body/member lookup at call time.
        let predicates = self.declaration_predicates(function.syntax())?;
        let signature = FunctionSignature {
            definition,
            scope: context.scope.clone(),
            header: header.clone(),
            result_symbolic: match function.ret_type() {
                Some(ret) => ret
                    .ty()
                    .map(|ty| lower_type(&ty, definition, &generics, 0))
                    .transpose()?
                    .flatten(),
                None => Some(crate::traits::term::TyTerm::Primitive(
                    crate::ir::op::PrimitiveType::Unit,
                )),
            },
            generics,
            generic_source_layout: Some(layout),
            parameters,
            result: self.type_tokens(
                function
                    .ret_type()
                    .and_then(|r| r.ty())
                    .map(|n| n.syntax().clone()),
            )?,
            predicates,
        };
        Ok(signature)
    }
}
impl StableEncode for FunctionSignature {
    const DOMAIN: &'static str = "lrr/v3/function-signature";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.record(&self.scope);
        e.record(&self.header);
        e.option(&self.generic_source_layout, |e, layout| e.record(layout));
        e.sequence(&self.generics, |e, p| {
            e.u32(p.ordinal);
            e.u32(match p.kind {
                GenericKind::Lifetime => 1,
                GenericKind::Type => 2,
                GenericKind::Const => 3,
            });
            e.string(&p.name);
            e.sequence(&p.bounds, |e, t| e.record(t));
            e.sequence(&p.default, |e, t| e.record(t));
            e.sequence(&p.const_type, |e, t| e.record(t));
        });
        e.sequence(&self.parameters, |e, p| {
            e.option(&p.symbolic, |e, t| e.record(t));
            e.bool(p.receiver);
            e.bool(p.variadic);
            e.sequence(&p.pattern, |e, t| e.record(t));
            e.sequence(&p.ty, |e, t| e.record(t));
        });
        e.sequence(&self.result, |e, t| e.record(t));
        e.option(&self.result_symbolic, |e, t| e.record(t));
        e.sequence(&self.predicates, |e, t| e.record(t));
    }
}
fn token_bytes(tokens: &[TokenRecipe], capacity: usize) -> Option<usize> {
    tokens.iter().try_fold(
        capacity.checked_mul(std::mem::size_of::<TokenRecipe>())?,
        |bytes, token| bytes.checked_add(token.text.capacity()),
    )
}
impl FunctionSignature {
    fn checked_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.header.estimated_bytes())?
            .checked_add(self.scope.heap_bytes())?
            .checked_add(
                self.generics
                    .capacity()
                    .checked_mul(std::mem::size_of::<DefinitionParameter>())?,
            )?
            .checked_add(
                self.parameters
                    .capacity()
                    .checked_mul(std::mem::size_of::<ParameterRecipe>())?,
            )?;
        for parameter in &self.generics {
            bytes = bytes
                .checked_add(parameter.name.capacity())?
                .checked_add(token_bytes(&parameter.bounds, parameter.bounds.capacity())?)?
                .checked_add(token_bytes(
                    &parameter.default,
                    parameter.default.capacity(),
                )?)?
                .checked_add(token_bytes(
                    &parameter.const_type,
                    parameter.const_type.capacity(),
                )?)?;
        }
        for parameter in &self.parameters {
            bytes = bytes
                .checked_add(token_bytes(
                    &parameter.pattern,
                    parameter.pattern.capacity(),
                )?)?
                .checked_add(token_bytes(&parameter.ty, parameter.ty.capacity())?)?
                .checked_add(
                    parameter
                        .symbolic
                        .as_ref()
                        .map_or(Some(0), |term| term.heap_bytes())?,
                )?;
        }
        bytes = bytes.checked_add(
            self.result_symbolic
                .as_ref()
                .map_or(Some(0), |term| term.heap_bytes())?,
        )?;
        bytes
            .checked_add(token_bytes(&self.result, self.result.capacity())?)?
            .checked_add(token_bytes(&self.predicates, self.predicates.capacity())?)
    }
    pub fn estimated_bytes(&self) -> usize {
        self.checked_bytes().unwrap_or(usize::MAX)
    }
}

/// Unknown paths/projections/binders remain owned syntax for later demand resolution.
/// No nominal identity, region fact or runtime width is guessed here.
pub(crate) fn lower_type(
    ty: &ast::Type,
    definition: Digest,
    generics: &[DefinitionParameter],
    depth: usize,
) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
    use crate::{
        ir::op::PrimitiveType as P,
        traits::term::{ConstTerm, ParamKey, RegionTerm, TyTerm as T},
    };
    let key = |ordinal| ParamKey {
        definition,
        ordinal,
    };
    let parameter = |name: &str, kind| {
        generics
            .iter()
            .find(|p| p.kind == kind && p.name == name)
            .map(|p| key(p.ordinal))
    };
    let nested = |ty: Option<ast::Type>| -> Result<Option<T>, Diagnostic> {
        ty.as_ref()
            .map(|ty| lower_type(ty, definition, generics, depth + 1))
            .transpose()
            .map(Option::flatten)
    };
    let result = match ty {
        ast::Type::PathType(_) => {
            let ts = tokens(ty.syntax(), None);
            if ts.len() != 1 {
                return Ok(None);
            }
            let Some(name) = semantic_name(&ts[0].text) else {
                return Ok(None);
            };
            parameter(&name, GenericKind::Type).map(T::Param)
        }
        ast::Type::TupleType(tuple) => {
            let mut fields = Vec::new();
            for field in tuple.fields() {
                let Some(term) = lower_type(&field, definition, generics, depth + 1)? else {
                    return Ok(None);
                };
                fields.push(term);
            }
            if fields.is_empty() {
                Some(T::Primitive(P::Unit))
            } else {
                Some(T::Tuple(fields))
            }
        }
        ast::Type::SliceType(slice) => nested(slice.ty())?.map(|t| T::Slice(Box::new(t))),
        ast::Type::ArrayType(array) => {
            let Some(element) = nested(array.ty())? else {
                return Ok(None);
            };
            let ts = owned(array.const_arg().map(|c| c.syntax().clone()));
            if ts.len() != 1 {
                return Ok(None);
            }
            let length = if let Some(key) =
                semantic_name(&ts[0].text).and_then(|name| parameter(&name, GenericKind::Const))
            {
                ConstTerm::Param(key)
            } else if let Ok(value) = ts[0].text.replace('_', "").parse::<u64>() {
                ConstTerm::Value(value)
            } else {
                return Ok(None);
            };
            Some(T::Array(Box::new(element), length))
        }
        ast::Type::RefType(reference) => {
            let Some(lifetime) = reference.lifetime() else {
                return Ok(None);
            };
            let Some(name) = canonical_lifetime(&lifetime.syntax().text().to_string()) else {
                return Ok(None);
            };
            let region = if name == "'static" {
                RegionTerm::Static
            } else if let Some(key) = parameter(&name, GenericKind::Lifetime) {
                RegionTerm::Param(key)
            } else {
                return Ok(None);
            };
            nested(reference.ty())?
                .map(|t| T::Ref(region, reference.mut_token().is_some(), Box::new(t)))
        }
        // `*const T` / `*mut T` (E-R5-008): the reserved-identity nominal.
        ast::Type::PtrType(pointer) => nested(pointer.ty())?.map(|pointee| {
            crate::traits::pointer_terms::raw_pointer(pointer.mut_token().is_some(), pointee)
        }),
        // A fn pointer whose leaves are not generic parameters (`fn(i32)`)
        // stays owned syntax for the demand-driven source Type service.
        ast::Type::ForType(_) | ast::Type::FnPtrType(_) => {
            super::symbolic_headers::capture_function_type(ty, definition, generics, depth).ok()
        }
        ast::Type::NeverType(_) => Some(T::Never),
        _ => None,
    };
    Ok(result)
}

pub(crate) fn canonical_lifetime(name: &str) -> Option<String> {
    semantic_name(name.strip_prefix('\'')?).map(|name| format!("'{name}"))
}

/// Pure owned metadata capture. Check token counts and bytes before reserving
/// syntax-dependent storage; this neither resolves names nor evaluates syntax.
fn generic_tokens(
    node: Option<ra_ap_syntax::SyntaxNode>,
    bytes: &mut usize,
) -> Result<Vec<TokenRecipe>, Diagnostic> {
    let Some(node) = node else {
        return Ok(Vec::new());
    };
    let mut count = 0usize;
    let mut text_bytes = 0usize;
    for token in node
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .filter(|token| !token.kind().is_trivia())
    {
        count = count
            .checked_add(1)
            .ok_or_else(|| fault("generic token count overflow"))?;
        text_bytes = text_bytes
            .checked_add(token.text().len())
            .ok_or_else(|| fault("generic token text size overflow"))?;
    }
    let charge = count
        .checked_mul(std::mem::size_of::<TokenRecipe>())
        .and_then(|n| n.checked_add(text_bytes))
        .ok_or_else(|| fault("generic token storage overflow"))?;
    *bytes = bytes
        .checked_add(charge)
        .ok_or_else(|| fault("generic metadata exceeds the source recipe budget"))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| fault("generic token allocation failed"))?;
    for token in node
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .filter(|token| !token.kind().is_trivia())
    {
        let mut text = String::new();
        text.try_reserve_exact(token.text().len())
            .map_err(|_| fault("generic token text allocation failed"))?;
        text.push_str(token.text());
        result.push(TokenRecipe {
            kind: token.kind() as u16,
            text,
        });
    }
    Ok(result)
}

/// An associated function's own generics followed by its owner's: an impl's
/// parameters (`impl<T> W<T>`), or a trait's parameters and its implicit
/// `Self`, as rustc's generics of an associated item. Own parameters keep
/// their ordinals, so explicit arguments still address them.
pub(crate) fn owner_generics(
    function: &ast::Fn,
    generics: Vec<DefinitionParameter>,
) -> Result<Vec<DefinitionParameter>, Diagnostic> {
    associated_owner_generics(function.syntax(), generics)
}
/// Only the immediate associated container contributes a binder. A local
/// item inside a function does not capture the function's generic parameters.
fn associated_owner(declaration: &SyntaxNode) -> Option<SyntaxNode> {
    declaration
        .parent()
        .and_then(|items| items.parent())
        .filter(|owner| ast::Impl::can_cast(owner.kind()) || ast::Trait::can_cast(owner.kind()))
}
fn associated_owner_generics(
    declaration: &SyntaxNode,
    generics: Vec<DefinitionParameter>,
) -> Result<Vec<DefinitionParameter>, Diagnostic> {
    associated_generics_with_layout(declaration, generics).map(|(generics, _)| generics)
}
fn associated_generics_with_layout(
    declaration: &SyntaxNode,
    mut generics: Vec<DefinitionParameter>,
) -> Result<(Vec<DefinitionParameter>, GenericSourceLayout), Diagnostic> {
    let mut layout = GenericSourceLayout {
        own: u32::try_from(generics.len())
            .map_err(|_| fault("generic parameter index overflow"))?,
        inherited: 0,
        implicit_self: false,
    };
    let Some(owner) = associated_owner(declaration) else {
        return Ok((generics, layout));
    };
    let (inherited, implicit_self) = if let Some(implementation) = ast::Impl::cast(owner.clone()) {
        (
            capture_generics(implementation.generic_param_list())?,
            false,
        )
    } else if let Some(definition) = ast::Trait::cast(owner) {
        (capture_generics(definition.generic_param_list())?, true)
    } else {
        return Ok((generics, layout));
    };
    layout.inherited =
        u32::try_from(inherited.len()).map_err(|_| fault("generic parameter index overflow"))?;
    layout.implicit_self = implicit_self;
    for mut parameter in inherited {
        parameter.ordinal =
            u32::try_from(generics.len()).map_err(|_| fault("generic parameter index overflow"))?;
        generics.push(parameter);
    }
    if implicit_self {
        generics.push(DefinitionParameter {
            ordinal: u32::try_from(generics.len())
                .map_err(|_| fault("generic parameter index overflow"))?,
            kind: GenericKind::Type,
            name: "Self".into(),
            bounds: Vec::new(),
            default: Vec::new(),
            const_type: Vec::new(),
        });
    }
    Ok((generics, layout))
}
pub(crate) fn capture_generics(
    list: Option<ast::GenericParamList>,
) -> Result<Vec<DefinitionParameter>, Diagnostic> {
    let mut generics = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(list) = list {
        let count = list.generic_params().count();
        let mut bytes = count
            .checked_mul(std::mem::size_of::<DefinitionParameter>())
            .ok_or_else(|| fault("generic parameter storage exceeds the source recipe budget"))?;
        u32::try_from(count).map_err(|_| fault("generic parameter index overflow"))?;
        generics
            .try_reserve_exact(count)
            .map_err(|_| fault("generic parameter allocation failed"))?;
        for (ordinal, parameter) in list.generic_params().enumerate() {
            let (kind, name, bounds, default, const_type) = match parameter {
                ast::GenericParam::TypeParam(p) => (
                    GenericKind::Type,
                    p.name().map(|n| n.text().to_string()),
                    generic_tokens(p.type_bound_list().map(|n| n.syntax().clone()), &mut bytes)?,
                    generic_tokens(p.default_type().map(|n| n.syntax().clone()), &mut bytes)?,
                    Vec::new(),
                ),
                ast::GenericParam::LifetimeParam(p) => (
                    GenericKind::Lifetime,
                    p.lifetime().map(|n| n.syntax().text().to_string()),
                    generic_tokens(p.type_bound_list().map(|n| n.syntax().clone()), &mut bytes)?,
                    Vec::new(),
                    Vec::new(),
                ),
                ast::GenericParam::ConstParam(p) => (
                    GenericKind::Const,
                    p.name().map(|n| n.text().to_string()),
                    Vec::new(),
                    generic_tokens(p.default_val().map(|n| n.syntax().clone()), &mut bytes)?,
                    generic_tokens(p.ty().map(|n| n.syntax().clone()), &mut bytes)?,
                ),
            };
            let name = name.ok_or_else(|| fault("generic parameter lacks its declared name"))?;
            let name = if kind == GenericKind::Lifetime {
                canonical_lifetime(&name).ok_or_else(|| fault("invalid lifetime parameter name"))?
            } else {
                semantic_name(&name).ok_or_else(|| fault("invalid generic parameter name"))?
            };
            bytes = bytes
                .checked_add(name.capacity())
                .ok_or_else(|| fault("generic name storage exceeds the source recipe budget"))?;
            // Lifetimes occupy their own namespace; type/const parameter names
            // retain the existing common identity namespace and source ordinals.
            let mut indexed_name = String::new();
            indexed_name
                .try_reserve_exact(name.len())
                .map_err(|_| fault("generic name index allocation failed"))?;
            indexed_name.push_str(&name);
            if !names.insert((kind == GenericKind::Lifetime, indexed_name)) {
                return Err(fault("duplicate generic parameter identity"));
            }
            generics.push(DefinitionParameter {
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| fault("generic parameter index overflow"))?,
                kind,
                name,
                bounds,
                default,
                const_type,
            });
        }
    }
    Ok(generics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        source::snapshot::{CfgSet, SourceBlob},
    };

    #[test]
    fn rustc_initializer_parameters_require_actual_source_declaration() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-struct.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed
            .outline(Digest([154; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let y = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "y")
            .unwrap();
        let x = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "x")
            .unwrap();
        let context = DeclarationContext {
            scope: y.scope.clone(),
            range: y.declaration_span,
        };
        let body = y.body.as_ref().unwrap();
        assert!(parsed
            .initializer_parameters(&y.header, &context, body)
            .unwrap()
            .is_empty());
        assert_eq!(
            parsed
                .initializer_parameters(&x.header, &context, body)
                .unwrap_err()
                .code,
            "LRR-E-CALL-SIGNATURE"
        );
        let main = outline
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        let main_context = DeclarationContext {
            scope: main.scope.clone(),
            range: main.declaration_span,
        };
        assert_eq!(
            parsed
                .initializer_parameters(&main.header, &main_context, main.body.as_ref().unwrap())
                .unwrap_err()
                .code,
            "LRR-E-CALL-SIGNATURE"
        );
        // Only checked declaration metadata: no function signature, constant
        // evaluator, import settlement, macro expansion or guest main execution.
    }

    #[test]
    fn rustc_associated_initializer_binders_retain_real_owner_parameters() {
        const GENERIC: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/associated-consts/associated-const-type-parameters.rs"
        );
        const SELF: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/associated-consts/associated-const-self-type.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            for original in [GENERIC, SELF] {
                let mut profile = ProfileIdentity::current();
                profile.edition = edition;
                let parsed = ParsedFile::parse(
                    &SourceBlob::new(original.as_bytes().to_vec()).unwrap(),
                    profile,
                )
                .unwrap();
                let outline = parsed
                    .outline(Digest([155; 32]), vec![], &CfgSet::guest_default())
                    .unwrap();
                let mut constants = 0;
                let mut inherited = 0;
                let mut trait_self = 0;
                for item in &outline.items {
                    if item.def_key.item_kind != crate::source::manifest::ItemKind::Constant {
                        continue;
                    }
                    constants += 1;
                    let context = DeclarationContext {
                        scope: item.scope.clone(),
                        range: item.declaration_span,
                    };
                    let identity = item.def_key.fingerprint();
                    let signature = parsed
                        .function_signature(identity, &item.header, &context, item.body.as_ref())
                        .unwrap();
                    assert!(signature.parameters.is_empty());
                    assert_eq!(signature.scope, item.scope);
                    let declaration = parsed
                        .syntax_at(item.declaration_span, |node| {
                            ast::Const::can_cast(node.kind())
                        })
                        .unwrap()
                        .unwrap();
                    let owner = associated_owner(&declaration).unwrap();
                    if ast::Trait::can_cast(owner.kind()) {
                        trait_self += 1;
                        assert_eq!(signature.generics.len(), 1);
                        assert_eq!(signature.generics[0].name, "Self");
                        assert_eq!(signature.generics[0].kind, GenericKind::Type);
                        assert_eq!(signature.generics[0].ordinal, 0);
                        if original == SELF {
                            assert_eq!(
                                signature.result_symbolic,
                                Some(crate::traits::term::TyTerm::Param(
                                    crate::traits::term::ParamKey {
                                        definition: identity,
                                        ordinal: 0,
                                    }
                                ))
                            );
                        }
                    } else if owner
                        .children()
                        .any(|node| ast::GenericParamList::can_cast(node.kind()))
                    {
                        inherited += 1;
                        assert_eq!(signature.generics.len(), 1);
                        let parameter = &signature.generics[0];
                        assert_eq!(
                            (parameter.ordinal, parameter.kind, parameter.name.as_str()),
                            (0, GenericKind::Type, "T")
                        );
                        assert_eq!(
                            parameter
                                .bounds
                                .iter()
                                .map(|token| token.text.as_str())
                                .collect::<Vec<_>>(),
                            ["Foo"]
                        );
                    } else {
                        assert!(signature.generics.is_empty());
                    }
                    if let Some(body) = &item.body {
                        assert_eq!(
                            parsed
                                .initializer_parameters(&item.header, &context, body)
                                .unwrap(),
                            signature.generics
                        );
                    }
                }
                assert_eq!(
                    (constants, inherited, trait_self),
                    if original == GENERIC {
                        (5, 1, 2)
                    } else {
                        (2, 0, 1)
                    }
                );
                // Declaration metadata only: no body index/lowering, selected
                // guest main, evaluator, instantiated packet or cache evidence.
            }
        }
    }

    #[test]
    fn rustc_duplicate_const_parameter_retains_identity_and_tokens() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/const-generics/defaults/self-referential.rs"
        );
        let parsed = ParsedFile::parse(
            &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let tree = parsed.parsed.tree();
        let list = tree
            .syntax()
            .descendants()
            .find_map(ast::GenericParamList::cast)
            .unwrap();
        let first = list.generic_params().next().unwrap();
        let ast::GenericParam::ConstParam(first) = first else {
            panic!("original trait parameter is not const")
        };
        let ty = first.ty().unwrap();
        let mut bytes = 0;
        let captured = generic_tokens(Some(ty.syntax().clone()), &mut bytes).unwrap();
        assert_eq!(captured, tokens(ty.syntax(), None));
        assert!(bytes > 0);
        for _ in 0..2 {
            let error = capture_generics(Some(list.clone())).unwrap_err();
            assert_eq!(error.code, "LRR-E-CALL-SIGNATURE");
            assert_eq!(error.message, "duplicate generic parameter identity");
        }
        // Original duplicate identity only; trait selection, compiler stderr
        // parity and complete original compile-fail are not exercised here.
    }
}
