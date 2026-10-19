"""Pinned next-solver region-response collector/dedup and assumption ownership.
No legacy query evaluator, region solver, fake delegate or missing-fact default.
"""
import hashlib,textwrap
from pathlib import Path
from upstream_infer_client import adapt as client_adapt
from upstream_region_collector import selected, replace_method, adapt as region_adapt
from upstream_obligation_state import record
ROOT=Path(__file__).resolve().parents[1]
BASE='compiler/rustc_infer/src/infer/'
HASHES={BASE+'canonical/query_response.rs':'56b972b5a87fd18c3494cc015f6c9a95cdd136090da623b298db45dd7ca0239f',BASE+'outlives/mod.rs':'fae5c371428bd36956a2da21627ce44205c0eeadb87b5b22d541d2c094d2f346',BASE+'outlives/obligations.rs':'30b0ed80442caa513768a5b25a28310d3c37370d3f1f588cd3e3a6f210ca2dae',BASE+'mod.rs':'0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327',BASE+'region_constraints/mod.rs':'01e53909ea1a62b3415889611c7593e3d226c3a1eadf19f6dee4525b368fd5cb','compiler/rustc_middle/src/infer/canonical.rs':'a93e513ae058def7ec200af342b23a1f3088417ffdd335bbbf6345fd8953d4c0','compiler/rustc_trait_selection/src/solve/delegate.rs':'ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095'}
def original(name):
 raw=(ROOT/'vendor/rustc-trait-solver'/name).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=HASHES[name]:raise ValueError('region response original differs: '+name)
 return raw.decode()
def adapt(text):
 text=client_adapt(text).replace("ty::RegionConstraint<'db>","ty::RegionConstraint<DbInterner<'db>>")
 text=text.replace("ty::ArgOutlivesClause<'db>","ArgOutlivesPredicate<'db>")
 text=text.replace('category: ConstraintCategory<\'db>', 'origin: Span')
 text=text.replace('category: origin.to_constraint_category()', 'origin')
 text=text.replace('self.0.', 'self.')
 text=text.replace('.map(|(c, origin)|', '.map(|&(c, origin)|')
 return text

def extend_region_response(outputs,header,rustfmt):
 compiler=original(BASE+'region_constraints/mod.rs')
 body=outputs['region_constraints/mod.rs'].decode()
 body=body.replace(record(body,'pub struct RegionConstraintData'),region_adapt(record(compiler,'pub struct RegionConstraintData')),1)
 body=body.replace('use super::MemberConstraint;\n','')
 body=replace_method(body,'is_empty',region_adapt(selected(compiler,'is_empty')))
 outputs['region_constraints/mod.rs']=rustfmt(body)
 body=outputs['mod.rs'].decode()
 member='#[derive(Debug, Clone, PartialEq, Eq, Hash)]\n'+record(body,'pub struct MemberConstraint')
 assert body.count(member)==1
 body=body.replace(member,'',1)
 getter=adapt(selected(original(BASE+'mod.rs'),'region_assumptions'))
 body+= "\nimpl<'db> InferCtxtInner<'db> {\n"+getter+'}\nmod region_response;\n'
 outputs['mod.rs']=rustfmt(body)
 source=original('compiler/rustc_middle/src/infer/canonical.rs')
 records='#[derive(Clone, Debug, Default)]\n'+record(source,'pub struct QueryRegionConstraints')+'\n'
 records+='#[derive(Copy, Clone, Debug)]\n'+record(source,"pub struct QueryRegionConstraint<'tcx>")+'\n'
 collect=selected(textwrap.indent(original(BASE+'canonical/query_response.rs'),'    '),'make_query_region_constraints')
 methods=selected(original(BASE+'outlives/mod.rs'),'with_region_constraints')
 obligations=original(BASE+'outlives/obligations.rs')
 for name in ['register_region_assumption','take_registered_region_assumptions']:
  methods+=selected(obligations,name)
 methods+=selected(original('compiler/rustc_trait_selection/src/solve/delegate.rs'),'make_deduplicated_region_constraints')
 imports='''use super::{InferenceContext, InferCtxtInner, DbInterner, ArgOutlivesPredicate, Span, TypeOutlivesConstraint};
use super::region_constraints::{RegionConstraintData,ConstraintKind};
use super::snapshot::undo_log::UndoLog;
use ena::undo_log::UndoLogs;
use rustc_type_ir::{self as ty, Interner};
use rustc_type_ir::data_structures::HashMap as FxHashMap;
use std::collections::hash_map::Entry;
use tracing::debug;
'''
 outputs['region_response.rs']=rustfmt(header+imports+adapt(records)+adapt(collect)+"impl<'db> InferenceContext<'db> {\n"+adapt(methods)+'}\n')
