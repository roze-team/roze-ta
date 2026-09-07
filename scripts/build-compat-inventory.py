"""Generate source-specific variant specification cards for the 114 audit gaps."""
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return json.loads((ROOT/p).read_text(encoding='utf8'))
catalog={e['id']:e for e in read('docs/evidence/reference-catalog.json')['entries']}
mapping={(r['source'],r['name']):r for r in read('docs/reference-indicator-mapping.json')['entries']}
result=[]
baseline='docs/evidence/cross-library-parity-baseline.json' if (ROOT/'docs/evidence/cross-library-parity-baseline.json').exists() else 'docs/evidence/cross-library-parity-audit.json'
for row in read(baseline)['entries']:
    if row['status'] not in ('numeric_mismatch','reference_exception'):continue
    original={'operation_id':row['operation_id'],'parameters':row['parameters']}
    entry=dict(catalog[original['operation_id']])
    entry.update(id='compat.'+row['source']+'.'+row['name'],name=row['name'],source_library=row['source'],base_operation={'id':original['operation_id'],'params':original['parameters']},example_params=original['parameters'],formula_variant='source-parity-20260907-v1',source='crates/roze-ta/src/reference_all/compat',documentation='Source-specific formula card: docs/contracts/source-parity-v1.md; explicit configuration and source warmup/undefined conventions.',output_type='source_named_values',complexity='bounded prefix replay; at most 4096 observations')
    if row['name']=='FibonacciRetracement':entry['input_type']='f64'
    result.append(entry)
assert len(result)==114
path=ROOT/'crates/roze-ta/src/reference_all/compat/inventory.json';path.parent.mkdir(parents=True,exist_ok=True)
path.write_text(json.dumps(result,indent=2)+'\n',encoding='utf8',newline='\n')
