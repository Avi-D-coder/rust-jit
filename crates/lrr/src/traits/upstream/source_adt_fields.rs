//! Actual declaration field Types through the ordinary source Type service.
//! No body preparation, guest effects, independent inference or solver entry.
use super::{
    configuration::PreparedSolverConfiguration,
    model::{AdtField, AdtKind, AdtVariant, Symbol},
    prepared::{self, Cx, DefinitionReceipt},
    source_bindings::{DefinitionKind, SourceBindings, SourceRequirements, require_type},
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
        let mut required = SourceRequirements::from(BTreeMap::from([(key.definition, DefinitionKind::Nominal)]));
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
                let ty = super::source_terms::encode_declared(cx, configuration, bindings.type_bindings(), &field.ty)
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
    ProjectedField,
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
    let explicit_pending = 0;
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
        let projection = matches!(control, SourceFieldControl::ProjectedField);
        let (expected_kind, expected_name) = match control {
            SourceFieldControl::GenericPair => (AdtKind::Struct, "Pair"),
            SourceFieldControl::ArrayRecord => (AdtKind::Struct, "ArrayStruct"),
            SourceFieldControl::ArrayTuple => (AdtKind::Struct, "ArrayTuple"),
            SourceFieldControl::GenericUnion => (AdtKind::Union, "U"),
            SourceFieldControl::ProjectedField => (AdtKind::Union, "MaybeItem"),
        };
        assert_eq!((plan.kind(), plan.name()), (expected_kind, expected_name));
        if projection {
            assert_eq!(plan.schema().variants[0].fields[0].name.as_deref(), Some("elem"));
            assert_eq!(plan.schema().variants[0].fields[1].name.as_deref(), Some("none"));
        }
        assert_eq!(plan.pending(), None, "actual fixture field preparation must be ready");
        let layouts = plan.generic_layouts()?;
        let before_encoding = db.trace.events.len();
        let fields = configuration.with_source_context(db, Arc::clone(&input), Some(&layouts), |cx, _| {
            assert!(cx.services().ready(cx).is_err(), "field metadata must not mint complete solver facts");
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
            if projection {
                assert_eq!(typed.len(), 2);
                assert_eq!((typed[0].name.0, typed[1].name.0), ("elem", "none"));
                let TyTerm::Nominal(wrapper, source_args) = &plan.variants[0].fields[0].ty else { panic!("actual MaybeItem field wrapper source lost") };
                let Some(crate::traits::term::GenericArg::Type(TyTerm::Projection(source))) = source_args.first() else { panic!("actual MaybeItem field source projection lost") };
                let binding = &plan.bindings.projections[&(source.trait_ref.trait_key, source.item)];
                let ty::TyKind::Adt(actual_wrapper, args) = typed[0].ty.skip_binder().kind() else { panic!("actual projected field wrapper shared Type lost") };
                assert_eq!(actual_wrapper.definition.source(), *wrapper);
                let Some(Arg::Type(projected)) = args.items().first() else { panic!("actual projected wrapper argument lost") };
                super::source_terms::check_actual_projection_alias(cx, *projected,
                    ParamKey { definition, ordinal: 0 }, binding.trait_definition.source(), binding.associated_definition.source());
                assert!(matches!(typed[1].ty.skip_binder().kind(), ty::TyKind::Tuple(fields) if fields.items().is_empty()));
                return typed.len();
            }
            let expected_names: &[&str] = match control {
                SourceFieldControl::GenericPair | SourceFieldControl::GenericUnion => &["a", "b"],
                SourceFieldControl::ArrayRecord => &["data"],
                SourceFieldControl::ArrayTuple => &["0"],
                SourceFieldControl::ProjectedField => unreachable!(),
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
        encoded_fields += fields;
        current.require_current(db)?;
        held.push(current);
    }
    Ok((held, encoded_fields, explicit_pending))
}

/// External fixture controls supply independent actual trait/member IDs.
#[cfg(test)]
pub(crate) fn check_actual_projected_source_fields(
    db: &mut Database, inventory: &SealKey, owner: Digest,
    expected_trait: Digest, expected_item: Digest,
) -> Result<(CurrentAdtSourceFields, usize), Diagnostic> {
    use super::{configuration::SolverConfigurationKey, model::{Arg, Args}};
    use rustc_type_ir::{self as ty, inherent::*};
    let current = match CurrentAdtSourceFields::capture(db, &AdtSourceFieldsKey { inventory: inventory.clone(), definition: owner }) {
        QueryPoll::Ready(Ok(value)) => value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual projected field preparation needs input")),
    };
    current.require_current(db)?;
    let plan = current.plan();
    assert_eq!(plan.pending(), None, "actual projected field prerequisites must be complete");
    assert_eq!(plan.kind(), AdtKind::Union);
    let TyTerm::Nominal(wrapper, source_args) = &plan.variants[0].fields[0].ty else { panic!("actual projected field source wrapper absent") };
    let Some(crate::traits::term::GenericArg::Type(TyTerm::Projection(source))) = source_args.first() else { panic!("actual projected field source term absent") };
    assert_eq!(source.trait_ref.trait_key, expected_trait);
    let binding = &plan.bindings.projections[&(expected_trait, source.item)];
    assert_eq!(binding.associated_definition.source(), expected_item);
    assert_eq!(binding.trait_definition.source(), expected_trait);
    let input = Arc::clone(&plan.bindings.catalog);
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else { unreachable!("actual source catalog") };
    let configuration = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(
        SolverConfigurationKey::from_source(catalog.impl_catalog.as_ref().expect("checked catalog").root()),
    ))) {
        QueryPoll::Ready(Ok(value)) => value.value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual projected field configuration needs input")),
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else { unreachable!("actual field configuration") };
    let layouts = plan.generic_layouts()?;
    let before = db.trace.events.len();
    let fields = configuration.with_source_context(db, input, Some(&layouts), |cx, _| {
        assert!(cx.services().ready(cx).is_err());
        let variants = plan.materialize(cx, configuration).unwrap_or_else(|pending| panic!("actual projected field encoding pending: {pending:?}"));
        assert_eq!(variants.len(), 1); assert_eq!(variants[0].fields.len(), 2);
        let fields = variants[0].fields;
        assert_eq!((fields[0].name.0, fields[1].name.0), ("elem", "none"));
        let args = Args::identity_for_item(cx, plan.bindings.definitions[&owner]);
        let ty::TyKind::Adt(actual_wrapper, actual_args) = fields[0].ty(cx, args).skip_norm_wip().kind() else { panic!("actual projected field wrapper Type absent") };
        assert_eq!(actual_wrapper.definition.source(), *wrapper);
        let Some(Arg::Type(projected)) = actual_args.items().first() else { panic!("actual projected wrapper argument absent") };
        super::source_terms::check_actual_projection_alias(cx, *projected,
            crate::traits::term::ParamKey { definition: owner, ordinal: 0 }, expected_trait, expected_item);
        assert!(matches!(fields[1].ty(cx, args).skip_norm_wip().kind(), ty::TyKind::Tuple(fields) if fields.items().is_empty()));
        assert!(cx.services().ready(cx).is_err());
        fields.len()
    })?;
    assert_eq!(before, db.trace.events.len(), "projected field TypeIR construction must not query source");
    current.require_current(db)?;
    Ok((current, fields))
}

/// Independent schema expectations supplied by the host fixture harness.
/// These select observations of actual source Types; they construct no recipe.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum ProjectionSelfExpectation {
    Parameter(crate::traits::term::ParamKey),
    Nominal(Digest),
}
#[cfg(test)]
pub(crate) struct DeclaredProjectionFieldExpectation<'a> {
    pub(crate) field_name: &'a str,
    pub(crate) wrappers: &'a [Digest],
    pub(crate) trait_definition: Digest,
    pub(crate) associated_definition: Digest,
    pub(crate) self_type: ProjectionSelfExpectation,
    pub(crate) unsigned_trait_arguments: &'a [rustc_type_ir::UintTy],
    pub(crate) parent_count: usize,
}

/// Inspect one declared field through the ordinary actual-source encoder.
/// Actual generated publication IDs and expected field shape come from an
/// independent Outline selection; no normalization or ready facts are used.
#[cfg(test)]
pub(crate) fn check_actual_declared_projection_field(
    db: &mut Database, inventory: &SealKey, owner: Digest,
    expected: &DeclaredProjectionFieldExpectation<'_>,
) -> Result<(CurrentAdtSourceFields, usize), Diagnostic> {
    use super::{configuration::SolverConfigurationKey, model::{AllocateList, Arg, Args}};
    use crate::traits::term::BoundKind;
    use rustc_type_ir::{self as ty, inherent::*, Interner};
    let current = match CurrentAdtSourceFields::capture(db, &AdtSourceFieldsKey {
        inventory: inventory.clone(), definition: owner,
    }) {
        QueryPoll::Ready(Ok(value)) => value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("declared projection field source needs input")),
    };
    current.require_current(db)?;
    let plan = current.plan();
    if let Some(pending) = plan.pending() {
        return Err(fault(&format!("actual declared projection field prerequisite pending: {pending:?}")));
    }
    assert_eq!(plan.kind(), AdtKind::Struct);
    assert_eq!(plan.variants.len(), 1);
    assert_eq!(plan.variants[0].fields.len(), 1);
    assert_eq!(plan.variants[0].fields[0].name, expected.field_name);
    let mut source = &plan.variants[0].fields[0].ty;
    for wrapper in expected.wrappers {
        let TyTerm::Nominal(actual, arguments) = source else { panic!("actual field nominal wrapper missing") };
        assert_eq!(actual, wrapper);
        let [crate::traits::term::GenericArg::Type(argument)] = arguments.as_slice() else {
            panic!("actual field wrapper must retain its one Type argument")
        };
        source = argument;
    }
    let TyTerm::Projection(source) = source else { panic!("actual declared field projection missing") };
    assert_eq!(source.trait_ref.trait_key, expected.trait_definition);
    let binding = &plan.bindings.projections[&(expected.trait_definition, source.item)];
    assert_eq!(binding.associated_definition.source(), expected.associated_definition);
    assert_eq!(binding.trait_definition.source(), expected.trait_definition);
    let layouts = plan.generic_layouts()?;
    assert_eq!(layouts.projection_parent(binding.associated_definition), Some(binding.trait_definition));
    let expected_kinds = vec![BoundKind::Type; expected.parent_count];
    assert_eq!(layouts.projection_argument_kinds(binding.associated_definition), Some(expected_kinds.as_slice()));
    let associated = layouts.get(binding.associated_definition).expect("actual associated generic declaration");
    let layout = associated.layout().expect("actual associated source binder");
    assert!(layout.implicit_self);
    assert_eq!(layout.own, 0);
    assert_eq!(layout.inherited as usize + 1, expected.parent_count);
    let input = Arc::clone(&plan.bindings.catalog);
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else { unreachable!("actual field catalog") };
    let configuration = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(
        SolverConfigurationKey::from_source(catalog.impl_catalog.as_ref().expect("actual source catalog").root()),
    ))) {
        QueryPoll::Ready(Ok(value)) => value.value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("declared projection field configuration needs input")),
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else { unreachable!("actual field configuration") };
    let before = db.trace.events.len();
    let count = configuration.with_source_context(db, input, Some(&layouts), |cx, _| {
        assert!(cx.services().ready(cx).is_err());
        let variants = plan.materialize(cx, configuration)
            .unwrap_or_else(|pending| panic!("actual declared field TypeIR pending: {pending:?}"));
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].fields.len(), 1);
        let field = variants[0].fields[0];
        assert_eq!(field.name.0, expected.field_name);
        let owner_args = Args::identity_for_item(cx, plan.bindings.definitions[&owner]);
        let mut value = field.ty(cx, owner_args).skip_norm_wip();
        for wrapper in expected.wrappers {
            let ty::TyKind::Adt(actual, arguments) = value.kind() else { panic!("encoded field wrapper missing") };
            assert_eq!(actual.definition.source(), *wrapper);
            assert_eq!(actual.definition.world(), cx.services().source_world());
            let [Arg::Type(argument)] = arguments.items() else { panic!("encoded wrapper argument layout differs") };
            value = *argument;
        }
        let ty::TyKind::Alias(ty::IsRigid::No, alias) = value.kind() else { panic!("field must retain an unnormalized alias") };
        let ty::AliasTyKind::Projection { def_id } = alias.kind else { panic!("field lost its projection alias kind") };
        assert_eq!(def_id, binding.associated_definition);
        assert_eq!(cx.projection_parent(def_id), binding.trait_definition);
        assert_eq!(cx.generics_of(def_id).parent_count, expected.parent_count);
        assert_eq!(cx.generics_of(def_id).count(), expected.parent_count);
        assert!(cx.check_alias_term_args_compatible(alias.kind.into(), alias.args));
        let wrong = cx.list(Vec::<Arg<'_, prepared::PreparedProvider<'_>>>::new());
        assert!(!cx.check_alias_term_args_compatible(alias.kind.into(), wrong));
        assert_eq!(ty::AliasTy::new_from_args(cx, alias.kind, alias.args), alias);
        let (reference, own) = alias.trait_ref_and_own_args(cx);
        assert_eq!(reference.def_id, binding.trait_definition);
        assert!(own.is_empty());
        assert_eq!(reference.args.items().len(), 1 + expected.unsigned_trait_arguments.len());
        let Arg::Type(self_type) = reference.args.items()[0] else { panic!("actual trait Self Type missing") };
        match expected.self_type {
            ProjectionSelfExpectation::Parameter(expected) => assert!(matches!(self_type.kind(),
                ty::TyKind::Param(actual) if actual.source() == expected)),
            ProjectionSelfExpectation::Nominal(expected) => {
                let ty::TyKind::Adt(actual, arguments) = self_type.kind() else { panic!("actual nominal trait Self missing") };
                assert_eq!(actual.definition.source(), expected);
                assert_eq!(actual.definition.world(), cx.services().source_world());
                assert!(arguments.items().is_empty());
            }
        }
        for (argument, expected) in reference.args.items()[1..].iter().zip(expected.unsigned_trait_arguments) {
            assert!(matches!(argument, Arg::Type(actual) if matches!(actual.kind(), ty::TyKind::Uint(kind) if kind == *expected)));
        }
        assert!(cx.services().ready(cx).is_err());
        variants[0].fields.len()
    })?;
    assert_eq!(before, db.trace.events.len(), "declared projection encoding must not query source");
    current.require_current(db)?;
    Ok((current, count))
}
