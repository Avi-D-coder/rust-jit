//! Actual reached source relations on one pinned inference owner.
//! No raw caller ParamEnv, numeric default, custom relation or fulfillment loop.
use super::{
    caller_application::VerifiedCallerApplication,
    configuration::PreparedSolverConfiguration,
    identity::DefinitionId,
    infer::{
        delegate::SolverContext,
        fulfillment::{
            FulfillmentContext, FulfillmentOverflow, NextSolverError, PredicateObligation,
        },
    },
    model::{self, AllocateList, Arg, Span, Ty},
    parameters::{DeclarationParameters, DeclaredParameterPreparation, ParameterPending},
    prepared::{Cx, PreparedProvider},
    root_input::{SourceCallRelationKind, SourceRootCallInput},
    root_variables::RootVariables,
    source_caller_clauses::{PreparedSourceCallerClauses, SourceCallerClausesPending},
    source_generics::SourceGenericLayouts,
    source_terms::{self, SourceTypePending},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    traits::term::{BoundKind, ParamKey},
};
use rustc_type_ir::error::TypeError;
use rustc_type_ir::relate::solver_relating::RelateExt;
use rustc_type_ir::{self as ty, Interner, Upcast, inherent::*};
use thin_vec::ThinVec;

type RootTy<'cx> = Ty<'cx, PreparedProvider<'cx>>;
type RootObligation<'cx> = PredicateObligation<'cx, RootRelationCause>;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum RootRelationSide {
    Formal,
    Actual,
    Argument(ParamKey),
}

/// Preparation requirements are distinct from an actual upstream TypeError.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum RootRelationsPending {
    Owner(SourceTypePending),
    VariableSpecifications,
    SourceWorld,
    CalleeDeclaration,
    Parameters(ParameterPending),
    MissingArgument {
        parameter: ParamKey,
        kind: BoundKind,
    },
    ParameterMap,
    Caller(SourceCallerClausesPending),
    Type {
        relation: Option<usize>,
        side: RootRelationSide,
        pending: SourceTypePending,
    },
}

#[derive(Debug)]
pub(super) enum RootRelationsPreparationError {
    Pending(RootRelationsPending),
    Source(Diagnostic),
}
impl From<RootRelationsPending> for RootRelationsPreparationError {
    fn from(value: RootRelationsPending) -> Self {
        Self::Pending(value)
    }
}

/// This indexes the retained reached packet; no source expression is copied or
/// evaluated. The actual root/call owner must keep and revalidate that packet.
#[derive(Debug, Clone, Copy)]
pub(super) struct RootRelationCause {
    pub(super) callee: Digest,
    pub(super) signature: Digest,
    pub(super) relation: usize,
    pub(super) kind: SourceCallRelationKind,
    pub(super) site: Span,
}

#[derive(Debug)]
pub(super) struct RootRelationError<'cx> {
    pub(super) cause: RootRelationCause,
    pub(super) formal: RootTy<'cx>,
    pub(super) actual: RootTy<'cx>,
    pub(super) error: TypeError<Cx<'cx>>,
}

struct EncodedRelation<'cx> {
    cause: RootRelationCause,
    formal: RootTy<'cx>,
    actual: RootTy<'cx>,
}

/// Typed immutable relation preparation, not source-fact/root/VM admission.
/// The source caller alone constructs its environment. No callee bound or raw
/// externally supplied ParamEnv can become an assumption through this API.
pub(super) struct PreparedRootRelations<'owner, 'cx> {
    application: SourceRootApplication<'owner, 'cx>,
    relations: Vec<EncodedRelation<'cx>>,
}

/// Immutable values actually used by source relation preparation. There is no
/// raw-Args/ParamEnv constructor: views can only be read from that retained
/// application, and they confer no source-currentness or root-entry authority.
pub(super) struct SourceRootApplication<'owner, 'cx> {
    owner: &'owner SolverContext<'cx>,
    variables: &'owner RootVariables<'owner, 'cx>,
    snapshot: usize,
    callee: DefinitionId,
    signature: Digest,
    arguments: model::Args<'cx, PreparedProvider<'cx>>,
    param_env: model::ParamEnv<'cx, PreparedProvider<'cx>>,
    // Set from the actual successful apply output before any view is exposed.
    // This component never certifies fulfillment or clears/decrements it.
    outstanding_relation_goals: usize,
}

/// Equality constraints have been applied to this owner, while every returned
/// child/coercion goal still needs its same-owner admitted root fulfillment.
/// This is not a successful coercion adjustment, witness or VM commit token.
pub(super) struct AppliedRootRelations<'owner, 'cx> {
    application: SourceRootApplication<'owner, 'cx>,
    obligations: ThinVec<RootObligation<'cx>>,
}

/// Consume the actual relation output together with its callee application.
/// The bundle is neither an admitted solver root nor a replacement-answer API.
pub(super) struct RootRelationParts<'owner, 'cx> {
    application: SourceRootApplication<'owner, 'cx>,
    obligations: ThinVec<RootObligation<'cx>>,
}

/// Retained original relation work, with no externally callable execution
/// method. Actual packet-bound source/query admission is still unavailable.
/// A future admitted runner in this module must own that join before calling
/// fulfill; merely constructing this work or publishing facts grants nothing.
pub(super) struct RootRelationFulfillmentWork<'owner, 'cx> {
    application: SourceRootApplication<'owner, 'cx>,
    original_obligations: ThinVec<RootObligation<'cx>>,
    // Keep the live terminal context on failure, including fatal overflow's
    // actual pending/stalled state. No retry/mutable-context accessor exists.
    fulfillment: Option<FulfillmentContext<'owner, 'cx, RootRelationCause>>,
}

/// Construction is private to actual successful registration/evaluation/drain.
/// This settles relation goals only, not runtime adjustments, external
/// constraints, numeric source goals, source admission or VM commit.
pub(super) struct SettledRootApplication<'owner, 'cx> {
    application: SourceRootApplication<'owner, 'cx>,
    fulfilled_obligations: ThinVec<RootObligation<'cx>>,
}

pub(super) enum RootRelationFulfillmentOutcome<'owner, 'cx> {
    Settled(SettledRootApplication<'owner, 'cx>),
    Unresolved {
        work: RootRelationFulfillmentWork<'owner, 'cx>,
        // TrueError and Ambiguity remain distinct; Ambiguity retains actual
        // upstream MaybeInfo in its stalled state, including search overflow.
        errors: ThinVec<NextSolverError<'cx, RootRelationCause>>,
    },
    Overflow {
        work: RootRelationFulfillmentWork<'owner, 'cx>,
        overflow: FulfillmentOverflow<'cx, RootRelationCause>,
    },
}

impl<'owner, 'cx> PreparedRootRelations<'owner, 'cx> {
    pub(super) fn prepare(
        owner: &'owner SolverContext<'cx>,
        variables: &'owner RootVariables<'owner, 'cx>,
        layouts: &'owner SourceGenericLayouts,
        configuration: &'owner PreparedSolverConfiguration,
        input: &SourceRootCallInput<'_>,
        caller_source: &PreparedSourceCallerClauses,
        application: Option<&VerifiedCallerApplication>,
    ) -> Result<Self, RootRelationsPreparationError> {
        variables
            .require_owner(owner)
            .map_err(RootRelationsPending::Owner)?;
        if variables.specifications() != input.variables {
            return Err(RootRelationsPending::VariableSpecifications.into());
        }
        let cx = variables.context();
        if cx.services().source_configuration() != configuration.fingerprint()
            || !layouts.matches_catalog(caller_source.catalog_input())
        {
            return Err(RootRelationsPending::SourceWorld.into());
        }
        // Missing GenericEnv never implies an empty environment. Both branches
        // require the retained actual caller source plan's complete proof.
        let param_env = match application {
            Some(application) => {
                caller_source.encode_applied(cx, configuration, input.caller, application)
            }
            None => caller_source.encode(cx, configuration, input.caller),
        }
        .map_err(RootRelationsPending::Caller)?;

        let bindings = layouts.type_bindings();
        let definition = bindings
            .definitions
            .get(&input.callee)
            .filter(|definition| definition.world() == cx.services().source_world())
            .copied()
            .ok_or(RootRelationsPending::CalleeDeclaration)?;
        let declaration = layouts
            .get(definition)
            .ok_or(RootRelationsPending::CalleeDeclaration)?;
        let parameters = match DeclarationParameters::prepare(declaration)
            .map_err(RootRelationsPreparationError::Source)?
        {
            DeclaredParameterPreparation::Ready(parameters) => parameters,
            DeclaredParameterPreparation::Pending(why) => {
                return Err(RootRelationsPending::Parameters(why).into());
            }
        };
        let mut arguments = Vec::new();
        for index in 0..parameters.count() {
            let parameter = parameters
                .from_upstream(index)
                .map_err(RootRelationsPreparationError::Source)?;
            // The reached packet currently owns Type arguments. An absent
            // region/const argument requires its real producer; never guess
            // erased/static regions, zero constants or fresh unrelated values.
            if parameter.kind() != BoundKind::Type {
                return Err(RootRelationsPending::MissingArgument {
                    parameter: parameter.source(),
                    kind: parameter.kind(),
                }
                .into());
            }
            let term = input.parameters.get(&parameter.source()).ok_or(
                RootRelationsPending::MissingArgument {
                    parameter: parameter.source(),
                    kind: parameter.kind(),
                },
            )?;
            let value = source_terms::encode_root_type(
                owner,
                variables,
                configuration,
                layouts.type_bindings(),
                term,
            )
            .map_err(|pending| RootRelationsPending::Type {
                relation: None,
                side: RootRelationSide::Argument(parameter.source()),
                pending,
            })?;
            arguments.push(Arg::Type(value));
        }
        if input.parameters.len() != arguments.len() {
            return Err(RootRelationsPending::ParameterMap.into());
        }
        let arguments: model::Args<'cx, PreparedProvider<'cx>> = cx.list(arguments);
        let mut relations = Vec::new();
        // Complete all fallible source encoding before any mutable inference
        // operation. Source Types are encoded once, then ordinary upstream
        // EarlyBinder instantiation substitutes the genuine callee arguments.
        for (relation, source) in input.relations.iter().enumerate() {
            let formal = source_terms::encode_root_type(
                owner,
                variables,
                configuration,
                layouts.type_bindings(),
                &source.formal,
            )
            .map_err(|pending| RootRelationsPending::Type {
                relation: Some(relation),
                side: RootRelationSide::Formal,
                pending,
            })?;
            // Keep aliases unnormalized for the actual relation/solver, as in
            // this pin's next-solver EarlyBinder call sites. This unwrap is not
            // a claim that any source projection has already normalized.
            let formal = ty::EarlyBinder::bind(cx, formal)
                .instantiate(cx, arguments)
                .skip_norm_wip();
            let actual = source_terms::encode_root_type(
                owner,
                variables,
                configuration,
                layouts.type_bindings(),
                &source.actual,
            )
            .map_err(|pending| RootRelationsPending::Type {
                relation: Some(relation),
                side: RootRelationSide::Actual,
                pending,
            })?;
            relations.push(EncodedRelation {
                cause: RootRelationCause {
                    callee: input.callee,
                    signature: input.signature,
                    relation,
                    kind: source.kind,
                    site: source.site,
                },
                formal,
                actual,
            });
        }
        Ok(Self {
            application: SourceRootApplication {
                owner,
                variables,
                snapshot: owner.num_open_snapshots(),
                callee: definition,
                signature: input.signature,
                arguments,
                param_env,
                outstanding_relation_goals: 0,
            },
            relations,
        })
    }

    /// Use only the already-existing pinned inference transaction and relation
    /// implementations. No failed equality leaks its partial constraints into
    /// another attempt, and success retains every actual emitted child goal.
    pub(super) fn apply(self) -> Result<AppliedRootRelations<'owner, 'cx>, RootRelationError<'cx>> {
        let Self {
            mut application,
            relations,
        } = self;
        let owner = application.owner;
        application
            .require_owner(owner)
            .expect("retained relation application owner");
        let param_env = application.param_env;
        let cx = owner.interner;
        let obligations = owner.commit_if_ok(|_| {
            let mut obligations = ThinVec::new();
            for relation in relations {
                let EncodedRelation {
                    cause,
                    formal,
                    actual,
                } = relation;
                match cause.kind {
                    SourceCallRelationKind::Equal => {
                        // Exact next-solver infer/at.rs::eq_trace dispatch.
                        let goals = RelateExt::relate(
                            &**owner,
                            param_env,
                            formal,
                            ty::Invariant,
                            actual,
                            cause.site,
                        )
                        .map_err(|error| RootRelationError {
                            cause,
                            formal,
                            actual,
                            error,
                        })?;
                        for goal in goals {
                            obligations.push(PredicateObligation {
                                cause,
                                span: cause.site,
                                param_env: goal.param_env,
                                predicate: goal.predicate,
                                recursion_depth: 0,
                            });
                        }
                    }
                    SourceCallRelationKind::Coerce | SourceCallRelationKind::CoerceResult => {
                        let (a, b) = match cause.kind {
                            SourceCallRelationKind::Coerce => (actual, formal),
                            SourceCallRelationKind::CoerceResult => (formal, actual),
                            SourceCallRelationKind::Equal => unreachable!("equality handled above"),
                        };
                        // The pinned shared compute_coerce_goal owns this real
                        // inference obligation. HIR adjustment/runtime value
                        // conversion acceptance is a separate common mechanism.
                        let predicate =
                            ty::Binder::dummy(ty::PredicateKind::Coerce(ty::CoercePredicate {
                                a,
                                b,
                            }))
                            .upcast(cx);
                        obligations.push(PredicateObligation {
                            cause,
                            span: cause.site,
                            param_env,
                            predicate,
                            recursion_depth: 0,
                        });
                    }
                }
            }
            Ok(obligations)
        })?;
        application.outstanding_relation_goals = obligations.len();
        Ok(AppliedRootRelations {
            application,
            obligations,
        })
    }
}

impl<'owner, 'cx> AppliedRootRelations<'owner, 'cx> {
    /// Reuse actual arguments and caller assumptions for source-clause
    /// instantiation. Reading the view never admits or evaluates another root.
    pub(super) fn source_application(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<&SourceRootApplication<'owner, 'cx>, SourceTypePending> {
        self.application.require_owner(owner)?;
        Ok(&self.application)
    }

    /// No goal, argument or environment replacement. Keep the exact application
    /// with every emitted obligation instead of discarding it during transfer.
    pub(super) fn into_parts(
        self,
        owner: &SolverContext<'cx>,
    ) -> Result<RootRelationParts<'owner, 'cx>, SourceTypePending> {
        self.application.require_owner(owner)?;
        Ok(RootRelationParts {
            application: self.application,
            obligations: self.obligations,
        })
    }
}

impl<'owner, 'cx> RootRelationParts<'owner, 'cx> {
    /// Bind the retained work to its original variables before any solver call.
    /// On an ownership mismatch return all consumed work unchanged. There is
    /// deliberately no externally callable method that executes this work.
    pub(super) fn into_fulfillment_work(
        self,
        owner: &SolverContext<'cx>,
        variables: &RootVariables<'_, 'cx>,
    ) -> Result<RootRelationFulfillmentWork<'owner, 'cx>, (Self, SourceTypePending)> {
        if let Err(pending) = self
            .application
            .require_owner(owner)
            .and_then(|()| self.application.require_variables(variables))
        {
            return Err((self, pending));
        }
        Ok(RootRelationFulfillmentWork {
            application: self.application,
            original_obligations: self.obligations,
            fulfillment: None,
        })
    }

    pub(super) fn source_application(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<&SourceRootApplication<'owner, 'cx>, SourceTypePending> {
        self.application.require_owner(owner)?;
        Ok(&self.application)
    }

    /// Transfer the originally constructed application and obligations together.
    /// The source root owner still owns demand/activation/currentness checks.
    pub(super) fn into_application_and_obligations(
        self,
        owner: &SolverContext<'cx>,
    ) -> Result<
        (
            SourceRootApplication<'owner, 'cx>,
            ThinVec<RootObligation<'cx>>,
        ),
        SourceTypePending,
    > {
        self.application.require_owner(owner)?;
        Ok((self.application, self.obligations))
    }
}

impl<'owner, 'cx> RootRelationFulfillmentWork<'owner, 'cx> {
    pub(super) fn source_application(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<&SourceRootApplication<'owner, 'cx>, SourceTypePending> {
        self.application.require_owner(owner)?;
        Ok(&self.application)
    }

    pub(super) fn original_obligations(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<&[RootObligation<'cx>], SourceTypePending> {
        self.application.require_owner(owner)?;
        Ok(&self.original_obligations)
    }

    /// Read-only retention on unresolved/fatal outcomes; it exposes neither a
    /// mutable retry path nor a way to register replacement goals.
    pub(super) fn retained_fulfillment(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<Option<&FulfillmentContext<'owner, 'cx, RootRelationCause>>, SourceTypePending>
    {
        self.application.require_owner(owner)?;
        Ok(self.fulfillment.as_ref())
    }

    /// Only the private source-bound runner can construct the admission passed
    /// here. Original application/variables remain attached to all outcomes.
    pub(super) fn fulfill(mut self, admission: &super::reached_call::SourceRootAdmission) -> RootRelationFulfillmentOutcome<'owner, 'cx> {
        let owner = self.application.owner;
        owner.require_source_root();
        assert_eq!(owner.interner.services().active_source_root(), Some(admission.identity()));
        self.application
            .require_owner(owner)
            .expect("original relation fulfillment owner");
        self.application
            .require_variables(self.application.variables)
            .expect("original relation fulfillment variables");
        assert!(
            self.fulfillment.is_none(),
            "relation work is registered only once"
        );
        let mut fulfillment = FulfillmentContext::new(owner);
        for obligation in &self.original_obligations {
            fulfillment.register_predicate_obligation(obligation.clone());
        }
        let mut errors = match fulfillment.try_evaluate_obligations() {
            Ok(errors) => errors,
            Err(overflow) => {
                self.fulfillment = Some(fulfillment);
                return RootRelationFulfillmentOutcome::Overflow {
                    work: self,
                    overflow,
                };
            }
        };
        // This consumes only the real unresolved goals and preserves their
        // exact upstream stall/Maybe information as typed Ambiguity records.
        match fulfillment.collect_remaining_errors() {
            Ok(residuals) => errors.extend(residuals),
            Err(mut overflow) => {
                // A fatal terminal must never become ordinary ambiguity or
                // success. Preserve any already returned errors as well.
                errors.extend(overflow.errors);
                overflow.errors = errors;
                self.fulfillment = Some(fulfillment);
                return RootRelationFulfillmentOutcome::Overflow {
                    work: self,
                    overflow,
                };
            }
        }
        if errors.is_empty() && !fulfillment.has_pending_obligations() {
            RootRelationFulfillmentOutcome::Settled(SettledRootApplication {
                application: self.application,
                fulfilled_obligations: self.original_obligations,
            })
        } else {
            self.fulfillment = Some(fulfillment);
            RootRelationFulfillmentOutcome::Unresolved { work: self, errors }
        }
    }
}

impl<'owner, 'cx> SettledRootApplication<'owner, 'cx> {
    pub(super) fn source_application(
        &self,
        owner: &SolverContext<'cx>,
        variables: &RootVariables<'_, 'cx>,
    ) -> Result<&SourceRootApplication<'owner, 'cx>, SourceTypePending> {
        self.application.require_owner(owner)?;
        self.application.require_variables(variables)?;
        Ok(&self.application)
    }

    pub(super) fn fulfilled_obligations(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<&[RootObligation<'cx>], SourceTypePending> {
        self.application.require_owner(owner)?;
        Ok(&self.fulfilled_obligations)
    }
}

impl<'owner, 'cx> SourceRootApplication<'owner, 'cx> {
    /// The application of one selected call site of a source component: the
    /// same inference owner, variables and caller assumptions, with that
    /// callee's generic Type arguments (each a component variable). It
    /// confers no more authority than the component's own application.
    pub(super) fn site(
        &self,
        layouts: &SourceGenericLayouts,
        configuration: &PreparedSolverConfiguration,
        callee: Digest,
        signature: Digest,
        parameters: &std::collections::BTreeMap<ParamKey, crate::traits::term::TyTerm>,
    ) -> Result<Self, RootRelationsPreparationError> {
        let owner = self.owner;
        let cx = owner.interner;
        let bindings = layouts.type_bindings();
        let definition = bindings
            .definitions
            .get(&callee)
            .filter(|definition| definition.world() == cx.services().source_world())
            .copied()
            .ok_or(RootRelationsPending::CalleeDeclaration)?;
        let declaration = layouts
            .get(definition)
            .ok_or(RootRelationsPending::CalleeDeclaration)?;
        let declared = match DeclarationParameters::prepare(declaration)
            .map_err(RootRelationsPreparationError::Source)?
        {
            DeclaredParameterPreparation::Ready(declared) => declared,
            DeclaredParameterPreparation::Pending(why) => {
                return Err(RootRelationsPending::Parameters(why).into());
            }
        };
        let mut arguments = Vec::new();
        for index in 0..declared.count() {
            let parameter = declared
                .from_upstream(index)
                .map_err(RootRelationsPreparationError::Source)?;
            if parameter.kind() != BoundKind::Type {
                return Err(RootRelationsPending::MissingArgument {
                    parameter: parameter.source(),
                    kind: parameter.kind(),
                }
                .into());
            }
            let term = parameters.get(&parameter.source()).ok_or(
                RootRelationsPending::MissingArgument {
                    parameter: parameter.source(),
                    kind: parameter.kind(),
                },
            )?;
            let value = source_terms::encode_root_type(owner, self.variables, configuration, bindings, term)
                .map_err(|pending| RootRelationsPending::Type {
                    relation: None,
                    side: RootRelationSide::Argument(parameter.source()),
                    pending,
                })?;
            arguments.push(Arg::Type(value));
        }
        if parameters.len() != arguments.len() {
            return Err(RootRelationsPending::ParameterMap.into());
        }
        Ok(Self {
            owner,
            variables: self.variables,
            snapshot: owner.num_open_snapshots(),
            callee: definition,
            signature,
            arguments: cx.list(arguments),
            param_env: self.param_env,
            outstanding_relation_goals: 0,
        })
    }

    pub(super) fn require_variables(
        &self,
        variables: &RootVariables<'_, 'cx>,
    ) -> Result<(), SourceTypePending> {
        if !std::ptr::eq(self.variables, variables) {
            return Err(SourceTypePending::InferenceOwner);
        }
        self.require_snapshot();
        Ok(())
    }

    pub(super) fn require_owner(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<(), SourceTypePending> {
        if !std::ptr::eq(self.owner, owner) {
            return Err(SourceTypePending::InferenceOwner);
        }
        self.require_snapshot();
        Ok(())
    }

    fn require_snapshot(&self) {
        assert_eq!(self.snapshot, self.owner.num_open_snapshots());
    }

    pub(super) fn callee(&self) -> DefinitionId {
        self.require_snapshot();
        self.callee
    }

    pub(super) fn signature(&self) -> Digest {
        self.require_snapshot();
        self.signature
    }

    pub(super) fn arguments(&self) -> model::Args<'cx, PreparedProvider<'cx>> {
        self.require_snapshot();
        self.arguments
    }

    pub(super) fn param_env(&self) -> model::ParamEnv<'cx, PreparedProvider<'cx>> {
        self.require_snapshot();
        self.param_env
    }

    /// Actual child/coercion goals emitted by relation application remain
    /// unfulfilled here. Source numeric incidence must hold if nonzero: a
    /// callee Type variable may not yet reveal its original numeric argument.
    /// Zero means apply emitted no deferred relation goals, not root success.
    /// Only a future admitted fulfillment owner can issue its separate settled
    /// application proof; this source view has no completion/decrement API.
    pub(super) fn outstanding_relation_goals(&self) -> usize {
        self.require_snapshot();
        self.outstanding_relation_goals
    }
}
