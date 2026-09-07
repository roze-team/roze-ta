"""Record completed, strict source-variant checks without erasing the baseline."""
import collections, hashlib, json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return json.loads((ROOT/p).read_text(encoding='utf8'))
def write(p,data):(ROOT/p).write_text(json.dumps(data,indent=2,ensure_ascii=False)+'\n',encoding='utf8',newline='\n')
baseline=ROOT/'docs/evidence/cross-library-parity-baseline.json'
if not baseline.exists():
    raw=read('docs/evidence/cross-library-parity-audit.json')
    assert raw['counts'].get('numeric_mismatch')==106 and raw['counts'].get('reference_exception')==8
    write('docs/evidence/cross-library-parity-baseline.json',raw)
records=read('target/compat-progress.json')
assert len(records)==342 and all(r['differences']==0 for r in records)
inventory=read('crates/roze-ta/src/reference_all/compat/inventory.json')
causal={(f['source'],f['name']):f['alignment'] for f in read('crates/roze-ta/tests/fixtures/causal-parity.json')['fixtures']}
bindings=read('docs/reference-parity-bindings.json')
bindings['bindings']=[b for b in bindings['bindings'] if not b['operation_id'].startswith('compat.')]
entries=[]
for e in inventory:
    key=e['source_library'],e['name'];cases=[r for r in records if (r['source'],r['name'])==key]
    assert len(cases)==3
    alignment=causal.get(key,'same_observation')
    status='verified_causal_cases' if key in causal else 'verified_source_cases'
    entries.append({'source':key[0],'name':key[1],'operation_id':e['id'],'parameters':e['example_params'],'status':status,'alignment':alignment,'evidence':'docs/evidence/source-compat-parity.json','scope':'explicit constructor parameters; every row on mixed, flat and trend; source exceptions normalize to transactional undefined_result; causal alignment is explicit; not whole source API or all parameter combinations','cases':cases})
    bindings['bindings'].append({'source':key[0],'name':key[1],'operation_id':e['id'],'parameters':e['example_params'],'reason':'Independent source-specific numerical acceptance; alignment: '+alignment})
paths=['crates/roze-ta/tests/fixtures/python-parity.json','crates/roze-ta/tests/fixtures/ttr-parity.json','crates/roze-ta/tests/fixtures/causal-parity.json']
write('docs/evidence/source-compat-parity.json',{'schema_version':1,'requirements':['FR-IND-009','AC-IND-009'],'counts':{'variants':114,'numeric_cases':334,'normalized_reference_exceptions':8,'failed_cases':0},'fixture_sha256':{p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in paths},'entries':entries})
write('docs/reference-parity-bindings.json',bindings)
print('Recorded 114 variants / 342 passing cases; original 106+8 baseline retained.')
