"""Verify complete repository blobs and replay the separately reviewed supplements."""
import argparse
import difflib
import hashlib
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "vendor/wickra-full"
MANIFEST = ROOT / "docs/evidence/wickra-repository-derived.json"
PATCH = ROOT / "docs/patches/wickra-repository.patch"
PAIRS = [
    ("crates/wickra-data/src/aggregator.rs", "crates/roze-ta/src/wickra_data/aggregator.rs"),
    ("crates/wickra-data/src/resample.rs", "crates/roze-ta/src/wickra_data/resample.rs"),
    ("crates/wickra-core/tests/invariants.rs", "crates/roze-ta/tests/wickra_invariants.rs"),
]
def digest(data):
    return hashlib.sha256(data).hexdigest()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--record",action="store_true")
    args = parser.parse_args()
    tree = json.loads((ROOT / "docs/evidence/wickra-repository-tree.json").read_text(encoding="utf-8"))
    if tree["truncated"] or tree["sha"] != "7ed1504c805cb45fff7a317a659eb3ad559a80a7":
        raise ValueError("incomplete or unexpected source tree")
    expected = {e["path"]:e for e in tree["tree"] if e["type"] == "blob"}
    actual = {p.relative_to(BASE).as_posix() for p in BASE.rglob("*") if p.is_file()}
    if len(expected) != 2639 or actual != set(expected):
        raise ValueError("repository file coverage differs from pinned tree")
    for name, entry in expected.items():
        path = (BASE / name).resolve()
        if not path.is_relative_to(BASE.resolve()):
            raise ValueError("source path escapes repository")
        data = path.read_bytes()
        blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        if blob != entry["sha"] or len(data) != entry["size"]:
            raise ValueError(f"original blob mismatch: {name}")
    if args.record:
        patch = ""
        files = []
        for source, target in PAIRS:
            patch += "".join(difflib.unified_diff(
                (BASE/source).read_bytes().decode("utf-8").splitlines(keepends=True),
                (ROOT/target).read_bytes().decode("utf-8").splitlines(keepends=True),
                fromfile="a/"+target,tofile="b/"+target))
            files.append({"original":source,"derived":target,"sha256":digest((ROOT/target).read_bytes())})
        PATCH.write_text(patch,encoding="utf-8",newline="\n")
        MANIFEST.write_text(json.dumps({"commit":tree["sha"],"patch_sha256":digest(PATCH.read_bytes()),"files":files},indent=2)+"\n",encoding="utf-8",newline="\n")
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if manifest["commit"] != tree["sha"] or manifest["patch_sha256"] != digest(PATCH.read_bytes()):
        raise ValueError("supplement source or patch identity changed")
    if [(e["original"],e["derived"]) for e in manifest["files"]] != PAIRS:
        raise ValueError("supplement mapping differs")
    parent = (ROOT/"target/provenance-replay").resolve()
    if not parent.is_relative_to(ROOT.resolve()):
        raise ValueError("replay parent escapes workspace")
    parent.mkdir(parents=True,exist_ok=True)
    replay = (parent/("wickra-repository-"+uuid.uuid4().hex)).resolve()
    replay.mkdir()
    try:
        for entry in manifest["files"]:
            if digest((ROOT/entry["derived"]).read_bytes()) != entry["sha256"]:
                raise ValueError(f"unrecorded maintained change: {entry['derived']}")
            path = replay/entry["derived"]
            path.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(BASE/entry["original"],path)
        subprocess.run(["rtk","proxy","git","-c","core.autocrlf=false","apply","--no-index","--whitespace=error-all","--",str(PATCH)],cwd=replay,env={**os.environ,"GIT_CEILING_DIRECTORIES":str(parent)},check=True)
        for entry in manifest["files"]:
            if digest((replay/entry["derived"]).read_bytes()) != entry["sha256"]:
                raise ValueError("supplement replay differs")
    finally:
        if replay.parent != parent or not replay.name.startswith("wickra-repository-"):
            raise ValueError("unexpected cleanup target")
        shutil.rmtree(replay)
    print("Verified complete Wickra repository: 2639 original blobs, licenses, 3 supplemental source patches")

if __name__ == "__main__":
    main()
