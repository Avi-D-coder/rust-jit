//! Per-publication installed PC table. Hot lookups do not enter the query engine.
use crate::{
    core::diagnostic::Diagnostic,
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    frontend::prepared_constructors::PreparedRecordConstructorHandle,
    ir::{pc::PcKey, recipe::VerifiedRecipe},
};
use std::{collections::BTreeMap, sync::Arc};
#[path = "install_call_renewal.rs"]
mod call_renewal;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RecipeId(u32);
struct InstalledRecordConstructor {
    source: PreparedRecordConstructorHandle,
    inputs: crate::db::memo::PreparedInputGuard,
}
struct InstalledRecipe {
    recipe: Arc<VerifiedRecipe>,
    constructor: Option<InstalledRecordConstructor>,
}
pub struct InstalledRecipes {
    source_revision: u64,
    limit: usize,
    bytes: usize,
    by_pc: BTreeMap<PcKey, RecipeId>,
    entries: Vec<InstalledRecipe>,
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
        self.entries.iter().map(|entry| entry.recipe.as_ref())
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
        Ok(self.entry(id)?.recipe.as_ref())
    }
    fn entry(&self, id: RecipeId) -> Result<&InstalledRecipe, Diagnostic> {
        let index =
            id.0.checked_sub(1)
                .ok_or_else(|| fault("LRR-E-INTERNAL-IR", "installed recipe ID is zero"))?;
        self.entries
            .get(index as usize)
            .ok_or_else(|| fault("LRR-E-INTERNAL-IR", "installed recipe ID absent"))
    }
    /// Normal interpreter execution must use this accessor, including resume.
    /// `get` remains a data inspection API, not a currentness authorization.
    pub(crate) fn get_current(
        &self,
        db: &Database,
        id: RecipeId,
    ) -> Result<&VerifiedRecipe, Diagnostic> {
        if db.source_revision() != self.source_revision {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "installed PC table belongs to another source publication",
            ));
        }
        let recipe = self.get(id)?;
        recipe.revalidate_namespace(db)?;
        if let Some(constructor) = &self.entry(id)?.constructor {
            constructor.inputs.require_current(db)?;
        }
        Ok(recipe)
    }
    pub(crate) fn prepared_constructor(
        &self,
        db: &Database,
        id: RecipeId,
    ) -> Result<Option<PreparedRecordConstructorHandle>, Diagnostic> {
        // This only borrows immutable source facts. The dispatcher separately
        // authorizes the executable recipe through get_current. Do not repeat
        // its historical namespace census while preparing runtime frame facts.
        if db.source_revision() != self.source_revision {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "installed constructor source publication is stale",
            ));
        }
        let constructor = self.entry(id)?.constructor.as_ref();
        if let Some(value) = constructor {
            value.inputs.require_current(db)?;
        }
        Ok(constructor.map(|value| value.source.clone()))
    }
    /// Borrow retained source-call evidence at the actual installed PC. The
    /// dispatcher already validated the recipe; this accessor adds no query,
    /// source planning or historical namespace-census work.
    pub(crate) fn source_call(
        &self,
        db: &Database,
        pc: &PcKey,
    ) -> Result<
        Option<&crate::frontend::explicit_type_arguments::PreparedExplicitTypeArguments>,
        Diagnostic,
    > {
        if db.source_revision() != self.source_revision {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "installed source call publication differs",
            ));
        }
        let Some(id) = self.by_pc.get(pc) else {
            return Ok(None);
        };
        Ok(self.get(*id)?.source_call())
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
            return QueryPoll::Ready(self.get_current(db, *id).map(|_| *id));
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
        self.prepare_install(db, key, recipe.as_ref())
    }
    /// Cold installation joins the actual prepared source binding before the
    /// PC is published. NeedInput cannot leave a half-installed recipe.
    pub(crate) fn prepare_install(
        &mut self,
        db: &mut Database,
        key: PcKey,
        recipe: &VerifiedRecipe,
    ) -> QueryPoll<RecipeId> {
        if db.source_revision() != self.source_revision {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "prepared installation source publication is stale",
            )));
        }
        if recipe.recipe().key != key || self.by_pc.contains_key(&key) {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-IR",
                "prepared installation PC mismatch or duplicate",
            )));
        }
        let mut constructor = None;
        if recipe
            .recipe()
            .operations
            .iter()
            .any(|op| matches!(op, crate::ir::op::Op::ConstructStart { .. }))
            && db
                .published_input(&crate::db::dependency::InputKey::BodyEnvironment(
                    key.body.clone(),
                ))
                .is_some()
        {
            let syntax = match db.fetch(&SyntaxProvider, QueryKey::ConstructorPlan(key.clone())) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let QueryValue::ConstructorPlan(plan) = syntax.value.as_ref() else {
                return QueryPoll::Ready(Err(fault(
                    "LRR-E-INTERNAL-IR",
                    "installed constructor plan variant differs",
                )));
            };
            if matches!(
                plan.kind(),
                crate::syntax::constructors::ConstructorKind::Record { rest: None, .. }
            ) {
                let demand = QueryKey::BoundRecordConstructor(key.clone());
                let prepared = match db.fetch(&SyntaxProvider, demand.clone()) {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                if !prepared.cacheable {
                    return QueryPoll::Ready(Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "uncacheable constructor cannot install",
                    )));
                }
                let inputs =
                    match db.prepare_input_guard(&SyntaxProvider, &demand, prepared.fingerprint) {
                        QueryPoll::Ready(Ok(inputs)) => inputs,
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    };
                let source = match PreparedRecordConstructorHandle::from_query(prepared.value, &key)
                {
                    Ok(source) => source,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                constructor = Some(InstalledRecordConstructor { source, inputs });
            }
        }
        QueryPoll::Ready(self.install_ready(db, key, recipe, constructor))
    }
    #[cfg(test)]
    pub(crate) fn install_prepared(
        &mut self,
        db: &Database,
        key: PcKey,
        recipe: &VerifiedRecipe,
    ) -> Result<RecipeId, Diagnostic> {
        self.install_ready(db, key, recipe, None)
    }
    fn install_ready(
        &mut self,
        db: &Database,
        key: PcKey,
        recipe: &VerifiedRecipe,
        constructor: Option<InstalledRecordConstructor>,
    ) -> Result<RecipeId, Diagnostic> {
        if db.source_revision() != self.source_revision {
            return Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "prepared installation source publication is stale",
            ));
        }
        recipe.revalidate_namespace(db)?;
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
            .checked_add(std::mem::size_of::<PcKey>() + 64 + std::mem::size_of::<InstalledRecipe>())
            .and_then(|bytes| {
                bytes.checked_add(
                    constructor
                        .as_ref()
                        .map_or(0, |value| value.inputs.metadata_bytes()),
                )
            })
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
        self.entries.push(InstalledRecipe {
            recipe: owned,
            constructor,
        });
        self.by_pc.insert(key, id);
        self.bytes = bytes;
        Ok(id)
    }
}
