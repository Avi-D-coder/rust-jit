"""Pinned compiler canonical-input instantiation and pure context cache transport.
No independent canonicalizer, query-response engine or fake solver delegate.
"""
import hashlib,re
from pathlib import Path
from upstream_infer_client import adapt as client_adapt
from upstream_region_collector import selected
ROOT=Path(__file__).resolve().parents[1]
BASE='compiler/rustc_infer/src/infer/'
HASHES={BASE+'canonical/mod.rs':'6d521765f1c5630f1c4eac6478250e523a91d31b80e82455e5d026b1756a7a2d',BASE+'canonical/instantiate.rs':'309dfb67695984b4e0a3d09bb95ccc396d680825bdad893fde1848ad691a522e',BASE+'mod.rs':'0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327'}

def original(name):
 raw=(ROOT/'vendor/rustc-trait-solver'/name).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=HASHES[name]:raise ValueError('canonical instantiation original differs: '+name)
 return raw.decode()

def adapt(text):
 text=client_adapt(text)
 text=re.sub(r'\btcx\b','interner',text)
 text=text.replace('if let &ty::Infer(', 'if let ty::Infer(')
 text=text.replace('RegionVariableOrigin::Misc(span)', 'span')
 text=text.replace('self.next_ty_vid_in_universe(span, universe_map(ui))','self.next_ty_var_id_in_universe(universe_map(ui), span)')
 text=text.replace('self.next_float_var(span, None)', 'self.next_float_var()')
 text=text.replace('.next_region_var_in_universe(span, universe_map(ui))', '.next_region_var_in_universe(universe_map(ui), span)')
 text=text.replace('.next_const_var_in_universe(span, universe_map(ui))', '.next_const_var_in_universe(universe_map(ui), span)')
 text=text.replace('self.var_values[', 'self.var_values.items()[')
 text=text.replace('&canonical.var_kinds', 'canonical.var_kinds')
 text=text.replace('&self.var_values[..=index]', '&self.var_values.items()[..=index]')
 return text

def extend_canonical_instantiation(outputs,header,rustfmt):
 imports='''use super::{InferenceContext, DbInterner, PreparedProvider, Span, Ty, Const};
use crate::traits::upstream::model::{Args, Arg, Clauses};
use rustc_type_ir::{self as ty, Interner, TypeFoldable, TypeFolder, TypeSuperFoldable, TypeSuperVisitable, TypeVisitable, TypeVisitableExt, TypeVisitor, TypeFlags, PredicateProxy, inherent::*};
use rustc_type_ir::inherent::{Ty as _, GenericArg as _};
use rustc_index::IndexVec;
use rustc_type_ir::data_structures::DelayedMap;
type Canonical<'db,V> = ty::Canonical<DbInterner<'db>,V>;
type CanonicalQueryInput<'db,V> = ty::CanonicalQueryInput<DbInterner<'db>,V>;
type CanonicalVarValues<'db> = ty::CanonicalVarValues<DbInterner<'db>>;
type CanonicalVarKind<'db> = ty::CanonicalVarKind<DbInterner<'db>>;
type GenericArg<'db> = Arg<'db,PreparedProvider<'db>>;
type CanonicalClauses<'db> = Clauses<'db,PreparedProvider<'db>>;
'''
 source=original(BASE+'canonical/mod.rs')
 owner=source[source.index("impl<'tcx> InferCtxt<'tcx>"):]
 owner=adapt(owner)
 # Original builder initializes the current compiler owner in the canonical
 # input's actual typing mode. Transport that ownership to the existing owner.
 builder=selected(original(BASE+'mod.rs'),'build_with_canonical')
 builder=builder.replace('mut self,','interner: DbInterner<\'db>,',1)
 builder=builder.replace('let infcx = self.build(input.typing_mode.0);','let infcx = Self::new(interner, input.typing_mode.0);')
 owner+='\nimpl<\'db> InferenceContext<\'db> {\n'+adapt(builder)+'}\n'
 source=original(BASE+'canonical/instantiate.rs')
 marker="impl<'tcx, V> Canonical<'tcx, V> {"
 body=source[source.index(marker):]
 body=body.replace(marker,"impl<'tcx, V> CanonicalExt<'tcx, V> for Canonical<'tcx, V> {",1)
 body=adapt(body)
 body=body.replace("Clauses<'db>","CanonicalClauses<'db>")
 body=body.replace('self.interner.caches.', 'self.interner.instantiation_caches().')
 # The multiline original accesses caches with separately indented components.
 body=re.sub(r'\.interner\s*\.caches\s*\.', '.interner.instantiation_caches().',body)
 body=body.replace('.lock()', '.borrow_mut()')
 body=body.replace("impl<'db> TypeVisitor<DbInterner<'db>> for HighestVarInClauses {", "impl<'db> TypeVisitor<DbInterner<'db>> for HighestVarInClauses {\n type Result = ();")
 body=body.replace('.get(&(c, c_args))', '.get(&(c, c_args)).copied()')
 trait='''pub(crate) trait CanonicalExt<'db,V> {
 fn instantiate(&self, interner: DbInterner<'db>, var_values: &CanonicalVarValues<'db>) -> V where V: TypeFoldable<DbInterner<'db>>;
 fn instantiate_projected<T>(&self, interner: DbInterner<'db>, var_values: &CanonicalVarValues<'db>, projection_fn: impl FnOnce(&V)->T) -> T where T: TypeFoldable<DbInterner<'db>>;
}
'''
 outputs['canonical.rs']=rustfmt(header+imports+trait+owner+body)
 outputs['mod.rs']+=b'\npub(crate) mod canonical;\n'
