//! Numeric-connected source-clause demand metadata after actual root relations.
//! No trait matching, numeric choice, fallback, fulfillment or root admission.
use super::{
    configuration::PreparedSolverConfiguration,
    identity::DefinitionId,
    infer::delegate::SolverContext,
    model::{self, Clause, Span, Ty},
    prepared::{Cx, PreparedProvider},
    root_input::SourceVariableFamily,
    root_relations::{SettledRootApplication, SourceRootApplication},
    root_variables::RootVariables,
    source_declaration_clauses::{
        PreparedSourceDeclarationClauses, SourceDeclarationClausesPending,
    },
    source_terms::SourceTypePending,
};
use crate::{
    core::digest::{Digest, StableEncode},
    frontend::declaration_source_predicates::DeclarationSourcePredicatesKey,
    traits::term::InferVarKey,
};
use rustc_type_ir::{self as ty, TypeSuperVisitable, TypeVisitable, TypeVisitor, inherent::*};
use std::collections::{BTreeSet, HashMap};

type SourceClause<'cx> = Clause<'cx, PreparedProvider<'cx>>;
type RootTy<'cx> = Ty<'cx, PreparedProvider<'cx>>;

#[derive(Debug)]
pub(super) enum SourceRuntimeDemandPending {
    Owner(SourceTypePending),
    Application(SourceTypePending),
    RelationGoals(usize),
    Clauses(SourceDeclarationClausesPending),
    SourceDefinition,
    SourceWorld,
    Callee,
    NumericSpecification(InferVarKey),
}

/// The only constructor encodes the actual retained Q80 plan once. A raw
/// supplied clause vector cannot manufacture source identity or completeness.
pub(super) struct EncodedRuntimeSourceClauses<'source, 'cx> {
    source: &'source PreparedSourceDeclarationClauses,
    fingerprint: Digest,
    context: Cx<'cx>,
    definition: DefinitionId,
    clauses: Vec<(SourceClause<'cx>, Span)>,
}
impl<'source, 'cx> EncodedRuntimeSourceClauses<'source, 'cx> {
    pub(super) fn prepare(
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
        source: &'source PreparedSourceDeclarationClauses,
    ) -> Result<Self, SourceRuntimeDemandPending> {
        let bindings = source
            .bindings()
            .map_err(SourceRuntimeDemandPending::Clauses)?;
        let definition = *bindings
            .definitions
            .get(&source.key().definition)
            .ok_or(SourceRuntimeDemandPending::SourceDefinition)?;
        if definition.world() != cx.services().source_world() {
            return Err(SourceRuntimeDemandPending::SourceWorld);
        }
        let clauses = source
            .encode(cx, configuration)
            .map_err(SourceRuntimeDemandPending::Clauses)?;
        Ok(Self {
            source,
            fingerprint: source.fingerprint(),
            context: cx,
            definition,
            clauses,
        })
    }
    pub(super) fn source_key(&self) -> &DeclarationSourcePredicatesKey {
        self.source.key()
    }
    pub(super) fn source_fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(super) fn clause_count(&self) -> usize {
        self.clauses.len()
    }
}

/// Identity is (retained Q80 key/fingerprint, ordinal, original span). The
/// original clause remains with its retained plan; this handle is instantiated
/// by the actual applied callee Args, without normalization or a width hint.
pub(super) struct NumericSourceClause<'cx> {
    ordinal: usize,
    span: Span,
    clause: SourceClause<'cx>,
    numeric_variables: BTreeSet<InferVarKey>,
}
impl<'cx> NumericSourceClause<'cx> {
    pub(super) fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub(super) fn span(&self) -> Span {
        self.span
    }
    pub(super) fn clause(&self) -> SourceClause<'cx> {
        self.clause
    }
    pub(super) fn numeric_variables(&self) -> &BTreeSet<InferVarKey> {
        &self.numeric_variables
    }
}

/// This describes selected source reads in one actual inference owner. It
/// carries neither a successful constraint answer nor permission for a root or
/// VM commit; the ordinary source constraint service still owns closure.
pub(super) struct SourceRuntimeDemands<'source, 'owner, 'cx> {
    source: &'source EncodedRuntimeSourceClauses<'source, 'cx>,
    owner: &'owner SolverContext<'cx>,
    snapshot: usize,
    callee: DefinitionId,
    signature: Digest,
    param_env: model::ParamEnv<'cx, PreparedProvider<'cx>>,
    clauses: Vec<NumericSourceClause<'cx>>,
}
impl<'source, 'owner, 'cx> SourceRuntimeDemands<'source, 'owner, 'cx> {
    pub(super) fn after_relations(
        owner: &'owner SolverContext<'cx>,
        variables: &RootVariables<'_, 'cx>,
        application: &SourceRootApplication<'_, 'cx>,
        source: &'source EncodedRuntimeSourceClauses<'source, 'cx>,
    ) -> Result<Self, SourceRuntimeDemandPending> {
        variables
            .require_owner(owner)
            .map_err(SourceRuntimeDemandPending::Owner)?;
        application
            .require_owner(owner)
            .map_err(SourceRuntimeDemandPending::Application)?;
        application
            .require_variables(variables)
            .map_err(SourceRuntimeDemandPending::Application)?;
        let outstanding = application.outstanding_relation_goals();
        if outstanding != 0 {
            return Err(SourceRuntimeDemandPending::RelationGoals(outstanding));
        }
        Self::select(owner, variables, application, source)
    }

    /// The settled value can only be constructed by real same-owner relation
    /// fulfillment. Its original application count is deliberately unchanged;
    /// this distinct path supplies the missing proof, not a completion flag.
    pub(super) fn after_fulfillment(
        owner: &'owner SolverContext<'cx>,
        variables: &RootVariables<'_, 'cx>,
        settled: &SettledRootApplication<'_, 'cx>,
        source: &'source EncodedRuntimeSourceClauses<'source, 'cx>,
    ) -> Result<Self, SourceRuntimeDemandPending> {
        variables
            .require_owner(owner)
            .map_err(SourceRuntimeDemandPending::Owner)?;
        let application = settled
            .source_application(owner, variables)
            .map_err(SourceRuntimeDemandPending::Application)?;
        Self::select(owner, variables, application, source)
    }

    fn select(
        owner: &'owner SolverContext<'cx>,
        variables: &RootVariables<'_, 'cx>,
        application: &SourceRootApplication<'_, 'cx>,
        source: &'source EncodedRuntimeSourceClauses<'source, 'cx>,
    ) -> Result<Self, SourceRuntimeDemandPending> {
        let cx = variables.context();
        if cx != source.context || owner.interner != cx {
            return Err(SourceRuntimeDemandPending::SourceWorld);
        }
        if application.callee() != source.definition {
            return Err(SourceRuntimeDemandPending::Callee);
        }
        let mut numeric = HashMap::<RootTy<'cx>, BTreeSet<InferVarKey>>::new();
        for (&key, &kind) in variables.specifications() {
            if kind == SourceVariableFamily::Type {
                continue;
            }
            let original = variables
                .variable(key)
                .map_err(SourceRuntimeDemandPending::Owner)?;
            let valid_original = matches!(
                (kind, original.kind()),
                (
                    SourceVariableFamily::Integer,
                    ty::Infer(ty::InferTy::IntVar(_))
                ) | (
                    SourceVariableFamily::Float,
                    ty::Infer(ty::InferTy::FloatVar(_))
                )
            );
            if !valid_original {
                return Err(SourceRuntimeDemandPending::NumericSpecification(key));
            }
            // Pinned resolution follows actual inference equalities and returns
            // canonical roots for merged Int/Float variables. Never relabel a
            // general Type source key as a numeric original.
            let resolved = variables
                .resolved(key)
                .map_err(SourceRuntimeDemandPending::Owner)?;
            match (kind, resolved.kind()) {
                (SourceVariableFamily::Integer, ty::Infer(ty::InferTy::IntVar(_)))
                | (SourceVariableFamily::Float, ty::Infer(ty::InferTy::FloatVar(_))) => {
                    numeric.entry(resolved).or_default().insert(key);
                }
                (SourceVariableFamily::Integer, ty::Int(_) | ty::Uint(_))
                | (SourceVariableFamily::Float, ty::Float(_)) => {}
                _ => return Err(SourceRuntimeDemandPending::NumericSpecification(key)),
            }
        }
        let mut clauses = Vec::new();
        if !numeric.is_empty() {
            for (ordinal, &(clause, span)) in source.clauses.iter().enumerate() {
                let instantiated = ty::EarlyBinder::bind(cx, clause)
                    .instantiate(cx, application.arguments())
                    .skip_norm_wip();
                let resolved = owner.deeply_resolve_ignoring_regions(instantiated);
                let mut occurrences = NumericOccurrences {
                    numeric: &numeric,
                    keys: BTreeSet::new(),
                };
                resolved.visit_with(&mut occurrences);
                if !occurrences.keys.is_empty() {
                    clauses.push(NumericSourceClause {
                        ordinal,
                        span,
                        clause: instantiated,
                        numeric_variables: occurrences.keys,
                    });
                }
            }
        }
        Ok(Self {
            source,
            owner,
            snapshot: owner.num_open_snapshots(),
            callee: application.callee(),
            signature: application.signature(),
            param_env: application.param_env(),
            clauses,
        })
    }
    pub(super) fn require_owner(
        &self,
        owner: &SolverContext<'cx>,
    ) -> Result<(), SourceTypePending> {
        if !std::ptr::eq(self.owner, owner) {
            return Err(SourceTypePending::InferenceOwner);
        }
        assert_eq!(self.snapshot, owner.num_open_snapshots());
        Ok(())
    }
    pub(super) fn source_key(&self) -> &DeclarationSourcePredicatesKey {
        self.source.source_key()
    }
    pub(super) fn source_fingerprint(&self) -> Digest {
        self.source.source_fingerprint()
    }
    pub(super) fn callee(&self) -> DefinitionId {
        self.callee
    }
    pub(super) fn signature(&self) -> Digest {
        self.signature
    }
    pub(super) fn param_env(&self) -> model::ParamEnv<'cx, PreparedProvider<'cx>> {
        self.param_env
    }
    pub(super) fn clauses(&self) -> &[NumericSourceClause<'cx>] {
        &self.clauses
    }
}

/// Syntactic dependency inspection over actual resolved TypeIR only. Walking
/// aliases visits their retained arguments; it never normalizes them, searches
/// trait candidates or expands a semantic connection graph.
struct NumericOccurrences<'a, 'cx> {
    numeric: &'a HashMap<RootTy<'cx>, BTreeSet<InferVarKey>>,
    keys: BTreeSet<InferVarKey>,
}
impl<'cx> TypeVisitor<Cx<'cx>> for NumericOccurrences<'_, 'cx> {
    type Result = ();
    fn visit_ty(&mut self, value: RootTy<'cx>) {
        if matches!(
            value.kind(),
            ty::Infer(ty::InferTy::IntVar(_) | ty::InferTy::FloatVar(_))
        ) && let Some(keys) = self.numeric.get(&value)
        {
            self.keys.extend(keys.iter().copied());
        }
        value.super_visit_with(self);
    }
}
