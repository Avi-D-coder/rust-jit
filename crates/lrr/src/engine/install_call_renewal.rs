//! Cold atomic renewal of already installed phases of one actual source call.
//! No argument evaluation, frame mutation, guard refresh or future phase walk.
use super::*;
use crate::{
    db::memo::RetainedCallBodyTransition,
    frontend::explicit_type_arguments::PreparedExplicitTypeArguments,
    syntax::body_index::BodyIndex,
};
#[derive(Clone, Copy)]
enum RenewalScope {
    Publisher,
    Observer,
}
struct Replacement {
    index: usize,
    recipe: Arc<VerifiedRecipe>,
}
fn invalid(message: &str) -> Diagnostic {
    fault("LRR-E-INTERNAL-SNAPSHOT", message)
}
fn recipe_charge(recipe: &VerifiedRecipe) -> Result<usize, Diagnostic> {
    recipe
        .metadata_bytes()
        .checked_add(std::mem::size_of::<PcKey>() + 64 + std::mem::size_of::<InstalledRecipe>())
        .ok_or_else(crate::runtime::arena::resource)
}
impl InstalledRecipes {
    /// Only an actual target-publication receipt permits replacing stale
    /// source-call evidence. New ordinary lowering must produce exactly the
    /// same executable operations/continuation for every already entered phase.
    /// RecipeIds remain stable. NeedInput or a failed preflight swaps nothing.
    pub(crate) fn renew_source_call(
        &mut self,
        db: &mut Database,
        transition: &RetainedCallBodyTransition,
        before: &PreparedExplicitTypeArguments,
        body: &BodyIndex,
        pc: &PcKey,
    ) -> QueryPoll<PreparedExplicitTypeArguments> {
        self.renew_call(db, transition, before, body, pc, RenewalScope::Publisher)
    }
    /// Renew another already selected call to the same published target using
    /// that observer's own genuine prior target stamps and source contract.
    pub(crate) fn renew_observed_source_call(
        &mut self,
        db: &mut Database,
        transition: &RetainedCallBodyTransition,
        before: &PreparedExplicitTypeArguments,
        body: &BodyIndex,
        pc: &PcKey,
    ) -> QueryPoll<PreparedExplicitTypeArguments> {
        self.renew_call(db, transition, before, body, pc, RenewalScope::Observer)
    }
    fn renew_call(
        &mut self,
        db: &mut Database,
        transition: &RetainedCallBodyTransition,
        before: &PreparedExplicitTypeArguments,
        body: &BodyIndex,
        pc: &PcKey,
        scope: RenewalScope,
    ) -> QueryPoll<PreparedExplicitTypeArguments> {
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        if db.source_revision() != self.source_revision || !self.by_pc.contains_key(pc) {
            return QueryPoll::Ready(Err(invalid("call renewal has no current installed site")));
        }
        checked!(before.require_site(body, pc));
        checked!(match scope {
            RenewalScope::Publisher => transition.require_before_plan(db, before),
            RenewalScope::Observer => transition.require_observer_plan(db, before),
        });
        let rebound = |db: &mut Database, fresh: &PreparedExplicitTypeArguments| match scope {
            RenewalScope::Publisher => transition.require_rebound(db, before, fresh, body, pc),
            RenewalScope::Observer => {
                transition.require_observer_rebound(db, before, fresh, body, pc)
            }
        };
        let result = match db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedExplicitTypeArguments(before.key().clone()),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::PreparedExplicitTypeArguments(fresh) = result.value.as_ref() else {
            return QueryPoll::Ready(Err(invalid("call renewal Type result variant differs")));
        };
        if !result.cacheable {
            return QueryPoll::Ready(Err(invalid("call renewal Type result is not retained")));
        }
        checked!(rebound(db, fresh));
        // A bounded BTreeMap range selects this node's installed phases. Never
        // scan all installed bodies or prepare unentered phases.
        let mut first = before.key().clone();
        first.phase = 0;
        let mut last = first.clone();
        last.phase = u16::MAX;
        let count = self.by_pc.range(first.clone()..=last.clone()).count();
        if count == 0 {
            return QueryPoll::Ready(Err(invalid("call renewal has no installed phase")));
        }
        let mut replacements = Vec::<Replacement>::new();
        checked!(
            replacements
                .try_reserve_exact(count)
                .map_err(|_| crate::runtime::arena::resource())
        );
        let mut old_charge = 0usize;
        let mut new_charge = 0usize;
        for (key, id) in self.by_pc.range(first..=last) {
            let old = checked!(self.entry(*id));
            if old.constructor.is_some()
                || old
                    .recipe
                    .source_call()
                    .is_none_or(|plan| plan.fingerprint() != before.fingerprint())
            {
                return QueryPoll::Ready(Err(invalid(
                    "call renewal encountered another installed route",
                )));
            }
            checked!(match scope {
                RenewalScope::Publisher => old.recipe.require_call_body_transition(db, transition),
                RenewalScope::Observer => old.recipe.require_call_body_observer(db, transition),
            });
            let value = match db.fetch(&SyntaxProvider, QueryKey::Thunk(key.clone())) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let QueryValue::Thunk(recipe) = value.value.as_ref() else {
                return QueryPoll::Ready(Err(invalid("call renewal thunk variant differs")));
            };
            let previous = old.recipe.recipe();
            let next = recipe.recipe();
            if !value.cacheable
                || previous.key != next.key
                || previous.origin != next.origin
                || previous.operations != next.operations
                || previous.exit != next.exit
                || recipe
                    .source_call()
                    .is_none_or(|plan| plan.fingerprint() != fresh.fingerprint())
                || recipe.prepared_inputs_fingerprint() != Some(fresh.guard().fingerprint())
            {
                return QueryPoll::Ready(Err(invalid(
                    "call renewal changed executable shape or source route",
                )));
            }
            checked!(recipe.revalidate_namespace(db));
            old_charge = checked!(
                old_charge
                    .checked_add(checked!(recipe_charge(&old.recipe)))
                    .ok_or_else(crate::runtime::arena::resource)
            );
            new_charge = checked!(
                new_charge
                    .checked_add(checked!(recipe_charge(recipe)))
                    .ok_or_else(crate::runtime::arena::resource)
            );
            let index = checked!(
                id.0.checked_sub(1)
                    .map(|i| i as usize)
                    .ok_or_else(|| invalid("call renewal recipe ID is zero"))
            );
            replacements.push(Replacement {
                index,
                recipe: Arc::new(recipe.as_ref().clone()),
            });
        }
        let bytes = checked!(
            self.bytes
                .checked_sub(old_charge)
                .and_then(|bytes| bytes.checked_add(new_charge))
                .ok_or_else(crate::runtime::arena::resource)
        );
        // All fallible work and relevant currentness checks precede the swap.
        checked!(rebound(db, fresh));
        let selected = fresh.as_ref().clone();
        for replacement in replacements {
            self.entries[replacement.index].recipe = replacement.recipe;
        }
        self.bytes = bytes;
        QueryPoll::Ready(Ok(selected))
    }
}
