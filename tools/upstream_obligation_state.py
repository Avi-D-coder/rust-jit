"""Pinned current opaque/outlives origin records and inference wake predicate.

Actual upstream data operations, not missing InferCtxtLike callbacks or fake facts.
"""
import hashlib
from pathlib import Path
from upstream_region_collector import selected, adapt

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / 'vendor/rustc-trait-solver'
BASE = 'compiler/rustc_infer/src/infer/'
HASHES = {
 BASE+'mod.rs':'0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327',
 BASE+'context.rs':'ce9a4271702995747f262724abbfedeb2dbc353dd6ac24a1e726b742acabb886',
 BASE+'opaque_types/mod.rs':'5fa09869ec7c26a4065504a401729e05f588efdffe4bb74998faa14d9cd2a1a6',
 BASE+'opaque_types/table.rs':'633e22af8b50fa814659215d25ff4d7bdfe6bc1973475092a3f565c213beab71',
 BASE+'outlives/obligations.rs':'30b0ed80442caa513768a5b25a28310d3c37370d3f1f588cd3e3a6f210ca2dae',
 'compiler/rustc_middle/src/ty/mod.rs':'bd5acb8cd43199c1a97bce6ac9e699cecba9f5e26f18e3be96e28953283c97e7',
}


def original(path):
 raw=(SUPPORT/path).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=HASHES[path]:raise ValueError('obligation state original differs: '+path)
 return raw.decode()


def record(source, marker):
 start=source.index(marker);end=source.index('\n}',start)+2
 return source[start:end]


def project(text):
 text=adapt(text)
 text=text.replace("ty::OpaqueTypeKey<'db>", "OpaqueTypeKey<'db>")
 text=text.replace('ty::ProvisionalHiddenType', 'ProvisionalHiddenType')
 return text


def extend_obligation_state(outputs, header, rustfmt):
 compiler=original(BASE+'mod.rs')
 outlives=original(BASE+'outlives/obligations.rs')
 hidden=record(original('compiler/rustc_middle/src/ty/mod.rs'), 'pub struct ProvisionalHiddenType')
 # Compiler-only derive paths are not exported on this concrete adapter record;
 # actual Fold/Visit record traits remain a separate integration requirement.
 outputs['opaque_types/mod.rs']=rustfmt(header+'''pub(crate) mod table;
pub(crate) use table::{OpaqueTypeStorage, OpaqueTypeTable, OpaqueTypeStorageEntries};
use super::{Span, Ty};
#[derive(Copy, Clone, Debug)]
'''+project(hidden)+'\n')
 table=original(BASE+'opaque_types/table.rs')
 table=table.replace('use rustc_data_structures::fx::FxIndexMap;', 'use super::super::FxIndexMap;')
 table=table.replace('use rustc_data_structures::undo_log::UndoLogs;', 'use ena::undo_log::UndoLogs;')
 table=table.replace('use rustc_middle::ty::{OpaqueTypeKey, ProvisionalHiddenType, Ty};', 'use super::ProvisionalHiddenType;\nuse super::super::{OpaqueTypeKey, Ty};')
 table=table.replace('use rustc_span::bug;\n','').replace('bug!(', 'panic!(')
 table=table.replace('crate::infer::', 'crate::traits::upstream::infer::')
 outputs['opaque_types/table.rs']=rustfmt(header+project(table))
 outputs['snapshot/undo_log.rs']=outputs['snapshot/undo_log.rs'].replace(b'OpaqueHiddenType',b'ProvisionalHiddenType')
 marker='#[derive(Clone, Debug)]\npub struct TypeOutlivesConstraint'
 body=outputs['mod.rs'].decode();body=body.replace(record(body,marker),project(record(compiler,marker)).replace("ty::Region<DbInterner<'db>>", "Region<'db>"),1)
 outputs['mod.rs']=rustfmt(body)+b'\nmod obligation_state;\n'
 methods=''.join(selected(compiler,n) for n in ['take_opaque_types','clone_opaque_types'])
 methods+=selected(original(BASE+'opaque_types/mod.rs'),'register_hidden_type_in_storage')
 context=original(BASE+'context.rs')
 for name in ['opaque_types_storage_num_entries','clone_opaque_types_lookup_table','clone_duplicate_opaque_types','clone_opaque_types_added_since','add_duplicate_opaque_type','reset_opaque_types']:
  methods+=selected(context,name)
 methods+=selected(outlives,'register_type_outlives_constraint_inner')
 high=selected(outlives,'register_type_outlives_constraint')
 high=high.replace("cause: &ObligationCause<'tcx>", 'span: Span')
 high=high.replace('debug!(?sup_type, ?sub_region, ?cause);','debug!(?sup_type, ?sub_region, ?span);')
 begin=high.index('        let origin = SubregionOrigin::from_obligation_cause(')
 end=high.index('        self.register_type_outlives_constraint_inner',begin)
 high=high[:begin]+'        let origin = span;\n\n'+high[end:]
 methods+=high
 methods+='    #[inline(always)]\n'+selected(compiler,'ty_or_const_infer_var_changed')
 imports='''use super::{InferenceContext, DbInterner, Ty, Region, Span, OpaqueTypeKey, TypeOutlivesConstraint};
use super::{type_variable::TypeVariableValue, unify_key::ConstVariableValue};
use super::opaque_types::{ProvisionalHiddenType, OpaqueTypeStorageEntries};
use super::snapshot::undo_log::UndoLog;
use ena::undo_log::UndoLogs;
use rustc_type_ir::{self as ty, Interner, TypeVisitableExt, solve::TyOrConstInferVar};
use tracing::debug;
'''
 outputs['obligation_state.rs']=rustfmt(header+imports+"impl<'db> InferenceContext<'db> {\n"+project(methods)+'}\n')
