//! Pure coordinator facts for current source-view admission, never a query.
use super::*;
use crate::core::digest::StableEncode;
/// The complete published input census of one input epoch. Every input write
/// increases the revision, so the table of an unchanged `(instance, revision)`
/// is exactly this map; certificates minted at that epoch share one allocation
/// instead of copying every key again per call.
#[derive(Debug)]
pub(crate) struct InputCensus {
    epoch: (u64, u64),
    stamps: BTreeMap<InputKey, Digest>,
    /// `size_of::<DependencyStamp>() + heap_bytes` summed over `stamps`.
    stamp_bytes: usize,
    fingerprint: std::sync::OnceLock<Digest>,
}
impl InputCensus {
    pub(crate) fn epoch(&self) -> (u64, u64) {
        self.epoch
    }
    pub(crate) fn stamps(&self) -> &BTreeMap<InputKey, Digest> {
        &self.stamps
    }
    pub(crate) fn stamp_bytes(&self) -> usize {
        self.stamp_bytes
    }
    /// Identity of the whole census, computed at most once and only when a
    /// certificate fingerprint is actually requested.
    pub(crate) fn fingerprint(&self) -> Digest {
        *self.fingerprint.get_or_init(|| {
            let mut e = crate::core::digest::StableEncoder::new("lrr/v1/input-census");
            e.u64(self.stamps.len() as u64);
            for (key, fingerprint) in &self.stamps {
                e.u32(0);
                e.record(key);
                e.digest(*fingerprint);
            }
            e.finish()
        })
    }
}
impl Database {
    /// The published input census of the current input epoch, built once per
    /// epoch and shared. Pure coordinator fact, never a query.
    pub(crate) fn source_view_namespace_census(&self) -> Result<Arc<InputCensus>, Diagnostic> {
        self.require_coordinator()?;
        let epoch = self.input_epoch();
        if let Ok(cached) = self.source_view_census.lock()
            && let Some(census) = cached.as_ref()
            && census.epoch == epoch
        {
            return Ok(Arc::clone(census));
        }
        let mut stamps = BTreeMap::new();
        let mut stamp_bytes = 0usize;
        for (key, row) in &self.inputs {
            stamp_bytes = stamp_bytes
                .saturating_add(std::mem::size_of::<DependencyStamp>())
                .saturating_add(DependencyStamp::input_heap_bytes(key));
            stamps.insert(key.clone(), row.fingerprint);
        }
        let census = Arc::new(InputCensus {
            epoch,
            stamps,
            stamp_bytes,
            fingerprint: std::sync::OnceLock::new(),
        });
        if let Ok(mut cached) = self.source_view_census.lock() {
            *cached = Some(Arc::clone(&census));
        }
        Ok(census)
    }
    pub(crate) fn source_view_namespace_stamps(
        &self,
    ) -> Result<BTreeMap<InputKey, Digest>, Diagnostic> {
        Ok(self.source_view_namespace_census()?.stamps.clone())
    }
    /// Epoch of the census currently cached, if any (test observation of
    /// whether a check rebuilt it).
    #[cfg(test)]
    pub(crate) fn cached_source_view_census_epoch(&self) -> Option<(u64, u64)> {
        self.source_view_census
            .lock()
            .ok()?
            .as_ref()
            .map(|census| census.epoch)
    }
    /// Publish (or with `None` withdraw) one raw control input, increasing the
    /// revision exactly as every real input write does.
    #[cfg(test)]
    pub(crate) fn write_control_input_for_test(
        &mut self,
        key: InputKey,
        value: Option<InputValue>,
    ) {
        match value {
            Some(value) => {
                let fingerprint = value.fingerprint();
                self.inputs.insert(
                    key,
                    InputRecord {
                        fingerprint,
                        value: Arc::new(value),
                    },
                );
            }
            None => {
                self.inputs.remove(&key);
            }
        }
        self.revision = self.revision.checked_add(1).expect("control revision");
    }
    pub(crate) fn source_view_scope_certificate(
        &self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<Arc<InputValue>, Diagnostic> {
        self.require_coordinator()?;
        let certificate = self.lexical_scope_certificate(source, scope)?;
        self.require_source_view_certificate(certificate)
    }
    /// The current Type certificate of `scope`: its own primary seal or a
    /// facet contained in a current seal of the same world. `None` is an
    /// unestablished prerequisite, never a negative namespace answer.
    pub(crate) fn source_view_type_scope_certificate(
        &self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<Option<Arc<InputValue>>, Diagnostic> {
        use crate::{db::seals::SealDomain, source::manifest::Namespace};
        self.require_coordinator()?;
        // Type facts of a Type fixed point only: the scope's own Type seal,
        // else the unique facet a current Type seal contains for it.
        let mut primary = None;
        let mut contained = None;
        for key in self.scope_certificates.candidates(source, scope, Namespace::Type) {
            let SealDomain::Scope {
                id,
                namespace: Namespace::Type,
            } = key.domain
            else {
                continue;
            };
            let row = self.inputs.get(&InputKey::DiscoverySeal(key.clone())).ok_or_else(|| {
                fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "indexed Type scope certificate input absent",
                )
            })?;
            let InputValue::DiscoverySeal(input) = row.value.as_ref() else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "indexed Type scope certificate variant differs",
                ));
            };
            if !matches!(self.inputs.get(&InputKey::ExpansionEpoch).map(|r| r.value.as_ref()),
                Some(InputValue::ExpansionEpoch(epoch)) if *epoch == key.epoch)
                || input
                    .scope_certificate(scope, Namespace::Type, &source.outline.parse.profile)?
                    .is_none()
            {
                continue;
            }
            let slot = if id == scope.fingerprint() {
                &mut primary
            } else {
                &mut contained
            };
            if slot.replace(Arc::clone(&row.value)).is_some() {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "Type scope certificate selection is ambiguous",
                ));
            }
        }
        primary
            .or(contained)
            .map(|certificate| self.require_source_view_certificate(certificate))
            .transpose()
    }
    fn require_source_view_certificate(
        &self,
        certificate: Arc<InputValue>,
    ) -> Result<Arc<InputValue>, Diagnostic> {
        let InputValue::DiscoverySeal(input) = certificate.as_ref() else {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "source-view discovery certificate variant differs",
            ));
        };
        if !matches!(self.inputs.get(&InputKey::ExpansionEpoch).map(|r|r.value.as_ref()), Some(InputValue::ExpansionEpoch(epoch)) if *epoch==input.seal.key().epoch)
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "source-view discovery expansion epoch is absent or stale",
            ));
        }
        if !matches!(self.inputs.get(&InputKey::SourceDependencies).map(|r|r.value.as_ref()), Some(InputValue::SourceDependencies(graph)) if graph.fingerprint()==input.source_dependencies)
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "source-view discovery dependency graph differs",
            ));
        }
        for (key, fingerprint) in &input.compiler_inputs {
            if self
                .inputs
                .get(key)
                .is_none_or(|row| row.fingerprint != *fingerprint)
            {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "source-view discovery compiler input differs",
                ));
            }
        }
        Ok(certificate)
    }
}
