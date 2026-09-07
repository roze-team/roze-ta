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
parity = read("docs/evidence/talib-c-parity.json") if (ROOT/"docs/evidence/talib-c-parity.json").exists() else {"records":[]}
talib_cases = {}
bindings={(b["source"],b["name"]):b for b in read("docs/reference-parity-bindings.json")["bindings"]}
audit_path=ROOT/"docs/evidence/cross-library-parity-audit.json"
case_audit={(e["source"],e["name"]):e for e in read("docs/evidence/cross-library-parity-audit.json")["entries"]} if audit_path.exists() else {}
for record in parity["records"]:
    talib_cases.setdefault(record["name"], []).append(record)
rows = []
for item in audit:
    row = {"source": item["source"], "name": item["name"]}
    if item["classification"] == "non_indicator":
        row.update(status="non_indicator", reason=item["reason"])
    else:
        binding=bindings.get((item["source"],item["name"]))
        entry = catalog[binding["operation_id"] if binding else item["candidate"]]
        migrated = item["source"] == "wickra"
        row.update(
            status="implemented_variant" if migrated else "mapped_variant",
            operation_id=entry["id"], formula_variant=entry["formula_variant"],
            parameters=binding["parameters"] if binding else entry["example_params"], input_type=entry["input_type"],
            output_type=entry["output_type"],
            contract="docs/contracts/reference-all-v1.md" if entry["id"].startswith("wickra.") else "docs/contracts/reference-extras.md",
            evidence="docs/evidence/2026-09-07-reference-full.md",
            compatibility="pinned_source_algorithm" if migrated else "cross_library_numeric_parity_not_verified",
            notes="Constructor parameters are explicit. Source seeds, units, output layout and neutral values are preserved; the wire contract supplies bounds and observation times." if migrated else "A callable local formula variant is available. This mapping is not acceptance of all source defaults, smoothing choices, units, output order or edge cases. Compare the local contract before substitution.",
        )
        previous = old.get((item["source"],item["name"]), {})
        if binding:
            row["notes"] += " Audited binding correction: " + binding["reason"]
            if entry["id"].startswith("talib."):
                row["contract"]="docs/contracts/talib-reference-v1.md"
        if previous.get("profile"):
            row["profile"] = previous["profile"]
        if item["source"] == "ta-lib" and "talib."+item["name"] in catalog:
            entry = catalog["talib."+item["name"]]
            cases = talib_cases.get(item["name"], [])
            passed = len(cases)==3 and {c["case"] for c in cases}=={"mixed","flat","trend"} and all(c["status"]=="pass" for c in cases)
            row.update(operation_id=entry["id"], formula_variant=entry["formula_variant"], parameters=entry["example_params"], input_type=entry["input_type"], output_type=entry["output_type"],
                status="verified_default_cases" if passed else "implemented_variant", compatibility="independent_C_default_parameters_three_datasets" if passed else "cross_library_numeric_parity_not_verified",
                contract="docs/contracts/talib-reference-v1.md", evidence="docs/evidence/talib-c-parity.json",
                notes="Official pinned pure Rust TA-Lib variant. Independent C reference checks every output row for 256-sample mixed, flat and trend series at explicit default parameters, absolute/relative tolerance 2e-9. This is not all-parameter or bitwise compatibility acceptance.")
        elif item["name"].startswith("CDL"):
            row["notes"] += " Local candle patterns use signed unit signals and Wickra shape thresholds; TA-Lib candle settings and +/-100 magnitudes are not reproduced."
        if item["source"] != "ta-lib" and item["name"] in ("MA","MAVP","volatility","runMAD","runVar","runSD","runCov","stoch","SFX","PivotsHL"):
            row["notes"] += " The selected local variant implements only the parameters/formula specified in its contract, not the source function's entire option family."
    verified=case_audit.get((item["source"],item["name"]))
    if verified and verified["operation_id"]==row.get("operation_id") and verified["parameters"]==row.get("parameters"):
        row["case_audit"]={"status":verified["status"],"evidence":"docs/evidence/cross-library-parity-audit.json","scope":verified["scope"]}
        if row.get('operation_id','').startswith('compat.'):
            row.update(status=verified['status'],compatibility='independent_source_selected_cases',contract='docs/contracts/source-parity-v1.md',evidence='docs/evidence/source-compat-parity.json',notes=verified['scope'],alignment=verified['alignment'])
    rows.append(row)
result = {"schema_version":2,"scope":"Algorithm availability is separate from cross-library numeric parity; mapped_variant must not be counted as fully audited compatibility.","entries":rows}
(ROOT / "docs/reference-indicator-mapping.json").write_text(json.dumps(result,indent=2,ensure_ascii=False)+"\n",encoding="utf-8",newline="\n")
print({s:sum(r["status"]==s for r in rows) for s in {r["status"] for r in rows}})
