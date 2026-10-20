//! Demanded ordinary impl requirements -> actual shared compiler clauses.
//! This prepares source facts, not applicability, a solver answer or VM entry.
use super::{
    configuration::PreparedSolverConfiguration,
    identity::DefinitionId,
    model::Clause,
    prepared::{self, Cx},
    source_bindings::{DefinitionKind, SourceBindings, SourceRequirements, require_type},
    source_terms::SourceTypePending,
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
    frontend::impl_predicates::{ImplPredicateResult, PreparedImplPredicates},
    traits::{
        header_syntax::ImplHeaderSyntaxKey,
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::{GenericArg, TraitRef},
    },
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourcePredicatePending {
    Head,
    Bound(PredicatePending),
    Sized,
    Source(SourceTypePending),
}
#[derive(Debug)]
pub struct PreparedImplSourcePredicates {
    key: ImplHeaderSyntaxKey,
    requirements: Arc<QueryValue>,
    bindings: SourceBindings,
    pending: Option<SourcePredicatePending>,
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
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
fn require_trait(required: &mut SourceRequirements, reference: &TraitRef) {
    required.insert(reference.trait_key, DefinitionKind::Trait);
    require_type(required, &reference.self_ty);
    for arg in &reference.args {
        if let GenericArg::Type(ty) = arg {
            require_type(required, ty);
        }
    }
}
impl PreparedImplSourcePredicates {
    pub fn key(&self) -> &ImplHeaderSyntaxKey {
        &self.key
    }
    pub fn pending(&self) -> Option<SourcePredicatePending> {
        self.pending
    }
    fn requirements(&self) -> &PreparedImplPredicates {
        let QueryValue::ImplPredicates(requirements) = self.requirements.as_ref() else {
            unreachable!("captured actual impl requirements")
        };
        requirements
    }
    pub(super) fn bindings(&self) -> &SourceBindings {
        &self.bindings
    }
    pub(crate) fn prepare(db: &mut Database, key: &ImplHeaderSyntaxKey) -> QueryPoll<Self> {
        let requirements = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::ImplPredicates(Box::new(key.clone()))
        ))
        .value;
        let Some(input) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("source predicate catalog absent")));
        };
        let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault("source predicate catalog variant differs")));
        };
        if let Err(error) = prepared::catalog_world(catalog) {
            return QueryPoll::Ready(Err(error));
        }
        let mut plan = Self {
            key: key.clone(),
            requirements,
            bindings: SourceBindings::empty(Arc::clone(&input)),
            pending: None,
        };
        let ImplPredicateResult::Requirements(recipes) = plan.requirements().result() else {
            plan.pending = Some(SourcePredicatePending::Head);
            return QueryPoll::Ready(Ok(plan));
        };
        let syntax = plan.requirements().head().syntax();
        let Some(owner) = syntax.canonical_owner() else {
            return QueryPoll::Ready(Err(fault("source predicate canonical owner absent")));
        };
        let definition = owner.identity();
        let mut required = SourceRequirements::from(BTreeMap::from([(definition, DefinitionKind::Impl)]));
        for recipe in recipes {
            match recipe {
                SourcePredicateRecipe::ImplicitSizedDefault(_) => {
                    plan.pending = Some(SourcePredicatePending::Sized);
                    return QueryPoll::Ready(Ok(plan));
                }
                SourcePredicateRecipe::ResolvedSizedDefault(reference) => {
                    require_trait(&mut required, reference)
                }
                SourcePredicateRecipe::Explicit { result, .. } => match result {
                    PredicateResult::Implemented(reference)
                    | PredicateResult::Relaxed {
                        trait_ref: reference,
                        ..
                    } => require_trait(&mut required, reference),
                    PredicateResult::TypeOutlives { ty, .. } => require_type(&mut required, ty),
                    PredicateResult::RegionOutlives { .. } => {}
                    PredicateResult::Pending(why) => {
                        plan.pending = Some(SourcePredicatePending::Bound(*why));
                        return QueryPoll::Ready(Ok(plan));
                    }
                },
            }
        }
        plan.bindings = ready!(SourceBindings::prepare(db, input, required));
        plan.pending = plan.bindings.pending.map(SourcePredicatePending::Source);
        QueryPoll::Ready(Ok(plan))
    }
    /// Construct own (not inherited/elaborated/proven) clauses inside the exact
    /// real generic/source context. A pending recipe never becomes an empty env.
    pub(super) fn encode<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<
        Vec<(
            Clause<'cx, prepared::PreparedProvider<'cx>>,
            super::model::Span,
        )>,
        SourcePredicatePending,
    > {
        if let Some(pending) = self.pending {
            return Err(pending);
        }
        let InputValue::DiscoverySeal(catalog) = self.bindings.catalog.as_ref() else {
            unreachable!("captured catalog")
        };
        if configuration.source()
            != catalog
                .impl_catalog
                .as_ref()
                .expect("checked catalog")
                .root()
            || cx.services().source_world() != catalog.seal.fingerprint()
        {
            return Err(SourcePredicatePending::Source(
                SourceTypePending::SourceDefinition,
            ));
        }
        let ImplPredicateResult::Requirements(recipes) = self.requirements().result() else {
            return Err(SourcePredicatePending::Head);
        };
        let syntax = self.requirements().head().syntax();
        let mut clauses = Vec::new();
        for recipe in recipes {
            let implicit;
            let (predicate, source) = match recipe {
                SourcePredicateRecipe::ResolvedSizedDefault(reference) => {
                    let crate::traits::term::TyTerm::Param(parameter) = reference.self_ty else {
                        return Err(SourcePredicatePending::Source(
                            SourceTypePending::ParameterLayout,
                        ));
                    };
                    let source = self.requirements().parameter_range(parameter).ok_or(
                        SourcePredicatePending::Source(SourceTypePending::ParameterLayout),
                    )?;
                    implicit = PredicateResult::Implemented(reference.clone());
                    (&implicit, source)
                }
                SourcePredicateRecipe::Explicit {
                    result: PredicateResult::Relaxed { .. },
                    ..
                } => continue,
                SourcePredicateRecipe::Explicit {
                    result: PredicateResult::Pending(why),
                    ..
                } => {
                    return Err(SourcePredicatePending::Bound(*why));
                }
                SourcePredicateRecipe::Explicit { source, result } => (result, *source),
                SourcePredicateRecipe::ImplicitSizedDefault(_) => {
                    return Err(SourcePredicatePending::Sized);
                }
            };
            let clause = super::source_terms::encode_predicate(
                cx,
                configuration,
                self.bindings.type_bindings(),
                predicate,
            )
            .map_err(SourcePredicatePending::Source)?;
            clauses.push((
                clause,
                super::model::Span {
                    origin: syntax.source().content,
                    start: source.start,
                    end: source.end,
                },
            ));
        }
        Ok(clauses)
    }
}
impl StableEncode for PreparedImplSourcePredicates {
    const DOMAIN: &'static str = "lrr/v1/actual-source-impl-clauses";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.requirements());
        e.record(&self.bindings);
        // Exact bound/head pending reason is already encoded in retained Q59.
        e.option(&self.pending, |e, why| match why {
            SourcePredicatePending::Head => e.u32(0),
            SourcePredicatePending::Bound(_) => e.u32(1),
            SourcePredicatePending::Sized => e.u32(2),
            SourcePredicatePending::Source(why) => {
                e.u32(3);
                e.u32(*why as u32);
            }
        });
    }
}

/// Actual memo roots and joined input closure for metadata-only declaration
/// clause controls. This is not an impl record or ready solver provider.
#[cfg(test)]
pub(crate) struct CurrentSourcePredicates {
    _source: Arc<QueryValue>,
    _bindings: SourceBindings,
    guard: crate::db::memo::PreparedInputGuard,
}
#[cfg(test)]
impl CurrentSourcePredicates {
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> { self.guard.require_current(db) }
}
#[cfg(test)]
pub(crate) fn check_actual_projected_source_predicates(
    db: &mut Database, inventory: &crate::db::seals::SealKey, owner: Digest,
    expected_trait: Digest, expected_item: Digest, expected_bound_trait: Digest,
) -> Result<(CurrentSourcePredicates, usize, usize), Diagnostic> {
    use super::{configuration::SolverConfigurationKey, source_generics::SourceGenericLayouts};
    use crate::frontend::declaration_source_predicates::DeclarationSourcePredicatesKey;
    use crate::traits::{predicate::PredicateResult, term::{TyTerm, ParamKey}};
    use rustc_type_ir::{self as ty, inherent::*};
    let query = QueryKey::DeclarationSourcePredicates(Box::new(DeclarationSourcePredicatesKey { inventory: inventory.clone(), definition: owner }));
    let source = match db.fetch(&SyntaxProvider, query.clone()) {
        QueryPoll::Ready(Ok(value)) => value.value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual declaration predicates need source input")),
    };
    let QueryValue::DeclarationSourcePredicates(plan) = source.as_ref() else { return Err(fault("actual declaration predicate query differs")); };
    let recipes = plan.recipes().map_err(|pending| fault(&format!("actual declaration predicate metadata pending: {pending:?}")))?;
    let input = db.input(&InputKey::DiscoverySeal(inventory.clone())).ok_or_else(|| fault("actual declaration predicate catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else { return Err(fault("actual declaration predicate catalog differs")); };
    let mut required = SourceRequirements::from(BTreeMap::from([(owner, DefinitionKind::Declaration)]));
    for recipe in recipes {
        match recipe {
            SourcePredicateRecipe::ResolvedSizedDefault(reference) => require_trait(&mut required, reference),
            SourcePredicateRecipe::Explicit { result: PredicateResult::Implemented(reference) | PredicateResult::Relaxed { trait_ref: reference, .. }, .. } => require_trait(&mut required, reference),
            SourcePredicateRecipe::Explicit { result: PredicateResult::TypeOutlives { ty, .. }, .. } => require_type(&mut required, ty),
            SourcePredicateRecipe::Explicit { result: PredicateResult::RegionOutlives { .. }, .. } => {},
            SourcePredicateRecipe::Explicit { result: PredicateResult::Pending(pending), .. } => return Err(fault(&format!("actual projected predicate remains pending: {pending:?}"))),
            SourcePredicateRecipe::ImplicitSizedDefault(_) => return Err(fault("actual declaration Sized source identity is not prepared")),
        }
    }
    let bindings = match SourceBindings::prepare(db, Arc::clone(&input), required) {
        QueryPoll::Ready(Ok(value)) => value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual projected predicate bindings need input")),
    };
    if let Some(pending) = bindings.pending { return Err(fault(&format!("actual projected predicate source closure pending: {pending:?}"))); }
    let owner_id = bindings.definitions[&owner];
    let QueryValue::DeclarationGenericTypes(actual) = bindings.generics[&owner_id].as_ref() else { unreachable!("actual owner generics") };
    let (parse, header, context, _) = plan.declaration().source_parts().expect("actual declared predicate source");
    assert!(actual.declaration().matches_metadata(parse, header, context));
    let configuration_query = QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
        catalog.impl_catalog.as_ref().expect("actual catalog root").root(),
    )));
    let configuration = match db.fetch(&SyntaxProvider, configuration_query.clone()) {
        QueryPoll::Ready(Ok(value)) => value.value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual projected predicate configuration needs input")),
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else { unreachable!("actual source configuration") };
    let mut keys = vec![query, configuration_query, QueryKey::DiscoverySeal(inventory.clone())];
    keys.extend(bindings.generics.keys().map(|id| QueryKey::DeclarationGenericTypes(Box::new(
        crate::frontend::declaration_generic_types::DeclarationGenericTypesKey { inventory: inventory.clone(), definition: id.source() },
    ))));
    keys.extend(bindings.projections.values().map(|binding| {
        let QueryValue::TraitAssociatedTypes(plan) = binding.query.as_ref() else { unreachable!("actual associated query") };
        QueryKey::TraitAssociatedTypes(Box::new(plan.key().clone()))
    }));
    let mut guards = Vec::new();
    for key in keys {
        let value = match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(value)) => value, QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("actual projected predicate guard needs input")),
        };
        let guard = match db.prepare_input_guard(&SyntaxProvider, &key, value.fingerprint) {
            QueryPoll::Ready(Ok(value)) => value, QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("actual projected predicate closure guard needs input")),
        };
        guards.push(guard);
    }
    let guard = crate::db::memo::PreparedInputGuard::join(&guards.iter().collect::<Vec<_>>())?;
    guard.require_current(db)?;
    let layouts = SourceGenericLayouts::from_bindings(Arc::clone(&input), [&bindings])?;
    let before = db.trace.events.len();
    let (count, projected) = configuration.with_source_context(db, Arc::clone(&input), Some(&layouts), |cx, _| {
        assert!(cx.services().ready(cx).is_err());
        let mut count = 0; let mut projected = 0;
        for recipe in recipes {
            let implicit;
            let predicate = match recipe {
                SourcePredicateRecipe::ResolvedSizedDefault(reference) => {
                    implicit = PredicateResult::Implemented(reference.clone()); &implicit
                }
                SourcePredicateRecipe::Explicit { result: PredicateResult::Relaxed { .. }, .. } => continue,
                SourcePredicateRecipe::Explicit { result, .. } => result,
                SourcePredicateRecipe::ImplicitSizedDefault(_) => panic!("unresolved actual Sized recipe"),
            };
            let clause = super::source_terms::encode_predicate(cx, configuration, bindings.type_bindings(), predicate)
                .unwrap_or_else(|pending| panic!("actual projected clause TypeIR pending: {pending:?}"));
            if let PredicateResult::Implemented(reference) = predicate
                && let TyTerm::Projection(projection) = &reference.self_ty {
                assert_eq!(reference.trait_key, expected_bound_trait);
                assert_eq!(projection.trait_ref.trait_key, expected_trait);
                assert_eq!(bindings.projections[&(expected_trait, projection.item)].associated_definition.source(), expected_item);
                let ty::ClauseKind::Trait(encoded) = clause.kind().skip_binder() else { panic!("actual projected bound lost its trait clause") };
                assert_eq!(encoded.trait_ref.def_id.source(), expected_bound_trait);
                assert_eq!(encoded.polarity, ty::ClausePolarity::Positive);
                assert_eq!(encoded.trait_ref.args.items().len(), 2);
                let [super::model::Arg::Type(self_ty), super::model::Arg::Type(argument)] = encoded.trait_ref.args.items() else { panic!("actual projected PartialEq arguments lost") };
                super::source_terms::check_actual_projection_alias(cx, *self_ty,
                    ParamKey { definition: owner, ordinal: 0 }, expected_trait, expected_item);
                assert!(matches!(argument.kind(), ty::TyKind::Param(parameter) if parameter.source()==(ParamKey { definition: owner, ordinal: 1 })));
                projected += 1;
            }
            count += 1;
        }
        assert!(cx.services().ready(cx).is_err());
        (count, projected)
    })?;
    assert_eq!(before, db.trace.events.len(), "actual projected clause encoding must not query source");
    guard.require_current(db)?;
    Ok((CurrentSourcePredicates { _source: source, _bindings: bindings, guard }, count, projected))
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct SourceClauseEvidence {
    pub(crate) implementations: usize,
    pub(crate) clauses: usize,
    pub(crate) traits: usize,
    pub(crate) regions: usize,
    pub(crate) types: usize,
    pub(crate) relaxations: usize,
    pub(crate) pending: Vec<SourcePredicatePending>,
}
/// Supplemental real-source route control. No SourceFacts/ReadyProvider,
/// synthetic candidate, client callback or proof is constructed here.
#[cfg(test)]
pub(crate) fn check_actual_clauses(
    db: &mut Database,
    key: &crate::db::seals::SealKey,
    local_unit: Digest,
) -> Result<SourceClauseEvidence, Diagnostic> {
    use super::{
        configuration::SolverConfigurationKey, model, source_generics::SourceGenericLayouts,
    };
    use rustc_type_ir::{self as ty, Interner, TypeVisitableExt, inherent::*};
    fn fetch(db: &mut Database, key: QueryKey) -> Result<Arc<QueryValue>, Diagnostic> {
        match db.fetch(&SyntaxProvider, key) {
            QueryPoll::Ready(Ok(value)) => Ok(value.value),
            QueryPoll::Ready(Err(error)) => Err(error),
            QueryPoll::NeedInput(_) => Err(fault("actual source clause control requires input")),
        }
    }
    let input = db
        .input(&InputKey::DiscoverySeal(key.clone()))
        .ok_or_else(|| fault("clause source catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        return Err(fault("clause source catalog differs"));
    };
    let source = catalog
        .impl_catalog
        .as_ref()
        .ok_or_else(|| fault("actual complete source catalog absent"))?
        .root();
    let configuration = fetch(
        db,
        QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(source))),
    )?;
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        unreachable!()
    };
    let headers = fetch(db, QueryKey::PreparedScopeImplHeaders(key.clone()))?;
    let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else {
        unreachable!()
    };
    let mut result = SourceClauseEvidence::default();
    let mut plans = Vec::new();
    let mut first = None;
    for raw in headers
        .inventory_root()
        .headers()
        .iter()
        .filter(|raw| raw.scope.crate_key == local_unit)
    {
        let query = QueryKey::ImplSourcePredicates(Box::new(ImplHeaderSyntaxKey {
            inventory: key.clone(),
            implementation: raw.identity,
        }));
        let value = fetch(db, query.clone())?;
        let reused = fetch(db, query.clone())?;
        assert!(Arc::ptr_eq(&value, &reused));
        let QueryValue::ImplSourcePredicates(plan) = value.as_ref() else {
            unreachable!()
        };
        let actual = fetch(db, QueryKey::ImplPredicates(Box::new(plan.key.clone())))?;
        assert!(
            Arc::ptr_eq(&actual, &plan.requirements),
            "reuse the common source-bound query"
        );
        result.implementations += 1;
        if let Some(pending) = plan.pending() {
            result.pending.push(pending);
        }
        if first.is_none() {
            first = Some(query);
        }
        plans.push(value);
    }
    let ready_plans = plans.iter().filter_map(|v| match v.as_ref() {
        QueryValue::ImplSourcePredicates(p) if p.pending().is_none() => Some(p.bindings()),
        _ => None,
    });
    let layouts = SourceGenericLayouts::from_bindings(Arc::clone(&input), ready_plans)?;
    let before = db.trace.events.len();
    configuration.with_source_context(
        db,
        Arc::clone(&input),
        Some(&layouts),
        |cx, provider| -> Result<(), Diagnostic> {
            assert!(
                provider.ready(cx).is_err(),
                "actual own clauses are not complete solver source facts"
            );
            for value in &plans {
                let QueryValue::ImplSourcePredicates(plan) = value.as_ref() else {
                    unreachable!()
                };
                if let Some(pending) = plan.pending() {
                    assert_eq!(
                        plan.encode(cx, configuration).unwrap_err(),
                        pending,
                        "unsupported actual source bound must not yield an empty environment"
                    );
                    continue;
                }
                let clauses = plan
                    .encode(cx, configuration)
                    .map_err(|p| fault(&format!("actual source clause encoding: {p:?}")))?;
                let repeated = plan.encode(cx, configuration).map_err(|p| {
                    fault(&format!("actual repeated source clause encoding: {p:?}"))
                })?;
                assert_eq!(clauses, repeated, "one context interns the actual clauses");
                let syntax = plan.requirements().head().syntax();
                let definition = syntax.canonical_owner().unwrap().identity();
                let owner = plan.bindings.definitions[&definition];
                let replacements =
                    model::Args::for_item(cx, owner, |parameter, _| match parameter {
                        model::GenericParameter::Type(_) => {
                            model::Arg::Type(cx.ty(ty::TyKind::Int(ty::IntTy::I32)))
                        }
                        model::GenericParameter::Region(_) => {
                            model::Arg::Region(cx.intern_region(ty::RegionKind::ReStatic))
                        }
                        model::GenericParameter::Const(_, _) => {
                            panic!("this source clause substitution remains unverified")
                        }
                    });
                for (clause, span) in &clauses {
                    assert_eq!(span.origin, syntax.source().content);
                    assert!(span.start < span.end);
                    let substituted = ty::EarlyBinder::bind(cx, *clause)
                        .instantiate(cx, replacements)
                        .skip_norm_wip();
                    assert!(
                        !substituted.has_param(),
                        "actual compiler EarlyBinder substitutes source owner namespaces"
                    );
                    match clause.kind().skip_binder() {
                        ty::ClauseKind::Trait(reference) => {
                            assert_eq!(reference.polarity, ty::ClausePolarity::Positive);
                            assert_eq!(
                                reference.trait_ref.def_id.world(),
                                catalog.seal.fingerprint()
                            );
                            result.traits += 1;
                        }
                        ty::ClauseKind::RegionOutlives(_) => result.regions += 1,
                        ty::ClauseKind::TypeOutlives(_) => result.types += 1,
                        _ => panic!("this source clause kind remains unverified"),
                    }
                    result.clauses += 1;
                }
                let ImplPredicateResult::Requirements(recipes) = plan.requirements().result()
                else {
                    unreachable!()
                };
                let mut positive = 0;
                for recipe in recipes {
                    match recipe {
                        SourcePredicateRecipe::Explicit {
                            result: PredicateResult::Relaxed { .. },
                            ..
                        } => result.relaxations += 1,
                        SourcePredicateRecipe::ImplicitSizedDefault(_)
                        | SourcePredicateRecipe::Explicit {
                            result: PredicateResult::Pending(_),
                            ..
                        } => panic!("pending source recipe was incorrectly classified ready"),
                        _ => positive += 1,
                    }
                }
                assert_eq!(
                    positive,
                    clauses.len(),
                    "all source requirements, no relaxation-as-goal"
                );
            }
            Ok(())
        },
    )??;
    assert_eq!(
        before,
        db.trace.events.len(),
        "typed encoding must not reenter source queries"
    );
    db.begin_expansion_epoch(key.epoch + 1)?;
    assert!(
        matches!(
            db.fetch(
                &SyntaxProvider,
                first.ok_or_else(|| fault("actual source impl absent"))?
            ),
            QueryPoll::Ready(Err(_))
        ),
        "old source clause key must not acquire the next expansion epoch"
    );
    Ok(result)
}
