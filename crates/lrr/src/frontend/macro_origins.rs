//! Expansion-order ancestry is separate from syntax-context ancestry. This
//! stage consumes candidates already filtered for namespace/name hygiene.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::scopes::MacroDefinition,
    syntax::token_wire::Key,
};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MacroCandidate {
    pub definition: MacroDefinition,
    pub origin: Option<Key>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MacroSelection {
    Defined(MacroDefinition),
    Absent,
    Ambiguous(Vec<MacroDefinition>),
}
impl MacroSelection {
    pub fn require_definition(self) -> Result<MacroDefinition, Diagnostic> {
        match self {
            Self::Defined(definition) => Ok(definition),
            Self::Absent => Err(error("macro name is absent")),
            Self::Ambiguous(_) => Err(error("macro name is ambiguous between expansion origins")),
        }
    }
}
#[derive(Default, Clone)]
pub struct ExpansionAncestry {
    parents: BTreeMap<Key, Option<Key>>,
    depths: BTreeMap<Key, usize>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl ExpansionAncestry {
    pub(crate) fn contains(&self, origin: Key) -> bool {
        self.parents.contains_key(&origin)
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.parents
            .len()
            .checked_mul(128)
            .ok_or_else(|| error("expansion ancestry metadata overflow"))
    }

    /// Root-to-leaf producer records from the explicit expansion-order table.
    pub(crate) fn chain(&self, origin: Key) -> Result<Vec<Digest>, Diagnostic> {
        self.validate(Some(origin))?;
        let mut chain = Vec::new();
        chain
            .try_reserve_exact(
                *self
                    .depths
                    .get(&origin)
                    .ok_or_else(|| error("expansion ancestry depth absent"))?,
            )
            .map_err(|_| error("expansion ancestry allocation"))?;
        let mut cursor = Some(origin);
        while let Some(key) = cursor {
            if chain.len() >= 256 {
                return Err(error("expansion ancestry depth limit"));
            }
            chain.push(Digest(key.0));
            cursor = *self
                .parents
                .get(&key)
                .ok_or_else(|| error("expansion ancestry parent absent"))?;
        }
        chain.reverse();
        Ok(chain)
    }

    pub fn register(&mut self, key: Key, parent: Option<Key>) -> Result<(), Diagnostic> {
        if let Some(old) = self.parents.get(&key) {
            return if *old == parent {
                Ok(())
            } else {
                Err(error("conflicting expansion-order parent"))
            };
        }
        let depth = match parent {
            None => 1,
            Some(parent) => self
                .depths
                .get(&parent)
                .copied()
                .ok_or_else(|| error("expansion-order parent input is missing"))?
                .checked_add(1)
                .ok_or_else(|| error("expansion-order depth overflow"))?,
        };
        if depth > 256 || self.parents.len() >= 1_000_000 {
            return Err(error("expansion-order table limit exceeded"));
        }
        self.parents.insert(key, parent);
        self.depths.insert(key, depth);
        Ok(())
    }
    /// Join explicit producer ancestry only; syntax-context marks never supply parents.
    pub(crate) fn extend(&mut self, other: &Self) -> Result<(), Diagnostic> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(other.parents.len())
            .map_err(|_| error("expansion-order join allocation failed"))?;
        for (key, parent) in &other.parents {
            rows.push((other.depths[key], *key, *parent));
        }
        rows.sort();
        let mut joined = self.clone();
        for (_, key, parent) in rows {
            joined.register(key, parent)?;
        }
        *self = joined;
        Ok(())
    }
    fn validate(&self, origin: Option<Key>) -> Result<(), Diagnostic> {
        if origin.is_some_and(|key| !self.parents.contains_key(&key)) {
            return Err(error("expansion-order input is missing"));
        }
        Ok(())
    }
    pub fn descendant_of(
        &self,
        mut descendant: Option<Key>,
        ancestor: Option<Key>,
    ) -> Result<bool, Diagnostic> {
        self.validate(descendant)?;
        self.validate(ancestor)?;
        for _ in 0..=256 {
            if descendant == ancestor {
                return Ok(true);
            }
            let Some(key) = descendant else {
                return Ok(false);
            };
            descendant = *self
                .parents
                .get(&key)
                .ok_or_else(|| error("expansion-order ancestor input is missing"))?;
        }
        Err(error("expansion-order traversal limit exceeded"))
    }
    pub fn fingerprint(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/expansion-order");
        e.u64(u64::try_from(self.parents.len()).expect("bounded expansion-order table"));
        for (key, parent) in &self.parents {
            e.bytes(&key.0);
            e.option(parent, |e, p| e.bytes(&p.0));
        }
        e.finish()
    }
    /// Candidates are ordered from the innermost/latest textual binding outward.
    /// A declaration can shadow only when its expansion certainly precedes the
    /// reference or the outer declaration in the expansion partial order.
    /// Missing producer/name-compatibility facts must suspend before this stage.
    pub fn select_compatible(
        &self,
        reference: Option<Key>,
        candidates: &[MacroCandidate],
    ) -> Result<MacroSelection, Diagnostic> {
        if candidates.len() > 1_000_000 {
            return Err(error("macro candidate limit exceeded"));
        }
        self.validate(reference)?;
        // Reject malformed inputs even if a nearer binding could hide them.
        let mut seen = BTreeMap::new();
        for candidate in candidates {
            candidate.definition.validate()?;
            self.validate(candidate.origin)?;
            if let Some(old) = seen.insert(candidate.definition.identity, candidate)
                && old != candidate
            {
                return Err(error("conflicting macro candidate identity"));
            }
        }
        let Some(first) = candidates.first() else {
            return Ok(MacroSelection::Absent);
        };
        for other in &candidates[1..] {
            if first.definition.identity == other.definition.identity {
                if first.definition != other.definition || first.origin != other.origin {
                    return Err(error("conflicting macro candidate identity"));
                }
                continue;
            }
            if !self.descendant_of(other.origin, first.origin)?
                && !self.descendant_of(reference, first.origin)?
            {
                let mut definitions = vec![first.definition.clone(), other.definition.clone()];
                definitions.sort();
                return Ok(MacroSelection::Ambiguous(definitions));
            }
        }
        Ok(MacroSelection::Defined(first.definition.clone()))
    }
}
