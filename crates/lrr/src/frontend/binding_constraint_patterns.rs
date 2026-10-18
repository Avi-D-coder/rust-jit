//! Written pattern bindings whose entered Value goal is not yet answered.
//!
//! The entered Value goal of an identifier pattern classifies the name
//! (fresh binding, or constant / unit constructor in scope) through the
//! module's published name answers and prelude receipt. The constraint
//! demand for a literal runs before the reached code publishes them for the
//! patterns of its function, so such a goal is still pending when asked.
//! The pattern engine's retained occurrence rows state the same pattern's
//! hygienic local identity without that classification (the identity every
//! use of the binding selects), and the classification itself is the
//! dispatcher's own pattern-name selection (`pattern_names`) over the body's
//! module context. Nothing here guesses a name's meaning: an identifier that
//! names a constant or constructor, or a row that is not resolved, stays an
//! unanswered goal.
use super::entry;
use crate::{
    core::digest::Digest,
    db::{memo::Database, poll::QueryPoll},
    frontend::{
        callables::CallResolver,
        pattern_binding_occurrences::PatternLocalOccurrence,
        pattern_names::{PatternSelection, select_pattern_name},
        source_value_goals::EnteredValueGoal,
    },
    ir::pc::{NodeId, PcKey},
    syntax::{
        pattern_bindings::BindingUncertainty,
        pattern_tree::{NameForm, PatternName},
    },
};

/// The binding identity of the identifier pattern `pattern`, whose entered
/// Value goal `goal` is pending, from the retained occurrence rows.
pub(super) fn unclassified_binding(
    db: &mut Database,
    key: &PcKey,
    pattern: NodeId,
    goal: &EnteredValueGoal,
) -> QueryPoll<Option<Digest>> {
    if goal.pending().is_none() || goal.node() != pattern || goal.key() != &entry(key, pattern) {
        return QueryPoll::Ready(Ok(None));
    }
    let plan = goal.retained_plan();
    let rows = plan.pattern_occurrences().rows();
    let Ok(position) = rows.binary_search_by_key(&pattern, |row| row.pattern()) else {
        return QueryPoll::Ready(Ok(None));
    };
    let row = &rows[position];
    // TEMPTRACE
    if std::env::var_os("LRR_NUM_TRACE").is_some() {
        eprintln!("NUMTRACE fallback row node={} resolved={} local={:?} uncertainty={:?} canonical={:?}", pattern.0, row.conflicts().is_resolved(), row.local_source(), row.uncertainty(), row.canonical_candidate());
    }
    if !row.conflicts().is_resolved() {
        return QueryPoll::Ready(Ok(None));
    }
    let PatternLocalOccurrence::Joined { identity, .. } = row.local_source() else {
        return QueryPoll::Ready(Ok(None));
    };
    match row.uncertainty() {
        BindingUncertainty::ExplicitBinding => QueryPoll::Ready(Ok(Some(identity))),
        // A plain identifier is a fresh binding unless a constant or
        // constructor of that name is in scope.
        BindingUncertainty::BindingOrConstant => {
            let Some(environment) = plan.environment() else {
                return QueryPoll::Ready(Ok(None));
            };
            let environment = environment.macros().source_environment();
            let sources: Vec<_> = environment.sources().iter().collect();
            let resolver = match CallResolver::new(environment.names(), &sources) {
                Ok(resolver) => resolver,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let name = PatternName {
                spelling: row.name().to_owned(),
                form: NameForm::Identifier,
                lexical: None,
            };
            match select_pattern_name(
                &resolver,
                environment.names(),
                &sources,
                db,
                key.body.definition,
                &name,
            ) {
                QueryPoll::Ready(Ok((_, PatternSelection::Binding))) => {
                    QueryPoll::Ready(Ok(Some(identity)))
                }
                QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(None)),
                QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                    QueryPoll::Ready(Ok(None))
                }
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            }
        }
    }
}
