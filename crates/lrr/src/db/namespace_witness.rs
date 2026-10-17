//! Process-local currentness for recipes whose source meaning used namespace
//! absence. This sidecar is not a stable recipe or a namespace admission proof.
use super::{Database, account_add, fault};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    db::dependency::InputKey,
};
use std::collections::BTreeMap;

/// Only Database can capture this census; callers cannot supply membership.
#[derive(Debug)]
pub(crate) struct NamespaceWitness {
    stamps: std::sync::Arc<BTreeMap<InputKey, Digest>>,
    metadata_bytes: usize,
    /// The census reads only published inputs: repeated per input revision,
    /// not per dispatch.
    checked: super::CheckedRevision,
}
impl NamespaceWitness {
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        // Compare membership, not just old fingerprints: immutable derived
        // publication may add names without changing source_revision or epoch.
        // Parsing-only retained inputs and exact definition rows do not add
        // names. Their source/body consumers retain separate currentness checks.
        if self.checked.is_current(db) {
            return Ok(());
        }
        if db.lexical_namespace_stamps()? != self.stamps {
            return Err(fault(
                "LRR-E-NAMESPACE-STALE",
                "installed recipe namespace membership changed; fresh source preparation is required",
            ));
        }
        self.checked.record(db);
        Ok(())
    }
}
impl Database {
    pub(crate) fn capture_namespace_witness(&self) -> Result<NamespaceWitness, Diagnostic> {
        let stamps = self.lexical_namespace_stamps()?;
        // Same conservative BTree row allowance as the census, plus owned key
        // heaps and the Arc allocation/control block used by VerifiedRecipe.
        let mut bytes = account_add(
            std::mem::size_of::<NamespaceWitness>(),
            2 * std::mem::size_of::<usize>(),
        )?;
        for key in stamps.keys() {
            bytes = account_add(bytes, std::mem::size_of::<(InputKey, Digest)>())?;
            bytes = account_add(bytes, 128)?;
            bytes = account_add(bytes, key.heap_bytes())?;
        }
        Ok(NamespaceWitness {
            stamps,
            metadata_bytes: bytes,
            checked: Default::default(),
        })
    }
}
