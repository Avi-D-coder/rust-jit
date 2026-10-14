//! Immutable owned normal forms supplied by the pinned adapter. This module
//! never computes an alternative hygiene algorithm or imports frontend IDs.
use crate::{
    core::diagnostic::Diagnostic,
    syntax::token_wire::{
        ContextRecord, Key, NameContextKey, NameResolutionContext, ResolutionContext, TokenForest,
        Transparency,
    },
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct HygieneFacts {
    contexts: BTreeMap<Key, ContextRecord>,
    normalized: BTreeMap<Key, ResolutionContext>,
    names: BTreeMap<Key, NameResolutionContext>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
// Structural validation of supplied normal forms; actual normalization is
// performed by the pinned adapter, not reconstructed here.
fn root(
    contexts: &BTreeMap<Key, ContextRecord>,
    mut key: Key,
    item: Option<bool>,
) -> Result<Key, Diagnostic> {
    for _ in 0..=256 {
        let record = contexts
            .get(&key)
            .ok_or_else(|| error("normalization ancestor is missing"))?;
        if record.parent.is_none() {
            if record.transparency != Transparency::Opaque {
                return Err(error("normalization root must be opaque"));
            }
            return Ok(key);
        }
        if item.is_some_and(|item| {
            record.transparency == Transparency::Transparent
                || (item && record.transparency != Transparency::Opaque)
        }) {
            return Err(error("normalization row contains excluded transparency"));
        }
        key = record.parent.expect("checked parent");
    }
    Err(error("normalization ancestor depth exceeded"))
}
impl HygieneFacts {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.contexts
            .len()
            .checked_mul(256)
            .and_then(|n| {
                self.normalized
                    .len()
                    .checked_mul(128)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| {
                self.names
                    .len()
                    .checked_mul(128)
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(|| error("owned hygiene metadata overflow"))
    }
    pub(crate) fn extend(&mut self, other: &Self) -> Result<(), Diagnostic> {
        fn count<K: Ord, V: PartialEq>(
            old: &BTreeMap<K, V>,
            new: &BTreeMap<K, V>,
        ) -> Result<usize, Diagnostic> {
            for (key, value) in new {
                if old.get(key).is_some_and(|existing| existing != value) {
                    return Err(error("conflicting owned hygiene facts"));
                }
            }
            old.len()
                .checked_add(new.keys().filter(|key| !old.contains_key(*key)).count())
                .filter(|n| *n <= 2_000_000)
                .ok_or_else(|| error("joined hygiene fact count limit"))
        }
        let contexts = count(&self.contexts, &other.contexts)?;
        let normalized = count(&self.normalized, &other.normalized)?;
        let names = count(&self.names, &other.names)?;
        contexts
            .checked_mul(256)
            .and_then(|n| normalized.checked_mul(128).and_then(|s| n.checked_add(s)))
            .and_then(|n| names.checked_mul(128).and_then(|s| n.checked_add(s)))
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| error("joined hygiene metadata limit"))?;
        self.contexts
            .extend(other.contexts.iter().map(|(k, v)| (*k, v.clone())));
        self.normalized
            .extend(other.normalized.iter().map(|(k, v)| (*k, v.clone())));
        self.names
            .extend(other.names.iter().map(|(k, v)| (*k, v.clone())));
        Ok(())
    }
    pub fn new(
        contexts: Vec<ContextRecord>,
        rows: Vec<ResolutionContext>,
    ) -> Result<Self, Diagnostic> {
        if rows.len() > 2_000_000 {
            return Err(error("resolution context table limit exceeded"));
        }
        let forest = TokenForest::new(vec![], contexts, vec![]).map_err(|e| error(&e.message))?;
        forest.encoded_len().map_err(|e| error(&e.message))?;
        let contexts: BTreeMap<_, _> = forest
            .contexts()
            .iter()
            .map(|r| (r.key(), r.clone()))
            .collect();
        let mut normalized = BTreeMap::new();
        for row in rows {
            for key in [row.original, row.item, row.local] {
                if !contexts.contains_key(&key) {
                    return Err(error("normalization row references unknown context"));
                }
            }
            let original_root = root(&contexts, row.original, None)?;
            if root(&contexts, row.item, Some(true))? != original_root
                || root(&contexts, row.local, Some(false))? != original_root
            {
                return Err(error(
                    "normalization changed source root authority or edition",
                ));
            }
            if normalized.insert(row.original, row).is_some() {
                return Err(error("duplicate resolution context row"));
            }
        }
        Ok(Self {
            contexts,
            normalized,
            names: BTreeMap::new(),
        })
    }
    /// Comparison facts come from the audited pinned adapter. Class keys are
    /// comparison-only identities; they are never accepted as source authority.
    pub fn with_name_contexts(
        mut self,
        rows: Vec<NameResolutionContext>,
    ) -> Result<Self, Diagnostic> {
        if rows.len() > 2_000_000 {
            return Err(error("name comparison table limit exceeded"));
        }
        let mut names = BTreeMap::new();
        for row in rows {
            if !self.contexts.contains_key(&row.original) {
                return Err(error("name comparison references unknown context"));
            }
            if names.insert(row.original, row).is_some() {
                return Err(error("duplicate name comparison row"));
            }
        }
        self.names = names;
        Ok(self)
    }
    pub fn name_item(&self, key: Key) -> Result<NameContextKey, Diagnostic> {
        self.names
            .get(&key)
            .map(|row| row.item)
            .ok_or_else(|| error("item name comparison input is missing"))
    }
    pub fn name_local(&self, key: Key) -> Result<NameContextKey, Diagnostic> {
        self.names
            .get(&key)
            .map(|row| row.local)
            .ok_or_else(|| error("local name comparison input is missing"))
    }
    pub fn item(&self, key: Key) -> Result<Key, Diagnostic> {
        self.normalized
            .get(&key)
            .map(|r| r.item)
            .ok_or_else(|| error("item normalization input is missing"))
    }
    pub fn local(&self, key: Key) -> Result<Key, Diagnostic> {
        self.normalized
            .get(&key)
            .map(|r| r.local)
            .ok_or_else(|| error("local normalization input is missing"))
    }
    pub fn context(&self, key: Key) -> Result<&ContextRecord, Diagnostic> {
        self.contexts
            .get(&key)
            .ok_or_else(|| error("hygiene context is missing"))
    }
    /// Ordinary item lookup begins at the emitting definition unit, never at
    /// a caller alias. Keep namespace selection explicit in the query key.
    pub fn dollar_crate_bucket(
        &self,
        context: Key,
        components: &[crate::expand::scopes::MacroName],
        namespace: crate::source::manifest::Namespace,
    ) -> Result<crate::db::key::BucketKey, Diagnostic> {
        let unit = self.dollar_crate(context)?;
        let Some((name, modules)) = components.split_last() else {
            return Err(error("empty dollar-crate item path"));
        };
        if components.len() > 256 {
            return Err(error("dollar-crate path depth exceeded"));
        }
        let mut bytes = 0usize;
        for component in components {
            bytes = bytes
                .checked_add(component.as_str().len())
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("dollar-crate path text limit exceeded"))?;
        }
        Ok(crate::db::key::BucketKey {
            scope: crate::syntax::outline::ScopeKey {
                crate_key: crate::core::digest::Digest(unit.0),
                module_path: modules.iter().map(|n| n.as_str().to_owned()).collect(),
                owner: None,
            },
            namespace,
            name: name.as_str().to_owned(),
        })
    }
    /// `$crate` uses the exact emitting definition's authority, even through
    /// renames. Normalize only for name comparison, not for this owner route.
    pub fn dollar_crate(&self, key: Key) -> Result<Key, Diagnostic> {
        let record = self.context(key)?;
        if record.expansion.is_none() || record.transparency == Transparency::Transparent {
            return Err(error("dollar-crate token lacks definition-site authority"));
        }
        Ok(record.defining_unit)
    }
}
