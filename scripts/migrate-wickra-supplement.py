"""Reproducible pure-data and whole-catalog invariant integration."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "vendor/wickra-full"
HEADER = "// Copyright (c) 2026 kingchenc and Wickra contributors.\n// SPDX-License-Identifier: MIT\n// Derived from Wickra 7ed1504; see docs/patches/wickra-repository-migration.md.\n\n"
for name in ("aggregator", "resample"):
    source = BASE / f"crates/wickra-data/src/{name}.rs"
    text = source.read_text(encoding="utf-8").replace("crate::", "crate::wickra_data::").replace("wickra_core::", "roze_ta::wickra_all::").replace("wickra_data::", "roze_ta::wickra_data::")
    text = text.replace("crate::roze_ta::wickra_data::", "crate::wickra_data::")
    text = "".join(line if line.lstrip().startswith("//") else line.replace("roze_ta::wickra_all::", "crate::wickra_all::") for line in text.splitlines(keepends=True))
    target = ROOT / f"crates/roze-ta/src/wickra_data/{name}.rs"
    target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(HEADER + text, encoding="utf-8",newline="\n")
source = BASE / "crates/wickra-core/tests/invariants.rs"
text = source.read_text(encoding="utf-8").replace("use wickra_core::*;", "use roze_ta::wickra_all::*;").replace('include_str!("invariants.rs")','include_str!("wickra_invariants.rs")')
(ROOT / "crates/roze-ta/tests/wickra_invariants.rs").write_text(HEADER+text,encoding="utf-8",newline="\n")
print("Migrated 2 pure data modules and the whole-catalog invariant suite")
