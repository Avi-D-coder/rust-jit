import pathlib,json,hashlib,subprocess,datetime,sys
out=pathlib.Path(sys.argv[1]);phase=sys.argv[2]
paths=subprocess.check_output(['git','ls-files','-co','--exclude-standard','-z','crates','vendor','Cargo.toml','Cargo.lock','rust-toolchain.toml']).split(b'\0')
files={p.decode():hashlib.sha256(pathlib.Path(p.decode()).read_bytes()).hexdigest() for p in sorted(set(paths)) if p and pathlib.Path(p.decode()).is_file()}
r={'time_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'head':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'files':files,'file_count':len(files),'source_sha256':hashlib.sha256(json.dumps(files,sort_keys=True,separators=(',',':')).encode()).hexdigest(),'generator_sha256':hashlib.sha256(pathlib.Path('tools/gen_core_int_impls.py').read_bytes()).hexdigest()}
for name,p in [('test_binary','target/debug/deps/lrr-3b8db33db37431d2'),('cli_binary','target/debug/lrr')]:
 if pathlib.Path(p).exists():r[name]={'path':p,'sha256':hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()}
if phase!='before':
 b=json.loads((out/'before.json').read_text());r['source_matches_before']=r['files']==b['files'];r['head_matches_before']=r['head']==b['head'];r['generator_matches_before']=r['generator_sha256']==b['generator_sha256'];r['changed_sources']=[p for p in set(r['files'])|set(b['files']) if r['files'].get(p)!=b['files'].get(p)]
(out/(phase+'.json')).write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({k:v for k,v in r.items() if k!='files'}))
