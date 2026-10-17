//! Stage immutable adopted-module leases without replacing the current world.
use super::{Database, Diagnostic, fault};
use crate::source::{
    changes::PreparedSourceState, module_syntax::ModuleSyntaxInput,
    ordinary_module::OrdinaryModuleSyntaxInput,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Weak},
};
impl Database {
    /// Called only after exact module adoption and derived-input validation.
    /// The returned registry is charged with the complete current cache before
    /// either it or the matching discovery inputs become visible.
    pub(super) fn module_prepared_registry(
        &self,
        modules: &[Arc<ModuleSyntaxInput>],
        ordinary: &[Arc<OrdinaryModuleSyntaxInput>],
    ) -> Result<Vec<Weak<PreparedSourceState>>, Diagnostic> {
        let limit = || {
            fault(
                "LRR-E-METADATA-LIMIT",
                "discovery prepared source registry limit",
            )
        };
        let count = self
            .prepared_sources
            .len()
            .checked_add(modules.len())
            .and_then(|n| n.checked_add(ordinary.len()))
            .ok_or_else(limit)?;
        // Bound both the eventual vector and the transient identity set before
        // cloning any weak handle. Pointer identity deduplicates allocations;
        // it never enters a source key, diagnostic identity or stable encoding.
        count.checked_mul(128).ok_or_else(limit)?;
        let mut staged = Vec::new();
        staged.try_reserve_exact(count).map_err(|_| limit())?;
        let mut seen = BTreeSet::new();
        for weak in self
            .prepared_sources
            .iter()
            .cloned()
            .chain(
                modules
                    .iter()
                    .map(|module| module.prepared_source().retained_state_weak()),
            )
            .chain(
                ordinary
                    .iter()
                    .map(|module| module.prepared_source().retained_state_weak()),
            )
        {
            if weak.strong_count() != 0 && seen.insert(weak.as_ptr()) {
                staged.push(weak);
            }
        }
        if staged.len() == count {
            return Ok(staged);
        }
        // Reserve for the actual unique owners, not the number of repeated
        // publication arguments. An identical seal must not grow registry
        // capacity merely by supplying the same module lease a second time.
        let mut exact = Vec::new();
        exact.try_reserve_exact(staged.len()).map_err(|_| limit())?;
        exact.extend(staged);
        Ok(exact)
    }
}
