use roze_ta::{
    analysis::{self, Request},
    error::ErrorCode,
};
use serde_json::{json, Value};

fn request(kind: &str, units: &str, xs: &[f64], task: Value) -> Request {
    serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"research","instrument":"TEST","timeframe":"1d","source":"manual","data_version":"1"},
        "input_kind":kind,"units":units,"as_of_ms":100,"fit_cutoff_ms":100,
        "points":xs.iter().enumerate().map(|(i,x)|json!({"at_ms":i+1,"available_at_ms":i+1,"x":x,"y":null})).collect::<Vec<_>>(),"events":[],"operations":[{"method":"research","task":task}]})).unwrap()
}
fn output(r: &Request) -> Value {
    serde_json::to_value(analysis::calculate(r).unwrap()).unwrap()["results"][0]["result"].clone()
}
fn v(o: &Value, name: &str) -> f64 {
    o["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == name)
        .unwrap()["value"]["value"]
        .as_f64()
        .unwrap()
}
fn close(x: f64, y: f64) {
    assert!((x - y).abs() < 1e-10, "{x} != {y}");
}
fn sharpe_task() -> Value {
    json!({"kind":"sharpe_inference","aggregation_periods":2,"benchmark_sharpe":0.0,"confidence":0.95,"assume_stationary":true,"assume_iid_for_psr":true,"trials":{"experiment_id":"all-trials","available_at_ms":10,"total_trials":2,"effective_independent_trials":2.0,"sharpe_mean":0.0,"sharpe_stddev":1.0}})
}

#[test]
fn sharpe_zero_mean_and_nonzero_independent_reference() {
    let r = request(
        "excess_simple_return",
        "ratio",
        &[-1., 0., 1.],
        sharpe_task(),
    );
    let o = output(&r);
    close(v(&o, "period_sharpe"), 0.);
    close(v(&o, "psr"), 0.5);
    close(v(&o, "non_excess_kurtosis"), 1.5);
    let r = request(
        "excess_simple_return",
        "ratio",
        &[0., 1., 2.],
        sharpe_task(),
    );
    let o = output(&r);
    close(v(&o, "period_sharpe"), 1.);
    close(v(&o, "lo_aggregated_sharpe"), 2f64.sqrt());
    // Normal CDF(4/3), from independently tabulated standard normal distribution.
    close(v(&o, "psr"), 0.9087887802741321);
    assert!(v(&o, "dsr") < v(&o, "psr"));
}
#[test]
fn bh_step_up_reordered_ties_and_bonferroni() {
    let o = output(&request(
        "p_value",
        "probability",
        &[0.04, 0.001, 0.03, 0.2],
        json!({"kind":"multiple_testing","alpha":0.05}),
    ));
    close(v(&o, "bh_rejections"), 1.);
    let s = o["series"].as_array().unwrap();
    close(s[0]["value"]["value"].as_f64().unwrap(), 0.16 / 3.);
    close(s[2]["value"]["value"].as_f64().unwrap(), 0.004);
    close(s[3]["value"]["value"].as_f64().unwrap(), 0.004);
}
#[test]
fn dm_uses_biased_hac_covariance_and_two_sided_normal_tail() {
    let o = output(&request(
        "loss_difference",
        "squared_price",
        &[1., 2., 3.],
        json!({"kind":"diebold_mariano","hac_lags":0,"assume_stationary":true}),
    ));
    close(v(&o, "long_run_variance"), 2. / 3.);
    close(v(&o, "dm_statistic"), 18f64.sqrt());
    close(v(&o, "two_sided_p_value"), 0.00002209049699858544);
}
fn rows(values: &[[f64; 2]]) -> Value {
    json!(values
        .iter()
        .enumerate()
        .map(|(i, v)| json!({"at_ms":i+1,"available_at_ms":i+1,"values":v}))
        .collect::<Vec<_>>())
}
#[test]
fn cscv_enumerates_complements_and_reversals() {
    let o = output(&request(
        "candidate_performance",
        "ratio",
        &[],
        json!({"kind":"pbo","candidates":["a","b"],"rows":rows(&[[3.,1.],[4.,2.],[1.,3.],[2.,4.]]),"blocks":2,"metric":"mean"}),
    ));
    close(v(&o, "pbo"), 1.);
    close(v(&o, "splits"), 2.);
    close(
        o["series"][0]["value"]["value"].as_f64().unwrap(),
        -2f64.ln(),
    );
}
#[test]
fn joint_bootstrap_is_reproducible_and_constant_spa_is_undefined() {
    let finite = request(
        "benchmark_loss_advantage",
        "loss",
        &[],
        json!({"kind":"reality_check_spa","candidates":["a","b"],"rows":rows(&[[1.,0.],[3.,-2.],[2.,-1.]]),"block_length":3,"replicates":99,"seed":42,"hac_lags":0,"assume_stationary":true}),
    );
    let finite_result = output(&finite);
    close(v(&finite_result, "spa_consistent_p_value"), 0.01);
    close(v(&finite_result, "reality_check_p_value"), 0.01);
    let task = json!({"kind":"reality_check_spa","candidates":["a","b"],"rows":rows(&[[1.,0.],[1.,0.],[1.,0.],[1.,0.]]),"block_length":2,"replicates":99,"seed":42,"hac_lags":1,"assume_stationary":true});
    let r = request("benchmark_loss_advantage", "loss", &[], task);
    let o = output(&r);
    assert_eq!(o, output(&r));
    close(v(&o, "reality_check_statistic"), 2.);
    close(v(&o, "reality_check_p_value"), 0.01);
    assert_eq!(o["values"][2]["value"]["status"], "undefined");
}
#[test]
fn time_cutoff_and_selection_hash_are_stable() {
    let r = request(
        "excess_simple_return",
        "ratio",
        &[0., 1., 2.],
        sharpe_task(),
    );
    let o = output(&r);
    let mut future = r.clone();
    future.points.push(analysis::Point {
        at_ms: 101,
        available_at_ms: 101,
        x: 999.,
        y: None,
    });
    assert_eq!(o, output(&future));
    future.points[1].available_at_ms = 101;
    assert_eq!(
        analysis::calculate(&future).unwrap_err().code,
        ErrorCode::InvalidTime
    );
}
#[test]
fn reject_invalid_trials_budget_and_cancel() {
    let mut task = sharpe_task();
    task["trials"]["effective_independent_trials"] = json!(3.0);
    assert!(analysis::calculate(&request(
        "excess_simple_return",
        "ratio",
        &[0., 1., 2.],
        task
    ))
    .is_err());
    let r = request(
        "excess_simple_return",
        "ratio",
        &[0., 1., 2.],
        sharpe_task(),
    );
    assert!(
        analysis::calculate_controlled(&r, || Err(roze_ta::error::TaError::new(
            ErrorCode::LimitExceeded,
            "cancelled"
        )))
        .is_err()
    );
    let mut r = request(
        "candidate_performance",
        "ratio",
        &[],
        json!({"kind":"pbo","candidates":["a","b"],"rows":rows(&vec![[1.,2.];4000]),"blocks":10,"metric":"mean"}),
    );
    r.as_of_ms = 5000;
    r.fit_cutoff_ms = 5000;
    assert_eq!(
        analysis::calculate(&r).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
}
