//! One source-bound ordinary reached call, with no escaping IR or delegate.
use super::{
    caller_application::{CallerApplicationPreparation, VerifiedCallerApplication},
    configuration::PreparedSolverConfiguration,
    infer::{
        InferenceContext,
        delegate::SolverContext,
        fulfillment::{FulfillmentContext, NextSolverError, PredicateObligation},
    },
    model::Span,
    prepared::{Cx, PreparedFacts},
    root_input::{SourceCallRelationKind, SourceRootCallInput, SourceVariableFamily},
    root_relations::{PreparedRootRelations, RootRelationFulfillmentOutcome},
    root_response::export_root_response,
    root_variables::RootVariables,
    source_caller_clauses::PreparedSourceCallerClauses,
    source_definition_headers::DefinitionHeaderKey,
    source_fact_bundle::{SourceFactBundle, SourceFactRequest},
    source_factory,
    source_generics::SourceGenericLayouts,
    source_runtime_demands::{EncodedRuntimeSourceClauses, SourceRuntimeDemands},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::InputKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
        seals::SealKey,
    },
    frontend::callables::SourceCallable,
    traits::{
        predicate::{PredicateResult, SourcePredicateRecipe},
        term::{GenericArg, InferVarKey, TraitRef, TyTerm},
    },
    vm::context::GenericEnv,
};
use rustc_type_ir::{self as ty, Upcast, inherent::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(crate) struct ReachedCallResponse {
    pub(crate) world: Digest,
    pub(crate) values: BTreeMap<InferVarKey, TyTerm>,
    pub(crate) unresolved: BTreeMap<InferVarKey, SourceVariableFamily>,
    current: Arc<ReachedCallCurrent>,
}
struct ReachedCallCurrent {
    bundle: Arc<SourceFactBundle>,
    caller: Arc<PreparedSourceCallerClauses>,
    application: Option<Arc<VerifiedCallerApplication>>,
    environment: Option<Arc<GenericEnv>>,
    caller_callable: Arc<SourceCallable>,
    callee_callable: Arc<SourceCallable>,
    descriptor: super::root_input::CallerSourceContext,
}
impl ReachedCallResponse {
    pub(crate) fn require_current(&self, db: &mut Database) -> Result<(), Diagnostic> {
        self.current.bundle.require_current(db)?;
        self.current.caller.require_current(db)?;
        self.current.caller_callable.revalidate(db)?;
        self.current.callee_callable.revalidate(db)?;
        if let Some(application) = &self.current.application {
            application
                .require_current(
                    db,
                    self.current
                        .environment
                        .as_ref()
                        .expect("retained actual environment"),
                    &self.current.caller_callable,
                    &self.current.descriptor,
                )
                .map_err(|why| fault(&format!("reached caller application changed: {why:?}")))?;
        }
        Ok(())
    }
}
impl std::fmt::Debug for ReachedCallResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReachedCallResponse")
            .field("world", &self.world)
            .field("values", &self.values)
            .field("unresolved", &self.unresolved)
            .finish_non_exhaustive()
    }
}
#[derive(Debug)]
pub(crate) struct ReachedCallHold {
    pub(crate) stage: &'static str,
    pub(crate) reason: String,
}
#[derive(Debug)]
pub(crate) enum ReachedFailureKind {
    Pending { stalled: Option<String> },
    NoSolution,
    Maybe { stalled: String },
    Overflow,
}
#[derive(Debug)]
pub(crate) struct ReachedGoalFailure {
    pub(crate) ordinal: Option<usize>,
    pub(crate) site: Span,
    pub(crate) predicate: String,
    pub(crate) kind: ReachedFailureKind,
}
#[derive(Debug)]
pub(crate) enum ReachedCallOutcome {
    Solved(ReachedCallResponse),
    Held(ReachedCallHold),
    Failed(Vec<ReachedGoalFailure>),
    Overflow(Vec<ReachedGoalFailure>),
}
fn held(stage: &'static str, reason: impl std::fmt::Debug) -> ReachedCallOutcome {
    ReachedCallOutcome::Held(ReachedCallHold {
        stage,
        reason: format!("{reason:?}"),
    })
}
fn fault(reason: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", reason.into())
}

/// Private construction is confined to the source-verified entry below. It
/// names this packet/configuration, never a caller-supplied Goal or ParamEnv.
pub(super) struct SourceRootAdmission {
    world: Digest,
    configuration: Digest,
    identity: Digest,
}
impl SourceRootAdmission {
    pub(super) fn world(&self) -> Digest {
        self.world
    }
    pub(super) fn configuration(&self) -> Digest {
        self.configuration
    }
    pub(super) fn identity(&self) -> Digest {
        self.identity
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

/// Called by the retained ordinary CallSeal attempt, before numeric fallback.
/// All DB work precedes `execute`; no query/input/coordinator is reachable from
/// the private root scope. Generic caller application support remains exactly
/// the existing source producer's supported domain.
pub(crate) fn poll_reached_call(
    db: &mut Database,
    inventory: &SealKey,
    packet: &SourceRootCallInput<'_>,
    caller_environment: Option<&Arc<GenericEnv>>,
    caller_callable: &Arc<SourceCallable>,
    callee_callable: &Arc<SourceCallable>,
) -> QueryPoll<ReachedCallOutcome> {
    if caller_callable.identity() != packet.caller.body_definition
        || callee_callable.identity() != packet.callee
        || callee_callable.signature().fingerprint() != packet.signature
    {
        return QueryPoll::Ready(Err(fault(
            "reached source callable/signature identity differs",
        )));
    }
    if let Err(error) = caller_callable
        .revalidate(db)
        .and_then(|()| callee_callable.revalidate(db))
    {
        return QueryPoll::Ready(Err(error));
    }
    // Authenticate each formal to its declared parameter/result or actual
    // explicit source generic key. The VM retains actual argument occurrences.
    let mut arguments = 0usize;
    let mut result = false;
    for relation in packet.relations {
        let valid = match relation.kind {
            SourceCallRelationKind::Equal => match &relation.formal {
                TyTerm::Param(key) => packet.parameters.get(key) == Some(&relation.actual),
                _ => false,
            },
            SourceCallRelationKind::Coerce => {
                let valid = callee_callable
                    .signature()
                    .parameters
                    .get(arguments)
                    .and_then(|parameter| parameter.symbolic.as_ref())
                    == Some(&relation.formal);
                arguments += 1;
                valid
            }
            SourceCallRelationKind::CoerceResult => {
                let valid = !result
                    && callee_callable.signature().result_symbolic.as_ref()
                        == Some(&relation.formal);
                result = true;
                valid
            }
        };
        if !valid {
            return QueryPoll::Ready(Err(fault(
                "reached relation formal is not the actual declaration",
            )));
        }
    }
    if arguments != callee_callable.signature().parameters.len() {
        return QueryPoll::Ready(Err(fault("reached argument relation coverage differs")));
    }
    let Some(catalog) = db.input(&InputKey::DiscoverySeal(inventory.clone())) else {
        return QueryPoll::NeedInput(InputRequest::Discover {
            key: inventory.clone(),
        });
    };
    let caller = Arc::new(ready!(PreparedSourceCallerClauses::prepare(
        db,
        &DefinitionHeaderKey {
            inventory: inventory.clone(),
            definition: packet.caller.body_definition
        }
    )));
    if let Some(why) = caller.pending() {
        return QueryPoll::Ready(Ok(held("caller-source", why)));
    }
    let application = match caller_environment {
        Some(environment) => match ready!(VerifiedCallerApplication::prepare(
            db,
            Arc::clone(&caller),
            Arc::clone(environment),
            Arc::clone(caller_callable),
            packet.caller
        )) {
            CallerApplicationPreparation::Verified(application) => Some(Arc::new(application)),
            CallerApplicationPreparation::Pending(why) => {
                return QueryPoll::Ready(Ok(held("caller-application", why)));
            }
        },
        None => None,
    };
    let Some(configuration) = caller.configuration() else {
        return QueryPoll::Ready(Ok(held(
            "configuration",
            "actual caller configuration unavailable",
        )));
    };
    let input_types: Vec<_> = packet
        .parameters
        .values()
        .cloned()
        .chain(packet.caller.substitutions.values().cloned())
        .chain(
            packet
                .relations
                .iter()
                .flat_map(|relation| [relation.formal.clone(), relation.actual.clone()]),
        )
        .collect();
    let clause_key = super::source_declaration_clauses::SourceDeclarationClausesKey {
        inventory: inventory.clone(),
        definition: packet.callee,
    };
    let clauses = ready!(
        super::source_declaration_clauses::PreparedSourceDeclarationClauses::prepare(
            db,
            &clause_key
        )
    );
    if let Some(why) = clauses.pending() {
        return QueryPoll::Ready(Ok(held("callee-source", why)));
    }
    let mut demanded = potential_numeric_traits(packet, &clauses);
    // Finite immutable metadata dependency closure. This never evaluates a
    // clause, rejects an impl, normalizes an alias or retries a solver answer.
    // All complete bucket members are retained, including pending producers.
    let bundle = loop {
        let bundle = ready!(SourceFactBundle::prepare(
            db,
            Arc::clone(&catalog),
            SourceFactRequest {
                caller: packet.caller.body_definition,
                callee: packet.callee,
                traits: demanded.clone(),
                input_types: input_types.clone(),
            }
        ));
        let mut next = demanded.clone();
        // Only candidate requirements can produce solver children. A trait
        // present merely as unrelated source metadata is not a root demand.
        for facts in bundle.implementations().values() {
            if let Ok(bindings) = facts.semantics().full_clause_bindings() {
                next.extend(bindings.definitions.keys().filter(|source| bundle.definitions().get(*source)
                    .is_some_and(|facts| facts.header().role() == super::source_definition_headers::DefinitionHeaderRole::Trait)).copied());
            }
        }
        next.extend(bundle.request().traits.iter().copied());
        if next == demanded {
            break bundle;
        }
        demanded = next;
    };
    if let Err(error) = bundle
        .require_current(db)
        .and_then(|()| caller.require_current(db))
    {
        return QueryPoll::Ready(Err(error));
    }
    if let Some(application) = &application {
        if let Err(why) = application.require_current(
            db,
            caller_environment.expect("verified environment"),
            caller_callable,
            packet.caller,
        ) {
            return QueryPoll::Ready(Ok(held("caller-currentness", why)));
        }
    }
    let layouts = match bundle.generic_layouts() {
        Ok(layouts) => layouts,
        Err(why) => return QueryPoll::Ready(Ok(held("generic-layouts", why))),
    };
    let bundle = Arc::new(bundle);
    let current = Arc::new(ReachedCallCurrent {
        bundle: Arc::clone(&bundle),
        caller: Arc::clone(&caller),
        application: application.clone(),
        environment: caller_environment.cloned(),
        caller_callable: Arc::clone(caller_callable),
        callee_callable: Arc::clone(callee_callable),
        descriptor: packet.caller.clone(),
    });
    let output = configuration.with_source_context(
        db,
        Arc::clone(&catalog),
        Some(&layouts),
        |cx, provider| {
            let materialized =
                match source_factory::materialize(cx, &bundle, &layouts, configuration) {
                    Ok(facts) => facts,
                    Err(error) => return Err(error),
                };
            if let Err((source, why)) = require_callback_domains(&materialized.facts, &bundle) {
                let reasons: Vec<_> = materialized
                    .unprepared
                    .iter()
                    .filter(|hold| hold.definition == source)
                    .collect();
                return Ok(held(
                    "callback-domain",
                    format!("{why}; source_producer_holds={reasons:?}"),
                ));
            }
            provider.publish(materialized.facts)?;
            execute(
                cx,
                packet,
                &caller,
                application.as_deref(),
                &bundle,
                &layouts,
                configuration,
                &current,
            )
        },
    );
    QueryPoll::Ready(output.and_then(|value| value))
}

/// Check actual compiler-query slots for the bounded normal source profile.
/// Unrelated fn/opaque/coroutine/const-evaluation domains are not censused.
fn require_callback_domains(
    facts: &PreparedFacts<'_>,
    bundle: &SourceFactBundle,
) -> Result<(), (Digest, String)> {
    for source in &bundle.request().traits {
        let id = bundle
            .definition_id(*source)
            .ok_or_else(|| (*source, "candidate trait identity missing".to_string()))?;
        let row = facts
            .traits
            .get(&id)
            .ok_or_else(|| (*source, "candidate trait metadata missing".to_string()))?;
        let definition = &facts.definitions[&id];
        let actual = &bundle.definitions()[source];
        let source_owner = format!(
            "{} ({source:?}) at {:?}",
            actual.header().name().unwrap_or("<anonymous declaration>"),
            actual.header().span()
        );
        let language = bundle.definitions()[source]
            .language()
            .classification()
            .map_err(|why| {
                (
                    *source,
                    format!("{source_owner}: language classification {why:?}"),
                )
            })?;
        if let Some(role) = source_factory::trait_role(language.map(|(name, _)| name)) {
            use rustc_type_ir::lang_items::SolverTraitLangItem;
            if !matches!(
                role,
                SolverTraitLangItem::DiscriminantKind
                    | SolverTraitLangItem::Sized
                    | SolverTraitLangItem::MetaSized
            ) {
                return Err((
                    *source,
                    format!(
                        "{source_owner}: builtin callback domain {role:?} is not prepared for this source scope"
                    ),
                ));
            }
        }

        macro_rules! known {
            ($value:expr, $name:literal) => {
                $value
                    .checked($name)
                    .map_err(|why| (*source, format!("{source_owner}: {why:?}")))?
            };
        }
        known!(row.implementations, "complete relevant impl bucket");
        known!(row.coinductive, "trait coinduction");
        if *known!(row.automatic, "auto trait") || *known!(row.alias, "trait alias") {
            return Err((
                *source,
                format!("{source_owner}: automatic/alias callback closure not produced"),
            ));
        }
        known!(
            definition.predicates.explicit_super,
            "explicit super clauses"
        );
        known!(
            definition.predicates.explicit_implied,
            "explicit implied clauses"
        );
        for implementation in *known!(row.implementations, "complete relevant impl bucket") {
            let metadata = facts
                .implementations
                .get(implementation)
                .ok_or_else(|| (*source, "impl metadata missing".to_string()))?;
            let declaration = facts
                .definitions
                .get(implementation)
                .ok_or_else(|| (*source, "impl declaration missing".to_string()))?;
            known!(metadata.trait_ref, "impl trait head");
            known!(metadata.polarity, "impl polarity");
            known!(metadata.default_impl, "default impl");
            known!(metadata.super_outlives, "impl super outlives");
            known!(declaration.predicates.all, "complete impl clauses");
        }
    }
    Ok(())
}

fn execute(
    cx: Cx<'_>,
    packet: &SourceRootCallInput<'_>,
    caller: &PreparedSourceCallerClauses,
    application: Option<&VerifiedCallerApplication>,
    bundle: &SourceFactBundle,
    layouts: &SourceGenericLayouts,
    configuration: &PreparedSolverConfiguration,
    current: &Arc<ReachedCallCurrent>,
) -> Result<ReachedCallOutcome, Diagnostic> {
    let Some(source) = bundle.own_clauses(packet.callee) else {
        return Ok(held("callee-clauses", "actual Q80 unavailable"));
    };
    let encoded = match EncodedRuntimeSourceClauses::prepare(cx, configuration, source) {
        Ok(value) => value,
        Err(why) => return Ok(held("callee-clauses", why)),
    };
    let owner = SolverContext::from_inference_context(InferenceContext::new(
        cx,
        ty::TypingMode::PostAnalysis,
    ));
    let span = packet
        .relations
        .first()
        .map(|relation| relation.site)
        .unwrap_or_else(|| {
            caller
                .header()
                .encode_header(cx)
                .expect("published actual header")
                .span
        });
    let variables = RootVariables::new(
        &owner,
        packet.variables.iter().map(|(key, kind)| (*key, *kind)),
        span,
    )?;
    let prepared = match PreparedRootRelations::prepare(
        &owner,
        &variables,
        layouts,
        configuration,
        packet,
        caller,
        application,
    ) {
        Ok(value) => value,
        Err(why) => return Ok(held("call-relations", why)),
    };
    let mut identity = StableEncoder::new("lrr/v1/reached-source-root-session");
    identity.digest(cx.services().source_world());
    identity.digest(configuration.fingerprint());
    identity.digest(packet.callee);
    identity.digest(packet.signature);
    identity.digest(packet.caller.body_definition);
    for relation in packet.relations {
        identity.record(&relation.formal);
        identity.record(&relation.actual);
        identity.digest(relation.site.origin);
        identity.u32(relation.site.start);
        identity.u32(relation.site.end);
    }
    let admission = SourceRootAdmission {
        world: cx.services().source_world(),
        configuration: configuration.fingerprint(),
        identity: identity.finish(),
    };
    let _scope = cx.services().enter_source_root(&admission)?;
    owner.admit_source_root(&admission);
    let applied = match prepared.apply() {
        Ok(value) => value,
        Err(error) => {
            return Ok(ReachedCallOutcome::Failed(vec![ReachedGoalFailure {
                ordinal: Some(error.cause.relation),
                site: error.cause.site,
                predicate: format!("{:?} ~ {:?}: {:?}", error.formal, error.actual, error.error),
                kind: ReachedFailureKind::NoSolution,
            }]));
        }
    };
    let parts = applied
        .into_parts(&owner)
        .map_err(|why| fault(&format!("original relation transfer failed: {why:?}")))?;
    let work = parts
        .into_fulfillment_work(&owner, &variables)
        .map_err(|(_, why)| fault(&format!("original relation owner failed: {why:?}")))?;
    let settled = match work.fulfill(&admission) {
        RootRelationFulfillmentOutcome::Settled(value) => value,
        RootRelationFulfillmentOutcome::Unresolved { errors, .. } => {
            return Ok(ReachedCallOutcome::Failed(
                errors
                    .iter()
                    .map(|error| failure(error, |cause| Some(cause.relation)))
                    .collect(),
            ));
        }
        RootRelationFulfillmentOutcome::Overflow { overflow, work } => {
            let mut errors: Vec<_> = overflow
                .errors
                .iter()
                .map(|error| failure(error, |cause| Some(cause.relation)))
                .collect();
            if let Some(context) = work
                .retained_fulfillment(&owner)
                .expect("original overflow owner")
            {
                errors.extend(pending_failures(context, |cause| Some(cause.relation)));
            }
            errors.push(ReachedGoalFailure {
                ordinal: Some(overflow.obligation.cause.relation),
                site: overflow.obligation.span,
                predicate: format!("{:?}", overflow.obligation.predicate),
                kind: ReachedFailureKind::Overflow,
            });
            return Ok(ReachedCallOutcome::Overflow(errors));
        }
    };
    let demands =
        match SourceRuntimeDemands::after_fulfillment(&owner, &variables, &settled, &encoded) {
            Ok(value) => value,
            Err(why) => return Ok(held("numeric-source-incidence", why)),
        };
    if let Ok(traits) = demands.candidate_traits(&owner) {
        if !traits.is_subset(&bundle.request().traits) {
            return Ok(held("numeric-candidate-closure", traits));
        }
    } else {
        return Ok(held(
            "numeric-candidate-closure",
            "actual source demand owner unavailable",
        ));
    }
    let mut fulfillment = FulfillmentContext::new(&owner);
    for clause in demands.clauses() {
        fulfillment.register_predicate_obligation(PredicateObligation {
            cause: clause.ordinal(),
            span: clause.span(),
            param_env: demands.param_env(),
            predicate: clause.clause().upcast(cx),
            recursion_depth: 0,
        });
    }
    let mut errors = match fulfillment.try_evaluate_obligations() {
        Ok(errors) => errors,
        Err(overflow) => {
            return Ok(ReachedCallOutcome::Overflow(
                overflow
                    .errors
                    .iter()
                    .map(|error| failure(error, |ordinal| Some(*ordinal)))
                    .chain(pending_failures(&fulfillment, |ordinal| Some(*ordinal)))
                    .chain(std::iter::once(ReachedGoalFailure {
                        ordinal: Some(overflow.obligation.cause),
                        site: overflow.obligation.span,
                        predicate: format!("{:?}", overflow.obligation.predicate),
                        kind: ReachedFailureKind::Overflow,
                    }))
                    .collect(),
            ));
        }
    };
    match fulfillment.collect_remaining_errors() {
        Ok(residual) => errors.extend(residual),
        Err(overflow) => {
            return Ok(ReachedCallOutcome::Overflow(
                errors
                    .iter()
                    .chain(overflow.errors.iter())
                    .map(|error| failure(error, |ordinal| Some(*ordinal)))
                    .chain(pending_failures(&fulfillment, |ordinal| Some(*ordinal)))
                    .chain(std::iter::once(ReachedGoalFailure {
                        ordinal: Some(overflow.obligation.cause),
                        site: overflow.obligation.span,
                        predicate: format!("{:?}", overflow.obligation.predicate),
                        kind: ReachedFailureKind::Overflow,
                    }))
                    .collect(),
            ));
        }
    }
    if !errors.is_empty() {
        return Ok(ReachedCallOutcome::Failed(
            errors
                .iter()
                .map(|error| failure(error, |ordinal| Some(*ordinal)))
                .collect(),
        ));
    }
    if fulfillment.has_pending_obligations() {
        return Ok(held("fulfillment", "actual undrained obligations"));
    }
    if let Some(constraints) = owner.source_root_external_constraints() {
        return Ok(held("external-constraints", constraints));
    }
    let response = match export_root_response(&owner, &variables, layouts) {
        Ok(response) => response,
        Err(why) => return Ok(held("response-export", why)),
    };
    let (world, values, unresolved) = response.into_parts();
    Ok(ReachedCallOutcome::Solved(ReachedCallResponse {
        world,
        values,
        unresolved,
        current: Arc::clone(current),
    }))
}
fn pending_failures<C: std::fmt::Debug + Clone>(
    fulfillment: &FulfillmentContext<'_, '_, C>,
    ordinal: impl Fn(&C) -> Option<usize>,
) -> Vec<ReachedGoalFailure> {
    fulfillment
        .pending_with_stall()
        .iter()
        .map(|(obligation, stalled)| ReachedGoalFailure {
            ordinal: ordinal(&obligation.cause),
            site: obligation.span,
            predicate: format!(
                "{:?} under {:?}; cause={:?}; depth={}",
                obligation.predicate,
                obligation.param_env,
                obligation.cause,
                obligation.recursion_depth
            ),
            kind: ReachedFailureKind::Pending {
                stalled: stalled.as_ref().map(|value| format!("{value:?}")),
            },
        })
        .collect()
}
fn failure<C: std::fmt::Debug>(
    error: &NextSolverError<'_, C>,
    ordinal: impl Fn(&C) -> Option<usize>,
) -> ReachedGoalFailure {
    match error {
        NextSolverError::TrueError(obligation) => ReachedGoalFailure {
            ordinal: ordinal(&obligation.cause),
            site: obligation.span,
            predicate: format!("{:?}", obligation.predicate),
            kind: ReachedFailureKind::NoSolution,
        },
        NextSolverError::Ambiguity {
            obligation,
            stalled_on,
        } => ReachedGoalFailure {
            ordinal: ordinal(&obligation.cause),
            site: obligation.span,
            predicate: format!("{:?}", obligation.predicate),
            kind: ReachedFailureKind::Maybe {
                stalled: format!("{stalled_on:?}"),
            },
        },
    }
}

/// Conservative source dependency incidence, never a relation or answer. A
/// numeric occurrence in an actual reached argument/result connects only the
/// formal source parameter occurrences of that same relation. Real clause
/// selection remains the post-fulfillment TypeIR visitor above.
fn potential_numeric_traits(
    packet: &SourceRootCallInput<'_>,
    clauses: &super::source_declaration_clauses::PreparedSourceDeclarationClauses,
) -> BTreeSet<Digest> {
    let mut parameters = BTreeSet::new();
    for relation in packet.relations {
        let mut numeric = false;
        visit_source_type(&relation.actual, &mut |ty| {
            if let TyTerm::Infer(key) = ty {
                numeric |= matches!(
                    packet.variables.get(key),
                    Some(SourceVariableFamily::Integer | SourceVariableFamily::Float)
                );
            }
        });
        if numeric {
            visit_source_type(&relation.formal, &mut |ty| {
                if let TyTerm::Param(key) = ty {
                    parameters.insert(*key);
                }
            });
        }
    }
    let mut traits = BTreeSet::new();
    for recipe in clauses.source().recipes().into_iter().flatten() {
        let reference = match recipe {
            SourcePredicateRecipe::ResolvedSizedDefault(reference)
            | SourcePredicateRecipe::Explicit {
                result: PredicateResult::Implemented(reference),
                ..
            } => reference,
            _ => continue,
        };
        let mut connected = false;
        visit_source_reference(reference, &mut |ty| {
            if let TyTerm::Param(key) = ty {
                connected |= parameters.contains(key);
            }
        });
        if connected {
            traits.insert(reference.trait_key);
            visit_source_reference(reference, &mut |ty| {
                if let TyTerm::Projection(projection) = ty {
                    traits.insert(projection.trait_ref.trait_key);
                }
            });
        }
    }
    traits
}
fn visit_source_reference(reference: &TraitRef, visit: &mut impl FnMut(&TyTerm)) {
    visit_source_type(&reference.self_ty, visit);
    for argument in &reference.args {
        if let GenericArg::Type(ty) = argument {
            visit_source_type(ty, visit);
        }
    }
}
fn visit_source_type(value: &TyTerm, visit: &mut impl FnMut(&TyTerm)) {
    visit(value);
    match value {
        TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => {
            for arg in args {
                if let GenericArg::Type(ty) = arg {
                    visit_source_type(ty, visit);
                }
            }
        }
        TyTerm::Projection(projection) => {
            visit_source_reference(&projection.trait_ref, visit);
            for arg in &projection.item_args {
                if let GenericArg::Type(ty) = arg {
                    visit_source_type(ty, visit);
                }
            }
        }
        TyTerm::Function(signature) => {
            for ty in &signature.value.parameters {
                visit_source_type(ty, visit);
            }
            visit_source_type(&signature.value.result, visit);
        }
        TyTerm::Tuple(types) => {
            for ty in types {
                visit_source_type(ty, visit);
            }
        }
        TyTerm::Array(ty, _) | TyTerm::Slice(ty) | TyTerm::Ref(_, _, ty) => {
            visit_source_type(ty, visit)
        }
        _ => {}
    }
}
