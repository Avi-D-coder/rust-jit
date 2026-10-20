#!/usr/bin/env python3
"""Pinned full transmutability automaton plus compiler layout-tree client.

Originals stay in .upstream files; adapters preserve full branches and consume
immutable typed compiler-query leaves. No layout calculation or evaluator.
"""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
from sync_upstream_binders import method
ROOT=Path(__file__).resolve().parents[1]
BASE=ROOT/'vendor/rustc-trait-solver/compiler/rustc_transmute'
PIN='ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
PINS={
'Cargo.toml':'9f9a09b694d0076770d357900b76bbc80494c4efc4be815f2f1c8b72d559060e',
'src/lib.rs':'2a633afa84df60051dec79f9b837ebcefc2d04c47949fd042b80121d23208088',
'src/layout/mod.rs':'9193d455a3e8be89f3b31547464a6263c7728c9164a734ff5022815e49989eb4',
'src/layout/tree.rs':'755cd4c22bf2405d8aeba2ca265341e4cfa3cc2d1702bbffb067813094b69e20',
'src/layout/dfa.rs':'26f40c187b57e39f334cf589eae52024478a2075f4cb196ce79bad4cd37f6863',
'src/maybe_transmutable/mod.rs':'f91b800126a2af653a1323b088e833dd430f5ad95814b9034e23da6378ef0996',
'src/maybe_transmutable/query_context.rs':'fc7bd92708ce9c0e0d45d627563b343d1cd3424250e5a08db8328f6a89bb853b',
'src/layout/tree/tests.rs':'4da0d33e9a6e3c026b9cb92e04ad76448e135589f66938a387cd32a414522c18',
'src/layout/dfa/tests.rs':'6aa647dcbbd8ad4de2fa4f6ab7eddc4b477b89f865ab75fc210ccb927a99c9f1',
'src/maybe_transmutable/tests.rs':'0c5b04c7626159af6f94a6b4d359509bc7ecaacc1a2b31bc7a964896e055763c',
}
HELPERS={
'compiler/rustc_trait_selection/src/solve/delegate.rs':'ad43375618c2e3a7c5ff6f69abfc8a4af33ae1d003cb003d350aea0bb0d17095',
'compiler/rustc_middle/src/ty/sty.rs':'6c29bfe00c8de834c370bbc2cfb9d6ffd39d6d662d9145860b9c0153b37f5487',
'compiler/rustc_middle/src/ty/adt.rs':'f4f1b017d032c9276fdb32545f3168c64c4b664859da5b8b0ea17332b28f0c34',
'compiler/rustc_middle/src/ty/consts.rs':'6337715239e9bf252c749c85d1c19fb2f4b4c5fad2430046f1bcc106ceb2c52c',
'compiler/rustc_middle/src/ty/consts/valtree.rs':'1ab0c5918c3a8b8ff86a3eef72bb726e559ba43d9503433b7ab2c153d7368b81',
'compiler/rustc_middle/src/ty/consts/int.rs':'adc87f2118e4023afcd21b22dfc94553471eca2458d73515336d02808c1da1b9',
'compiler/rustc_data_structures/src/fx.rs':'4aa1903ab2e425e4651bd9fb2954ca76399c6d0cb1cc8ddda214638141605e51',
}
TARGET=ROOT/'crates/lrr/src/traits/upstream/infer/transmute.rs'
def read(path,digest):
 raw=path.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=digest:raise ValueError('pinned transmute original differs: '+str(path))
 return raw.decode()
def original(name):return read(BASE/(name+'.upstream'),PINS[name])
def helper(name):return read(ROOT/'vendor/rustc-trait-solver'/name,HELPERS[name])
def selected(source,name):
 source=re.sub(r'(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?fn\s+', '    pub fn ', source)
 return method(source,name)
def rustfmt(text):return subprocess.run(['rustfmt','+1.99.0','--edition','2024'],input=text.encode(),capture_output=True,check=True).stdout

def stable(name,source):
 source=source.replace('#![cfg_attr(test, feature(test))]', '#![cfg_attr(feature = \"nightly-bench\", feature(test))]').replace('#![feature(option_into_flat_iter)]\n','')
 source=source.replace('pub(crate) use rustc_data_structures::fx::{FxIndexMap as Map, FxIndexSet as Set};','pub type Map<K,V> = indexmap::IndexMap<K,V,rustc_hash::FxBuildHasher>;\npub type Set<V> = indexmap::IndexSet<V,rustc_hash::FxBuildHasher>;')
 source=source.replace('pub(crate)', 'pub').replace('mod maybe_transmutable;', 'pub mod maybe_transmutable;')
 source=source.replace('.into_flat_iter()', '.into_iter().flatten()')
 # Only standalone type !, never !=, logical ! or macro invocations.
 source=re.sub(r'(?<=[<,= ])!(?=[,> ])','std::convert::Infallible',source)
 source=source.replace('impl Def for ! {','impl Def for std::convert::Infallible {').replace('impl Region for ! {}','impl Region for std::convert::Infallible {}').replace('impl Type for ! {}','impl Type for std::convert::Infallible {}')
 if name=='src/maybe_transmutable/tests.rs':
  source=source.replace('rustc_data_structures::fx::FxIndexSet', 'crate::Set')
  source=source.replace('extern crate test;', '#[cfg(feature = \"nightly-bench\")]\nextern crate test;')
  source=source.replace('mod benches {', '#[cfg(feature = "nightly-bench")]\nmod benches {')
 return rustfmt(source)

def adapt(text):
 text=text.replace("'tcx","'db").replace("TyCtxt<'db>","DbInterner<'db>")
 text=text.replace("ty::Const<'db>","Const<'db>")
 text=text.replace("AdtDef<'db>","AdtDefinition<'db, PreparedProvider<'db>>")
 text=text.replace("Region<'db>","Region<'db>")
 text=text.replace("&'db List<Ty<'db>>", "model::Types<'db, PreparedProvider<'db>>")
 text=text.replace('ErrorGuaranteed','ErrorToken')
 text=text.replace('self.0.tcx','infcx.interner').replace('self.tcx.erase_and_anonymize_regions','infcx.interner.erase_and_anonymize_regions')
 text=re.sub(r'\bbug!\(', 'panic!(', text)
 for n in ['Bool','Float','Int','Uint','Tuple','Array','Adt','Ref','Char']:
  text=text.replace('ty::'+n, 'ty::TyKind::'+n)
 return text

def adapter():
 lib=original('src/lib.rs');tree=original('src/layout/tree.rs');mq=original('src/maybe_transmutable/mod.rs')
 adt=helper('compiler/rustc_middle/src/ty/adt.rs');sty=helper('compiler/rustc_middle/src/ty/sty.rs')
 consts=helper('compiler/rustc_middle/src/ty/consts.rs');value=helper('compiler/rustc_middle/src/ty/consts/valtree.rs')
 helper('compiler/rustc_data_structures/src/fx.rs');helper('compiler/rustc_middle/src/ty/consts/int.rs')
 # The complete compiler Assume reader: actual lang identity and named fields.
 assume=adapt(selected(lib,'from_const')).replace('pub fn from_const', 'fn from_const')
 assume=assume.replace('            use rustc_middle::ty::ScalarInt;\n','').replace('            use rustc_span::sym;\n','')
 assume=assume.replace('tcx.is_lang_item(adt_def.did(), LangItem::TransmuteOpts)', 'tcx.services().transmute_inputs().transmute_opts == Some(adt_def.definition)')
 assume=assume.replace('tcx.dcx().delayed_bug(', 'tcx.delay_bug(').replace('LangItem::TransmuteOpts.name()', '"transmute_opts"')
 assume=assume.replace('sym::alignment','"alignment"').replace('sym::lifetimes','"lifetimes"').replace('sym::safety','"safety"').replace('sym::validity','"validity"')
 assume=assume.replace('name == field_def.name', 'name == field_def.name.0')
 assume=assume.replace("fn from_const<'db>",'fn from_const').replace('if !tcx.services().transmute_inputs().transmute_opts == Some(adt_def.definition)', 'if tcx.services().transmute_inputs().transmute_opts != Some(adt_def.definition)')
 assume_impl="trait AssumeFromConst<'db>: Sized { fn from_const(tcx:DbInterner<'db>,ct:Const<'db>)->Option<Self>; }\nimpl<'db> AssumeFromConst<'db> for rustc_transmute::Assume {\n"+assume+'}\n'
 # Pinned compiler convenience helpers, represented by local extension traits.
 methods=''.join(selected(consts,n) for n in ['try_to_value','try_to_leaf'])
 methods=adapt(methods).replace("ty::Value<'db>","model::ValueConst<'db,PreparedProvider<'db>>").replace('pub fn ','fn ')
 const_impl="trait ConstTryExt<'db> { fn try_to_value(self)->Option<model::ValueConst<'db,PreparedProvider<'db>>>;fn try_to_leaf(self)->Option<ScalarInt>; }\nimpl<'db> ConstTryExt<'db> for Const<'db> {\n"+methods+'}\n'
 vl=adapt(selected(value,'try_to_leaf')).replace('match &**self.valtree {','match self.tree.kind() {').replace('Some(*s)','Some(s)')
 value_impl="impl<'db> model::ValueConst<'db,PreparedProvider<'db>> {\n"+vl+'}\n'
 ty_adt=adapt(selected(sty,'ty_adt_def')).replace("Option<AdtDefinition<'db, PreparedProvider<'db>>>","Option<AdtDefinition<'db,PreparedProvider<'db>>>")
 ty_adt=ty_adt.replace('Adt(adt, _) => Some(*adt)', 'ty::TyKind::Adt(adt, _) => Some(adt)')
 ty_box=adapt(selected(sty,'is_box')).replace('Adt(def, _)', 'ty::TyKind::Adt(def, _)')
 ty_impl="impl<'db> Ty<'db> {\n"+ty_adt+ty_box+'}\n'
 nonenum=adapt(selected(adt,'non_enum_variant')).replace("&'db VariantDef", "&'db model::AdtVariant<'db,PreparedProvider<'db>>")
 kind=adapt(selected(adt,'adt_kind'))
 adt_impl="impl<'db> AdtDefinition<'db,PreparedProvider<'db>> {\n"+nonenum+kind+'''fn is_union(self)->bool { self.context.services().adt_info(self.context,self.definition).kind==AdtKind::Union }
 fn is_box(self)->bool { self.context.is_adt_lang_item(self.definition,ty::lang_items::SolverAdtLangItem::OwnedBox) }
}\n'''
 # Full compiler tree constructors, with query fallibility separated from rustc errors.
 names=['from_ty','from_tuple','from_struct','from_enum','from_variant','from_tag','from_union']
 body=adapt(''.join(selected(tree,n) for n in names))
 body=body.replace('            use rustc_abi::HasDataLayout;\n','')
 body=body.replace('*inner_ty','inner_ty').replace('*adt_def','adt_def').replace('*region','region').replace('referent: *ty','referent: TransmuteTy(ty)').replace('region: region,','region: TransmuteRegion(region),').replace('layout_of(cx, *ty)', 'layout_of(cx, ty)')
 body=body.replace('pub fn ','fn ').replace('members[0]','members.items()[0]')
 body=body.replace('cx.data_layout().endian.into()', 'match cx.data_layout().endian { rustc_abi::Endian::Little=>rustc_transmute::layout::tree::Endian::Little, rustc_abi::Endian::Big=>rustc_transmute::layout::tree::Endian::Big }')
 body=body.replace('ty_variant(cx, (ty, layout), index);','ty_variant(cx, (ty, layout), index)?;')
 body=re.sub(r'cx\.tcx\(\)\.tag_for_variant\(\s*cx\.typing_env\.as_query_input\(\(cx\.tcx\(\)\.erase_and_anonymize_regions\(ty\), index\)\),\s*\)', 'cx.tag_for_variant(ty,index)?',body)
 body=body.replace('Def::Variant(def.variant(index))','Def::Variant { owner:def, index }')
 body=body.replace('def.discriminants(cx.tcx())', 'cx.discriminants(def)?.iter().map(|discr| (discr.index,discr.value))')
 body=body.replace('ty_field(cx, (ty, layout), field_idx);','ty_field(cx, (ty, layout), field_idx)?;').replace('ty_field(cx, (ty, layout), idx);','ty_field(cx, (ty, layout), idx)?;')
 body=body.replace('fields.iter_enumerated()', 'fields.iter().enumerate()')
 body=body.replace('|fields, (idx, _field_def)| {', '|fields, (idx, _field_def)| {\n let idx=FieldIdx::from_usize(idx);')
 body=body.replace('cx.tcx().data_layout.endian', 'cx.data_layout().endian').replace('tcx.data_layout.endian', 'tcx.services().transmute_inputs().target.endian')
 body=body.replace('cx.data_layout().endian.into()', 'match cx.data_layout().endian { rustc_abi::Endian::Little=>rustc_transmute::layout::tree::Endian::Little, rustc_abi::Endian::Big=>rustc_transmute::layout::tree::Endian::Big }')
 declarations=''
 for n in names:
  raw=adapt(selected(tree,n));sig=raw[:raw.index(' {')]
  sig=re.sub(r'\(ty, layout\):','ty_and_layout:',sig).replace('pub fn ','fn ')
  declarations+=sig+';\n'
 constructors="trait CompilerTree<'db>: Sized {\n"+declarations+"}\nimpl<'db> CompilerTree<'db> for Tree<'db> {\n"+body+'}\n'
 # Source-preserving field projection; tag primitive -> Type is a typed query leaf.
 field=adapt(selected(tree,'ty_field')).replace('pub fn ty_field','fn ty_field').replace("-> Ty<'db>","-> Result<Ty<'db>, Err<'db>>")
 field=field.replace('        match ty.kind() {', '        Ok(match ty.kind() {')
 field=field.replace('.fields[i];','.fields[i.as_usize()];').replace('fields[i.as_usize()]','fields.items()[i.as_usize()]')
 field=field.replace('ty::layout::PrimitiveExt::to_ty(&tag.primitive(), cx.tcx())','cx.primitive_type(tag.primitive())?').replace('.fields.items()[','.fields[')
 # Close the newly wrapped result at the final match, not the function body.
 pos=field.rfind('\n        }');field=field[:pos]+field[pos:].replace('\n        }','\n        })',1)
 variant=adapt(selected(tree,'ty_variant')).replace('pub fn ty_variant','fn ty_variant').replace("-> Layout<'db>","-> Result<Layout<'db>,Err<'db>>")
 variant=variant.replace('TyAndLayout { ty, layout }.for_variant(&cx, i).layout','cx.variant_layout(ty,layout,i)')
 # Preserve full source/DST error precedence; admission failures are separate.
 answer=selected(mq,'answer')
 answer=adapt(answer)
 start=answer.index('        {') if '\n        {' in answer else answer.index(' {')+1
 answer_body=answer[start:]
 answer_body=answer_body.replace('let Self { src, dst, assume, context } = self;','')
 answer_body=answer_body.replace('LayoutCx::new(context, TypingEnv::fully_monomorphized())','LayoutCx::new(context, TypingEnv::fully_monomorphized(context))')
 answer_body=answer_body.replace('            match (src, dst) {','            Ok(match (src, dst) {\n (Err(Err::Preparation(dep)),_) | (_,Err(Err::Preparation(dep)))=>return Err(dep),')
 answer_body=answer_body.replace('MaybeTransmutableQuery { src, dst, assume, context }.answer()', 'MaybeTransmutableQuery::new(src, dst, assume, context).answer()')
 pos=answer_body.rfind('\n            }');answer_body=answer_body[:pos]+answer_body[pos:].replace('\n            }','\n            })',1)
 answer_fn="fn answer<'db>(src:Ty<'db>,dst:Ty<'db>,assume:rustc_transmute::Assume,context:DbInterner<'db>)->Result<Answer<TransmuteRegion<'db>,TransmuteTy<'db>>,TransmuteDependency<'db>> "+answer_body
 # Pinned delegate branch mapping plus explicit outer preparation dependency.
 delegate=adapt(selected(helper('compiler/rustc_trait_selection/src/solve/delegate.rs'),'is_transmutable'))
 delegate=delegate.replace('    pub fn is_transmutable(\n        &self,',"pub(crate) fn is_transmutable<'db>(\n        infcx:&InferenceContext<'db>,")
 delegate=delegate.replace('Result<Certainty, NoSolution>','Result<Result<Certainty,NoSolution>,TransmuteDependency<\'db>>')
 delegate=delegate.replace('self.tcx','infcx.interner').replace('return Err(NoSolution);','return Ok(Err(NoSolution));')
 delegate=delegate.replace('match rustc_transmute::TransmuteTypeEnv::new(infcx.interner).is_transmutable(src, dst, assume) {','Ok(match answer(src,dst,assume,infcx.interner)? {')
 pos=delegate.rfind('\n        }');delegate=delegate[:pos]+delegate[pos:].replace('\n        }','\n        })',1)
 boundary=BOUNDARY
 layout=original('src/layout/mod.rs')
 safety=selected(layout[layout.index("impl<'tcx> super::Def for Def<'tcx>"):], 'has_safety_invariants').replace('pub fn ','fn ')
 boundary=boundary.replace(' fn has_safety_invariants(&self)->bool {self!=&Self::Primitive}',safety)
 # Preserve the compiler's entire error categorization method, discarding only
 # diagnostic payloads which that method itself never observes.
 err_start=tree.index('pub(crate) enum Err {');err_end=tree.index('\n    }',err_start)+6
 err_enum=tree[err_start:err_end].replace('pub(crate) enum Err {',"enum Err<'db> {").replace('ErrorGuaranteed','ErrorToken')
 err_enum=err_enum[:-1]+"Preparation(TransmuteDependency<'db>),\n}"
 convert=selected(tree[tree.index("impl<'tcx> From<&LayoutError<'tcx>>"):],'from')
 convert=adapt(convert).replace("LayoutError<'db>",'LayoutError').replace('pub fn ','fn ')
 for category in ['Unknown','ReferencesError','TooGeneric','NormalizationFailure','SizeOverflow']:
  convert=convert.replace('LayoutError::'+category+'(..)','LayoutError::'+category)
 convert=convert.replace('LayoutError::InvalidSimd { .. }','LayoutError::InvalidSimd')
 old_start=boundary.index('#[derive(Debug,Copy,Clone)]\nenum Err')
 old_end=boundary.index('#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]',old_start)
 boundary=boundary[:old_start]+'#[derive(Debug,Copy,Clone)]\n'+err_enum+"\nimpl<'db>From<&LayoutError>for Err<'db>{\n"+convert+'}\n'+boundary[old_end:]

 scalar=helper('compiler/rustc_middle/src/ty/consts/int.rs')
 truth=re.search(r'pub const TRUE: ScalarInt = [^;]+;',scalar).group(0)
 truth=truth.replace('pub const','const').replace('data:','bits:').replace('size: NonZero::new(1).unwrap()','bytes: 1')
 boundary=boundary.replace('const TRUE:Self=Self{bits:1,bytes:1};',truth)
 text=PRELUDE+boundary+assume_impl+const_impl+value_impl+ty_impl+adt_impl+constructors+field+variant+answer_fn+delegate
 text=re.sub(r'\bErr(?=[>,])', "Err<'db>", text)
 return rustfmt(text)

def outputs():
 read(BASE/'LICENSE-MIT','b71bd43a069ca0641a9ecfe585ca7b3c53b5cc1608f8b68321168698e28b5ea1')
 read(BASE/'LICENSE-APACHE','62c7a1e35f56406896d7aa7ca52d0cc0d272ac022b5d2796e7d6905db8a3636a')
 out={BASE/name:stable(name,original(name)) for name in PINS if name!='Cargo.toml'}
 out[BASE/'Cargo.toml']=CARGO.encode()
 out[TARGET]=adapter()
 return out
CARGO='''# Stable generic packaging; exact original retained in Cargo.toml.upstream.
[package]
name="rustc_transmute"
version="0.0.0"
edition="2024"
license="MIT OR Apache-2.0"
rust-version="1.99"
[dependencies]
indexmap="=2.14.2"
rustc-hash="=2.1.3"
smallvec="=1.16.2"
tracing="=0.1.44"
[dev-dependencies]
itertools="0.15"
[features]
default=[]
rustc=[]
nightly-bench=[]
'''
PRELUDE='''//! Pinned full transmutability compiler client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_transmute.py.
use super::{Const,DbInterner,InferenceContext,PreparedProvider,Region,Ty};
use super::const_eval::TypingEnv;
use crate::traits::upstream::{facts::SourceFacts as _,identity::DefinitionId,
 model::{self,AdtDefinition,AdtKind,ScalarInt,ErrorToken}};
use rustc_type_ir::{self as ty,inherent::*,Interner,TypeVisitableExt,ValTreeKind};
use rustc_abi::{FieldIdx,VariantIdx,FIRST_VARIANT,FieldsShape,LayoutData,Primitive,Size,TagEncoding,TargetDataLayout,Variants};
use rustc_transmute::{Answer,Reason,layout::tree::Tree as RawTree,layout::Reference,
 maybe_transmutable::{MaybeTransmutableQuery,query_context::QueryContext}};
use rustc_type_ir::solve::{Certainty,NoSolution};
use std::{collections::HashMap,hash::{Hash,Hasher},ops::Deref};
type Tree<'db>=RawTree<Def<'db>,TransmuteRegion<'db>,TransmuteTy<'db>>;
'''
BOUNDARY='''
#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]
pub(crate) struct TransmuteRegion<'db>(pub(crate) Region<'db>);
#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]
pub(crate) struct TransmuteTy<'db>(pub(crate) Ty<'db>);
impl rustc_transmute::layout::Region for TransmuteRegion<'_> {}
impl rustc_transmute::layout::Type for TransmuteTy<'_> {}
#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]
pub(crate) enum Def<'db> {
 Adt(AdtDefinition<'db,PreparedProvider<'db>>),
 Variant{owner:AdtDefinition<'db,PreparedProvider<'db>>,index:VariantIdx},
 Field{owner:AdtDefinition<'db,PreparedProvider<'db>>,variant:VariantIdx,index:FieldIdx},
 Primitive,
}
impl rustc_transmute::layout::Def for Def<'_> {
 fn has_safety_invariants(&self)->bool {self!=&Self::Primitive}
}
impl<'db> QueryContext for DbInterner<'db> {type Def=Def<'db>;type Region=TransmuteRegion<'db>;type Type=TransmuteTy<'db>;}
/// Same interned immutable ABI LayoutData handle, without compiler-only traits.
#[derive(Debug,Copy,Clone)]
pub(crate) struct Layout<'db>(pub(crate) &'db LayoutData<FieldIdx,VariantIdx>);
impl PartialEq for Layout<'_>{fn eq(&self,b:&Self)->bool{std::ptr::eq(self.0,b.0)}}
impl Eq for Layout<'_>{}
impl Hash for Layout<'_>{fn hash<H:Hasher>(&self,h:&mut H){std::ptr::from_ref(self.0).hash(h)}}
impl<'db>Deref for Layout<'db>{type Target=LayoutData<FieldIdx,VariantIdx>;fn deref(&self)->&Self::Target{self.0}}
impl<'db>Layout<'db>{fn fields(self)->&'db FieldsShape<FieldIdx>{&self.0.fields}fn variants(self)->&'db Variants<FieldIdx,VariantIdx>{&self.0.variants}}
#[derive(Debug,Copy,Clone,Eq,PartialEq)]
pub(crate) enum LayoutError{Unknown,ReferencesError,TooGeneric,InvalidSimd,NormalizationFailure,SizeOverflow}
#[derive(Debug,Copy,Clone)]
enum Err<'db>{NotYetSupported,UnknownLayout,SizeOverflow,TypeError(ErrorToken),Preparation(TransmuteDependency<'db>)}
impl From<&LayoutError> for Err<'_>{fn from(error:&LayoutError)->Self{match error{LayoutError::Unknown|LayoutError::ReferencesError|LayoutError::TooGeneric|LayoutError::InvalidSimd|LayoutError::NormalizationFailure=>Self::UnknownLayout,LayoutError::SizeOverflow=>Self::SizeOverflow}}}
#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]
pub(crate) struct LayoutQuery<'db>{pub(crate) typing_env:TypingEnv<'db>,pub(crate) ty:Ty<'db>}
#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]
pub(crate) struct VariantQuery<'db>{pub(crate) base:LayoutQuery<'db>,pub(crate) parent:Layout<'db>,pub(crate) index:VariantIdx}
#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]
pub(crate) struct TagQuery<'db>{pub(crate) base:LayoutQuery<'db>,pub(crate) index:VariantIdx}
#[derive(Debug,Copy,Clone,Eq,PartialEq,Hash)]
pub(crate) enum TransmuteDependency<'db>{Layout(LayoutQuery<'db>),Variant(VariantQuery<'db>),Tag(TagQuery<'db>),Primitive(Primitive),Discriminants(DefinitionId)}
#[derive(Debug,Copy,Clone)]
pub(crate) struct Discriminant<'db>{pub(crate) index:VariantIdx,pub(crate) value:u128,pub(crate) ty:Ty<'db>}
/// Typed immutable query data only: no constructor grants complete-source/root readiness.
pub(crate) struct PreparedTransmuteInputs<'db>{
 pub(crate) target:&'db TargetDataLayout,
 pub(crate) transmute_opts:Option<DefinitionId>,
 pub(crate) layouts:HashMap<LayoutQuery<'db>,Result<Layout<'db>,LayoutError>>,
 pub(crate) variants:HashMap<VariantQuery<'db>,Layout<'db>>,
 pub(crate) tags:HashMap<TagQuery<'db>,Option<ScalarInt>>,
 pub(crate) primitives:HashMap<Primitive,Ty<'db>>,
 pub(crate) discriminants:HashMap<DefinitionId,&'db[Discriminant<'db>]>,
}
#[derive(Copy,Clone)]
struct LayoutCx<'db>{tcx:DbInterner<'db>,typing_env:TypingEnv<'db>}
impl<'db>LayoutCx<'db>{
 fn new(tcx:DbInterner<'db>,typing_env:TypingEnv<'db>)->Self{Self{tcx,typing_env}}
 fn tcx(self)->DbInterner<'db>{self.tcx}
 fn data_layout(self)->&'db TargetDataLayout{self.tcx.services().transmute_inputs().target}
 fn query(self,ty:Ty<'db>)->LayoutQuery<'db>{LayoutQuery{typing_env:self.typing_env,ty:self.tcx.erase_and_anonymize_regions(ty)}}
 fn variant_layout(self,ty:Ty<'db>,parent:Layout<'db>,index:VariantIdx)->Result<Layout<'db>,Err<'db>>{
  let query=VariantQuery{base:self.query(ty),parent,index};self.tcx.services().transmute_inputs().variants.get(&query).copied().ok_or(Err::Preparation(TransmuteDependency::Variant(query)))
 }
 fn tag_for_variant(self,ty:Ty<'db>,index:VariantIdx)->Result<Option<ScalarInt>,Err<'db>>{
  let query=TagQuery{base:self.query(ty),index};self.tcx.services().transmute_inputs().tags.get(&query).copied().ok_or(Err::Preparation(TransmuteDependency::Tag(query)))
 }
 fn primitive_type(self,primitive:Primitive)->Result<Ty<'db>,Err<'db>>{self.tcx.services().transmute_inputs().primitives.get(&primitive).copied().ok_or(Err::Preparation(TransmuteDependency::Primitive(primitive)))}
 fn discriminants(self,adt:AdtDefinition<'db,PreparedProvider<'db>>)->Result<&'db[Discriminant<'db>],Err<'db>>{self.tcx.services().transmute_inputs().discriminants.get(&adt.definition).copied().ok_or(Err::Preparation(TransmuteDependency::Discriminants(adt.definition)))}
}
fn layout_of<'db>(cx:LayoutCx<'db>,ty:Ty<'db>)->Result<Layout<'db>,Err<'db>>{
 let query=cx.query(ty);cx.tcx.services().transmute_inputs().layouts.get(&query).copied().ok_or(Err::Preparation(TransmuteDependency::Layout(query)))?.map_err(|error|Err::from(&error))
}
impl ScalarInt {const TRUE:Self=Self{bits:1,bytes:1};}
'''

def manifest_rows():
 rows=[]
 for name,digest in PINS.items():
  for suffix in ['', '.upstream']:
   path=BASE/(name+suffix);raw=path.read_bytes()
   rows.append({'path':path.relative_to(ROOT/'vendor/rustc-trait-solver').as_posix(),'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'upstream':'compiler/rustc_transmute/'+name,'upstream_sha256':digest})
 for name in ['LICENSE-MIT','LICENSE-APACHE']:
  path=BASE/name;raw=path.read_bytes();digest=hashlib.sha256(raw).hexdigest()
  rows.append({'path':path.relative_to(ROOT/'vendor/rustc-trait-solver').as_posix(),'bytes':len(raw),'sha256':digest,'upstream':name,'upstream_sha256':digest})
 return rows

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--write',action='store_true');p.add_argument('--manifest-rows',action='store_true');a=p.parse_args()
 if a.manifest_rows:
  print(json.dumps(manifest_rows(),indent=2));return
 for path,data in outputs().items():
  if a.write:path.write_bytes(data)
  elif path.read_bytes()!=data:raise ValueError('transmute adaptation differs: '+str(path))
 print('Pinned complete transmute generic/client transforms match; actual query producer/root/runtime acceptance remains held.')
if __name__=='__main__':main()
