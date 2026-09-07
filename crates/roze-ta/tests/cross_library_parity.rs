//! Positive compatibility cases only; unresolved discrepancies remain in the audit.
use roze_ta::reference_all::{self, Request};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn equivalent(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            (a - b).abs() <= 2e-9 * (1.0 + a.abs())
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|x| equivalent(v, x)))
        }
        _ => a == b,
    }
}

#[test]
fn accepted_selected_cases_keep_their_independent_reference_values() {
    let audit: Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/cross-library-parity-audit.json"
    ))
    .unwrap();
    let entries = audit["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 368);
    let accepted = entries
        .iter()
        .filter(|e| e["status"] == "verified_selected_cases")
        .map(|e| {
            (
                (e["source"].as_str().unwrap(), e["name"].as_str().unwrap()),
                e,
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(accepted.len(), 53);
    let mut exercised = BTreeSet::new();
    for raw in [
        include_str!("fixtures/python-parity.json"),
        include_str!("fixtures/ttr-parity.json"),
    ] {
        let data: Value = serde_json::from_str(raw).unwrap();
        for fixture in data["fixtures"].as_array().unwrap() {
            let key = (
                fixture["source"].as_str().unwrap(),
                fixture["name"].as_str().unwrap(),
            );
            let Some(entry) = accepted.get(&key) else {
                continue;
            };
            assert_eq!(entry["operation_id"], fixture["request"]["operation"]["id"]);
            assert_eq!(
                entry["parameters"],
                fixture["request"]["operation"]["params"]
            );
            let mut request = fixture["request"].clone();
            request["samples"] = data["datasets"][fixture["dataset"].as_str().unwrap()].clone();
            let request: Request = serde_json::from_value(request).unwrap();
            let actual = reference_all::calculate(&request).unwrap();
            let rows = actual["rows"].as_array().unwrap();
            let expected = fixture["expected"].as_array().unwrap();
            assert_eq!(expected.len(), rows.len());
            for (i, (want, row)) in expected.iter().zip(rows).enumerate() {
                assert!(
                    equivalent(want, &row["value"]),
                    "{key:?} {} row {i}: expected {want}, actual {}",
                    fixture["case"],
                    row["value"]
                );
            }
            exercised.insert((
                key.0.to_owned(),
                key.1.to_owned(),
                fixture["case"].as_str().unwrap().to_owned(),
            ));
        }
    }
    assert_eq!(exercised.len(), accepted.len() * 3);
}
