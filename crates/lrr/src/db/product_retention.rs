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
    Pattern(Arc<PatternProductPlan>),
    StringLiteral(Arc<crate::syntax::string_literal::PreparedStringLiteral>),
}
impl RetainedSourceArtifact {
    fn retain(root: SourceArtifactRef<'_>) -> Self {
        match root {
            SourceArtifactRef::LocalPlan(value) => Self::LocalPlan(Arc::clone(value)),
            SourceArtifactRef::BodyEnvironment(value) => Self::BodyEnvironment(Arc::clone(value)),
            SourceArtifactRef::Pattern(value) => Self::Pattern(Arc::clone(value)),
            SourceArtifactRef::StringLiteral(value) => Self::StringLiteral(Arc::clone(value)),
        }
    }
    fn root(&self) -> SourceArtifactRef<'_> {
        match self {
            Self::LocalPlan(value) => SourceArtifactRef::LocalPlan(value),
            Self::BodyEnvironment(value) => SourceArtifactRef::BodyEnvironment(value),
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
        self.register_roots(|| value.source_roots())
    }
    pub(super) fn roots(&self) -> impl Iterator<Item = SourceArtifactRef<'_>> {
        self.roots.values().map(RetainedSourceArtifact::root)
    }
    pub(super) fn input_extra_bytes(
        &self,
        input: &Arc<super::super::BodyEnvironmentInput>,
    ) -> Result<usize, Diagnostic> {
        self.extra_bytes(SourceArtifactRef::BodyEnvironment(input))
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
        self.register_roots(|| std::iter::once(SourceArtifactRef::BodyEnvironment(input)))
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
                        account_add(additional, account_add(ROW_BYTES, root.metadata_bytes()?)?)?;
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
            external_parents.contains(allocation)
                || artifact.root().strong_count() > owners.get(allocation).copied().unwrap_or(0)
        });
        self.retained_bytes = self.roots.values().try_fold(0, |bytes, artifact| {
            account_add(
                bytes,
                account_add(ROW_BYTES, artifact.root().metadata_bytes()?)?,
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
        if let SourceArtifactRef::LocalPlan(plan) = root {
            if let Some(environment) = plan.environment() {
                *inner.entry(Arc::as_ptr(environment) as usize).or_insert(0) += 1;
            }
        }
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
