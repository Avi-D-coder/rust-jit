//! Retained cfg-active impl positions from the owning generated header plan.
//!
//! This is declaration metadata, not a namespace-completion receipt. Only the
//! parent header-plan module may construct it from its actual active Outline.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::generated_items::GeneratedItems,
    source::snapshot::CfgSet,
    syntax::{outline::ScopeKey, spans::SourceRange},
};
use std::sync::Arc;

pub(crate) struct ActiveImplProjection {
    descriptor: Digest,
    cfg: Digest,
    scope: ScopeKey,
    epoch: u64,
    ranges: Arc<[SourceRange]>,
    metadata_bytes: usize,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
fn scope_bytes(scope: &ScopeKey) -> Option<usize> {
    scope.module_path.iter().try_fold(
        scope
            .module_path
            .capacity()
            .checked_mul(std::mem::size_of::<String>())?,
        |bytes, part| bytes.checked_add(part.capacity()),
    )
}
impl ActiveImplProjection {
    /// The caller is the owning header plan, after cfg-active Outline capture.
    pub(super) fn prepare(
        items: &GeneratedItems,
        cfg: &CfgSet,
        scope: &ScopeKey,
        epoch: u64,
        ranges: Vec<SourceRange>,
    ) -> Result<Self, Diagnostic> {
        if items.epoch() != epoch
            || ranges
                .iter()
                .any(|range| range.start >= range.end || range.end as usize > items.text().len())
            || ranges
                .windows(2)
                .any(|pair| (pair[0].start, pair[0].end) >= (pair[1].start, pair[1].end))
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "active impl projection input differs",
            ));
        }
        // Include both Arc allocation headers (projection and range slice).
        // Owning plan/waiter Arc handles are charged by their containers.
        let fixed_bytes = ranges
            .len()
            .checked_mul(std::mem::size_of::<SourceRange>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "active impl projection budget"))?;
        fixed_bytes
            .checked_add(
                scope_bytes(scope)
                    .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "active impl scope budget"))?,
            )
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "active impl projection budget"))?;
        let scope = scope.clone();
        let metadata_bytes = scope_bytes(&scope)
            .and_then(|n| fixed_bytes.checked_add(n))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "active impl scope budget"))?;
        Ok(Self {
            descriptor: items.record().descriptor().fingerprint(),
            cfg: cfg.fingerprint(),
            scope,
            epoch,
            ranges: Arc::from(ranges.into_boxed_slice()),
            metadata_bytes,
        })
    }
    pub(crate) fn active_impl_ranges(
        &self,
        items: &GeneratedItems,
        cfg: &CfgSet,
        scope: &ScopeKey,
        epoch: u64,
    ) -> Result<&[SourceRange], Diagnostic> {
        if self.descriptor != items.record().descriptor().fingerprint()
            || self.cfg != cfg.fingerprint()
            || &self.scope != scope
            || self.epoch != epoch
            || items.epoch() != epoch
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "active impl projection owner differs",
            ));
        }
        Ok(&self.ranges)
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        <Self as StableEncode>::fingerprint(self)
    }
}
impl StableEncode for ActiveImplProjection {
    const DOMAIN: &'static str = "lrr/v1/generated-active-impl-projection";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.descriptor);
        e.digest(self.cfg);
        e.record(&self.scope);
        e.u64(self.epoch);
        e.sequence(&self.ranges, |e, range| {
            e.u32(range.start);
            e.u32(range.end);
        });
    }
}
