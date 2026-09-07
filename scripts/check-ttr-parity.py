"""Compare all TTR mappings, keeping native-version and output-shape differences explicit."""
import hashlib,json,math,random,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
from parity_sources import verify_source
verify_source('ttr')
rows=[r for r in json.loads((ROOT/'docs/reference-indicator-mapping.json').read_text(encoding='utf8'))['entries'] if r['source']=='ttr' and r['status']!='non_indicator']
def bars(case,n=512):
    rng=random.Random(51881);result=[]
    for i in range(n):
        if case=='flat':o=c=h=l=100.;v=0.
        elif case=='trend':o=100.+i*.1;c=o+.05;h=c+.1;l=o-.1;v=100.+i%5
        else:o=100+10*math.sin(i*.41)+rng.uniform(-4,4);c=o+rng.uniform(-4,4);h=max(o,c)+rng.uniform(0,4);l=min(o,c)-rng.uniform(0,4);v=10+rng.random()*100
        result.append(dict(open=o,high=h,low=l,close=c,volume=v,timestamp=i+1))
    return result
datasets={case:bars(case) for case in ('mixed','flat','trend')}
(ROOT/'target/ttr-parity-input.json').write_text(json.dumps({'entries':rows,'datasets':datasets}),encoding='utf8')
subprocess.run(['rtk','proxy',str(ROOT/'target/R-runtime/app/bin/x64/Rscript.exe'),'--vanilla',str(ROOT/'scripts/generate-ttr-parity.R')],check=True,cwd=ROOT)
oracle=json.loads((ROOT/'target/ttr-parity-output.json').read_text())
oracle['oracle']['native_dll_sha256']=hashlib.sha256((ROOT/'target/cross-library/ttr-msvc/build/Release/TTR.dll').read_bytes()).hexdigest()
bridge=subprocess.Popen(['rtk','proxy',str(ROOT/'target/debug/examples/reference_compare.exe')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding='utf8')
entries={r['name']:r for r in rows};records=[];fixtures=[];inputs={}
aliases={'dn':'lower','mavg':'middle','up':'upper','diplus':'plus_di','diminus':'minus_di','aroonUp':'up','aroonDn':'down','fastK':'k','fastD':'d','DIp':'plus_di','DIn':'minus_di','ADX':'adx','SNR':'snr','DI':'di','TDI':'tdi','smi':'smi','signal':'signal','magnitude':'magnitude','stretch':'stretch','dvi':'dvi'}
def equal(a,b):
    if a is None or b is None:return a is b
    if isinstance(a,dict):return isinstance(b,dict) and a.keys()==b.keys() and all(equal(a[k],b[k]) for k in a)
    if isinstance(a,(int,float)) and isinstance(b,(int,float)):return abs(a-b)<=2e-9*(1+abs(a))
    return a==b
for record in oracle['records']:
    if record['status']=='oracle_error':records.append(record);continue
    row=entries[record['name']];candles=datasets[record['case']];n=len(candles)
    values=candles if row['input_type']=='Candle' else [[c['close'],c['close']*.8+math.cos((i+1)*.37)] for i,c in enumerate(candles)] if row['input_type']=='(f64, f64)' else [c['close'] for c in candles]
    expected=[{aliases.get(k,k):v for k,v in e.items()} if isinstance(e,dict) else e for e in record.pop('expected')]
    request={'schema_version':1,'identity':{'series_id':'parity','instrument':'TEST','timeframe':'1ms','source':'independent-R','data_version':'v1'},'operation':{'id':row['operation_id'],'params':row['parameters']},'as_of_ms':n+1,'samples':[{'at_ms':i+1,'available_at_ms':i+2,'value':v} for i,v in enumerate(values)]}
    bridge.stdin.write(json.dumps(request)+'\n');bridge.stdin.flush();actual=json.loads(bridge.stdout.readline())
    if 'error' in actual:record.update(status='local_error',error=actual['error'])
    else:
        assert len(expected)==len(actual['rows'])
        diffs=[{'index':i,'expected':e,'actual':r['value']} for i,(e,r) in enumerate(zip(expected,actual['rows'])) if not equal(e,r['value'])]
        record.update(status='mismatch' if diffs else 'pass_selected_configuration',differences=len(diffs),first_difference=diffs[:1])
    key=record['case']+':'+row['input_type'];inputs[key]=request.pop('samples')
    fixtures.append({'source':'ttr','name':row['name'],'case':record['case'],'dataset':key,'request':request,'expected':expected})
    records.append(record)
bridge.stdin.close();bridge.wait(timeout=30)
(ROOT/'docs/evidence/ttr-parity.json').write_text(json.dumps({'schema_version':1,'oracle':oracle['oracle'],'tolerance':{'absolute':2e-9,'relative':2e-9},'records':records},indent=2)+'\n',encoding='utf8',newline='\n')
(ROOT/'crates/roze-ta/tests/fixtures/ttr-parity.json').write_text(json.dumps({'datasets':inputs,'fixtures':fixtures},separators=(',',':'),allow_nan=False)+'\n',encoding='utf8',newline='\n')
print({s:sum(r['status']==s for r in records) for s in sorted({r['status'] for r in records})})
