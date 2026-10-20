//! Independent actual-source ADT query leaves, before admitted solver entry.
//! A retained declaration is not evidence for destructor, Copy or sizedness.
use super::{
    configuration::PreparedSolverConfiguration,
    enum_repr::PreparedEnumDiscriminant,
    identity::DefinitionId,
    model::{AdtInfo, AdtKind},
    prepared::{self, Cx, DefinitionReceipt, PreparedAdtVariants, PreparedProvider},
    query_facts::QueryFact,
    source_adt_fields::{AdtSourceFieldsKey, PreparedAdtSourceFields, SourceAdtFieldsPending},
    source_definition_headers::{
        DefinitionHeaderKey, DefinitionHeaderPending, DefinitionHeaderRole,
        PreparedDefinitionHeader,
    },
    source_generics::SourceGenericLayouts,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::SealKey,
        value::QueryValue,
    },
};
use rustc_type_ir::Interner;
use std::sync::Arc;

/// Field Type preparation is a real demand distinction. Discriminant queries
/// do not need to prepare an unrelated field Type dependency closure.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SourceAdtFactsKey {
    pub inventory: SealKey,
    pub definition: Digest,
    pub fields: bool,
}
impl StableEncode for SourceAdtFactsKey {
    const DOMAIN: &'static str = "lrr/v1/source-adt-facts-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.inventory);
        e.digest(self.definition);
        e.bool(self.fields);
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceAdtDomain {
    Nominal(AdtKind),
    NotApplicable,
    Pending,
}

/// These domains have no producer in this source declaration component.
/// Their absence never becomes a successful false, None or empty answer.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AdtSemanticDomain {
    PhantomData,
    ManuallyDrop,
    UnsafeFields,
    Fundamental,
    Destructor,
    Copy,
    SizedConstraint,
    MetaSizedConstraint,
    Unsizing,
    FieldRepresentingType,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceAdtFactsPending {
    SourceDefinition,
    Header(DefinitionHeaderPending),
    NotApplicable,
    Context,
    FieldsNotDemanded,
    Fields(SourceAdtFieldsPending),
    Semantic(AdtSemanticDomain),
}

pub struct PreparedSourceAdtFacts {
    key: SourceAdtFactsKey,
    catalog: Arc<InputValue>,
    header: Arc<QueryValue>,
    receipt: Option<DefinitionReceipt>,
    schema: Option<Arc<QueryValue>>,
    source_variants: Option<Arc<PreparedAdtVariants>>,
    discriminant: Option<Arc<PreparedEnumDiscriminant>>,
    fields: Option<Arc<QueryValue>>,
    domain: SourceAdtDomain,
    packed: Option<bool>,
    pending: Option<SourceAdtFactsPending>,
}
impl std::fmt::Debug for PreparedSourceAdtFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSourceAdtFacts")
            .field("key", &self.key)
            .field("domain", &self.domain)
            .field("pending", &self.pending)
            .field("fields_pending", &self.fields_pending())
            .finish_non_exhaustive()
    }
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
impl PreparedSourceAdtFacts {
    pub fn key(&self) -> &SourceAdtFactsKey {
        &self.key
    }
    pub(crate) fn domain(&self) -> SourceAdtDomain {
        self.domain
    }
    /// Declaration capture availability only, not all callback completeness.
    pub(crate) fn pending(&self) -> Option<SourceAdtFactsPending> {
        self.pending
    }
    pub(super) fn catalog_input(&self) -> &Arc<InputValue> {
        &self.catalog
    }
    pub(super) fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(plan) = self.header.as_ref() else {
            unreachable!("retained ADT header")
        };
        plan
    }
    pub(super) fn field_plan(&self) -> Result<&PreparedAdtSourceFields, SourceAdtFactsPending> {
        match self.fields.as_deref() {
            Some(QueryValue::AdtSourceFields(plan)) => Ok(plan),
            _ if !self.key.fields => Err(SourceAdtFactsPending::FieldsNotDemanded),
            _ => Err(self
                .pending
                .unwrap_or(SourceAdtFactsPending::SourceDefinition)),
        }
    }
    pub(crate) fn fields_pending(&self) -> Option<SourceAdtFactsPending> {
        match self.field_plan() {
            Ok(plan) => plan.pending().map(SourceAdtFactsPending::Fields),
            Err(why) => Some(why),
        }
    }
    pub(super) fn source_variants(
        &self,
    ) -> Result<Arc<PreparedAdtVariants>, SourceAdtFactsPending> {
        self.source_variants.as_ref().cloned().ok_or(
            self.pending
                .unwrap_or(SourceAdtFactsPending::SourceDefinition),
        )
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        self.field_plan()
            .map_err(|why| fault(&format!("ADT generic metadata unavailable: {why:?}")))?
            .generic_layouts()
    }
    pub(crate) fn semantic_pending(&self, domain: AdtSemanticDomain) -> SourceAdtFactsPending {
        SourceAdtFactsPending::Semantic(domain)
    }

    pub(crate) fn prepare(db: &mut Database, key: &SourceAdtFactsKey) -> QueryPoll<Self> {
        let Some(catalog) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("ADT source catalog absent")));
        };
        let InputValue::DiscoverySeal(source) = catalog.as_ref() else {
            return QueryPoll::Ready(Err(fault("ADT source catalog variant differs")));
        };
        if let Err(error) = prepared::catalog_world(source) {
            return QueryPoll::Ready(Err(error));
        }
        let header_key = DefinitionHeaderKey {
            inventory: key.inventory.clone(),
            definition: key.definition,
        };
        let captured = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(header_key.clone()))
        ));
        if !captured.cacheable
            || !matches!(captured.value.as_ref(), QueryValue::DefinitionHeaderPlan(plan) if plan.key() == &header_key)
        {
            return QueryPoll::Ready(Err(fault(
                "ADT header is not a retained actual declaration",
            )));
        }
        let mut result = Self {
            key: key.clone(),
            catalog,
            header: captured.value,
            receipt: None,
            schema: None,
            source_variants: None,
            discriminant: None,
            fields: None,
            domain: SourceAdtDomain::Pending,
            packed: None,
            pending: None,
        };
        if let Some(why) = result.header().pending() {
            result.pending = Some(SourceAdtFactsPending::Header(why));
            return QueryPoll::Ready(Ok(result));
        }
        let Some(receipt) = ready!(DefinitionReceipt::observe(db, key.definition)) else {
            result.pending = Some(SourceAdtFactsPending::SourceDefinition);
            return QueryPoll::Ready(Ok(result));
        };
        if !receipt.matches_generics(result.header().declaration()) {
            return QueryPoll::Ready(Err(fault("ADT header and receipt differ")));
        }
        let role = result.header().role();
        if !matches!(
            role,
            DefinitionHeaderRole::Struct | DefinitionHeaderRole::Enum | DefinitionHeaderRole::Union
        ) {
            let nominal = match receipt.is_nominal() {
                Ok(value) => value,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            if nominal {
                return QueryPoll::Ready(Err(fault("ADT header role differs from actual source")));
            }
            result.receipt = Some(receipt);
            result.domain = SourceAdtDomain::NotApplicable;
            return QueryPoll::Ready(Ok(result));
        }
        let (kind, name) = match receipt.nominal_kind_and_name() {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let expected = match role {
            DefinitionHeaderRole::Struct => AdtKind::Struct,
            DefinitionHeaderRole::Enum => AdtKind::Enum,
            DefinitionHeaderRole::Union => AdtKind::Union,
            _ => unreachable!(),
        };
        if kind != expected || result.header().name() != Some(name.as_str()) {
            return QueryPoll::Ready(Err(fault("ADT declaration role or name differs")));
        }
        let captured = ready!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(key.definition)));
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("ADT schema variant differs")));
        };
        if !captured.cacheable || !receipt.matches_enum_schema(schema) {
            return QueryPoll::Ready(Err(fault("ADT schema differs from actual declaration")));
        }
        let packed = match crate::frontend::declaration_repr::declared_repr(schema, &name) {
            Ok(repr) => repr.layout.packed.is_some(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let shape = ready!(PreparedAdtVariants::prepare(db, key.definition));
        if kind == AdtKind::Enum {
            let Some(discriminant) = ready!(PreparedEnumDiscriminant::prepare(db, key.definition))
            else {
                return QueryPoll::Ready(Err(fault("actual enum discriminant source plan absent")));
            };
            if !discriminant.matches(&receipt) {
                return QueryPoll::Ready(Err(fault("ADT discriminant receipt differs")));
            }
            result.discriminant = Some(Arc::new(discriminant));
        }
        result.receipt = Some(receipt);
        result.schema = Some(captured.value);
        result.source_variants = Some(Arc::new(shape));
        result.domain = SourceAdtDomain::Nominal(kind);
        result.packed = Some(packed);
        if key.fields {
            let fields_key = AdtSourceFieldsKey {
                inventory: key.inventory.clone(),
                definition: key.definition,
            };
            let captured = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::AdtSourceFields(Box::new(fields_key.clone()))
            ));
            let QueryValue::AdtSourceFields(fields) = captured.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("ADT field memo variant differs")));
            };
            if !captured.cacheable
                || fields.key() != &fields_key
                || fields.kind() != kind
                || fields.name() != name
                || !result
                    .receipt
                    .as_ref()
                    .expect("actual nominal")
                    .matches_enum_schema(fields.schema())
            {
                return QueryPoll::Ready(Err(fault(
                    "ADT fields belong to another source declaration",
                )));
            }
            result.fields = Some(captured.value);
        }
        QueryPoll::Ready(Ok(result))
    }

    /// No source callbacks or field preparation. Every other slot is explicitly
    /// unavailable, rather than an invented false/None/empty value.
    pub(super) fn declaration_info<'cx>(
        &self,
    ) -> Result<AdtInfo<'cx, PreparedProvider<'cx>>, SourceAdtFactsPending> {
        if let Some(why) = self.pending {
            return Err(why);
        }
        let kind = match self.domain {
            SourceAdtDomain::Nominal(kind) => kind,
            SourceAdtDomain::NotApplicable => return Err(SourceAdtFactsPending::NotApplicable),
            SourceAdtDomain::Pending => return Err(SourceAdtFactsPending::SourceDefinition),
        };
        let mut info = AdtInfo::declaration(kind);
        info.packed = QueryFact::Known(self.packed.ok_or(SourceAdtFactsPending::SourceDefinition)?);
        info.discriminant = QueryFact::Known(match kind {
            AdtKind::Enum => Some(
                self.discriminant
                    .as_ref()
                    .cloned()
                    .ok_or(SourceAdtFactsPending::SourceDefinition)?,
            ),
            AdtKind::Struct | AdtKind::Union => None,
        });
        Ok(info)
    }
    pub(super) fn definition_id(&self) -> Result<DefinitionId, SourceAdtFactsPending> {
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else {
            unreachable!("actual catalog")
        };
        let local = catalog
            .impl_catalog
            .as_ref()
            .ok_or(SourceAdtFactsPending::Context)?
            .root()
            .outline
            .crate_key;
        Ok(self
            .receipt
            .as_ref()
            .ok_or(SourceAdtFactsPending::SourceDefinition)?
            .definition_id(catalog.seal.fingerprint(), local))
    }
    /// Translate retained actual field Types using the existing common encoder.
    /// No normalization or semantic inference occurs here.
    pub(super) fn encode<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<AdtInfo<'cx, PreparedProvider<'cx>>, SourceAdtFactsPending> {
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else {
            unreachable!("actual catalog")
        };
        if cx.services().source_world() != catalog.seal.fingerprint()
            || cx.services().source_configuration() != configuration.fingerprint()
            || configuration.source()
                != catalog
                    .impl_catalog
                    .as_ref()
                    .ok_or(SourceAdtFactsPending::Context)?
                    .root()
        {
            return Err(SourceAdtFactsPending::Context);
        }
        let mut info = self.declaration_info()?;
        if self.key.fields {
            let variants = self
                .field_plan()?
                .materialize(cx, configuration)
                .map_err(SourceAdtFactsPending::Fields)?;
            let fields = cx.mk_type_list_from_iter(
                variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|field| field.ty.skip_binder())),
            );
            // Pinned AdtDef::struct_tail_ty is the last declared field Type;
            // structural sizedness is a separate, still-unprepared query.
            if info.kind == AdtKind::Struct {
                info.tail = QueryFact::Known(
                    variants
                        .last()
                        .and_then(|variant| variant.fields.last())
                        .map(|field| field.ty.skip_binder()),
                );
            } else {
                info.tail = QueryFact::NotApplicable;
            }
            info.variants = QueryFact::Known(variants);
            info.fields = QueryFact::Known(fields);
        }
        Ok(info)
    }
    // The enclosing memo guard authenticates the full direct/nested query
    // closure. These are additional publication proofs, not a replacement.
    fn require_auxiliary_current(&self, db: &Database) -> Result<(), Diagnostic> {
        if let Some(shape) = &self.source_variants {
            shape.require_current(db)?;
        }
        if let Some(discriminant) = &self.discriminant {
            discriminant.require_current(db)?;
        }
        Ok(())
    }
}

impl StableEncode for PreparedSourceAdtFacts {
    const DOMAIN: &'static str = "lrr/v1/source-adt-facts";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.catalog.fingerprint());
        e.digest(self.header.fingerprint());
        e.option(&self.schema, |e, root| e.digest(root.fingerprint()));
        e.option(&self.fields, |e, root| e.digest(root.fingerprint()));
        match self.domain {
            SourceAdtDomain::Nominal(kind) => {
                e.u8(0);
                e.u8(match kind {
                    AdtKind::Struct => 0,
                    AdtKind::Enum => 1,
                    AdtKind::Union => 2,
                });
            }
            SourceAdtDomain::NotApplicable => e.u8(1),
            SourceAdtDomain::Pending => e.u8(2),
        }
        e.option(&self.packed, |e, packed| e.bool(*packed));
        // Header/schema fingerprints include the actual declaration inputs and
        // representation spelling. The owned proofs derive only from them.
        e.bool(self.source_variants.is_some());
        e.bool(self.discriminant.is_some());
        e.option(&self.pending, |e, pending| match pending {
            SourceAdtFactsPending::SourceDefinition => e.u8(0),
            SourceAdtFactsPending::Header(why) => {
                e.u8(1);
                match why {
                    DefinitionHeaderPending::ParentJoin => e.u8(0),
                    DefinitionHeaderPending::ImplMembership => e.u8(1),
                    DefinitionHeaderPending::Source(why) => {
                        e.u8(2);
                        e.u32(*why as u32);
                    }
                    DefinitionHeaderPending::UnsupportedRole => e.u8(3),
                }
            }
            // These are return-domain reasons, never stored capture states.
            SourceAdtFactsPending::NotApplicable => e.u8(2),
            SourceAdtFactsPending::Context => e.u8(3),
            SourceAdtFactsPending::FieldsNotDemanded => e.u8(4),
            SourceAdtFactsPending::Fields(why) => {
                e.u8(5);
                match why {
                    SourceAdtFieldsPending::Environment => e.u8(0),
                    SourceAdtFieldsPending::SourceDefinition => e.u8(1),
                    SourceAdtFieldsPending::FieldType { variant, field } => {
                        e.u8(2);
                        e.u32(*variant);
                        e.u32(*field);
                    }
                    SourceAdtFieldsPending::Source(why) => {
                        e.u8(3);
                        e.u32(*why as u32);
                    }
                }
            }
            SourceAdtFactsPending::Semantic(domain) => {
                e.u8(6);
                e.u32(*domain as u32);
            }
        });
    }
}

#[derive(Debug)]
pub struct CurrentSourceAdtFacts {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceAdtFacts {
    pub(crate) fn capture(db: &mut Database, key: &SourceAdtFactsKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceAdtFacts(Box::new(key.clone()));
        let captured = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !captured.cacheable
            || !matches!(captured.value.as_ref(), QueryValue::SourceAdtFacts(plan) if plan.key() == key)
        {
            return QueryPoll::Ready(Err(fault(
                "ADT fact memo is not a retained actual source plan",
            )));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, captured.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: captured.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceAdtFacts {
        let QueryValue::SourceAdtFacts(plan) = self.value.as_ref() else {
            unreachable!("captured ADT fact memo")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)?;
        self.plan().require_auxiliary_current(db)
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.guard.fingerprint()
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
