//! Coordinator ownership of immutable pattern graphs. Addresses are private
//! allocation locators; they never grant pattern authority or enter stable keys.
//! Retirement leases release only between runs, so invalidation cannot move the
//! last graph destructor into a reached PatternFinish operation.
use super::{Database, QueryPin, account_add, fault};
use crate::{
    core::diagnostic::Diagnostic,
    db::value::QueryValue,
    engine::telemetry::CacheUsage,
    syntax::{body_index::BodyIndex, pattern_product::PatternProductPlan},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const MAX_PRODUCTS: usize = 100_000;
const MAX_REGISTRY_BYTES: usize = 32 * 1024 * 1024;
const MAX_RETAINED_PRODUCT_BYTES: usize = 128 * 1024 * 1024;
// Includes a conservative BTree node allowance and the retirement lease.
const ROW_BYTES: usize = 64 + std::mem::size_of::<(usize, Arc<PatternProductPlan>)>();
const ARC_HEADER: usize = 2 * std::mem::size_of::<usize>();

#[derive(Default)]
pub(super) struct ProductPins {
    roots: BTreeMap<usize, Arc<PatternProductPlan>>,
    retained_bytes: usize,
}
impl ProductPins {
    pub(super) fn register(&mut self, value: &QueryValue) -> Result<(), Diagnostic> {
        // Preflight all new rows before publishing any. Repeated handles within
        // one recipe do not allocate repeated registry rows.
        let mut new = BTreeSet::new();
        let mut additional = 0usize;
        for product in value.product_roots() {
            let allocation = Arc::as_ptr(product) as usize;
            if !self.roots.contains_key(&allocation) {
                if new.insert(allocation) {
                    additional = account_add(
                        additional,
                        account_add(ROW_BYTES + ARC_HEADER, product.metadata_bytes())?,
                    )?;
                    if account_add(self.retained_bytes, additional)? > MAX_RETAINED_PRODUCT_BYTES {
                        return Err(limit());
                    }
                }
                let count = self.roots.len().checked_add(new.len()).ok_or_else(limit)?;
                if count > MAX_PRODUCTS
                    || count
                        .checked_mul(ROW_BYTES)
                        .is_none_or(|n| n > MAX_REGISTRY_BYTES)
                {
                    return Err(limit());
                }
            }
        }
        let retained_bytes = account_add(self.retained_bytes, additional)?;
        for product in value.product_roots() {
            self.roots
                .entry(Arc::as_ptr(product) as usize)
                .or_insert_with(|| Arc::clone(product));
        }
        self.retained_bytes = retained_bytes;
        Ok(())
    }
    pub(super) fn retain_external(
        &mut self,
        owners: &BTreeMap<usize, usize>,
    ) -> Result<(), Diagnostic> {
        self.roots.retain(|allocation, product| {
            Arc::strong_count(product) > owners.get(allocation).copied().unwrap_or(0)
        });
        self.retained_bytes = self.roots.values().try_fold(0, |bytes, product| {
            account_add(
                bytes,
                account_add(ROW_BYTES + ARC_HEADER, product.metadata_bytes())?,
            )
        })?;
        Ok(())
    }
    fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.roots.len().checked_mul(ROW_BYTES).ok_or_else(limit)
    }
    pub(super) fn index_roots(&self) -> impl Iterator<Item = &Arc<BodyIndex>> {
        self.roots.values().map(|product| product.index())
    }
    pub(super) fn count_owners(
        &self,
        inner: &mut BTreeMap<usize, usize>,
        allocations: &mut BTreeSet<usize>,
    ) {
        for product in self.roots.values() {
            count_owner(product, inner, allocations);
        }
    }
}
fn limit() -> Diagnostic {
    fault(
        "LRR-E-METADATA-LIMIT",
        "retained pattern product registry limit",
    )
}

/// Count handles, not semantic identities. Each actual Product allocation owns
/// one index Arc even when many recipes/source plans share that Product.
pub(super) fn count_owner(
    product: &Arc<PatternProductPlan>,
    inner: &mut BTreeMap<usize, usize>,
    allocations: &mut BTreeSet<usize>,
) {
    let allocation = Arc::as_ptr(product) as usize;
    *inner.entry(allocation).or_insert(0) += 1;
    if allocations.insert(allocation) {
        *inner
            .entry(Arc::as_ptr(product.index()) as usize)
            .or_insert(0) += 1;
    }
}

fn account_product(
    product: &Arc<PatternProductPlan>,
    pinned: bool,
    usage: &mut CacheUsage,
    bodies: &mut BTreeSet<*const BodyIndex>,
) -> Result<(), Diagnostic> {
    let bytes = account_add(product.metadata_bytes(), ARC_HEADER)?;
    usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
    if pinned {
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
    }
    let index = product.index();
    if bodies.insert(Arc::as_ptr(index)) {
        let bytes = account_add(index.estimated_bytes(), ARC_HEADER)?;
        usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
        if pinned {
            usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        }
    }
    Ok(())
}
impl Database {
    pub(super) fn account_products(
        &self,
        usage: &mut CacheUsage,
        outer: &BTreeMap<usize, usize>,
        inner: &BTreeMap<usize, usize>,
        bodies: &mut BTreeSet<*const BodyIndex>,
    ) -> Result<(), Diagnostic> {
        // Borrow cached handles; cloning them before the strong-count checks
        // would make the accounting traversal itself look like an external user.
        let mut cached: BTreeMap<usize, (&Arc<PatternProductPlan>, bool)> = BTreeMap::new();
        for memo in self.memos.values() {
            let external = QueryPin::new(&memo.value).external(outer, inner);
            for product in memo.value.product_roots() {
                cached
                    .entry(Arc::as_ptr(product) as usize)
                    .and_modify(|(_, pinned)| *pinned |= external)
                    .or_insert((product, external));
            }
        }
        for (product, pinned) in cached.values() {
            account_product(product, *pinned, usage, bodies)?;
        }
        // This also finds Products retained after ALL outer query values and
        // SourcePlans retire. The Product itself retains its actual index.
        for (allocation, product) in &self.product_pins.roots {
            if cached.contains_key(allocation) {
                continue;
            }
            account_product(product, true, usage, bodies)?;
        }
        let bytes = self.product_pins.metadata_bytes()?;
        usage.memo_bytes = account_add(usage.memo_bytes, bytes)?;
        // This registry can only be pruned by the between-run coordinator.
        usage.pinned_memo_bytes = account_add(usage.pinned_memo_bytes, bytes)?;
        Ok(())
    }
}
