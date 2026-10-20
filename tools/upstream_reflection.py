#!/usr/bin/env python3
"""Audit the complete pinned compiler reflective-field client transport."""
import argparse,hashlib,re,subprocess
from pathlib import Path
from sync_upstream_binders import method
ROOT=Path(__file__).resolve().parents[1]
PINS={'compiler/rustc_middle/src/ty/adt.rs':'f4f1b017d032c9276fdb32545f3168c64c4b664859da5b8b0ea17332b28f0c34','compiler/rustc_span/src/symbol.rs':'75b077bf393efddd9436fbf3d907b55595415a66320e504d649ff05576206358'}
TARGET=ROOT/'crates/lrr/src/traits/upstream/reflection.rs'
def original(name):
 raw=(ROOT/'vendor/rustc-trait-solver'/name).read_bytes()
 if hashlib.sha256(raw).hexdigest()!=PINS[name]:raise ValueError('reflection compiler dependency differs: '+name)
 return raw.decode()
def expected():
 for name in PINS:original(name)
 integer=method(original('compiler/rustc_span/src/symbol.rs'),'integer')
 # Symbol's representation is owned text rather than rustc's global numeric
 # IDs. Both the static-digit fast path and itoa fallback map to the same
 # canonical decimal text via usize::to_string and a context-owned pool.
 if not all(part in integer for part in ['idx < 10','SYMBOL_DIGITS_BASE','buffer.format(n)','Symbol::intern(printed)']):raise ValueError('pinned integer symbol algorithm changed')
 context=(ROOT/'crates/lrr/src/traits/upstream/context.rs').read_text()
 symbol=method(context.replace('pub(super) fn ', 'pub fn '),'intern_integer_symbol')
 expected_symbol="""    pub fn intern_integer_symbol(self, ordinal:usize)->super::model::Symbol<'cx> {
        super::model::Symbol(self.state.symbol_strings.intern(ordinal.to_string()).value().as_str())
    }
"""
 normalize=lambda text:re.sub(r'\s+','',re.sub(r'//[^\n]*','',text))
 if normalize(symbol)!=normalize(expected_symbol):raise ValueError('integer-symbol representation adapter changed')
 block=method(original('compiler/rustc_middle/src/ty/adt.rs'),'field_representing_type_info')
 block=block.replace('pub fn field_representing_type_info','pub(super) fn pinned_field_representing_type_info')
 block=block.replace("'tcx","'cx").replace("TyCtxt<'cx>","Context<'cx,F>").replace("ty::GenericArgsRef<'cx>","Args<'cx,F>")
 block=block.replace('ty::Adt(', 'ty::TyKind::Adt(').replace('ty::Tuple(', 'ty::TyKind::Tuple(')
 block=block.replace('variant.fields[field_idx]', 'variant.fields[field_idx.as_usize()]').replace('field_idx.index()', 'field_idx.as_usize()')
 block=block.replace('tys.get(', 'tys.items().get(').replace('sym::integer(field_idx.as_usize())','tcx.intern_integer_symbol(field_idx.as_usize())')
 block=re.sub(r'\bbug!\(', 'panic!(', block)
 # rustc AdtDefData::new derives this flag from exact lang-item identity
 # (adt.rs IS_FIELD_REPRESENTING_TYPE initialization), never the type's name.
 code=PRELUDE+"\nimpl<'cx,F:SourceFacts<'cx>> AdtDefinition<'cx,F> {\n"+block+'''
 fn is_field_representing_type(self)->bool {
   self.context.services().field_representing_type_definition(self.context)==Some(self.definition)
 }
}
'''
 return subprocess.run(['rustfmt','+1.99.0','--edition','2024'],input=code.encode(),capture_output=True,check=True).stdout
PRELUDE='''//! Pinned rustc reflective-field client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_reflection.py.
use super::{context::Context,facts::SourceFacts,model::{AdtDefinition,Args}};
use rustc_type_ir::{self as ty,ConstKind,FieldInfo,inherent::*};
use rustc_abi::{FIRST_VARIANT,VariantIdx,FieldIdx};
'''
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--write',action='store_true');a=p.parse_args();data=expected()
 if a.write:TARGET.write_bytes(data)
 elif TARGET.read_bytes()!=data:raise ValueError('reflection client differs from registered compiler transport')
 print('Pinned reflection source and registered client transport match; actual source publication/root execution remain separate.')
if __name__=='__main__':main()
