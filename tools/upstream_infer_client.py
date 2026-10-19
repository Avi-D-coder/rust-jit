"""Pinned complete InferCtxtLike implementation, lower-universe folder and
compiler generalizer. No custom solver, relation, canonicalizer or stub callbacks.
"""
import hashlib,re
from pathlib import Path
from upstream_region_collector import selected
from upstream_obligation_state import project

ROOT=Path(__file__).resolve().parents[1]
SUPPORT=ROOT/'vendor/rustc-trait-solver'
BASE='compiler/rustc_infer/src/infer/'
HASHES={
 BASE+'context.rs':'ce9a4271702995747f262724abbfedeb2dbc353dd6ac24a1e726b742acabb886',
 BASE+'mod.rs':'0e3d962f0dae5a19ae4293de73a606c692e222eae08043bc1beb9d0cd0f6f327',
 BASE+'relate/generalize.rs':'782bf0cc12f8128332d7b09e9e37593637c3b8298034a36e98be7d98b243c9b1',
 BASE+'type_variable.rs':'2e5f26dba08abf4f96c52975f1ac0fba42c37d2e650de96f9a182314e7b88f53',
}


def original(path):
 raw=(SUPPORT/path).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=HASHES[path]:raise ValueError('infer client original differs: '+path)
 return raw.decode()


def adapt(text):
 text=project(text)
 text=text.replace("ty::TypingMode<'db>", "ty::TypingMode<DbInterner<'db>>")
 text=text.replace("ty::GenericArgsRef<'db>", "crate::traits::upstream::model::Args<'db, PreparedProvider<'db>>")
 for name in ('PlaceholderType','PlaceholderRegion','PlaceholderConst','AliasTerm','AliasConst','OpaqueAliasTy'):
  text=text.replace('ty::'+name+"<'db>", 'ty::'+name+"<DbInterner<'db>>")
 text=re.sub(r'\bty::(Re[A-Z]\w*)', r'ty::RegionKind::\1', text)
 text=text.replace('RegionVariableOrigin::Misc(DUMMY_SP)', 'DUMMY_SP').replace('RegionVariableOrigin::Misc(self.span)', 'self.span')
 text=text.replace('SubregionOrigin::RelateRegionParamBound(span, None)', 'span')
 text=text.replace('&ObligationCause::dummy_with_span(span)', 'span')
 text=text.replace('self.infcx.const_var_origin(', 'self.infcx.const_var_span(')
 text=text.replace('self.const_var_origin(', 'self.const_var_span(')
 text=re.sub(r'(ConstVariableValue::Unknown\s*\{\s*)origin\b', r'\1span: origin', text)
 text=text.replace('*t.kind()', 't.kind()').replace('*hidden_ty.ty.kind()', 'hidden_ty.ty.kind()')
 text=text.replace('root_var(*vid)', 'root_var(vid)')
 text=re.sub(r'\bbug!\(', 'panic!(', text)
 text=text.replace('ty::VarianceDiagInfo', 'ty::relate::VarianceDiagInfo')
 text=text.replace('.expect_type()', '.expect_ty()')
 text=text.replace('.var_origin(', '.var_span(')
 text=text.replace('if let Some(r) = r.ty_vid()', 'if let ty::Infer(ty::TyVar(r)) = r.kind()')
 text=text.replace('unexpected infer type: {t}', 'unexpected infer type: {t:?}')
 for kind in ('ty','region','const'):
  for origin in ('DUMMY_SP','self.span'):
   text=re.sub(r'\.next_'+kind+r'_var_in_universe\(\s*'+re.escape(origin)+r',\s*self\.for_universe\s*,?\s*\)', f'.next_{kind}_var_in_universe(self.for_universe, {origin})', text)
 # The compatible source table exposes the exact same Unknown/Known predicate
 # through its known() Option; no new value or inference behavior.
 text=re.sub(r'\.probe_value\(target_vid\)\s*\.is_unknown\(\)', '.probe_value(target_vid).known().is_none()', text)
 text=text.replace("TypingMode<'db>", "ty::TypingMode<DbInterner<'db>>")
 text=text.replace('tcx.features().generic_const_exprs()', "ty::inherent::Features::<DbInterner<'db>>::generic_const_exprs(tcx.features())")
 return text


def extend_infer_client(outputs, header, rustfmt):
 source=original(BASE+'context.rs')
 body=source[source.index("impl<'tcx> rustc_type_ir::InferCtxtLike"):]
 body=body.replace('    fn next_trait_solver(&self) -> bool {\n        self.next_trait_solver\n    }', '''    fn next_trait_solver(&self) -> bool {
        // Shared InferCtxtLike default: noncompiler clients use the new solver.
        true
    }''')
 body=body.replace('self.disable_trait_solver_fast_paths()', 'self.interner.services().disable_trait_solver_fast_paths()',1)
 body=body.replace('self.typing_mode_raw()', 'self.typing_mode',1)
 # Compiler inherent ownership is represented by Deref here; qualify calls
 # so the trait's methods do not shadow the actual snapshot/universe owner.
 for name in ('universe','create_next_universe'):
  body=body.replace(f'        self.{name}()\n', f'        super::InferenceStorage::{name}(self)\n',1)
 body=body.replace('self.probe(|_| probe())', 'super::InferenceStorage::probe(self, |_| probe())')
 body=body.replace('self.commit_if_ok(|_| f())', 'super::InferenceStorage::commit_if_ok(self, |_| f())')
 # Compiler has an inherent origin-first helper; this adapter has the real trait
 # entry, taking actual regions/visibility/span. Preserve the source dummy FIXME.
 body=body.replace('''SubregionOrigin::RelateRegionParamBound(DUMMY_SP, None),
                        r,
                        new_region,
                        ty::VisibleForLeakCheck::Yes,''', '''r,
                        new_region,
                        ty::VisibleForLeakCheck::Yes,
                        DUMMY_SP,''')
 imports='''use super::{InferenceContext, DbInterner, PreparedProvider, Ty, Const, Region, Span, OpaqueTypeKey};
use super::{type_variable::TypeVariableValue, unify_key::ConstVariableValue};
use super::opaque_types::{ProvisionalHiddenType, OpaqueTypeStorageEntries};
use super::variables::BoundRegionConversionTime;
use crate::traits::upstream::{identity::DefinitionId as DefId, model::ErrorToken as ErrorGuaranteed};
use rustc_type_ir::{self as ty, TypeFoldable, TypeSuperFoldable, TypeVisitableExt, InferCtxtLike, Interner, inherent::*, relate::combine::PredicateEmittingRelation, solve::TyOrConstInferVar};
use rustc_type_ir::inherent::{Ty as _, Term as _};
use rustc_type_ir::data_structures::SsoHashMap;
use std::range::RangeInclusive;
type RelateResult<'db,T> = ty::relate::RelateResult<DbInterner<'db>,T>;
const DUMMY_SP: Span = Span { origin: crate::core::digest::Digest([0;32]), start:0,end:0 };
'''
 # Digest has no fabricated source identity: use the exact existing dummy span
 # representation. Its const construction spelling is checked by the build.
 outputs['client.rs']=rustfmt(header+imports+adapt(body))
 general=original(BASE+'relate/generalize.rs')
 general=general[general.index('#[derive(Copy, Clone, Eq, PartialEq, Debug)]'):]
 imports='''use std::mem;
use super::{InferenceContext, DbInterner, PreparedProvider, Ty, Const, Span};
use super::{type_variable::TypeVariableValue, unify_key::ConstVariableValue};
use crate::traits::upstream::identity::DefinitionId as DefId;
use rustc_type_ir::{self as ty, InferCtxtLike, InferConst, Interner, TypeFlags, TypeVisitableExt, inherent::*, relate::{self, Relate, TypeRelation, combine::PredicateEmittingRelation}};
use rustc_type_ir::inherent::{Ty as _, Term as _};
use rustc_type_ir::data_structures::SsoHashMap;
use tracing::{debug,instrument,warn};
type Term<'db> = crate::traits::upstream::model::Term<'db,PreparedProvider<'db>>;
type RelateResult<'db,T> = ty::relate::RelateResult<DbInterner<'db>,T>;
type TypeError<'db> = ty::error::TypeError<DbInterner<'db>>;
'''
 outputs['generalize.rs']=rustfmt(header+imports+adapt(general))
 compiler=original(BASE+'mod.rs')
 operations=selected(compiler,'opaques_with_sub_unified_hidden_type')
 operations+=selected(compiler,'typing_mode_raw')
 outputs['client_operations.rs']=rustfmt(header+'''use super::{InferenceContext, DbInterner};
use rustc_type_ir::{self as ty, InferCtxtLike, inherent::*, Interner, TyVid};
impl<'db> InferenceContext<'db> {
'''+adapt(operations)+'}\n')
 table=original(BASE+'type_variable.rs')
 accessor=selected(table.replace('    pub(crate) fn ', '    pub fn '),'sub_unification_table_ref')
 outputs['context_probe.rs']=rustfmt(header.replace('//!','//')+"impl<'db> TypeVariableStorage<'db> {\n"+adapt(accessor)+'}\n')
 outputs['type_variable.rs']+=b'\ninclude!("context_probe.rs");\n'
 outputs['mod.rs']+=b'\nmod client;\nmod client_operations;\nmod generalize;\n'
