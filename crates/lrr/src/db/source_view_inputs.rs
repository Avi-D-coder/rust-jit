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
            if stamps.len() >= 100_000 {
                return Err(fault(
                    "LRR-E-METADATA-LIMIT",
                    "source-view current input count limit",
                ));
            }
            bytes = account_add(
                account_add(
                    account_add(bytes, std::mem::size_of::<(InputKey, Digest)>())?,
                    128,
                )?,
                key.heap_bytes(),
            )?;
            if bytes > 32 * 1024 * 1024 {
                return Err(fault(
                    "LRR-E-METADATA-LIMIT",
                    "source-view current input byte limit",
                ));
            }
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
        if input.compiler_inputs.len() > 100_000 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "source-view compiler dependency count limit",
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
