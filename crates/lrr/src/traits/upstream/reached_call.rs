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
    relevant_candidates::{bucket_clause_traits, relevant_candidate_traits},
    root_input::{SourceCallRelationKind, SourceRootCallInput, SourceVariableFamily},
    root_relations::{PreparedRootRelations, RootRelationFulfillmentOutcome},
    root_response::export_root_response,
    root_variables::RootVariables,
    source_caller_clauses::PreparedSourceCallerClauses,
    source_adt_fields::CurrentAdtSourceFields,
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
    /// A source component had no typed hole, so rustc's fallback was applied
    /// to its open numeric variables. Always false for a call or constructor.
    pub(crate) closed: bool,
    current: Arc<ReachedCallCurrent>,
}
struct ReachedCallCurrent {
    bundle: Arc<SourceFactBundle>,
    caller: Arc<PreparedSourceCallerClauses>,
    application: Option<Arc<VerifiedCallerApplication>>,
    environment: Option<Arc<GenericEnv>>,
    caller_callable: Arc<SourceCallable>,
    subject: ReachedSubject,
    descriptor: super::root_input::CallerSourceContext,
}
/// What the reached root is for. Both share one inference owner, relation
/// application, fulfillment and response export; they differ only in the
/// actual declaration that authenticates the packet's formals.
#[derive(Clone)]
pub(crate) enum ReachedSubject {
    /// The selected function: its actual signature authenticates the formals.
    Callable(Arc<SourceCallable>),
    /// A record constructor of this nominal variant: the actual declaration's
    /// source field Types authenticate the formals.
    Adt(Arc<CurrentAdtSourceFields>, u32),
    /// A source component rooted at the caller body itself: the packet's rows
    /// are its relations. `(owner identity, row fingerprint)`.
    Component(Digest, Digest),
}
impl ReachedSubject {
    fn identity(&self) -> Digest {
        match self {
            Self::Callable(callable) => callable.identity(),
            Self::Adt(fields, _) => fields.plan().key().definition,
            Self::Component(identity, _) => *identity,
        }
    }
    /// The signature identity of a function, the declaration receipt of a nominal.
    fn fingerprint(&self) -> Digest {
        match self {
            Self::Callable(callable) => callable.signature().fingerprint(),
            Self::Adt(fields, _) => fields.fingerprint(),
            Self::Component(_, fingerprint) => *fingerprint,
        }
    }
    fn revalidate(&self, db: &mut Database) -> Result<(), Diagnostic> {
        match self {
            Self::Callable(callable) => callable.revalidate(db),
            Self::Adt(fields, _) => fields.require_current(db),
            // Its rows are derived from the caller's own source plan, which the
            // caller revalidation already covers.
            Self::Component(..) => Ok(()),
        }
    }
}
impl ReachedCallResponse {
    pub(crate) fn require_current(&self, db: &mut Database) -> Result<(), Diagnostic> {
        self.current.bundle.require_current(db)?;
        self.current.caller.require_current(db)?;
        self.current.caller_callable.revalidate(db)?;
        self.current.subject.revalidate(db)?;
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
#[path = "component_root.rs"]
mod component_root;
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
    poll_reached_root(
        db,
        inventory,
        packet,
        caller_environment,
        caller_callable,
        &ReachedSubject::Callable(Arc::clone(callee_callable)),
    )
}
/// The one reached source root for a selected function or a record
/// constructor's nominal declaration.
pub(crate) fn poll_reached_root(
    db: &mut Database,
    inventory: &SealKey,
    packet: &SourceRootCallInput<'_>,
    caller_environment: Option<&Arc<GenericEnv>>,
    caller_callable: &Arc<SourceCallable>,
    subject: &ReachedSubject,
) -> QueryPoll<ReachedCallOutcome> {
    if caller_callable.identity() != packet.caller.body_definition
        || subject.identity() != packet.callee
        || subject.fingerprint() != packet.signature
    {
        return QueryPoll::Ready(Err(fault(
            "reached source callable/signature identity differs",
        )));
    }
    if let Err(error) = caller_callable
        .revalidate(db)
        .and_then(|()| subject.revalidate(db))
    {
        return QueryPoll::Ready(Err(error));
    }
    match subject {
        ReachedSubject::Callable(callee_callable) => {
            if let Err(error) = authenticate_callable(packet, callee_callable) {
                return QueryPoll::Ready(Err(error));
            }
        }
        ReachedSubject::Component(..) => {
            // The rows are Equal relations of the caller's own source plan.
            if packet.component.is_none()
                || packet
                    .relations
                    .iter()
                    .any(|relation| relation.kind != SourceCallRelationKind::Equal)
            {
                return QueryPoll::Ready(Err(fault("component relation is not a source row")));
            }
        }
        ReachedSubject::Adt(fields, variant) => {
            // Every formal is one of this variant's actual declared field Types.
            let plan = fields.plan();
            let declared: Vec<_> = (0..plan.field_count(*variant))
                .filter_map(|field| plan.field_term(*variant, field))
                .collect();
            if packet.relations.iter().any(|relation| !declared.contains(&&relation.formal)) {
                return QueryPoll::Ready(Err(fault(
                    "reached relation formal is not an actual declared field",
                )));
            }
        }
    }
    if packet.component.is_some() && !matches!(subject, ReachedSubject::Component(..)) {
        return QueryPoll::Ready(Err(fault("component extras on a call or constructor root")));
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
    let mut input_types: Vec<_> = packet
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
    if let Some(component) = packet.component {
        for site in component.sites {
            input_types.extend(site.parameters.values().cloned());
        }
        for row in component.deferred {
            input_types.extend([row.first.clone(), row.second.clone(), row.result.clone()]);
        }
    }
    let extra_callees: BTreeSet<Digest> = packet
        .component
        .map(|component| component.sites.iter().map(|site| site.callee).collect())
        .unwrap_or_default();
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
    if let Some(component) = packet.component {
        // Each selected call site's clauses name the traits its generic
        // Type variables may be asked about.
        for site in component.sites {
            let site_key = super::source_declaration_clauses::SourceDeclarationClausesKey {
                inventory: inventory.clone(),
                definition: site.callee,
            };
            let site_clauses = ready!(
                super::source_declaration_clauses::PreparedSourceDeclarationClauses::prepare(
                    db,
                    &site_key
                )
            );
            if let Some(why) = site_clauses.pending() {
                return QueryPoll::Ready(Ok(held("site-source", why)));
            }
            let mut parameters = BTreeSet::new();
            parameters.extend(site.parameters.keys().copied());
            demanded.extend(all_clause_traits(&site_clauses, &parameters));
        }
    }
    let mut seeds = BTreeSet::new();
    loop {
        let mut promote = BTreeSet::new();
        // Finite immutable metadata dependency closure. This never evaluates a
        // clause, rejects an impl, normalizes an alias or retries a solver answer.
        // The root's own candidate traits come from pinned relevance (`execute`);
        // only seeds promoted from relevant impls keep complete-bucket propagation
        // because their child goals are not known before the solver runs.
        let mut closing = seeds.clone();
        let bundle = loop {
            let bundle = ready!(SourceFactBundle::prepare(
                db,
                Arc::clone(&catalog),
                SourceFactRequest {
                    caller: packet.caller.body_definition,
                    callee: packet.callee,
                    extra_callees: extra_callees.clone(),
                    traits: demanded.clone(),
                    input_types: input_types.clone(),
                }
            ));
            let mut next = demanded.clone();
            for trait_definition in closing.clone() {
                for source in bucket_clause_traits(&bundle, trait_definition) {
                    next.insert(source);
                    closing.insert(source);
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
            subject: subject.clone(),
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
                        .filter(|hold| hold.definition == source || hold.query.starts_with("clauses_of") || hold.query.starts_with("impl_super"))
                        .collect();
                    return Ok(held(
                        "callback-domain",
                        format!("{why}; source_producer_holds={reasons:?}"),
                    ));
                }
                provider.publish(materialized.facts)?;
                let outcome = match packet.component {
                    Some(component) => component_root::execute_component(
                        cx,
                        packet,
                        component,
                        &caller,
                        application.as_deref(),
                        &bundle,
                        &layouts,
                        configuration,
                        &current,
                        &mut promote,
                    ),
                    None => execute(
                        cx,
                        packet,
                        &caller,
                        application.as_deref(),
                        &bundle,
                        &layouts,
                        configuration,
                        &current,
                        &mut promote,
                    ),
                };
                // A relevant impl the traversal withheld may have changed any
                // outcome (a missing candidate reads as NoSolution): hold.
                let blocked = provider.take_blocked_impls();
                if !blocked.is_empty() {
                    return Ok(held("relevant-impl-unprepared", format!("{blocked:?}")));
                }
                outcome
            },
        );
        if !promote.is_empty() {
            demanded.extend(promote.iter().copied());
            seeds.extend(promote);
            continue;
        }
        return QueryPoll::Ready(output.and_then(|value| value));
    }
}

/// Authenticate each formal to its declared parameter/result or actual
/// explicit source generic key. The VM retains actual argument occurrences.
fn authenticate_callable(
    packet: &SourceRootCallInput<'_>,
    callee_callable: &Arc<SourceCallable>,
) -> Result<(), Diagnostic> {
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
            return Err(fault("reached relation formal is not the actual declaration"));
        }
    }
    if arguments != callee_callable.signature().parameters.len() {
        return Err(fault("reached argument relation coverage differs"));
    }
    Ok(())
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
            ) && !source_factory::builtin_requires_coroutine_self(role)
            {
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
        // Each impl's own queries (head, polarity, default, super outlives,
        // complete clauses) are read only when the pinned traversal reaches it
        // for a goal: `PreparedProvider::for_each_relevant_impl` withholds an
        // unprepared relevant impl and records it, and the root then holds
        // (`poll_reached_root`). Examining a whole bucket here would refuse
        // goals whose Self never reaches the unprepared members.
        for implementation in *known!(row.implementations, "complete relevant impl bucket") {
            if !facts.implementations.contains_key(implementation) {
                return Err((*source, "impl metadata missing".to_string()));
            }
            if !facts.definitions.contains_key(implementation) {
                return Err((*source, "impl declaration missing".to_string()));
            }
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
    promote: &mut BTreeSet<Digest>,
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
    // Selected clauses name their own parent traits; any not yet demanded are
    // requested and the whole attempt repeats over the larger immutable bundle.
    match demands.candidate_traits(&owner) {
        Ok(traits) => {
            promote.extend(traits.difference(&bundle.request().traits).copied());
            if !promote.is_empty() {
                return Ok(held("numeric-candidate-closure", &*promote));
            }
        }
        Err(_) => {
            return Ok(held(
                "numeric-candidate-closure",
                "actual source demand owner unavailable",
            ));
        }
    }
    // Pinned selection reads only the relevant impls of each selected goal, after
    // the solver's own alias normalization. Their child traits are the candidate
    // dependencies still to request; nothing else in a complete bucket is.
    match relevant_candidate_traits(cx, &admission, &owner, &demands, bundle, span) {
        Ok(traits) => {
            promote.extend(traits.difference(&bundle.request().traits).copied());
            if !promote.is_empty() {
                return Ok(held("relevant-candidate-closure", &*promote));
            }
        }
        Err(why) => return Ok(held("relevant-candidates", why)),
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
        closed: false,
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
/// Every trait a selected call site's clauses mention over the given
/// parameters (all of a site's generics: the component links them).
fn all_clause_traits(
    clauses: &super::source_declaration_clauses::PreparedSourceDeclarationClauses,
    parameters: &BTreeSet<crate::traits::term::ParamKey>,
) -> BTreeSet<Digest> {
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
