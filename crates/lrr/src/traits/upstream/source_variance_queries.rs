//! Actual source domains of the pinned variance queries. This is not an
//! independent inference algorithm or permission to invent opaque/RPIT IDs.
use super::{
    identity::DefinitionId,
    model::Generics,
    parameters::{DeclarationParameters, DeclaredParameterPreparation, ParameterPending},
    prepared::{Cx, DefinitionReceipt, PreparedProvider},
    source_definition_headers::{
        DefinitionHeaderKey, DefinitionHeaderPending, DefinitionHeaderRole,
        PreparedDefinitionHeader,
    },
    variance::{self, PreparedNominalVariances, VariancePending},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
};
use rustc_type_ir::{AliasTermKind, Variance};
use std::sync::Arc;
pub type SourceVarianceQueriesKey = DefinitionHeaderKey;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum VarianceQueryDomain {
    ZeroGenerics = 0,
    NominalInference = 1,
    FunctionInference = 2,
    NotApplicable = 3,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum VarianceQueryPending {
    Header(DefinitionHeaderPending),
    Parameters(ParameterPending),
    Nominal(VariancePending),
    FunctionInference,
    UnsupportedIdentity,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum VarianceQueryOutcome<'a> {
    Known(&'a [Variance]),
    /// A variances_of call here is an upstream compiler invariant failure.
    NotApplicable,
}
pub struct PreparedSourceVarianceQueries {
    key: SourceVarianceQueriesKey,
    header: Arc<QueryValue>,
    generic_count: Option<usize>,
    domain: Option<VarianceQueryDomain>,
    nominal: Option<PreparedNominalVariances>,
    pending: Option<VarianceQueryPending>,
}
impl std::fmt::Debug for PreparedSourceVarianceQueries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSourceVarianceQueries")
            .field("key", &self.key)
            .field("generic_count", &self.generic_count)
            .field("domain", &self.domain)
            .field("pending", &self.pending)
            .finish()
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
impl PreparedSourceVarianceQueries {
    pub(crate) fn key(&self) -> &SourceVarianceQueriesKey {
        &self.key
    }
    fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(header) = self.header.as_ref() else {
            unreachable!("actual variance source header")
        };
        header
    }
    pub(crate) fn generic_count(&self) -> Option<usize> {
        self.generic_count
    }
    pub(crate) fn domain(&self) -> Option<VarianceQueryDomain> {
        self.domain
    }
    pub(crate) fn pending(&self) -> Option<VarianceQueryPending> {
        self.pending
    }
    pub(crate) fn outcome(&self) -> Result<VarianceQueryOutcome<'_>, VarianceQueryPending> {
        if let Some(pending) = self.pending {
            return Err(pending);
        }
        match self.domain {
            Some(VarianceQueryDomain::ZeroGenerics) => Ok(VarianceQueryOutcome::Known(&[])),
            Some(VarianceQueryDomain::NotApplicable) => Ok(VarianceQueryOutcome::NotApplicable),
            Some(VarianceQueryDomain::NominalInference) => self
                .nominal
                .as_ref()
                .ok_or(VarianceQueryPending::UnsupportedIdentity)?
                .variances()
                .map(VarianceQueryOutcome::Known)
                .map_err(VarianceQueryPending::Nominal),
            Some(VarianceQueryDomain::FunctionInference) => {
                Err(VarianceQueryPending::FunctionInference)
            }
            None => Err(VarianceQueryPending::UnsupportedIdentity),
        }
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceVarianceQueriesKey) -> QueryPoll<Self> {
        let header = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(key.clone()))
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            header,
            generic_count: None,
            domain: None,
            nominal: None,
            pending: None,
        };
        let parameters = match DeclarationParameters::prepare(result.header().declaration()) {
            Ok(DeclaredParameterPreparation::Ready(parameters)) => parameters,
            Ok(DeclaredParameterPreparation::Pending(pending)) => {
                result.pending = Some(VarianceQueryPending::Parameters(pending));
                return QueryPoll::Ready(Ok(result));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let count = match usize::try_from(parameters.count()) {
            Ok(count) => count,
            Err(_) => {
                return QueryPoll::Ready(Err(fault(
                    "actual variance generic count cannot be represented",
                )));
            }
        };
        result.generic_count = Some(count);
        // Exact rustc_hir_analysis::variance::variances_of ordering: the total
        // upstream count includes inherited parameters and actual implicit Self.
        // The count==0 rule precedes kind dispatch, including ordinary aliases.
        if count == 0 {
            result.domain = Some(VarianceQueryDomain::ZeroGenerics);
            return QueryPoll::Ready(Ok(result));
        }
        if let Some(pending) = result.header().pending() {
            result.pending = Some(VarianceQueryPending::Header(pending));
            return QueryPoll::Ready(Ok(result));
        }
        let domain = match result.header().role() {
            DefinitionHeaderRole::Struct
            | DefinitionHeaderRole::Enum
            | DefinitionHeaderRole::Union => VarianceQueryDomain::NominalInference,
            // Foreign functions also have rustc DefKind::Fn. Declaration
            // placement does not narrow the pinned inference query domain.
            DefinitionHeaderRole::Function => VarianceQueryDomain::FunctionInference,
            // These are real explicit source declarations. A type-alias AST
            // is neither its hidden opaque ID nor a compiler-created RPITIT.
            DefinitionHeaderRole::Trait
            | DefinitionHeaderRole::Impl
            | DefinitionHeaderRole::Constant
            | DefinitionHeaderRole::Static
            | DefinitionHeaderRole::TypeAlias => VarianceQueryDomain::NotApplicable,
            DefinitionHeaderRole::Other => {
                result.pending = Some(VarianceQueryPending::UnsupportedIdentity);
                return QueryPoll::Ready(Ok(result));
            }
        };
        result.domain = Some(domain);
        if domain == VarianceQueryDomain::NominalInference {
            let nominal = ready!(variance::prepare_nominal(db, key));
            if nominal.key() != key {
                return QueryPoll::Ready(Err(fault(
                    "actual nominal variance query identity differs",
                )));
            }
            result.pending = nominal.pending().map(VarianceQueryPending::Nominal);
            result.nominal = Some(nominal);
        } else if domain == VarianceQueryDomain::FunctionInference {
            result.pending = Some(VarianceQueryPending::FunctionInference);
        }
        QueryPoll::Ready(Ok(result))
    }

}

/// Source-only control view of a prepared plan (no solver context).
#[cfg(test)]
impl PreparedSourceVarianceQueries {
    pub(super) fn bind(&self) -> Result<BoundVariances<'_>, VarianceQueryPending> {
        Ok(BoundVariances { source: self, outcome: self.outcome()? })
    }
}
#[cfg(test)]
pub(super) struct BoundVariances<'a> {
    source: &'a PreparedSourceVarianceQueries,
    outcome: VarianceQueryOutcome<'a>,
}
#[cfg(test)]
impl<'a> BoundVariances<'a> {
    pub(super) fn variances_of(&self) -> &'a [Variance] {
        match self.outcome {
            VarianceQueryOutcome::Known(values) => values,
            VarianceQueryOutcome::NotApplicable => panic!(
                "asked to compute variance for actual source {:?}: {:?}",
                self.source.key.definition,
                self.source.header().role()
            ),
        }
    }
}

/// Published data has no Pending variant. The variance vector is retained in
/// the context's own arena, so the fact outlives no borrowed source plan.
pub(super) struct DefinitionVarianceQueries<'cx> {
    key: SourceVarianceQueriesKey,
    receipt: DefinitionReceipt,
    generic_count: Option<usize>,
    role: DefinitionHeaderRole,
    outcome: VarianceQueryOutcome<'cx>,
}
impl<'cx> DefinitionVarianceQueries<'cx> {
    /// Retain one prepared source plan's outcome for the solver context.
    pub(super) fn retain(
        cx: Cx<'cx>,
        source: &PreparedSourceVarianceQueries,
    ) -> Result<Self, VarianceQueryPending> {
        let outcome = match source.outcome()? {
            VarianceQueryOutcome::Known(values) => {
                VarianceQueryOutcome::Known(cx.retain_variances(values.to_vec()))
            }
            VarianceQueryOutcome::NotApplicable => VarianceQueryOutcome::NotApplicable,
        };
        Ok(Self {
            key: source.key.clone(),
            receipt: source.header().receipt().clone(),
            generic_count: source.generic_count,
            role: source.header().role(),
            outcome,
        })
    }
    pub(super) fn variances_of(&self) -> &'cx [Variance] {
        match self.outcome {
            VarianceQueryOutcome::Known(values) => values,
            VarianceQueryOutcome::NotApplicable => panic!(
                "asked to compute variance for actual source {:?}: {:?}",
                self.key.definition, self.role
            ),
        }
    }
    pub(super) fn require_source(
        &self,
        receipt: &DefinitionReceipt,
        generics: Generics<'cx, PreparedProvider<'cx>>,
        impl_trait_in_trait: bool,
        inventory: &crate::db::seals::SealKey,
    ) -> Result<(), Diagnostic> {
        if &self.key.inventory != inventory
            || !receipt.same_source_metadata(&self.receipt)
            || self.generic_count != Some(generics.parameters.items().len())
        {
            return Err(fault(
                "variance query source or total generic count differs from actual definition",
            ));
        }
        if let VarianceQueryOutcome::Known(values) = self.outcome
            && values.len() != generics.parameters.items().len()
        {
            return Err(fault(
                "known variance vector length differs from actual total generic count",
            ));
        }
        // No generated opaque/RPITIT definition identity producer exists in
        // this source adapter. Explicit associated aliases cannot assert it.
        if impl_trait_in_trait {
            return Err(fault(
                "actual opaque/RPITIT variance identity producer remains pending",
            ));
        }
        Ok(())
    }
}

/// Exact pinned rustc_middle::ty::util::opt_alias_variances dispatch. Both
/// callbacks read already published immutable facts; none prepare source.
pub(super) fn opt_alias_variances<'cx>(
    kind: AliasTermKind<Cx<'cx>>,
    mut is_impl_trait_in_trait: impl FnMut(DefinitionId) -> bool,
    mut variances_of: impl FnMut(DefinitionId) -> &'cx [Variance],
) -> Option<&'cx [Variance]> {
    match kind {
        AliasTermKind::ProjectionTy { def_id } => {
            is_impl_trait_in_trait(def_id).then(|| variances_of(def_id))
        }
        AliasTermKind::OpaqueTy { def_id } => Some(variances_of(def_id)),
        AliasTermKind::InherentTy { .. }
        | AliasTermKind::InherentConstSelf { .. }
        | AliasTermKind::InherentConstImpl { .. }
        | AliasTermKind::FreeTy { .. }
        | AliasTermKind::FreeConst { .. }
        | AliasTermKind::AnonConst { .. }
        | AliasTermKind::ProjectionConst { .. } => None,
    }
}

/// Actual source metadata control: no complete provider or synthetic alias is
/// constructed. Only a real trait-associated type declaration exercises the
/// ProjectionTy branch. Opaque/RPITIT branches remain unexecuted.
#[cfg(test)]
pub(crate) fn check_actual_variance_query(
    db: &mut Database,
    key: &SourceVarianceQueriesKey,
    expected_count: usize,
    expected_domain: VarianceQueryDomain,
    actual_trait_associated_type: bool,
) -> Result<CurrentSourceVarianceQueries, Diagnostic> {
    use super::source_definition_headers::SourceHeaderParent;
    use crate::db::dependency::{InputKey, InputValue};
    let current = match CurrentSourceVarianceQueries::capture(db, key) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual variance source control needs input")),
    };
    current.require_current(db)?;
    let plan = current.plan();
    assert_eq!(plan.generic_count(), Some(expected_count));
    assert_eq!(plan.domain(), Some(expected_domain));
    match expected_domain {
        VarianceQueryDomain::ZeroGenerics => assert_eq!(plan.bind().unwrap().variances_of(), &[]),
        VarianceQueryDomain::NominalInference => {
            let values = plan.bind().unwrap_or_else(|why| {
                panic!("actual nominal variance preparation pending: {why:?}")
            });
            assert_eq!(values.variances_of().len(), expected_count);
        }
        VarianceQueryDomain::FunctionInference => {
            assert_eq!(
                plan.pending(),
                Some(VarianceQueryPending::FunctionInference)
            );
            assert!(plan.bind().is_err());
        }
        VarianceQueryDomain::NotApplicable => {
            let values = plan.bind().unwrap();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| values.variances_of()))
                    .is_err(),
                "invalid upstream variances_of must retain its compiler invariant failure"
            );
        }
    }
    if actual_trait_associated_type {
        assert_eq!(plan.header().role(), DefinitionHeaderRole::TypeAlias);
        let SourceHeaderParent::Associated(parent) = plan.header().source_parent() else {
            panic!("actual alias has no source parent")
        };
        let parent = match db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(DefinitionHeaderKey {
                inventory: key.inventory.clone(),
                definition: *parent,
            })),
        ) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => {
                return Err(fault("actual associated parent control needs input"));
            }
        };
        let QueryValue::DefinitionHeaderPlan(parent) = parent.as_ref() else {
            unreachable!()
        };
        assert_eq!(parent.role(), DefinitionHeaderRole::Trait);
        let input = db
            .input(&InputKey::DiscoverySeal(key.inventory.clone()))
            .ok_or_else(|| fault("actual variance catalog absent"))?;
        let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
            return Err(fault("actual variance catalog differs"));
        };
        let world = super::prepared::catalog_world(catalog)?;
        let local = catalog
            .impl_catalog
            .as_ref()
            .ok_or_else(|| fault("actual variance catalog root absent"))?
            .root()
            .outline
            .crate_key;
        let id = plan.header().receipt().definition_id(world, local);
        assert_eq!(id.source(), key.definition);
        assert_eq!(
            opt_alias_variances(
                AliasTermKind::ProjectionTy { def_id: id },
                |observed| {
                    assert_eq!(observed, id);
                    false // actual explicit associated declaration, never synthetic RPITIT
                },
                |_| panic!("ordinary associated alias must not query variance")
            ),
            None
        );
    }
    Ok(current)
}

#[derive(Debug)]
pub(crate) struct CurrentSourceVarianceQueries {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceVarianceQueries {
    pub(crate) fn capture(db: &mut Database, key: &SourceVarianceQueriesKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceVarianceQueries(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(), QueryValue::SourceVarianceQueries(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("source variance query memo differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceVarianceQueries {
        let QueryValue::SourceVarianceQueries(plan) = self.value.as_ref() else {
            unreachable!("captured variance query")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-VARIANCE-QUERY", message.into())
}
impl StableEncode for PreparedSourceVarianceQueries {
    const DOMAIN: &'static str = "lrr/v1/source-variance-query-domain";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.header());
        e.option(&self.generic_count, |e, count| {
            e.u64(u64::try_from(*count).expect("actual compiler generic count"))
        });
        e.option(&self.domain, |e, domain| e.u32(*domain as u32));
        e.option(&self.nominal, |e, nominal| e.record(nominal));
        e.option(&self.pending, |e, pending| match pending {
            VarianceQueryPending::Header(why) => {
                e.u32(0);
                match why {
                    DefinitionHeaderPending::ParentJoin => e.u32(0),
                    DefinitionHeaderPending::ImplMembership => e.u32(1),
                    DefinitionHeaderPending::Source(why) => {
                        e.u32(2);
                        e.u32(*why as u32);
                    }
                    DefinitionHeaderPending::UnsupportedRole => e.u32(3),
                }
            }
            VarianceQueryPending::Parameters(ParameterPending::SourceLayout) => {
                e.u32(1);
                e.u32(0);
            }
            VarianceQueryPending::Parameters(ParameterPending::FunctionRegionBinders) => {
                e.u32(1);
                e.u32(1);
            }
            VarianceQueryPending::Parameters(ParameterPending::AnonymousFunctionParameters) => {
                e.u32(1);
                e.u32(2);
            }
            VarianceQueryPending::Parameters(ParameterPending::FunctionParameterMacros) => {
                e.u32(1);
                e.u32(3);
            }
            VarianceQueryPending::Parameters(ParameterPending::ImplRegionBinders) => {
                e.u32(1);
                e.u32(4);
            }
            VarianceQueryPending::Nominal(why) => {
                e.u32(2);
                e.record(why);
            }
            VarianceQueryPending::FunctionInference => e.u32(3),
            VarianceQueryPending::UnsupportedIdentity => e.u32(4),
        });
    }
}
