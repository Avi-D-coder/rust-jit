#!/usr/bin/env python3
"""Audit/regenerate the complete pinned discriminant-Type client, no solver fork."""
import argparse, hashlib, re, subprocess
from pathlib import Path
from sync_upstream_binders import method
ROOT=Path(__file__).resolve().parents[1]
PIN='ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
SOURCE=ROOT/'vendor/rustc-trait-solver/compiler/rustc_middle/src/ty/sty.rs'
SHA='6c29bfe00c8de834c370bbc2cfb9d6ffd39d6d662d9145860b9c0153b37f5487'
TARGET=ROOT/'crates/lrr/src/traits/upstream/discriminant.rs'
def expected():
 raw=SOURCE.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=SHA:raise ValueError('pinned discriminant source differs')
 source=raw.decode(); source=source[source.index('    /// Returns the type of the discriminant of this type.'):]
 body=method(source,'discriminant_ty')
 body=body.replace("TyCtxt<'tcx>","Context<'cx,F>").replace("Ty<'tcx>","Ty<'cx,F>")
 body=body.replace('pub fn discriminant_ty','pub(super) fn pinned_discriminant_ty').replace('.discriminant_ty(','.pinned_discriminant_ty(')
 body=body.replace('adt.repr().discr_type().to_ty(tcx)','adt.enum_discriminant_type(tcx)')
 body=body.replace('tcx.associated_item_def_ids(', 'tcx.associated_type_def_ids(')
 body=body.replace('tcx.require_lang_item(LangItem::DiscriminantKind, DUMMY_SP)','tcx.require_trait_lang_item(SolverTraitLangItem::DiscriminantKind)')
 body=body.replace('assoc_items[0]','assoc_items.into_iter().next().expect("prepared DiscriminantKind has its associated type")')
 body=body.replace('tcx.types.u8','Ty::new_u8(tcx)')
 body=body.replace('(*bound_ty).into()','bound_ty.into()')
 variants=['Adt','Coroutine','Param','Alias','Infer','Pat','UnsafeBinder','Bool','Char','Int','Uint','Float','Foreign','Str','Array','Slice','RawPtr','Ref','FnDef','FnPtr','Dynamic','Closure','CoroutineClosure','CoroutineWitness','Never','Tuple','Error','Bound','Placeholder']
 for name in variants:body=re.sub(r'\bty::'+name+r'\b','ty::TyKind::'+name,body)
 body=body.replace('bug!(', 'panic!(')
 # Complete pinned CoroutineArgsExt::discr_ty, whose result is independent of args.
 prelude=SOURCE.read_text();a=prelude.index('    fn discr_ty(');b=prelude.index('\n    }',a)+6
 coroutine=prelude[a:b].replace("TyCtxt<'tcx>","Context<'cx,F>").replace("Ty<'tcx>","Ty<'cx,F>")
 coroutine=coroutine.replace('tcx.types.u32','tcx.ty(ty::TyKind::Uint(ty::UintTy::U32))')
 header=f'''//! Complete pinned compiler discriminant-Type client; MIT OR Apache-2.0.
//! Pin {PIN}; original/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/upstream_discriminant.py.
use super::{{context::Context,facts::SourceFacts,model::Ty}};
use rustc_type_ir::{{self as ty,Interner,inherent::*,InferTy::*}};
use rustc_type_ir::lang_items::SolverTraitLangItem;
use rustc_type_ir::inherent::Ty as _;
trait CoroutineDiscriminant<'cx,F:SourceFacts<'cx>> {{ fn discr_ty(&self,cx:Context<'cx,F>)->Ty<'cx,F>; }}
'''
 text=header+"impl<'cx,F:SourceFacts<'cx>> CoroutineDiscriminant<'cx,F> for ty::CoroutineArgs<Context<'cx,F>> {\n"+coroutine+'}\n'
 text+="impl<'cx,F:SourceFacts<'cx>> Ty<'cx,F> {\n"+body+'}\n'
 return subprocess.run(['rustfmt','+1.99.0','--edition','2024'],input=text.encode(),capture_output=True,check=True).stdout
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--write',action='store_true');a=p.parse_args();data=expected()
 if a.write:TARGET.write_bytes(data)
 if TARGET.read_bytes()!=data:raise SystemExit('discriminant client differs from registered pinned port')
 print('Pinned discriminant-Type client: exact original hash and registered full port match.')
if __name__=='__main__':main()
