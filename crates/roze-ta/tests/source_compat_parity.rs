//! Source-specific numerical acceptance, including causal alignment and failures.
use roze_ta::{
    error::ErrorCode,
    reference_all::{self, Request, Stream},
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn equivalent(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let a = a.as_f64().unwrap();
            let b = b.as_f64().unwrap();
            (a - b).abs() <= 2e-9 * (1. + a.abs())
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|b| equivalent(v, b)))
        }
        _ => a == b,
    }
}
fn cases() -> Vec<(Value, Request)> {
    let inventory: Value =
        serde_json::from_str(include_str!("../src/reference_all/compat/inventory.json")).unwrap();
    let causal: Value = serde_json::from_str(include_str!("fixtures/causal-parity.json")).unwrap();
    let mut result = Vec::new();
    for raw in [
        include_str!("fixtures/python-parity.json"),
        include_str!("fixtures/ttr-parity.json"),
    ] {
        let data: Value = serde_json::from_str(raw).unwrap();
        for original in data["fixtures"].as_array().unwrap() {
            let Some(entry) = inventory.as_array().unwrap().iter().find(|e| {
                e["source_library"] == original["source"] && e["name"] == original["name"]
            }) else {
                continue;
            };
            let f = causal["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| {
                    f["source"] == original["source"]
                        && f["name"] == original["name"]
                        && f["case"] == original["case"]
                })
                .unwrap_or(original);
            let mut request = f["request"].clone();
            request["operation"]["id"] = entry["id"].clone();
            request["samples"] = data["datasets"][f["dataset"].as_str().unwrap()].clone();
            if f["name"] == "FibonacciRetracement" {
                for s in request["samples"].as_array_mut().unwrap() {
                    s["value"] = s["value"]["close"].clone();
                }
            }
            result.push((f.clone(), serde_json::from_value(request).unwrap()));
        }
    }
    result
}
#[test]
fn all_114_source_variants_match_independent_numeric_cases() {
    let mut names = BTreeSet::new();
    let mut count = 0;
    for (f, request) in cases() {
        let actual = reference_all::calculate(&request)
            .unwrap_or_else(|e| panic!("{} {}: {e}", request.operation.id, f["case"]));
        let expected = f["expected"].as_array().unwrap();
        let rows = actual["rows"].as_array().unwrap();
        assert_eq!(expected.len(), rows.len());
        for (i, (a, b)) in expected.iter().zip(rows).enumerate() {
            assert!(
                equivalent(a, &b["value"]),
                "{} {} row {i}: expected {a}, actual {}",
                request.operation.id,
                f["case"],
                b["value"]
            );
        }
        names.insert(request.operation.id);
        count += 1;
    }
    assert_eq!(names.len(), 114);
    assert_eq!(count, 334);
}
#[test]
fn eight_reference_failures_are_structured_and_transactional() {
    let exceptions = [
        ("talipp", "CCI"),
        ("talipp", "MassIndex"),
        ("talipp", "VTX"),
        ("talipp", "VWMA"),
        ("ttr", "ADX"),
        ("ttr", "EMV"),
        ("ttr", "SMI"),
        ("ttr", "stoch"),
    ];
    let cases = cases();
    for (source, name) in exceptions {
        let (_, r) = cases
            .iter()
            .find(|(f, _)| f["source"] == source && f["name"] == name)
            .unwrap();
        let mut stream = Stream::new(r.identity.clone(), r.operation.clone()).unwrap();
        let mut failure = false;
        for original in &r.samples {
            let mut s = original.clone();
            s.value = json!({"open":100.,"high":100.,"low":100.,"close":100.,"volume":0.,"timestamp":s.at_ms});
            let before = serde_json::to_value(stream.snapshot().unwrap()).unwrap();
            if let Err(error) = stream.update(&s, s.available_at_ms) {
                assert_eq!(error.code, ErrorCode::UndefinedResult, "{source}.{name}");
                assert_eq!(
                    before,
                    serde_json::to_value(stream.snapshot().unwrap()).unwrap()
                );
                failure = true;
                break;
            }
        }
        assert!(
            failure,
            "{source}.{name} failed to report the reference exception"
        );
    }
}
#[test]
fn causal_variants_preserve_prefix_values_and_restore_with_real_timestamps() {
    for name in [
        "compat.ta.KSTIndicator",
        "compat.ttr.DPO",
        "compat.ttr.ZigZag",
        "compat.talipp.PivotsHL",
    ] {
        let (_, mut r) = cases()
            .into_iter()
            .find(|(f, r)| r.operation.id == name && f["case"] == "mixed")
            .unwrap();
        for (i, s) in r.samples.iter_mut().enumerate() {
            s.at_ms = 1_700_000_000_000 + i as i64 * 60000;
            s.available_at_ms = s.at_ms + 5;
            if s.value.is_object() {
                s.value["timestamp"] = json!(s.at_ms);
            }
        }
        r.as_of_ms = r.samples.last().unwrap().available_at_ms;
        let full = reference_all::calculate(&r).unwrap();
        let mut stream = Stream::new(r.identity.clone(), r.operation.clone()).unwrap();
        for (i, s) in r.samples.iter().enumerate() {
            let row = stream.update(s, s.available_at_ms).unwrap();
            assert_eq!(
                serde_json::to_value(row).unwrap(),
                full["rows"][i],
                "{name} row {i}"
            );
            if i == 47 {
                stream = Stream::restore(&stream.snapshot().unwrap(), &r.identity, &r.operation)
                    .unwrap();
            }
        }
    }
}
