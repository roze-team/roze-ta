"""Maintain TA-Lib's official pure Rust port with portable safe FMA dispatch."""
import hashlib
import json
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "target/cross-library/ta-lib"
BASE = ROOT / "vendor/ta-lib-rust"
TARGET = ROOT / "crates/roze-ta/src/talib"
prefix = "ta_codegen/output/rust/library/"
source_manifest = json.loads((ROOT/"docs/evidence/cross-library-sources.json").read_text(encoding="utf-8"))
record = next(s for s in source_manifest["sources"] if s["id"] == "ta-lib")
files = {e["path"]:e["git_blob_sha"] for e in record["files"] if e["path"].startswith(prefix) or e["path"] in ["LICENSE","ta_func_api.xml"]}
derived = []
for relative, expected in files.items():
    raw = (SOURCE/relative).read_bytes()
    actual = hashlib.sha1(b"blob "+str(len(raw)).encode()+b"\0"+raw).hexdigest()
    if actual != expected:
        raise ValueError(f"reference source differs: {relative}")
    original = BASE/relative
    original.parent.mkdir(parents=True,exist_ok=True)
    if original.exists() and original.read_bytes() != raw:
        raise ValueError("refusing to rebaseline original")
    original.write_bytes(raw)
    if relative.startswith(prefix+"src/"):
        subpath = relative[len(prefix+"src/"):]
        target = TARGET/("mod.rs" if subpath == "lib.rs" else subpath)
        text = raw.decode("utf-8")
        if target.suffix == ".rs":
            text = text.replace("crate::", "crate::talib::").replace("ta_lib::", "roze_ta::talib::")
            text = re.sub(r"ta_lib_dispatch::dispatch_fma!\(self, \w+, (\w+), \(([^;]*)\)\)",r"self.\1(\2)",text)
            text = text.replace('include_str!("../README.md")','include_str!("README.md")')
            start = text.find('//! The crate is `#![forbid(unsafe_code)]`')
            end = text.find('//! # Live data', start)
            if start >= 0 and end > start:
                text = text[:start] + '//! This maintained module forbids unsafe code. Batch functions use the portable\n//! implementation directly, preserving correctly rounded mul_add semantics.\n//! Runtime FMA dispatch and the ta-lib-dispatch dependency are not used.\n//! The native streaming tier remains available separately from the bounded\n//! Reference adapter, which replays the observed prefix on each update.\n//!\n' + text[end:]
            text = "// Maintained TA-Lib derivative; BSD-3-Clause. See docs/patches/talib-rust-migration.md.\n"+text
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_text(text,encoding="utf-8",newline="\n")
        derived.append({"original":original.relative_to(ROOT).as_posix(),"derived":target.relative_to(ROOT).as_posix()})
readme = (BASE/prefix/"README.md").read_text(encoding="utf-8").replace("ta_lib::","roze_ta::talib::")
(TARGET/"README.md").write_text(readme,encoding="utf-8",newline="\n")
shutil.copyfile(BASE/"LICENSE",TARGET/"LICENSE-BSD-3-Clause")
(ROOT/"docs/evidence/talib-rust-source.json").write_text(json.dumps({"commit":record["commit"],"files":[{"path":k,"git_blob_sha":v} for k,v in files.items()],"derived":derived},indent=2)+"\n",encoding="utf-8",newline="\n")
print(f"Preserved {len(files)} original files; migrated {len(derived)} Rust source/data modules")
