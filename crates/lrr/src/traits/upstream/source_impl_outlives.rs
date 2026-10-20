//! Actual immutable inputs for pinned impl-super-outlives elaboration.
//! This is a metadata dependency closure, never a trait/cycle evaluator.
//! Only the parent's admitted shared provider may invoke the pinned iterator.
use super::{
    configuration::{PreparedSolverConfiguration, SolverConfigurationKey},
    identity::DefinitionId,
    model::{Clause, Span, Ty},
    prepared::{self, Cx},
    source_bindings::SourceBindings,
    source_definition_headers::DefinitionHeaderKey,
    source_generics::SourceGenericLayouts,
    source_impl_semantics::{ImplSemantic, ImplSemanticsPending, PreparedSourceImplSemantics},
    source_lang_identity::LangIdentityPending,
    source_terms::SourceTypePending,
    source_trait_clauses::{
        PreparedSourceTraitClauses, SourceTraitClauseFilter, SourceTraitClausesKey,
        SourceTraitClausesPending,
    },
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
        value::QueryValue,
    },
    traits::header_syntax::ImplHeaderSyntaxKey,
};
use rustc_type_ir::{
    self as ty, TypeSuperVisitable, TypeVisitable, TypeVisitableExt, TypeVisitor, inherent::*,
};
use std::{collections::BTreeMap, collections::BTreeSet, sync::Arc};

pub type SourceImplOutlivesKey = ImplHeaderSyntaxKey;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ImplOutlivesDomain {
    TraitImpl {
        implementation: Digest,
        trait_definition: Digest,
    },
    NotApplicable,
    Pending,
}

/// Actual unavailable source-query values, retaining their concrete keys and
/// reasons. The corresponding query roots are kept in the immutable plan.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ImplOutlivesRequirement {
    Implementation(ImplSemanticsPending),
    Head(SourceTypePending),
    Implied {
        key: SourceTraitClausesKey,
        pending: SourceTraitClausesPending,
    },
    Language {
        key: DefinitionHeaderKey,
        pending: LangIdentityPending,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ImplOutlivesEncodePending {
    SourceRequirements,
    NotApplicable,
    Configuration,
    Context,
    Head(SourceTypePending),
    Implied {
        definition: Digest,
        pending: SourceTraitClausesPending,
    },
    Identity,
}

#[derive(Debug)]
pub struct PreparedSourceImplOutlives {
    key: SourceImplOutlivesKey,
    catalog: Arc<InputValue>,
    implementation: Arc<QueryValue>,
    configuration: Option<Arc<QueryValue>>,
    traits: BTreeMap<Digest, Arc<QueryValue>>,
    languages: BTreeMap<Digest, Arc<QueryValue>>,
    domain: ImplOutlivesDomain,
    requirements: Vec<ImplOutlivesRequirement>,
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

impl PreparedSourceImplOutlives {
    pub fn key(&self) -> &SourceImplOutlivesKey {
        &self.key
    }
    pub(crate) fn domain(&self) -> ImplOutlivesDomain {
        self.domain
    }
    pub(crate) fn requirements(&self) -> &[ImplOutlivesRequirement] {
        &self.requirements
    }
    /// Source availability only; inspect domain() and encoded callback
    /// requirements independently before any admitted algorithm entry.
    pub(crate) fn pending(&self) -> Option<&ImplOutlivesRequirement> {
        self.requirements.first()
    }
    pub(super) fn catalog_input(&self) -> &Arc<InputValue> {
        &self.catalog
    }
    fn implementation(&self) -> &PreparedSourceImplSemantics {
        let QueryValue::SourceImplSemantics(plan) = self.implementation.as_ref() else {
            unreachable!("retained actual impl semantics")
        };
        plan
    }
    pub(super) fn implementation_plan(&self) -> &PreparedSourceImplSemantics {
        self.implementation()
    }
    pub(super) fn trait_plans(&self) -> impl Iterator<Item = &PreparedSourceTraitClauses> {
        self.traits.values().map(|root| {
            let QueryValue::SourceTraitClauses(plan) = root.as_ref() else {
                unreachable!()
            };
            plan.as_ref()
        })
    }
    pub(super) fn configuration(
        &self,
    ) -> Result<&PreparedSolverConfiguration, ImplOutlivesEncodePending> {
        match self.configuration.as_deref() {
            Some(QueryValue::SolverConfiguration(configuration)) => Ok(configuration),
            _ => Err(ImplOutlivesEncodePending::Configuration),
        }
    }
    fn require(&mut self, requirement: ImplOutlivesRequirement) {
        // Graph keys are already unique. Only the two initial Q86 domain
        // reads can yield the same adjacent capture hold.
        if self.requirements.last() != Some(&requirement) {
            self.requirements.push(requirement);
        }
    }

    pub(crate) fn prepare(db: &mut Database, key: &SourceImplOutlivesKey) -> QueryPoll<Self> {
        let implementation = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceImplSemantics(Box::new(key.clone()))
        ))
        .value;
        let Some(catalog) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("impl elaboration source catalog absent")));
        };
        let InputValue::DiscoverySeal(source) = catalog.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl elaboration catalog variant differs")));
        };
        if let Err(error) = prepared::catalog_world(source) {
            return QueryPoll::Ready(Err(error));
        }
        let mut result = Self {
            key: key.clone(),
            catalog,
            implementation,
            configuration: None,
            traits: BTreeMap::new(),
            languages: BTreeMap::new(),
            domain: ImplOutlivesDomain::Pending,
            requirements: Vec::new(),
        };
        if result.implementation().key() != key {
            return QueryPoll::Ready(Err(fault("impl elaboration source key differs")));
        }
        let actual_impl = match result.implementation().impl_definition() {
            ImplSemantic::Known(value) => Some(value),
            ImplSemantic::Pending(why) => {
                result.require(ImplOutlivesRequirement::Implementation(why));
                None
            }
            ImplSemantic::NotApplicable => {
                return QueryPoll::Ready(Err(fault("impl definition domain differs")));
            }
        };
        let actual_trait = match result.implementation().trait_definition() {
            ImplSemantic::Known(value) => Some(value),
            ImplSemantic::Pending(why) => {
                result.require(ImplOutlivesRequirement::Implementation(why));
                None
            }
            ImplSemantic::NotApplicable => {
                result.domain = ImplOutlivesDomain::NotApplicable;
                return QueryPoll::Ready(Ok(result));
            }
        };
        let (Some(implementation), Some(trait_definition)) = (actual_impl, actual_trait) else {
            return QueryPoll::Ready(Ok(result));
        };
        result.domain = ImplOutlivesDomain::TraitImpl {
            implementation,
            trait_definition,
        };
        match result.implementation().source_types() {
            Some(types) => {
                if !Arc::ptr_eq(types.catalog_input(), &result.catalog) {
                    return QueryPoll::Ready(Err(fault(
                        "impl head belongs to another source catalog",
                    )));
                }
                if let Some(why) = types.pending() {
                    result.require(ImplOutlivesRequirement::Head(why));
                }
            }
            None => result.require(ImplOutlivesRequirement::Head(SourceTypePending::HeadTypes)),
        }

        // This set deduplicates concrete source-query preparation, not semantic
        // clauses/instantiations/cycles. All edges remain in retained Q88 roots.
        let mut work = BTreeSet::from([trait_definition]);
        while let Some(definition) = work.pop_first() {
            if result.traits.contains_key(&definition) {
                continue;
            }
            let trait_key = SourceTraitClausesKey {
                inventory: key.inventory.clone(),
                definition,
                filter: SourceTraitClauseFilter::Implied,
            };
            let root = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceTraitClauses(Box::new(trait_key.clone()))
            ))
            .value;
            let QueryValue::SourceTraitClauses(plan) = root.as_ref() else {
                return QueryPoll::Ready(Err(fault("impl elaboration implied query differs")));
            };
            if plan.key() != &trait_key || !Arc::ptr_eq(&plan.bindings().catalog, &result.catalog) {
                return QueryPoll::Ready(Err(fault(
                    "impl elaboration trait source identity differs",
                )));
            }
            if let Some(pending) = plan.pending() {
                result.require(ImplOutlivesRequirement::Implied {
                    key: trait_key,
                    pending,
                });
            }
            for dependency in plan.positive_dependencies() {
                if dependency.filter != SourceTraitClauseFilter::Implied {
                    return QueryPoll::Ready(Err(fault("impl elaboration query family differs")));
                }
                work.insert(dependency.definition);
            }
            result.traits.insert(definition, root);
            let language_key = DefinitionHeaderKey {
                inventory: key.inventory.clone(),
                definition,
            };
            let language = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceLangIdentity(Box::new(language_key.clone()))
            ))
            .value;
            let QueryValue::SourceLangIdentity(identity) = language.as_ref() else {
                return QueryPoll::Ready(Err(fault("impl elaboration language query differs")));
            };
            if identity.key() != &language_key {
                return QueryPoll::Ready(Err(fault(
                    "impl elaboration language source key differs",
                )));
            }
            if let Err(pending) = identity.classification() {
                result.require(ImplOutlivesRequirement::Language {
                    key: language_key,
                    pending,
                });
            }
            result.languages.insert(definition, language);
        }
        let InputValue::DiscoverySeal(catalog) = result.catalog.as_ref() else {
            unreachable!()
        };
        let root = catalog
            .impl_catalog
            .as_ref()
            .expect("validated source catalog")
            .root();
        let configuration_key = SolverConfigurationKey::from_source(root);
        let configuration = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SolverConfiguration(Box::new(configuration_key.clone()))
        ))
        .value;
        let QueryValue::SolverConfiguration(actual) = configuration.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl elaboration configuration query differs")));
        };
        if !actual.matches_key(&configuration_key) {
            return QueryPoll::Ready(Err(fault("impl elaboration configuration source differs")));
        }
        result.configuration = Some(configuration);
        QueryPoll::Ready(Ok(result))
    }

    fn bindings(&self) -> Result<Vec<&SourceBindings>, ImplOutlivesEncodePending> {
        if !self.requirements.is_empty() {
            return Err(ImplOutlivesEncodePending::SourceRequirements);
        }
        if !matches!(self.domain, ImplOutlivesDomain::TraitImpl { .. }) {
            return Err(ImplOutlivesEncodePending::NotApplicable);
        }
        let types = self
            .implementation()
            .source_types()
            .ok_or(ImplOutlivesEncodePending::Head(
                SourceTypePending::HeadTypes,
            ))?;
        let mut bindings = vec![types.bindings()];
        for root in self.traits.values() {
            let QueryValue::SourceTraitClauses(plan) = root.as_ref() else {
                unreachable!()
            };
            bindings.push(plan.bindings());
        }
        Ok(bindings)
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        let bindings = self
            .bindings()
            .map_err(|why| fault(&format!("impl elaboration source remains pending: {why:?}")))?;
        SourceGenericLayouts::from_bindings(Arc::clone(&self.catalog), bindings)
    }

    /// Encode the genuine callback input values in one actual source context.
    /// This does not run elaborate, publish facts, or authorize solver entry.
    pub(super) fn encode_inputs<'cx>(
        &self,
        cx: Cx<'cx>,
    ) -> Result<EncodedImplOutlivesInputs<'cx>, ImplOutlivesEncodePending> {
        self.bindings()?;
        let ImplOutlivesDomain::TraitImpl {
            implementation,
            trait_definition,
        } = self.domain
        else {
            return Err(ImplOutlivesEncodePending::NotApplicable);
        };
        let configuration = self.configuration()?;
        let types = self
            .implementation()
            .source_types()
            .ok_or(ImplOutlivesEncodePending::Head(
                SourceTypePending::HeadTypes,
            ))?;
        let implementation_id = *types
            .bindings()
            .definitions
            .get(&implementation)
            .ok_or(ImplOutlivesEncodePending::Identity)?;
        if cx.services().source_world() != implementation_id.world() {
            return Err(ImplOutlivesEncodePending::Context);
        }
        let head = self
            .implementation()
            .encode_head(cx, configuration)
            .map_err(ImplOutlivesEncodePending::Head)?
            .trait_arguments
            .ok_or(ImplOutlivesEncodePending::Identity)?
            .construct(cx);
        if head.def_id.source() != trait_definition {
            return Err(ImplOutlivesEncodePending::Identity);
        }
        let mut implied = BTreeMap::new();
        let mut sized = BTreeMap::new();
        for (source, root) in &self.traits {
            let QueryValue::SourceTraitClauses(plan) = root.as_ref() else {
                unreachable!()
            };
            let id = *plan
                .bindings()
                .definitions
                .get(source)
                .ok_or(ImplOutlivesEncodePending::Identity)?;
            let clauses = plan.encode(cx, configuration).map_err(|pending| {
                ImplOutlivesEncodePending::Implied {
                    definition: *source,
                    pending,
                }
            })?;
            let Some(QueryValue::SourceLangIdentity(identity)) =
                self.languages.get(source).map(Arc::as_ref)
            else {
                return Err(ImplOutlivesEncodePending::Identity);
            };
            let role = identity
                .classification()
                .map_err(|_| ImplOutlivesEncodePending::SourceRequirements)?;
            sized.insert(id, matches!(role, Some(("sized", _))));
            implied.insert(id, clauses);
        }
        let mut result = EncodedImplOutlivesInputs {
            implementation: implementation_id,
            head,
            implied,
            sized,
            requirements: Vec::new(),
            source: self.fingerprint(),
        };
        result.inspect_trait(head.def_id, true);
        // Inspect every encoded form, including rows not selected by an
        // assumed source-level applicability test. No branch is guessed absent.
        let mut observations = Vec::new();
        let mut type_outlives = false;
        for clauses in result.implied.values() {
            for (clause, _) in clauses {
                match clause.kind().skip_binder() {
                    ty::ClauseKind::Trait(trait_clause) => observations.push((
                        trait_clause.def_id(),
                        trait_clause.polarity == ty::ClausePolarity::Positive,
                    )),
                    ty::ClauseKind::HostEffect(effect) => result.requirements.push(
                        ImplOutlivesEncodedRequirement::ImpliedConstBounds(effect.def_id()),
                    ),
                    ty::ClauseKind::TypeOutlives(ty::OutlivesClause(_, region))
                        if !region.is_bound() =>
                    {
                        type_outlives = true;
                    }
                    ty::ClauseKind::TypeOutlives(_)
                    | ty::ClauseKind::RegionOutlives(_)
                    | ty::ClauseKind::Projection(_)
                    | ty::ClauseKind::WellFormed(_)
                    | ty::ClauseKind::ConstEvaluatable(_)
                    | ty::ClauseKind::ConstArgHasType(_, _)
                    | ty::ClauseKind::UnstableFeature(_) => {}
                }
            }
        }
        for (definition, positive) in observations {
            result.inspect_trait(definition, positive);
        }
        if type_outlives {
            // A raw Self/T outlives subject can acquire aliases through the
            // actual impl/intermediate trait arguments. Inspect all genuine
            // inputs conservatively, without reimplementing substitution.
            // Bound inputs require a future binder-aware callback proof;
            // otherwise substitution cannot introduce escaping bound vars.
            if head.args.has_bound_vars()
                || result
                    .implied
                    .values()
                    .flatten()
                    .any(|(clause, _)| clause.has_bound_vars())
            {
                result
                    .requirements
                    .push(ImplOutlivesEncodedRequirement::OutlivesBoundInputs);
            }
            let mut visitor = EscapingAliasInputs {
                requirements: &mut result.requirements,
            };
            head.args.visit_with(&mut visitor);
            for (clause, _) in result.implied.values().flatten() {
                clause.visit_with(&mut visitor);
            }
        }
        Ok(result)
    }
}

/// Additional concrete domains found by inspecting the real encoded clauses.
/// These are required before the parent's admitted context may run elaborate.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ImplOutlivesEncodedRequirement<'cx> {
    SizedIdentity(DefinitionId),
    ImpliedClauses(DefinitionId),
    ImpliedConstBounds(DefinitionId),
    EscapingAliasVariances(ty::AliasTyKind<Cx<'cx>>),
    OutlivesFunctionBinder(DefinitionId),
    OutlivesBoundInputs,
}

/// Private construction ensures these values derive from actual retained plans.
/// Empty requirements alone is not an arbitrary-root/provider certificate.
#[derive(Debug)]
pub(super) struct EncodedImplOutlivesInputs<'cx> {
    implementation: DefinitionId,
    head: ty::TraitRef<Cx<'cx>>,
    implied: BTreeMap<DefinitionId, Vec<(Clause<'cx, prepared::PreparedProvider<'cx>>, Span)>>,
    sized: BTreeMap<DefinitionId, bool>,
    requirements: Vec<ImplOutlivesEncodedRequirement<'cx>>,
    source: Digest,
}
impl<'cx> EncodedImplOutlivesInputs<'cx> {
    pub(super) fn implementation(&self) -> DefinitionId {
        self.implementation
    }
    pub(super) fn head(&self) -> ty::TraitRef<Cx<'cx>> {
        self.head
    }
    pub(super) fn implied(
        &self,
    ) -> &BTreeMap<DefinitionId, Vec<(Clause<'cx, prepared::PreparedProvider<'cx>>, Span)>> {
        &self.implied
    }
    pub(super) fn sized(&self) -> &BTreeMap<DefinitionId, bool> {
        &self.sized
    }
    pub(super) fn requirements(&self) -> &[ImplOutlivesEncodedRequirement<'cx>] {
        &self.requirements
    }
    pub(super) fn source_fingerprint(&self) -> Digest {
        self.source
    }
    fn inspect_trait(&mut self, definition: DefinitionId, positive: bool) {
        match self.sized.get(&definition) {
            None => self
                .requirements
                .push(ImplOutlivesEncodedRequirement::SizedIdentity(definition)),
            Some(false) if positive && !self.implied.contains_key(&definition) => self
                .requirements
                .push(ImplOutlivesEncodedRequirement::ImpliedClauses(definition)),
            _ => {}
        }
    }
}

// Dependency inspection only: no region relation or alias normalization is
// derived here. The pinned elaborator/outlives visitor owns those semantics.
struct EscapingAliasInputs<'a, 'cx> {
    requirements: &'a mut Vec<ImplOutlivesEncodedRequirement<'cx>>,
}
impl<'cx> TypeVisitor<Cx<'cx>> for EscapingAliasInputs<'_, 'cx> {
    type Result = ();
    fn visit_ty(&mut self, value: Ty<'cx, prepared::PreparedProvider<'cx>>) {
        if let ty::Alias(_, alias) = value.kind() {
            if alias.has_escaping_bound_vars() {
                self.requirements
                    .push(ImplOutlivesEncodedRequirement::EscapingAliasVariances(
                        alias.kind,
                    ));
            }
            return;
        }
        // The pinned outlives visitor requires FnDef arguments with no bound
        // variables before traversal; do not enter it with an unproved input.
        if let ty::FnDef(definition, arguments) = value.kind()
            && arguments.no_bound_vars().is_none()
        {
            self.requirements
                .push(ImplOutlivesEncodedRequirement::OutlivesFunctionBinder(
                    definition,
                ));
            return;
        }
        value.super_visit_with(self);
    }
}

impl StableEncode for PreparedSourceImplOutlives {
    const DOMAIN: &'static str = "lrr/v1/source-impl-outlives-inputs";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.catalog.fingerprint());
        e.digest(self.implementation.fingerprint());
        e.option(&self.configuration, |e, root| e.digest(root.fingerprint()));
        e.map(&self.traits, |e, definition, root| {
            e.digest(*definition);
            e.digest(root.fingerprint());
        });
        e.map(&self.languages, |e, definition, root| {
            e.digest(*definition);
            e.digest(root.fingerprint());
        });
        match self.domain {
            ImplOutlivesDomain::TraitImpl {
                implementation,
                trait_definition,
            } => {
                e.u32(0);
                e.digest(implementation);
                e.digest(trait_definition);
            }
            ImplOutlivesDomain::NotApplicable => e.u32(1),
            ImplOutlivesDomain::Pending => e.u32(2),
        }
        // Reasons are authenticated by the exact retained query fingerprints;
        // these source-key tags identify every outstanding dependency as well.
        e.sequence(&self.requirements, |e, requirement| match requirement {
            ImplOutlivesRequirement::Implementation(why) => {
                e.u32(0);
                e.u32(*why as u32);
            }
            ImplOutlivesRequirement::Head(why) => {
                e.u32(1);
                e.u32(*why as u32);
            }
            ImplOutlivesRequirement::Implied { key, .. } => {
                e.u32(2);
                e.record(key);
            }
            ImplOutlivesRequirement::Language { key, pending } => {
                e.u32(3);
                e.record(key);
                e.u32(*pending as u32);
            }
        });
    }
}

#[derive(Debug)]
pub(crate) struct CurrentSourceImplOutlives {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceImplOutlives {
    pub(crate) fn capture(db: &mut Database, key: &SourceImplOutlivesKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceImplOutlives(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(), QueryValue::SourceImplOutlives(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("impl elaboration memo key differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceImplOutlives {
        let QueryValue::SourceImplOutlives(plan) = self.value.as_ref() else {
            unreachable!()
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}

/// Source-aware observation of the actual production elaborator. This control
/// exports no arena handle and cannot supply alternate query inputs or goals.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ImplElaborationObservation {
    pub(crate) implementation: Digest,
    pub(crate) trait_definition: Digest,
    pub(crate) source: Digest,
    pub(crate) implied_traits: usize,
    pub(crate) outlives: usize,
    pub(crate) static_type_parameters: Vec<crate::traits::term::ParamKey>,
}

#[cfg(test)]
pub(crate) fn check_actual_impl_elaboration(
    db: &mut Database,
    key: &SourceImplOutlivesKey,
) -> Result<ImplElaborationObservation, Diagnostic> {
    let current = match CurrentSourceImplOutlives::capture(db, key) {
        QueryPoll::Ready(Ok(current)) => current,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => {
            return Err(fault("actual impl elaboration requires source input"));
        }
    };
    current.require_current(db)?;
    let plan = current.plan();
    let ImplOutlivesDomain::TraitImpl {
        implementation,
        trait_definition,
    } = plan.domain()
    else {
        return Err(fault("actual source is not a prepared trait impl"));
    };
    let layouts = plan.generic_layouts()?;
    let configuration = plan.configuration().map_err(|why| {
        fault(&format!(
            "actual impl elaboration configuration pending: {why:?}"
        ))
    })?;
    let before = db.trace.events.len();
    let observed = configuration.with_source_context(
        db,
        Arc::clone(plan.catalog_input()),
        Some(&layouts),
        |cx, provider| {
            let actual = provider.prepare_impl_super_outlives(cx, plan)?;
            assert_eq!(actual.implementation.source(), implementation);
            assert_eq!(actual.source, plan.fingerprint());
            let clauses = actual.clauses.skip_binder();
            let mut static_type_parameters = Vec::new();
            for clause in clauses.items() {
                match clause.kind().skip_binder() {
                    ty::ClauseKind::TypeOutlives(ty::OutlivesClause(subject, region)) => {
                        if let ty::TyKind::Param(parameter) = subject.kind()
                            && matches!(region.kind(), ty::RegionKind::ReStatic)
                        {
                            static_type_parameters.push(parameter.source());
                        }
                    }
                    ty::ClauseKind::RegionOutlives(_) => {}
                    other => panic!("pinned impl query emitted unfiltered clause: {other:?}"),
                }
            }
            Ok(ImplElaborationObservation {
                implementation,
                trait_definition,
                source: actual.source,
                implied_traits: plan.trait_plans().count(),
                outlives: clauses.items().len(),
                static_type_parameters,
            })
        },
    )??;
    assert_eq!(
        db.trace.events.len(),
        before,
        "compiler elaboration must not query source"
    );
    current.require_current(db)?;
    Ok(observed)
}
