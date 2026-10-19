import pathlib,json,hashlib,subprocess,time,os,re,sys
repo=pathlib.Path.cwd();out=pathlib.Path(sys.argv[1]);mode=sys.argv[2]
manifest=json.loads((repo/'fixtures/upstream/rustc-checkout/manifest.json').read_text());compiled=json.loads((out/'after-cli.json').read_text());before=json.loads((out/'before.json').read_text())
assert compiled['source_matches_before'] and compiled['head_matches_before']
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
if mode=='stock':
 binary=repo/'target/debug/lrr';expected=compiled['cli_binary']['sha256']; roots=['tests/ui/numbers-arithmetic/i128.rs','tests/ui/numbers-arithmetic/u128.rs','tests/ui/associated-types/associated-types-struct-field-numbered.rs','tests/ui/associated-types/associated-types-ref-from-struct.rs','tests/ui/consts/issue-17718.rs','tests/ui/associated-types/associated-types-project-from-type-param-via-bound-in-where.rs'];results=[]
 if len(sys.argv)>3:roots=json.loads(pathlib.Path(sys.argv[3]).read_text())
 files={r['path']:r for r in manifest['files']};cases={r['path']:r for r in manifest['cases']}
 for i,path in enumerate(roots):
  assert sha(binary)==expected
  original=repo/'fixtures/upstream/rustc-checkout'/path;upstream=repo/'rustc/rust'/path;assert original.read_bytes()==upstream.read_bytes();assert sha(original)==files[path]['sha256']
  directives=[l for l in original.read_text().splitlines() if l.startswith('//@')];aux=[];args=[str(binary),'--edition','2015']
  if 'issue-17718' in path:
   ap='tests/ui/consts/auxiliary/issue-17718-aux.rs';a=repo/'fixtures/upstream/rustc-checkout'/ap;assert a.read_bytes()==(repo/'rustc/rust'/ap).read_bytes();assert sha(a)==files[ap]['sha256'];aux=[{'path':ap,'sha256':sha(a),'edition':2015}];args+=['--extern','issue_17718_aux='+str(a),'--extern-edition','issue_17718_aux=2015']
  context=hashlib.sha256((str(out)+path+expected).encode()).hexdigest();args+=['run',str(original),'--runtime-capture','--capture-context',context]
  env=os.environ.copy();env['LRR_EMBEDDED_FACADE']='1';start=time.monotonic()
  with (out/f'stock-{i}.stdout').open('wb') as stdout,(out/f'stock-{i}.stderr').open('wb') as stderr:r=subprocess.run(args,stdout=stdout,stderr=stderr,env=env)
  stdout=(out/f'stock-{i}.stdout').read_text();stderr=(out/f'stock-{i}.stderr').read_text()
  try:data=json.loads(stdout)
  except json.JSONDecodeError:data=None
  row={'path':path,'sha256':sha(original),'edition':2015,'manifest_case':cases[path],'directives':directives,'aux':aux,'upstream_commit':manifest['commit'],'binary_sha256':expected,'source_sha256':before['source_sha256'],'exit':r.returncode,'seconds':round(time.monotonic()-start,3),'argv':args,'stdout':f'stock-{i}.stdout','stderr':f'stock-{i}.stderr','json':data,'runtime_contract_pass':r.returncode==0 and bool(data) and data.get('capture',{}).get('complete') is True}
  assert sha(binary)==expected;results.append(row);(out/'stock-results.json').write_text(json.dumps(results,indent=2)+'\n')
  print(json.dumps({'path':path,'exit':row['exit'],'seconds':row['seconds'],'pass':row['runtime_contract_pass'],'terminal':{k:(data.get('terminal') or {}).get(k) for k in ['status','code','message']} if data else stderr[-1800:],'facts':data.get('capture',{}).get('facts') if data else None}),flush=True)
else:
 binary=repo/'target/debug/deps/lrr-3b8db33db37431d2';expected=compiled['test_binary']['sha256'];selectors=json.loads((out/'selected-controls.json').read_text());listing=subprocess.check_output([str(binary),'--list'],text=True);(out/'test-list.txt').write_text(listing);results=[]
 for i,selector in enumerate(selectors):
  assert listing.count(selector+': test\n')==1,selector;assert sha(binary)==expected;start=time.monotonic();args=[str(binary),selector,'--exact','--nocapture','--test-threads=1']
  with (out/f'control-{i}.log').open('wb') as log:r=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT)
  text=(out/f'control-{i}.log').read_text();m=re.search(r'running (\d+) test',text);bodies=int(m.group(1)) if m else 0;row={'selector':selector,'exit':r.returncode,'seconds':round(time.monotonic()-start,3),'bodies':bodies,'pass':r.returncode==0 and bodies==1,'log':f'control-{i}.log','argv':args,'binary_sha256':expected,'source_sha256':before['source_sha256']};assert sha(binary)==expected;results.append(row);(out/'controls.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(row),flush=True)
