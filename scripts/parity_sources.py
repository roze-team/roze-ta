"""Shared integrity guard run before executing an external reference implementation."""
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def verify_source(source_id):
    sources=json.loads((ROOT/'docs/evidence/cross-library-sources.json').read_text(encoding='utf8'))['sources']
    source=next(s for s in sources if s['id']==source_id)
    base=ROOT/'target/cross-library'/source_id
    for record in source['files']:
        raw=(base/record['path']).read_bytes()
        actual=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
        if actual!=record['git_blob_sha']:raise RuntimeError('Reference source changed: '+source_id+'/'+record['path'])
    return source['commit']
