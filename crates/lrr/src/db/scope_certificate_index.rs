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

impl StableEncode for SelectionKey {
    const DOMAIN: &'static str = "lrr/v1/scope-certificate-selection";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.source);
        e.digest(self.scope);
        e.u32(self.namespace.tag());
        e.u16(self.edition);
    }
}
#[derive(Clone)]
struct SelectionEntry {
    candidates: BTreeSet<SealKey>,
    fingerprint: Digest,
}
impl SelectionEntry {
    fn new() -> Self {
        Self {
            candidates: BTreeSet::new(),
            fingerprint: missing_fingerprint(),
        }
    }
    fn insert(&mut self, seal: SealKey) {
        self.candidates.insert(seal);
        let mut e = StableEncoder::new("lrr/v1/scope-certificate-candidates");
        e.u64(self.candidates.len() as u64); // bounded at publication
        for seal in &self.candidates {
            e.record(seal);
        }
        self.fingerprint = e.finish();
    }
}
#[derive(Default, Clone)]
pub(super) struct ScopeCertificateIndex {
    entries: BTreeMap<SelectionKey, SelectionEntry>,
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
                .is_some_and(|keys| keys.candidates.contains(&seal))
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
                        + std::mem::size_of::<SelectionEntry>(),
                )?;
            }
            if bytes > MAX_METADATA {
                return Err(fault(
                    "LRR-E-METADATA-LIMIT",
                    "scope certificate index byte limit",
                ));
            }
            let next = next.get_or_insert_with(|| self.clone());
            next.entries
                .entry(key)
                .or_insert_with(SelectionEntry::new)
                .insert(seal.clone());
            next.associations = associations;
            next.bytes = bytes;
        }
        Ok(next)
    }
    pub(super) fn selection_row(
        &self,
        source: &SourceSelection,
        scope: &ScopeKey,
        namespace: Namespace,
    ) -> (InputKey, InputValue) {
        let key = SelectionKey::new(source, scope, namespace);
        let selection = key.fingerprint();
        let candidate_set = self
            .entries
            .get(&key)
            .map_or_else(missing_fingerprint, |row| row.fingerprint);
        (
            InputKey::ScopeCertificateSelection(selection),
            InputValue::ScopeCertificateSelection {
                selection,
                candidate_set,
            },
        )
    }
    pub(super) fn selection_rows(&self, input: &DiscoveryInput) -> BTreeMap<InputKey, InputValue> {
        let mut rows = BTreeMap::new();
        if let Some(scope) = &input.scope {
            for source in &input.sources {
                if source.outline.parse.profile.edition == scope.edition {
                    let (key, value) = self.selection_row(source, &scope.scope, scope.namespace);
                    rows.insert(key, value);
                }
            }
        }
        rows
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
            .flat_map(|keys| keys.candidates.iter())
    }
}
