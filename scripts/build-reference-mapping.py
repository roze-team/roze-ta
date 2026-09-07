"""Link every enumerated source export to a callable, explicitly scoped variant.

Cross-library equivalence remains distinct from implementation availability.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def read(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))

catalog = {e["id"]: e for e in read("docs/evidence/reference-catalog.json")["entries"]}
audit = read("docs/evidence/reference-coverage-audit.json")
old = {(e["source"], e["name"]): e for e in read("docs/reference-indicator-mapping.json")["entries"]}
rows = []
for item in audit:
    row = {"source": item["source"], "name": item["name"]}
    if item["classification"] == "non_indicator":
        row.update(status="non_indicator", reason=item["reason"])
    else:
        entry = catalog[item["candidate"]]
        migrated = item["source"] == "wickra"
        row.update(
            status="implemented_variant" if migrated else "mapped_variant",
            operation_id=entry["id"], formula_variant=entry["formula_variant"],
            parameters=entry["example_params"], input_type=entry["input_type"],
            output_type=entry["output_type"],
            contract="docs/contracts/reference-all-v1.md" if entry["id"].startswith("wickra.") else "docs/contracts/reference-extras.md",
            evidence="docs/evidence/2026-09-07-reference-full.md",
            compatibility="pinned_source_algorithm" if migrated else "cross_library_numeric_parity_not_verified",
            notes="Constructor parameters are explicit. Source seeds, units, output layout and neutral values are preserved; the wire contract supplies bounds and observation times." if migrated else "A callable local formula variant is available. This mapping is not acceptance of all source defaults, smoothing choices, units, output order or edge cases. Compare the local contract before substitution.",
        )
        previous = old.get((item["source"],item["name"]), {})
        if previous.get("profile"):
            row["profile"] = previous["profile"]
        if item["name"].startswith("CDL"):
            row["notes"] += " Local candle patterns use signed unit signals and Wickra shape thresholds; TA-Lib candle settings and +/-100 magnitudes are not reproduced."
        if item["name"] in ("MA","MAVP","volatility","runMAD","runVar","runSD","runCov","stoch","SFX","PivotsHL"):
            row["notes"] += " The selected local variant implements only the parameters/formula specified in its contract, not the source function's entire option family."
    rows.append(row)
result = {"schema_version":2,"scope":"Algorithm availability is separate from cross-library numeric parity; mapped_variant must not be counted as fully audited compatibility.","entries":rows}
(ROOT / "docs/reference-indicator-mapping.json").write_text(json.dumps(result,indent=2,ensure_ascii=False)+"\n",encoding="utf-8",newline="\n")
print({s:sum(r["status"]==s for r in rows) for s in {r["status"] for r in rows}})
