#!/usr/bin/env python3
"""Audit the pinned new-solver WF algorithm and registered representation adapters."""
import argparse, hashlib, re, subprocess
from pathlib import Path
from sync_upstream_binders import method
ROOT=Path(__file__).resolve().parents[1]
PINS={'compiler/rustc_trait_selection/src/traits/wf.rs': '435c6263bce04bc57eda37461de1d65be70aad19c2dbb038d9b967d15708b151', 'compiler/rustc_trait_selection/src/traits/project.rs': '307ac25ad047ace41cb671052f786020c8360f5a840d9319455f9fabd393270c', 'compiler/rustc_infer/src/infer/at.rs': '96d484310f78fa1e5973ba656574dbdd1bbda8e2743ece10b720bb01e0c7cc21', 'compiler/rustc_infer/src/infer/mod.rs': '0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327', 'compiler/rustc_middle/src/ty/consts/valtree.rs': '1ab0c5918c3a8b8ff86a3eef72bb726e559ba43d9503433b7ab2c153d7368b81', 'compiler/rustc_middle/src/ty/context.rs': '442e5ac01600b2274bb6456aa6319c4e85a0266afb3245cb23a614f7d72ae9f6', 'compiler/rustc_feature/src/unstable.rs': 'fb03282838cb966e61e2e720eafa6df462313b2484061d2acbfeb8bfb7626d2d'}
PINS['compiler/rustc_middle/src/ty/mod.rs']='bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7'
TARGET=ROOT/'crates/lrr/src/traits/upstream/infer/well_formed.rs'
def original(name):
 raw=(ROOT/'vendor/rustc-trait-solver'/name).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=PINS[name]:raise ValueError('pinned WF dependency differs: '+name)
 return raw.decode()
def picked(source,name):
 source='\n'+source.replace('\npub fn ', '\n    pub fn ').replace('\n    fn ', '\n    pub fn ')
 return method(source,name).replace('    pub fn ', '    fn ', 1)
def balanced(text,start,opening='(',closing=')'):
 depth=0;quote=False;escape=False
 for i in range(start,len(text)):
  c=text[i]
  if quote:
   if escape:escape=False
   elif c=='\\':escape=True
   elif c=='"':quote=False
   continue
  if c=='"':quote=True;continue
  if c==opening:depth+=1
  elif c==closing:
   depth-=1
   if depth==0:return i+1
 raise ValueError('unbalanced pinned expression')
def drop_expr(text,prefix,replacement):
 pattern=re.compile(re.escape(prefix)+r'[A-Za-z_][A-Za-z_0-9]*')
 while match:=pattern.search(text):
  end=match.end()
  if text[end:end+1]=='(':end=balanced(text,end)
  text=text[:match.start()]+replacement+text[end:]
 return text
def drop_if(text,prefix):
 start=text.index(prefix);brace=text.index('{',start);end=balanced(text,brace,'{','}')
 return text[:start]+text[end:]
def select_else(text,prefix):
 start=text.index(prefix);brace=text.index('{',start);end=balanced(text,brace,'{','}')
 other=text.index('else',end);other_brace=text.index('{',other);other_end=balanced(text,other_brace,'{','}')
 return text[:start]+text[other_brace:other_end]+text[other_end:]
def adapt(text):
 text=text.replace("'tcx","'db").replace("TyCtxt<'db>","DbInterner<'db>")
 for name in ['ParamEnv','AliasTerm','AliasTy','TraitRef','AliasConst','Pattern','GenericArg']:
  text=text.replace('ty::'+name+"<'db>",'ty::'+name+"<DbInterner<'db>>")
 text=text.replace("GenericArgsRef<'db>","Args<'db>").replace("PredicateObligation<'db>","Obligation<'db>")
 text=text.replace("PredicateObligations<'db>","Vec<Obligation<'db>>").replace("PredicateObligations::new()","Vec::new()")
 text=text.replace("InferCtxt<'db>","InferenceContext<'db>").replace("Term<'db>","Term<'db>")
 text=text.replace('traits::Obligation::','Obligation::').replace('traits::elaborate','ty::elaborate::elaborate')
 text=text.replace('self.infcx.tcx','self.infcx.interner').replace('TyCtxt', 'DbInterner')
 text=text.replace('traits::project::compute_inherent_assoc_term_args','compute_inherent_assoc_term_args').replace('traits::SelectionContext','SelectionContext')
 text=text.replace("SelectionContext<'b, 'db>","SelectionContext<'b, 'db>")
 text=text.replace('match *t.kind()', 'match t.kind()').replace('match *pat', 'match pat.kind()')
 text=text.replace('tcx.types.usize','Ty::new_usize(tcx)').replace('tcx.types.trait_object_dummy_self','tcx.ty(ty::TyKind::Infer(ty::InferTy::FreshTy(0)))')
 text=text.replace('def.did()', 'def.def_id()')
 for name in ['gca_const_items','generic_const_exprs','coroutine_clone']:
  text=text.replace('tcx.features().'+name+'()', "ty::inherent::Features::<DbInterner<'db>>::"+name+'(tcx.features())')
 # Model Type::kind returns a copy, whereas rustc borrows the enum payload.
 text=text.replace('ConstArgHasType(elem_val, *elem_ty)', 'ConstArgHasType(elem_val, elem_ty)')
 text=re.sub(r'tcx\s*\.type_of\(field_def.did\)\s*\.instantiate\(tcx, args\)', 'field_def.ty(tcx, args)', text)
 text=text.replace('tcx.type_of(field_def.did)\n                                        .instantiate(tcx, args)', 'field_def.ty(tcx, args)')
 text=text.replace('self.tcx().require_lang_item(LangItem::Sized, cause.span)', 'self.tcx().require_trait_lang_item(SolverTraitLangItem::Sized)')
 text=re.sub(r'self\.tcx\(\)\.require_lang_item\(\s*LangItem::BikeshedGuaranteedNoDrop,\s*self.span,?\s*\)', 'self.tcx().require_trait_lang_item(SolverTraitLangItem::BikeshedGuaranteedNoDrop)',text)
 text=drop_expr(text,'ObligationCauseCode::','()')
 text=text.replace("traits::ObligationCauseCode<'db>",'()')
 text=text.replace('cause.span', 'self.span')
 text=text.replace('self.tcx().is_lang_item(def_id, LangItem::Sized)','is_wf_lang_item(self.tcx(), def_id, WfLangItem::Sized)')
 text=text.replace('self.tcx().is_lang_item(def_id, LangItem::ConstParamTy)','is_wf_lang_item(self.tcx(), def_id, WfLangItem::ConstParamTy)')
 text=text.replace("&'db ty::List<ty::PolyExistentialPredicate<'db>>", "model::Existentials<'db, super::PreparedProvider<'db>>")
 text=text.replace("Vec<ty::Region<'db>>", "Vec<super::Region<'db>>")
 for name in ['Bool','Char','Int','Uint','Float','Error','Str','CoroutineWitness','Never','Param','Bound','Placeholder','Foreign','Infer','Slice','Array','Pat','Tuple','RawPtr','Alias','Adt','FnDef','Ref','Coroutine','Closure','CoroutineClosure','FnPtr','UnsafeBinder','Dynamic']:
  text=re.sub(r'\bty::'+name+r'\b','ty::TyKind::'+name,text)
 for name in ['TyVar','IntVar','FloatVar']:
  text=re.sub(r'\bty::'+name+r'\b','ty::InferTy::'+name,text)
 for name in ['Projection','Opaque','Free','Inherent']:
  text=re.sub(r'\bty::'+name+r'(?=\s*\{)','ty::AliasTyKind::'+name,text)
 text=text.replace('ty::Unnormalized::new_wip(alias_term.self_ty())', "ty::Unnormalized::<DbInterner<'db>, _>::new_wip(alias_term.self_ty())")
 text=re.sub(r'\bbug!\(', 'panic!(', text)
 text=text.replace("ty::ParamEnv<DbInterner<'db>>", "ParamEnv<'db>").replace("ty::Args<'db>", "Args<'db>")
 text=text.replace("ty::Pattern<DbInterner<'db>>", "model::Pattern<'db, super::PreparedProvider<'db>>")
 text=text.replace("ty::Region<'db>", "super::Region<'db>").replace("ty::Const<'db>", "Const<'db>")
 return text
def expected():
 for name in PINS: original(name)
 field_source=original('compiler/rustc_middle/src/ty/mod.rs')
 field_source=field_source[field_source.index("impl<'tcx> FieldDef {"):]
 field=picked(field_source,'ty').replace("'tcx","'cx")
 field=field.replace("TyCtxt<'cx>","Context<'cx, F>").replace("GenericArgsRef<'cx>","Args<'cx, F>").replace("Unnormalized<'cx, Ty<'cx>>","Unnormalized<Context<'cx, F>, Ty<'cx, F>>").replace('tcx.type_of(self.did).instantiate(tcx, args)','self.ty.instantiate(tcx, args)').replace('tcx:', 'cx:').replace('instantiate(tcx,','instantiate(cx,')
 actual=(ROOT/'crates/lrr/src/traits/upstream/model.rs').read_text()
 actual=actual[actual.index("impl<'cx, F: SourceFacts<'cx>> AdtField<'cx, F> {"):]
 actual=picked(actual.replace('pub(super) fn ', 'pub fn '),'ty')
 if re.sub(r'\s+','',actual)!=re.sub(r'\s+','',field): raise ValueError('prepared field binder differs from registered compiler FieldDef::ty adapter')
 source=original('compiler/rustc_trait_selection/src/traits/wf.rs')
 root=picked(source,'unnormalized_obligations').replace('fn unnormalized_obligations','pub(super) fn unnormalized_obligations')
 root=root.replace('    body_def_id: LocalDefId,\n','').replace('        body_def_id,\n','').replace('        item: None,\n','')
 methods=''.join(picked(source,name) for name in ['tcx','require_sized','add_wf_preds_for_term','nominal_obligations','add_wf_preds_for_dyn_ty','add_wf_preds_for_pat_ty','add_wf_preds_for_inherent_projection'])
 # The shared interner already exposes flattened, parent-first clauses. Span/cause
 # bookkeeping is discarded by rustc's final as_goal map, not another WF rule.
 begin=methods.index('        let tcx = self.tcx();',methods.index('fn nominal_obligations'))
 end=methods.index('    fn add_wf_preds_for_dyn_ty',begin)
 methods=methods[:begin]+"""        let tcx = self.tcx();
        for clause in tcx.clauses_of(def_id).iter_instantiated(tcx, args) {
            if !clause.has_escaping_bound_vars() {
                let obligation = Obligation::with_depth(tcx, self.cause(()), self.recursion_depth, self.param_env, clause.skip_norm_wip());
                push_obligation(self, obligation);
            }
        }
    }
"""+methods[end:]
 methods=drop_if(methods,'                    if !tcx.features().generic_pattern_types()')
 visitor=source[source.index("impl<'a, 'tcx> TypeVisitor"):source.index('pub fn object_region_bounds')]
 visitor=visitor.replace("for WfPredicates<'a, 'tcx> {", "for WfPredicates<'a, 'tcx> {\n    type Result = ();")
 visitor=drop_if(visitor,'                        if elem.is_scalable_vector()')
 visitor=drop_if(visitor,'                    if last.is_scalable_vector()')
 bounds=source[source.index("pub fn object_region_bounds"):]
 project=picked(original('compiler/rustc_trait_selection/src/traits/project.rs'),'compute_inherent_assoc_term_args')
 project=select_else(project,'if !selcx.infcx.next_trait_solver()')
 project=select_else(project,'if !selcx.infcx.next_trait_solver()')
 project=project.replace('tcx.parent(alias_def_id)','tcx.inherent_alias_term_parent(alias_def_id)')
 project=project.replace('selcx.infcx.fresh_args_for_item(cause.span, impl_def_id)','ty::InferCtxtLike::fresh_args_for_item(selcx.infcx, impl_def_id)')
 project=project.replace("cause: ObligationCause<'tcx>",'cause: ()')
 project=project.replace('selcx.infcx.at(&cause, param_env).eq(DefineOpaqueTypes::Yes, impl_ty, self_ty)',
    'rustc_type_ir::relate::solver_relating::RelateExt::relate(selcx.infcx, param_env, impl_ty, ty::Invariant, self_ty, <Span as ty::inherent::Span<DbInterner<\'db>>>::dummy())')
 project=project.replace('Ok(mut ok) => obligations.append(&mut ok.obligations)',
    'Ok(goals) => obligations.extend(goals.into_iter().map(Obligation::from_goal))')
 error_start=project.index('            tcx.dcx().span_bug(');error_end=project.index('\n        }',error_start)
 project=project[:error_start]+'''            panic!("inherent Self types differ after selection: {self_ty:?}, {impl_ty:?}");'''+project[error_end:]
 features=original('compiler/rustc_feature/src/unstable.rs')
 gca=picked(features,'gca')
 # Actual declare_features! accessors. Symbol-list membership is the retained
 # FeatureSet representation of rustc's enabled_features set; query tracking is
 # unnecessary inside an immutable, already-keyed prepared source context.
 accessor=picked(features,'$feature')
 feature_methods=''.join(accessor.replace('$feature',name).replace('sym::'+name,'model::Symbol("'+name+'")') for name in ['gca_min_const_items','gca_adts','const_param_ty_unchecked'])
 enabled=picked(features,'enabled').replace('feature: Symbol',"feature: model::Symbol<'_>").replace('self.enabled_features.contains(&feature)','self.enabled.contains(&feature)').replace('            TRACK_FEATURE(feature);\n','')
 code=adapt(root+"\nimpl<'a, 'tcx> WfPredicates<'a, 'tcx> {\n"+methods+"\n}\n"+visitor+bounds+project)
 term_source=original('compiler/rustc_middle/src/ty/mod.rs')
 term_source=term_source[term_source.index("impl<'tcx> Term<'tcx> {"):term_source.index("impl<'tcx> TermKind<'tcx> {")]
 code+="\nimpl<'db> Term<'db> {\n"+adapt(''.join(picked(term_source,name) for name in ['is_infer','as_const']))+'\n}\n'
 code+="\nimpl model::FeatureSet<'_> {\n"+gca+feature_methods+enabled+'\n}\n'
 return subprocess.run(['rustfmt','+1.99.0','--edition','2024'],input=(PRELUDE+code).encode(),capture_output=True,check=True).stdout
PRELUDE="""//! Pinned rustc unnormalized WF goals; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_wellformed.py.
use super::{InferenceContext, DbInterner, Ty, Const, Span};
use crate::traits::upstream::{model::{self, *}, identity::DefinitionId as DefId};
use crate::traits::upstream::facts::SourceFacts as _;
use rustc_type_ir::{self as ty, inherent::*, Interner, TypeVisitor, TypeVisitable, TypeSuperVisitable, TypeVisitableExt, PredicateProxy, Upcast, TermKind};
use rustc_type_ir::inherent::Ty as _;
use rustc_type_ir::data_structures::DelayedSet;
use rustc_type_ir::solve::Goal;
use rustc_type_ir::lang_items::SolverTraitLangItem;
use tracing::{debug, instrument};
type ParamEnv<'db> = model::ParamEnv<'db, super::PreparedProvider<'db>>;
type Args<'db> = model::Args<'db, super::PreparedProvider<'db>>;
type Term<'db> = model::Term<'db, super::PreparedProvider<'db>>;
type Predicate<'db> = model::Predicate<'db, super::PreparedProvider<'db>>;
#[derive(Debug)]
pub(super) struct Obligation<'db> { pub(super) predicate: Predicate<'db>, param_env:model::ParamEnv<'db,super::PreparedProvider<'db>> }
impl<'db> Obligation<'db> {
 fn with_depth<P:Upcast<DbInterner<'db>,Predicate<'db>>>(cx:DbInterner<'db>,_cause:(),_depth:usize,param_env:model::ParamEnv<'db,super::PreparedProvider<'db>>,predicate:P)->Self { Self{param_env,predicate:predicate.upcast(cx)} }
 fn new<P:Upcast<DbInterner<'db>,Predicate<'db>>>(cx:DbInterner<'db>,cause:(),param_env:model::ParamEnv<'db,super::PreparedProvider<'db>>,predicate:P)->Self { Self::with_depth(cx,cause,0,param_env,predicate) }
 pub(super) fn as_goal(self)->Goal<DbInterner<'db>,Predicate<'db>> { Goal{param_env:self.param_env,predicate:self.predicate} }
 fn from_goal(goal:Goal<DbInterner<'db>,Predicate<'db>>)->Self { Self{param_env:goal.param_env,predicate:goal.predicate} }
}
struct SelectionContext<'a,'db> { infcx:&'a InferenceContext<'db> }
impl<'a,'db> SelectionContext<'a,'db> {
 fn new(infcx:&'a InferenceContext<'db>)->Self {Self{infcx}}
 fn tcx(&self)->DbInterner<'db> {self.infcx.interner}
}
fn dummy_span<'db>()->Span { <Span as ty::inherent::Span<DbInterner<'db>>>::dummy() }
enum WfLangItem { Sized, ConstParamTy }
fn is_wf_lang_item(cx:DbInterner<'_>,id:DefId,item:WfLangItem)->bool {
 match item {
  WfLangItem::Sized=>cx.services().is_trait_lang_item(cx,id,SolverTraitLangItem::Sized),
  WfLangItem::ConstParamTy=>cx.services().const_param_ty_trait()==Some(id),
 }
}
struct WfPredicates<'a,'db> {
 infcx:&'a InferenceContext<'db>, param_env:model::ParamEnv<'db,super::PreparedProvider<'db>>,
 span:Span, out:Vec<Obligation<'db>>, recursion_depth:usize,
 visited_tys:DelayedSet<Ty<'db>>,
}
impl<'a,'db> WfPredicates<'a,'db> { fn cause(&self,_code:())->() {} }
"""
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
 data=expected()
 if args.write:TARGET.write_bytes(data)
 elif TARGET.read_bytes()!=data:raise ValueError('WF port differs from registered pinned transformation')
 print('Pinned WF source hash and registered transformation match; helper/API/root integration requires separate evidence.')
if __name__=='__main__':main()
