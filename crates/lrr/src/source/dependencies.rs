//! Immutable source-world dependency aliases supplied during explicit preparation.
use super::changes::SourceSelection;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::scopes::MacroName,
};
use std::collections::BTreeMap;
#[derive(Debug, Clone)]
pub struct SourceDependency {
    pub consumer: Digest,
    pub alias: String,
    pub provider: Digest,
}
#[derive(Debug)]
pub struct SourceDependencies {
    roots: BTreeMap<Digest, SourceSelection>,
    aliases: BTreeMap<(Digest, String), Digest>,
    by_consumer: BTreeMap<Digest, std::sync::Arc<BTreeMap<MacroName, Digest>>>,
    empty: std::sync::Arc<BTreeMap<MacroName, Digest>>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNIT-GRAPH", message.into())
}
impl SourceDependencies {
    pub(crate) fn prepare(
        roots: Vec<SourceSelection>,
        rows: Vec<SourceDependency>,
    ) -> Result<Self, Diagnostic> {
        if roots.len() > 100_000 || rows.len() > 100_000 {
            return Err(fault("source dependency graph count limit"));
        }
        let mut graph = Self {
            roots: BTreeMap::new(),
            aliases: BTreeMap::new(),
            by_consumer: BTreeMap::new(),
            empty: std::sync::Arc::new(BTreeMap::new()),
        };
        let mut bytes = 0usize;
        for root in roots {
            bytes = bytes
                .checked_add(root.metadata_bytes()?)
                .and_then(|n| n.checked_add(128))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("source dependency root metadata limit"))?;
            if !root.outline.module_path.is_empty()
                || root.canonical()? != root
                || graph.roots.insert(root.outline.crate_key, root).is_some()
            {
                return Err(fault(
                    "source dependency root identity is invalid or duplicated",
                ));
            }
        }
        for row in rows {
            if row.alias.len() > 4096 {
                return Err(fault("source dependency alias text limit"));
            }
            let root = graph
                .roots
                .get(&row.consumer)
                .ok_or_else(|| fault("source dependency consumer root is not captured"))?;
            if !graph.roots.contains_key(&row.provider) {
                return Err(fault("source dependency provider root is not captured"));
            }
            let alias =
                crate::frontend::names::owned_name(&row.alias, root.outline.parse.profile.edition)?
                    .as_str()
                    .to_owned();
            bytes = bytes
                .checked_add(
                    alias
                        .len()
                        .checked_mul(2)
                        .ok_or_else(|| fault("source dependency alias size overflow"))?,
                )
                .and_then(|n| n.checked_add(384))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("source dependency alias metadata limit"))?;
            if let Some(old) = graph.aliases.insert((row.consumer, alias), row.provider)
                && old != row.provider
            {
                return Err(fault("source dependency alias has conflicting providers"));
            }
        }
        let mut maps = BTreeMap::<Digest, BTreeMap<MacroName, Digest>>::new();
        for ((consumer, alias), provider) in &graph.aliases {
            let root = &graph.roots[consumer];
            maps.entry(*consumer).or_default().insert(
                crate::frontend::names::owned_name(alias, root.outline.parse.profile.edition)?,
                *provider,
            );
        }
        graph.by_consumer = maps
            .into_iter()
            .map(|(unit, rows)| (unit, std::sync::Arc::new(rows)))
            .collect();
        Ok(graph)
    }
    pub(crate) fn aliases_for(
        &self,
        consumer: Digest,
        edition: u16,
    ) -> Result<std::sync::Arc<BTreeMap<MacroName, Digest>>, Diagnostic> {
        if self.by_consumer.contains_key(&consumer)
            && self
                .roots
                .get(&consumer)
                .is_none_or(|root| root.outline.parse.profile.edition != edition)
        {
            return Err(fault("source dependency consumer edition differs"));
        }
        Ok(self
            .by_consumer
            .get(&consumer)
            .unwrap_or(&self.empty)
            .clone())
    }
    pub(crate) fn rows(&self) -> impl Iterator<Item = (Digest, &str, Digest)> {
        self.aliases
            .iter()
            .map(|((unit, name), target)| (*unit, name.as_str(), *target))
    }
    pub(crate) fn copy_rows(&self) -> Result<Vec<SourceDependency>, Diagnostic> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.aliases.len())
            .map_err(|_| fault("source dependency refresh allocation"))?;
        for (consumer, alias, provider) in self.rows() {
            rows.push(SourceDependency {
                consumer,
                alias: alias.into(),
                provider,
            });
        }
        Ok(rows)
    }
    pub(crate) fn has_aliases(&self) -> bool {
        !self.aliases.is_empty()
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        for root in self.roots.values() {
            bytes = bytes
                .checked_add(root.metadata_bytes()?)
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("source dependency metadata overflow"))?;
        }
        for (_, name) in self.aliases.keys() {
            bytes = bytes
                .checked_add(name.capacity())
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("source dependency metadata overflow"))?;
        }
        for rows in self.by_consumer.values() {
            bytes = bytes
                .checked_add(128)
                .ok_or_else(|| fault("source dependency map metadata overflow"))?;
            for name in rows.keys() {
                bytes = bytes
                    .checked_add(name.as_str().len())
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| fault("source dependency map metadata overflow"))?;
            }
        }
        Ok(bytes)
    }
}
impl StableEncode for SourceDependencies {
    const DOMAIN: &'static str = "lrr/v2/captured-source-dependencies";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u64(u64::try_from(self.roots.len()).expect("bounded source roots"));
        for (unit, root) in &self.roots {
            e.digest(*unit);
            e.string(root.path.as_str());
            e.record(&crate::db::key::QueryKey::Outline(root.outline.clone()));
        }
        e.u64(u64::try_from(self.aliases.len()).expect("bounded source aliases"));
        for ((unit, name), provider) in &self.aliases {
            e.digest(*unit);
            e.string(name);
            e.digest(*provider);
        }
    }
}
