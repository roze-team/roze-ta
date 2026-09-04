use roze_ta::{
    analysis::{self, Request},
    error::{ErrorCode, TaError},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
#[test]
fn every_paper_task_is_callable_discoverable_and_has_time_validation() {
    let examples: Vec<Value> =
        serde_json::from_str(include_str!("../../../docs/usage/paper-requests-v1.json")).unwrap();
    let mut seen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for example in examples {
        let op = &example["operations"][0];
        let method = op["method"].as_str().unwrap();
        let task = if method == "research" {
            &op["task"]
        } else if method == "regression" {
            &op["spec"]["method"]
        } else {
            &op["spec"]["task"]
        };
        seen.entry(method.into())
            .or_default()
            .insert(task["kind"].as_str().unwrap().into());
        let r: Request = serde_json::from_value(example.clone()).unwrap();
        let result = analysis::calculate(&r).unwrap();
        let second = analysis::calculate(&r).unwrap();
        assert_eq!(result.output_hash, second.output_hash);
        // Expensive work must honor cancellation propagated by the shared boundary.
        let error = analysis::calculate_controlled(&r, || {
            Err(TaError::new(ErrorCode::LimitExceeded, "cancelled"))
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::LimitExceeded);
        if !matches!(method, "research" | "regression") {
            let mut unavailable = example;
            unavailable["operations"][0]["spec"]["available_at_ms"] = Value::from(101);
            let r: Request = serde_json::from_value(unavailable).unwrap();
            assert_eq!(
                analysis::calculate(&r).unwrap_err().code,
                ErrorCode::InvalidTime
            );
        }
    }
    let catalog = analysis::catalog();
    for capability in catalog["capabilities"].as_array().unwrap() {
        if let Some(tasks) = capability.get("tasks") {
            let method = capability["methods"][0].as_str().unwrap();
            let advertised = tasks
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect();
            assert_eq!(seen.remove(method).unwrap(), advertised);
        }
    }
    assert!(seen.is_empty());
}
