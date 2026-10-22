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
    enum_casts: Arc<[(usize, crate::ir::pc::SlotId)]>,
}
pub struct InstalledRecipes {
    source_revision: u64,
    /// Installed metadata observation only; no LRIR budget (E-R5-006).
    bytes: usize,
    by_pc: BTreeMap<PcKey, RecipeId>,
    /// The same PC -> recipe mapping, hashed: the per-dispatch membership and
    /// resolution lookups use it, while the ordered map above serves the
    /// range queries of call renewal. Both change only in `install_ready`.
    lookup: rustc_hash::FxHashMap<PcKey, RecipeId>,
    entries: Vec<InstalledRecipe>,
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
impl InstalledRecipes {
    // E-R5-006: ignored, remove at callers (`_limit`; no installed LRIR budget).
    pub fn new(source_revision: u64, _limit: usize) -> Result<Self, Diagnostic> {
        Ok(Self {
            source_revision,
            bytes: 0,
            by_pc: BTreeMap::new(),
            lookup: Default::default(),
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
    /// Retained ordinary LRIR positions; no source planning on preparation/resume.
    pub(crate) fn enum_casts(
        &self,
        db: &Database,
        id: RecipeId,
    ) -> Result<Arc<[(usize, crate::ir::pc::SlotId)]>, Diagnostic> {
        self.get_current(db, id)?;
        Ok(Arc::clone(&self.entry(id)?.enum_casts))
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
        let entry = self.entry(id)?;
        let recipe = entry.recipe.as_ref();
        recipe.revalidate_namespace(db)?;
        if let Some(constructor) = &entry.constructor {
            constructor.inputs.require_current(db)?;
        }
        Ok(recipe)
    }
    /// Demand-driven source publication grows the candidate sets of scope
    /// certificate selections that an earlier installed phase read (a reached
    /// inference root certifying more of the world). That supersedes the phase's
    /// captured inputs without changing what its ordinary lowering answers.
    /// Renew only when the freshly demanded recipe is identical operation for
    /// operation; any difference is a genuine source change and stays stale.
    fn renew_installed(&mut self, db: &mut Database, id: RecipeId, key: &PcKey) -> QueryPoll<()> {
        use crate::core::digest::StableEncode;
        let stale = match self.get_current(db, id) {
            Ok(_) => return QueryPoll::Ready(Ok(())),
            Err(error) if error.prepared_input_change.is_some() => error,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let index = (id.0 - 1) as usize;
        let fresh = match db.fetch(&SyntaxProvider, QueryKey::Thunk(key.clone())) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(_)) => return QueryPoll::Ready(Err(stale)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::Thunk(recipe) = fresh.value.as_ref() else {
            return QueryPoll::Ready(Err(stale));
        };
        if !fresh.cacheable
            || recipe.recipe().key != *key
            || recipe.recipe().fingerprint() != self.entries[index].recipe.recipe().fingerprint()
        {
            return QueryPoll::Ready(Err(stale));
        }
        self.entries[index].recipe = Arc::new(recipe.as_ref().clone());
        match self.renew_constructor_guard(db, id) {
            QueryPoll::Ready(Ok(())) => QueryPoll::Ready(Ok(())),
            other => other,
        }
    }
    /// Demand-driven source publication (the scope certificate candidate sets a
    /// reached inference root grows) can supersede the guard an installed record
    /// constructor captured, without changing what that constructor's prepared
    /// query answers. Renew the guard only when the freshly demanded query value
    /// is identical; any difference is a genuine source change and stays stale.
    pub(crate) fn renew_constructor_guard(
        &mut self,
        db: &mut Database,
        id: RecipeId,
    ) -> QueryPoll<()> {
        use crate::core::digest::StableEncode;
        let Some(index) = id.0.checked_sub(1).map(|index| index as usize) else {
            return QueryPoll::Ready(Err(fault("LRR-E-INTERNAL-IR", "installed recipe ID is zero")));
        };
        let Some(constructor) = self.entries.get(index).and_then(|entry| entry.constructor.as_ref()) else {
            return QueryPoll::Ready(Ok(()));
        };
        if constructor.inputs.require_current(db).is_ok() {
            return QueryPoll::Ready(Ok(()));
        }
        let key = constructor.source.prepared().key().clone();
        let before = StableEncode::fingerprint(constructor.source.prepared());
        let demand = QueryKey::BoundRecordConstructor(key.clone());
        let fresh = match db.fetch(&SyntaxProvider, demand.clone()) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let unchanged = fresh.cacheable
            && matches!(fresh.value.as_ref(), QueryValue::BoundRecordConstructor(value)
                if StableEncode::fingerprint(value.as_ref()) == before);
        if !unchanged {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "installed record constructor source changed",
            )));
        }
        let inputs = match db.prepare_input_guard(&SyntaxProvider, &demand, fresh.fingerprint) {
            QueryPoll::Ready(Ok(inputs)) => inputs,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let source = match PreparedRecordConstructorHandle::from_query(fresh.value, &key) {
            Ok(source) => source,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        self.entries[index].constructor = Some(InstalledRecordConstructor { source, inputs });
        QueryPoll::Ready(Ok(()))
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
        let Some(id) = self.lookup.get(pc) else {
            return Ok(None);
        };
        Ok(self.get(*id)?.source_call())
    }
    pub(crate) fn contains_pc(&self, key: &PcKey) -> bool {
        self.lookup.contains_key(key)
    }
    pub fn resolve(&mut self, db: &mut Database, key: PcKey) -> QueryPoll<RecipeId> {
        if db.source_revision() != self.source_revision {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "installed PC table belongs to another source publication",
            )));
        }
        if let Some(id) = self.lookup.get(&key).copied() {
            // A current recipe (the hot case) is checked once. A reached
            // inference root this constructor's preparation ran (and the input
            // publication it requested) can supersede the guard the installed
            // constructor captured without changing what it answers: renew it.
            match self.get_current(db, id) {
                Ok(_) => return QueryPoll::Ready(Ok(id)),
                Err(error) if error.prepared_input_change.is_some() => {}
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
            match self.renew_installed(db, id, &key) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
            return QueryPoll::Ready(self.get_current(db, id).map(|_| id));
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
                crate::syntax::constructors::ConstructorKind::Record { .. }
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
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "installed recipe size overflow"))?;
        self.entries.try_reserve(1).map_err(|_| {
            fault(
                "LRR-E-METADATA-LIMIT",
                "installed recipe table allocation failed",
            )
        })?;
        let owned = Arc::new(recipe.clone());
        let enum_casts = recipe.recipe().operations.iter().enumerate().filter_map(|(index, op)| {
            match op {
                crate::ir::op::Op::PrimitiveCast { source, .. } => Some((index, *source)),
                _ => None,
            }
        }).collect::<Arc<[_]>>();
        let id = RecipeId(index);
        self.entries.push(InstalledRecipe {
            recipe: owned,
            constructor,
            enum_casts,
        });
        self.lookup.insert(key.clone(), id);
        self.by_pc.insert(key, id);
        self.bytes = bytes;
        Ok(id)
    }
}
