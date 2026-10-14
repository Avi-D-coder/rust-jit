//! Per-publication installed PC table. Hot lookups do not enter the query engine.
use crate::{
    core::diagnostic::Diagnostic,
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    ir::{pc::PcKey, recipe::VerifiedRecipe},
};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RecipeId(u32);
pub struct InstalledRecipes {
    source_revision: u64,
    limit: usize,
    bytes: usize,
    by_pc: BTreeMap<PcKey, RecipeId>,
    entries: Vec<Arc<VerifiedRecipe>>,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
impl InstalledRecipes {
    pub fn new(source_revision: u64, limit: usize) -> Result<Self, Diagnostic> {
        if limit > super::telemetry::INSTALLED_LRIR_BUDGET {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "installed LRIR limit exceeds fixed maximum",
            ));
        }
        Ok(Self {
            source_revision,
            limit,
            bytes: 0,
            by_pc: BTreeMap::new(),
            entries: Vec::new(),
        })
    }
    pub fn source_revision(&self) -> u64 {
        self.source_revision
    }
    pub fn recipes(&self) -> impl Iterator<Item = &VerifiedRecipe> {
        self.entries.iter().map(AsRef::as_ref)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub fn get(&self, id: RecipeId) -> Result<&VerifiedRecipe, Diagnostic> {
        let index =
            id.0.checked_sub(1)
                .ok_or_else(|| fault("LRR-E-INTERNAL-IR", "installed recipe ID is zero"))?;
        self.entries
            .get(index as usize)
            .map(AsRef::as_ref)
            .ok_or_else(|| fault("LRR-E-INTERNAL-IR", "installed recipe ID absent"))
    }
    pub(crate) fn contains_pc(&self, key: &PcKey) -> bool {
        self.by_pc.contains_key(key)
    }
    pub fn resolve(&mut self, db: &mut Database, key: PcKey) -> QueryPoll<RecipeId> {
        if db.source_revision() != self.source_revision {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "installed PC table belongs to another source publication",
            )));
        }
        if let Some(id) = self.by_pc.get(&key) {
            return QueryPoll::Ready(Ok(*id));
        }
        let outcome = match db.fetch(&SyntaxProvider, QueryKey::Thunk(key.clone())) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-IR",
                "thunk query returned wrong installation variant",
            )));
        };
        let install = self.install_prepared(db, key, recipe.as_ref());
        QueryPoll::Ready(install)
    }
    pub(crate) fn install_prepared(
        &mut self,
        db: &Database,
        key: PcKey,
        recipe: &VerifiedRecipe,
    ) -> Result<RecipeId, Diagnostic> {
        if db.source_revision() != self.source_revision {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "prepared installation source publication is stale",
            ));
        }
        if self.by_pc.contains_key(&key) {
            return Err(fault(
                "LRR-E-INTERNAL-IR",
                "prepared installation would replace an installed PC",
            ));
        }

        if recipe.recipe().key != key {
            return Err(fault(
                "LRR-E-INTERNAL-IR",
                "recipe PC mismatch at installation",
            ));
        }
        let index = self
            .entries
            .len()
            .checked_add(1)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "installed recipe ID exhausted"))?;
        let charge = recipe
            .metadata_bytes()
            .checked_add(
                std::mem::size_of::<PcKey>() + 64 + std::mem::size_of::<Arc<VerifiedRecipe>>(),
            )
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "installed recipe size overflow"))?;
        let bytes = self
            .bytes
            .checked_add(charge)
            .filter(|bytes| *bytes <= self.limit)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "installed LRIR budget exhausted"))?;
        self.entries.try_reserve_exact(1).map_err(|_| {
            fault(
                "LRR-E-METADATA-LIMIT",
                "installed recipe table allocation failed",
            )
        })?;
        let owned = Arc::new(recipe.clone());
        let id = RecipeId(index);
        self.entries.push(owned);
        self.by_pc.insert(key, id);
        self.bytes = bytes;
        Ok(id)
    }
}
