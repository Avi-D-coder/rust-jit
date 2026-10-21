#!/usr/bin/env python3
"""Audit/project pinned rustc next-solver root fulfillment over its existing owner.

The progress loop, registration and pending filters are selected from exact
compiler bytes. Representation joins bind one safe SolverContext reference,
retain caller causes and return typed errors instead of running diagnostics.
No independent root scheduler, canonicalizer or semantic fallback is present.
"""
import argparse
import hashlib
import re
import subprocess
from pathlib import Path

from upstream_region_collector import selected

ROOT = Path(__file__).resolve().parents[1]
TARGET = ROOT / 'crates/lrr/src/traits/upstream/infer/fulfillment.rs'
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
FULFILL = 'compiler/rustc_trait_selection/src/solve/fulfill.rs'
OBLIGATION = 'compiler/rustc_infer/src/traits/mod.rs'
LIMIT = 'compiler/rustc_structures/src/limit.rs'
HASHES = {
    FULFILL: 'b847f09571a9b182c1456ea0c3fc9ccae43457ac7b12f01f1d28c0573e06ad7f',
    OBLIGATION: '4bfd95cdd4b0e3b18cdc0a60ae81470d916556d2270fe9a1556d31206a03757c',
    LIMIT: 'aaed7ebb13ff9ba166be0ef015e86f9bdfff8f1cba9e4e15a950a96932ae9faf',
}


def original(name):
    raw = (ROOT / 'vendor/rustc-trait-solver' / name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != HASHES[name]:
        raise ValueError('pinned fulfillment input differs: ' + name)
    return raw.decode()


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError('pinned fulfillment anchor differs: ' + old)
    return text.replace(old, new, 1)


def project(text):
    text = text.replace("'tcx", "'db")
    text = text.replace("TyCtxt<'db>", "DbInterner<'db>")
    for name in ['PredicateObligation', 'PredicateObligations', 'PendingObligations', 'ObligationStorage']:
        text = text.replace(name + "<'db>", name + "<'db, C>")
    text = text.replace('    pub fn ', '    pub(crate) fn ')
    text = text.replace('obligation.cause.span', 'obligation.span')
    return text


def owner_method(source, name):
    text = project(selected(source, name))
    # Bound safe owner, not compiler's transparent reference transmute.
    text = text.replace("        infcx: &InferCtxt<'db>,\n", '')
    text = text.replace(", infcx: &InferCtxt<'db>", '')
    text = text.replace('let delegate = <&SolverDelegate<\'db>>::from(infcx);',
                        'let delegate = infcx;')
    text = text.replace('infcx.tcx.disable_trait_solver_fast_paths()',
                        'infcx.disable_trait_solver_fast_paths()')
    text = text.replace('.as_type(infcx.tcx)', '.as_type(infcx.interner)')
    text = text.replace('*infcx.shallow_resolve(ty).kind()', 'infcx.shallow_resolve(ty).kind()')
    # A selected declaration may wrap its opening brace after a where clause;
    # all these methods have ordinary signatures, selected from the pinned file.
    body = text.index(' {') + 2
    text = text[:body] + '\n        let infcx = self.owner;' + text[body:]
    return text


def expected():
    source = original(FULFILL)
    engine_source = source[source.index("impl<'tcx, E> TraitEngine"):]
    obligation_source = original(OBLIGATION)
    # The exact compiler comparison is <=. Do not accidentally introduce a
    # stricter '<' gate or an unrelated work counter while removing the wrapper.
    limit = selected(original(LIMIT), 'value_within_limit')
    if 'value <= self.0' not in limit:
        raise ValueError('pinned rustc recursion comparison differs')

    start = obligation_source.index("pub struct Obligation<'tcx, T>")
    end = obligation_source.index('\n}', start) + 2
    obligation = obligation_source[start:end]
    obligation = re.sub(r'^    #\[(?:type_foldable|type_visitable)\([^\n]*\)\]\n', '', obligation, flags=re.M)
    obligation = obligation.replace("pub struct Obligation<'tcx, T>", "pub(crate) struct PredicateObligation<'db, C>")
    obligation = obligation.replace("pub cause: ObligationCause<'tcx>,", 'pub cause: C,\n    pub span: Span,')
    obligation = obligation.replace("ty::ParamEnv<'tcx>", "ParamEnv<'db>").replace('pub predicate: T,', "pub predicate: Predicate<'db>,")
    obligation = '#[derive(Clone, Debug)]\n' + obligation + '\n'
    as_goal = selected(obligation_source, 'as_goal')
    as_goal = as_goal.replace("solve::Goal<'tcx, T>", "Goal<DbInterner<'db>, Predicate<'db>>")
    as_goal = as_goal.replace('solve::Goal {', 'Goal {').replace('pub fn', 'pub(crate) fn')
    obligation += "impl<'db, C> PredicateObligation<'db, C> {\n" + as_goal + '}\n'

    # Exact selected pending-store operations; coroutine Typeck drain is not an
    # interpreter operation, so its unused partition helper is not projected.
    storage = "struct ObligationStorage<'db, C> { pending: PendingObligations<'db, C> }\n"
    storage += "impl<'db, C> Default for ObligationStorage<'db, C> {\n    fn default() -> Self { Self { pending: ThinVec::new() } }\n}\n"
    storage += "impl<'db, C: Clone> ObligationStorage<'db, C> {\n"
    storage += ''.join(project(selected(source, name)) for name in [
        'register', 'has_pending_obligations', 'clone_pending', 'clone_pending_filtered'])
    storage += '}\n'

    registration = owner_method(engine_source, 'register_predicate_obligation')
    registration = replace_once(registration,
        '        let infcx = self.owner;',
        '        assert!(self.overflow.is_none(), "fulfillment cannot resume after fatal overflow");\n        let infcx = self.owner;')

    evaluate = owner_method(engine_source, 'try_evaluate_obligations')
    evaluate = replace_once(evaluate,
        '    pub(crate) fn try_evaluate_obligations(&mut self) -> TraitErrors<E> {',
        '''    pub(crate) fn try_evaluate_obligations_with_inspector(
            &mut self,
            mut inspect: impl FnMut(&PredicateObligation<'db, C>, &Result<GoalEvaluation<DbInterner<'db>>, NoSolution>),
        ) -> Result<ThinVec<NextSolverError<'db, C>>, FulfillmentOverflow<'db, C>> {''')
    evaluate = replace_once(evaluate,
        '        let mut errors = TraitErrors::NoErrors;',
        '''        if let Some(overflow) = &self.overflow { return Err(overflow.clone()); }
        let mut errors = ThinVec::new();
        let mut overflow_obligation = None;''')
    evaluate = replace_once(evaluate,
        '            self.obligations.pending.retain_mut(|(obligation, opt_stalled_on)| {',
        '''            self.obligations.pending.retain_mut(|(obligation, opt_stalled_on)| {
                // rustc emits a diverging diagnostic at this boundary. Retain
                // every later root without evaluating it after the typed abort.
                if overflow_obligation.is_some() { return true; }''')
    evaluate = replace_once(evaluate,
        '                Self::inspect_evaluated_obligation(infcx, &obligation, &result);',
        '                inspect(obligation, &result);')
    evaluate = replace_once(evaluate,
        '''                        errors.push(E::from_solver_error(
                            infcx,
                            NextSolverError::TrueError(obligation.clone()),
                        ));''',
        '                        errors.push(NextSolverError::TrueError(obligation.clone()));')
    evaluate = replace_once(evaluate,
        '!infcx.tcx.recursion_limit().value_within_limit(obligation.recursion_depth)',
        'obligation.recursion_depth > infcx.interner.recursion_limit()')
    evaluate = replace_once(evaluate,
        '                        infcx.err_ctxt().report_overflow_obligation(obligation, true);',
        '''                        overflow_obligation = Some(obligation.clone());
                        return true;''')
    # HIR's later MIR borrowck replay is a static compiler pass. It does not
    # participate in fulfillment or the interpreter's relaxed runtime semantics.
    yes = evaluate.index('                    Certainty::Yes => {')
    maybe = evaluate.index('                    Certainty::Maybe(_) => {', yes)
    evaluate = evaluate[:yes] + '                    Certainty::Yes => false,\n' + evaluate[maybe:]
    evaluate = replace_once(evaluate,
        '''            if !any_changed {
                break;
            }''',
        '''            if let Some(obligation) = overflow_obligation {
                let overflow = FulfillmentOverflow { obligation, errors };
                self.overflow = Some(overflow.clone());
                return Err(overflow);
            }
            if !any_changed {
                break;
            }''')
    evaluate = replace_once(evaluate, '        errors\n', '        Ok(errors)\n')

    pending = ''.join(owner_method(engine_source, name) for name in [
        'pending_obligations_potentially_referencing_sub_root',
        'pending_obligations_potentially_referencing_float_infer'])
    simple = ''.join(project(selected(engine_source, name)) for name in [
        'has_pending_obligations', 'pending_obligations'])

    header = f'''//! Pinned rustc next-solver multi-root fulfillment; MIT OR Apache-2.0.
//! Pin {PIN}; tools/upstream_fulfillment.py.
//! Exact originals/licenses: vendor/rustc-trait-solver. No compiler diagnostics.
use super::{{DbInterner, Span, PreparedProvider, delegate::SolverContext}};
use crate::traits::upstream::model;
use rustc_next_trait_solver::solve::fast_path::compute_goal_fast_path;
use rustc_next_trait_solver::solve::{{GoalEvaluation, SolverDelegateEvalExt as _, HasChanged}};
use rustc_type_ir::{{self as ty, InferCtxtLike, Interner, inherent::*}};
use rustc_type_ir::solve::{{Goal, GoalStalledOn, Certainty, NoSolution}};
use thin_vec::ThinVec;
type Predicate<'db> = model::Predicate<'db, PreparedProvider<'db>>;
type ParamEnv<'db> = model::ParamEnv<'db, PreparedProvider<'db>>;
type PredicateObligations<'db,C> = ThinVec<PredicateObligation<'db,C>>;
// Pinned ThinVec retains the compiler's hot retain_mut representation.
type PendingObligations<'db,C> = ThinVec<(PredicateObligation<'db,C>, Option<GoalStalledOn<DbInterner<'db>>>)>;
'''
    interface = '''
/// Non-diagnostic outcomes retain actual original obligations. A residual's
/// stalled state carries upstream MaybeInfo, including search overflow. None
/// means it was registered but not evaluated, not a proof of no solution.
#[derive(Clone, Debug)]
pub(crate) enum NextSolverError<'db, C> {
    TrueError(PredicateObligation<'db,C>),
    Ambiguity { obligation: PredicateObligation<'db,C>, stalled_on: Option<GoalStalledOn<DbInterner<'db>>> },
}
/// rustc's fatal fulfillment-progress overflow, rather than NoSolution.
/// Earlier no-solution errors and every unprocessed root remain available.
#[derive(Clone, Debug)]
pub(crate) struct FulfillmentOverflow<'db,C> {
    pub(crate) obligation: PredicateObligation<'db,C>,
    pub(crate) errors: ThinVec<NextSolverError<'db,C>>,
}
/// Pinned root fulfillment on one actual inference owner in one snapshot.
/// Caller C retains source/effect identity; it contributes no solving policy.
pub(crate) struct FulfillmentContext<'a,'db,C> {
    owner: &'a SolverContext<'db>,
    obligations: ObligationStorage<'db,C>,
    usable_in_snapshot: usize,
    overflow: Option<FulfillmentOverflow<'db,C>>,
}
impl<'a,'db,C:Clone> FulfillmentContext<'a,'db,C> {
    pub(crate) fn new(owner: &'a SolverContext<'db>) -> Self {
        assert!(owner.next_trait_solver(), "next-solver fulfillment requires the next solver");
        Self { owner, obligations: ObligationStorage::default(),
            usable_in_snapshot: owner.num_open_snapshots(), overflow: None }
    }
    pub(crate) fn try_evaluate_obligations(&mut self) -> Result<ThinVec<NextSolverError<'db,C>>,FulfillmentOverflow<'db,C>> {
        self.try_evaluate_obligations_with_inspector(|_,_|{})
    }
    /// Like rustc's collect_remaining_errors_impl, drain pending obligations as
    /// ambiguity. Preserve the actual stall rather than invoking diagnostics.
    /// Fatal overflow cannot be drained into an ordinary ambiguity/success.
    pub(crate) fn collect_remaining_errors(&mut self) -> Result<ThinVec<NextSolverError<'db,C>>,FulfillmentOverflow<'db,C>> {
        assert_eq!(self.usable_in_snapshot, self.owner.num_open_snapshots());
        if let Some(overflow) = &self.overflow { return Err(overflow.clone()); }
        Ok(self.obligations.pending.drain(..).map(|(obligation,stalled_on)| NextSolverError::Ambiguity {obligation,stalled_on}).collect())
    }
'''
    result = header + obligation + storage + interface + registration + evaluate + simple + pending + '}\n'
    # Read-only, lossless error transport across the nonescaping IR owner.
    result += """
impl<'a, 'db, C: Clone> FulfillmentContext<'a, 'db, C> {
    pub(crate) fn pending_with_stall(&self) -> PendingObligations<'db, C> {
        self.obligations.pending.clone()
    }
}
"""

    result = result.replace("pub(crate) fn new(owner: &'a SolverContext<'db>) -> Self {", "pub(crate) fn new(owner: &'a SolverContext<'db>) -> Self {\n        owner.require_source_root();")
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'],
                          input=result.encode(), stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, check=True).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    try:
        raw = expected()
        if args.write:
            TARGET.write_bytes(raw)
        if TARGET.read_bytes() != raw:
            raise ValueError('fulfillment differs from registered pinned projection')
        print('Pinned root fulfillment projection matches; source admission/integration/acceptance remain separate.')
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + '\n' + getattr(error, 'stderr', b'').decode(errors='replace'))


if __name__ == '__main__':
    main()
