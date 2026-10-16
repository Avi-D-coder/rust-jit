//! Derived locator for genuine coordinator-published scope certificates.
//! It grants no name/source authority and retains no extra input/parser owners.
use super::*;
use crate::{
    core::digest::{StableEncode, StableEncoder},
    db::seals::{DiscoveryInput, SealDomain, SealKey},
    source::{changes::SourceSelection, manifest::Namespace},
    syntax::outline::ScopeKey,
};
use std::collections::BTreeSet;

const MAX_ASSOCIATIONS: usize = 100_000;
const MAX_METADATA: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct SelectionKey {
    source: Digest,
    scope: Digest,
    namespace: Namespace,
    edition: u16,
}
impl SelectionKey {
    fn new(source: &SourceSelection, scope: &ScopeKey, namespace: Namespace) -> Self {
        let mut encoder = StableEncoder::new("lrr/v1/scope-certificate-source-selection");
        encoder.string(source.path.as_str());
        encoder.record(&source.outline);
        Self {
            source: encoder.finish(),
            scope: scope.fingerprint(),
            namespace,
            edition: source.outline.parse.profile.edition,
        }
    }
}

#[derive(Default, Clone)]
pub(super) struct ScopeCertificateIndex {
    entries: BTreeMap<SelectionKey, BTreeSet<SealKey>>,
    associations: usize,
    bytes: usize,
}
impl ScopeCertificateIndex {
    pub(super) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    /// Cold coordinator preparation only. All fallible checks and allocations
    /// precede publication of either the input rows or their derived locator.
    pub(super) fn with_input(&self, input: &DiscoveryInput) -> Result<Option<Self>, Diagnostic> {
        let Some(scope) = &input.scope else {
            return Ok(None);
        };
        let seal = input.seal.key();
        if !matches!(&seal.domain, SealDomain::Scope { id, namespace }
            if *id == scope.scope.fingerprint() && *namespace == scope.namespace)
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "lexical namespace seal owner differs",
            ));
        }
        let mut next = None;
        for source in &input.sources {
            // Match the existing reader's source-edition selection exactly.
            if source.outline.parse.profile.edition != scope.edition {
                continue;
            }
            let key = SelectionKey::new(source, &scope.scope, scope.namespace);
            let current = next.as_ref().unwrap_or(self);
            if current
                .entries
                .get(&key)
                .is_some_and(|keys| keys.contains(&seal))
            {
                continue;
            }
            let associations = current
                .associations
                .checked_add(1)
                .filter(|n| *n <= MAX_ASSOCIATIONS)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "scope certificate index count limit",
                    )
                })?;
            // Conservative BTree node allowance, including owned inline keys.
            let mut bytes = account_add(current.bytes, 128 + std::mem::size_of::<SealKey>())?;
            if !current.entries.contains_key(&key) {
                bytes = account_add(
                    bytes,
                    128 + std::mem::size_of::<SelectionKey>()
                        + std::mem::size_of::<BTreeSet<SealKey>>(),
                )?;
            }
            if bytes > MAX_METADATA {
                return Err(fault(
                    "LRR-E-METADATA-LIMIT",
                    "scope certificate index byte limit",
                ));
            }
            let next = next.get_or_insert_with(|| self.clone());
            next.entries.entry(key).or_default().insert(seal.clone());
            next.associations = associations;
            next.bytes = bytes;
        }
        Ok(next)
    }
    pub(super) fn candidates(
        &self,
        source: &SourceSelection,
        scope: &ScopeKey,
        namespace: Namespace,
    ) -> impl Iterator<Item = &SealKey> {
        self.entries
            .get(&SelectionKey::new(source, scope, namespace))
            .into_iter()
            .flat_map(|keys| keys.iter())
    }
}
