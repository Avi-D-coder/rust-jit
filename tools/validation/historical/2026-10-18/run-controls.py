import subprocess,json,pathlib,time
out=pathlib.Path(__file__).parent
rows=[]
selectors=json.loads((out/'selected-controls.json').read_text())

for n,s in enumerate(selectors):
 start=time.monotonic()
 with (out/f'control-{n}.log').open('w') as f:
  r=subprocess.run(['target/debug/deps/lrr-3b8db33db37431d2',s,'--exact','--nocapture'],stdout=f,stderr=subprocess.STDOUT)
 log=(out/f'control-{n}.log').read_text()
 row={'selector':s,'exit':r.returncode,'one_body':'running 1 test' in log,'seconds':round(time.monotonic()-start,3),'log':str(out/f'control-{n}.log')}
 rows.append(row);print(json.dumps(row),flush=True)
 (out/'control-results.json').write_text(json.dumps(rows,indent=2)+'\n')
