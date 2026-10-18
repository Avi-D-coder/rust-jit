//! Owned deferred nominal field recipes. They contain no guest handles or layouts.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    syntax::{
        ast_adapter::TokenRecipe,
        outline::{HeaderRecipe, ScopeKey, Visibility},
        signatures::{DefinitionParameter, GenericKind},
    },
    traits::term::TyTerm,
};
use std::{borrow::Cow, collections::BTreeMap};

#[path = "declaration_nominal_types.rs"]
mod nominal_header_types;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FieldShape {
    Unit,
    Tuple,
    Record,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FieldRecipe {
    /// None for positional fields. The vector ordinal is the storage ordinal.
    pub name: Option<String>,
    pub visibility: Visibility,
    pub ty: Vec<TokenRecipe>,
    /// Exact current parser Type occurrence, not part of nominal semantic identity.
    pub(crate) type_range: crate::syntax::spans::SourceRange,
    /// Captured syntax terms only: no name resolution, predicate or layout demand.
    pub symbolic: Option<TyTerm>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VariantRecipe {
    /// Structs have one unnamed variant; enum variants retain their source name.
    pub name: Option<String>,
    pub shape: FieldShape,
    pub fields: Vec<FieldRecipe>,
    pub discriminant: Vec<TokenRecipe>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NominalSchema {
    pub definition: Digest,
    pub scope: ScopeKey,
    pub header: HeaderRecipe,
    pub generics: Vec<DefinitionParameter>,
    pub variants: Vec<VariantRecipe>,
}
pub(crate) fn schema_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
pub(crate) fn schema_limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "deferred type schema exceeds its metadata budget".into(),
    )
}
impl VariantRecipe {
    /// Validates a reached constructor and retains written order independently
    /// of declaration-order storage. It evaluates no initializer.
    pub fn named_initializer_slots(&self, written: &[&str]) -> Result<Vec<u32>, Diagnostic> {
        // A unit struct also accepts the empty braced form (`S {}`, and the
        // `RangeFull` record of `..`); it has no field to name.
        if !matches!(
            self.shape,
            FieldShape::Record | FieldShape::Tuple | FieldShape::Unit
        ) {
            return Err(schema_fault("named initializer requires record fields"));
        }
        // A tuple struct or variant may also be initialized with braces, its
        // fields named by their decimal ordinals (`S { 0: a, 1: b }`).
        let ordinals = if self.shape == FieldShape::Tuple {
            (0..self.fields.len()).map(|n| n.to_string()).collect()
        } else {
            Vec::new()
        };
        let mut declared = BTreeMap::new();
        for (ordinal, field) in self.fields.iter().enumerate() {
            let name = match self.shape {
                FieldShape::Tuple => ordinals[ordinal].as_str(),
                _ => field
                    .name
                    .as_deref()
                    .ok_or_else(|| schema_fault("record field lacks its name"))?,
            };
            let ordinal = u32::try_from(ordinal).map_err(|_| schema_limit())?;
            if declared.insert(name, ordinal).is_some() {
                return Err(schema_fault("duplicate field in reached schema"));
            }
        }
        let mut initialized = Vec::new();
        initialized
            .try_reserve_exact(self.fields.len())
            .map_err(|_| schema_limit())?;
        initialized.resize(self.fields.len(), false);
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(written.len())
            .map_err(|_| schema_limit())?;
        for name in written {
            let ordinal = *declared
                .get(name)
                .ok_or_else(|| schema_fault("initializer field is absent from reached schema"))?;
            let flag = &mut initialized[ordinal as usize];
            if *flag {
                return Err(schema_fault("duplicate field in reached initializer"));
            }
            *flag = true;
            slots.push(ordinal);
        }
        if initialized.iter().any(|initialized| !initialized) {
            return Err(schema_fault("missing field in reached initializer"));
        }
        Ok(slots)
    }
}
impl NominalSchema {
    /// Declaration-order discriminant table of a fieldless-castable enum. An
    /// omitted discriminant is its predecessor plus one (zero for the first).
    /// Only an integer literal, optionally negated, is decoded here; any other
    /// discriminant expression needs the pure constant evaluator and yields
    /// `None` for that variant and every implicit successor.
    pub(crate) fn literal_discriminants(
        &self,
    ) -> Result<std::sync::Arc<[Option<i128>]>, Diagnostic> {
        let mut table = Vec::new();
        table
            .try_reserve_exact(self.variants.len())
            .map_err(|_| schema_limit())?;
        let mut previous: Option<Option<i128>> = None;
        for variant in &self.variants {
            let value = if variant.discriminant.is_empty() {
                match previous {
                    None => Some(0),
                    Some(previous) => previous.and_then(|value| value.checked_add(1)),
                }
            } else {
                literal_discriminant(&variant.discriminant)
            };
            table.push(value);
            previous = Some(value);
        }
        Ok(table.into())
    }
    /// The variant at or before `variant` whose discriminant is an explicit
    /// expression that is not a plain integer literal (E-R5-013). When
    /// `literal_discriminants` has no value for `variant`, every entry from
    /// this anchor up to it is implicit, so `variant`'s discriminant is the
    /// anchor's evaluated value plus the distance.
    pub(crate) fn expression_anchor(&self, variant: usize) -> Option<usize> {
        let last = variant.min(self.variants.len().checked_sub(1)?);
        (0..=last).rev().find(|&index| {
            let tokens = &self.variants[index].discriminant;
            !tokens.is_empty() && literal_discriminant(tokens).is_none()
        })
    }
    /// A discriminant table that knows only the anchor's evaluated `value`
    /// and its implicit successors up to the next explicit discriminant;
    /// the store merges it into the descriptor's table.
    pub(crate) fn anchored_discriminants(
        &self,
        anchor: usize,
        value: i128,
    ) -> std::sync::Arc<[Option<i128>]> {
        let mut table = vec![None; self.variants.len()];
        table[anchor] = Some(value);
        for index in anchor + 1..self.variants.len() {
            if !self.variants[index].discriminant.is_empty() {
                break;
            }
            table[index] = table[index - 1].and_then(|previous| previous.checked_add(1));
        }
        table.into()
    }
    pub fn variant(&self, name: Option<&str>) -> Result<&VariantRecipe, Diagnostic> {
        let mut selected = self
            .variants
            .iter()
            .filter(|variant| variant.name.as_deref() == name);
        let result = selected
            .next()
            .ok_or_else(|| schema_fault("reached variant is absent"))?;
        if selected.next().is_some() {
            return Err(schema_fault("reached variant is ambiguous"));
        }
        Ok(result)
    }
    pub fn estimated_bytes(&self) -> usize {
        self.checked_bytes().unwrap_or(usize::MAX)
    }
    fn checked_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.scope.heap_bytes())?
            .checked_add(self.header.estimated_bytes())?
            .checked_add(
                self.generics
                    .capacity()
                    .checked_mul(std::mem::size_of::<DefinitionParameter>())?,
            )?
            .checked_add(
                self.variants
                    .capacity()
                    .checked_mul(std::mem::size_of::<VariantRecipe>())?,
            )?;
        for parameter in &self.generics {
            bytes = bytes.checked_add(parameter.name.capacity())?;
            for tokens in [&parameter.bounds, &parameter.default, &parameter.const_type] {
                bytes = bytes.checked_add(token_bytes(tokens)?)?;
            }
        }
        for variant in &self.variants {
            bytes = bytes
                .checked_add(variant.name.as_ref().map_or(0, String::capacity))?
                .checked_add(token_bytes(&variant.discriminant)?)?
                .checked_add(
                    variant
                        .fields
                        .capacity()
                        .checked_mul(std::mem::size_of::<FieldRecipe>())?,
                )?;
            for field in &variant.fields {
                bytes = bytes
                    .checked_add(field.name.as_ref().map_or(0, String::capacity))?
                    .checked_add(token_bytes(&field.ty)?)?
                    .checked_add(crate::syntax::imports::visibility_bytes(&field.visibility))?
                    .checked_add(
                        field
                            .symbolic
                            .as_ref()
                            .map_or(Some(0), TyTerm::heap_bytes)?,
                    )?;
            }
        }
        Some(bytes)
    }
}
/// Field-name interpretation uses the declaration's source namespace and generic
/// context. A captured primitive spelling is not proof against source shadowing.
pub struct SchemaNames<'a> {
    graph: &'a super::names::NameNamespaces,
    schema: &'a NominalSchema,
    definition: Cow<'a, crate::source::manifest::DefKey>,
    body_type: Option<&'a super::source_locals::owned_plan::type_declarations::PreparedBodyType>,
    body_owner: Option<super::source_types::LocalNominalOwner>,
}
/// One lookup result and, when available, its source-selected concrete type.
/// Metadata-only graph callers cannot mint nominal schema authority.
struct SchemaTypeName {
    name: Option<super::signature_names::SignatureName>,
    concrete: Option<super::source_types::SourceType>,
    /// A compound field Type selected leaf by leaf.
    written: Option<crate::ir::written_type::WrittenType>,
}
/// One field-Type demand of the shared binder: a written name (the
/// declaration's own name, or a single-name field Type), or a compound
/// field Type.
#[derive(Clone, Copy)]
enum FieldTypeQuery<'q> {
    Name(&'q str, Option<&'q FieldRecipe>),
    Compound(&'q FieldRecipe),
}
impl SchemaTypeName {
    fn metadata(name: super::signature_names::SignatureName) -> Result<Self, Diagnostic> {
        let concrete = if let super::signature_names::SignatureName::Primitive(p) = &name {
            Some(super::source_types::SourceType::selected_primitive(*p)?)
        } else {
            None
        };
        Ok(Self {
            name: Some(name),
            concrete,
            written: None,
        })
    }
    /// A compound field Type the metadata graph cannot select.
    fn deferred() -> Self {
        Self {
            name: None,
            concrete: None,
            written: None,
        }
    }
    fn from_preparation(
        prepared: super::source_types::NamedTypePreparation,
    ) -> Result<Self, Diagnostic> {
        use super::{signature_names::SignatureName, source_types::NamedTypePreparation};
        Ok(match prepared {
            NamedTypePreparation::Ready(concrete) => Self {
                name: None,
                concrete: Some(concrete),
                written: None,
            },
            NamedTypePreparation::Name(name) => Self {
                name: Some(name),
                concrete: None,
                written: None,
            },
            NamedTypePreparation::Parameter(parameter) => Self {
                name: Some(SignatureName::Parameter(parameter)),
                concrete: None,
                written: None,
            },
            NamedTypePreparation::Alias(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "field type requires further source alias binding".into(),
                ));
            }
        })
    }
}
impl<'a> SchemaNames<'a> {
    fn current_name(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        name: &str,
        field: Option<&FieldRecipe>,
    ) -> crate::db::poll::QueryPoll<SchemaTypeName> {
        use crate::db::poll::QueryPoll;
        if let Some(body_type) = self.body_type
            && field.is_none()
            && name == self.definition.declared_name
        {
            let declaration = body_type.declaration();
            return QueryPoll::Ready(Ok(SchemaTypeName {
                name: Some(super::signature_names::SignatureName::Source(
                    super::names::NameDefinition {
                        namespace: crate::source::manifest::Namespace::Type,
                        identity: declaration.definition_identity(),
                        defining_unit: declaration.definition().crate_key,
                        definition_edition: declaration.token_edition(),
                    },
                )),
                concrete: Some(body_type.source_type().clone()),
                written: None,
            }));
        }
        if let Some(field) = field
            && self.schema.scope.owner.is_some()
            && let Some(plan) = self
                .body_type
                .map(|ty| ty.declaration().retained_plan())
                .or_else(|| self.body_owner.as_ref().map(|owner| &owner.plan))
        {
            return match plan.prepare_field_type(db, self.schema, field) {
                QueryPoll::Ready(Ok(prepared)) => {
                    QueryPoll::Ready(SchemaTypeName::from_preparation(prepared))
                }
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
        }
        if let Some(owner) = &self.body_owner {
            if field.is_none() && name == self.definition.declared_name {
                return QueryPoll::Ready(Ok(SchemaTypeName {
                    name: Some(super::signature_names::SignatureName::Source(
                        super::names::NameDefinition {
                            namespace: crate::source::manifest::Namespace::Type,
                            identity: self.schema.definition,
                            defining_unit: owner.definition.crate_key,
                            definition_edition: owner.edition,
                        },
                    )),
                    concrete: None,
                    written: None,
                }));
            }
        }
        let selected = match super::type_name_namespace::resolve_declaration_name(
            db,
            super::type_name_namespace::SourceTypeOwner {
                definition: self.schema.definition,
                scope: &self.schema.scope,
                header: &self.schema.header,
            },
            &self.schema.generics,
            sources,
            name,
        ) {
            QueryPoll::Ready(Ok(selected)) => selected,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        // This is the same selected-name/alias/schema kernel used by source
        // parameters and reference pointees. It does not bind child fields.
        let (concrete, alias) = match super::source_types::prepare_selected(
            db,
            self.graph,
            sources,
            selected.clone(),
        ) {
            QueryPoll::Ready(Ok(super::source_types::NamedTypePreparation::Ready(proof))) => {
                (Some(proof), None)
            }
            // An alias of a generic application (`type R = Result<(), E>`)
            // is its target's structure, written leaf by leaf.
            QueryPoll::Ready(Ok(super::source_types::NamedTypePreparation::Alias(
                super::type_aliases::AliasTarget::Type(term),
            ))) => (None, Some(term)),
            QueryPoll::Ready(Ok(_)) => (None, None),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let written = match alias {
            Some(term) => match super::written_terms::written_template(
                db,
                sources,
                &term,
                self.schema.definition,
            ) {
                QueryPoll::Ready(Ok(written)) => written,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            },
            None => None,
        };
        QueryPoll::Ready(Ok(SchemaTypeName {
            name: Some(selected),
            concrete,
            written,
        }))
    }
    fn current_query(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        query: FieldTypeQuery<'_>,
    ) -> crate::db::poll::QueryPoll<SchemaTypeName> {
        match query {
            FieldTypeQuery::Name(name, field) => self.current_name(db, sources, name, field),
            FieldTypeQuery::Compound(field) => match self.current_compound(db, sources, field) {
                crate::db::poll::QueryPoll::Ready(Ok(written)) => {
                    crate::db::poll::QueryPoll::Ready(Ok(SchemaTypeName {
                        written,
                        ..SchemaTypeName::deferred()
                    }))
                }
                crate::db::poll::QueryPoll::Ready(Err(error)) => {
                    crate::db::poll::QueryPoll::Ready(Err(error))
                }
                crate::db::poll::QueryPoll::NeedInput(input) => {
                    crate::db::poll::QueryPoll::NeedInput(input)
                }
            },
        }
    }
    /// A compound field Type (`&'a T`, `(i32, P)`, `[u8; 3]`, `W<T>`) of
    /// this declaration: its written structure around named leaves, each
    /// selected by the same declaration-context Type service as a
    /// single-name field (`current_name`). `None` where a part needs a further
    /// source integration (projections, qualified paths, trait objects, const
    /// lengths, function pointers, body-local declarations).
    fn current_compound(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        field: &FieldRecipe,
    ) -> crate::db::poll::QueryPoll<Option<crate::ir::written_type::WrittenType>> {
        use crate::db::poll::QueryPoll;
        if self.schema.scope.owner.is_some() {
            return self.local_compound(db, sources, field);
        }
        let term = match self.actual_field_type(db, sources, field) {
            QueryPoll::Ready(Ok(Some(term))) => term,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.written_field_type(db, sources, &term)
    }
    /// A body-local declaration's compound field Type, selected by its owning
    /// body plan's header service; its own type parameters become their
    /// declaration indexes.
    fn local_compound(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        field: &FieldRecipe,
    ) -> crate::db::poll::QueryPoll<Option<crate::ir::written_type::WrittenType>> {
        use crate::db::poll::QueryPoll;
        let Some(plan) = self
            .body_type
            .map(|ty| ty.declaration().retained_plan())
            .or_else(|| self.body_owner.as_ref().map(|owner| &owner.plan))
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let term = match plan.prepare_field_type(db, self.schema, field) {
            QueryPoll::Ready(Ok(super::source_types::NamedTypePreparation::Alias(
                super::type_aliases::AliasTarget::Type(term),
            ))) => term,
            QueryPoll::Ready(Ok(_)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.written_field_type(db, sources, &term)
    }
    fn written_field_type(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        term: &TyTerm,
    ) -> crate::db::poll::QueryPoll<Option<crate::ir::written_type::WrittenType>> {
        use crate::{db::poll::QueryPoll, ir::written_type::WrittenType};
        let written =
            match super::written_terms::written_template(db, sources, term, self.schema.definition)
            {
                QueryPoll::Ready(Ok(Some(written))) => written,
                QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        // Generic ordinals become indexes among the type parameters; a part
        // left to inference is not a field Type.
        fn index(written: WrittenType, schema: &NominalSchema) -> Option<WrittenType> {
            Some(match written {
                WrittenType::Parameter(ordinal) => WrittenType::Parameter(
                    schema
                        .generics
                        .iter()
                        .filter(|parameter| parameter.kind == GenericKind::Type)
                        .position(|parameter| parameter.ordinal == ordinal)
                        .and_then(|index| u32::try_from(index).ok())?,
                ),
                WrittenType::Infer | WrittenType::Dyn(_) => return None,
                WrittenType::Tuple(members) => WrittenType::Tuple(
                    members
                        .into_iter()
                        .map(|member| index(member, schema))
                        .collect::<Option<_>>()?,
                ),
                WrittenType::Array(element, len) => {
                    WrittenType::Array(Box::new(index(*element, schema)?), len)
                }
                WrittenType::Slice(element) => {
                    WrittenType::Slice(Box::new(index(*element, schema)?))
                }
                WrittenType::Ptr { mutable, target } => WrittenType::Ptr {
                    mutable,
                    target: Box::new(index(*target, schema)?),
                },
                WrittenType::FnPtr { params, result } => WrittenType::FnPtr {
                    params: params
                        .into_iter()
                        .map(|param| index(param, schema))
                        .collect::<Option<_>>()?,
                    result: Box::new(index(*result, schema)?),
                },
                WrittenType::Ref {
                    region,
                    mutable,
                    target,
                } => WrittenType::Ref {
                    region,
                    mutable,
                    target: Box::new(index(*target, schema)?),
                },
                WrittenType::Generic {
                    identity,
                    definition,
                    schema: fingerprint,
                    args,
                } => WrittenType::Generic {
                    identity,
                    definition,
                    schema: fingerprint,
                    args: args
                        .into_iter()
                        .map(|arg| index(arg, schema))
                        .collect::<Option<_>>()?,
                },
                other => other,
            })
        }
        QueryPoll::Ready(Ok(index(written, self.schema)))
    }
    pub fn from_database(
        db: &mut crate::db::memo::Database,
        graph: &'a super::names::NameNamespaces,
        source: &'a crate::source::changes::PreparedSourceView,
        schema: &'a NominalSchema,
    ) -> crate::db::poll::QueryPoll<Self> {
        use crate::db::{
            dependency::{InputKey, InputValue},
            poll::QueryPoll,
        };
        if let Err(error) = graph.require_view(source) {
            return QueryPoll::Ready(Err(error));
        }
        if !matches!(db.input(&InputKey::DefinitionSource(schema.definition)).as_deref(), Some(InputValue::DefinitionSource(parse)) if parse == &source.selection().outline.parse)
            || !source.outline().items.iter().any(|item| {
                item.def_key.fingerprint() == schema.definition
                    && item.scope == schema.scope
                    && item.header == schema.header
            })
        {
            return QueryPoll::Ready(Err(schema_fault(
                "field schema/source namespace revision mismatch",
            )));
        }
        let Some(item) = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == schema.definition)
        else {
            return QueryPoll::Ready(Err(schema_fault("source definition is absent")));
        };
        Self::from_definition(db, graph, schema, Cow::Borrowed(&item.def_key))
    }
    /// Current ordinary member preparation uses the same canonical source map
    /// as named/reference Type preparation. This constructs no runtime binding;
    /// the current declaration Type callback must still admit selected names.
    pub(crate) fn from_current_database(
        db: &mut crate::db::memo::Database,
        graph: &'a super::names::NameNamespaces,
        sources: &[&crate::source::changes::PreparedSourceView],
        schema: &'a NominalSchema,
    ) -> crate::db::poll::QueryPoll<Self> {
        use crate::db::poll::QueryPoll;
        for source in sources {
            if let Err(error) = graph.require_view(source) {
                return QueryPoll::Ready(Err(error));
            }
        }
        let local = match super::source_types::local_nominal_owner(db, schema.definition) {
            QueryPoll::Ready(Ok(owner)) => owner,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        if let Some(owner) = local {
            return match Self::from_definition(
                db,
                graph,
                schema,
                Cow::Owned(owner.definition.clone()),
            ) {
                QueryPoll::Ready(Ok(mut names)) => {
                    names.body_owner = Some(owner);
                    QueryPoll::Ready(Ok(names))
                }
                other => other,
            };
        }
        let definition =
            match super::source_types::nominal_definition(db, sources, schema.definition) {
                Ok(definition) => definition,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        Self::from_definition(db, graph, schema, Cow::Owned(definition))
    }
    /// Same schema binder after the actual entered Type occurrence selected its
    /// local or module declaration. Module outlines are not local-item authority.
    pub(crate) fn from_body_type(
        db: &mut crate::db::memo::Database,
        graph: &'a super::names::NameNamespaces,
        schema: &'a NominalSchema,
        body_type: &'a super::source_locals::owned_plan::type_declarations::PreparedBodyType,
    ) -> crate::db::poll::QueryPoll<Self> {
        use crate::db::poll::QueryPoll;
        let declaration = body_type.declaration();
        match declaration.retained_plan().require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        match declaration.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        if declaration.definition_identity() != schema.definition {
            return QueryPoll::Ready(Err(schema_fault(
                "entered Type declaration differs from selected field schema",
            )));
        }
        match Self::from_definition(
            db,
            graph,
            schema,
            Cow::Owned(declaration.definition().clone()),
        ) {
            QueryPoll::Ready(Ok(mut names)) => {
                names.body_type = Some(body_type);
                QueryPoll::Ready(Ok(names))
            }
            other => other,
        }
    }
    fn from_definition(
        db: &mut crate::db::memo::Database,
        graph: &'a super::names::NameNamespaces,
        schema: &'a NominalSchema,
        definition: Cow<'a, crate::source::manifest::DefKey>,
    ) -> crate::db::poll::QueryPoll<Self> {
        use crate::db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        };
        if definition.namespace != crate::source::manifest::Namespace::Type
            || definition.crate_key != schema.scope.crate_key
            || definition.module_path != schema.scope.module_path
            || !matches!(
                definition.item_kind,
                crate::source::manifest::ItemKind::Struct | crate::source::manifest::ItemKind::Enum
            )
        {
            return QueryPoll::Ready(Err(schema_fault(
                "nominal schema canonical declaration differs",
            )));
        }
        if !matches!(db.input(&InputKey::DefinitionHeader(schema.definition)).as_deref(), Some(InputValue::DefinitionHeader(header)) if header == &schema.header)
            || !matches!(db.input(&InputKey::DeclarationContext(schema.definition)).as_deref(), Some(InputValue::DeclarationContext(context)) if context.scope == schema.scope)
            || !matches!(db.input(&InputKey::DefinitionSource(schema.definition)).as_deref(), Some(InputValue::DefinitionSource(source)) if source.profile == schema.header.profile)
        {
            return QueryPoll::Ready(Err(schema_fault(
                "field schema/database declaration context mismatch",
            )));
        }
        // Demand only the selected schema, never a body, trait goal or expansion
        // effect. Suspended discovery propagates without caching an incomplete view.
        let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(schema.definition)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        if !matches!(captured.value.as_ref(), QueryValue::TypeSchema(actual) if actual.as_ref() == schema)
        {
            return QueryPoll::Ready(Err(schema_fault(
                "field schema is stale or differs from its captured recipe",
            )));
        }
        QueryPoll::Ready(Ok(Self {
            graph,
            schema,
            definition,
            body_type: None,
            body_owner: None,
        }))
    }
    pub(crate) fn definition(&self) -> &crate::source::manifest::DefKey {
        &self.definition
    }
    pub fn unqualified(
        &self,
        name: &str,
        prelude: Option<&ScopeKey>,
    ) -> Result<super::signature_names::SignatureName, Diagnostic> {
        super::signature_names::declaration_type_name(
            self.graph,
            self.schema.definition,
            &self.schema.scope,
            self.schema.header.profile.edition,
            &self.schema.generics,
            name,
            prelude,
        )
    }
    pub fn qualified(
        &self,
        path: &super::names::NamePath,
    ) -> Result<super::signature_names::SignatureName, Diagnostic> {
        use super::{names::NameLookup, signature_names::SignatureName};
        if self.schema.scope.owner.is_some() {
            return Ok(SignatureName::NeedOwnerContext(self.schema.scope.clone()));
        }
        Ok(
            match self.graph.lookup(
                crate::source::manifest::Namespace::Type,
                path,
                &self.schema.scope,
            )? {
                NameLookup::Defined(definition) => SignatureName::Source(definition),
                other => SignatureName::Unresolved(other),
            },
        )
    }

    /// Type and lifetime parameters only; a const generic still needs its
    /// separately evaluated argument.
    fn generics_supported(&self) -> bool {
        self.schema
            .generics
            .iter()
            .all(|parameter| parameter.kind != GenericKind::Const)
    }
    fn type_parameter_count(&self) -> Result<u32, Diagnostic> {
        u32::try_from(
            self.schema
                .generics
                .iter()
                .filter(|parameter| parameter.kind == GenericKind::Type)
                .count(),
        )
        .map_err(|_| schema_limit())
    }
    /// The type-parameter index of a field written as a bare parameter of this
    /// declaration (`x: T`). Its captured symbolic term names the parameter by
    /// declaration identity, never by spelling.
    fn generic_field(&self, recipe: &FieldRecipe) -> Option<u32> {
        let Some(TyTerm::Param(key)) = &recipe.symbolic else {
            return None;
        };
        if key.definition != self.schema.definition {
            return None;
        }
        self.schema
            .generics
            .iter()
            .filter(|parameter| parameter.kind == GenericKind::Type)
            .position(|parameter| parameter.ordinal == key.ordinal)
            .and_then(|index| u32::try_from(index).ok())
    }
    /// Bind only the reached variant. A pending source name remains a demand;
    /// captured spelling never establishes primitive or nominal identity.
    /// Alias, generic and compound-field resolution continue through the source
    /// type engine rather than being guessed from a guest descriptor.
    pub fn bind_variant(&self, name: Option<&str>) -> Result<NominalBinding, Diagnostic> {
        match self.bind_variant_with(name, |query| match query {
            FieldTypeQuery::Name(name, _) => crate::db::poll::QueryPoll::Ready(
                self.unqualified(name, None)
                    .and_then(SchemaTypeName::metadata),
            ),
            FieldTypeQuery::Compound(_) => {
                crate::db::poll::QueryPoll::Ready(Ok(SchemaTypeName::deferred()))
            }
        })? {
            crate::db::poll::QueryPoll::Ready(result) => result,
            crate::db::poll::QueryPoll::NeedInput(_) => {
                Err(schema_fault("graph variant lookup unexpectedly suspended"))
            }
        }
    }
    fn bind_variant_with(
        &self,
        name: Option<&str>,
        mut lookup: impl FnMut(FieldTypeQuery<'_>) -> crate::db::poll::QueryPoll<SchemaTypeName>,
    ) -> Result<crate::db::poll::QueryPoll<NominalBinding>, Diagnostic> {
        use super::signature_names::SignatureName;
        let selected = self.schema.variant(name)?;
        let own_name = match lookup(FieldTypeQuery::Name(&self.definition.declared_name, None)) {
            crate::db::poll::QueryPoll::Ready(result) => result?,
            crate::db::poll::QueryPoll::NeedInput(input) => {
                return Ok(crate::db::poll::QueryPoll::NeedInput(input));
            }
        };
        if !matches!(&own_name.name, Some(SignatureName::Source(found)) if found.identity == self.schema.definition)
        {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalBinding::Deferred {
                    field: None,
                    demand: NominalBindingDemand::Name(own_name.name.ok_or_else(|| {
                        schema_fault("own nominal lookup lacks its selected identity")
                    })?),
                },
            )));
        }
        if !self.generics_supported() {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalBinding::Deferred {
                    field: None,
                    demand: NominalBindingDemand::GenericContext,
                },
            )));
        }
        let variant = self
            .schema
            .variants
            .iter()
            .position(|v| v.name.as_deref() == name)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(schema_limit)?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(selected.fields.len())
            .map_err(|_| schema_limit())?;
        let mut field_bytes = fields
            .capacity()
            .checked_mul(std::mem::size_of::<NominalFieldType>())
            .ok_or_else(schema_limit)?;
        for (ordinal, recipe) in selected.fields.iter().enumerate() {
            let field = Some(u32::try_from(ordinal).map_err(|_| schema_limit())?);
            if let Some(parameter) = self.generic_field(recipe) {
                fields.push(NominalFieldType::Parameter(parameter));
                continue;
            }
            let resolved = if recipe.ty.len() == 1 {
                match lookup(FieldTypeQuery::Name(&recipe.ty[0].text, Some(recipe))) {
                    crate::db::poll::QueryPoll::Ready(result) => result?,
                    crate::db::poll::QueryPoll::NeedInput(input) => {
                        return Ok(crate::db::poll::QueryPoll::NeedInput(input));
                    }
                }
            } else if matches!(
                recipe.symbolic,
                Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit))
            ) {
                SchemaTypeName::metadata(SignatureName::Primitive(
                    crate::ir::op::PrimitiveType::Unit,
                ))?
            } else {
                let compound = match lookup(FieldTypeQuery::Compound(recipe)) {
                    crate::db::poll::QueryPoll::Ready(result) => result?,
                    crate::db::poll::QueryPoll::NeedInput(input) => {
                        return Ok(crate::db::poll::QueryPoll::NeedInput(input));
                    }
                };
                let Some(written) = compound.written else {
                    return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                        NominalBinding::Deferred {
                            field,
                            demand: NominalBindingDemand::TypeRecipe,
                        },
                    )));
                };
                let field = NominalFieldType::Compound(std::sync::Arc::new(written));
                field_bytes = field_bytes
                    .checked_add(field.heap_bytes()?)
                    .ok_or_else(schema_limit)?;
                fields.push(field);
                continue;
            };
            let Some(concrete) = resolved.concrete else {
                if let Some(written) = resolved.written {
                    let field = NominalFieldType::Compound(std::sync::Arc::new(written));
                    field_bytes = field_bytes
                        .checked_add(field.heap_bytes()?)
                        .ok_or_else(schema_limit)?;
                    fields.push(field);
                    continue;
                }
                return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                    NominalBinding::Deferred {
                        field,
                        demand: NominalBindingDemand::Name(resolved.name.ok_or_else(|| {
                            schema_fault("deferred field lookup lacks its demand")
                        })?),
                    },
                )));
            };
            field_bytes = field_bytes
                .checked_add(concrete.heap_bytes()?)
                .ok_or_else(schema_limit)?;
            fields.push(NominalFieldType::Concrete(concrete));
        }
        let discriminants = if self.definition.item_kind == crate::source::manifest::ItemKind::Enum
        {
            self.schema.literal_discriminants()?
        } else {
            std::sync::Arc::from(Vec::new())
        };
        Ok(crate::db::poll::QueryPoll::Ready(Ok(
            NominalBinding::Ready(BoundNominalVariant {
                identity: self.schema.definition,
                definition: self.definition.as_ref().clone(),
                schema: self.schema.fingerprint(),
                variant,
                fields,
                type_parameters: self.type_parameter_count()?,
                discriminants,
            }),
        )))
    }
    /// Reached constructor preparation (unit/tuple structs and enum variants
    /// selected through the Value namespace) binds the same selected variant as
    /// record construction, with field Types admitted by the current
    /// declaration service.
    pub(crate) fn bind_variant_current(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        name: Option<&str>,
    ) -> crate::db::poll::QueryPoll<NominalBinding> {
        match self.bind_variant_with(name, |query| self.current_query(db, sources, query)) {
            Ok(poll) => poll,
            Err(error) => crate::db::poll::QueryPoll::Ready(Err(error)),
        }
    }
    fn field_visibility<'b>(&'b self, declared: &'b Visibility) -> &'b Visibility {
        if self.definition.item_kind == crate::source::manifest::ItemKind::Enum {
            &self.schema.header.visibility
        } else {
            declared
        }
    }
    /// Select only the reached named field from the declaration's captured
    /// namespace. Unused field recipes are not bound, and privacy is checked
    /// before asking even the selected field's type name.
    pub fn bind_named_field(
        &self,
        variant_name: Option<&str>,
        member: &str,
        caller: &ScopeKey,
    ) -> Result<NominalFieldBinding, Diagnostic> {
        match self.bind_named_field_with(variant_name, member, caller, |query| match query {
            FieldTypeQuery::Name(name, _) => crate::db::poll::QueryPoll::Ready(
                self.unqualified(name, None)
                    .and_then(SchemaTypeName::metadata),
            ),
            FieldTypeQuery::Compound(_) => {
                crate::db::poll::QueryPoll::Ready(Ok(SchemaTypeName::deferred()))
            }
        })? {
            crate::db::poll::QueryPoll::Ready(result) => result,
            crate::db::poll::QueryPoll::NeedInput(_) => {
                Err(schema_fault("graph field lookup unexpectedly suspended"))
            }
        }
    }
    /// Reached ordinary field preparation uses the current source Type service
    /// for its own nominal identity and the single selected member type. The
    /// metadata graph adapter above shares this exact structural/privacy binder.
    pub(crate) fn bind_named_field_current(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        variant_name: Option<&str>,
        member: &str,
        caller: &ScopeKey,
    ) -> crate::db::poll::QueryPoll<NominalFieldBinding> {
        let outcome = self.bind_named_field_with(variant_name, member, caller, |query| {
            self.current_query(db, sources, query)
        });
        match outcome {
            Ok(poll) => poll,
            Err(error) => crate::db::poll::QueryPoll::Ready(Err(error)),
        }
    }
    /// The positional member `ordinal` of a tuple struct or tuple variant
    /// (`t.0`): the same structural/privacy binder as a named member.
    pub(crate) fn bind_positional_field_current(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        variant_name: Option<&str>,
        ordinal: u32,
        caller: &ScopeKey,
    ) -> crate::db::poll::QueryPoll<NominalFieldBinding> {
        let outcome = self.bind_field_with(
            variant_name,
            FieldSelector::Positional(ordinal),
            caller,
            |query| self.current_query(db, sources, query),
        );
        match outcome {
            Ok(poll) => poll,
            Err(error) => crate::db::poll::QueryPoll::Ready(Err(error)),
        }
    }
    fn bind_named_field_with(
        &self,
        variant_name: Option<&str>,
        member: &str,
        caller: &ScopeKey,
        lookup: impl FnMut(FieldTypeQuery<'_>) -> crate::db::poll::QueryPoll<SchemaTypeName>,
    ) -> Result<crate::db::poll::QueryPoll<NominalFieldBinding>, Diagnostic> {
        self.bind_field_with(variant_name, FieldSelector::Named(member), caller, lookup)
    }
    fn bind_field_with(
        &self,
        variant_name: Option<&str>,
        member: FieldSelector<'_>,
        caller: &ScopeKey,
        mut lookup: impl FnMut(FieldTypeQuery<'_>) -> crate::db::poll::QueryPoll<SchemaTypeName>,
    ) -> Result<crate::db::poll::QueryPoll<NominalFieldBinding>, Diagnostic> {
        use super::signature_names::SignatureName;
        let selected = self.schema.variant(variant_name)?;
        let (ordinal, recipe) = match member {
            FieldSelector::Named(member) => {
                if selected.shape != FieldShape::Record {
                    return Err(schema_fault("named field requires a record variant"));
                }
                let member = super::names::owned_name(member, self.schema.header.profile.edition)?;
                let mut matching = selected
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, field)| field.name.as_deref() == Some(member.as_str()));
                let found = matching
                    .next()
                    .ok_or_else(|| schema_fault("reached source field is absent"))?;
                if matching.next().is_some() {
                    return Err(schema_fault("reached source field is ambiguous"));
                }
                found
            }
            FieldSelector::Positional(ordinal) => {
                if selected.shape != FieldShape::Tuple {
                    return Err(schema_fault("positional field requires a tuple variant"));
                }
                let recipe = selected
                    .fields
                    .get(ordinal as usize)
                    .ok_or_else(|| schema_fault("reached source field is absent"))?;
                (ordinal as usize, recipe)
            }
        };
        let field = u32::try_from(ordinal).map_err(|_| schema_limit())?;
        if !crate::syntax::visibility::VisibilityScope::for_declaration(
            self.field_visibility(&recipe.visibility),
            &self.schema.scope,
            self.schema.header.profile.edition,
        )?
        .allows(caller)?
        {
            return Err(schema_fault("reached source field is inaccessible"));
        }
        let own_name = match lookup(FieldTypeQuery::Name(&self.definition.declared_name, None)) {
            crate::db::poll::QueryPoll::Ready(Ok(selected)) => selected,
            crate::db::poll::QueryPoll::Ready(Err(error)) => return Err(error),
            crate::db::poll::QueryPoll::NeedInput(request) => {
                return Ok(crate::db::poll::QueryPoll::NeedInput(request));
            }
        };
        if !matches!(&own_name.name, Some(SignatureName::Source(found)) if found.identity == self.schema.definition)
        {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::Name(own_name.name.ok_or_else(|| {
                        schema_fault("own nominal lookup lacks its selected identity")
                    })?),
                },
            )));
        }
        if !self.generics_supported() {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::GenericContext,
                },
            )));
        }
        let variant = self
            .schema
            .variants
            .iter()
            .position(|variant| variant.name.as_deref() == variant_name)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(schema_limit)?;
        if let Some(parameter) = self.generic_field(recipe) {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Ready(BoundNominalField {
                    identity: self.schema.definition,
                    definition: self.definition.as_ref().clone(),
                    schema: self.schema.fingerprint(),
                    variant,
                    field,
                    field_type: NominalFieldType::Parameter(parameter),
                }),
            )));
        }
        let resolved = if recipe.ty.len() == 1 {
            match lookup(FieldTypeQuery::Name(&recipe.ty[0].text, Some(recipe))) {
                crate::db::poll::QueryPoll::Ready(Ok(selected)) => selected,
                crate::db::poll::QueryPoll::Ready(Err(error)) => return Err(error),
                crate::db::poll::QueryPoll::NeedInput(request) => {
                    return Ok(crate::db::poll::QueryPoll::NeedInput(request));
                }
            }
        } else if matches!(
            recipe.symbolic,
            Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit))
        ) {
            SchemaTypeName::metadata(SignatureName::Primitive(crate::ir::op::PrimitiveType::Unit))?
        } else {
            let compound = match lookup(FieldTypeQuery::Compound(recipe)) {
                crate::db::poll::QueryPoll::Ready(Ok(selected)) => selected,
                crate::db::poll::QueryPoll::Ready(Err(error)) => return Err(error),
                crate::db::poll::QueryPoll::NeedInput(request) => {
                    return Ok(crate::db::poll::QueryPoll::NeedInput(request));
                }
            };
            let Some(written) = compound.written else {
                return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                    NominalFieldBinding::Deferred {
                        field,
                        demand: NominalBindingDemand::TypeRecipe,
                    },
                )));
            };
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Ready(BoundNominalField {
                    identity: self.schema.definition,
                    definition: self.definition.as_ref().clone(),
                    schema: self.schema.fingerprint(),
                    variant,
                    field,
                    field_type: NominalFieldType::Compound(std::sync::Arc::new(written)),
                }),
            )));
        };
        let Some(source_type) = resolved.concrete else {
            if let Some(written) = resolved.written {
                return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                    NominalFieldBinding::Ready(BoundNominalField {
                        identity: self.schema.definition,
                        definition: self.definition.as_ref().clone(),
                        schema: self.schema.fingerprint(),
                        variant,
                        field,
                        field_type: NominalFieldType::Compound(std::sync::Arc::new(written)),
                    }),
                )));
            }
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::Name(
                        resolved.name.ok_or_else(|| {
                            schema_fault("deferred field lookup lacks its demand")
                        })?,
                    ),
                },
            )));
        };
        Ok(crate::db::poll::QueryPoll::Ready(Ok(
            NominalFieldBinding::Ready(BoundNominalField {
                identity: self.schema.definition,
                definition: self.definition.as_ref().clone(),
                schema: self.schema.fingerprint(),
                variant,
                field,
                field_type: NominalFieldType::Concrete(source_type),
            }),
        )))
    }
    /// Bind the written field order after the caller has selected this exact
    /// source schema. Shape errors precede any selected field type demand.
    /// A deferred type binding retains its field map but cannot start a guard.
    pub fn bind_record_constructor(
        &self,
        name: Option<&str>,
        plan: &crate::syntax::constructors::ConstructorPlan,
        caller: &ScopeKey,
    ) -> Result<NominalConstructorBinding, Diagnostic> {
        match self.bind_record_constructor_with(name, plan, caller, |query| match query {
            FieldTypeQuery::Name(name, _) => crate::db::poll::QueryPoll::Ready(
                self.unqualified(name, None)
                    .and_then(SchemaTypeName::metadata),
            ),
            FieldTypeQuery::Compound(_) => {
                crate::db::poll::QueryPoll::Ready(Ok(SchemaTypeName::deferred()))
            }
        })? {
            crate::db::poll::QueryPoll::Ready(result) => result,
            crate::db::poll::QueryPoll::NeedInput(_) => Err(schema_fault(
                "graph constructor lookup unexpectedly suspended",
            )),
        }
    }
    /// Same record-shape/privacy/field-order binder; schema field Type names are
    /// admitted by the shared current declaration service used by ordinary members.
    pub(crate) fn bind_record_constructor_current(
        &self,
        db: &mut crate::db::memo::Database,
        sources: &[&crate::source::changes::PreparedSourceView],
        name: Option<&str>,
        plan: &crate::syntax::constructors::ConstructorPlan,
        caller: &ScopeKey,
    ) -> crate::db::poll::QueryPoll<NominalConstructorBinding> {
        match self.bind_record_constructor_with(name, plan, caller, |query| {
            self.current_query(db, sources, query)
        }) {
            Ok(poll) => poll,
            Err(error) => crate::db::poll::QueryPoll::Ready(Err(error)),
        }
    }
    fn bind_record_constructor_with(
        &self,
        name: Option<&str>,
        plan: &crate::syntax::constructors::ConstructorPlan,
        caller: &ScopeKey,
        lookup: impl FnMut(FieldTypeQuery<'_>) -> crate::db::poll::QueryPoll<SchemaTypeName>,
    ) -> Result<crate::db::poll::QueryPoll<NominalConstructorBinding>, Diagnostic> {
        use crate::syntax::constructors::ConstructorKind;
        let ConstructorKind::Record {
            fields, rest: None, ..
        } = plan.kind()
        else {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "record update requires its ordered construction integration".into(),
            ));
        };
        let selected = self.schema.variant(name)?;
        let mut written = Vec::new();
        written
            .try_reserve_exact(fields.len())
            .map_err(|_| schema_limit())?;
        written.extend(fields.iter().map(|field| field.name.as_str()));
        let slots = selected.named_initializer_slots(&written)?;
        for recipe in &selected.fields {
            if !crate::syntax::visibility::VisibilityScope::for_declaration(
                self.field_visibility(&recipe.visibility),
                &self.schema.scope,
                self.schema.header.profile.edition,
            )?
            .allows(caller)?
            {
                return Err(schema_fault("reached constructor field is inaccessible"));
            }
        }
        let mapping = BoundRecordFields {
            definition: self.schema.definition,
            schema: self.schema.fingerprint(),
            constructor: plan.fingerprint(),
            written_to_declaration: slots,
        };
        let bound = match self.bind_variant_with(name, lookup)? {
            crate::db::poll::QueryPoll::Ready(result) => result?,
            crate::db::poll::QueryPoll::NeedInput(input) => {
                return Ok(crate::db::poll::QueryPoll::NeedInput(input));
            }
        };
        Ok(crate::db::poll::QueryPoll::Ready(Ok(match bound {
            NominalBinding::Ready(variant) => {
                NominalConstructorBinding::Ready(BoundNominalConstructor { variant, mapping })
            }
            NominalBinding::Deferred { field, demand } => NominalConstructorBinding::Deferred {
                mapping,
                field,
                demand,
            },
        })))
    }
}

#[derive(Clone, Copy)]
enum FieldSelector<'a> {
    Named(&'a str),
    Positional(u32),
}
/// Source/schema checked order only. It confers no runtime field type or guard
/// authority until paired with a successfully bound selected variant.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoundRecordFields {
    definition: Digest,
    schema: Digest,
    constructor: Digest,
    written_to_declaration: Vec<u32>,
}
impl BoundRecordFields {
    pub fn written_to_declaration(&self) -> &[u32] {
        &self.written_to_declaration
    }
    pub fn constructor(&self) -> Digest {
        self.constructor
    }
    pub fn schema(&self) -> Digest {
        self.schema
    }
    pub fn definition(&self) -> Digest {
        self.definition
    }
    fn heap_bytes(&self) -> Option<usize> {
        self.written_to_declaration
            .capacity()
            .checked_mul(std::mem::size_of::<u32>())
    }
}
impl StableEncode for BoundRecordFields {
    const DOMAIN: &'static str = "lrr/v2/bound-record-fields";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.schema);
        e.digest(self.constructor);
        e.sequence(&self.written_to_declaration, |e, ordinal| e.u32(*ordinal));
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoundNominalConstructor {
    variant: BoundNominalVariant,
    mapping: BoundRecordFields,
}
impl BoundNominalConstructor {
    pub fn variant(&self) -> &BoundNominalVariant {
        &self.variant
    }
    pub fn mapping(&self) -> &BoundRecordFields {
        &self.mapping
    }
    pub(crate) fn estimated_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(
                self.variant
                    .estimated_bytes()?
                    .checked_sub(std::mem::size_of::<BoundNominalVariant>())?,
            )?
            .checked_add(self.mapping.heap_bytes()?)
    }
}
impl StableEncode for BoundNominalConstructor {
    const DOMAIN: &'static str = "lrr/v1/bound-nominal-constructor";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.variant);
        e.record(&self.mapping);
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NominalConstructorBinding {
    Ready(BoundNominalConstructor),
    Deferred {
        mapping: BoundRecordFields,
        field: Option<u32>,
        demand: NominalBindingDemand,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NominalBindingDemand {
    Name(super::signature_names::SignatureName),
    GenericContext,
    /// The selected field's captured recipe needs compound/source-type binding.
    TypeRecipe,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NominalBinding {
    Ready(BoundNominalVariant),
    Deferred {
        field: Option<u32>,
        demand: NominalBindingDemand,
    },
}
/// An opaque source member identity for one reached field. Unlike a complete
/// construction witness, this cannot force or authorize unrelated fields.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoundNominalField {
    identity: Digest,
    definition: crate::source::manifest::DefKey,
    schema: Digest,
    variant: u32,
    field: u32,
    field_type: NominalFieldType,
}
impl BoundNominalField {
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn estimated_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.definition
                    .module_path
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())?,
            )?
            .checked_add(self.definition.declared_name.capacity())?;
        for segment in &self.definition.module_path {
            bytes = bytes.checked_add(segment.capacity())?;
        }
        bytes.checked_add(self.field_type.heap_bytes().ok()?)
    }
    pub fn definition(&self) -> &crate::source::manifest::DefKey {
        &self.definition
    }
    pub fn schema(&self) -> Digest {
        self.schema
    }
    pub fn variant(&self) -> u32 {
        self.variant
    }
    pub fn field(&self) -> u32 {
        self.field
    }
    pub fn primitive(&self) -> Option<crate::ir::op::PrimitiveType> {
        self.field_type.concrete()?.primitive()
    }
    pub(crate) fn field_type(&self) -> &NominalFieldType {
        &self.field_type
    }
}
impl StableEncode for BoundNominalField {
    const DOMAIN: &'static str = "lrr/v4/bound-nominal-field";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.record(&self.definition);
        e.digest(self.schema);
        e.u32(self.variant);
        e.u32(self.field);
        e.record(&self.field_type);
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NominalFieldBinding {
    Ready(BoundNominalField),
    Deferred {
        field: u32,
        demand: NominalBindingDemand,
    },
}
/// A source-checked construction witness; no public constructor or writable
/// fields allow a runtime caller to substitute a different nominal recipe.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BoundNominalVariant {
    identity: Digest,
    definition: crate::source::manifest::DefKey,
    schema: Digest,
    variant: u32,
    fields: Vec<NominalFieldType>,
    /// Number of the declaration's type parameters; a constructed value's
    /// `TypeKey::Nominal` args instantiate them in declaration order.
    type_parameters: u32,
    /// Enums only: the declaration-order discriminant table (see
    /// `NominalSchema::literal_discriminants`). Empty for structs.
    discriminants: std::sync::Arc<[Option<i128>]>,
}
/// A bound field's Type: concrete, or one of the declaration's own type
/// parameters, instantiated by the value's `TypeKey::Nominal` args.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum NominalFieldType {
    Concrete(super::source_types::SourceType),
    /// Index among the declaration's type parameters.
    Parameter(u32),
    /// A compound written field Type (reference, tuple, array, slice or
    /// generic application) whose named leaves the declaration's own Type
    /// service selected, over concrete leaves and the declaration's type
    /// parameters.
    Compound(std::sync::Arc<crate::ir::written_type::WrittenType>),
}
impl NominalFieldType {
    pub(crate) fn concrete(&self) -> Option<&super::source_types::SourceType> {
        match self {
            Self::Concrete(source) => Some(source),
            Self::Parameter(_) | Self::Compound(_) => None,
        }
    }
    pub(crate) fn primitive(&self) -> Option<crate::ir::op::PrimitiveType> {
        self.concrete()?.primitive()
    }
    /// Whether one of the declaration's type parameters occurs in this field.
    pub(crate) fn generic(&self) -> bool {
        match self {
            Self::Concrete(_) => false,
            Self::Parameter(_) => true,
            Self::Compound(written) => written.has_parameters(),
        }
    }
    /// The run-local descriptor under the instantiating type arguments.
    pub(crate) fn materialize(
        &self,
        store: &mut crate::runtime::cell::GuestStore,
        args: &[crate::runtime::shape::TypeId],
    ) -> Result<crate::runtime::shape::TypeId, Diagnostic> {
        match self {
            Self::Concrete(source) => source.materialize(store),
            Self::Parameter(index) => args
                .get(*index as usize)
                .copied()
                .ok_or_else(|| schema_fault("generic field lacks its type argument")),
            Self::Compound(written) => written
                .materialize_in(store, args)?
                .ok_or_else(|| schema_fault("compound field Type names no single descriptor")),
        }
    }
    /// The descriptor under type arguments that may still be open: `None`
    /// while a parameter the field names is not fixed.
    pub(crate) fn materialize_partial(
        &self,
        store: &mut crate::runtime::cell::GuestStore,
        args: &[Option<crate::runtime::shape::TypeId>],
    ) -> Result<Option<crate::runtime::shape::TypeId>, Diagnostic> {
        match self {
            Self::Concrete(source) => source.materialize(store).map(Some),
            Self::Parameter(index) => Ok(args.get(*index as usize).copied().flatten()),
            Self::Compound(written) => written
                .materialize_with(store, &|index| args.get(index as usize).copied().flatten()),
        }
    }
    pub(crate) fn find_descriptor(
        &self,
        store: &crate::runtime::cell::GuestStore,
        args: &[crate::runtime::shape::TypeId],
    ) -> Option<crate::runtime::shape::TypeId> {
        match self {
            Self::Concrete(source) => source.find_descriptor(store),
            Self::Parameter(index) => args.get(*index as usize).copied(),
            Self::Compound(written) => written.find_in(store, args).ok().flatten(),
        }
    }
    /// The field's source Type under the instantiating type arguments; a
    /// parameter's argument is a closed runtime descriptor's source term.
    pub(crate) fn source(
        &self,
        store: &crate::runtime::cell::GuestStore,
        args: &[crate::runtime::shape::TypeId],
    ) -> Option<TyTerm> {
        match self {
            Self::Concrete(source) => Some(source.source().clone()),
            Self::Parameter(index) => store.descriptor_source_term(*args.get(*index as usize)?),
            Self::Compound(written) => written.source_in(Some(store), args),
        }
    }
    /// Match a completed field value's descriptor against this field's Type,
    /// fixing the declaration type parameters it determines.
    pub(crate) fn bind(
        &self,
        ty: crate::runtime::shape::TypeId,
        store: &crate::runtime::cell::GuestStore,
        args: &mut [Option<crate::runtime::shape::TypeId>],
    ) -> Result<bool, Diagnostic> {
        match self {
            Self::Concrete(source) => Ok(source.find_descriptor(store) == Some(ty)),
            Self::Parameter(index) => {
                crate::ir::written_type::WrittenType::Parameter(*index).bind(ty, store, args)
            }
            Self::Compound(written) => written.bind(ty, store, args),
        }
    }
    pub(crate) fn heap_bytes(&self) -> Result<usize, Diagnostic> {
        match self {
            Self::Concrete(source) => source.heap_bytes(),
            Self::Parameter(_) => Ok(0),
            Self::Compound(written) => std::mem::size_of::<crate::ir::written_type::WrittenType>()
                .checked_add(written.heap_bytes())
                .ok_or_else(schema_limit),
        }
    }
}
impl StableEncode for NominalFieldType {
    const DOMAIN: &'static str = "lrr/v1/bound-nominal-field-type";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Concrete(source) => {
                e.u32(0);
                e.record(source);
            }
            Self::Parameter(index) => {
                e.u32(1);
                e.u32(*index);
            }
            Self::Compound(written) => {
                e.u32(2);
                e.record(written.as_ref());
            }
        }
    }
}
impl StableEncode for BoundNominalVariant {
    const DOMAIN: &'static str = "lrr/v1/bound-nominal-variant";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.record(&self.definition);
        e.digest(self.schema);
        e.u32(self.variant);
        e.sequence(&self.fields, |e, field| e.record(field));
        if self.type_parameters != 0 {
            e.u32(self.type_parameters);
        }
        if !self.discriminants.is_empty() {
            e.sequence(&self.discriminants, |e, value| {
                e.option(value, |e, value| e.u64(*value as u64));
                e.option(value, |e, value| e.u64((*value >> 64) as u64));
            });
        }
    }
}
impl BoundNominalVariant {
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn estimated_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.fields
                    .capacity()
                    .checked_mul(std::mem::size_of::<NominalFieldType>())?,
            )?
            .checked_add(
                self.definition
                    .module_path
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())?,
            )?
            .checked_add(self.definition.declared_name.capacity())?;
        for segment in &self.definition.module_path {
            bytes = bytes.checked_add(segment.capacity())?;
        }
        for field in &self.fields {
            bytes = bytes.checked_add(field.heap_bytes().ok()?)?;
        }
        bytes = bytes.checked_add(
            self.discriminants
                .len()
                .checked_mul(std::mem::size_of::<Option<i128>>())?,
        )?;
        Some(bytes)
    }
    pub(crate) fn definition(&self) -> &crate::source::manifest::DefKey {
        &self.definition
    }
    pub(crate) fn schema(&self) -> Digest {
        self.schema
    }
    pub(crate) fn variant(&self) -> u32 {
        self.variant
    }
    pub(crate) fn fields(&self) -> &[NominalFieldType] {
        &self.fields
    }
    pub(crate) fn type_parameters(&self) -> u32 {
        self.type_parameters
    }
    /// The source Type of field `ordinal` of a value whose nominal
    /// descriptor carries these instantiating type arguments.
    pub(crate) fn field_source(
        &self,
        ordinal: u32,
        store: &crate::runtime::cell::GuestStore,
        args: &[crate::runtime::shape::TypeId],
    ) -> Option<TyTerm> {
        self.fields.get(ordinal as usize)?.source(store, args)
    }
    pub(crate) fn discriminants(&self) -> &std::sync::Arc<[Option<i128>]> {
        &self.discriminants
    }
}
/// `[INT_NUMBER]` or `[-, INT_NUMBER]` with an optional integer suffix and any
/// radix prefix, as written in an explicit enum discriminant.
pub(crate) fn literal_discriminant(tokens: &[TokenRecipe]) -> Option<i128> {
    use ra_ap_syntax::SyntaxKind;
    let (negative, number) = match tokens {
        [number] => (false, number),
        [minus, number] if minus.kind == SyntaxKind::MINUS as u16 => (true, number),
        _ => return None,
    };
    if number.kind != SyntaxKind::INT_NUMBER as u16 {
        return None;
    }
    let text = number.text.replace('_', "");
    let (radix, digits) = match text.get(..2) {
        Some("0x") => (16, &text[2..]),
        Some("0o") => (8, &text[2..]),
        Some("0b") => (2, &text[2..]),
        _ => (10, text.as_str()),
    };
    // A suffix starts at the first `i`/`u`, neither of which is a digit in
    // any integer radix.
    let end = digits.find(['i', 'u']).unwrap_or(digits.len());
    let suffix = &digits[end..];
    if !matches!(
        suffix,
        "" | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
    ) {
        return None;
    }
    let magnitude = u128::from_str_radix(&digits[..end], radix).ok()?;
    if negative {
        0i128.checked_sub_unsigned(magnitude)
    } else {
        i128::try_from(magnitude).ok()
    }
}
fn token_bytes(tokens: &Vec<TokenRecipe>) -> Option<usize> {
    let mut bytes = tokens
        .capacity()
        .checked_mul(std::mem::size_of::<TokenRecipe>())?;
    for token in tokens {
        bytes = bytes.checked_add(token.text.capacity())?;
    }
    Some(bytes)
}
impl StableEncode for NominalSchema {
    const DOMAIN: &'static str = "lrr/v2/nominal-schema";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.record(&self.scope);
        e.record(&self.header);
        e.sequence(&self.generics, |e, parameter| {
            e.u32(parameter.ordinal);
            e.u32(match parameter.kind {
                GenericKind::Lifetime => 1,
                GenericKind::Type => 2,
                GenericKind::Const => 3,
            });
            e.string(&parameter.name);
            e.sequence(&parameter.bounds, |e, token| e.record(token));
            e.sequence(&parameter.default, |e, token| e.record(token));
            e.sequence(&parameter.const_type, |e, token| e.record(token));
        });
        e.sequence(&self.variants, |e, variant| {
            e.option(&variant.name, |e, name| e.string(name));
            e.u32(match variant.shape {
                FieldShape::Unit => 0,
                FieldShape::Tuple => 1,
                FieldShape::Record => 2,
            });
            e.sequence(&variant.discriminant, |e, token| e.record(token));
            e.sequence(&variant.fields, |e, field| {
                e.option(&field.name, |e, name| e.string(name));
                e.record(&field.visibility);
                e.sequence(&field.ty, |e, token| e.record(token));
                e.option(&field.symbolic, |e, term| e.record(term));
            });
        });
    }
}

#[cfg(test)]
#[path = "nominal_field_storage_tests.rs"]
mod nominal_field_storage_tests;
