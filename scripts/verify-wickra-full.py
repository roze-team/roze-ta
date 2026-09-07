"""Verify immutable source blobs and replay the separately recorded migration patch.

--record explicitly records reviewed maintained files; it never updates original
blob identities. Normal verification is read-only except for its own temp replay.
"""
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
TARGET = ROOT / "crates/roze-ta/src/wickra_all"
SOURCE = ROOT / "docs/evidence/wickra-full-source.json"
DERIVED = ROOT / "docs/evidence/wickra-full-derived.json"
PATCH = ROOT / "docs/patches/wickra-full.patch"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def rooted(root, relative):
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f"path leaves expected tree: {relative}")
    return path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--record", action="store_true")
    args = parser.parse_args()
    source = json.loads(SOURCE.read_text(encoding="utf-8"))
    records = source["files"]
    if len(records) != 527 or source["expected_files"] != 527 or len({e["path"] for e in records}) != 527:
        raise ValueError("incomplete or duplicate source inventory")
    for entry in records:
        raw = rooted(BASE, entry["path"]).read_bytes()
        actual = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
        if actual != entry["git_blob_sha"]:
            raise ValueError(f"original blob mismatch: {entry['path']}")
    prefix = "crates/wickra-core/src/"
    pairs = []
    for entry in records:
        if entry["path"].startswith(prefix):
            relative = entry["path"][len(prefix):]
            target = TARGET / ("mod.rs" if relative == "lib.rs" else relative)
            pairs.append((BASE / entry["path"], target))
    if len(pairs) != 524 or {p.resolve() for _, p in pairs} != {p.resolve() for p in TARGET.rglob("*.rs")}:
        raise ValueError("derived modules do not exactly cover the source module tree")
    if (TARGET / "LICENSE-MIT").read_bytes() != (BASE / "LICENSE-MIT").read_bytes():
        raise ValueError("maintained MIT license differs from original")
    if args.record:
        patch = ""
        files = []
        for original, target in pairs:
            relative = target.relative_to(ROOT).as_posix()
            patch += "".join(difflib.unified_diff(
                original.read_bytes().decode("utf-8").splitlines(keepends=True),
                target.read_bytes().decode("utf-8").splitlines(keepends=True),
                fromfile="a/"+relative, tofile="b/"+relative,
            ))
            files.append({"original":original.relative_to(ROOT).as_posix(), "derived":relative, "sha256":sha(target.read_bytes())})
        PATCH.write_text(patch, encoding="utf-8", newline="\n")
        DERIVED.write_text(json.dumps({"source_commit":source["commit"],"patch":PATCH.relative_to(ROOT).as_posix(),"patch_sha256":sha(PATCH.read_bytes()),"files":files},indent=2)+"\n",encoding="utf-8",newline="\n")
    derived = json.loads(DERIVED.read_text(encoding="utf-8"))
    if derived["source_commit"] != source["commit"] or derived["patch_sha256"] != sha(PATCH.read_bytes()):
        raise ValueError("source commit or patch digest mismatch")
    expected = {(o.relative_to(ROOT).as_posix(),t.relative_to(ROOT).as_posix()) for o,t in pairs}
    actual = {(e["original"],e["derived"]) for e in derived["files"]}
    if actual != expected or len(derived["files"]) != len(expected):
        raise ValueError("derived manifest coverage differs")
    for entry in derived["files"]:
        if sha(rooted(ROOT,entry["derived"]).read_bytes()) != entry["sha256"]:
            raise ValueError(f"maintained source changed without recorded patch: {entry['derived']}")
    # Use ordinary inherited directory permissions on Windows. Python's secure
    # temporary-directory ACL can exclude the sandbox token used by child git.
    replay_parent = (ROOT / "target" / "provenance-replay").resolve()
    if not replay_parent.is_relative_to(ROOT.resolve()):
        raise ValueError("replay parent leaves workspace")
    replay_parent.mkdir(parents=True, exist_ok=True)
    replay = (replay_parent / ("roze-ta-wickra-full-" + uuid.uuid4().hex)).resolve()
    replay.mkdir()
    try:
        if replay.parent != replay_parent or not replay.name.startswith("roze-ta-wickra-full-"):
            raise ValueError("unexpected replay cleanup target")
        for entry in derived["files"]:
            destination = rooted(replay,entry["derived"])
            destination.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(rooted(ROOT,entry["original"]),destination)
        result = subprocess.run(["rtk","proxy","git","-c","core.autocrlf=false","apply","--no-index","--whitespace=error-all","--",str(PATCH)],cwd=replay,env={**os.environ,"GIT_CEILING_DIRECTORIES":str(replay_parent)},capture_output=True,text=True,encoding="utf-8")
        if result.returncode:
            raise ValueError(result.stderr)
        for entry in derived["files"]:
            if sha(rooted(replay,entry["derived"]).read_bytes()) != entry["sha256"]:
                raise ValueError(f"replayed source mismatch: {entry['derived']}")
    finally:
        if replay.parent != replay_parent or not replay.name.startswith("roze-ta-wickra-full-"):
            raise ValueError("unexpected replay cleanup target")
        shutil.rmtree(replay)
    print("Verified 527 original blobs, 524 maintained Rust files, MIT license and exact patch replay")


if __name__ == "__main__":
    main()
