"""Freeze independently computed observations for explicitly causal contracts."""
import json, math, subprocess, sys
from pathlib import Path
from parity_sources import verify_source
ROOT=Path(__file__).resolve().parents[1]
verify_source('ta');verify_source('ttr')
sys.path.insert(0,str(ROOT/'target/cross-library/ta'))
import pandas as pd
from ta.trend import KSTIndicator
data=json.loads((ROOT/'crates/roze-ta/tests/fixtures/python-parity.json').read_text())
records=[]
def clean(x): return float(x) if math.isfinite(x) else None
for f in data['fixtures']:
    if (f['source'],f['name'])!=('ta','KSTIndicator'):continue
    bars=data['datasets'][f['dataset']]
    close=pd.Series([s['value'] for s in bars])
    p=f['request']['operation']['params']
    kwargs={**{f'roc{i}':p[f'roc{i}'] for i in range(1,5)},**{f'window{i}':p[f'sma{i}'] for i in range(1,5)},'nsig':p['signal']}
    f['expected']=[]
    for size in range(1,len(close)+1):
        obj=KSTIndicator(close=close.iloc[:size],**kwargs)
        f['expected'].append({'kst':clean(obj.kst().iloc[-1]),'signal':clean(obj.kst_sig().iloc[-1])})
    f['alignment']='prefix_snapshot_last; early ROC fill mean uses only observed prefix'
    records.append(f)
subprocess.run(['rtk','proxy',str(ROOT/'target/R-runtime/app/bin/x64/Rscript.exe'),'scripts/generate-causal-parity.R'],cwd=ROOT,check=True)
records+=json.loads((ROOT/'target/causal-ttr.json').read_text())
(ROOT/'crates/roze-ta/tests/fixtures/causal-parity.json').write_text(json.dumps({'schema_version':1,'fixtures':records},indent=2)+'\n',encoding='utf8',newline='\n')
print(f'Generated {len(records)} independent causal fixtures')
