"""Reproduce the maintained Wickra source tree from its immutable pinned inputs.

This is an explicit developer command, never a build-time download or runtime I/O.
Original blob identities are checked before any derived file is written.
"""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "vendor/wickra-full"
TARGET = ROOT / "crates/roze-ta/src/wickra_all"
MANIFEST = ROOT / "docs/evidence/wickra-full-source.json"
HEADER = """// Copyright (c) 2026 kingchenc and Wickra contributors.
// SPDX-License-Identifier: MIT
// Derived from the pinned Wickra baseline; see docs/patches/wickra-full-migration.md.
// Local changes: module paths, output serialization and dependency/test integration.

"""


def closing_brace(text, start):
    """Find a Rust block end, skipping quoted strings and line/block comments."""
    depth = 0
    i = start
    while i < len(text):
        if text.startswith("//", i):
            n = text.find("\n", i)
            i = len(text) if n < 0 else n + 1
            continue
        if text.startswith("/*", i):
            n = text.find("*/", i + 2)
            if n < 0:
                raise ValueError("unterminated comment")
            i = n + 2
            continue
        if text[i] == '"':
            i += 1
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            continue
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise ValueError("unterminated block")


def transform(text, rel):
    text = text.replace("crate::", "crate::wickra_all::")
    text = text.replace("wickra_core::", "roze_ta::wickra_all::")
    # Preserve the source optional parallel methods and original property tests.
    text = text.replace('#[cfg(feature = "parallel")]', '#[cfg(feature = "wickra-parallel")]')
    text = text.replace('Requires the `parallel` feature (enabled by\n    /// default), which pulls in `rayon`.', 'Requires the opt-in `wickra-parallel` feature,\n    /// which pulls in `rayon` and is disabled by default.')
    # Equivalent idioms required by the project's pinned Rust 1.98 Clippy.
    text = text.replace("period % 2 != 0", "!period.is_multiple_of(2)")
    text = text.replace("n % 2 == 0", "n.is_multiple_of(2)")
    # Only wire value types, never expose mutable internal state via serde.
    names = set(re.findall(r"type (?:Output|Bar) = ([A-Z]\w*);", text))
    names.update(re.findall(r"pub (?:struct|enum) (\w+(?:Output|Bar|Brick|Column))\b", text))
    names.add("FootprintLevel")
    if rel in {"ohlcv.rs", "microstructure.rs", "derivatives.rs", "cross_section.rs"}:
        names.update(re.findall(r"pub (?:struct|enum) (\w+)\b", text))
    if rel.endswith("macd_ext.rs"):
        names.add("MaType")
    for name in sorted(names):
        text = re.sub(rf"(?m)^pub (struct|enum) {name}\b", rf"#[derive(serde::Serialize, serde::Deserialize)]\n#[serde(deny_unknown_fields)]\npub \1 {name}", text)
    if rel == "lib.rs":
        text = text.replace("#![cfg_attr(docsrs, feature(doc_cfg))]", "")
        text = "//! Maintained Wickra algorithms. Original initialization and neutral-value\n//! conventions apply; this module is distinct from the strict R1 profiles.\n" + text
    return HEADER + text


def main():
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if len(manifest["files"]) != manifest["expected_files"]:
        raise SystemExit("Incomplete source acquisition; refusing partial migration")
    derived = []
    prefix = "crates/wickra-core/src/"
    for entry in manifest["files"]:
        original = BASE / entry["path"]
        data = original.read_bytes()
        blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        if blob != entry["git_blob_sha"]:
            raise SystemExit(f"Original blob mismatch: {entry['path']}")
        if not entry["path"].startswith(prefix):
            continue
        rel = entry["path"][len(prefix):]
        dest = TARGET / ("mod.rs" if rel == "lib.rs" else rel)
        dest.parent.mkdir(parents=True, exist_ok=True)
        result = transform(data.decode("utf-8"), rel)
        dest.write_text(result, encoding="utf-8", newline="\n")
        derived.append({"original": original.relative_to(ROOT).as_posix(), "derived": dest.relative_to(ROOT).as_posix()})
    (TARGET / "LICENSE-MIT").write_bytes((BASE / "LICENSE-MIT").read_bytes())
    (ROOT / "docs/evidence/wickra-full-derived-paths.json").write_text(json.dumps(derived, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"Verified {len(manifest['files'])} original blobs and migrated {len(derived)} Rust files")


if __name__ == "__main__":
    main()
