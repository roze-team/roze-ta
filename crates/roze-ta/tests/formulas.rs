use roze_ta::analysis::{self, Request};
use serde_json::{json, Value};
fn calc(task: Value) -> Value {
    let r:Request=serde_json::from_value(json!({"schema_version":1,"identity":{"series_id":"formula","instrument":"TEST","timeframe":"snapshot","source":"manual","data_version":"1"},"input_kind":"explicit_formula_parameters","units":"specified_by_task","as_of_ms":10,"fit_cutoff_ms":10,"points":[],"events":[],"operations":[{"method":"formula","spec":{"available_at_ms":10,"task":task}}]})).unwrap();
    serde_json::to_value(analysis::calculate(&r).unwrap()).unwrap()["results"][0]["result"].clone()
}
fn val(o: &Value, name: &str) -> f64 {
    o["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == name)
        .unwrap()["value"]["value"]
        .as_f64()
        .unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} != {b}");
}
#[test]
fn black_scholes_reference_and_parity() {
    let o = calc(
        json!({"kind":"black_scholes","spot":100.0,"strike":100.0,"years":1.0,"rate":0.05,"dividend_yield":0.0,"volatility":0.2}),
    );
    close(val(&o, "call"), 10.450583572185565);
    close(val(&o, "put"), 5.573526022256971);
    close(val(&o, "call_delta"), 0.6368306511756191);
    close(
        val(&o, "call") - val(&o, "put"),
        100.0 * (1.0 - (-0.05f64).exp()),
    );
}
#[test]
fn execution_cost_reconciliation_buy_and_sell() {
    for (side, sign) in [("buy", 1.0), ("sell", -1.0)] {
        let o = calc(
            json!({"kind":"execution_quality","side":side,"requested_quantity":10.0,"decision_price":100.0,"arrival_price":101.0,"terminal_price":103.0,"multiplier":2.0,"fills":[{"quantity":6.0,"price":102.0,"fee":1.0}]}),
        );
        close(val(&o, "fill_ratio"), 0.6);
        close(val(&o, "implementation_shortfall"), sign * 48.0 + 1.0);
        close(
            val(&o, "implementation_shortfall"),
            val(&o, "delay_cost")
                + val(&o, "trading_cost")
                + val(&o, "opportunity_cost")
                + val(&o, "fees"),
        );
    }
}
#[test]
fn almgren_limits_and_overflow_resistant_path() {
    let mut t = json!({"kind":"almgren_chriss","quantity":100.0,"horizon":10.0,"volatility":2.0,"risk_aversion":0.0,"temporary_impact":0.5,"steps":2});
    let o = calc(t.clone());
    close(o["series"][1][1]["value"]["value"].as_f64().unwrap(), 50.0);
    close(o["series"][1][2]["value"]["value"].as_f64().unwrap(), 10.0);
    t["risk_aversion"] = json!(10000.0);
    let o = calc(t);
    close(o["series"][0][1]["value"]["value"].as_f64().unwrap(), 100.0);
    close(o["series"][2][1]["value"]["value"].as_f64().unwrap(), 0.0);
}
#[test]
fn ofi_equal_prices_and_price_changes() {
    let o = calc(
        json!({"kind":"order_flow_imbalance","quotes":[{"bid":99.0,"ask":101.0,"bid_size":10.0,"ask_size":8.0},{"bid":99.0,"ask":101.0,"bid_size":12.0,"ask_size":5.0},{"bid":100.0,"ask":102.0,"bid_size":7.0,"ask_size":9.0}]}),
    );
    close(val(&o, "ofi"), 17.0);
}
#[test]
fn risk_neutral_quote_limit_and_cost_basis_points() {
    let o = calc(
        json!({"kind":"avellaneda_stoikov","mid":100.0,"inventory":3.0,"volatility":2.0,"remaining_time":1.0,"risk_aversion":0.0,"intensity_decay":2.0}),
    );
    close(val(&o, "bid"), 99.5);
    close(val(&o, "ask"), 100.5);
    let o = calc(
        json!({"kind":"cost_model","gross_return":0.1,"turnover":2.0,"commission_bps":3.0,"half_spread_bps":2.0,"slippage_bps":4.0,"impact_bps":1.0}),
    );
    close(val(&o, "net_return"), 0.098);
}
#[test]
fn total_variance_bayes_and_information_boundaries() {
    let o = calc(
        json!({"kind":"discrete_probability","probabilities":[0.25,0.75],"values":[1.0,3.0],"conditional_variances":[4.0,0.0]}),
    );
    close(val(&o, "expectation"), 2.5);
    close(val(&o, "total_variance"), 1.75);
    let o = calc(
        json!({"kind":"bayes","prior":0.1,"likelihood_if_true":0.9,"likelihood_if_false":0.2}),
    );
    close(val(&o, "posterior_probability"), 1.0 / 3.0);
    let o = calc(json!({"kind":"information","p":[0.5,0.5],"q":[1.0,0.0]}));
    close(val(&o, "entropy_nats"), 2f64.ln());
    assert_eq!(o["values"][1]["value"]["status"], "undefined");
}
#[test]
fn futures_negative_prices_stop_floor_carry_hedge_kelly_losses() {
    let o = calc(
        json!({"kind":"futures","entry":-10.0,"exit":-5.0,"multiplier":100.0,"contracts":2.0,"side":"buy","margin_rate":0.1,"equity":10000.0,"costs":0.0}),
    );
    close(val(&o, "pnl"), 1000.0);
    close(val(&o, "initial_margin"), 200.0);
    let o = calc(
        json!({"kind":"stop_sizing","equity":10000.0,"risk_fraction":0.01,"entry":10.0,"stop":7.0,"multiplier":10.0,"cost_per_contract":0.0}),
    );
    close(val(&o, "contracts"), 3.0);
    let o = calc(
        json!({"kind":"carry","spot":100.0,"futures":101.0,"rate":0.0,"storage_rate":0.0,"convenience_yield":0.0,"years":1.0}),
    );
    close(val(&o, "fair_futures"), 100.0);
    close(val(&o, "basis_futures_minus_spot"), 1.0);
    let o = calc(
        json!({"kind":"hedge","correlation":0.8,"spot_volatility":0.2,"futures_volatility":0.4,"exposure_value":10000.0,"contract_value":100.0}),
    );
    close(val(&o, "hedge_contracts"), 40.0);
    let o = calc(json!({"kind":"kelly","win_probability":0.6,"payoff_ratio":2.0,"fraction":0.5}));
    close(val(&o, "fractional_kelly"), 0.2);
    let o =
        calc(json!({"kind":"losses","actual":[0.0,0.0],"predicted":[1.0,3.0],"huber_delta":2.0}));
    close(val(&o, "mse"), 5.0);
    close(val(&o, "mae"), 2.0);
    close(val(&o, "huber"), 2.25);
}
