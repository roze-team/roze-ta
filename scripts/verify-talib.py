"""Check immutable original Git blobs and the reviewed derivative patch hashes."""
import hashlib,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
manifest=json.loads((ROOT/'docs/evidence/talib-rust-source.json').read_text(encoding='utf8'))
expected=set()
for row in manifest['files']:
    path=ROOT/'vendor/ta-lib-rust'/row['path'];raw=path.read_bytes();expected.add(path.resolve())
    assert hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==row['git_blob_sha'],path
assert expected=={p.resolve() for p in (ROOT/'vendor/ta-lib-rust').rglob('*') if p.is_file()}
hashes={p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT/'crates/roze-ta/src/talib').rglob('*')) if p.is_file()}
patch=ROOT/'docs/patches/talib-rust-derived.json'
if '--record-initial' in sys.argv:
    if patch.exists():raise RuntimeError('refusing to overwrite reviewed derivative manifest')
    patch.write_text(json.dumps({'schema_version':1,'original_commit':manifest['commit'],'patch_description':'docs/patches/talib-rust-migration.md','sha256':hashes},indent=2)+'\n',encoding='utf8',newline='\n')
assert json.loads(patch.read_text(encoding='utf8'))['sha256']==hashes,'Derived file differs; review and record a separate patch, never rebaseline original'
print('TA-Lib: verified',len(expected),'original files and',len(hashes),'reviewed derivative files')
