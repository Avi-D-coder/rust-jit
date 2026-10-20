"""Concrete next-solver interface transport over the existing inference owner.

The source-dependent client functions remain mandatory integration obligations.
No fallback client or root-ready token is constructed by this generator.
"""
import hashlib
from pathlib import Path
from upstream_infer_client import adapt
from upstream_region_collector import selected

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'compiler/rustc_trait_selection/src/solve/delegate.rs'
SHA256 = 'ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095'


def extend_solver_delegate(outputs, header, rustfmt):
    raw = (ROOT / 'vendor/rustc-trait-solver' / SOURCE).read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA256:
        raise ValueError('pinned compiler delegate differs')
    original = raw.decode()
    methods = ''.join(selected(original, name) for name in (
        'fresh_var_for_kind', 'leak_check', 'instantiate_canonical', 'instantiate_canonical_var'))
    methods = adapt(methods)
    methods = methods.replace("    pub fn ", "    fn ").replace("ty::GenericArg<'db>", "GenericArg<'db>")
    methods = methods.replace('self.0.tcx', 'self.0.interner')
    # Existing fixed-profile owner has no no-leak-check opt-out. Dispatch to
    # the actual compiler collector; do not copy RA's unconditional Ok.
    methods = methods.replace('self.0.leak_check(max_input_universe, None)', 'self.0.inner.borrow_mut().unwrap_region_constraints().leak_check(self.interner,max_input_universe,self.universe(),None)')
    methods = methods.replace('RegionVariableOrigin::Misc(span)', 'span')
    methods = methods.replace('self.next_region_var_in_universe(span, universe)', 'self.next_region_var_in_universe(universe, span)')
    methods = methods.replace('self.next_ty_var_in_universe(span, universe)', 'self.next_ty_var_in_universe(universe, span)')
    methods = methods.replace('self.next_const_var_in_universe(span, universe)', 'self.next_const_var_in_universe(universe, span)')
    # Full pinned projection eligibility policy. Its graph query reads actual
    # immutable prepared leaves; unavailable source is never semantic NotFound.
    eligibility = adapt(selected(original, 'fetch_eligible_assoc_item'))
    eligibility = eligibility.replace('    pub fn ', '    fn ')
    eligibility = eligibility.replace("ty::TraitRef<'db>", "ty::TraitRef<Self::Interner>")
    eligibility = eligibility.replace('DefId', 'DefinitionId')
    eligibility = eligibility.replace("FetchEligibleAssocItemResponse<'db>", 'FetchEligibleAssocItemResponse<Self::Interner>')
    eligibility = eligibility.replace('specialization_graph::assoc_def(self.interner, impl_def_id, trait_assoc_def_id)',
        'self.interner.services().associated_leaf(impl_def_id, trait_assoc_def_id)')
    eligibility = eligibility.replace('node_item.item.def_id', 'node_item.item')
    methods += eligibility
    imports = '''use super::{InferenceContext, DbInterner, PreparedProvider, Span, Ty, Const};
use super::canonical::CanonicalExt;
use crate::traits::upstream::{identity::DefinitionId, model};
use rustc_next_trait_solver::delegate::SolverDelegate;
use rustc_type_ir::{self as ty, TypeFoldable, inherent::*};
use rustc_type_ir::{MayBeErased, TypingMode, TypeVisitableExt};
use rustc_type_ir::solve::{Goal, ComputeGoalFastPathOutcome, FetchEligibleAssocItemResponse, Certainty, NoSolution, VisibleForLeakCheck};
use std::{fmt::Debug, ops::Deref};
type GenericArg<'db> = model::Arg<'db,PreparedProvider<'db>>;
type ParamEnv<'db> = model::ParamEnv<'db,PreparedProvider<'db>>;
type Predicate<'db> = model::Predicate<'db,PreparedProvider<'db>>;
type Term<'db> = model::Term<'db,PreparedProvider<'db>>;
type Args<'db> = model::Args<'db,PreparedProvider<'db>>;
type Canonical<'db,V> = ty::Canonical<DbInterner<'db>,V>;
type CanonicalVarValues<'db> = ty::CanonicalVarValues<DbInterner<'db>>;
type CanonicalVarKind<'db> = ty::CanonicalVarKind<DbInterner<'db>>;

/// Required real compiler-client functions over admitted immutable source facts.
/// No Default, test stub, guest-loaded function or implicit incomplete answer.
/// Implementing/installing these clients remains a separate root-entry gate.
pub(crate) struct SolverClients<'db> {
    /// Err(()) means only the supplied normalization callback failed. Other
    /// evaluation errors are represented as the actual error Const, as upstream.
    pub(crate) evaluate_const: fn(&InferenceContext<'db>, ParamEnv<'db>, ty::AliasConst<DbInterner<'db>>,
        &mut dyn FnMut(ty::Unnormalized<DbInterner<'db>,Ty<'db>>) -> Option<Ty<'db>>) -> Result<Option<Const<'db>>,()>,
    pub(crate) transmutable: fn(&InferenceContext<'db>,Ty<'db>,Ty<'db>,Const<'db>) -> Result<Certainty,NoSolution>,
}

/// Own exactly one inference context. Safe ownership replaces the upstream
/// transparent borrowed-reference transmute; there is no second mutable store.
pub(crate) struct SolverContext<'db>(InferenceContext<'db>);
impl<'db> SolverContext<'db> {
    pub(crate) fn from_inference_context(context: InferenceContext<'db>) -> Self { Self(context) }
}
impl<'db> Deref for SolverContext<'db> {
    type Target = InferenceContext<'db>;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<'db> SolverDelegate for SolverContext<'db> {
    type Infcx = InferenceContext<'db>;
    type Interner = DbInterner<'db>;
    fn build_with_canonical<V>(cx:Self::Interner, input:&ty::CanonicalQueryInput<Self::Interner,V>) -> (Self,V,CanonicalVarValues<'db>)
    where V:TypeFoldable<Self::Interner> {
        let span = <Span as ty::inherent::Span<Self::Interner>>::dummy();
        let (context,value,variables) = InferenceContext::build_with_canonical(cx,span,input);
        (Self(context),value,variables)
    }
    fn compute_goal_fast_path(&self,goal:Goal<Self::Interner,Predicate<'db>>,span:Span) -> ComputeGoalFastPathOutcome<Self::Interner> {
        self.0.compute_goal_fast_path(goal,span)
    }
'''
    clients = '''
    fn evaluate_const<E:Debug>(&self,env:ParamEnv<'db>,alias:ty::AliasConst<Self::Interner>,normalize:impl FnOnce(ty::Unnormalized<Self::Interner,Ty<'db>>) -> Result<Ty<'db>,E>) -> Result<Option<Const<'db>>,E> {
        let mut normalize = Some(normalize);
        let mut failure = None;
        let mut callback = |value| match normalize.take().expect("upstream const normalization is FnOnce")(value) {
            Ok(value) => Some(value),
            Err(error) => { failure = Some(error); None }
        };
        let result = (self.interner.services().solver_clients().evaluate_const)(&self.0,env,alias,&mut callback);
        match result {
            Ok(value) => { assert!(failure.is_none(), "const client discarded normalization failure"); Ok(value) }
            Err(()) => Err(failure.expect("const client failed without normalization error")),
        }
    }
    fn well_formed_goals(&self,env:ParamEnv<'db>,term:Term<'db>) -> Option<Vec<Goal<Self::Interner,Predicate<'db>>>> {
        super::well_formed::unnormalized_obligations(&self.0,env,term,<Span as ty::inherent::Span<Self::Interner>>::dummy())
            .map(|obligations| obligations.into_iter().map(|obligation| obligation.as_goal()).collect())
    }
    fn make_deduplicated_region_constraints(&self) -> Vec<(ty::RegionConstraint<Self::Interner>,VisibleForLeakCheck)> {
        self.0.make_deduplicated_region_constraints()
    }
    fn add_item_bounds_for_hidden_type(&self,id:DefinitionId,args:Args<'db>,env:ParamEnv<'db>,hidden:Ty<'db>,goals:&mut Vec<Goal<Self::Interner,Predicate<'db>>>) {
        self.0.add_item_bounds_for_hidden_type(id,args,env,hidden,goals)
    }
    fn is_transmutable(&self,src:Ty<'db>,dst:Ty<'db>,assume:Const<'db>) -> Result<Certainty,NoSolution> {
        (self.interner.services().solver_clients().transmutable)(&self.0,src,dst,assume)
    }
    fn emit_next_solver_overflow_fcw(&self,goal:Goal<Self::Interner,Predicate<'db>>,span:Span) {
        // E006: no static lint enforcement. Retain non-enforcing diagnostic
        // evidence; exact rustc lint/proof-tree formatting is a separate gate.
        self.interner.services().record_solver_overflow(span,format!("next solver overflow warning: {goal:?}"));
    }
}
'''
    outputs['delegate.rs'] = rustfmt(header + imports + methods + clients)
    outputs['mod.rs'] += b'\npub(crate) mod delegate;\n'
    outputs['mod.rs'] += b'\npub(crate) mod well_formed;\n'
