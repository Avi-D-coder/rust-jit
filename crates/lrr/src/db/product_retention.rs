//! Coordinator ownership of immutable source artifacts. Addresses are private
//! allocation locators; they never grant pattern authority or enter stable keys.
//! Retirement leases release only between runs, so invalidation cannot move the
//! last graph destructor into a reached PatternFinish operation.
use super::{Database, QueryPin, account_add, fault};
use crate::{
    core::diagnostic::Diagnostic,
    db::value::{QueryValue, SourceArtifactRef},
    engine::telemetry::CacheUsage,
    syntax::{body_index::BodyIndex, pattern_product::PatternProductPlan},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const MAX_ARTIFACTS: usize = 100_000;
const MAX_REGISTRY_BYTES: usize = 32 * 1024 * 1024;
const MAX_RETAINED_ARTIFACT_BYTES: usize = 128 * 1024 * 1024;
// Includes a conservative BTree node allowance and the retirement lease.
const ROW_BYTES: usize = 64 + std::mem::size_of::<(usize, RetainedSourceArtifact)>();
const ARC_HEADER: usize = 2 * std::mem::size_of::<usize>();

enum RetainedSourceArtifact {
    LocalPlan(Arc<crate::frontend::source_locals::PreparedSourceLocalPlan>),
    BodyEnvironment(Arc<super::super::BodyEnvironmentInput>),
    CompletedEnvironment(Arc<crate::expand::source_discovery::CompletedSourceEnvironment>),
    Pattern(Arc<PatternProductPlan>),
    StringLiteral(Arc<crate::syntax::string_literal::PreparedStringLiteral>),
}
impl RetainedSourceArtifact {
    fn retain(root: SourceArtifactRef<'_>) -> Self {
        match root {
            SourceArtifactRef::LocalPlan(value) => Self::LocalPlan(Arc::clone(value)),
            SourceArtifactRef::BodyEnvironment(value) => Self::BodyEnvironment(Arc::clone(value)),
            SourceArtifactRef::CompletedEnvironment(value) => {
                Self::CompletedEnvironment(Arc::clone(value))
            }
            SourceArtifactRef::Pattern(value) => Self::Pattern(Arc::clone(value)),
            SourceArtifactRef::StringLiteral(value) => Self::StringLiteral(Arc::clone(value)),
        }
    }
    fn root(&self) -> SourceArtifactRef<'_> {
        match self {
            Self::LocalPlan(value) => SourceArtifactRef::LocalPlan(value),
            Self::BodyEnvironment(value) => SourceArtifactRef::BodyEnvironment(value),
            Self::CompletedEnvironment(value) => SourceArtifactRef::CompletedEnvironment(value),
            Self::Pattern(value) => SourceArtifactRef::Pattern(value),
            Self::StringLiteral(value) => SourceArtifactRef::StringLiteral(value),
        }
    }
}

#[derive(Default)]
pub(super) struct SourcePlanPins {
    roots: BTreeMap<usize, RetainedSourceArtifact>,
    retained_bytes: usize,
    #[cfg(test)]
    artifact_count_limit: Option<usize>,
}
impl SourcePlanPins {
    #[cfg(test)]
    pub(super) fn set_test_artifact_limit(&mut self, maximum: usize) {
        self.artifact_count_limit = Some(maximum.min(MAX_ARTIFACTS));
    }
    #[cfg(test)]
    pub(super) fn registered_artifact_count(&self) -> usize {
        self.roots.len()
    }
    pub(super) fn register(&mut self, value: &QueryValue) -> Result<(), Diagnostic> {
        let completed = match value {
            QueryValue::SourceLocalPlan(plan) => plan.environment().map(|environment| {
                SourceArtifactRef::CompletedEnvironment(environment.macros().source_environment())
            }),
            QueryValue::EnteredCall(value) => {
                value.retained_plan().environment().map(|environment| {
                    SourceArtifactRef::CompletedEnvironment(
                        environment.macros().source_environment(),
                    )
                })
            }
            QueryValue::EnteredType(value) => {
                value.retained_plan().environment().map(|environment| {
                    SourceArtifactRef::CompletedEnvironment(
                        environment.macros().source_environment(),
                    )
                })
            }
            QueryValue::PreparedTypePlan(value) => {
                value.retained_plan().environment().map(|environment| {
                    SourceArtifactRef::CompletedEnvironment(
                        environment.macros().source_environment(),
                    )
                })
            }
            QueryValue::EnteredValue(value) => {
                value.retained_plan().environment().map(|environment| {
                    SourceArtifactRef::CompletedEnvironment(
                        environment.macros().source_environment(),
                    )
                })
            }
            QueryValue::QualifiedBodyType(value) => {
                value.retained_plan().environment().map(|environment| {
                    SourceArtifactRef::CompletedEnvironment(
                        environment.macros().source_environment(),
                    )
                })
            }
            _ => None,
        };
        self.register_roots(|| value.source_roots().chain(completed))
    }
    pub(super) fn roots(&self) -> impl Iterator<Item = SourceArtifactRef<'_>> {
        self.roots.values().map(RetainedSourceArtifact::root)
    }
    pub(super) fn input_extra_bytes(
        &self,
        input: &Arc<super::super::BodyEnvironmentInput>,
    ) -> Result<usize, Diagnostic> {
        account_add(
            self.extra_bytes(SourceArtifactRef::BodyEnvironment(input))?,
            self.extra_bytes(SourceArtifactRef::CompletedEnvironment(
                input.macros().source_environment(),
            ))?,
        )
    }
    pub(super) fn extra_bytes(&self, root: SourceArtifactRef<'_>) -> Result<usize, Diagnostic> {
        if self.roots.contains_key(&root.allocation()) {
            Ok(0)
        } else {
            account_add(ROW_BYTES, root.metadata_bytes()?)
        }
    }
    pub(super) fn register_input(
        &mut self,
        input: &Arc<super::super::BodyEnvironmentInput>,
    ) -> Result<(), Diagnostic> {
        self.register_roots(|| {
            [
                SourceArtifactRef::BodyEnvironment(input),
                SourceArtifactRef::CompletedEnvironment(input.macros().source_environment()),
            ]
            .into_iter()
        })
    }
    pub(super) fn register_completed(
        &mut self,
        input: &Arc<crate::expand::source_discovery::CompletedSourceEnvironment>,
    ) -> Result<(), Diagnostic> {
        self.register_roots(|| std::iter::once(SourceArtifactRef::CompletedEnvironment(input)))
    }
    fn register_roots<'a, I: Iterator<Item = SourceArtifactRef<'a>>>(
        &mut self,
        roots: impl Fn() -> I,
    ) -> Result<(), Diagnostic> {
        let maximum = MAX_ARTIFACTS;
        #[cfg(test)]
        let maximum = self.artifact_count_limit.unwrap_or(maximum);
        // Preflight all new rows before publishing any. Repeated handles within
        // one recipe do not allocate repeated registry rows.
        let mut new = BTreeSet::new();
        let mut additional = 0usize;
        for root in roots() {
            let allocation = root.allocation();
            if !self.roots.contains_key(&allocation) {
                if new.insert(allocation) {
                    additional =
                        account_add(additional, account_add(ROW_BYTES, root.admission_bytes()?)?)?;
                    if account_add(self.retained_bytes, additional)? > MAX_RETAINED_ARTIFACT_BYTES {
                        return Err(limit());
                    }
                }
                let count = self.roots.len().checked_add(new.len()).ok_or_else(limit)?;
                if count > maximum
                    || count
                        .checked_mul(ROW_BYTES)
                        .is_none_or(|n| n > MAX_REGISTRY_BYTES)
                {
                    return Err(limit());
                }
            }
        }
        let retained_bytes = account_add(self.retained_bytes, additional)?;
        for root in roots() {
            self.roots
                .entry(root.allocation())
                .or_insert_with(|| RetainedSourceArtifact::retain(root));
        }
        self.retained_bytes = retained_bytes;
        Ok(())
    }
    pub(super) fn retain_external(
        &mut self,
        owners: &BTreeMap<usize, usize>,
        external_parents: &BTreeSet<usize>,
    ) -> Result<(), Diagnostic> {
        self.roots.retain(|allocation, artifact| {
            external_parents.contains(allocation) || has_external_owners(artifact.root(), owners)
        });
        self.retained_bytes = self.roots.values().try_fold(0, |bytes, artifact| {
            account_add(
                bytes,
                account_add(ROW_BYTES, artifact.root().admission_bytes()?)?,
            )
        })?;
        Ok(())
    }
    fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.roots.len().checked_mul(ROW_BYTES).ok_or_else(limit)
    }
    pub(super) fn index_roots(&self) -> impl Iterator<Item = &Arc<BodyIndex>> {
        self.roots
            .values()
            .filter_map(|artifact| artifact.root().index())
    }
    pub(super) fn count_owners(
        &self,
        inner: &mut BTreeMap<usize, usize>,
        allocations: &mut BTreeSet<usize>,
    ) {
        for artifact in self.roots.values() {
            count_owner(artifact.root(), inner, allocations);
        }
    }
}
fn limit() -> Diagnostic {
    fault(
        "LRR-E-METADATA-LIMIT",
        "retained source artifact registry limit",
    )
}

/// Count handles, not semantic identities. Each actual Product allocation owns
/// one index Arc even when many recipes/source plans share that Product.
pub(super) fn count_owner(
    root: SourceArtifactRef<'_>,
    inner: &mut BTreeMap<usize, usize>,
    allocations: &mut BTreeSet<usize>,
) {
    let allocation = root.allocation();
    *inner.entry(allocation).or_insert(0) += 1;
    if allocations.insert(allocation) {
        if let Some(index) = root.index() {
            *inner.entry(Arc::as_ptr(index) as usize).or_insert(0) += 1;
        }
        match root {
            SourceArtifactRef::LocalPlan(plan) => {
                for context in plan.context_roots() {
                    let id = Arc::as_ptr(context) as usize;
                    *inner.entry(id).or_insert(0) += 1;
                    if allocations.insert(id) {
                        count_admitted_source(context.source(), inner, allocations);
                    }
                }
                if let Some(environment) = plan.environment() {
                    count_owner(
                        SourceArtifactRef::BodyEnvironment(environment),
                        inner,
                        allocations,
                    );
                }
            }
            SourceArtifactRef::BodyEnvironment(environment) => {
                count_admitted_source(environment.source(), inner, allocations);
                // Each selected macro receipt owns exactly one completed root;
                // multiple BodyEnvironment handles do not clone that handle.
                let macros = environment.macros();
                let allocation = Arc::as_ptr(macros) as usize;
                *inner.entry(allocation).or_insert(0) += 1;
                if allocations.insert(allocation) {
                    count_owner(
                        SourceArtifactRef::CompletedEnvironment(macros.source_environment()),
                        inner,
                        allocations,
                    );
                }
            }
            SourceArtifactRef::CompletedEnvironment(environment) => {
                *inner
                    .entry(Arc::as_ptr(environment.names_root()) as usize)
                    .or_insert(0) += 1;
                *inner
                    .entry(Arc::as_ptr(environment.sources_root()) as usize)
                    .or_insert(0) += 1;
            }
            _ => {}
        }
    }
}

/// The parent Arc is physically owned once by this admitted source allocation,
/// even when both its prepared context and environment refer to that source.
fn count_admitted_source(
    source: &Arc<crate::frontend::source_body_context::AdmittedBodyContext>,
    inner: &mut BTreeMap<usize, usize>,
    allocations: &mut BTreeSet<usize>,
) {
    let allocation = Arc::as_ptr(source) as usize;
    *inner.entry(allocation).or_insert(0) += 1;
    if allocations.insert(allocation) {
        if let Some(parent) = source.local_parent() {
            count_owner(SourceArtifactRef::LocalPlan(parent), inner, allocations);
        }
    }
}

/// External extraction of an opaque context/source handle must retain its
/// existing retirement wrapper, even after every outer query/input is gone.
pub(super) fn has_external_owners(
    root: SourceArtifactRef<'_>,
    owners: &BTreeMap<usize, usize>,
) -> bool {
    let external = |id, strong| strong > owners.get(&id).copied().unwrap_or(0);
    if external(root.allocation(), root.strong_count()) {
        return true;
    }
    match root {
        SourceArtifactRef::LocalPlan(plan) => plan.context_roots().any(|context| {
            external(Arc::as_ptr(context) as usize, Arc::strong_count(context))
                || external(
                    Arc::as_ptr(context.source()) as usize,
                    Arc::strong_count(context.source()),
                )
        }),
        SourceArtifactRef::BodyEnvironment(environment) => {
            external(
                Arc::as_ptr(environment.source()) as usize,
                Arc::strong_count(environment.source()),
            ) || external(
                Arc::as_ptr(environment.macros()) as usize,
                Arc::strong_count(environment.macros()),
            )
        }
        SourceArtifactRef::CompletedEnvironment(environment) => {
            external(
                Arc::as_ptr(environment.names_root()) as usize,
                Arc::strong_count(environment.names_root()),
            ) || external(
                Arc::as_ptr(environment.sources_root()) as usize,
                Arc::strong_count(environment.sources_root()),
            )
        }
        _ => false,
    }
}

fn account_artifact(
    root: SourceArtifactRef<'_>,
    pinned: bool,
    usage: &mut CacheUsage,
    bodies: &mut BTreeSet<*const BodyIndex>,
) -> Result<(), Diagnostic> {
    let bytes = root.metadata_bytes()?;
    usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
    if pinned {
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
    }
    if let Some(index) = root.index()
        && bodies.insert(Arc::as_ptr(index))
    {
        let bytes = account_add(index.estimated_bytes(), ARC_HEADER)?;
        usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
        if pinned {
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
    }
    Ok(())
}
impl Database {
    pub(crate) fn retain_completed_source_environment(
        &mut self,
        environment: &Arc<crate::expand::source_discovery::CompletedSourceEnvironment>,
    ) -> Result<(), Diagnostic> {
        self.require_source_preparation()?;
        let row = self
            .source_plan_pins
            .extra_bytes(SourceArtifactRef::CompletedEnvironment(environment))?;
        if row != 0 {
            // Existing roots may share allocations; this standalone coordinator
            // admission intentionally overcharges rather than assuming sharing.
            let additional = account_add(row, environment.retained_metadata_bytes()?)?;
            let usage = self.cache_usage()?;
            if account_add(usage.memo_bytes, additional)?
                > self.run_budgets.unwrap_or_default().query_bytes
            {
                return Err(limit());
            }
        }
        self.source_plan_pins.register_completed(environment)
    }
    pub(super) fn account_source_artifacts(
        &self,
        usage: &mut CacheUsage,
        outer: &BTreeMap<usize, usize>,
        inner: &BTreeMap<usize, usize>,
        bodies: &mut BTreeSet<*const BodyIndex>,
    ) -> Result<(), Diagnostic> {
        // Borrow cached handles; cloning them before the strong-count checks
        // would make the accounting traversal itself look like an external user.
        let mut cached: BTreeMap<usize, (SourceArtifactRef<'_>, bool)> = BTreeMap::new();
        for memo in self.memos.values() {
            let external = QueryPin::new(&memo.value).external(outer, inner);
            for root in memo.value.source_roots() {
                cached
                    .entry(root.allocation())
                    .and_modify(|(_, pinned)| *pinned |= external)
                    .or_insert((root, external));
            }
        }
        for (root, pinned) in cached.values() {
            account_artifact(*root, *pinned, usage, bodies)?;
        }
        // This also finds Products retained after ALL outer query values and
        // SourcePlans retire. The Product itself retains its actual index.
        for (allocation, artifact) in &self.source_plan_pins.roots {
            if cached.contains_key(allocation) {
                continue;
            }
            account_artifact(artifact.root(), true, usage, bodies)?;
        }
        let bytes = self.source_plan_pins.metadata_bytes()?;
        usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
        // This registry can only be pruned by the between-run coordinator.
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        Ok(())
    }
}
