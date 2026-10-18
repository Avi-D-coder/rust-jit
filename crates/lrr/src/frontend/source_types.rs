//! Common declaration-context named Type preparation; never infer from a value.
use super::{
    names::NameNamespaces,
    signature_names::{SignatureName, SignatureNames},
    type_aliases::{AliasNormalizer, AliasTarget},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    runtime::{
        cell::GuestStore,
        shape::{PrimitiveIdentity, TypeId, TypeKey},
    },
    source::{
        changes::PreparedSourceView,
        manifest::{DefKey, ItemKind, Namespace},
    },
    syntax::{
        closed_reference_signatures::ClosedReferenceTypeSyntax, signatures::FunctionSignature,
    },
    traits::term::TyTerm,
};

/// Selected source type/schema from the current owning Type lookup. Call-boundary
/// currentness remains a separate proof; this does not authorize an arbitrary
/// graph or a public runtime descriptor as a source Type.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SourceType {
    source: TyTerm,
    key: TypeKey,
}
pub(crate) enum NamedTypePreparation {
    Ready(SourceType),
    Parameter(crate::traits::term::ParamKey),
    Alias(AliasTarget),
    Name(SignatureName),
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
fn metadata(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-METADATA-LIMIT", message.into())
}
impl SourceType {
    /// Classification has already established the builtin through the ordinary
    /// owning Type lookup. This is not a spelling-to-runtime-type fallback.
    pub(super) fn selected_primitive(
        primitive: crate::ir::op::PrimitiveType,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            source: TyTerm::Primitive(primitive),
            key: TypeKey::Primitive(PrimitiveIdentity::from_recipe(primitive)?),
        })
    }
    pub(crate) fn primitive(&self) -> Option<crate::ir::op::PrimitiveType> {
        match self.source {
            TyTerm::Primitive(primitive) => Some(primitive),
            _ => None,
        }
    }
    /// Called only after the ordinary owning Type service selects builtin str
    /// or an alias genuinely normalized to it. Descriptor16 is already unsized
    /// and NotCopy; this proof does not publish a standalone owning value.
    pub(super) fn selected_str() -> Result<Self, Diagnostic> {
        Ok(Self {
            source: TyTerm::Str,
            key: TypeKey::Primitive(PrimitiveIdentity::new(TypeId(16))?),
        })
    }
    pub(crate) fn source(&self) -> &TyTerm {
        &self.source
    }
    pub(crate) fn same_type(&self, other: &Self) -> bool {
        self.source == other.source && self.key == other.key
    }
    pub(crate) fn find_descriptor(&self, store: &GuestStore) -> Option<TypeId> {
        store.find_type(&self.key)
    }
    pub(crate) fn materialize(&self, store: &mut GuestStore) -> Result<TypeId, Diagnostic> {
        if let Some(id) = self.find_descriptor(store) {
            return Ok(id);
        }
        store.intern_type(self.key.clone())
    }
    pub(crate) fn require_descriptor(
        &self,
        store: &GuestStore,
        actual: TypeId,
    ) -> Result<(), Diagnostic> {
        if store.type_descriptor(actual)?.key.as_ref() != &self.key {
            return Err(fault(
                "selected source Type/schema differs from actual descriptor",
            ));
        }
        Ok(())
    }
    pub(crate) fn heap_bytes(&self) -> Result<usize, Diagnostic> {
        self.allocated_bytes()?
            .checked_sub(std::mem::size_of::<Self>())
            .ok_or_else(|| metadata("source Type size underflow"))
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.source
                    .heap_bytes()
                    .ok_or_else(|| metadata("source Type size overflow"))?,
            )
            .ok_or_else(|| metadata("source Type size overflow"))?;
        if let TypeKey::Nominal { definition, .. } = &self.key {
            bytes = definition.module_path.iter().try_fold(
                bytes
                    .checked_add(definition.declared_name.capacity())
                    .and_then(|n| {
                        definition
                            .module_path
                            .capacity()
                            .checked_mul(std::mem::size_of::<String>())
                            .and_then(|m| n.checked_add(m))
                    })
                    .ok_or_else(|| metadata("source nominal Type size overflow"))?,
                |bytes, part| {
                    bytes
                        .checked_add(part.capacity())
                        .ok_or_else(|| metadata("source nominal Type size overflow"))
                },
            )?;
        }
        Ok(bytes)
    }
}
impl StableEncode for SourceType {
    const DOMAIN: &'static str = "lrr/v1/source-selected-concrete-type";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        e.record(&self.source);
        e.record(&self.key);
    }
}

/// Shared by ordinary named parameters and named reference pointees. This uses
/// exactly the existing Type name/alias/schema services; it does not parse text,
/// force executable code, solve a trait goal or inspect a runtime descriptor.
pub(crate) fn prepare_named(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
) -> QueryPoll<NamedTypePreparation> {
    prepare_named_at(db, graph, sources, signature, name, None)
}

/// The same Type service with the exact parser-owned source occurrence. This
/// narrows origin replay; it does not change classification or Type semantics.
pub(crate) fn prepare_reference_pointee(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    syntax: &ClosedReferenceTypeSyntax,
) -> QueryPoll<NamedTypePreparation> {
    if let Err(error) = syntax.require(signature) {
        return QueryPoll::Ready(Err(error));
    }
    prepare_named_at(db, graph, sources, signature, syntax.target(), Some(syntax))
}
fn prepare_named_at(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
    reference: Option<&ClosedReferenceTypeSyntax>,
) -> QueryPoll<NamedTypePreparation> {
    let selected = match resolve_named_at(db, graph, sources, signature, name, reference) {
        QueryPoll::Ready(Ok(selected)) => selected,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    prepare_selected(db, graph, sources, selected)
}

/// The common current Type lookup/classification used before optional alias or
/// nominal schema preparation. A result header can retain its own alias trace.
pub(crate) fn resolve_named(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
) -> QueryPoll<SignatureName> {
    resolve_named_at(db, graph, sources, signature, name, None)
}
fn resolve_named_at(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
    reference: Option<&ClosedReferenceTypeSyntax>,
) -> QueryPoll<SignatureName> {
    // A completed receipt supplements the actual source catalogue. It does
    // not authorize substituting an unrelated graph/source universe.
    if let Err(error) = AliasNormalizer::new(graph, sources) {
        return QueryPoll::Ready(Err(error));
    }
    // An impl introduces no Type namespace: written names in its associated
    // functions resolve in the enclosing module, and `Self` names the impl's
    // self type.
    if let Some(owner) = signature.scope.owner {
        let generic = signature.generics.iter().any(|parameter| {
            parameter.kind == crate::syntax::signatures::GenericKind::Type && parameter.name == name
        });
        if !generic {
            // A macro-generated impl's `Self` names the self type written in
            // its own generated unit's header.
            let in_sources = sources.iter().any(|source| {
                source.outline().items.iter().any(|item| {
                    item.def_key.item_kind == ItemKind::Impl
                        && item.def_key.enclosing_impl == Some(owner)
                })
            });
            let generated = if name == "Self" && !in_sources {
                let parse = match db
                    .input(&InputKey::DefinitionSource(signature.definition))
                    .as_deref()
                {
                    Some(InputValue::DefinitionSource(parse)) => Some(parse.clone()),
                    _ => None,
                };
                match parse.and_then(|parse| db.generated_declaration_unit(&parse)) {
                    Some(unit) => match unit.outline() {
                        Ok(outline) => Some(outline),
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    },
                    None => None,
                }
            } else {
                None
            };
            return QueryPoll::Ready(impl_owned_name(
                graph,
                sources,
                generated.as_deref(),
                signature,
                owner,
                name,
            ));
        }
    }
    let names = match SignatureNames::from_database(db, graph, signature) {
        Ok(names) => names,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    match names.unqualified_local(name) {
        Ok(Some(local)) => return QueryPoll::Ready(Ok(local)),
        Ok(None) => {}
        Err(error) => return QueryPoll::Ready(Err(error)),
    }
    let Some(source) = db.input(&InputKey::DefinitionSource(signature.definition)) else {
        return QueryPoll::Ready(Err(fault("named Type declaration source absent")));
    };
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        return QueryPoll::Ready(Err(fault("named Type declaration source variant differs")));
    };
    if reference.is_some_and(|reference| reference.parse() != parse) {
        return QueryPoll::Ready(Err(fault("named Type source occurrence is not current")));
    }
    let lookup = if db.locates_generated_declaration(parse) {
        if let Some(reference) = reference {
            super::call_contracts::generated_result_namespace::lookup_name(
                db,
                signature,
                parse,
                name,
                Some(reference),
            )
        } else {
            match super::call_contracts::generated_result_namespace::lookup_prepared_name(
                db, signature, parse, name,
            ) {
                QueryPoll::Ready(result) => result,
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            }
        }
    } else {
        let stamps = match db.lexical_namespace_stamps() {
            Ok(stamps) => stamps,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        super::type_name_namespace::lookup_name(
            db,
            super::type_name_namespace::SourceTypeOwner {
                definition: signature.definition,
                scope: &signature.scope,
                header: &signature.header,
            },
            sources,
            &stamps,
            name,
        )
    };
    match lookup {
        Ok(lookup) => QueryPoll::Ready(names.unqualified_with_lookup(name, lookup)),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
/// A written composite type (tuple, array, parenthesized, never) whose leaves
/// are single names resolved by the same declaration-context Type service.
/// `None` when some part needs a further source integration.
pub(crate) fn resolve_written_type(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    tokens: &[crate::syntax::ast_adapter::TokenRecipe],
) -> QueryPoll<Option<TyTerm>> {
    use ra_ap_syntax::ast::{self, HasModuleItem};
    let text = crate::syntax::ast_adapter::token_text(tokens);
    let parser = match signature.header.profile.edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        _ => ra_ap_syntax::Edition::Edition2024,
    };
    let parsed = ast::SourceFile::parse(&format!("type LrrWritten = {text};"), parser);
    if !parsed.errors().is_empty() {
        return QueryPoll::Ready(Ok(None));
    }
    let Some(ast::Item::TypeAlias(alias)) = parsed.tree().items().next() else {
        return QueryPoll::Ready(Ok(None));
    };
    let Some(ty) = alias.ty() else {
        return QueryPoll::Ready(Ok(None));
    };
    resolve_actual_type(
        db,
        graph,
        sources,
        &WrittenTypeDeclaration::function(signature),
        &ty,
    )
}

/// The actual declaration facts needed by the common Type walk. This is a
/// borrowed header context, not an invented function/signature/body.
pub(super) struct WrittenTypeDeclaration<'a> {
    definition: crate::core::digest::Digest,
    scope: &'a crate::syntax::outline::ScopeKey,
    header: &'a crate::syntax::outline::HeaderRecipe,
    generics: &'a [crate::syntax::signatures::DefinitionParameter],
    predicates: &'a [crate::syntax::ast_adapter::TokenRecipe],
    function: Option<&'a FunctionSignature>,
    syntax: Option<&'a ra_ap_syntax::SyntaxNode>,
}
impl<'a> WrittenTypeDeclaration<'a> {
    fn function(signature: &'a FunctionSignature) -> Self {
        Self {
            definition: signature.definition,
            scope: &signature.scope,
            header: &signature.header,
            generics: &signature.generics,
            predicates: &signature.predicates,
            function: Some(signature),
            syntax: None,
        }
    }
    pub(super) fn nominal(
        schema: &'a super::types::NominalSchema,
        syntax: &'a ra_ap_syntax::SyntaxNode,
    ) -> Self {
        Self {
            definition: schema.definition,
            scope: &schema.scope,
            header: &schema.header,
            generics: &schema.generics,
            predicates: &[],
            function: None,
            syntax: Some(syntax),
        }
    }
    fn resolve_name(
        &self,
        db: &mut Database,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        name: &str,
    ) -> QueryPoll<SignatureName> {
        if let Some(signature) = self.function {
            return resolve_named(db, graph, sources, signature, name);
        }
        super::type_name_namespace::resolve_declaration_name(
            db,
            super::type_name_namespace::SourceTypeOwner {
                definition: self.definition,
                scope: self.scope,
                header: self.header,
            },
            self.generics,
            sources,
            name,
        )
    }
}
/// Resolve an actual retained Type occurrence through the same walk as legacy
/// written signature Types. The caller proves its declaration/span/currentness.
pub(super) fn resolve_actual_type(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &WrittenTypeDeclaration<'_>,
    ty: &ra_ap_syntax::ast::Type,
) -> QueryPoll<Option<TyTerm>> {
    fn walk(
        db: &mut Database,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        signature: &WrittenTypeDeclaration<'_>,
        ty: &ra_ap_syntax::ast::Type,
    ) -> QueryPoll<Option<TyTerm>> {
        use ra_ap_syntax::{AstNode, ast};
        match ty {
            ast::Type::TupleType(tuple) => {
                let mut fields = Vec::new();
                for field in tuple.fields() {
                    match walk(db, graph, sources, signature, &field) {
                        QueryPoll::Ready(Ok(Some(term))) => fields.push(term),
                        other => return other,
                    }
                }
                // `()` is the unit primitive, as in symbolic signature
                // lowering and every runtime descriptor's source term.
                if fields.is_empty() {
                    return QueryPoll::Ready(Ok(Some(TyTerm::Primitive(
                        crate::ir::op::PrimitiveType::Unit,
                    ))));
                }
                QueryPoll::Ready(Ok(Some(TyTerm::Tuple(fields))))
            }
            ast::Type::ParenType(paren) => match paren.ty() {
                Some(inner) => walk(db, graph, sources, signature, &inner),
                None => QueryPoll::Ready(Ok(None)),
            },
            ast::Type::NeverType(_) => QueryPoll::Ready(Ok(Some(TyTerm::Never))),
            // `dyn Trait` (and 2015's bare trait object): the object schema is
            // the principal trait's identity; values keep their concrete
            // runtime type and methods dispatch on it.
            // `impl Trait` in argument position is an anonymous generic
            // parameter: like a trait object it accepts the reached concrete
            // value, which keeps its own type.
            ast::Type::ImplTraitType(_) | ast::Type::DynTraitType(_) => {
                let bounds = match ty {
                    ast::Type::ImplTraitType(object) => object.type_bound_list(),
                    ast::Type::DynTraitType(object) => object.type_bound_list(),
                    _ => None,
                };
                let principal = bounds.and_then(|bounds| {
                    bounds.bounds().find_map(|bound| match bound.ty() {
                        Some(ast::Type::PathType(path)) => path.path(),
                        _ => None,
                    })
                });
                let Some((segments, absolute)) =
                    principal.and_then(super::member_index::path_segments)
                else {
                    return QueryPoll::Ready(Ok(None));
                };
                let scope = crate::syntax::outline::ScopeKey {
                    owner: None,
                    ..(*signature.scope).clone()
                };
                match graph.lookup_source_path(
                    Namespace::Type,
                    &segments,
                    absolute,
                    &scope,
                    signature.header.profile.edition,
                ) {
                    Ok(crate::frontend::names::NameLookup::Defined(definition)) => {
                        QueryPoll::Ready(Ok(Some(TyTerm::Dyn(crate::traits::term::DynSchemaKey(
                            definition.identity,
                        )))))
                    }
                    // The closure traits are language items of the prelude;
                    // without a source declaration their object schema is
                    // the builtin trait's own identity. A closure or function
                    // item value keeps its concrete callable type.
                    Ok(crate::frontend::names::NameLookup::Absent)
                        if !absolute
                            && matches!(segments.as_slice(),
                                [crate::syntax::imports::ImportSegment::Name(name)]
                                    if matches!(name.as_str(), "Fn" | "FnMut" | "FnOnce")) =>
                    {
                        let crate::syntax::imports::ImportSegment::Name(name) = &segments[0] else {
                            return QueryPoll::Ready(Ok(None));
                        };
                        QueryPoll::Ready(Ok(Some(TyTerm::Dyn(crate::traits::term::DynSchemaKey(
                            crate::core::digest::Digest::of_bytes(
                                format!("lrr/v1/builtin-closure-trait/{name}").as_bytes(),
                            ),
                        )))))
                    }
                    Ok(_) => QueryPoll::Ready(Ok(None)),
                    Err(error) => QueryPoll::Ready(Err(error)),
                }
            }
            // Regions are not runtime data; the interpreter binds through
            // erased regions after rustc's own borrow checking.
            ast::Type::RefType(reference) => match reference.ty() {
                Some(target) => match walk(db, graph, sources, signature, &target) {
                    QueryPoll::Ready(Ok(Some(target))) => QueryPoll::Ready(Ok(Some(TyTerm::Ref(
                        crate::traits::term::RegionTerm::Static,
                        reference.mut_token().is_some(),
                        Box::new(target),
                    )))),
                    other => other,
                },
                None => QueryPoll::Ready(Ok(None)),
            },
            ast::Type::SliceType(slice) => match slice.ty() {
                Some(element) => match walk(db, graph, sources, signature, &element) {
                    QueryPoll::Ready(Ok(Some(element))) => {
                        QueryPoll::Ready(Ok(Some(TyTerm::Slice(Box::new(element)))))
                    }
                    other => other,
                },
                None => QueryPoll::Ready(Ok(None)),
            },
            // `*const T` / `*mut T` (E-R5-008): the reserved-identity nominal.
            ast::Type::PtrType(pointer) => match pointer.ty() {
                Some(pointee) => match walk(db, graph, sources, signature, &pointee) {
                    QueryPoll::Ready(Ok(Some(pointee))) => {
                        QueryPoll::Ready(Ok(Some(crate::traits::pointer_terms::raw_pointer(
                            pointer.mut_token().is_some(),
                            pointee,
                        ))))
                    }
                    other => other,
                },
                None => QueryPoll::Ready(Ok(None)),
            },
            // `fn(A) -> R` (any ABI; `for<'a>` binders name regions only).
            ast::Type::FnPtrType(function) => {
                let Some(list) = function.param_list() else {
                    return QueryPoll::Ready(Ok(None));
                };
                let mut parameters = Vec::new();
                for parameter in list.params() {
                    let Some(ty) = parameter.ty() else {
                        return QueryPoll::Ready(Ok(None));
                    };
                    match walk(db, graph, sources, signature, &ty) {
                        QueryPoll::Ready(Ok(Some(term))) => parameters.push(term),
                        other => return other,
                    }
                }
                let result = match function.ret_type().and_then(|ret| ret.ty()) {
                    Some(ty) => match walk(db, graph, sources, signature, &ty) {
                        QueryPoll::Ready(Ok(Some(term))) => term,
                        other => return other,
                    },
                    None => TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit),
                };
                QueryPoll::Ready(Ok(Some(crate::traits::pointer_terms::fn_pointer(
                    parameters, result,
                ))))
            }
            ast::Type::ArrayType(array) => {
                let Some(element) = array.ty() else {
                    return QueryPoll::Ready(Ok(None));
                };
                let length = match array.const_arg().and_then(|arg| arg.expr()) {
                    Some(ast::Expr::Literal(length)) => length,
                    // `[T; N]` with `N` a const parameter of the signature
                    // (an impl's `const N: usize`): its length is that
                    // parameter, bound from the reached array at a call.
                    Some(ast::Expr::PathExpr(path)) => {
                        let parameter = path
                            .path()
                            .filter(|path| path.qualifier().is_none())
                            .and_then(|path| path.segment())
                            .and_then(|segment| segment.name_ref())
                            .and_then(|name| {
                                signature.generics.iter().find(|parameter| {
                                    parameter.kind == crate::syntax::signatures::GenericKind::Const
                                        && parameter.name == name.text()
                                })
                            });
                        let Some(parameter) = parameter else {
                            return QueryPoll::Ready(Ok(None));
                        };
                        let length =
                            crate::traits::term::ConstTerm::Param(crate::traits::term::ParamKey {
                                definition: signature.definition,
                                ordinal: parameter.ordinal,
                            });
                        return match walk(db, graph, sources, signature, &element) {
                            QueryPoll::Ready(Ok(Some(element))) => {
                                QueryPoll::Ready(Ok(Some(TyTerm::Array(Box::new(element), length))))
                            }
                            other => other,
                        };
                    }
                    _ => return QueryPoll::Ready(Ok(None)),
                };
                let digits: String = length
                    .syntax()
                    .text()
                    .to_string()
                    .trim_end_matches("usize")
                    .chars()
                    .filter(|c| *c != '_')
                    .collect();
                let Ok(len) = digits.parse::<u64>() else {
                    return QueryPoll::Ready(Ok(None));
                };
                match walk(db, graph, sources, signature, &element) {
                    QueryPoll::Ready(Ok(Some(element))) => {
                        QueryPoll::Ready(Ok(Some(TyTerm::Array(
                            Box::new(element),
                            crate::traits::term::ConstTerm::Value(len),
                        ))))
                    }
                    other => other,
                }
            }
            ast::Type::PathType(path) => {
                let Some(path) = path.path() else {
                    return QueryPoll::Ready(Ok(None));
                };
                if let Some(qualifier) = path.qualifier() {
                    // A module-qualified nominal (`m::W<T>`, `crate::ops::Range<A>`)
                    // names a Type item through its whole path; otherwise the
                    // qualifier is a type (`T::Item`, `Self::Item`).
                    match module_path_type(db, graph, sources, signature, &path) {
                        QueryPoll::Ready(Ok(Some(term))) => {
                            return QueryPoll::Ready(Ok(Some(term)));
                        }
                        QueryPoll::Ready(Ok(None)) => {}
                        other => return other,
                    }
                    return shorthand_projection(db, graph, sources, signature, &qualifier, &path);
                }
                let Some(segment) = path.segment() else {
                    return QueryPoll::Ready(Ok(None));
                };
                // Written generic arguments of a nominal (`W<T>`); lifetime
                // arguments are regions and carry no runtime type.
                let mut arguments = Vec::new();
                let generic = ra_ap_syntax::ast::HasGenericArgs::generic_arg_list(&segment);
                if let Some(list) = &generic {
                    for argument in list.generic_args() {
                        match argument {
                            ast::GenericArg::TypeArg(argument) => {
                                let Some(ty) = argument.ty() else {
                                    return QueryPoll::Ready(Ok(None));
                                };
                                match walk(db, graph, sources, signature, &ty) {
                                    QueryPoll::Ready(Ok(Some(term))) => {
                                        arguments.push(crate::traits::term::GenericArg::Type(term))
                                    }
                                    other => return other,
                                }
                            }
                            ast::GenericArg::LifetimeArg(_) => {}
                            _ => return QueryPoll::Ready(Ok(None)),
                        }
                    }
                }
                // `Self` of an impl: its written self type, whose generic
                // names are the function's inherited impl parameters.
                if segment.kind() == Some(ast::PathSegmentKind::SelfTypeKw) {
                    if signature.generics.iter().any(|p| {
                        p.kind == crate::syntax::signatures::GenericKind::Type && p.name == "Self"
                    }) {
                        return match signature.resolve_name(db, graph, sources, "Self") {
                            QueryPoll::Ready(Ok(SignatureName::Parameter(key))) => {
                                QueryPoll::Ready(Ok(Some(TyTerm::Param(key))))
                            }
                            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(None)),
                            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                        };
                    }
                    let Some(owner) = signature.scope.owner else {
                        return QueryPoll::Ready(Ok(None));
                    };
                    let Some(self_ty) =
                        impl_self_type(sources, owner, signature.header.profile.edition).or_else(
                            || {
                                generated_impl_self_type(
                                    db,
                                    signature.definition,
                                    owner,
                                    signature.header.profile.edition,
                                )
                            },
                        )
                    else {
                        return QueryPoll::Ready(Ok(None));
                    };
                    return walk(db, graph, sources, signature, &self_ty);
                }
                let Some(name_ref) = segment.name_ref() else {
                    return QueryPoll::Ready(Ok(None));
                };
                let name = name_ref.text().to_string();
                let selected = match signature.resolve_name(db, graph, sources, &name) {
                    QueryPoll::Ready(Ok(selected)) => selected,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                QueryPoll::Ready(Ok(match selected {
                    SignatureName::Primitive(primitive) => Some(TyTerm::Primitive(primitive)),
                    SignatureName::Str => Some(TyTerm::Str),
                    SignatureName::Parameter(key) => Some(TyTerm::Param(key)),
                    SignatureName::Source(found) => {
                        let normalizer = match AliasNormalizer::new(graph, sources) {
                            Ok(normalizer) => normalizer,
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        };
                        match normalizer.normalize(db, found.identity) {
                            QueryPoll::Ready(Ok(AliasTarget::Type(term))) => Some(term),
                            QueryPoll::Ready(Ok(AliasTarget::Str)) => Some(TyTerm::Str),
                            QueryPoll::Ready(Ok(AliasTarget::Nominal(nominal))) => {
                                Some(TyTerm::Nominal(nominal, std::mem::take(&mut arguments)))
                            }
                            QueryPoll::Ready(Ok(AliasTarget::Deferred { definition, .. })) => {
                                return deferred_alias_type(
                                    db, graph, sources, definition, &arguments,
                                );
                            }
                            QueryPoll::Ready(Ok(_)) => None,
                            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                        }
                    }
                    _ => None,
                }))
            }
            _ => QueryPoll::Ready(Ok(None)),
        }
    }
    /// A multi-segment path that resolves, as written, to a Type item of the
    /// signature's module scope: its nominal term with the last segment's
    /// written generic arguments. `None` when the path names no such item.
    fn module_path_type(
        db: &mut Database,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        signature: &WrittenTypeDeclaration<'_>,
        path: &ra_ap_syntax::ast::Path,
    ) -> QueryPoll<Option<TyTerm>> {
        use ra_ap_syntax::ast;
        let Some((segments, absolute)) = super::member_index::path_segments(path.clone()) else {
            return QueryPoll::Ready(Ok(None));
        };
        let scope = crate::syntax::outline::ScopeKey {
            owner: None,
            ..(*signature.scope).clone()
        };
        let found = match graph.lookup_source_path(
            Namespace::Type,
            &segments,
            absolute,
            &scope,
            signature.header.profile.edition,
        ) {
            Ok(crate::frontend::names::NameLookup::Defined(found)) => found,
            Ok(_) => return QueryPoll::Ready(Ok(None)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut arguments = Vec::new();
        if let Some(list) = path
            .segment()
            .and_then(|segment| ra_ap_syntax::ast::HasGenericArgs::generic_arg_list(&segment))
        {
            for argument in list.generic_args() {
                match argument {
                    ast::GenericArg::TypeArg(argument) => {
                        let Some(ty) = argument.ty() else {
                            return QueryPoll::Ready(Ok(None));
                        };
                        match walk(db, graph, sources, signature, &ty) {
                            QueryPoll::Ready(Ok(Some(term))) => {
                                arguments.push(crate::traits::term::GenericArg::Type(term))
                            }
                            other => return other,
                        }
                    }
                    ast::GenericArg::LifetimeArg(_) => {}
                    _ => return QueryPoll::Ready(Ok(None)),
                }
            }
        }
        let normalizer = match AliasNormalizer::new(graph, sources) {
            Ok(normalizer) => normalizer,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match normalizer.normalize(db, found.identity) {
            QueryPoll::Ready(Ok(AliasTarget::Type(term))) => QueryPoll::Ready(Ok(Some(term))),
            QueryPoll::Ready(Ok(AliasTarget::Nominal(nominal))) => {
                QueryPoll::Ready(Ok(Some(TyTerm::Nominal(nominal, arguments))))
            }
            QueryPoll::Ready(Ok(AliasTarget::Deferred { definition, .. })) => {
                deferred_alias_type(db, graph, sources, definition, &arguments)
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// `P::Name` / `Self::Name`: a shorthand associated-type projection whose
    /// self type is the generic parameter (or impl self type) and whose trait
    /// is selected from that type's impls when it is known.
    fn shorthand_projection(
        db: &mut Database,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        signature: &WrittenTypeDeclaration<'_>,
        qualifier: &ra_ap_syntax::ast::Path,
        path: &ra_ap_syntax::ast::Path,
    ) -> QueryPoll<Option<TyTerm>> {
        use ra_ap_syntax::ast;
        let Some(segment) = path.segment() else {
            return QueryPoll::Ready(Ok(None));
        };
        if ra_ap_syntax::ast::HasGenericArgs::generic_arg_list(&segment).is_some()
            || qualifier.qualifier().is_some()
        {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(name) = segment.name_ref().map(|name| name.text().to_string()) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(base) = qualifier.segment() else {
            return QueryPoll::Ready(Ok(None));
        };
        let projection = |self_ty: TyTerm,
                          trait_key: crate::core::digest::Digest,
                          args: Vec<crate::traits::term::GenericArg>| {
            TyTerm::Projection(Box::new(crate::traits::term::ProjectionKey {
                trait_ref: crate::traits::term::TraitRef {
                    trait_key,
                    self_ty,
                    args,
                },
                item: crate::frontend::member_index::associated_name_key(&name),
                item_args: Vec::new(),
            }))
        };
        // `<Ty as Trait>::Name` (and `<Ty>::Name`): the written self type is
        // any Type this service resolves; the written trait, resolved in the
        // declaration's module like any trait path, selects among that
        // type's impls, and its written Type arguments (`TryFrom<T>`) select
        // among the impls of that trait.
        if let Some(ast::PathSegmentKind::Type {
            type_ref,
            trait_ref,
        }) = base.kind()
        {
            let Some(type_ref) = type_ref else {
                return QueryPoll::Ready(Ok(None));
            };
            let (trait_key, trait_args) = match trait_ref.and_then(|written| written.path()) {
                None => (crate::core::digest::Digest([0; 32]), Vec::new()),
                Some(written) => match projection_trait(db, graph, sources, signature, &written) {
                    QueryPoll::Ready(Ok(Some(selected))) => selected,
                    QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                },
            };
            return match walk(db, graph, sources, signature, &type_ref) {
                QueryPoll::Ready(Ok(Some(self_ty))) => {
                    QueryPoll::Ready(Ok(Some(projection(self_ty, trait_key, trait_args))))
                }
                other => other,
            };
        }
        if ra_ap_syntax::ast::HasGenericArgs::generic_arg_list(&base).is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        let base_name = if base.kind() == Some(ast::PathSegmentKind::SelfTypeKw) {
            "Self".to_string()
        } else {
            match base.name_ref() {
                Some(name) => name.text().to_string(),
                None => return QueryPoll::Ready(Ok(None)),
            }
        };
        let self_ty = match signature.resolve_name(db, graph, sources, &base_name) {
            QueryPoll::Ready(Ok(SignatureName::Parameter(key))) => TyTerm::Param(key),
            QueryPoll::Ready(Ok(_)) if base_name == "Self" => {
                let Some(owner) = signature.scope.owner else {
                    return QueryPoll::Ready(Ok(None));
                };
                let Some(written) =
                    impl_self_type(sources, owner, signature.header.profile.edition).or_else(
                        || {
                            generated_impl_self_type(
                                db,
                                signature.definition,
                                owner,
                                signature.header.profile.edition,
                            )
                        },
                    )
                else {
                    return QueryPoll::Ready(Ok(None));
                };
                match walk(db, graph, sources, signature, &written) {
                    QueryPoll::Ready(Ok(Some(term))) => term,
                    other => return other,
                }
            }
            QueryPoll::Ready(Ok(_)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let selected = match projection_bound(db, graph, sources, signature, &base_name, &name) {
            QueryPoll::Ready(Ok(selected)) => selected,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let (trait_key, args) =
            selected.unwrap_or((crate::core::digest::Digest([0; 32]), Vec::new()));
        QueryPoll::Ready(Ok(Some(projection(self_ty, trait_key, args))))
    }
    /// A trait path and its written Type arguments, shared by qualified
    /// projections and bounds supplying a shorthand projection.
    fn projection_trait(
        db: &mut Database,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        signature: &WrittenTypeDeclaration<'_>,
        path: &ra_ap_syntax::ast::Path,
    ) -> QueryPoll<
        Option<(
            crate::core::digest::Digest,
            Vec<crate::traits::term::GenericArg>,
        )>,
    > {
        use ra_ap_syntax::ast;
        let trait_key = match projection_trait_key(graph, signature, path) {
            Ok(Some(key)) => key,
            Ok(None) => return QueryPoll::Ready(Ok(None)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut arguments = Vec::new();
        if let Some(list) = path
            .segment()
            .and_then(|segment| ast::HasGenericArgs::generic_arg_list(&segment))
        {
            for argument in list.generic_args() {
                match argument {
                    ast::GenericArg::LifetimeArg(_) | ast::GenericArg::AssocTypeArg(_) => {}
                    ast::GenericArg::TypeArg(argument) => {
                        let Some(ty) = argument.ty() else {
                            return QueryPoll::Ready(Ok(None));
                        };
                        match walk(db, graph, sources, signature, &ty) {
                            QueryPoll::Ready(Ok(Some(term))) => {
                                arguments.push(crate::traits::term::GenericArg::Type(term))
                            }
                            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                        }
                    }
                    _ => return QueryPoll::Ready(Ok(None)),
                }
            }
        }
        QueryPoll::Ready(Ok(Some((trait_key, arguments))))
    }
    fn projection_trait_key(
        graph: &NameNamespaces,
        signature: &WrittenTypeDeclaration<'_>,
        path: &ra_ap_syntax::ast::Path,
    ) -> Result<Option<crate::core::digest::Digest>, Diagnostic> {
        let Some((segments, absolute)) = super::member_index::path_segments(path.clone()) else {
            return Ok(None);
        };
        let scope = crate::syntax::outline::ScopeKey {
            owner: None,
            ..(*signature.scope).clone()
        };
        match graph.lookup_source_path(
            Namespace::Type,
            &segments,
            absolute,
            &scope,
            signature.header.profile.edition,
        ) {
            Ok(crate::frontend::names::NameLookup::Defined(definition)) => {
                Ok(Some(definition.identity))
            }
            Ok(_) => Ok(Some(crate::core::digest::Digest([0; 32]))),
            Err(error) => Err(error),
        }
    }
    /// Select the actual bound declaring `item`, preserving its arguments.
    /// The existing trait-owned Type bucket supplies declaration evidence;
    /// unrelated bounds such as `Copy` cannot accidentally select a trait.
    fn projection_bound(
        db: &mut Database,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        signature: &WrittenTypeDeclaration<'_>,
        parameter: &str,
        item: &str,
    ) -> QueryPoll<
        Option<(
            crate::core::digest::Digest,
            Vec<crate::traits::term::GenericArg>,
        )>,
    > {
        use ra_ap_syntax::ast::{self, HasModuleItem, HasTypeBounds};
        let Some(parameter) = signature
            .generics
            .iter()
            .find(|generic| generic.name == parameter)
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut paths = Vec::new();
        if let Some(owner) = signature.syntax {
            use ra_ap_syntax::{AstNode, ast::HasName};
            let mut append = |list: Option<ast::TypeBoundList>| {
                for bound in list.into_iter().flat_map(|list| list.bounds()) {
                    if bound.question_mark_token().is_some() || bound.lifetime().is_some() {
                        continue;
                    }
                    if let Some(ast::Type::PathType(ty)) = bound.ty()
                        && let Some(path) = ty.path()
                    {
                        paths.push(path);
                    }
                }
            };
            if let Some(list) = owner.children().find_map(ast::GenericParamList::cast) {
                for actual in list
                    .generic_params()
                    .filter_map(|parameter| match parameter {
                        ast::GenericParam::TypeParam(parameter) => Some(parameter),
                        _ => None,
                    })
                    .filter(|actual| {
                        actual
                            .name()
                            .is_some_and(|name| name.text() == parameter.name)
                    })
                {
                    append(actual.type_bound_list());
                }
            }
            if let Some(clause) = owner.children().find_map(ast::WhereClause::cast) {
                for predicate in clause.predicates() {
                    if matches!(predicate.ty(), Some(ast::Type::PathType(ty))
                        if ty.path().is_some_and(|path| path.qualifier().is_none()
                            && path.segment().and_then(|segment| segment.name_ref())
                                .is_some_and(|name| name.text() == parameter.name)))
                    {
                        append(predicate.type_bound_list());
                    }
                }
            }
        } else {
            let mut lists = vec![parameter.bounds.as_slice()];
            let predicates = match signature.predicates.split_first() {
                Some((first, rest)) if first.text == "where" => rest,
                _ => &signature.predicates[..],
            };
            // Commas nested inside a trait's arguments do not split predicates.
            let mut depth = 0i32;
            let mut start = 0;
            for end in 0..=predicates.len() {
                let separator =
                    end == predicates.len() || (predicates[end].text == "," && depth == 0);
                if separator {
                    if let [subject, colon, bounds @ ..] = &predicates[start..end]
                        && subject.text == parameter.name
                        && colon.text == ":"
                    {
                        lists.push(bounds);
                    }
                    start = end + 1;
                } else {
                    match predicates[end].text.as_str() {
                        "<" | "(" | "[" => depth += 1,
                        ">" | ")" | "]" => depth -= 1,
                        _ => {}
                    }
                }
            }
            let edition = match signature.header.profile.edition {
                2015 => ra_ap_syntax::Edition::Edition2015,
                2018 => ra_ap_syntax::Edition::Edition2018,
                2021 => ra_ap_syntax::Edition::Edition2021,
                _ => ra_ap_syntax::Edition::Edition2024,
            };
            for list in lists {
                if list.is_empty() {
                    continue;
                }
                let text = crate::syntax::ast_adapter::token_text(list);
                let parsed =
                    ast::SourceFile::parse(&format!("type LrrBound = dyn {text};"), edition);
                let Some(ast::Item::TypeAlias(alias)) = parsed.tree().items().next() else {
                    continue;
                };
                let Some(ast::Type::DynTraitType(object)) = alias.ty() else {
                    continue;
                };
                for bound in object
                    .type_bound_list()
                    .into_iter()
                    .flat_map(|list| list.bounds())
                {
                    let Some(ast::Type::PathType(path)) = bound.ty() else {
                        continue;
                    };
                    let Some(path) = path.path() else {
                        continue;
                    };
                    paths.push(path);
                }
            }
        }
        let mut selected = None;
        for path in paths {
            let trait_key = match projection_trait_key(graph, signature, &path) {
                Ok(Some(key)) => key,
                Ok(None) => continue,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let context = db.input(&InputKey::DeclarationContext(trait_key));
            let Some(InputValue::DeclarationContext(context)) = context.as_deref() else {
                continue;
            };
            let scope = crate::syntax::outline::ScopeKey {
                owner: Some(trait_key),
                ..context.scope.clone()
            };
            // Some imported declaration-only contexts do not publish
            // their associated scope yet. Preserve the existing unknown
            // projection rather than imposing a new static rejection.
            if db.input(&InputKey::ScopeBlockers(scope.clone())).is_none() {
                continue;
            }
            let key = crate::db::key::BucketKey {
                scope,
                namespace: Namespace::Type,
                name: item.into(),
            };
            let outcome = match db.fetch(&SyntaxProvider, QueryKey::ScopeBucket(key)) {
                QueryPoll::Ready(Ok(outcome)) => outcome,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let QueryValue::ScopeBucket(bucket) = outcome.value.as_ref() else {
                return QueryPoll::Ready(Err(fault(
                    "associated declaration bucket result mismatch",
                )));
            };
            if !bucket.blockers.is_empty() || bucket.candidates.is_empty() {
                continue;
            }
            // Do not inspect arguments of unrelated bounds. They may
            // themselves name this projection (Other<T::Item>).
            let candidate = match projection_trait(db, graph, sources, signature, &path) {
                QueryPoll::Ready(Ok(Some(candidate))) => candidate,
                QueryPoll::Ready(Ok(None)) => continue,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if selected.as_ref().is_some_and(|prior| prior != &candidate) {
                return QueryPoll::Ready(Ok(None));
            }
            selected = Some(candidate);
        }
        QueryPoll::Ready(Ok(selected))
    }

    walk(db, graph, sources, signature, ty)
}
/// The written self type of impl `owner`.
pub(crate) fn impl_self_type(
    sources: &[&PreparedSourceView],
    owner: crate::core::digest::Digest,
    edition: u16,
) -> Option<ra_ap_syntax::ast::Type> {
    use ra_ap_syntax::ast::{self, HasModuleItem};
    let item = sources.iter().find_map(|source| {
        source.outline().items.iter().find(|item| {
            item.def_key.item_kind == ItemKind::Impl && item.def_key.enclosing_impl == Some(owner)
        })
    })?;
    let header = super::member_index::impl_header_text(&item.header.tokens);
    let parser = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        _ => ra_ap_syntax::Edition::Edition2024,
    };
    let parsed = ast::SourceFile::parse(&format!("{header} {{}}"), parser);
    let Some(ast::Item::Impl(implementation)) = parsed.tree().items().next() else {
        return None;
    };
    implementation.self_ty()
}
/// The written self type of a macro-generated impl `owner`, found through
/// the declaring function's own generated unit (no unit scan).
pub(crate) fn generated_impl_self_type(
    db: &mut Database,
    declaration: crate::core::digest::Digest,
    owner: crate::core::digest::Digest,
    edition: u16,
) -> Option<ra_ap_syntax::ast::Type> {
    use ra_ap_syntax::ast::{self, HasModuleItem};
    let source = db.input(&InputKey::DefinitionSource(declaration))?;
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        return None;
    };
    let unit = db.generated_declaration_unit(parse)?;
    let outline = unit.outline().ok()?;
    let item = outline.items.iter().find(|item| {
        item.def_key.item_kind == ItemKind::Impl && item.def_key.enclosing_impl == Some(owner)
    })?;
    let header = super::member_index::impl_header_text(&item.header.tokens);
    let parser = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        _ => ra_ap_syntax::Edition::Edition2024,
    };
    let parsed = ast::SourceFile::parse(&format!("{header} {{}}"), parser);
    let Some(ast::Item::Impl(implementation)) = parsed.tree().items().next() else {
        return None;
    };
    implementation.self_ty()
}
/// A written type name in a function owned by impl `owner`.
fn impl_owned_name(
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    generated: Option<&crate::syntax::outline::Outline>,
    signature: &FunctionSignature,
    owner: crate::core::digest::Digest,
    name: &str,
) -> Result<SignatureName, Diagnostic> {
    use ra_ap_syntax::ast::{self, HasGenericParams, HasModuleItem};
    let edition = signature.header.profile.edition;
    let module = crate::syntax::outline::ScopeKey {
        owner: None,
        ..signature.scope.clone()
    };
    let lookup = |segments: &[crate::syntax::imports::ImportSegment], absolute: bool| {
        graph.lookup_source_path(Namespace::Type, segments, absolute, &module, edition)
    };
    if name != "Self" {
        let found = lookup(
            &[crate::syntax::imports::ImportSegment::Name(name.to_owned())],
            false,
        )?;
        return super::signature_names::classify_completed_type_lookup(name, edition, found);
    }
    let owned = |item: &&crate::syntax::outline::ItemOutline| {
        item.def_key.item_kind == ItemKind::Impl && item.def_key.enclosing_impl == Some(owner)
    };
    let Some(item) = sources
        .iter()
        .find_map(|source| source.outline().items.iter().find(owned))
        .or_else(|| generated.and_then(|outline| outline.items.iter().find(owned)))
    else {
        return Ok(SignatureName::NeedOwnerContext(signature.scope.clone()));
    };
    let header = super::member_index::impl_header_text(&item.header.tokens);
    let parser = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        _ => ra_ap_syntax::Edition::Edition2024,
    };
    let parsed = ast::SourceFile::parse(&format!("{header} {{}}"), parser);
    let Some(ast::Item::Impl(implementation)) = parsed.tree().items().next() else {
        return Ok(SignatureName::NeedOwnerContext(signature.scope.clone()));
    };
    // A generic self type (`impl<T> W<T>`) needs its owner's parameters.
    if implementation.generic_param_list().is_some() {
        return Ok(SignatureName::NeedOwnerContext(signature.scope.clone()));
    }
    let Some(ast::Type::PathType(path)) = implementation.self_ty() else {
        return Ok(SignatureName::NeedOwnerContext(signature.scope.clone()));
    };
    let Some((segments, absolute)) = path.path().and_then(super::member_index::path_segments)
    else {
        return Ok(SignatureName::NeedOwnerContext(signature.scope.clone()));
    };
    let last = match segments.last() {
        Some(crate::syntax::imports::ImportSegment::Name(last)) => last.clone(),
        _ => return Ok(SignatureName::NeedOwnerContext(signature.scope.clone())),
    };
    let found = lookup(&segments, absolute)?;
    super::signature_names::classify_completed_type_lookup(&last, edition, found)
}
/// A type alias whose target is a written compound type (`type Foo =
/// GenericFoo<u32>`, `type Pair<T> = (T, T)`): the target resolved by the same
/// written-Type walk in the alias's own declaration context, its Type
/// parameters replaced by the use site's written arguments in order. `None`
/// when the target or the argument list is outside that walk.
pub(super) fn deferred_alias_type(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    definition: crate::core::digest::Digest,
    arguments: &[crate::traits::term::GenericArg],
) -> QueryPoll<Option<TyTerm>> {
    let outcome = match db.fetch(&SyntaxProvider, QueryKey::TypeAlias(definition)) {
        QueryPoll::Ready(Ok(outcome)) => outcome,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    let QueryValue::TypeAlias(alias) = outcome.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("alias query result mismatch")));
    };
    if alias.scope.owner.is_some() {
        return QueryPoll::Ready(Ok(None));
    }
    // The alias declaration is its own Type owner: its generics, scope and
    // edition, with the target as the only written Type.
    let signature = FunctionSignature {
        definition,
        scope: alias.scope.clone(),
        header: alias.header.clone(),
        generics: alias.generics.clone(),
        parameters: Vec::new(),
        result: alias.target.clone(),
        result_symbolic: None,
        predicates: Vec::new(),
    };
    let term = match resolve_written_type(db, graph, sources, &signature, &alias.target) {
        QueryPoll::Ready(Ok(Some(term))) => term,
        other => return other,
    };
    let mut supplied = arguments.iter().filter_map(|argument| match argument {
        crate::traits::term::GenericArg::Type(ty) => Some(ty),
        _ => None,
    });
    let mut map = std::collections::BTreeMap::new();
    for parameter in alias
        .generics
        .iter()
        .filter(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
    {
        let key = match resolve_named(db, graph, sources, &signature, &parameter.name) {
            QueryPoll::Ready(Ok(SignatureName::Parameter(key))) => key,
            QueryPoll::Ready(Ok(_)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let Some(argument) = supplied.next() else {
            return QueryPoll::Ready(Ok(None));
        };
        map.insert(key, argument.clone());
    }
    if supplied.next().is_some() {
        return QueryPoll::Ready(Ok(None));
    }
    QueryPoll::Ready(Ok(Some(super::associated_types::substitute(&term, &map))))
}
/// Resolve the selected result of the owning source Type lookup above. This
/// internal helper cannot serve as an alternate public namespace admission.
pub(super) fn prepare_selected(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    selected: SignatureName,
) -> QueryPoll<NamedTypePreparation> {
    let proof = match selected {
        SignatureName::Primitive(p) => match SourceType::selected_primitive(p) {
            Ok(proof) => proof,
            Err(error) => return QueryPoll::Ready(Err(error)),
        },
        SignatureName::Str => match SourceType::selected_str() {
            Ok(proof) => proof,
            Err(error) => return QueryPoll::Ready(Err(error)),
        },
        SignatureName::Parameter(key) => {
            return QueryPoll::Ready(Ok(NamedTypePreparation::Parameter(key)));
        }
        SignatureName::Source(selected) => {
            let normalizer = match AliasNormalizer::new(graph, sources) {
                Ok(normalizer) => normalizer,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let target = match normalizer.normalize(db, selected.identity) {
                QueryPoll::Ready(Ok(target)) => target,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            };
            let identity = match target {
                AliasTarget::Str | AliasTarget::Type(TyTerm::Str) => {
                    return QueryPoll::Ready(
                        SourceType::selected_str().map(NamedTypePreparation::Ready),
                    );
                }
                AliasTarget::Nominal(identity) => identity,
                AliasTarget::Deferred { definition, target } => {
                    return match deferred_alias_type(db, graph, sources, definition, &[]) {
                        QueryPoll::Ready(Ok(Some(term))) => QueryPoll::Ready(Ok(
                            NamedTypePreparation::Alias(AliasTarget::Type(term)),
                        )),
                        QueryPoll::Ready(Ok(None)) => {
                            QueryPoll::Ready(Ok(NamedTypePreparation::Alias(
                                AliasTarget::Deferred { definition, target },
                            )))
                        }
                        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(request) => QueryPoll::NeedInput(request),
                    };
                }
                other => return QueryPoll::Ready(Ok(NamedTypePreparation::Alias(other))),
            };
            let definition = match nominal_definition(db, sources, identity) {
                Ok(definition) => definition,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            return prepare_nominal(db, identity, definition);
        }
        other => return QueryPoll::Ready(Ok(NamedTypePreparation::Name(other))),
    };
    if let Err(error) = proof.allocated_bytes() {
        return QueryPoll::Ready(Err(error));
    }
    QueryPoll::Ready(Ok(NamedTypePreparation::Ready(proof)))
}
/// The same canonical nominal/schema kernel serves header-selected and
/// body-occurrence-selected Types. Namespace/use authority is supplied before
/// this function; neither a TypeId nor a public NameLookup enters this core.
fn prepare_nominal(
    db: &mut Database,
    identity: crate::core::digest::Digest,
    definition: DefKey,
) -> QueryPoll<NamedTypePreparation> {
    let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(identity)) {
        QueryPoll::Ready(Ok(captured)) => captured,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("selected nominal schema query differs")));
    };
    if schema.definition != identity
        || definition.namespace != Namespace::Type
        || !matches!(definition.item_kind, ItemKind::Struct | ItemKind::Enum)
        || !matches!(db.input(&InputKey::DefinitionHeader(identity)).as_deref(),Some(InputValue::DefinitionHeader(header)) if header==&schema.header)
        || !matches!(db.input(&InputKey::DeclarationContext(identity)).as_deref(),Some(InputValue::DeclarationContext(context)) if context.scope==schema.scope)
        || !matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),Some(InputValue::DefinitionSource(parse)) if parse.profile==schema.header.profile)
    {
        return QueryPoll::Ready(Err(fault(
            "selected nominal source/schema publication differs",
        )));
    }
    // Only Type/const parameters need arguments: regions are not runtime
    // data, so a lifetime-only declaration (`Formatter<'a>`) is closed.
    if schema
        .generics
        .iter()
        .any(|parameter| parameter.kind != crate::syntax::signatures::GenericKind::Lifetime)
    {
        return QueryPoll::Ready(Ok(NamedTypePreparation::Alias(AliasTarget::Nominal(
            identity,
        ))));
    }
    let proof = SourceType {
        source: TyTerm::Nominal(identity, vec![]),
        key: TypeKey::Nominal {
            identity,
            definition,
            schema: schema.fingerprint(),
            args: Box::new([]),
            const_args: Box::new([]),
        },
    };
    if let Err(error) = proof.allocated_bytes() {
        return QueryPoll::Ready(Err(error));
    }
    QueryPoll::Ready(Ok(NamedTypePreparation::Ready(proof)))
}

/// Body Type authority is the opaque current occurrence/declaration proof, not
/// a synthetic function signature or caller-selected source/name answer.
pub(crate) fn prepare_body_declaration(
    db: &mut Database,
    declaration: &super::source_locals::owned_plan::type_declarations::BodyTypeDeclaration,
) -> QueryPoll<NamedTypePreparation> {
    match declaration.require_current(db) {
        QueryPoll::Ready(Ok(())) => {}
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    }
    prepare_nominal(
        db,
        declaration.definition_identity(),
        declaration.definition().clone(),
    )
}

pub(super) fn nominal_definition(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    identity: crate::core::digest::Digest,
) -> Result<DefKey, Diagnostic> {
    for source in sources {
        if let Some(item) = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == identity)
        {
            if !matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),Some(InputValue::DefinitionSource(parse)) if parse==&source.selection().outline.parse)
            {
                return Err(fault(
                    "selected nominal source differs from actual catalogue",
                ));
            }
            return Ok(item.def_key.clone());
        }
    }
    let source = db
        .input(&InputKey::DefinitionSource(identity))
        .ok_or_else(|| fault("selected nominal source absent"))?;
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        return Err(fault("selected nominal source variant differs"));
    };
    let generated = db
        .input(&InputKey::GeneratedSyntax(parse.content))
        .ok_or_else(|| fault("selected generated nominal syntax absent"))?;
    let InputValue::GeneratedSyntax(unit) = generated.as_ref() else {
        return Err(fault("selected nominal generated variant differs"));
    };
    if unit.key != *parse {
        return Err(fault("selected generated nominal source key differs"));
    }
    // Generated declaration identity is the actual GeneratedItemKey, not the
    // independently retained parsed DefKey. The current canonical map joins
    // both; callers have already required the exact selected Type receipt and
    // nominal schema/header/context. Original catalogue selection above still
    // requires the DefKey fingerprint itself to equal identity.
    unit.definitions
        .get(&identity)
        .cloned()
        .ok_or_else(|| fault("selected generated nominal identity absent"))
}

/// Canonical local declaration ownership after a source consumer selected the
/// nominal identity. This does not perform a name lookup or derive authority
/// from a guest descriptor: the exact current owning plan must contain the row.
pub(super) struct LocalNominalOwner {
    pub(super) definition: DefKey,
    pub(super) plan: std::sync::Arc<super::source_locals::PreparedSourceLocalPlan>,
    pub(super) edition: u16,
}
pub(super) fn local_nominal_owner(
    db: &mut Database,
    identity: crate::core::digest::Digest,
) -> QueryPoll<Option<LocalNominalOwner>> {
    let Some(context) = db.input(&InputKey::DeclarationContext(identity)) else {
        return QueryPoll::Ready(Ok(None));
    };
    let InputValue::DeclarationContext(context) = context.as_ref() else {
        return QueryPoll::Ready(Err(fault("local nominal context variant differs")));
    };
    if context.scope.owner.is_none() {
        return QueryPoll::Ready(Ok(None));
    }
    // A scope owner is a lexical rib, not a definition. Only the admitted
    // declaration publication supplies its actual owning executable body.
    let Some(owner) = db.input(&InputKey::LocalDeclarationOwner(identity)) else {
        return QueryPoll::Ready(Err(fault("local nominal owning declaration is absent")));
    };
    let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
        return QueryPoll::Ready(Err(fault("local nominal owner variant differs")));
    };
    let Some(body) = db.input(&InputKey::DefinitionBody(owner.definition)) else {
        return QueryPoll::Ready(Err(fault("local nominal owning body is absent")));
    };
    let InputValue::DefinitionBody(body) = body.as_ref() else {
        return QueryPoll::Ready(Err(fault("local nominal owning body variant differs")));
    };
    if body.key() != *owner {
        return QueryPoll::Ready(Err(fault("local nominal owning body changed")));
    }
    let result = match db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body.key())) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::SourceLocalPlan(plan) = result.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("local nominal owning plan variant differs")));
    };
    if plan.environment().is_none() || plan.context().source().index().body() != &body.key() {
        return QueryPoll::Ready(Err(fault("local nominal owning environment differs")));
    }
    match plan.require_current(db) {
        QueryPoll::Ready(Ok(())) => {}
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    }
    let mut rows = plan
        .context()
        .facts()
        .items
        .iter()
        .filter(|row| row.item.def_key.fingerprint() == identity);
    let Some(row) = rows.next() else {
        return QueryPoll::Ready(Err(fault("local nominal canonical declaration is absent")));
    };
    if rows.next().is_some()
        || row.item.scope != context.scope
        || row.item.declaration_span != context.range
        || !matches!(db.input(&InputKey::DefinitionHeader(identity)).as_deref(),
            Some(InputValue::DefinitionHeader(header)) if header == &row.item.header)
        || !matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),
            Some(InputValue::DefinitionSource(parse)) if *parse == plan.context().source().parsed().key())
    {
        return QueryPoll::Ready(Err(fault("local nominal canonical source rows differ")));
    }
    // Read the actual selected declaration name, including its owning token
    // edition. Header profile alone cannot replace a generated token edition.
    use ra_ap_syntax::{
        AstNode,
        ast::{self, HasName},
    };
    let parsed = plan.context().source().parsed();
    let name = (|| -> Result<_, Diagnostic> {
        let selected = parsed
            .syntax_at(context.range, |node| {
                ast::Struct::can_cast(node.kind()) || ast::Enum::can_cast(node.kind())
            })?
            .and_then(ast::Item::cast);
        let name = match selected {
            Some(ast::Item::Struct(item)) => item.name(),
            Some(ast::Item::Enum(item)) => item.name(),
            _ => None,
        }
        .ok_or_else(|| fault("local nominal actual name is absent"))?;
        let range = crate::syntax::ast_adapter::range(name.syntax().text_range());
        let edition = parsed.token_edition(range)?;
        if super::names::owned_name(name.text(), edition)?.as_str()
            != row.item.def_key.declared_name
        {
            return Err(fault("local nominal actual name differs"));
        }
        Ok(edition)
    })();
    match name {
        Ok(edition) => QueryPoll::Ready(Ok(Some(LocalNominalOwner {
            definition: row.item.def_key.clone(),
            plan: std::sync::Arc::clone(plan),
            edition,
        }))),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
