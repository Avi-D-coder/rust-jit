//! Actual declaration field Types through the ordinary source Type service.
//! No body preparation, guest effects, independent inference or solver entry.
use super::{
    configuration::PreparedSolverConfiguration,
    model::{AdtField, AdtKind, AdtVariant, Symbol},
    prepared::{self, Cx, DefinitionReceipt},
    source_bindings::{DefinitionKind, SourceBindings, require_type},
    source_generics::SourceGenericLayouts,
    source_terms::SourceTypePending,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{
        dependency::{InputKey, InputValue}, key::QueryKey,
        memo::{Database, PreparedInputGuard}, poll::QueryPoll,
        provider::SyntaxProvider, seals::SealKey, value::QueryValue,
    },
    frontend::types::{NominalSchema, SchemaNames},
    traits::term::TyTerm,
};
use rustc_type_ir::EarlyBinder;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct AdtSourceFieldsKey {
    pub inventory: SealKey,
    pub definition: Digest,
}
impl StableEncode for AdtSourceFieldsKey {
    const DOMAIN: &'static str = "lrr/v1/adt-source-fields-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.inventory);
        e.digest(self.definition);
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceAdtFieldsPending {
    Environment,
    SourceDefinition,
    FieldType { variant: u32, field: u32 },
    Source(SourceTypePending),
}
#[derive(Debug)]
struct SourceField {
    name: String,
    ty: TyTerm,
}
#[derive(Debug)]
struct SourceVariant {
    name: String,
    fields: Vec<SourceField>,
}

/// Immutable memo value. Source facts remain distinct from full ADT facts and
/// solver readiness. A pending plan never publishes its partial field prefix.
#[derive(Debug)]
pub struct PreparedAdtSourceFields {
    key: AdtSourceFieldsKey,
    receipt: DefinitionReceipt,
    schema: Arc<QueryValue>,
    environment: Arc<QueryValue>,
    bindings: SourceBindings,
    kind: AdtKind,
    name: String,
    variants: Vec<SourceVariant>,
    pending: Option<SourceAdtFieldsPending>,
}

macro_rules! ready {
    ($value:expr) => { match $value {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    } };
}
impl PreparedAdtSourceFields {
    pub fn key(&self) -> &AdtSourceFieldsKey { &self.key }
    pub fn pending(&self) -> Option<SourceAdtFieldsPending> { self.pending }
    pub(super) fn kind(&self) -> AdtKind { self.kind }
    pub(super) fn name(&self) -> &str { &self.name }
    pub(super) fn schema(&self) -> &NominalSchema {
        let QueryValue::TypeSchema(schema) = self.schema.as_ref() else { unreachable!("captured ADT schema") };
        schema
    }
    pub(super) fn bindings(&self) -> Result<&SourceBindings, SourceAdtFieldsPending> {
        self.pending.map_or(Ok(&self.bindings), Err)
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        if self.pending.is_some() {
            return Err(fault("pending ADT field preparation cannot construct generic layouts"));
        }
        SourceGenericLayouts::from_bindings(Arc::clone(&self.bindings.catalog), [&self.bindings])
    }
    pub(crate) fn prepare(db: &mut Database, key: &AdtSourceFieldsKey) -> QueryPoll<Self> {
        let environment = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.inventory.clone()))).value;
        let schema = ready!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(key.definition))).value;
        let Some(receipt) = ready!(DefinitionReceipt::observe(db, key.definition)) else {
            return QueryPoll::Ready(Err(fault("actual ADT source receipt is not prepared")));
        };
        let (kind, name) = match receipt.nominal_kind_and_name() {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let QueryValue::TypeSchema(actual) = schema.as_ref() else {
            return QueryPoll::Ready(Err(fault("ADT field source schema query differs")));
        };
        if !receipt.matches_enum_schema(actual) {
            return QueryPoll::Ready(Err(fault("ADT field source schema differs from its receipt")));
        }
        let Some(catalog_input) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("ADT field source catalog absent")));
        };
        let InputValue::DiscoverySeal(catalog) = catalog_input.as_ref() else {
            return QueryPoll::Ready(Err(fault("ADT field source catalog variant differs")));
        };
        if let Err(error) = prepared::catalog_world(catalog) { return QueryPoll::Ready(Err(error)); }
        let mut result = Self {
            key: key.clone(), receipt, schema, environment,
            bindings: SourceBindings::empty(Arc::clone(&catalog_input)), kind, name,
            variants: Vec::new(), pending: None,
        };
        let QueryValue::PreparedScopeImplHeaders(inventory) = result.environment.as_ref() else {
            return QueryPoll::Ready(Err(fault("ADT field source environment query differs")));
        };
        if inventory.key() != &key.inventory {
            return QueryPoll::Ready(Err(fault("ADT field source environment inventory differs")));
        }
        let Some(environment) = inventory.inventory_root().environment() else {
            result.pending = Some(SourceAdtFieldsPending::Environment);
            return QueryPoll::Ready(Ok(result));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let names = ready!(SchemaNames::from_current_database(db, environment.names(), &sources, result.schema()));
        let mut required = BTreeMap::from([(key.definition, DefinitionKind::Nominal)]);
        let mut variants = Vec::new();
        for (variant_index, variant) in result.schema().variants.iter().enumerate() {
            let variant_index = match u32::try_from(variant_index) {
                Ok(index) => index,
                Err(_) => return QueryPoll::Ready(Err(fault("ADT source variant index overflows"))),
            };
            let terms = ready!(names.source_variant_fields(db, &sources, variant_index));
            if terms.len() != variant.fields.len() {
                return QueryPoll::Ready(Err(fault("ADT source field occurrence count differs")));
            }
            let mut fields = Vec::new();
            for (field_index, (field, term)) in variant.fields.iter().zip(terms).enumerate() {
                let Some(ty) = term else {
                    let field = match u32::try_from(field_index) {
                        Ok(index) => index,
                        Err(_) => return QueryPoll::Ready(Err(fault("ADT source field index overflows"))),
                    };
                    result.pending = Some(SourceAdtFieldsPending::FieldType { variant: variant_index, field });
                    return QueryPoll::Ready(Ok(result));
                };
                require_type(&mut required, &ty);
                fields.push(SourceField { name: field.name.clone().unwrap_or_else(|| field_index.to_string()), ty });
            }
            variants.push(SourceVariant { name: variant.name.clone().unwrap_or_else(|| result.name.clone()), fields });
        }
        let bindings = ready!(SourceBindings::prepare(db, catalog_input, required));
        if bindings.pending.is_none() {
            let Some(owner) = bindings.definitions.get(&key.definition) else {
                return QueryPoll::Ready(Err(fault("actual ADT owner source binding absent")));
            };
            if !bindings.generics.get(owner).is_some_and(|value|
                matches!(value.as_ref(), QueryValue::DeclarationGenericTypes(generics)
                    if result.receipt.matches_generics(generics.declaration())))
            {
                return QueryPoll::Ready(Err(fault("ADT source field and generic declaration receipts differ")));
            }
        }
        result.pending = bindings.pending.map(SourceAdtFieldsPending::Source);
        result.bindings = bindings;
        result.variants = variants;
        QueryPoll::Ready(Ok(result))
    }

    /// Translate retained real source terms only. This does not fetch source,
    /// normalize aliases, execute initializers, or grant full fact readiness.
    pub(super) fn encode<'cx>(&self, cx: Cx<'cx>, configuration: &PreparedSolverConfiguration)
        -> Result<Vec<EncodedAdtVariant<'cx>>, SourceAdtFieldsPending> {
        let bindings = self.bindings()?;
        let InputValue::DiscoverySeal(catalog) = bindings.catalog.as_ref() else { unreachable!("captured ADT catalog") };
        if catalog.seal.key() != self.key.inventory
            || configuration.source() != catalog.impl_catalog.as_ref().expect("checked source catalog").root()
            || cx.services().source_world() != catalog.seal.fingerprint()
            || !bindings.definitions.contains_key(&self.key.definition)
        {
            return Err(SourceAdtFieldsPending::SourceDefinition);
        }
        let mut variants = Vec::new();
        for variant in &self.variants {
            let mut fields = Vec::new();
            for field in &variant.fields {
                let ty = super::source_terms::encode_declared(cx, configuration, &bindings.definitions, &field.ty)
                    .map_err(SourceAdtFieldsPending::Source)?;
                fields.push(AdtField { name: cx.retain_adt_symbol(&field.name), ty: EarlyBinder::bind(cx, ty) });
            }
            variants.push(EncodedAdtVariant { name: cx.retain_adt_symbol(&variant.name), fields });
        }
        Ok(variants)
    }
    pub(super) fn materialize<'cx>(&self, cx: Cx<'cx>, configuration: &PreparedSolverConfiguration)
        -> Result<&'cx [AdtVariant<'cx, prepared::PreparedProvider<'cx>>], SourceAdtFieldsPending> {
        let variants = self.encode(cx, configuration)?.into_iter().map(|variant| AdtVariant {
            name: variant.name,
            fields: cx.retain_adt_fields(variant.fields),
        }).collect();
        Ok(cx.retain_adt_variants(variants))
    }
}
pub(super) struct EncodedAdtVariant<'cx> {
    pub(super) name: Symbol<'cx>,
    pub(super) fields: Vec<AdtField<'cx, prepared::PreparedProvider<'cx>>>,
}

/// The guard is captured only after the declaration memo is complete. It
/// includes the actual field lookup and generic/source binding dependencies.
#[derive(Debug)]
pub struct CurrentAdtSourceFields {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentAdtSourceFields {
    pub(crate) fn capture(db: &mut Database, key: &AdtSourceFieldsKey) -> QueryPoll<Self> {
        let query = QueryKey::AdtSourceFields(Box::new(key.clone()));
        let captured = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !captured.cacheable || !matches!(captured.value.as_ref(), QueryValue::AdtSourceFields(plan) if plan.key() == key) {
            return QueryPoll::Ready(Err(fault("ADT source field memo is not a retained actual plan")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, captured.fingerprint));
        QueryPoll::Ready(Ok(Self { value: captured.value, guard }))
    }
    pub(crate) fn plan(&self) -> &PreparedAdtSourceFields {
        let QueryValue::AdtSourceFields(plan) = self.value.as_ref() else { unreachable!("captured ADT field memo") };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> { self.guard.require_current(db) }
    pub(crate) fn fingerprint(&self) -> Digest { self.guard.fingerprint() }
}

impl StableEncode for PreparedAdtSourceFields {
    const DOMAIN: &'static str = "lrr/v1/actual-adt-source-fields";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.schema());
        e.record(&self.bindings);
        e.u8(match self.kind { AdtKind::Struct => 0, AdtKind::Enum => 1, AdtKind::Union => 2 });
        e.string(&self.name);
        e.sequence(&self.variants, |e, variant| {
            e.string(&variant.name);
            e.sequence(&variant.fields, |e, field| { e.string(&field.name); e.record(&field.ty); });
        });
        e.option(&self.pending, |e, pending| match pending {
            SourceAdtFieldsPending::Environment => e.u8(0),
            SourceAdtFieldsPending::SourceDefinition => e.u8(1),
            SourceAdtFieldsPending::FieldType { variant, field } => { e.u8(2); e.u32(*variant); e.u32(*field); },
            SourceAdtFieldsPending::Source(pending) => { e.u8(3); e.u32(*pending as u32); },
        });
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}

/// Source-only route controls around unchanged imported official fixtures.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum SourceFieldControl {
    GenericPair,
    ArrayRecord,
    ArrayTuple,
    GenericUnion,
    ProjectionPending,
}

/// The caller selects actual declaration IDs from its real fixture environment.
/// Returned guards allow the same harness to assert withdrawal after revoking
/// its source inputs; no typed facts or ready solver provider is published.
#[cfg(test)]
pub(crate) fn check_actual_source_fields(
    db: &mut Database,
    inventory: &SealKey,
    definitions: &[(Digest, SourceFieldControl)],
) -> Result<(Vec<CurrentAdtSourceFields>, usize, usize), Diagnostic> {
    use super::{configuration::SolverConfigurationKey, model::{Arg, Args}};
    use crate::traits::term::{BoundKind, ParamKey};
    use rustc_type_ir::{self as ty, inherent::*, Interner};

    let input = db.input(&InputKey::DiscoverySeal(inventory.clone()))
        .ok_or_else(|| fault("actual field control catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        return Err(fault("actual field control catalog differs"));
    };
    let root = catalog.impl_catalog.as_ref()
        .ok_or_else(|| fault("actual field control root absent"))?.root();
    let configuration = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(
        SolverConfigurationKey::from_source(root),
    ))) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual field control configuration needs input")),
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        return Err(fault("actual field control configuration differs"));
    };
    let mut held = Vec::new();
    let mut encoded_fields = 0;
    let mut explicit_pending = 0;
    for &(definition, control) in definitions {
        let key = AdtSourceFieldsKey { inventory: inventory.clone(), definition };
        let current = match CurrentAdtSourceFields::capture(db, &key) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("actual field control source needs input")),
        };
        current.require_current(db)?;
        let again = match CurrentAdtSourceFields::capture(db, &key) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("actual repeated field source needs input")),
        };
        assert!(Arc::ptr_eq(&current.value, &again.value), "one declaration shares its immutable field memo");
        assert_eq!(current.fingerprint(), again.fingerprint());
        let plan = current.plan();
        assert_eq!(plan.key(), &key);
        let projection = matches!(control, SourceFieldControl::ProjectionPending);
        let (expected_kind, expected_name) = match control {
            SourceFieldControl::GenericPair => (AdtKind::Struct, "Pair"),
            SourceFieldControl::ArrayRecord => (AdtKind::Struct, "ArrayStruct"),
            SourceFieldControl::ArrayTuple => (AdtKind::Struct, "ArrayTuple"),
            SourceFieldControl::GenericUnion => (AdtKind::Union, "U"),
            SourceFieldControl::ProjectionPending => (AdtKind::Union, "MaybeItem"),
        };
        assert_eq!((plan.kind(), plan.name()), (expected_kind, expected_name));
        if projection {
            assert_eq!(plan.schema().variants[0].fields[0].name.as_deref(), Some("elem"));
            assert_eq!(plan.schema().variants[0].fields[1].name.as_deref(), Some("none"));
        }
        if let Some(pending) = plan.pending() {
            assert!(projection, "actual fixture field preparation is pending: {pending:?}");
            assert!(matches!(pending,
                SourceAdtFieldsPending::FieldType { .. }
                | SourceAdtFieldsPending::Source(SourceTypePending::ProjectionMetadata)
                | SourceAdtFieldsPending::Source(SourceTypePending::SourceDefinition)
            ), "projection control must expose its actual source/field prerequisite: {pending:?}");
            explicit_pending += 1;
            held.push(current);
            continue;
        }
        let layouts = plan.generic_layouts()?;
        let before_encoding = db.trace.events.len();
        let fields = configuration.with_source_context(db, Arc::clone(&input), Some(&layouts), |cx, _| {
            assert!(cx.services().ready(cx).is_err(), "field metadata must not mint complete solver facts");
            if projection {
                assert!(matches!(plan.materialize(cx, configuration),
                    Err(SourceAdtFieldsPending::Source(SourceTypePending::ProjectionMetadata))),
                    "actual associated field projection stays explicitly unrepresented");
                return 0;
            }
            let variants = plan.materialize(cx, configuration)
                .unwrap_or_else(|pending| panic!("actual source field encoding pending: {pending:?}"));
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].name.0, plan.name());
            let id = plan.bindings.definitions[&definition];
            let args = Args::identity_for_item(cx, id);
            let inference=super::infer::InferenceContext::new(cx,ty::TypingMode::PostAnalysis);
            let env=super::model::ParamEnv(cx.clauses(super::model::List::default()));
            super::infer::const_eval::check_missing_owner_env(id);
            let typed = variants[0].fields;
            let expected_names: &[&str] = match control {
                SourceFieldControl::GenericPair | SourceFieldControl::GenericUnion => &["a", "b"],
                SourceFieldControl::ArrayRecord => &["data"],
                SourceFieldControl::ArrayTuple => &["0"],
                SourceFieldControl::ProjectionPending => unreachable!(),
            };
            assert_eq!(typed.len(), expected_names.len());
            assert_eq!(plan.schema().variants[0].fields.len(), typed.len());
            for (index, (field, expected_name)) in typed.iter().zip(expected_names).enumerate() {
                assert_eq!(field.name.0, *expected_name);
                let declared = field.ty.skip_binder();
                let instantiated = field.ty(cx, args).skip_norm_wip();
                assert_eq!(declared, instantiated, "identity instantiation preserves the declared field");
                let parameter = match control {
                    SourceFieldControl::GenericUnion => index as u32,
                    _ => 0,
                };
                let source_key = ParamKey { definition, ordinal: parameter };
                let actual = cx.services().source_parameter(source_key, BoundKind::Type)
                    .expect("actual owner Type parameter");
                let parameter_ty = match control {
                    SourceFieldControl::ArrayRecord | SourceFieldControl::ArrayTuple => {
                        let ty::TyKind::Array(element, length) = instantiated.kind() else {
                            panic!("actual array field lost its source shape")
                        };
                        super::infer::const_eval::check_actual_non_alias(&inference,env,length);
                        let ty::ConstKind::Param(length) = length.kind() else {
                            panic!("actual const-generic array length lost its parameter")
                        };
                        assert_eq!(length.source(), ParamKey { definition, ordinal: 1 });
                        assert_eq!(length.kind(), BoundKind::Const);
                        assert!(matches!(args.items().get(length.index() as usize),
                            Some(Arg::Const(value)) if matches!(value.kind(),ty::ConstKind::Param(p) if p==length)));
                        element
                    }
                    _ => instantiated,
                };
                assert!(matches!(parameter_ty.kind(), ty::TyKind::Param(parameter) if parameter == actual));
                assert!(matches!(args.items().get(actual.index() as usize), Some(Arg::Type(value)) if *value==parameter_ty));
            }
            assert_eq!(plan.encode(cx, configuration).unwrap().len(), variants.len());
            typed.len()
        })?;
        assert_eq!(before_encoding, db.trace.events.len(), "field encoding/materialization must not query source");
        if projection { explicit_pending += 1; } else { encoded_fields += fields; }
        current.require_current(db)?;
        held.push(current);
    }
    Ok((held, encoded_fields, explicit_pending))
}
