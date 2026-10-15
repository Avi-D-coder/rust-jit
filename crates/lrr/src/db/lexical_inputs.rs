//! Closed, coordinator-owned derived lexical function publication.
use super::*;
use crate::{core::digest::StableEncode, frontend::callables::lexical::LexicalAdmission};

impl Database {
    /// Namespace membership and epoch stamps include absence by map equality.
    /// Appending exact Definition* rows does not alter the namespace universe.
    pub(crate) fn lexical_namespace_stamps(
        &self,
    ) -> Result<BTreeMap<InputKey, Digest>, Diagnostic> {
        let mut stamps = BTreeMap::new();
        let mut bytes = 0usize;
        for (key, row) in &self.inputs {
            if matches!(
                key,
                InputKey::DefinitionSource(_)
                    | InputKey::DefinitionHeader(_)
                    | InputKey::DefinitionBody(_)
                    | InputKey::DeclarationContext(_)
                    | InputKey::SourceBlob(_)
            ) {
                continue;
            }
            if stamps.len() >= 100_000 {
                return Err(fault(
                    "LRR-E-METADATA-LIMIT",
                    "lexical namespace stamp count limit",
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
                    "lexical namespace stamp size limit",
                ));
            }
            stamps.insert(key.clone(), row.fingerprint);
        }
        Ok(stamps)
    }
    pub(crate) fn lexical_scope_certificate(
        &self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<Arc<InputValue>, Diagnostic> {
        self.optional_lexical_scope_certificate(source, scope)?
            .ok_or_else(|| {
                fault(
                    "LRR-E-CALL-CONTEXT",
                    "lexical source Value namespace is not currently sealed",
                )
            })
    }
    /// Optional inference may preserve unresolved syntax if no owning seal
    /// exists. Ambiguous, invalid and stale publication errors remain errors.
    pub(crate) fn optional_lexical_scope_certificate(
        &self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<Option<Arc<InputValue>>, Diagnostic> {
        let mut selected = None;
        for row in self.inputs.values() {
            let InputValue::DiscoverySeal(input) = row.value.as_ref() else {
                continue;
            };
            let Some(certified) = &input.scope else {
                continue;
            };
            if certified.scope != *scope
                || certified.namespace != crate::source::manifest::Namespace::Value
                || certified.edition != source.outline.parse.profile.edition
                || !input.sources.contains(source)
            {
                continue;
            }
            if !matches!(input.seal.domain(), crate::db::seals::SealDomain::Scope { id, namespace } if *id == scope.fingerprint() && *namespace == certified.namespace)
            {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "lexical namespace seal owner differs",
                ));
            }
            if self.inputs.get(&InputKey::ExpansionEpoch).is_some_and(|epoch| !matches!(epoch.value.as_ref(), InputValue::ExpansionEpoch(current) if *current == input.seal.key().epoch)) {
                continue; // old immutable seals remain cached across epochs
            }
            if selected.is_some() {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "lexical namespace seal selection is ambiguous",
                ));
            }
            selected = Some(row.value.clone());
        }
        Ok(selected)
    }
    /// The only accepted argument is a privately minted exact source proof.
    /// No run reset, base-source publication or provider mutation is possible.
    pub(crate) fn publish_lexical_function(
        &mut self,
        proof: &LexicalAdmission,
    ) -> Result<(), Diagnostic> {
        self.require_coordinator()?;
        if !self.run_active {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "lexical admission requires its active run",
            ));
        }
        proof.revalidate(self)?;
        if self.revision != proof.plan().query_revision() {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "lexical admission query revision changed",
            ));
        }
        let mut staged = BTreeMap::new();
        let mut existing = 0usize;
        let mut extra = 0usize;
        for (key, value) in proof.derived_inputs() {
            if !value.matches(&key) {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "lexical derived row kind differs",
                ));
            }
            let fingerprint = value.fingerprint();
            if let Some(old) = self.inputs.get(&key) {
                if old.fingerprint != fingerprint {
                    return Err(fault(
                        "LRR-E-CALL-CONTEXT",
                        "lexical immutable definition row conflict",
                    ));
                }
                existing = existing
                    .checked_add(1)
                    .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "lexical row count overflow"))?;
            } else {
                extra = account_add(extra, input_metadata_bytes(&key, &value)?)?;
                staged.insert(
                    key,
                    InputRecord {
                        value: Arc::new(value),
                        fingerprint,
                    },
                );
            }
        }
        if existing != 0 && existing != 4 {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "lexical definition has a partial prior publication",
            ));
        }
        if staged.is_empty() {
            return Ok(());
        }
        let usage = self.cache_usage()?;
        let budgets = self.run_budgets.unwrap_or_default();
        if account_add(usage.memo_bytes, extra)? > budgets.query_bytes
            || usage.source_input_bytes > budgets.source_input_bytes
        {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "lexical definition publication exceeds cache cap",
            ));
        }
        let revision = self.revision.checked_add(1).ok_or_else(|| {
            fault(
                "LRR-E-ID-EXHAUSTED",
                "lexical publication revision exhausted",
            )
        })?;
        // Every fallible check and owned row construction precedes mutation.
        self.inputs.extend(staged);
        self.revision = revision;
        Ok(())
    }
}
