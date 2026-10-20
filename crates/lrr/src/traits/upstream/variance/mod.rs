//! Actual source nominal variance inputs and the pinned compiler algorithm.
//! The owning SourceVarianceQueries memo calls preparation once and retains it.
use super::{
    configuration::SolverConfigurationKey,
    identity::DefinitionId,
    model::{self, AdtKind},
    prepared::{self, Cx},
    source_adt_fields::{AdtSourceFieldsKey, PreparedAdtSourceFields},
    source_definition_headers::DefinitionHeaderKey,
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
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    traits::term::{GenericArg, TyTerm},
};
use rustc_type_ir::inherent::Ty as _;
use rustc_type_ir::{self as ty, Interner, inherent::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use typed_arena::Arena;
mod adapter;
mod constraints;
mod solve;
mod terms;
use adapter::{VarianceGenerics, VarianceParameterKind, VarianceSource};
use adapter::{VarianceInputs, VarianceItem, VarianceLangItems, require_supported};
type Ty<'cx> = model::Ty<'cx, prepared::PreparedProvider<'cx>>;
type Args<'cx> = model::Args<'cx, prepared::PreparedProvider<'cx>>;
type VarianceMapEntries = BTreeMap<DefinitionId, Vec<ty::Variance>>;
struct VarianceMap {
    variances: VarianceMapEntries,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum VariancePending {
    Fields,
    Foreign,
    LangIdentity,
    Signature,
    TypeShape,
    FreeAlias,
    GenericLayout,
}
impl StableEncode for VariancePending {
    const DOMAIN: &'static str = "lrr/v1/source-variance-pending";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(*self as u32);
    }
}

/// Source-only immutable result. Partial input prefixes are retained with an
/// explicit pending outcome, never published as complete variance vectors.
#[derive(Debug)]
pub(crate) struct PreparedNominalVariances {
    key: DefinitionHeaderKey,
    catalog: Arc<InputValue>,
    fields: Vec<Arc<QueryValue>>,
    languages: Vec<Arc<QueryValue>>,
    configuration: Option<Arc<QueryValue>>,
    pending: Option<VariancePending>,
    values: Option<Vec<ty::Variance>>,
}
impl PreparedNominalVariances {
    pub(crate) fn key(&self) -> &DefinitionHeaderKey {
        &self.key
    }
    pub(crate) fn pending(&self) -> Option<VariancePending> {
        self.pending
    }
    pub(crate) fn variances(&self) -> Result<&[ty::Variance], VariancePending> {
        match (self.pending, &self.values) {
            (None, Some(values)) => Ok(values),
            (Some(pending), None) => Err(pending),
            _ => unreachable!("immutable variance result has one completed outcome"),
        }
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

/// Follow actual field Type dependencies only. This computes no variance and
/// neither selects impls nor normalizes projections. The pinned simultaneous
/// inference domain must contain every referenced local nominal with args.
fn field_dependencies(plan: &PreparedAdtSourceFields) -> Result<BTreeSet<Digest>, VariancePending> {
    let mut dependencies = BTreeSet::new();
    let mut work = plan.source_field_types().collect::<Vec<_>>();
    while let Some(ty) = work.pop() {
        if let Some((_, pointee)) = crate::traits::pointer_terms::as_raw_pointer(ty) {
            work.push(pointee);
            continue;
        }
        match ty {
            TyTerm::Nominal(id, args) => {
                if !args.is_empty() {
                    dependencies.insert(*id);
                }
                work.extend(args.iter().filter_map(|arg| {
                    if let GenericArg::Type(ty) = arg {
                        Some(ty)
                    } else {
                        None
                    }
                }));
            }
            TyTerm::Projection(projection) => {
                work.push(&projection.trait_ref.self_ty);
                work.extend(projection.arguments().filter_map(|arg| {
                    if let GenericArg::Type(ty) = arg {
                        Some(ty)
                    } else {
                        None
                    }
                }));
            }
            TyTerm::Tuple(fields) => work.extend(fields),
            TyTerm::Array(ty, _) | TyTerm::Slice(ty) | TyTerm::Ref(_, _, ty) => work.push(ty),
            TyTerm::Function(_) => return Err(VariancePending::Signature),
            TyTerm::Dyn(_)
            | TyTerm::Opaque(..)
            | TyTerm::Infer(_)
            | TyTerm::Placeholder(_)
            | TyTerm::Bound { .. } => return Err(VariancePending::TypeShape),
            _ => {}
        }
    }
    Ok(dependencies)
}

pub(crate) fn prepare_nominal(
    db: &mut Database,
    key: &DefinitionHeaderKey,
) -> QueryPoll<PreparedNominalVariances> {
    let Some(catalog) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
        return QueryPoll::Ready(Err(fault("nominal variance source catalog absent")));
    };
    let InputValue::DiscoverySeal(publication) = catalog.as_ref() else {
        return QueryPoll::Ready(Err(fault("nominal variance source catalog differs")));
    };
    if let Err(error) = prepared::catalog_world(publication) {
        return QueryPoll::Ready(Err(error));
    }
    let root = publication
        .impl_catalog
        .as_ref()
        .expect("checked actual source catalog")
        .root();
    let root_crate = root.outline.crate_key;
    let configuration_key = SolverConfigurationKey::from_source(root);
    let mut result = PreparedNominalVariances {
        key: key.clone(),
        catalog,
        fields: Vec::new(),
        languages: Vec::new(),
        configuration: None,
        pending: Some(VariancePending::Fields),
        values: None,
    };
    macro_rules! pending {
        ($why:expr) => {{
            result.pending = Some($why);
            return QueryPoll::Ready(Ok(result));
        }};
    }
    let mut work = BTreeSet::from([key.definition]);
    let mut seen = BTreeSet::new();
    while let Some(definition) = work.pop_first() {
        if !seen.insert(definition) {
            continue;
        }
        let query = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::AdtSourceFields(Box::new(AdtSourceFieldsKey {
                inventory: key.inventory.clone(),
                definition,
            }))
        ))
        .value;
        let QueryValue::AdtSourceFields(plan) = query.as_ref() else {
            return QueryPoll::Ready(Err(fault("variance actual field query differs")));
        };
        if plan.pending().is_some() {
            result.fields.push(query);
            pending!(VariancePending::Fields);
        }
        if plan.schema().scope.crate_key != root_crate {
            result.fields.push(query);
            pending!(VariancePending::Foreign);
        }
        let dependencies = match field_dependencies(plan) {
            Ok(dependencies) => dependencies,
            Err(pending) => {
                result.fields.push(query);
                pending!(pending);
            }
        };
        work.extend(dependencies);
        result.fields.push(query);
        let language = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceLangIdentity(Box::new(DefinitionHeaderKey {
                inventory: key.inventory.clone(),
                definition,
            }))
        ))
        .value;
        let QueryValue::SourceLangIdentity(plan) = language.as_ref() else {
            return QueryPoll::Ready(Err(fault("variance actual lang identity query differs")));
        };
        if plan.classification().is_err() {
            result.languages.push(language);
            pending!(VariancePending::LangIdentity);
        }
        result.languages.push(language);
    }
    let plans = result
        .fields
        .iter()
        .map(|query| match query.as_ref() {
            QueryValue::AdtSourceFields(plan) => plan.as_ref(),
            _ => unreachable!("retained field queries"),
        })
        .collect::<Vec<_>>();
    let bindings = plans
        .iter()
        .map(|plan| plan.bindings())
        .collect::<Result<Vec<_>, _>>();
    let bindings = match bindings {
        Ok(bindings) => bindings,
        Err(_) => pending!(VariancePending::Fields),
    };
    let layouts = match SourceGenericLayouts::from_bindings(Arc::clone(&result.catalog), bindings) {
        Ok(layouts) => layouts,
        Err(_) => pending!(VariancePending::GenericLayout),
    };
    result.configuration = Some(
        ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SolverConfiguration(Box::new(configuration_key))
        ))
        .value,
    );
    let Some(QueryValue::SolverConfiguration(configuration)) = result.configuration.as_deref()
    else {
        return QueryPoll::Ready(Err(fault("variance actual source configuration differs")));
    };
    let computed = match configuration.with_source_context(
        db,
        Arc::clone(&result.catalog),
        Some(&layouts),
        |cx, _| {
            // This context supplies actual generic construction facts; the
            // variance adapter reads no complete DefinitionFacts callbacks.
            let mut items = BTreeMap::new();
            let mut lang = VarianceLangItems::default();
            for (plan, language) in plans.iter().zip(&result.languages) {
                let id = plan
                    .bindings()
                    .map_err(|_| VariancePending::Fields)?
                    .definitions[&plan.key().definition];
                if !<DefinitionId as ty::inherent::DefId<Cx<'_>>>::is_local(id) {
                    return Err(VariancePending::Foreign);
                }
                if !matches!(
                    plan.kind(),
                    AdtKind::Struct | AdtKind::Enum | AdtKind::Union
                ) {
                    return Err(VariancePending::TypeShape);
                }
                let args = Args::identity_for_item(cx, id);
                let variants = plan
                    .materialize(cx, configuration)
                    .map_err(|_| VariancePending::Fields)?;
                let fields = variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter())
                    .map(|field| field.ty(cx, args).skip_norm_wip())
                    .collect();
                let generics = VarianceGenerics::nominal(cx.generics_of(id))?;
                let value = model::Ty::new_adt(
                    cx,
                    model::AdtDefinition {
                        context: cx,
                        definition: id,
                    },
                    args,
                );
                items.insert(
                    id,
                    VarianceItem {
                        generics,
                        ty: value,
                        fields,
                        signature: None,
                    },
                );
                let QueryValue::SourceLangIdentity(language) = language.as_ref() else {
                    unreachable!("retained lang source")
                };
                let role = language
                    .classification()
                    .map_err(|_| VariancePending::LangIdentity)?;
                let slot = match role.map(|(name, _)| name) {
                    Some("phantom_data") => Some(&mut lang.phantom_data),
                    Some("unsafe_cell") => Some(&mut lang.unsafe_cell),
                    Some("covariant_unsafe_cell") => Some(&mut lang.covariant_unsafe_cell),
                    _ => None,
                };
                if let Some(slot) = slot
                    && slot.replace(id).is_some()
                {
                    return Err(VariancePending::LangIdentity);
                }
            }
            let inputs = VarianceInputs { items, lang };
            require_supported(cx, &inputs)?;
            let target = inputs
                .items
                .keys()
                .copied()
                .find(|id| id.source() == key.definition)
                .ok_or(VariancePending::GenericLayout)?;
            // Exact upstream zero-generics branch. No empty fallback on missing
            // algorithm output is allowed for a nonzero source binder.
            if inputs.items[&target].generics.is_empty() {
                return Ok(Vec::new());
            }
            let arena = Arena::new();
            let source = VarianceSource { inputs: &inputs };
            let terms = terms::determine_parameters_to_be_inferred(source, &arena);
            let constraints = constraints::add_constraints_from_crate(terms);
            let mut output = solve::solve_constraints(constraints);
            output
                .variances
                .remove(&target)
                .ok_or(VariancePending::GenericLayout)
        },
    ) {
        Ok(result) => result,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    match computed {
        Ok(values) => {
            result.values = Some(values);
            result.pending = None;
        }
        Err(pending) => result.pending = Some(pending),
    }
    QueryPoll::Ready(Ok(result))
}

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
impl StableEncode for PreparedNominalVariances {
    const DOMAIN: &'static str = "lrr/v1/pinned-source-nominal-variances";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.catalog.fingerprint());
        e.sequence(&self.fields, |e, root| {
            let QueryValue::AdtSourceFields(plan) = root.as_ref() else {
                unreachable!()
            };
            e.record(plan.as_ref());
        });
        e.sequence(&self.languages, |e, root| {
            let QueryValue::SourceLangIdentity(plan) = root.as_ref() else {
                unreachable!()
            };
            e.record(plan.as_ref());
        });
        e.option(&self.configuration, |e, root| {
            let QueryValue::SolverConfiguration(plan) = root.as_ref() else {
                unreachable!()
            };
            e.record(plan.as_ref());
        });
        e.option(&self.pending, |e, pending| e.record(pending));
        e.option(&self.values, |e, values| {
            e.sequence(values, |e, value| {
                e.u32(match value {
                    ty::Covariant => 0,
                    ty::Invariant => 1,
                    ty::Contravariant => 2,
                    ty::Bivariant => 3,
                })
            })
        });
    }
}
