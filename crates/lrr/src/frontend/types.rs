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

pub(crate) const MAX_SCHEMA_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_SCHEMA_FIELDS: usize = 100_000;

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
        if self.shape != FieldShape::Record {
            return Err(schema_fault("named initializer requires record fields"));
        }
        if written.len() > MAX_SCHEMA_FIELDS || self.fields.len() > MAX_SCHEMA_FIELDS {
            return Err(schema_limit());
        }
        let mut declared = BTreeMap::new();
        for (ordinal, field) in self.fields.iter().enumerate() {
            let name = field
                .name
                .as_deref()
                .ok_or_else(|| schema_fault("record field lacks its name"))?;
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
}
impl<'a> SchemaNames<'a> {
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
        if let Err(error) = graph.require_source(source.selection(), source.outline()) {
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
            if let Err(error) = graph.require_source(source.selection(), source.outline()) {
                return QueryPoll::Ready(Err(error));
            }
        }
        let definition =
            match super::source_types::nominal_definition(db, sources, schema.definition) {
                Ok(definition) => definition,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        Self::from_definition(db, graph, schema, Cow::Owned(definition))
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

    /// Bind only the reached variant. A pending source name remains a demand;
    /// captured spelling never establishes primitive or nominal identity.
    /// Alias, generic and compound-field resolution continue through the source
    /// type engine rather than being guessed from a guest descriptor.
    pub fn bind_variant(&self, name: Option<&str>) -> Result<NominalBinding, Diagnostic> {
        use super::signature_names::SignatureName;
        let selected = self.schema.variant(name)?;
        let own_name = self.unqualified(&self.definition.declared_name, None)?;
        if !matches!(&own_name, SignatureName::Source(found) if found.identity == self.schema.definition)
        {
            return Ok(NominalBinding::Deferred {
                field: None,
                demand: NominalBindingDemand::Name(own_name),
            });
        }
        if !self.schema.generics.is_empty() {
            return Ok(NominalBinding::Deferred {
                field: None,
                demand: NominalBindingDemand::GenericContext,
            });
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
        for (ordinal, recipe) in selected.fields.iter().enumerate() {
            let field = Some(u32::try_from(ordinal).map_err(|_| schema_limit())?);
            let resolved = if recipe.ty.len() == 1 {
                self.unqualified(&recipe.ty[0].text, None)?
            } else if matches!(
                recipe.symbolic,
                Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit))
            ) {
                SignatureName::Primitive(crate::ir::op::PrimitiveType::Unit)
            } else {
                return Ok(NominalBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::TypeRecipe,
                });
            };
            let SignatureName::Primitive(primitive) = resolved else {
                return Ok(NominalBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::Name(resolved),
                });
            };
            fields.push(primitive);
        }
        Ok(NominalBinding::Ready(BoundNominalVariant {
            identity: self.schema.definition,
            definition: self.definition.as_ref().clone(),
            schema: self.schema.fingerprint(),
            variant,
            fields,
        }))
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
        match self.bind_named_field_with(variant_name, member, caller, |name| {
            crate::db::poll::QueryPoll::Ready(self.unqualified(name, None))
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
        let outcome = self.bind_named_field_with(variant_name, member, caller, |name| {
            super::type_name_namespace::resolve_declaration_name(
                db,
                super::type_name_namespace::SourceTypeOwner {
                    definition: self.schema.definition,
                    scope: &self.schema.scope,
                    header: &self.schema.header,
                },
                &self.schema.generics,
                sources,
                name,
            )
        });
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
        mut lookup: impl FnMut(
            &str,
        )
            -> crate::db::poll::QueryPoll<super::signature_names::SignatureName>,
    ) -> Result<crate::db::poll::QueryPoll<NominalFieldBinding>, Diagnostic> {
        use super::signature_names::SignatureName;
        let selected = self.schema.variant(variant_name)?;
        if selected.shape != FieldShape::Record {
            return Err(schema_fault("named field requires a record variant"));
        }
        let member = super::names::owned_name(member, self.schema.header.profile.edition)?;
        let mut matching = selected
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.name.as_deref() == Some(member.as_str()));
        let (ordinal, recipe) = matching
            .next()
            .ok_or_else(|| schema_fault("reached source field is absent"))?;
        if matching.next().is_some() {
            return Err(schema_fault("reached source field is ambiguous"));
        }
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
        let own_name = match lookup(&self.definition.declared_name) {
            crate::db::poll::QueryPoll::Ready(Ok(selected)) => selected,
            crate::db::poll::QueryPoll::Ready(Err(error)) => return Err(error),
            crate::db::poll::QueryPoll::NeedInput(request) => {
                return Ok(crate::db::poll::QueryPoll::NeedInput(request));
            }
        };
        if !matches!(&own_name, SignatureName::Source(found) if found.identity == self.schema.definition)
        {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::Name(own_name),
                },
            )));
        }
        if !self.schema.generics.is_empty() {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::GenericContext,
                },
            )));
        }
        let resolved = if recipe.ty.len() == 1 {
            match lookup(&recipe.ty[0].text) {
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
            SignatureName::Primitive(crate::ir::op::PrimitiveType::Unit)
        } else {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::TypeRecipe,
                },
            )));
        };
        let SignatureName::Primitive(primitive) = resolved else {
            return Ok(crate::db::poll::QueryPoll::Ready(Ok(
                NominalFieldBinding::Deferred {
                    field,
                    demand: NominalBindingDemand::Name(resolved),
                },
            )));
        };
        let variant = self
            .schema
            .variants
            .iter()
            .position(|variant| variant.name.as_deref() == variant_name)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(schema_limit)?;
        Ok(crate::db::poll::QueryPoll::Ready(Ok(
            NominalFieldBinding::Ready(BoundNominalField {
                identity: self.schema.definition,
                definition: self.definition.as_ref().clone(),
                schema: self.schema.fingerprint(),
                variant,
                field,
                primitive,
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
        match self.bind_variant(name)? {
            NominalBinding::Ready(variant) => {
                Ok(NominalConstructorBinding::Ready(BoundNominalConstructor {
                    variant,
                    mapping,
                }))
            }
            NominalBinding::Deferred { field, demand } => Ok(NominalConstructorBinding::Deferred {
                mapping,
                field,
                demand,
            }),
        }
    }
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
    primitive: crate::ir::op::PrimitiveType,
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
        Some(bytes)
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
    pub fn primitive(&self) -> crate::ir::op::PrimitiveType {
        self.primitive
    }
}
impl StableEncode for BoundNominalField {
    const DOMAIN: &'static str = "lrr/v3/bound-nominal-field";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.record(&self.definition);
        e.digest(self.schema);
        e.u32(self.variant);
        e.u32(self.field);
        e.u32(self.primitive.tag());
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
    fields: Vec<crate::ir::op::PrimitiveType>,
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
                    .checked_mul(std::mem::size_of::<crate::ir::op::PrimitiveType>())?,
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
    pub(crate) fn fields(&self) -> &[crate::ir::op::PrimitiveType] {
        &self.fields
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
