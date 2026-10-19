"""Pinned compiler complete goal fast path, stalled-state visitor and properties.
No independent trait algorithm, fake lang items or constant fallback callback.
"""
import hashlib,re,textwrap
from pathlib import Path
from upstream_infer_client import adapt as client_adapt
from upstream_region_collector import selected
ROOT=Path(__file__).resolve().parents[1]
HASHES={'compiler/rustc_trait_selection/src/traits/util.rs':'655043f657f6244e447355084e2e00d761527c4745be651306ff389be28ac28f','compiler/rustc_middle/src/ty/sty.rs':'6c29bfe00c8de834c370bbc2cfb9d6ffd39d6d662d9145860b9c0153b37f5487','compiler/rustc_middle/src/ty/mod.rs':'bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7','compiler/rustc_trait_selection/src/solve/delegate.rs':'ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095'}
def original(name):
 raw=(ROOT/'vendor/rustc-trait-solver'/name).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=HASHES[name]:raise ValueError('goal fast path original differs '+name)
 return raw.decode()
def adapt(text):
 text=client_adapt(text)
 text=text.replace('self.0.', 'self.').replace('self.tcx', 'self.interner')
 text=text.replace('Self::Interner', "DbInterner<'db>")
 text=text.replace("ty::Predicate<'db>","Predicate<'db>").replace("ty::ParamEnv<'db>","ParamEnv<'db>")
 text=text.replace("ty::GenericArg<'db>","GenericArg<'db>")
 text=text.replace('LangItem::', 'SolverTraitLangItem::')
 text=text.replace('.as_lang_item(', '.as_trait_lang_item(').replace('.is_lang_item(', '.is_trait_lang_item(')
 text=text.replace('.is_dyn_compatible(', '.trait_is_dyn_compatible(')
 text=text.replace('hir::Mutability::', 'ty::Mutability::')
 text=text.replace('*self.kind()', 'self.kind()')
 text=text.replace('&Infer(TyVar(vid))', 'ty::Infer(ty::TyVar(vid))')
 text=text.replace('(&ty::Infer(ty::TyVar(a_vid)), &ty::Infer(ty::TyVar(b_vid)))', '(ty::Infer(ty::TyVar(a_vid)), ty::Infer(ty::TyVar(b_vid)))')
 text=text.replace("""span,
                    outlives.1,
                    outlives.0,
                    ty::solve::VisibleForLeakCheck::No,""", """outlives.1,
                    outlives.0,
                    ty::solve::VisibleForLeakCheck::No,
                    span,""")
 return text

def extend_goal_fast_path(outputs,header,rustfmt):
 source=original('compiler/rustc_trait_selection/src/solve/delegate.rs')
 start=source.index("fn goal_stalled_on_args<'tcx>")
 end=source.index("impl<'tcx> rustc_next_trait_solver::delegate::SolverDelegate")
 helpers=source[start:end]
 helpers=helpers.replace("impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for CollectNonRegionInfer<'tcx> {", "impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for CollectNonRegionInfer<'tcx> {\n type Result = ();")
 methods=selected(source,'known_no_opaque_types_in_storage')+selected(source,'compute_goal_fast_path')
 sized=selected(textwrap.indent(original('compiler/rustc_trait_selection/src/traits/util.rs'),'    '),'sizedness_fast_path')
 types=original('compiler/rustc_middle/src/ty/sty.rs')
 properties=''.join(selected(types,n) for n in ['ty_vid','has_trivial_sizedness','is_trivially_pure_clone_copy','is_trivially_wf'])
 term=selected(original('compiler/rustc_middle/src/ty/mod.rs'),'is_trivially_wf')
 imports='''use super::{InferenceContext, DbInterner, PreparedProvider, Ty, Const, Span};
use crate::traits::upstream::model::{self, Arg};
use rustc_type_ir::{self as ty, InferCtxtLike, Interner, TypeFlags, TypeVisitable, TypeVisitableExt, TypeSuperVisitable, TypeVisitor, TermKind, inherent::*};
use rustc_type_ir::inherent::{Ty as _, GenericArg as _, Term as _, ParamEnv as _};
use rustc_type_ir::solve::{GoalStalledOn,GoalStalledOnOpaques,SucceededInErased,MaybeInfo,TyOrConstInferVar,SizedTraitKind};
use rustc_type_ir::lang_items::SolverTraitLangItem;
use rustc_type_ir::data_structures::HashSet as FxHashSet;
use thin_vec::{ThinVec,thin_vec};
use tracing::debug;
type Goal<'db,P> = ty::solve::Goal<DbInterner<'db>,P>;
type Predicate<'db> = model::Predicate<'db,PreparedProvider<'db>>;
type ParamEnv<'db> = model::ParamEnv<'db,PreparedProvider<'db>>;
type GenericArg<'db> = Arg<'db,PreparedProvider<'db>>;
type Term<'db> = model::Term<'db,PreparedProvider<'db>>;
type ComputeGoalFastPathOutcome<'db> = ty::solve::ComputeGoalFastPathOutcome<DbInterner<'db>>;
'''
 body=imports+adapt(helpers)+"impl<'db> InferenceContext<'db> {\n"+adapt(methods)+'}\n'
 body+=adapt(sized)+"impl<'db> Ty<'db> {\n"+adapt(properties)+'}\n'
 body+="impl<'db> Term<'db> {\n"+adapt(term)+'}\n'
 outputs['fast_path.rs']=rustfmt(header+body)
 outputs['mod.rs']+=b'\nmod fast_path;\n'
