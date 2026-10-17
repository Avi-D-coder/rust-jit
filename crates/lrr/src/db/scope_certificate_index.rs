//! Derived locator for genuine coordinator-published scope certificates.
//! It grants no name/source authority and retains no extra input/parser owners.
use super::*;
use crate::{
    core::digest::{StableEncode, StableEncoder},
    db::seals::{DiscoveryInput, SealKey},
    source::{changes::SourceSelection, manifest::Namespace},
    syntax::outline::ScopeKey,
};
use std::collections::BTreeSet;


// A locator need not associate a Type scope with foreign or sibling physical
// source roots. This is only structural narrowing; consumers still require
// actual Outline membership and the genuine certificate, not a prefix grant.
fn source_can_own(source: &SourceSelection, scope: &crate::db::seals::CertifiedScope) -> bool {
    source.outline.parse.profile.edition == scope.edition
        && (scope.namespace != Namespace::Type
            || (source.outline.crate_key == scope.scope.crate_key
                && scope
                    .scope
                    .module_path
                    .starts_with(&source.outline.module_path)))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct SelectionKey {
    source: Digest,
    scope: Digest,
    namespace: Namespace,
    edition: u16,
}
impl SelectionKey {
    fn source_digest(source: &SourceSelection) -> Digest {
        let mut encoder = StableEncoder::new("lrr/v1/scope-certificate-source-selection");
        encoder.string(source.path.as_str());
        encoder.record(&source.outline);
        encoder.finish()
    }
    fn new(source: &SourceSelection, scope: &ScopeKey, namespace: Namespace) -> Self {
        Self::with_digests(source, Self::source_digest(source), scope.fingerprint(), namespace)
    }
    fn with_digests(
        source: &SourceSelection,
        source_digest: Digest,
        scope: Digest,
        namespace: Namespace,
    ) -> Self {
        Self {
            source: source_digest,
            scope,
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
        input.require_scope_certificate_layout()?;
        let seal = input.seal.key();
        let mut next = None;
        // Each source and scope identity is encoded once, not per pair.
        let source_digests: Vec<Digest> =
            input.sources.iter().map(SelectionKey::source_digest).collect();
        for scope in input.scope_certificates() {
            let scope_digest = scope.scope.fingerprint();
            for (source, source_digest) in input.sources.iter().zip(&source_digests) {
                if !source_can_own(source, scope) {
                    continue;
                }
                let key = SelectionKey::with_digests(
                    source,
                    *source_digest,
                    scope_digest,
                    scope.namespace,
                );
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
                let next = next.get_or_insert_with(|| self.clone());
                next.entries
                    .entry(key)
                    .or_insert_with(SelectionEntry::new)
                    .insert(seal.clone());
                next.associations = associations;
                next.bytes = bytes;
            }
        }
        Ok(next)
    }
    pub(super) fn selection_row(
        &self,
        source: &SourceSelection,
        scope: &ScopeKey,
        namespace: Namespace,
    ) -> (InputKey, InputValue) {
        self.selection_row_for(SelectionKey::new(source, scope, namespace))
    }
    fn selection_row_for(&self, key: SelectionKey) -> (InputKey, InputValue) {
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
        let source_digests: Vec<Digest> =
            input.sources.iter().map(SelectionKey::source_digest).collect();
        for scope in input.scope_certificates() {
            let scope_digest = scope.scope.fingerprint();
            for (source, source_digest) in input.sources.iter().zip(&source_digests) {
                if source_can_own(source, scope) {
                    let (key, value) = self.selection_row_for(SelectionKey::with_digests(
                        source,
                        *source_digest,
                        scope_digest,
                        scope.namespace,
                    ));
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
