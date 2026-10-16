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
        current_module_names::ModuleNameCompletion,
        hygiene::HygieneFacts,
        names::NameLookup,
        signature_names::SignatureName,
        source_body_context::PreparedBodyContext,
        source_types::{NamedTypePreparation, SourceType},
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
    range: SourceRange,
    tokens: Vec<TokenRecipe>,
    name: Option<LocalTypeUse>,
}
struct HeaderItem {
    scope: u32,
    declaration: SourceRange,
    header: Digest,
    generics: Vec<HeaderGeneric>,
    fields: Vec<HeaderField>,
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
            .field
            .name
            .as_ref()
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
        .filter(|n| *n <= 32 * 1024 * 1024)
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
        let mut fingerprint = StableEncoder::new("lrr/v1/local-declaration-field-Type-occurrences");
        fingerprint.digest(source.identity());
        fingerprint.u64(
            context
                .facts()
                .items
                .iter()
                .filter(|row| {
                    matches!(
                        row.item.def_key.item_kind,
                        ItemKind::Struct | ItemKind::Enum | ItemKind::Union
                    )
                })
                .count() as u64,
        );
        for row in &context.facts().items {
            if !matches!(
                row.item.def_key.item_kind,
                ItemKind::Struct | ItemKind::Enum | ItemKind::Union
            ) {
                continue;
            }
            let syntax = parsed
                .syntax_at(row.item.declaration_span, |n| ast::Item::can_cast(n.kind()))?
                .ok_or_else(|| error("declaration field owning syntax absent"))?;
            if tokens(&syntax, None) != row.item.header.tokens {
                return Err(error("declaration field header syntax differs"));
            }
            let definition = row.item.def_key.fingerprint();
            let mut generics = Vec::new();
            if let Some(list) = syntax.children().find_map(ast::GenericParamList::cast) {
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
            // Confined to this exact header, never a full-file/node-phase scan.
            for field in syntax.descendants().filter_map(|n| {
                if let Some(f) = ast::RecordField::cast(n.clone()) {
                    f.ty()
                } else {
                    ast::TupleField::cast(n).and_then(|f| f.ty())
                }
            }) {
                let field_range = range(field.syntax().text_range());
                let name = if let ast::Type::PathType(ty) = &field {
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
                fields.push(HeaderField {
                    range: field_range,
                    tokens: tokens(field.syntax(), None),
                    name,
                });
                if fields.len() > crate::frontend::types::MAX_SCHEMA_FIELDS {
                    return Err(crate::runtime::arena::resource());
                }
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
                fingerprint.u32(field.range.start);
                fingerprint.u32(field.range.end);
                fingerprint.option(&field.name, |e, n| {
                    e.string(n.name());
                    e.u16(n.edition());
                    e.digest(Digest(n.context().0));
                });
            }
            if items
                .insert(
                    definition,
                    HeaderItem {
                        scope: row.scope,
                        declaration: row.item.declaration_span,
                        header: row.item.header.fingerprint(),
                        generics,
                        fields,
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
        let mut matches = item
            .fields
            .iter()
            .filter(|row| row.range == field.type_range && row.tokens == field.ty);
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
        })
    }
}
impl PreparedSourceLocalPlan {
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
        let Some(name) = &header.field.name else {
            return QueryPoll::Ready(Ok(NamedTypePreparation::Name(
                SignatureName::NeedOwnerContext(schema.scope.clone()),
            )));
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
            field: Some((schema.definition, field.type_range)),
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
        if matches!(prefix.selection(), LocalTypePrefixSelection::Parameter(_)) {
            if let LocalTypePrefixSelection::Parameter(parameter) = prefix.selection() {
                return QueryPoll::Ready(Ok(NamedTypePreparation::Parameter(*parameter)));
            }
        }
        if matches!(prefix.selection(), LocalTypePrefixSelection::ModuleTail) {
            let observation = ready!(prefix.module_name(db));
            if let Some(observation) = observation {
                if observation.completion() == ModuleNameCompletion::Complete
                    && matches!(observation.lookup(), Some(NameLookup::Absent))
                {
                    let classified = checked!(
                        crate::frontend::signature_names::classify_completed_type_lookup(
                            name.name(),
                            name.edition(),
                            NameLookup::Absent
                        )
                    );
                    return match classified {
                        SignatureName::Primitive(p) => QueryPoll::Ready(
                            SourceType::selected_primitive(p).map(NamedTypePreparation::Ready),
                        ),
                        other => QueryPoll::Ready(Ok(NamedTypePreparation::Name(other))),
                    };
                }
            }
        }
        match ready!(super::type_declarations::prepare(db, prefix)) {
            super::type_declarations::BodyTypeOutcome::Ready(proof) => {
                QueryPoll::Ready(Ok(NamedTypePreparation::Ready(proof.source_type().clone())))
            }
            super::type_declarations::BodyTypeOutcome::Pending(demand) => {
                QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    format!("declaration field Type requires actual source authority: {demand:?}"),
                )))
            }
        }
    }
}

#[cfg(test)]
#[path = "declaration_field_type_tests.rs"]
mod tests;
