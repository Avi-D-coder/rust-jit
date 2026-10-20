#!/usr/bin/env python3
"""Audit the pinned compiler's exact lang-item spelling vocabulary."""
import argparse,hashlib,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
SOURCE=ROOT/'vendor/rustc-trait-solver/compiler/rustc_attr_ir/src/lang_items.rs'
SYMBOLS=ROOT/'vendor/rustc-trait-solver/compiler/rustc_span/src/symbol.rs'
TARGET=ROOT/'crates/lrr/src/traits/upstream/lang_names.rs'
SHA='947f488e46c70f47f0565d6df979e8b5126af6fb49a8c1c32fe2e4b802c7a6a0'
def expected():
 raw=SOURCE.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=SHA:raise ValueError('pinned compiler lang vocabulary differs')
 symbols=SYMBOLS.read_bytes()
 if hashlib.sha256(symbols).hexdigest()!='75b077bf393efddd9436fbf3d907b55595415a66320e504d649ff05576206358':raise ValueError('pinned symbol spelling differs')
 keyword_text,symbol_text=symbols.decode().split('    Symbols {',1)
 def vocabulary(text):return {a:b or a for a,b in re.findall(r'^\s+(\w+)(?::\s*"([^"]*)")?,\s*(?://[^\n]*)?$',text,re.M)}
 vocab={'kw':vocabulary(keyword_text),'sym':vocabulary(symbol_text)}
 declarations=re.findall(r'^\s+(\w+),\s+(sym|kw)::(?:r#)?(\w+),',raw.decode(),re.M)
 rows=[(a,vocab[namespace][b]) for a,namespace,b in declarations]
 if len(rows)!=len(set(a for a,b in rows)) or not rows:raise ValueError('ambiguous compiler lang vocabulary')
 out='//! Exact pinned rustc_attr_ir lang-item vocabulary; MIT OR Apache-2.0.\n//! tools/upstream_lang_names.py; ae0fe511b15ab760d7e963b13389d1bc2ab42b18.\n'
 out+='pub(super) fn compiler_name(variant:&str)->Option<&\'static str> {\n match variant {\n'
 out+=''.join('  "'+a+'"=>Some("'+b+'"),\n' for a,b in rows)+'  _=>None,\n }\n}\n'
 out+='pub(super) fn known_name(name:&str)->bool { matches!(name,\n'+ '|'.join('"'+n+'"' for n in sorted(set(b for a,b in rows)))+' ) }\n'
 return out.encode()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--write',action='store_true');a=p.parse_args();data=expected()
 if a.write:TARGET.write_bytes(data)
 elif TARGET.read_bytes()!=data:raise ValueError('lang vocabulary differs from pinned generated transport')
 print('Pinned compiler lang-item names match; source roles/parity/registry readiness are separate.')
if __name__=='__main__':main()
