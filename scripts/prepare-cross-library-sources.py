"""Extract pinned reference sources for independent numerical verification only."""
import hashlib
import json
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
manifest_path = ROOT / "docs/evidence/cross-library-sources.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
for source in manifest["sources"]:
    archive_path = ROOT / f"target/parity-{source['id']}.tar.gz"
    source["archive_sha256"] = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    base = (ROOT / "target/cross-library" / source["id"]).resolve()
    prefix = source["repository"].split("/")[1] + "-" + source["commit"] + "/"
    files = []
    with tarfile.open(archive_path,"r:gz") as archive:
        for member in archive:
            if member.isdir():
                continue
            if not member.isfile() or not member.name.startswith(prefix):
                raise ValueError(f"unsupported reference archive member {member.name}")
            relative = member.name[len(prefix):]
            path = (base / relative).resolve()
            if not path.is_relative_to(base) or member.size > 32*1024*1024:
                raise ValueError("unsafe reference archive path or size")
            raw = archive.extractfile(member).read()
            path.parent.mkdir(parents=True,exist_ok=True)
            if path.exists() and path.read_bytes() != raw:
                raise ValueError(f"reference source changed: {path}")
            path.write_bytes(raw)
            blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
            files.append({"path":relative,"git_blob_sha":blob})
    source["files"] = files
    print(source["id"],len(files),source["commit"])
manifest_path.write_text(json.dumps(manifest,indent=2)+"\n",encoding="utf-8",newline="\n")
