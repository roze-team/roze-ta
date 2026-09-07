"""Create a complete 368-row audit, distinguishing tested equality from acceptance."""
import collections,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(p):return json.loads((ROOT/p).read_text(encoding='utf8'))
groups=collections.defaultdict(list)
for filename in ['talib-c-parity.json','python-parity.json','ttr-parity.json']:
    for record in read('docs/evidence/'+filename)['records']:
        source=record.get('source','ta-lib')
        groups[(source,record['name'])].append({**record,'evidence':'docs/evidence/'+filename})
rows=[]
resolved_path=ROOT/'docs/evidence/source-compat-parity.json'
resolved={(r['source'],r['name']):r for r in read('docs/evidence/source-compat-parity.json')['entries']} if resolved_path.exists() else {}
for mapping in read('docs/reference-indicator-mapping.json')['entries']:
    if mapping['source']=='wickra' or mapping['status']=='non_indicator':continue
    key=(mapping['source'],mapping['name']);cases=groups[key]
    if key in resolved:
        row=resolved[key]
        assert len(row['cases'])==3 and all(c['differences']==0 for c in row['cases'])
        rows.append(row)
        continue
    assert len(cases)==3 and {c['case'] for c in cases}=={'mixed','flat','trend'},key
    passed=all(c['status'].startswith('pass') for c in cases)
    errors=any('error' in c['status'] for c in cases)
    status='verified_default_cases' if passed and key[0]=='ta-lib' else 'verified_selected_cases' if passed else 'reference_exception' if errors else 'numeric_mismatch'
    rows.append({'source':key[0],'name':key[1],'operation_id':mapping['operation_id'],'parameters':mapping['parameters'],'status':status,'evidence':cases[0]['evidence'],'scope':'explicit parameters, mixed/flat/trend, all emitted rows; not all parameter families or additional getters','cases':[{'case':c['case'],'status':c['status'],'scope_note':c.get('scope_note',''),'first_difference':c.get('first_difference',[]),'error':c.get('error')} for c in cases]})
assert len(rows)==368
counts=dict(collections.Counter(r['status'] for r in rows))
result={'schema_version':2,'requirements':['FR-IND-009','AC-IND-009'],'scope':'All 368 mappings have scoped acceptance. Source exceptions are explicitly normalized; three causal variants specify different observation alignment. Selected-case acceptance is not complete source API or all-parameter compatibility. Original discrepancies remain in cross-library-parity-baseline.json.','counts':counts,'entries':rows}
(ROOT/'docs/evidence/cross-library-parity-audit.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf8',newline='\n')
print(counts)
