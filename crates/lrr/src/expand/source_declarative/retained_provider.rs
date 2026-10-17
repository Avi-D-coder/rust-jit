//! Cold immutable provider ownership accounting. Addresses only deduplicate
//! actual allocations; they never enter source keys or macro selection.
use super::*;
use std::collections::BTreeSet;
impl SourceDeclarative {
    pub(crate) fn retained_admission_bytes(
        &self,
        maximum: usize,
        source_bytes: &mut impl FnMut(&PreparedSourceView) -> Result<usize, Diagnostic>,
    ) -> Result<usize, Diagnostic> {
        let limit = || {
            fault(
                "LRR-E-METADATA-LIMIT",
                "sealed macro provider ownership budget",
            )
        };
        let mut bytes = 0usize;
        let mut seen = BTreeSet::new();
        let mut pending = Vec::new();
        pending.try_reserve_exact(1).map_err(|_| limit())?;
        pending.push(self);
        while let Some(provider) = pending.pop() {
            if !seen.insert(provider as *const Self as usize) {
                continue;
            }
            let mut add = |n: usize| -> Result<(), Diagnostic> {
                bytes = bytes
                    .checked_add(n)
                    .filter(|n| *n <= maximum)
                    .ok_or_else(limit)?;
                Ok(())
            };
            add(std::mem::size_of::<Self>() + 128)?;
            add(provider.module_snapshot_bytes()?)?;
            add(source_bytes(&provider.source)?)?;
            add(provider
                .parents
                .capacity()
                .checked_mul(std::mem::size_of::<(usize, Digest)>())
                .ok_or_else(limit)?)?;
            if let Some(compiler) = &provider.compiler {
                add(compiler.owning_profile_metadata_bytes()?)?;
            }
            if let Some(procedural) = &provider.procedural {
                add(procedural.metadata_bytes()?)?;
            }
            for catalog in provider
                .catalogs
                .values()
                .chain(provider.generated.values())
            {
                add(catalog.metadata_bytes()?)?;
                add(2 * std::mem::size_of::<usize>())?;
            }
            let children = provider
                .source_modules
                .len()
                .checked_add(provider.derive_prefixes.len())
                .ok_or_else(limit)?;
            pending
                .len()
                .checked_add(children)
                .ok_or_else(limit)?;
            // Only one set row is retained per visited provider. Scratch is a
            // simultaneous peak bound, not repeatedly charged as owned graph.
            add(160)?;
            bytes
                .checked_add(
                    seen.len()
                        .checked_add(pending.len())
                        .and_then(|n| n.checked_add(children))
                        .and_then(|n| n.checked_mul(160))
                        .ok_or_else(limit)?,
                )
                .filter(|n| *n <= maximum)
                .ok_or_else(limit)?;
            pending.try_reserve_exact(children).map_err(|_| limit())?;
            pending.extend(provider.source_modules.values().map(|p| p.as_ref()));
            pending.extend(provider.derive_prefixes.values().map(|(_, p)| p.as_ref()));
        }
        Ok(bytes)
    }
}
