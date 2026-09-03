"""Verify the unmodified vendored upstream archive against its recorded hashes."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
manifest = json.loads((root / "UPSTREAM.json").read_text(encoding="utf-8"))
vendor = root / "vendor" / "yata"
expected = manifest["files"]
actual = {
    p.relative_to(vendor).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
    for p in vendor.rglob("*") if p.is_file()
}
differences = sorted(k for k in expected.keys() | actual.keys() if expected.get(k) != actual.get(k))
if differences:
    raise SystemExit("Upstream differs: " + ", ".join(differences))
print(f"Verified {len(actual)} upstream files at {manifest['commit']}")
