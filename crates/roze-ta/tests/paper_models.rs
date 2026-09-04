use roze_ta::analysis::{self, Request};
use serde_json::{json, Value};
fn request(method: &str, kind: &str, spec: Value) -> Request {
    serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"models","instrument":"TEST","timeframe":"snapshot","source":"manual","data_version":"1"},"input_kind":kind,"units":"ratio","as_of_ms":100,"fit_cutoff_ms":100,"points":[],"events":[],"operations":[{"method":method,"spec":spec}]})).unwrap()
}
fn output(r: &Request) -> Value {
    serde_json::to_value(analysis::calculate(r).unwrap()).unwrap()["results"][0]["result"].clone()
}
fn allocation(task: Value) -> Value {
    output(&request(
        "allocation",
        "allocation_parameters",
        json!({"available_at_ms":1,"task":task}),
    ))
}
fn regression(method: Value, rows: Vec<Vec<f64>>) -> Value {
    output(&request(
        "regression",
        "regression_matrix",
        json!({"intercept":true,"method":method,"rows":rows.iter().enumerate().map(|(i,v)|json!({"at_ms":i+1,"available_at_ms":i+1,"values":v})).collect::<Vec<_>>()}),
    ))
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
#[test]
fn ols_qr_reference_and_rank_deficiency() {
    let o = regression(
        json!({"kind":"ols"}),
        (0..6)
            .map(|x| vec![1.0 + 2.0 * x as f64, x as f64])
            .collect(),
    );
    close(o["coefficients"][0].as_f64().unwrap(), 1.0);
    close(o["coefficients"][1].as_f64().unwrap(), 2.0);
    let req = request(
        "regression",
        "regression_matrix",
        json!({"intercept":true,"method":{"kind":"ols"},"rows":(1..6).map(|i|json!({"at_ms":i,"available_at_ms":i,"values":[i as f64,1.0]})).collect::<Vec<_>>()}),
    );
    assert!(analysis::calculate(&req).is_err());
}
#[test]
fn ridge_lasso_closed_form_and_logistic_balanced_sample() {
    let rows = vec![
        vec![-2.0, -1.0],
        vec![0.0, 0.0],
        vec![2.0, 1.0],
        vec![0.0, 0.0],
    ];
    let o = regression(json!({"kind":"ridge","lambda":2.0}), rows.clone());
    close(o["coefficients"][1].as_f64().unwrap(), 1.0);
    let o = regression(
        json!({"kind":"lasso","lambda":0.25,"max_iterations":50,"tolerance":1e-10}),
        rows,
    );
    close(o["coefficients"][1].as_f64().unwrap(), 1.5);
    assert_eq!(o["converged"], true);
    let o = regression(
        json!({"kind":"logistic","l2":1.0,"max_iterations":50,"tolerance":1e-10}),
        vec![
            vec![0.0, -1.0],
            vec![1.0, -1.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ],
    );
    close(o["fitted"][0].as_f64().unwrap(), 0.5);
    assert_eq!(o["converged"], true);
}
#[test]
fn pca_diagonal_covariance_and_eigenvectors() {
    let o = regression(
        json!({"kind":"pca","components":1}),
        vec![
            vec![1.0, 0.0],
            vec![-1.0, 0.0],
            vec![0.0, 2.0],
            vec![0.0, -2.0],
        ],
    );
    close(o["eigenvalues"][0].as_f64().unwrap(), 8.0 / 3.0);
    close(o["loadings"][1][0].as_f64().unwrap().abs(), 1.0);
}
#[test]
fn analytic_allocation_constraints() {
    let c = json!([[1.0, 0.0], [0.0, 4.0]]);
    let o = allocation(
        json!({"kind":"minimum_variance","covariance":c,"means":[0.1,0.2],"target_return":null}),
    );
    close(o["weights"][0].as_f64().unwrap(), 0.8);
    let o = allocation(
        json!({"kind":"minimum_variance","covariance":c,"means":[0.1,0.2],"target_return":0.15}),
    );
    close(o["weights"][0].as_f64().unwrap(), 0.5);
    let o = allocation(
        json!({"kind":"mean_variance","covariance":c,"means":[0.1,0.2],"risk_aversion":2.0}),
    );
    close(o["weights"][0].as_f64().unwrap(), 0.05);
    let o = allocation(
        json!({"kind":"maximum_sharpe","covariance":c,"means":[0.1,0.2],"risk_free":0.0}),
    );
    close(o["weights"][0].as_f64().unwrap(), 2.0 / 3.0);
}
#[test]
fn black_litterman_one_asset_conjugate_reference() {
    let o = allocation(
        json!({"kind":"black_litterman","covariance":[[2.0]],"market_weights":[1.0],"risk_aversion":1.0,"tau":0.5,"views":[[1.0]],"view_returns":[4.0],"view_covariance":[[1.0]]}),
    );
    close(o["means"][0].as_f64().unwrap(), 3.0);
    close(o["mean_uncertainty"][0][0].as_f64().unwrap(), 0.5);
    close(o["weights"][0].as_f64().unwrap(), 1.5);
}
#[test]
fn shrinkage_and_cvar_certificate() {
    let r = allocation(
        json!({"kind":"risk_parity","covariance":[[1.0,0.0],[0.0,4.0]],"max_iterations":30,"tolerance":1e-10}),
    );
    close(r["weights"][0].as_f64().unwrap(), 2.0 / 3.0);
    assert_eq!(r["converged"], true);
    let lw = allocation(
        json!({"kind":"ledoit_wolf","observations":[[1.0,0.0],[-1.0,0.0],[0.0,2.0],[0.0,-2.0]]}),
    );
    close(
        lw["diagnostics"][0]["value"]["value"].as_f64().unwrap(),
        17.0 / 18.0,
    );
    let o = allocation(json!({"kind":"ledoit_wolf","observations":[[-1.0,-1.0],[1.0,1.0]]}));
    close(o["covariance"][0][1].as_f64().unwrap(), 1.0);
    let o = allocation(
        json!({"kind":"cvar_optimize","returns":[[-0.1,-0.1],[-0.1,-0.1]],"confidence":0.95,"max_iterations":20,"tolerance":1e-8}),
    );
    assert_eq!(o["converged"], true);
    close(o["diagnostics"][0]["value"]["value"].as_f64().unwrap(), 0.1);
    close(o["diagnostics"][3]["value"]["value"].as_f64().unwrap(), 0.0);
}
