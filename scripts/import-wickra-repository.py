"""Import an authenticated pinned archive without executing upstream scripts."""
import hashlib
import json
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
tree = json.loads((ROOT / "docs/evidence/wickra-repository-tree.json").read_text(encoding="utf-8"))
expected = {e["path"]:e for e in tree["tree"] if e["type"] == "blob"}
base = ROOT / "vendor/wickra-full"
prefix = "wickra-" + tree["sha"] + "/"
files = {}
with tarfile.open(ROOT / "target/wickra-7ed1504.tar.gz", "r:gz") as archive:
    for member in archive:
        if member.isdir():
            continue
        if not member.isfile() or not member.name.startswith(prefix):
            raise ValueError(f"unsupported archive member {member.name}")
        name = member.name[len(prefix):]
        if name not in expected or name in files:
            raise ValueError(f"unlisted or duplicate archive file {name}")
        if member.size != expected[name]["size"]:
            raise ValueError(f"archive size differs from pinned tree: {name}")
        path = (base / name).resolve()
        if not path.is_relative_to(base.resolve()):
            raise ValueError("archive path escapes vendor tree")
        data = archive.extractfile(member).read()
        actual = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        if actual != expected[name]["sha"]:
            raise ValueError(f"Git blob mismatch {name}")
        if path.exists() and path.read_bytes() != data:
            raise ValueError(f"refusing to overwrite different existing baseline {name}")
        files[name] = (path, data)
if set(files) != set(expected):
    raise ValueError(f"archive does not cover tree: {set(expected)-set(files)}")
for path, data in files.values():
    if not path.exists():
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(data)
print(f"Verified and imported {len(files)} exact original Git blobs at {tree['sha']}")
