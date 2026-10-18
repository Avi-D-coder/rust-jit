//! Published runtime Drop facts from one source member world. Publication
//! updates only new impl rows; execution never rebuilds the namespace census.
//! This handle does not certify completeness of unentered impl producers.
use crate::core::digest::Digest;
use std::{collections::BTreeSet, sync::RwLock};

#[derive(Debug)]
pub(crate) struct DropFacts {
    types: RwLock<Option<BTreeSet<Digest>>>,
}
impl DropFacts {
    pub(crate) fn new(types: Option<BTreeSet<Digest>>) -> Self {
        Self {
            types: RwLock::new(types),
        }
    }
    pub(crate) fn known(&self) -> bool {
        self.types.read().is_ok_and(|types| types.is_some())
    }
    pub(crate) fn contains(&self, identity: &Digest) -> bool {
        self.types
            .read()
            .is_ok_and(|types| types.as_ref().is_some_and(|types| types.contains(identity)))
    }
    // Retains the existing cleanup/promotion inventory classification. It is
    // not a certificate of complete source-level absence for template caching.
    pub(crate) fn definitely_no_drop(&self, identity: &Digest) -> bool {
        self.types.read().is_ok_and(|types| {
            types
                .as_ref()
                .is_some_and(|types| !types.contains(identity))
        })
    }
    /// Called under the member world's existing publication lock. Unresolved
    /// Drop rows keep the inventory unknown rather than proving absence.
    pub(crate) fn append(&self, added: Option<BTreeSet<Digest>>) {
        let mut current = self.types.write().unwrap_or_else(|p| p.into_inner());
        match (&mut *current, added) {
            (Some(current), Some(added)) => {
                current.extend(added);
            }
            (Some(_), None) => {
                *current = None;
            }
            (None, _) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn published_drop_facts_track_related_and_unrelated_additions() {
        let facts = DropFacts::new(Some(BTreeSet::new()));
        let target = Digest([11; 32]);
        facts.append(Some(BTreeSet::from([Digest([12; 32])])));
        assert!(!facts.contains(&target));
        facts.append(Some(BTreeSet::from([target])));
        assert!(facts.contains(&target));
        assert!(!facts.definitely_no_drop(&target));
        facts.append(None);
        assert!(!facts.known());
        assert!(!facts.definitely_no_drop(&Digest([13; 32])));
    }
}
