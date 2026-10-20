//! Retained real declaration-header Type occurrences for the ordinary prefix.
use super::{
    LocalTypePrefix, LocalTypePrefixSelection, LocalTypeUse, PreparedSourceLocalPlan, error,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts,
        signature_names::SignatureName,
        source_body_context::PreparedBodyContext,
        source_types::NamedTypePreparation,
        types::{FieldRecipe, NominalSchema},
    },
    source::manifest::{ItemKind, Namespace},
    syntax::{
        ast_adapter::{TokenRecipe, range, tokens},
        spans::SourceRange,
        token_wire::{Key, NameContextKey},
    },
    traits::term::ParamKey,
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasName},
};
use std::{collections::BTreeMap, sync::Arc};
macro_rules! ready {
    ($v:expr) => {
        match $v {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
macro_rules! checked {
    ($v:expr) => {
        match $v {
            Ok(v) => v,
            Err(e) => return QueryPoll::Ready(Err(e)),
        }
    };
}
struct HeaderGeneric {
    name: MacroName,
    context: NameContextKey,
    parameter: ParamKey,
}
struct HeaderField {
    parameter: Option<u32>,
    range: SourceRange,
    tokens: Vec<TokenRecipe>,
    name: Option<LocalTypeUse>,
    /// A compound Type's written structure (`&mut u8`, `Option<T>`,
    /// `(S, [u8; 2])`); `None` for a single name or unsupported syntax.
    shape: Option<HeaderShape>,
}
/// The written structure of a compound header Type. Each leaf is an actual
/// single-segment Type name occurrence, resolved by the same header prefix
/// service as a single-name field or parameter Type.
enum HeaderShape {
    Leaf {
        name: LocalTypeUse,
        args: Vec<HeaderShape>,
    },
    Tuple(Vec<HeaderShape>),
    Array(Box<HeaderShape>, u64),
    Slice(Box<HeaderShape>),
    Ref(bool, Box<HeaderShape>),
}
impl HeaderShape {
    /// `None` when some part is not written structure around single names
    /// (a qualified path, function pointer, trait object or const length).
    fn capture(
        ty: &ast::Type,
        use_name: &dyn Fn(ast::NameRef) -> Result<LocalTypeUse, Diagnostic>,
    ) -> Result<Option<Self>, Diagnostic> {
        let nested = |ty: Option<ast::Type>| -> Result<Option<Self>, Diagnostic> {
            match ty {
                Some(ty) => Self::capture(&ty, use_name),
                None => Ok(None),
            }
        };
        Ok(Some(match ty {
            ast::Type::ParenType(paren) => return nested(paren.ty()),
            ast::Type::TupleType(tuple) => {
                let mut members = Vec::new();
                for member in tuple.fields() {
                    let Some(member) = Self::capture(&member, use_name)? else {
                        return Ok(None);
                    };
                    members.push(member);
                }
                Self::Tuple(members)
            }
            ast::Type::SliceType(slice) => match nested(slice.ty())? {
                Some(element) => Self::Slice(Box::new(element)),
                None => return Ok(None),
            },
            ast::Type::RefType(reference) => match nested(reference.ty())? {
                Some(target) => Self::Ref(reference.mut_token().is_some(), Box::new(target)),
                None => return Ok(None),
            },
            ast::Type::ArrayType(array) => {
                let Some(ast::Expr::Literal(length)) =
                    array.const_arg().and_then(|argument| argument.expr())
                else {
                    return Ok(None);
                };
                let Some(number) = length
                    .syntax()
                    .children_with_tokens()
                    .filter_map(|item| item.into_token())
                    .find_map(<ast::IntNumber as ra_ap_syntax::AstToken>::cast)
                else {
                    return Ok(None);
                };
                if !matches!(number.suffix(), None | Some("usize")) {
                    return Ok(None);
                }
                let Ok(len) = number.value() else {
                    return Ok(None);
                };
                match nested(array.ty())? {
                    Some(element) => Self::Array(Box::new(element), len as u64),
                    None => return Ok(None),
                }
            }
            ast::Type::PathType(path) => {
                let Some(path) = path.path().filter(|path| path.qualifier().is_none()) else {
                    return Ok(None);
                };
                let Some(segment) = path.segment().filter(|s| s.coloncolon_token().is_none())
                else {
                    return Ok(None);
                };
                let Some(name) = segment.name_ref() else {
                    return Ok(None);
                };
                let mut args = Vec::new();
                if let Some(list) = segment.generic_arg_list() {
                    for argument in list.generic_args() {
                        match argument {
                            // Regions are not runtime data.
                            ast::GenericArg::LifetimeArg(_) => {}
                            ast::GenericArg::TypeArg(argument) => {
                                let Some(argument) = nested(argument.ty())? else {
                                    return Ok(None);
                                };
                                args.push(argument);
                            }
                            _ => return Ok(None),
                        }
                    }
                }
                Self::Leaf {
                    name: use_name(name)?,
                    args,
                }
            }
            _ => return Ok(None),
        }))
    }
    fn heap_bytes(&self) -> usize {
        match self {
            Self::Leaf { name, args } => args.iter().fold(
                name.name
                    .capacity()
                    .saturating_add(args.capacity().saturating_mul(std::mem::size_of::<Self>())),
                |n, arg| n.saturating_add(arg.heap_bytes()),
            ),
            Self::Tuple(members) => members.iter().fold(
                members
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Self>()),
                |n, member| n.saturating_add(member.heap_bytes()),
            ),
            Self::Array(inner, _) | Self::Slice(inner) | Self::Ref(_, inner) => {
                std::mem::size_of::<Self>().saturating_add(inner.heap_bytes())
            }
        }
    }
    fn encode(&self, e: &mut StableEncoder) {
        match self {
            Self::Leaf { name, args } => {
                e.u8(0);
                e.string(name.name());
                e.u32(name.range.start);
                e.u32(name.range.end);
                e.u16(name.edition());
                e.digest(Digest(name.context().0));
                e.u64(args.len() as u64);
                for arg in args {
                    arg.encode(e);
                }
            }
            Self::Tuple(members) => {
                e.u8(1);
                e.u64(members.len() as u64);
                for member in members {
                    member.encode(e);
                }
            }
            Self::Array(element, len) => {
                e.u8(2);
                e.u64(*len);
                element.encode(e);
            }
            Self::Slice(element) => {
                e.u8(3);
                element.encode(e);
            }
            Self::Ref(mutable, target) => {
                e.u8(4);
                e.bool(*mutable);
                target.encode(e);
            }
        }
    }
}
struct HeaderItem {
    scope: u32,
    declaration: SourceRange,
    header: Digest,
    header_input: Digest,
    generics: Vec<HeaderGeneric>,
    fields: Vec<HeaderField>,
    // Source parameter ordinal -> original HeaderField row. Receiver/missing
    // Type positions deliberately remain None, never shifted to another row.
    parameter_fields: Vec<Option<u32>>,
}
pub(super) struct DeclarationFieldTypes {
    owner: Digest,
    items: BTreeMap<Digest, HeaderItem>,
    bytes: usize,
    fingerprint: Digest,
}
/// Borrowed only from the actual immutable plan; callers cannot mint/rebind it.
pub(in crate::frontend::source_locals) struct HeaderTypeSite<'a> {
    owner: Digest,
    item: &'a HeaderItem,
    field: &'a HeaderField,
    /// One leaf name occurrence of a compound field Type.
    leaf: Option<&'a LocalTypeUse>,
}
impl HeaderTypeSite<'_> {
    pub(in crate::frontend::source_locals) fn scope(&self) -> u32 {
        self.item.scope
    }
    pub(in crate::frontend::source_locals) fn require(
        &self,
        owner: Digest,
        site: &crate::frontend::source_locals::OrdinaryNameSite<'_>,
        edition: u16,
    ) -> Result<(), Diagnostic> {
        let name = self
            .leaf
            .or(self.field.name.as_ref())
            .ok_or_else(|| error("field Type path recipe is pending"))?;
        if self.owner != owner
            || site.namespace != Namespace::Type
            || site.at != u64::from(name.range().start)
            || site.name != name.name()
            || site.context != name.context()
            || edition != name.edition()
        {
            return Err(error("declaration Type occurrence owner/site differs"));
        }
        Ok(())
    }
    pub(in crate::frontend::source_locals) fn generic(
        &self,
        name: &MacroName,
        context: NameContextKey,
    ) -> Result<Option<ParamKey>, Diagnostic> {
        let mut selected = None;
        for generic in &self.item.generics {
            if &generic.name == name
                && generic.context == context
                && selected.replace(generic.parameter).is_some()
            {
                return Err(error("ambiguous declaration Type generic occurrence"));
            }
        }
        Ok(selected)
    }
}
fn add(bytes: usize, extra: usize) -> Result<usize, Diagnostic> {
    bytes
        .checked_add(extra)
        .ok_or_else(crate::runtime::arena::resource)
}
impl DeclarationFieldTypes {
    pub(super) fn prepare(
        context: &PreparedBodyContext,
        root: Key,
        hygiene: &HygieneFacts,
    ) -> Result<Self, Diagnostic> {
        let source = context.source();
        let parsed = source.parsed();
        let use_name = |name: ast::NameRef| -> Result<LocalTypeUse, Diagnostic> {
            let span = range(name.syntax().text_range());
            Ok(LocalTypeUse {
                name: name.text().to_string(),
                range: span,
                context: source.identifier_context_or_original(span, root)?,
                edition: parsed.token_edition(span)?,
            })
        };
        let mut items = BTreeMap::new();
        let mut bytes = 0;
        let mut fingerprint =
            StableEncoder::new("lrr/v5/local-declaration-header-Type-occurrences");
        fingerprint.digest(source.identity());
        fingerprint.u64(
            context
                .facts()
                .declarations()
                .filter(|row| {
                    matches!(
                        row.item.def_key.item_kind,
                        ItemKind::Struct
                            | ItemKind::Enum
                            | ItemKind::Union
                            | ItemKind::Function
                            | ItemKind::Impl
                            | ItemKind::Constant
                            | ItemKind::Static
                            | ItemKind::TypeAlias
                    ) && !row.item.is_discriminant_constant()
                })
                .count() as u64,
        );
        for row in context.facts().declarations() {
            if !matches!(
                row.item.def_key.item_kind,
                ItemKind::Struct
                    | ItemKind::Enum
                    | ItemKind::Union
                    | ItemKind::Function
                    | ItemKind::Impl
                    | ItemKind::Constant
                    | ItemKind::Static
                    | ItemKind::TypeAlias
            ) || row.item.is_discriminant_constant()
            {
                continue;
            }
            let syntax = parsed
                .syntax_at(row.item.declaration_span, |n| ast::Item::can_cast(n.kind()))?
                .ok_or_else(|| error("declaration field owning syntax absent"))?;
            let function = ast::Fn::cast(syntax.clone());
            let implementation = ast::Impl::cast(syntax.clone());
            let constant = ast::Const::cast(syntax.clone());
            let static_ = ast::Static::cast(syntax.clone());
            let excluded = function
                .as_ref()
                .and_then(|function| function.syntax().children().find_map(ast::BlockExpr::cast))
                .map(|body| body.syntax().text_range())
                .or_else(|| {
                    implementation
                        .as_ref()
                        .and_then(|implementation| implementation.assoc_item_list())
                        .map(|list| list.syntax().text_range())
                })
                .or_else(|| {
                    constant
                        .as_ref()
                        .and_then(|item| item.body())
                        .or_else(|| static_.as_ref().and_then(|item| item.body()))
                        .map(|body| body.syntax().text_range())
                });
            if (row.item.def_key.item_kind == ItemKind::Function) != function.is_some()
                || (row.item.def_key.item_kind == ItemKind::Impl) != implementation.is_some()
                || (row.item.def_key.item_kind == ItemKind::Constant) != constant.is_some()
                || (row.item.def_key.item_kind == ItemKind::Static) != static_.is_some()
                || tokens(&syntax, excluded) != row.item.header.tokens
            {
                return Err(error("declaration field header syntax differs"));
            }
            // Parameter/field token copies are subsets of this already owned
            // header. Bound their possible token storage before capturing any.
            add(bytes, row.item.header.estimated_bytes())?;
            let definition = row.item.def_key.fingerprint();
            let mut generics = Vec::new();
            if let Some(list) = syntax.children().find_map(ast::GenericParamList::cast) {
                let count = list.generic_params().count();
                add(
                    add(bytes, row.item.header.estimated_bytes())?,
                    count.checked_mul(
                        std::mem::size_of::<HeaderGeneric>()
                            + std::mem::size_of::<crate::syntax::signatures::DefinitionParameter>(),
                    ).ok_or_else(crate::runtime::arena::resource)?,
                )?;
                generics
                    .try_reserve_exact(count)
                    .map_err(|_| crate::runtime::arena::resource())?;
                let common = crate::syntax::signatures::capture_generics(Some(list.clone()))?;
                for (ordinal, parameter) in list.generic_params().enumerate() {
                    let ast::GenericParam::TypeParam(parameter) = parameter else {
                        continue;
                    };
                    let name = parameter
                        .name()
                        .ok_or_else(|| error("declaration Type parameter name absent"))?;
                    let span = range(name.syntax().text_range());
                    let edition = parsed.token_edition(span)?;
                    let parsed_name = MacroName::parse(name.text(), edition)?;
                    let actual = common
                        .get(ordinal)
                        .filter(|p| {
                            p.name == parsed_name.as_str()
                                && p.kind == crate::syntax::signatures::GenericKind::Type
                        })
                        .ok_or_else(|| error("declaration Type parameter common recipe differs"))?;
                    let raw = source.identifier_context_or_original(span, root)?;
                    generics.push(HeaderGeneric {
                        name: parsed_name,
                        context: hygiene.name_item(raw)?,
                        parameter: ParamKey {
                            definition,
                            ordinal: actual.ordinal,
                        },
                    });
                }
            }
            let mut fields = Vec::new();
            // Function bodies are excluded: only the actual parameter list is
            // source Type evidence, never an executable child body scan.
            let mut types = Vec::new();
            let mut parameter_count = 0usize;
            if let Some(function) = function {
                let list = function
                    .param_list()
                    .ok_or_else(|| error("declaration parameter list absent"))?;
                let first = usize::from(list.self_param().is_some());
                let count = list
                    .params()
                    .filter(|parameter| parameter.ty().is_some())
                    .count();
                parameter_count = list
                    .params()
                    .count()
                    .checked_add(first)
                    .ok_or_else(crate::runtime::arena::resource)?;
                add(
                    bytes,
                    count
                        .checked_mul(std::mem::size_of::<(Option<u32>, ast::Type)>())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )?;
                types
                    .try_reserve_exact(count)
                    .map_err(|_| crate::runtime::arena::resource())?;
                for (index, parameter) in list.params().enumerate() {
                    if let Some(ty) = parameter.ty() {
                        let ordinal = u32::try_from(
                            index
                                .checked_add(first)
                                .ok_or_else(crate::runtime::arena::resource)?,
                        )
                        .map_err(|_| crate::runtime::arena::resource())?;
                        types.push((Some(ordinal), ty));
                    }
                }
            } else if let Some(implementation) = &implementation {
                // An impl header's Type occurrences are its self type and its
                // trait path: each is entered at its leading name (the rest
                // of a qualified path resolves in the module graph).
                if let Some(trait_) = implementation.trait_() {
                    types.push((None, trait_));
                }
                if let Some(self_ty) = implementation.self_ty() {
                    types.push((None, self_ty));
                }
            } else if matches!(
                row.item.def_key.item_kind,
                ItemKind::Struct | ItemKind::Enum | ItemKind::Union
            ) {
                // Confined to this exact nominal header. Const/static result
                // annotations are prepared on demand, without walking bodies.
                let count = syntax
                    .descendants()
                    .filter(|node| {
                        ast::RecordField::can_cast(node.kind())
                            || ast::TupleField::can_cast(node.kind())
                    })
                    .count();
                add(
                    bytes,
                    count
                        .checked_mul(std::mem::size_of::<(Option<u32>, ast::Type)>())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )?;
                types
                    .try_reserve_exact(count)
                    .map_err(|_| crate::runtime::arena::resource())?;
                for ty in syntax.descendants().filter_map(|n| {
                    if let Some(f) = ast::RecordField::cast(n.clone()) {
                        f.ty()
                    } else {
                        ast::TupleField::cast(n).and_then(|f| f.ty())
                    }
                }) {
                    types.push((None, ty));
                }
            }
            let temporary_bytes = types
                .capacity()
                .checked_mul(std::mem::size_of::<(Option<u32>, ast::Type)>())
                .ok_or_else(crate::runtime::arena::resource)?;
            add(
                add(
                    add(bytes, temporary_bytes)?,
                    row.item.header.estimated_bytes(),
                )?,
                types
                    .len()
                    .checked_mul(std::mem::size_of::<HeaderField>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )?;
            fields
                .try_reserve_exact(types.len())
                .map_err(|_| crate::runtime::arena::resource())?;
            for (parameter, field) in types {
                let field_range = range(field.syntax().text_range());
                let name = if implementation.is_some() {
                    // The leading segment of the written path, through one
                    // reference (`impl Tr for &S`).
                    let path_type = match &field {
                        ast::Type::RefType(reference) => match reference.ty() {
                            Some(ast::Type::PathType(inner)) => Some(inner),
                            _ => None,
                        },
                        ast::Type::PathType(path) => Some(path.clone()),
                        _ => None,
                    };
                    path_type
                        .and_then(|ty| ty.path())
                        .and_then(|mut path| {
                            while let Some(qualifier) = path.qualifier() {
                                path = qualifier;
                            }
                            path.segment()
                        })
                        .filter(|s| s.coloncolon_token().is_none())
                        .and_then(|s| s.name_ref())
                        .map(&use_name)
                        .transpose()?
                } else if let ast::Type::PathType(ty) = &field {
                    ty.path()
                        .filter(|p| p.qualifier().is_none())
                        .and_then(|p| p.segment())
                        .filter(|s| {
                            s.generic_arg_list().is_none() && s.coloncolon_token().is_none()
                        })
                        .and_then(|s| s.name_ref())
                        .map(&use_name)
                        .transpose()?
                } else {
                    None
                };
                let shape = match name {
                    Some(_) => None,
                    None => HeaderShape::capture(&field, &use_name)?,
                };
                fields.push(HeaderField {
                    parameter,
                    range: field_range,
                    tokens: tokens(field.syntax(), None),
                    name,
                    shape,
                });
            }
            bytes = add(
                bytes,
                16 * std::mem::size_of::<(Digest, HeaderItem)>() + 256,
            )?;
            bytes = add(
                bytes,
                generics
                    .capacity()
                    .checked_mul(std::mem::size_of::<HeaderGeneric>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )?;
            for generic in &generics {
                bytes = add(
                    bytes,
                    generic
                        .name
                        .as_str()
                        .len()
                        .checked_mul(2)
                        .ok_or_else(crate::runtime::arena::resource)?,
                )?;
            }
            bytes = add(
                bytes,
                fields
                    .capacity()
                    .checked_mul(std::mem::size_of::<HeaderField>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )?;
            fingerprint.digest(definition);
            fingerprint.u32(row.scope);
            fingerprint.digest(row.item.header.fingerprint());
            fingerprint.u64(generics.len() as u64);
            for generic in &generics {
                fingerprint.string(generic.name.as_str());
                fingerprint.digest(Digest(generic.context.0.0));
                fingerprint.digest(generic.parameter.definition);
                fingerprint.u32(generic.parameter.ordinal);
            }
            fingerprint.u64(fields.len() as u64);
            for field in &fields {
                fingerprint.option(&field.parameter, |e, ordinal| e.u32(*ordinal));
                bytes = add(
                    bytes,
                    field
                        .tokens
                        .capacity()
                        .checked_mul(std::mem::size_of::<TokenRecipe>())
                        .ok_or_else(crate::runtime::arena::resource)?,
                )?;
                for token in &field.tokens {
                    bytes = add(bytes, token.text.capacity())?;
                }
                if let Some(name) = &field.name {
                    bytes = add(bytes, name.name.capacity())?;
                }
                if let Some(shape) = &field.shape {
                    bytes = add(bytes, shape.heap_bytes())?;
                }
                fingerprint.u32(field.range.start);
                fingerprint.u32(field.range.end);
                fingerprint.option(&field.name, |e, n| {
                    e.string(n.name());
                    e.u16(n.edition());
                    e.digest(Digest(n.context().0));
                });
                fingerprint.option(&field.shape, |e, shape| shape.encode(e));
            }
            // The retained vector preserves actual source ordinals, including
            // a receiver or missing Type. Its storage is admitted before reserve.
            let index_bytes = parameter_count
                .checked_mul(std::mem::size_of::<Option<u32>>())
                .ok_or_else(crate::runtime::arena::resource)?;
            bytes = add(bytes, index_bytes)?;
            let mut parameter_fields = Vec::new();
            parameter_fields
                .try_reserve_exact(parameter_count)
                .map_err(|_| crate::runtime::arena::resource())?;
            parameter_fields.resize(parameter_count, None);
            bytes = add(
                bytes,
                parameter_fields
                    .capacity()
                    .checked_sub(parameter_count)
                    .and_then(|extra| extra.checked_mul(std::mem::size_of::<Option<u32>>()))
                    .ok_or_else(crate::runtime::arena::resource)?,
            )?;
            for (field_index, field) in fields.iter().enumerate() {
                let Some(ordinal) = field.parameter else {
                    continue;
                };
                let slot = parameter_fields.get_mut(ordinal as usize).ok_or_else(|| {
                    error("parameter Type source ordinal outside actual parameter list")
                })?;
                let field_index =
                    u32::try_from(field_index).map_err(|_| crate::runtime::arena::resource())?;
                if slot.replace(field_index).is_some() {
                    return Err(error("parameter Type source ordinal is duplicated"));
                }
            }
            fingerprint.u64(parameter_fields.len() as u64);
            for field in &parameter_fields {
                fingerprint.option(field, |e, field| e.u32(*field));
            }
            // Cache the actual header-input projection once at owning-plan
            // preparation; parameter goals compare the published input stamp.
            add(bytes, row.item.header.estimated_bytes())?;
            let header_input = InputValue::DefinitionHeader(row.item.header.clone()).fingerprint();
            fingerprint.digest(header_input);
            if items
                .insert(
                    definition,
                    HeaderItem {
                        scope: row.scope,
                        declaration: row.item.declaration_span,
                        header: row.item.header.fingerprint(),
                        header_input,
                        generics,
                        fields,
                        parameter_fields,
                    },
                )
                .is_some()
            {
                return Err(error("declaration field owner repeats"));
            }
        }
        Ok(Self {
            owner: source.identity(),
            items,
            bytes,
            fingerprint: fingerprint.finish(),
        })
    }
    pub(super) fn extra_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    fn select(
        &self,
        schema: &NominalSchema,
        field: &FieldRecipe,
    ) -> Result<HeaderTypeSite<'_>, Diagnostic> {
        let item = self
            .items
            .get(&schema.definition)
            .ok_or_else(|| error("declaration field actual owning item absent"))?;
        if item.header != schema.header.fingerprint()
            || !item.declaration.contains(field.type_range)
        {
            return Err(error("declaration field actual header/range differs"));
        }
        let mut matches = item.fields.iter().filter(|row| {
            row.parameter.is_none() && row.range == field.type_range && row.tokens == field.ty
        });
        let selected = matches
            .next()
            .ok_or_else(|| error("declaration field exact Type occurrence absent"))?;
        if matches.next().is_some() {
            return Err(error("declaration field Type occurrence is ambiguous"));
        }
        Ok(HeaderTypeSite {
            owner: self.owner,
            item,
            field: selected,
            leaf: None,
        })
    }
    /// Cached actual owning header projection, shared by Value/header goals.
    /// The current input stamp is read by the caller before this comparison.
    pub(super) fn header_identity(
        &self,
        definition: Digest,
        actual_header_input: Digest,
    ) -> Result<Option<Digest>, Diagnostic> {
        let Some(item) = self.items.get(&definition) else {
            return Ok(None);
        };
        if item.header_input != actual_header_input {
            return Err(error("actual declaration header input differs"));
        }
        Ok(Some(item.header))
    }
    fn parameter(
        &self,
        signature: &crate::syntax::signatures::FunctionSignature,
        ordinal: u32,
        header_input: Digest,
    ) -> Result<HeaderTypeSite<'_>, Diagnostic> {
        let item = self
            .items
            .get(&signature.definition)
            .ok_or_else(|| error("parameter Type actual owning header absent"))?;
        if item.header_input != header_input {
            return Err(error("parameter Type actual header input differs"));
        }
        let parameter = signature
            .parameters
            .get(ordinal as usize)
            .ok_or_else(|| error("parameter Type ordinal absent"))?;
        let field_index = item
            .parameter_fields
            .get(ordinal as usize)
            .copied()
            .flatten()
            .ok_or_else(|| error("parameter Type exact occurrence absent"))?;
        let field = item
            .fields
            .get(field_index as usize)
            .ok_or_else(|| error("parameter Type indexed occurrence absent"))?;
        if field.parameter != Some(ordinal)
            || field.tokens != parameter.ty
            || !item.declaration.contains(field.range)
        {
            return Err(error("parameter Type indexed source occurrence differs"));
        }
        Ok(HeaderTypeSite {
            owner: self.owner,
            item,
            field,
            leaf: None,
        })
    }
}
impl PreparedSourceLocalPlan {
    /// Own declaration header only: its parent plan supplies lexical outers,
    /// while FunctionSignature supplies the actual current parameter ordinal.
    /// No child BodyStructure or executable recipe is requested.
    pub(crate) fn prepare_parameter_type(
        self: &Arc<Self>,
        db: &mut Database,
        signature: &crate::syntax::signatures::FunctionSignature,
        ordinal: u32,
    ) -> QueryPoll<NamedTypePreparation> {
        ready!(self.require_current(db));
        let current = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(signature.definition)
        ));
        if !matches!(current.value.as_ref(), QueryValue::FunctionSignature(actual) if std::ptr::eq(actual.as_ref(), signature) || actual.as_ref()==signature)
        {
            return QueryPoll::Ready(Err(error("parameter Type current signature differs")));
        }
        let row = checked!(
            self.context
                .facts()
                .declarations()
                .find(|row| row.item.def_key.fingerprint() == signature.definition)
                .ok_or_else(|| error("parameter Type canonical local owner absent"))
        );
        if row.item.def_key.item_kind != ItemKind::Function
            || row.item.scope != signature.scope
            || !matches!(db.input(&InputKey::LocalDeclarationOwner(signature.definition)).as_deref(),Some(InputValue::LocalDeclarationOwner(body)) if body==self.context.source().index().body())
            || !matches!(db.input(&InputKey::DefinitionSource(signature.definition)).as_deref(),Some(InputValue::DefinitionSource(parse)) if *parse==self.context.source().parsed().key())
            || !matches!(db.input(&InputKey::DeclarationContext(signature.definition)).as_deref(),Some(InputValue::DeclarationContext(actual)) if actual.scope==row.item.scope && actual.range==row.item.declaration_span)
        {
            return QueryPoll::Ready(Err(error(
                "parameter Type current owning declaration differs",
            )));
        }
        let header_key = InputKey::DefinitionHeader(signature.definition);
        if !matches!(
            db.input(&header_key).as_deref(),
            Some(InputValue::DefinitionHeader(_))
        ) {
            return QueryPoll::Ready(Err(error("parameter Type header input absent")));
        }
        let header_input = checked!(
            db.published_input(&header_key)
                .map(|(_, fingerprint)| fingerprint)
                .ok_or_else(|| error("parameter Type header stamp absent"))
        );
        let header = checked!(self.field_types.parameter(signature, ordinal, header_input));
        // A compound Type's nested NameRefs are its retained actual leaf
        // occurrences, each resolved by the same header service; other
        // complex syntax keeps its typed owner-context demand.
        let range = header.field.range;
        let site = super::DeclarationTypeSite::Parameter {
            definition: signature.definition,
            ordinal,
            range,
        };
        if header.field.name.is_none()
            && let Some(shape) = &header.field.shape
        {
            return self.prepare_compound_header_type(db, &header, shape, &site, &signature.scope);
        }
        self.prepare_header_type(db, header, site, &signature.scope)
    }
    /// A compound header Type: its written structure around leaf names that
    /// each enter the ordinary header prefix at their own occurrence.
    fn prepare_compound_header_type(
        self: &Arc<Self>,
        db: &mut Database,
        header: &HeaderTypeSite<'_>,
        shape: &HeaderShape,
        site: &super::DeclarationTypeSite,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> QueryPoll<NamedTypePreparation> {
        use crate::{
            frontend::type_aliases::AliasTarget,
            traits::term::{ConstTerm, GenericArg, RegionTerm, TyTerm},
        };
        macro_rules! nested {
            ($shape:expr) => {
                match ready!(self.prepare_compound_header_type(db, header, $shape, site, scope)) {
                    NamedTypePreparation::Alias(AliasTarget::Type(term)) => term,
                    other => return QueryPoll::Ready(Ok(other)),
                }
            };
        }
        let term = match shape {
            HeaderShape::Leaf { name, args } => {
                let leaf = HeaderTypeSite {
                    owner: header.owner,
                    item: header.item,
                    field: header.field,
                    leaf: Some(name),
                };
                let selected = ready!(self.prepare_header_name(
                    db,
                    leaf,
                    site.clone(),
                    scope,
                    !args.is_empty()
                ));
                let mut arguments = Vec::new();
                for arg in args {
                    arguments.push(GenericArg::Type(nested!(arg)));
                }
                match selected {
                    NamedTypePreparation::Ready(proof) => match proof.source() {
                        TyTerm::Nominal(identity, written) if written.is_empty() => {
                            TyTerm::Nominal(*identity, arguments)
                        }
                        source if arguments.is_empty() => source.clone(),
                        _ => {
                            return QueryPoll::Ready(Ok(NamedTypePreparation::Name(
                                crate::frontend::signature_names::SignatureName::NeedOwnerContext(
                                    scope.clone(),
                                ),
                            )));
                        }
                    },
                    NamedTypePreparation::Parameter(key) if arguments.is_empty() => {
                        TyTerm::Param(key)
                    }
                    NamedTypePreparation::Alias(AliasTarget::Nominal(nominal)) => {
                        TyTerm::Nominal(nominal, arguments)
                    }
                    NamedTypePreparation::Alias(AliasTarget::Type(term))
                        if arguments.is_empty() =>
                    {
                        term
                    }
                    NamedTypePreparation::Parameter(_)
                    | NamedTypePreparation::Alias(AliasTarget::Type(_)) => {
                        return QueryPoll::Ready(Ok(NamedTypePreparation::Name(
                            crate::frontend::signature_names::SignatureName::NeedOwnerContext(
                                scope.clone(),
                            ),
                        )));
                    }
                    other => return QueryPoll::Ready(Ok(other)),
                }
            }
            HeaderShape::Tuple(members) if members.is_empty() => {
                TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit)
            }
            HeaderShape::Tuple(members) => {
                let mut terms = Vec::new();
                for member in members {
                    terms.push(nested!(member));
                }
                TyTerm::Tuple(terms)
            }
            HeaderShape::Array(element, len) => {
                TyTerm::Array(Box::new(nested!(element)), ConstTerm::Value(*len))
            }
            HeaderShape::Slice(element) => TyTerm::Slice(Box::new(nested!(element))),
            // Regions are not runtime data (as in the module declaration
            // route's written Types).
            HeaderShape::Ref(mutable, target) => {
                TyTerm::Ref(RegionTerm::Static, *mutable, Box::new(nested!(target)))
            }
        };
        QueryPoll::Ready(Ok(NamedTypePreparation::Alias(AliasTarget::Type(term))))
    }
    pub(crate) fn prepare_field_type(
        self: &Arc<Self>,
        db: &mut Database,
        schema: &NominalSchema,
        field: &FieldRecipe,
    ) -> QueryPoll<NamedTypePreparation> {
        ready!(self.require_current(db));
        let current = ready!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(schema.definition)));
        if !matches!(current.value.as_ref(),QueryValue::TypeSchema(actual) if actual.as_ref()==schema)
        {
            return QueryPoll::Ready(Err(error("field Type current nominal schema differs")));
        }
        let row = checked!(
            self.context
                .facts()
                .items
                .iter()
                .find(|row| row.item.def_key.fingerprint() == schema.definition)
                .ok_or_else(|| error("field Type canonical local owner absent"))
        );
        if row.item.scope != schema.scope || row.item.header != schema.header {
            return QueryPoll::Ready(Err(error("field Type canonical local header differs")));
        }
        if !matches!(db.input(&InputKey::DefinitionSource(schema.definition)).as_deref(),Some(InputValue::DefinitionSource(parse)) if *parse==self.context.source().parsed().key())
            || !matches!(db.input(&InputKey::DeclarationContext(schema.definition)).as_deref(),Some(InputValue::DeclarationContext(actual)) if actual.scope==row.item.scope &&actual.range==row.item.declaration_span)
        {
            return QueryPoll::Ready(Err(error(
                "field Type current declaration publication differs",
            )));
        }
        if schema
            .variants
            .iter()
            .flat_map(|variant| &variant.fields)
            .filter(|actual| *actual == field)
            .count()
            != 1
        {
            return QueryPoll::Ready(Err(error(
                "field Type recipe is not an exact schema member",
            )));
        }
        let header = checked!(self.field_types.select(schema, field));
        let site = super::DeclarationTypeSite::Field {
            definition: schema.definition,
            range: field.type_range,
        };
        if header.field.name.is_none()
            && let Some(shape) = &header.field.shape
        {
            return self.prepare_compound_header_type(db, &header, shape, &site, &schema.scope);
        }
        self.prepare_header_type(db, header, site, &schema.scope)
    }
    /// Field and parameter sites enter the same opaque local/import/module
    /// prefix and final current declaration/Type preparation service.
    /// A retained declaration Type leaf enters the same header prefix as a
    /// field/parameter. The parser receipt and HeaderItem supply its real
    /// location, lexical scope, generics and token hygiene.
    pub(crate) fn resolve_declaration_type_name(
        self: &Arc<Self>, db: &mut Database,
        declaration: crate::frontend::type_name_namespace::SourceTypeOwner<'_>,
        candidate: &ast::NameRef,
    ) -> QueryPoll<Option<SignatureName>> {
        ready!(self.require_current(db));
        let source = self.context.source();
        let parsed = source.parsed();
        let input = db.input(&InputKey::DefinitionSource(declaration.definition));
        let context = db.input(&InputKey::DeclarationContext(declaration.definition));
        let header = db.input(&InputKey::DefinitionHeader(declaration.definition));
        let (Some(InputValue::DefinitionSource(parse)),Some(InputValue::DeclarationContext(context)),Some(InputValue::DefinitionHeader(header))) =
            (input.as_deref(),context.as_deref(),header.as_deref()) else {
                return QueryPoll::Ready(Err(error("declaration Type name inputs absent")));
            };
        if *parse != parsed.key() || context.scope != *declaration.scope || header != declaration.header {
            return QueryPoll::Ready(Err(error("declaration Type name source/header differs")));
        }
        let Some(item) = self.field_types.items.get(&declaration.definition) else {
            return QueryPoll::Ready(Ok(None));
        };
        if item.header_input != InputValue::DefinitionHeader(header.clone()).fingerprint()
            || item.declaration != context.range {
            return QueryPoll::Ready(Err(error("declaration Type name retained header differs")));
        }
        let proof = checked!(parsed.declaration_type_name(declaration.definition,header,context,candidate));
        checked!(proof.require(parse,declaration.definition,header,context,candidate.text()));
        let span = proof.range();
        let (root, _) = self.entered.local_hygiene();
        let field = HeaderField { parameter: None, range: span,
            tokens: tokens(candidate.syntax(),None), shape: None,
            name: Some(LocalTypeUse { name: candidate.text().to_owned(), range: span,
                context: checked!(source.identifier_context_or_original(span,root)),
                edition: checked!(parsed.token_edition(span)) }) };
        let site = HeaderTypeSite { owner: self.field_types.owner, item, field: &field, leaf: None };
        let prefix = ready!(self.prepare_header_prefix(db,site,
            super::DeclarationTypeSite::Name { definition: declaration.definition, range: span }));
        let name = prefix.use_name().expect("actual declaration prefix name");
        let found = match prefix.selection() {
            LocalTypePrefixSelection::Parameter(parameter) =>
                return QueryPoll::Ready(Ok(Some(SignatureName::Parameter(*parameter)))),
            LocalTypePrefixSelection::Lookup(found) => Some(found.clone()),
            LocalTypePrefixSelection::ModuleTail | LocalTypePrefixSelection::NeedDefinitionModule(_) =>
                ready!(prefix.module_name(db)).and_then(|observation| observation.lookup().cloned()),
            LocalTypePrefixSelection::NeedImport(producer) | LocalTypePrefixSelection::NeedDeclarationOccurrence(producer) =>
                Some(crate::frontend::names::NameLookup::NeedInput(*producer)),
            _ => None,
        };
        QueryPoll::Ready(found.map(|found|
            crate::frontend::signature_names::classify_completed_type_lookup(name.name(),name.edition(),found)).transpose())
    }
    fn prepare_header_type(
        self: &Arc<Self>,
        db: &mut Database,
        header: HeaderTypeSite<'_>,
        identity: super::DeclarationTypeSite,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> QueryPoll<NamedTypePreparation> {
        self.prepare_header_name(db, header, identity, scope, false)
    }
    fn prepare_header_name(
        self: &Arc<Self>,
        db: &mut Database,
        header: HeaderTypeSite<'_>,
        identity: super::DeclarationTypeSite,
        scope: &crate::syntax::outline::ScopeKey,
        generic_base: bool,
    ) -> QueryPoll<NamedTypePreparation> {
        let Some(name) = header.leaf.or(header.field.name.as_ref()) else {
            return QueryPoll::Ready(Ok(NamedTypePreparation::Name(
                SignatureName::NeedOwnerContext(scope.clone()),
            )));
        };
        // `Self` in a member of a block-level impl is the impl's self type.
        if name.name() == "Self"
            && let super::DeclarationTypeSite::Return { definition, .. }
            | super::DeclarationTypeSite::Parameter { definition, .. }
            | super::DeclarationTypeSite::Field { definition, .. } = &identity
            && let Some(term) = ready!(self.local_self_term(db, *definition))
        {
            return QueryPoll::Ready(Ok(NamedTypePreparation::Alias(
                crate::frontend::type_aliases::AliasTarget::Type(term),
            )));
        }
        let prefix = ready!(self.prepare_header_prefix(db, header, identity));
        if generic_base { prefix.prepare_named_base(db) } else { prefix.prepare_named(db) }
    }
    fn prepare_header_prefix(
        self: &Arc<Self>, db: &mut Database, header: HeaderTypeSite<'_>,
        identity: super::DeclarationTypeSite,
    ) -> QueryPoll<LocalTypePrefix> {
        ready!(self.require_current(db));
        let Some(name) = header.leaf.or(header.field.name.as_ref()) else {
            return QueryPoll::Ready(Err(error("header prefix actual name occurrence absent")));
        };
        let site = crate::frontend::source_locals::OrdinaryNameSite {
            source: self.entered.parent.source(),
            body: self.context.source().index().body(),
            at: u64::from(name.range.start),
            name: name.name(),
            context: name.context(),
            namespace: Namespace::Type,
        };
        let (selection, module_selector) = if let Some(key) = self.missing_expansions.first() {
            (
                LocalTypePrefixSelection::NeedExpansionDefinition(*key),
                None,
            )
        } else {
            let found = checked!(self.adapter().lookup_prepared_field_type_prefix(
                &site,
                &self.type_names,
                name.edition(),
                &header
            ));
            checked!(self.select_type_prefix(found, name.context()))
        };
        let mut prefix = LocalTypePrefix {
            plan: Arc::clone(self),
            node: None,
            header: Some(identity),
            selection,
            use_name: Some(LocalTypeUse {
                name: name.name.clone(),
                range: name.range,
                context: name.context,
                edition: name.edition,
            }),
            module_selector,
            fingerprint: Digest([0; 32]),
        };
        checked!(prefix.exclusive_metadata_bytes());
        prefix.fingerprint = StableEncode::fingerprint(&prefix);
        QueryPoll::Ready(Ok(prefix))
    }
}

#[cfg(test)]
#[path = "declaration_field_type_tests.rs"]
mod tests;

#[path = "declaration_return_types.rs"]
mod returns;

#[path = "local_containers.rs"]
mod containers;
