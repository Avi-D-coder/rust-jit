"""Pinned compiler complete hidden-type item bounds client and BottomUpFolder.

No alternative opaque matching or default source bound/const metadata.
"""
import hashlib
from pathlib import Path
from upstream_infer_client import adapt
from upstream_region_collector import selected
ROOT=Path(__file__).resolve().parents[1]
HASHES={
 'compiler/rustc_infer/src/infer/opaque_types/mod.rs':'5fa09869ec7c26a4065504a401729e05f588efdffe4bb74998faa14d9cd2a1a6',
 'compiler/rustc_middle/src/ty/fold.rs':'e07cb2d26e76490d9a5cb6af659d993f5a95e4ccc99088ed90f233e2842f50b4',
}
def original(path):
 raw=(ROOT/'vendor/rustc-trait-solver'/path).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=HASHES[path]:raise ValueError('hidden bounds source differs '+path)
 return raw.decode()
def extend_hidden_type_bounds(outputs,header,rustfmt):
 source=original('compiler/rustc_infer/src/infer/opaque_types/mod.rs')
 body=adapt(selected(source,'add_item_bounds_for_hidden_type'))
 body=body.replace("DefId", "DefinitionId")
 body=body.replace("ty::ParamEnv<'db>", "model::ParamEnv<'db,PreparedProvider<'db>>")
 # Source services expose owned copied items, not references to compiler Lists.
 body=body.replace(".iter_instantiated_copied(tcx, args)", ".iter_instantiated(tcx, args)")
 body=body.replace("ty::Clause<'db>","Clause<'db>").replace("ty::Predicate<'db>","Predicate<'db>")
 body=body.replace('BottomUpFolder {\n                tcx,','BottomUpFolder {\n                interner: tcx,')
 # Existing shared service carries the exact ordered implied-const predicates,
 # without compiler diagnostic spans. The original immediately discards spans.
 marker='        // If this opaque is being defined and it\'s conditionally const,'
 before,after=body.split(marker,1)
 after=after.replace('for (predicate, _) in','for predicate in',1)
 body=before+marker+after
 folder=original('compiler/rustc_middle/src/ty/fold.rs')
 a=folder.index('pub struct BottomUpFolder');b=folder.index('\n///////////////////////////////////////////////////////////////////////////',a)
 folder=adapt(folder[a:b]).replace('pub tcx:', 'pub interner:')
 imports='''use super::{InferenceContext,DbInterner,PreparedProvider,Ty,Const};
use crate::traits::upstream::{identity::DefinitionId,model};
use rustc_type_ir::{self as ty,Interner,InferCtxtLike,TypeFoldable,TypeFolder,TypeSuperFoldable,TypeVisitableExt,Unnormalized,inherent::*};
use tracing::debug;
type Goal<'db,P> = ty::solve::Goal<DbInterner<'db>,P>;
type Clause<'db> = model::Clause<'db,PreparedProvider<'db>>;
type Predicate<'db> = model::Predicate<'db,PreparedProvider<'db>>;
'''
 outputs['hidden_type_bounds.rs']=rustfmt(header+imports+folder+"impl<'db> InferenceContext<'db> {\n"+body+'}\n')
 outputs['mod.rs']+=b'\npub(crate) mod hidden_type_bounds;\n'
