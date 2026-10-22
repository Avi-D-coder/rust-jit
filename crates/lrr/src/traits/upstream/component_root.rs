//! The pinned-owner execution of one source component (`ReachedSubject::
//! Component`): its rows are Equal relations over source-owned variables, its
//! structural rows fire as the owner resolves their operand Types, the selected
//! call sites' numeric-connected where-clauses are registered against the
//! site-instantiated Types, and rustc's integer/float fallback is applied by
//! relating still-open literal variables to i32/f64 only when the component has
//! no typed hole (`ComponentInput::closed`). Nothing here selects a candidate or
//! solves by hand: every Type fact comes from the one inference owner.
use super::*;
use super::super::root_input::{ComponentInput, DeferredRelation, DeferredRelationKind, SourceCallRelation};
use crate::traits::term::{RegionTerm, TyTerm};
use crate::ir::op::PrimitiveType;
use ty::Interner as _;
use super::super::model::{AllocateList as _, Arg, Clause};

fn numeric_class(kind: ty::TyKind<Cx<'_>>, bit_or_char: bool) -> bool {
    use ty::TyKind::*;
    match kind {
        Int(_) | Uint(_) | Float(_) | Infer(ty::InferTy::IntVar(_) | ty::InferTy::FloatVar(_)) => true,
        Bool | Char => bit_or_char,
        _ => false,
    }
}

/// A batch's application: settled, or pending on ambiguous goals alone.
enum Batch<'owner, 'cx> {
    Settled(super::super::root_relations::SettledRootApplication<'owner, 'cx>),
    Pending(super::super::root_relations::RootRelationFulfillmentWork<'owner, 'cx>),
}
impl<'owner, 'cx> Batch<'owner, 'cx> {
    fn application(
        &self,
        owner: &SolverContext<'cx>,
        variables: &RootVariables<'_, 'cx>,
    ) -> Result<&super::super::root_relations::SourceRootApplication<'owner, 'cx>, super::super::source_terms::SourceTypePending> {
        match self {
            Self::Settled(settled) => settled.source_application(owner, variables),
            Self::Pending(work) => work.source_application(owner),
        }
    }
}

enum Fired {
    Relations(Vec<SourceCallRelation>),
    /// A comparison that is not builtin: the obligation `lhs: PartialEq<rhs>`.
    Obligation,
    Waiting,
    Hole,
}

/// Resolve `term` (a component variable or a closed primitive) on the owner.
fn resolved<'cx>(
    variables: &RootVariables<'_, 'cx>,
    term: &TyTerm,
) -> Option<RootTyOf<'cx>> {
    match term {
        TyTerm::Infer(key) => variables.resolved(*key).ok(),
        _ => None,
    }
}
type RootTyOf<'cx> = super::super::model::Ty<'cx, super::super::prepared::PreparedProvider<'cx>>;

fn fire<'cx>(
    variables: &RootVariables<'_, 'cx>,
    row: &DeferredRelation,
) -> Fired {
    let primitive_class = |term: &TyTerm| -> Option<bool> {
        match term {
            TyTerm::Primitive(p) => Some(matches!(p, PrimitiveType::Bool | PrimitiveType::Char)),
            _ => None,
        }
    };
    match row.kind {
        DeferredRelationKind::Deref => {
            let Some(operand) = resolved(variables, &row.first) else {
                return Fired::Hole;
            };
            match operand.kind() {
                ty::TyKind::Ref(_, _, mutability) => Fired::Relations(vec![SourceCallRelation {
                    formal: row.first.clone(),
                    actual: TyTerm::Ref(
                        RegionTerm::Static,
                        mutability == ty::Mutability::Mut,
                        Box::new(row.result.clone()),
                    ),
                    kind: SourceCallRelationKind::Equal,
                    site: row.site,
                }]),
                ty::TyKind::Infer(ty::InferTy::TyVar(_)) => Fired::Waiting,
                // `*x` of another type is its `Deref::Target` (a `Box` is a
                // nominal with the ordinary impl): the owner normalizes it.
                ty::TyKind::Adt(..) => match &row.projection {
                    Some(projection) => Fired::Relations(vec![SourceCallRelation {
                        formal: row.result.clone(),
                        actual: projection.clone(),
                        kind: SourceCallRelationKind::Equal,
                        site: row.site,
                    }]),
                    None => Fired::Hole,
                },
                _ => Fired::Hole,
            }
        }
        DeferredRelationKind::Shift => {
            // Builtin shifts take two integers independently; the result is the
            // left operand's Type.
            let mut known = [false, false];
            for (slot, term) in [&row.first, &row.second].into_iter().enumerate() {
                known[slot] = match resolved(variables, term) {
                    Some(resolved) => match resolved.kind() {
                        ty::TyKind::Infer(ty::InferTy::TyVar(_)) => return Fired::Waiting,
                        ty::TyKind::Int(_) | ty::TyKind::Uint(_) | ty::TyKind::Infer(ty::InferTy::IntVar(_)) => true,
                        _ => false,
                    },
                    None => matches!(term, TyTerm::Primitive(p) if !matches!(p, PrimitiveType::Bool | PrimitiveType::Char | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Unit)),
                };
            }
            if known == [true, true] {
                Fired::Relations(vec![SourceCallRelation {
                    formal: row.first.clone(),
                    actual: row.result.clone(),
                    kind: SourceCallRelationKind::Equal,
                    site: row.site,
                }])
            } else {
                Fired::Hole
            }
        }
        DeferredRelationKind::Arithmetic | DeferredRelationKind::Comparison | DeferredRelationKind::Compound => {
            let comparison = row.kind == DeferredRelationKind::Comparison;
            let compound = row.kind == DeferredRelationKind::Compound;
            let mut classes = [None, None];
            for (slot, term) in [&row.first, &row.second].into_iter().enumerate() {
                classes[slot] = match resolved(variables, term) {
                    Some(resolved) => match resolved.kind() {
                        ty::TyKind::Infer(ty::InferTy::TyVar(_)) => None,
                        kind => Some(numeric_class(kind, comparison)),
                    },
                    None => primitive_class(term).map(|bit_or_char| comparison || !bit_or_char),
                };
            }
            match classes {
                [Some(true), Some(true)] => {
                    // Builtin operator: operands share a Type (and the result
                    // is that Type for arithmetic).
                    let mut relations = vec![SourceCallRelation {
                        formal: row.first.clone(),
                        actual: row.second.clone(),
                        kind: SourceCallRelationKind::Equal,
                        site: row.site,
                    }];
                    if !comparison && !compound {
                        relations.push(SourceCallRelation {
                            formal: row.first.clone(),
                            actual: row.result.clone(),
                            kind: SourceCallRelationKind::Equal,
                            site: row.site,
                        });
                    }
                    Fired::Relations(relations)
                }
                [Some(false), _] | [_, Some(false)] if comparison => Fired::Obligation,
                [Some(false), _] | [_, Some(false)] => Fired::Hole,
                _ => Fired::Waiting,
            }
        }
    }
}

pub(super) fn execute_component(
    cx: Cx<'_>,
    packet: &SourceRootCallInput<'_>,
    component: &ComponentInput<'_>,
    caller: &PreparedSourceCallerClauses,
    application: Option<&VerifiedCallerApplication>,
    bundle: &SourceFactBundle,
    layouts: &SourceGenericLayouts,
    configuration: &PreparedSolverConfiguration,
    current: &Arc<ReachedCallCurrent>,
    promote: &mut BTreeSet<Digest>,
) -> Result<ReachedCallOutcome, Diagnostic> {
    let owner = SolverContext::from_inference_context(InferenceContext::new(
        cx,
        ty::TypingMode::PostAnalysis,
    ));
    let span = packet.relations.first().map(|relation| relation.site).unwrap_or_else(|| {
        caller
            .header()
            .encode_header(cx)
            .expect("published actual header")
            .span
    });
    let variables = RootVariables::with_consts(
        &owner,
        packet.variables.iter().map(|(key, kind)| (*key, *kind)),
        component.const_variables.iter().copied(),
        span,
    )?;
    let mut identity = StableEncoder::new("lrr/v1/component-root-session");
    identity.digest(cx.services().source_world());
    identity.digest(configuration.fingerprint());
    identity.digest(packet.callee);
    identity.digest(packet.signature);
    identity.digest(packet.caller.body_definition);
    let admission = SourceRootAdmission {
        world: cx.services().source_world(),
        configuration: configuration.fingerprint(),
        identity: identity.finish(),
    };
    let _scope = cx.services().enter_source_root(&admission)?;
    owner.admit_source_root(&admission);

    let mut ambiguous = false;
    // One batch of Equal relations through the pinned owner. `Err` is a
    // finished outcome (conflict, unresolved goal, overflow, or a hold).
    macro_rules! batch {
        ($relations:expr) => {{
            let relations: &[SourceCallRelation] = $relations;
            let input = SourceRootCallInput {
                callee: packet.callee,
                signature: packet.signature,
                parameters: packet.parameters,
                variables: packet.variables,
                relations,
                caller: packet.caller,
                component: packet.component,
            };
            let prepared = match PreparedRootRelations::prepare(
                &owner, &variables, layouts, configuration, &input, caller, application,
            ) {
                Ok(value) => value,
                Err(why) => return Ok(held("component-relations", why)),
            };
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
                .map_err(|why| fault(&format!("component relation transfer failed: {why:?}")))?;
            let work = parts
                .into_fulfillment_work(&owner, &variables)
                .map_err(|(_, why)| fault(&format!("component relation owner failed: {why:?}")))?;
            match work.fulfill(&admission) {
                RootRelationFulfillmentOutcome::Settled(settled) => Batch::Settled(settled),
                RootRelationFulfillmentOutcome::Unresolved { errors, work } => {
                    if errors
                        .iter()
                        .any(|error| matches!(error, NextSolverError::TrueError(_)))
                    {
                        return Ok(ReachedCallOutcome::Failed(
                            errors
                                .iter()
                                .map(|error| failure(error, |cause| Some(cause.relation)))
                                .collect(),
                        ));
                    }
                    // Only ambiguity remains (an alias over a Type the rows
                    // have not resolved): a later round cannot re-open this
                    // goal, so the component is no longer provably closed.
                    ambiguous = true;
                    Batch::Pending(work)
                }
                RootRelationFulfillmentOutcome::Overflow { overflow, .. } => {
                    return Ok(ReachedCallOutcome::Overflow(
                        overflow
                            .errors
                            .iter()
                            .map(|error| failure(error, |cause| Some(cause.relation)))
                            .collect(),
                    ));
                }
            }
        }};
    }

    let mut settled = batch!(packet.relations);
    // Structural rows fire as their operand Types resolve.
    let mut waiting: Vec<&DeferredRelation> = component.deferred.iter().collect();
    let mut closed = component.closed;
    let mut obligations: Vec<&DeferredRelation> = Vec::new();
    loop {
        let mut fired = Vec::new();
        let mut next = Vec::new();
        for row in waiting {
            match fire(&variables, row) {
                Fired::Relations(relations) => fired.extend(relations),
                Fired::Obligation => obligations.push(row),
                Fired::Waiting => next.push(row),
                Fired::Hole => {
                    if std::env::var_os("LRR_SCR_DEBUG").is_some() {
                        eprintln!(
                            "SCR-DEBUG hole {:?} first={:?}/{:?} second={:?}/{:?}",
                            row.kind,
                            row.first,
                            resolved(&variables, &row.first).map(|t| format!("{:?}", t.kind())),
                            row.second,
                            resolved(&variables, &row.second).map(|t| format!("{:?}", t.kind())),
                        );
                    }
                    closed = false
                }
            }
        }
        waiting = next;
        if fired.is_empty() {
            break;
        }
        settled = batch!(&fired);
    }
    if (!waiting.is_empty() || ambiguous) && std::env::var_os("LRR_SCR_DEBUG").is_some() {
        eprintln!("SCR-DEBUG not closed: waiting={} ambiguous={ambiguous}", waiting.len());
        for row in &waiting {
            eprintln!(
                "SCR-DEBUG waiting {:?} first={:?}/{:?} second={:?}/{:?}",
                row.kind,
                row.first,
                resolved(&variables, &row.first).map(|t| format!("{:?}", t.kind())),
                row.second,
                resolved(&variables, &row.second).map(|t| format!("{:?}", t.kind())),
            );
        }
    }
    if !waiting.is_empty() || ambiguous {
        closed = false;
    }

    // The selected call sites' numeric-connected where-clauses.
    let base = settled
        .application(&owner, &variables)
        .map_err(|why| fault(&format!("component application unavailable: {why:?}")))?;
    let mut encodings = Vec::new();
    let mut applications = Vec::new();
    for site in component.sites {
        let Some(source) = bundle.own_clauses(site.callee) else {
            return Ok(held("site-clauses", "actual Q80 unavailable for a selected call site"));
        };
        let encoded = match EncodedRuntimeSourceClauses::prepare(cx, configuration, source) {
            Ok(value) => value,
            Err(why) => return Ok(held("site-clauses", why)),
        };
        let site_application = match base.site(layouts, configuration, site.callee, site.signature, &site.parameters, &site.constants) {
            Ok(value) => value,
            Err(why) => return Ok(held("site-application", format!("{why:?}"))),
        };
        encodings.push(encoded);
        applications.push(site_application);
    }
    let mut demands = Vec::new();
    for (encoded, application) in encodings.iter().zip(&applications) {
        match SourceRuntimeDemands::after_site(&owner, &variables, application, encoded) {
            Ok(value) => demands.push(value),
            Err(why) => return Ok(held("numeric-source-incidence", why)),
        }
    }
    for demand in &demands {
        match demand.candidate_traits(&owner) {
            Ok(traits) => {
                promote.extend(traits.difference(&bundle.request().traits).copied());
            }
            Err(_) => return Ok(held("numeric-candidate-closure", "actual source demand owner unavailable")),
        }
    }
    if !promote.is_empty() {
        return Ok(held("numeric-candidate-closure", &*promote));
    }
    for demand in &demands {
        match relevant_candidate_traits(cx, &admission, &owner, demand, bundle, span) {
            Ok(traits) => promote.extend(traits.difference(&bundle.request().traits).copied()),
            Err(why) => return Ok(held("relevant-candidates", why)),
        }
    }
    if !promote.is_empty() {
        return Ok(held("relevant-candidate-closure", &*promote));
    }
    let mut fulfillment = FulfillmentContext::new(&owner);
    // Comparisons that are not builtin scalar operations: `lhs: PartialEq<rhs>`.
    if !obligations.is_empty() {
        let Some(trait_key) = component.eq_trait else {
            return Ok(held("operator-obligation", "core::cmp::PartialEq is not declared"));
        };
        let Some(&definition) = layouts.type_bindings().definitions.get(&trait_key) else {
            return Ok(held("operator-obligation", "PartialEq identity is absent from the prepared world"));
        };
        for (index, row) in obligations.iter().enumerate() {
            let encode = |term: &TyTerm| {
                super::super::source_terms::encode_root_type(&owner, &variables, configuration, layouts.type_bindings(), term)
            };
            let (Ok(lhs), Ok(rhs)) = (encode(&row.first), encode(&row.second)) else {
                return Ok(held("operator-obligation", "operand Type is not encodable"));
            };
            let trait_ref = ty::TraitRef::new_from_args(
                cx,
                definition,
                cx.list(vec![Arg::Type(lhs), Arg::Type(rhs)]),
            );
            let clause = Clause::new(
                cx,
                ty::Binder::dummy(ty::ClauseKind::Trait(ty::TraitClause {
                    trait_ref,
                    polarity: ty::ClausePolarity::Positive,
                })),
            );
            fulfillment.register_predicate_obligation(PredicateObligation {
                cause: usize::MAX - index,
                span: row.site,
                param_env: base.param_env(),
                predicate: clause.upcast(cx),
                recursion_depth: 0,
            });
        }
    }
    for demand in &demands {
        for clause in demand.clauses() {
            // An implied `Sized`/`MetaSized` bound cannot constrain a numeric
            // variable, and its ADT sizedness query is not prepared: the
            // adapter would fault on any nominal self type.
            if let ty::ClauseKind::Trait(bound) = clause.clause().kind().skip_binder()
                && cx.is_sizedness_trait(bound.trait_ref.def_id)
            {
                continue;
            }
            fulfillment.register_predicate_obligation(PredicateObligation {
                cause: clause.ordinal(),
                span: clause.span(),
                param_env: demand.param_env(),
                predicate: clause.clause().upcast(cx),
                recursion_depth: 0,
            });
        }
    }
    let overflow_outcome = |overflow: super::super::infer::fulfillment::FulfillmentOverflow<'_, usize>| -> ReachedCallOutcome {
        ReachedCallOutcome::Overflow(
            overflow
                .errors
                .iter()
                .map(|error| failure(error, |ordinal| Some(*ordinal)))
                .chain(std::iter::once(ReachedGoalFailure {
                    ordinal: Some(overflow.obligation.cause),
                    site: overflow.obligation.span,
                    predicate: format!("{:?}", overflow.obligation.predicate),
                    kind: ReachedFailureKind::Overflow,
                }))
                .collect(),
        )
    };
    let mut errors = match fulfillment.try_evaluate_obligations() {
        Ok(errors) => errors,
        Err(overflow) => return Ok(overflow_outcome(overflow)),
    };

    // rustc's fallback phase (fallback.rs `type_inference_fallback`): only a
    // component with no typed hole has proven every use of its variables.
    if closed {
        let mut fallback = Vec::new();
        for key in component.literals {
            let Ok(value) = variables.resolved(*key) else { continue };
            let primitive = match value.kind() {
                ty::TyKind::Infer(ty::InferTy::IntVar(_)) => PrimitiveType::I32,
                ty::TyKind::Infer(ty::InferTy::FloatVar(_)) => PrimitiveType::F64,
                _ => continue,
            };
            fallback.push(SourceCallRelation {
                formal: TyTerm::Infer(*key),
                actual: TyTerm::Primitive(primitive),
                kind: SourceCallRelationKind::Equal,
                site: span,
            });
        }
        if !fallback.is_empty() {
            let _ = batch!(&fallback);
            if ambiguous {
                closed = false;
            }
            match fulfillment.try_evaluate_obligations() {
                Ok(more) => errors.extend(more),
                Err(overflow) => return Ok(overflow_outcome(overflow)),
            }
        }
    }
    match fulfillment.collect_remaining_errors() {
        Ok(residual) => errors.extend(residual),
        Err(overflow) => return Ok(overflow_outcome(overflow)),
    }
    if closed {
        if !errors.is_empty() {
            return Ok(ReachedCallOutcome::Failed(
                errors.iter().map(|error| failure(error, |ordinal| Some(*ordinal))).collect(),
            ));
        }
        if fulfillment.has_pending_obligations() {
            return Ok(held("fulfillment", "actual undrained obligations"));
        }
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
        closed,
        current: Arc::clone(current),
    }))
}
