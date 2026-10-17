//! Pure coordinator facts for current source-view admission, never a query.
use super::*;
use crate::core::digest::StableEncode;
impl Database {
    pub(crate) fn source_view_namespace_stamps(
        &self,
    ) -> Result<BTreeMap<InputKey, Digest>, Diagnostic> {
        self.require_coordinator()?;
        let mut stamps = BTreeMap::new();
        let mut bytes = 0usize;
        for (key, row) in &self.inputs {
            bytes = account_add(
                account_add(
                    account_add(bytes, std::mem::size_of::<(InputKey, Digest)>())?,
                    128,
                )?,
                key.heap_bytes(),
            )?;
            stamps.insert(key.clone(), row.fingerprint);
        }
        Ok(stamps)
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
