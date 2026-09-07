"""Iterate source variants against frozen independent fixtures, never local expectations."""
import json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
inventory=json.loads((ROOT/'crates/roze-ta/src/reference_all/compat/inventory.json').read_text())
entries={(e['source_library'],e['name']):e for e in inventory}
bridge=subprocess.Popen(['rtk','proxy',str(ROOT/'target/debug/examples/reference_compare.exe')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding='utf8')
def eq(a,b):
    if a is None or b is None:return a is b
    if isinstance(a,dict):return isinstance(b,dict) and a.keys()==b.keys() and all(eq(a[k],b[k]) for k in a)
    if isinstance(a,list):return isinstance(b,list) and len(a)==len(b) and all(eq(x,y) for x,y in zip(a,b))
    if isinstance(a,(int,float)) and isinstance(b,(int,float)):return abs(a-b)<=2e-9*(1+abs(a))
    return a==b
records=[]
causal_path=ROOT/'crates/roze-ta/tests/fixtures/causal-parity.json'
causal={(f['source'],f['name'],f['case']):f for f in json.loads(causal_path.read_text())['fixtures']} if causal_path.exists() else {}
for lib in ('python','ttr'):
    data=json.loads((ROOT/f'crates/roze-ta/tests/fixtures/{lib}-parity.json').read_text())
    for f in data['fixtures']:
        f=causal.get((f['source'],f['name'],f['case']),f)
        key=f['source'],f['name']
        if key not in entries:continue
        entry=entries[key];r=f['request'];r['operation']['id']=entry['id'];r['samples']=data['datasets'][f['dataset']]
        if f['name']=='FibonacciRetracement':r['samples']=[{**s,'value':s['value']['close']} for s in r['samples']]
        bridge.stdin.write(json.dumps(r)+'\n');bridge.stdin.flush();actual=json.loads(bridge.stdout.readline())
        if 'error' in actual:diffs=[{'error':actual['error']}]
        else:diffs=[{'index':i,'expected':e,'actual':v['value']} for i,(e,v) in enumerate(zip(f['expected'],actual['rows'])) if not eq(e,v['value'])]
        records.append({'source':key[0],'name':key[1],'case':f['case'],'differences':len(diffs),'first':diffs[:1]})
for source,names in [('talipp',['CCI','MassIndex','VTX','VWMA']),('ttr',['ADX','EMV','SMI','stoch'])]:
    data=json.loads((ROOT/f'crates/roze-ta/tests/fixtures/{"python" if source=="talipp" else "ttr"}-parity.json').read_text())
    for name in names:
        f=next(f for f in data['fixtures'] if f['source']==source and f['name']==name)
        r=json.loads(json.dumps(f['request']));r['operation']['id']=entries[source,name]['id']
        samples=data['datasets'][f['dataset']]
        r['samples']=[{**s,'value':{**s['value'],'open':100.,'high':100.,'low':100.,'close':100.,'volume':0.}} for s in samples]
        bridge.stdin.write(json.dumps(r)+'\n');bridge.stdin.flush();actual=json.loads(bridge.stdout.readline())
        ok=actual.get('error_code')=='undefined_result'
        records.append({'source':source,'name':name,'case':'flat_reference_exception','differences':0 if ok else 1,'first':[] if ok else [actual]})
bridge.stdin.close();bridge.wait(timeout=30)
(ROOT/'target/compat-progress.json').write_text(json.dumps(records,indent=2)+'\n')
for source in ('ta','talipp','ttr'):
    rs=[r for r in records if r['source']==source]
    print(source,sum(r['differences']==0 for r in rs),'/',len(rs),flush=True)
    print([(r['name'],r['first']) for r in rs if r['case']=='mixed' and r['differences']])
