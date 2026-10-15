//! Test-only owned state facts. No coordinator/host effects or mutation API.
use super::{Database, Diagnostic, Digest, InputKey, fault};

/// Opaque equality witness for publication atomicity. Its fields never confer
/// input/source authority and expose neither registry mutation nor guest state.
#[derive(Debug, PartialEq)]
pub(crate) struct PreparedModuleRegistrySnapshot {
    inputs: Vec<(InputKey, Digest)>,
    revision: u64,
    source_revision: u64,
    registry: Vec<usize>,
    registry_capacity: usize,
    usage: (usize, usize, usize, usize, usize, usize),
}
impl Database {
    pub(crate) fn prepared_module_registry_snapshot(
        &self,
    ) -> Result<PreparedModuleRegistrySnapshot, Diagnostic> {
        let allocation = || {
            fault(
                "LRR-E-METADATA-LIMIT",
                "test publication snapshot allocation",
            )
        };
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(self.inputs.len())
            .map_err(|_| allocation())?;
        inputs.extend(
            self.inputs
                .iter()
                .map(|(key, value)| (key.clone(), value.fingerprint)),
        );
        let mut registry = Vec::new();
        registry
            .try_reserve_exact(self.prepared_sources.len())
            .map_err(|_| allocation())?;
        // Ephemeral allocation identities only, private and never serialized.
        registry.extend(
            self.prepared_sources
                .iter()
                .map(|weak| weak.as_ptr() as usize),
        );
        let usage = self.cache_usage()?;
        Ok(PreparedModuleRegistrySnapshot {
            inputs,
            revision: self.revision(),
            source_revision: self.source_revision(),
            registry,
            registry_capacity: self.prepared_sources.capacity(),
            usage: (
                usage.memo_entries,
                usage.memo_bytes,
                usage.parse_bytes,
                usage.pinned_memo_bytes,
                usage.pinned_parse_bytes,
                usage.source_input_bytes,
            ),
        })
    }
}
