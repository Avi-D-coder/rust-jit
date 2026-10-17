//! Closed, coordinator-owned derived lexical function publication.
use super::*;
use crate::{core::digest::StableEncode, frontend::callables::lexical::LexicalAdmission};

impl Database {
    /// Namespace membership and epoch stamps include absence by map equality.
    /// Exact Definition* rows and admitted parsing-only retained syntax do not
    /// alter the namespace universe. Their consumers check their own leases;
    /// source, epoch, records and discovery certificates remain in this census.
    /// The published namespace-membership census, shared per input revision:
    /// inputs change only with a revision increase, so callers at the same
    /// revision share one map (and compare it by identity first).
    pub(crate) fn lexical_namespace_stamps(
        &self,
    ) -> Result<Arc<BTreeMap<InputKey, Digest>>, Diagnostic> {
        if let Ok(cached) = self.lexical_stamps.lock()
            && let Some((revision, stamps)) = cached.as_ref()
            && *revision == self.revision
        {
            return Ok(Arc::clone(stamps));
        }
        let stamps = Arc::new(self.census_lexical_namespace_stamps()?);
        if let Ok(mut cached) = self.lexical_stamps.lock() {
            *cached = Some((self.revision, Arc::clone(&stamps)));
        }
        Ok(stamps)
    }
    fn census_lexical_namespace_stamps(&self) -> Result<BTreeMap<InputKey, Digest>, Diagnostic> {
        let mut stamps = BTreeMap::new();
        let mut bytes = 0usize;
        for (key, row) in &self.inputs {
            if matches!(
                key,
                InputKey::DefinitionSource(_)
                    | InputKey::LocalDeclarationOwner(_)
                    | InputKey::BodyEnvironment(_)
                    | InputKey::DefinitionHeader(_)
                    | InputKey::DefinitionBody(_)
                    | InputKey::DeclarationContext(_)
                    | InputKey::SourceBlob(_)
                    | InputKey::RetainedSyntax(_)
            ) {
                continue;
            }
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
    /// Pure current selection with a relevant missing/present dependency. The
    /// locator grants no name authority: selection still reads the genuine
    /// admitted scope certificate and validates its actual source/epoch.
    pub(crate) fn current_lexical_scope_certificate(
        &mut self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<Option<Arc<InputValue>>, Diagnostic> {
        self.current_scope_certificate(source, scope, crate::source::manifest::Namespace::Value)
    }
    pub(crate) fn current_scope_certificate(
        &mut self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
        namespace: crate::source::manifest::Namespace,
    ) -> Result<Option<Arc<InputValue>>, Diagnostic> {
        let (key, expected) = self
            .scope_certificates
            .selection_row(source, scope, namespace);
        let current = self.input(&key);
        match current.as_deref() {
            Some(actual)
                if actual.matches(&key) && actual.fingerprint() == expected.fingerprint() => {}
            None if matches!(expected, InputValue::ScopeCertificateSelection { candidate_set, .. }
                if candidate_set == missing_fingerprint()) => {}
            _ => {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "scope certificate selector input/index differs",
                ));
            }
        }
        // Epoch absence is a dependency too. An invalid present row must not
        // silently turn a stale selected certificate into a negative answer.
        if self
            .input(&InputKey::ExpansionEpoch)
            .as_deref()
            .is_some_and(|value| !matches!(value, InputValue::ExpansionEpoch(_)))
        {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "scope certificate epoch input differs",
            ));
        }
        let selected = self.optional_scope_certificate(source, scope, namespace)?;
        if let Some(selected) = selected {
            let InputValue::DiscoverySeal(input) = selected.as_ref() else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "selected scope certificate differs",
                ));
            };
            let current = self.input(&InputKey::DiscoverySeal(input.seal.key()));
            // The selector just returned this exact immutable input allocation;
            // input() records its cached fingerprint without hashing the seal's
            // complete source/producer payload again.
            if !current
                .as_ref()
                .is_some_and(|actual| Arc::ptr_eq(actual, &selected))
            {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "selected scope certificate input changed",
                ));
            }
            Ok(current)
        } else {
            Ok(None)
        }
    }
    /// Optional inference may preserve unresolved syntax if no owning seal
    /// exists. Ambiguous, invalid and stale publication errors remain errors.
    pub(crate) fn optional_lexical_scope_certificate(
        &self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<Option<Arc<InputValue>>, Diagnostic> {
        self.optional_scope_certificate(source, scope, crate::source::manifest::Namespace::Value)
    }
    fn optional_scope_certificate(
        &self,
        source: &crate::source::changes::SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
        namespace: crate::source::manifest::Namespace,
    ) -> Result<Option<Arc<InputValue>>, Diagnostic> {
        // A scope/namespace may be certified by its own (primary) seal and,
        // for Type scopes, derivatively by another seal's path-resolution
        // proofs (a Value seal certifies the Type scopes its paths traverse).
        // The primary certificate is authoritative; derived certificates
        // select only when every candidate certifies identical names.
        let mut primary: Option<Arc<InputValue>> = None;
        let mut derived: Option<(Arc<InputValue>, bool)> = None;
        let ambiguous = || {
            fault(
                "LRR-E-CALL-CONTEXT",
                "lexical namespace seal selection is ambiguous",
            )
        };
        for key in self.scope_certificates.candidates(source, scope, namespace) {
            let row = self
                .inputs
                .get(&InputKey::DiscoverySeal(key.clone()))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "indexed scope certificate input absent",
                    )
                })?;
            let InputValue::DiscoverySeal(input) = row.value.as_ref() else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "indexed scope certificate variant differs",
                ));
            };
            let Some(certificate) =
                input.scope_certificate(scope, namespace, &source.outline.parse.profile)?
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "indexed scope certificate source/owner differs",
                ));
            };
            if !input.sources.contains(source) {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "indexed scope certificate source/owner differs",
                ));
            }
            if self.inputs.get(&InputKey::ExpansionEpoch).is_some_and(|epoch| !matches!(epoch.value.as_ref(), InputValue::ExpansionEpoch(current) if *current == input.seal.key().epoch)) {
                continue; // old immutable seals remain cached across epochs
            }
            let is_primary = input
                .scope
                .as_ref()
                .is_some_and(|own| std::ptr::eq(own, certificate));
            if is_primary {
                if primary.is_some() {
                    return Err(ambiguous());
                }
                primary = Some(row.value.clone());
                continue;
            }
            match &mut derived {
                None => derived = Some((row.value.clone(), true)),
                Some((first, identical)) => {
                    let InputValue::DiscoverySeal(first) = first.as_ref() else {
                        return Err(ambiguous());
                    };
                    let same = first
                        .scope_certificate(scope, namespace, &source.outline.parse.profile)?
                        .is_some_and(|other| {
                            other.scope == certificate.scope
                                && other.namespace == certificate.namespace
                                && other.edition == certificate.edition
                                && other.names == certificate.names
                        });
                    *identical &= same;
                }
            }
        }
        match (primary, derived) {
            (Some(primary), _) => Ok(Some(primary)),
            (None, Some((_, false))) => Err(ambiguous()),
            (None, Some((derived, true))) => Ok(Some(derived)),
            (None, None) => Ok(None),
        }
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
                // Header fingerprints deliberately omit source attachments.
                // A prior cold publication must agree on the actual rows too.
                let identical = match (old.value.as_ref(), &value) {
                    (InputValue::DefinitionSource(a), InputValue::DefinitionSource(b)) => a == b,
                    (InputValue::DefinitionHeader(a), InputValue::DefinitionHeader(b)) => a == b,
                    (InputValue::DeclarationContext(a), InputValue::DeclarationContext(b)) => {
                        a == b
                    }
                    (InputValue::DefinitionBody(a), InputValue::DefinitionBody(b)) => a == b,
                    _ => false,
                };
                if !identical || old.fingerprint != fingerprint {
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
        let definition = proof.plan().item().def_key.fingerprint();
        let expected_owner = InputValue::LocalDeclarationOwner(proof.plan().caller().key());
        let owner = self
            .inputs
            .get(&InputKey::LocalDeclarationOwner(definition));
        let certified_owner = owner.is_some_and(|row| {
            matches!((row.value.as_ref(), &expected_owner),
                (InputValue::LocalDeclarationOwner(actual), InputValue::LocalDeclarationOwner(expected))
                if actual == expected)
                && row.fingerprint == expected_owner.fingerprint()
        });
        if owner.is_some() && !certified_owner {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "lexical cold declaration belongs to another owning body",
            ));
        }
        let exact_cold_headers = existing == 3
            && certified_owner
            && staged.len() == 1
            && staged.contains_key(&InputKey::DefinitionBody(definition));
        if existing != 0 && existing != 4 && !exact_cold_headers {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "lexical definition has a partial prior publication",
            ));
        }
        if staged.is_empty() {
            return Ok(());
        }
        let _ = extra; // E-R5-006: no byte accounting.
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

#[cfg(test)]
#[path = "lexical_inputs_tests.rs"]
mod tests;
