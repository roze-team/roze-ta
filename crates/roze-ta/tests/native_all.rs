use roze_ta::{
    core::{Action, Candle, IndicatorConfig, Method, OHLCV},
    engine::{ClosedBar, OutputMode, SeriesIdentity},
    error::ErrorCode,
    native::{self, Input, Operation, Request, Timed},
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn input(kind: &str) -> Input {
    let xs: Vec<_> = (1..=64)
        .map(|i| 100.0 + ((i as f64) / 3.0).sin() * 8.0)
        .collect();
    match kind {
        "bars" => Input::Bars(
            xs.iter()
                .enumerate()
                .map(|(i, &x)| ClosedBar {
                    candle: roze_ta::catalog::Candle {
                        closed_at_ms: i as i64 + 1,
                        open: x,
                        high: x + 2.0,
                        low: x - 2.0,
                        close: x + 0.5,
                        volume: (i % 7 + 1) as f64,
                    },
                    available_at_ms: i as i64 + 2,
                })
                .collect(),
        ),
        "scalars" => Input::Scalars(
            xs.iter()
                .enumerate()
                .map(|(i, &x)| Timed {
                    at_ms: i as i64 + 1,
                    available_at_ms: i as i64 + 2,
                    value: x,
                })
                .collect(),
        ),
        "pairs" => Input::Pairs(
            xs.iter()
                .enumerate()
                .map(|(i, &x)| Timed {
                    at_ms: i as i64 + 1,
                    available_at_ms: i as i64 + 2,
                    value: (x, 100.0),
                })
                .collect(),
        ),
        "values" => Input::Values(
            xs.iter()
                .enumerate()
                .map(|(i, &x)| Timed {
                    at_ms: i as i64 + 1,
                    available_at_ms: i as i64 + 2,
                    value: json!({"index":i,"x":x}),
                })
                .collect(),
        ),
        _ => panic!("unknown kind"),
    }
}
fn request(id: &str, kind: &str) -> Request {
    Request {
        schema_version: 1,
        identity: SeriesIdentity {
            series_id: "native-test".into(),
            instrument: "TEST".into(),
            timeframe: "1ms".into(),
            source: "fixture".into(),
            data_version: "1".into(),
        },
        as_of_ms: 1000,
        data: input(kind),
        operations: vec![Operation {
            id: id.into(),
            params: json!({}),
        }],
        output: OutputMode::Series,
    }
}
fn candle(b: &ClosedBar) -> Candle {
    let b = &b.candle;
    Candle {
        open: b.open,
        high: b.high,
        low: b.low,
        close: b.close,
        volume: b.volume,
    }
}
fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(if v == 0.0 { 0.0 } else { v })
}
trait Oracle {
    fn json(self) -> Value;
}
impl Oracle for f64 {
    fn json(self) -> Value {
        json!({"kind":"scalar","value":finite(self)})
    }
}
impl Oracle for u8 {
    fn json(self) -> Value {
        json!({"kind":"index","value":self})
    }
}
impl Oracle for Action {
    fn json(self) -> Value {
        json!({"kind":"signal","value":{"analog":self.analog(),"ratio":self.ratio()}})
    }
}
fn bar_json(b: &impl OHLCV) -> Value {
    json!({"open":finite(b.open()),"high":finite(b.high()),"low":finite(b.low()),"close":finite(b.close()),"volume":finite(b.volume())})
}
impl Oracle for Candle {
    fn json(self) -> Value {
        json!({"kind":"candle","value":bar_json(&self)})
    }
}
impl Oracle for Option<Candle> {
    fn json(self) -> Value {
        self.map_or_else(|| json!({"kind":"pending"}), Oracle::json)
    }
}
impl Oracle for Value {
    fn json(self) -> Value {
        json!({"kind":"data","value":self})
    }
}
impl Oracle for (f64, f64) {
    fn json(self) -> Value {
        json!({"kind":"data","value":[self.0,self.1]})
    }
}
impl Oracle for roze_ta::methods::renko::RenkoOutput {
    fn json(self) -> Value {
        json!({"kind":"candles","values":self.map(|b|bar_json(&b)).collect::<Vec<_>>()})
    }
}
fn compare(req: &Request, expected: Vec<Value>) {
    let actual = native::calculate(req).unwrap_or_else(|e| panic!("{}: {e}", req.operations[0].id));
    let values: Vec<_> = actual.results[0]
        .rows
        .iter()
        .map(|r| serde_json::to_value(&r.output).unwrap())
        .collect();
    assert_eq!(values, expected, "{}", req.operations[0].id);
    assert_eq!(
        actual.output_hash,
        native::calculate(req).unwrap().output_hash
    );
    assert_eq!(actual.results[0].rows.last().unwrap().available_at_ms, 65);
}
fn scalar<M>(id: &str, params: M::Params)
where
    M: Method<Input = f64>,
    M::Output: Oracle,
{
    let req = request(id, "scalars");
    let Input::Scalars(v) = &req.data else {
        unreachable!()
    };
    let mut state = M::new(params, &v[0].value).unwrap();
    compare(
        &req,
        v.iter().map(|v| state.next(&v.value).json()).collect(),
    );
}
fn pair<M>(id: &str, params: M::Params)
where
    M: Method<Input = (f64, f64)>,
    M::Output: Oracle,
{
    let req = request(id, "pairs");
    let Input::Pairs(v) = &req.data else {
        unreachable!()
    };
    let mut state = M::new(params, &v[0].value).unwrap();
    compare(
        &req,
        v.iter().map(|v| state.next(&v.value).json()).collect(),
    );
}
fn bars<M>(id: &str, params: M::Params)
where
    M: Method<Input = dyn OHLCV>,
    M::Output: Oracle,
{
    let req = request(id, "bars");
    let Input::Bars(v) = &req.data else {
        unreachable!()
    };
    let mut state = M::new(params, &candle(&v[0])).unwrap();
    compare(
        &req,
        v.iter().map(|v| state.next(&candle(v)).json()).collect(),
    );
}
fn owned<M>(id: &str, params: M::Params)
where
    M: Method<Input = Candle>,
    M::Output: Oracle,
{
    let req = request(id, "bars");
    let Input::Bars(v) = &req.data else {
        unreachable!()
    };
    let mut state = M::new(params, &candle(&v[0])).unwrap();
    compare(
        &req,
        v.iter().map(|v| state.next(&candle(v)).json()).collect(),
    );
}
fn indicator<C>(id: &str)
where
    C: IndicatorConfig + Default,
{
    let req = request(id, "bars");
    let Input::Bars(v) = &req.data else {
        unreachable!()
    };
    let cs: Vec<_> = v.iter().map(candle).collect();
    let native = C::default().over(&cs).unwrap();
    compare(&req,native.iter().map(|r|json!({"kind":"indicator","values":r.values().iter().copied().map(finite).collect::<Vec<_>>(),
        "signals":r.signals().iter().map(|s|json!({"analog":s.analog(),"ratio":s.ratio()})).collect::<Vec<_>>()})).collect());
}

#[test]
fn every_indicator_matches_direct_native_over() {
    indicator::<roze_ta::indicators::Aroon>("indicator.aroon");
    indicator::<roze_ta::indicators::AverageDirectionalIndex>(
        "indicator.average_directional_index",
    );
    indicator::<roze_ta::indicators::AwesomeOscillator>("indicator.awesome_oscillator");
    indicator::<roze_ta::indicators::BollingerBands>("indicator.bollinger_bands");
    indicator::<roze_ta::indicators::ChaikinMoneyFlow>("indicator.chaikin_money_flow");
    indicator::<roze_ta::indicators::ChaikinOscillator>("indicator.chaikin_oscillator");
    indicator::<roze_ta::indicators::ChandeKrollStop>("indicator.chande_kroll_stop");
    indicator::<roze_ta::indicators::ChandeMomentumOscillator>(
        "indicator.chande_momentum_oscillator",
    );
    indicator::<roze_ta::indicators::CommodityChannelIndex>("indicator.commodity_channel_index");
    indicator::<roze_ta::indicators::CoppockCurve>("indicator.coppock_curve");
    indicator::<roze_ta::indicators::DetrendedPriceOscillator>(
        "indicator.detrended_price_oscillator",
    );
    indicator::<roze_ta::indicators::DonchianChannel>("indicator.donchian_channel");
    indicator::<roze_ta::indicators::EaseOfMovement>("indicator.ease_of_movement");
    indicator::<roze_ta::indicators::EldersForceIndex>("indicator.elders_force_index");
    indicator::<roze_ta::indicators::Envelopes>("indicator.envelopes");
    indicator::<roze_ta::indicators::FisherTransform>("indicator.fisher_transform");
    indicator::<roze_ta::indicators::HullMovingAverage>("indicator.hull_moving_average");
    indicator::<roze_ta::indicators::IchimokuCloud>("indicator.ichimoku_cloud");
    indicator::<roze_ta::indicators::Kaufman>("indicator.kaufman");
    indicator::<roze_ta::indicators::KeltnerChannel>("indicator.keltner_channel");
    indicator::<roze_ta::indicators::KlingerVolumeOscillator>(
        "indicator.klinger_volume_oscillator",
    );
    indicator::<roze_ta::indicators::KnowSureThing>("indicator.know_sure_thing");
    indicator::<roze_ta::indicators::MACD>("indicator.macd");
    indicator::<roze_ta::indicators::MomentumIndex>("indicator.momentum_index");
    indicator::<roze_ta::indicators::MoneyFlowIndex>("indicator.money_flow_index");
    indicator::<roze_ta::indicators::ParabolicSAR>("indicator.parabolic_sar");
    indicator::<roze_ta::indicators::PivotReversalStrategy>("indicator.pivot_reversal_strategy");
    indicator::<roze_ta::indicators::PriceChannelStrategy>("indicator.price_channel_strategy");
    indicator::<roze_ta::indicators::RelativeStrengthIndex>("indicator.relative_strength_index");
    indicator::<roze_ta::indicators::RelativeVigorIndex>("indicator.relative_vigor_index");
    indicator::<roze_ta::indicators::SMIErgodicIndicator>("indicator.smi_ergodic_indicator");
    indicator::<roze_ta::indicators::StochasticOscillator>("indicator.stochastic_oscillator");
    indicator::<roze_ta::indicators::TrendStrengthIndex>("indicator.trend_strength_index");
    indicator::<roze_ta::indicators::Trix>("indicator.trix");
    indicator::<roze_ta::indicators::TrueStrengthIndex>("indicator.true_strength_index");
    indicator::<roze_ta::indicators::WoodiesCCI>("indicator.woodies_cci");
}
#[test]
fn every_method_matches_direct_native_calls() {
    bars::<roze_ta::methods::ADI>("method.adi", 14);
    scalar::<roze_ta::methods::CCI>("method.cci", 14);
    owned::<roze_ta::methods::CollapseTimeframe<Candle>>("method.collapse_timeframe", 14);
    scalar::<roze_ta::methods::Conv>("method.conv", vec![1.0, 2.0, 3.0]);
    pair::<roze_ta::methods::Cross>("method.cross", ());
    pair::<roze_ta::methods::CrossAbove>("method.cross_above", ());
    pair::<roze_ta::methods::CrossUnder>("method.cross_under", ());
    scalar::<roze_ta::methods::Derivative>("method.derivative", 14);
    scalar::<roze_ta::methods::EMA>("method.ema", 14);
    scalar::<roze_ta::methods::DMA>("method.dma", 14);
    scalar::<roze_ta::methods::TMA>("method.tma", 14);
    scalar::<roze_ta::methods::DEMA>("method.dema", 14);
    scalar::<roze_ta::methods::TEMA>("method.tema", 14);
    bars::<roze_ta::methods::HeikinAshi>("method.heikin_ashi", ());
    scalar::<roze_ta::methods::HighestLowestDelta>("method.highest_lowest_delta", 14);
    scalar::<roze_ta::methods::Highest>("method.highest", 14);
    scalar::<roze_ta::methods::Lowest>("method.lowest", 14);
    scalar::<roze_ta::methods::HighestIndex>("method.highest_index", 14);
    scalar::<roze_ta::methods::LowestIndex>("method.lowest_index", 14);
    scalar::<roze_ta::methods::HMA>("method.hma", 14);
    scalar::<roze_ta::methods::Integral>("method.integral", 14);
    scalar::<roze_ta::methods::LinReg>("method.lin_reg", 14);
    scalar::<roze_ta::methods::MeanAbsDev>("method.mean_abs_dev", 14);
    scalar::<roze_ta::methods::MedianAbsDev>("method.median_abs_dev", 14);
    scalar::<roze_ta::methods::Momentum>("method.momentum", 14);
    scalar::<roze_ta::methods::Past<f64>>("method.past", 14);
    scalar::<roze_ta::methods::RateOfChange>("method.rate_of_change", 14);
    bars::<roze_ta::methods::Renko>("method.renko", (0.01, roze_ta::core::Source::Close));
    scalar::<roze_ta::methods::ReversalSignal>("method.reversal_signal", (2, 3));
    scalar::<roze_ta::methods::UpperReversalSignal>("method.upper_reversal_signal", (2, 3));
    scalar::<roze_ta::methods::LowerReversalSignal>("method.lower_reversal_signal", (2, 3));
    scalar::<roze_ta::methods::RMA>("method.rma", 14);
    scalar::<roze_ta::methods::SMA>("method.sma", 14);
    scalar::<roze_ta::methods::SMM>("method.smm", 14);
    scalar::<roze_ta::methods::StDev>("method.st_dev", 14);
    scalar::<roze_ta::methods::SWMA>("method.swma", 14);
    bars::<roze_ta::methods::TR>("method.tr", ());
    scalar::<roze_ta::methods::TRIMA>("method.trima", 14);
    scalar::<roze_ta::methods::TSI>("method.tsi", (2, 3));
    scalar::<roze_ta::methods::Vidya>("method.vidya", 14);
    scalar::<roze_ta::methods::LinearVolatility>("method.linear_volatility", 14);
    pair::<roze_ta::methods::VWMA>("method.vwma", 14);
    scalar::<roze_ta::methods::WMA>("method.wma", 14);
    scalar::<roze_ta::methods::WSMA>("method.wsma", 14);
    pair::<roze_ta::methods::Past<(f64, f64)>>("method.past", 14);
    owned::<roze_ta::methods::Past<Candle>>("method.past", 14);
}

#[test]
fn inventory_covers_all_source_modules_methods_and_aliases() {
    let cat = native::catalog().unwrap();
    let entries = cat["entries"].as_array().unwrap();
    assert_eq!(cat["counts"], json!({"indicators":36,"methods":44}));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source = std::fs::read_to_string(root.join("indicators/mod.rs")).unwrap();
    let modules: BTreeSet<_> = source
        .lines()
        .filter_map(|l| l.strip_prefix("mod ").and_then(|l| l.strip_suffix(';')))
        .map(str::to_owned)
        .collect();
    let actual: BTreeSet<_> = entries
        .iter()
        .filter(|e| e["kind"] == "indicator")
        .map(|e| e["module"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(modules, actual);
    let mut indicator_aliases = BTreeSet::new();
    for file in std::fs::read_dir(root.join("indicators")).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|s| s != "rs") {
            continue;
        }
        for line in std::fs::read_to_string(path).unwrap().lines() {
            if let Some(tail) = line.strip_prefix("pub type ") {
                indicator_aliases.insert(
                    tail.split([' ', '<', '='])
                        .next()
                        .unwrap()
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    let registered_indicator_aliases: BTreeSet<_> = entries
        .iter()
        .filter(|e| e["kind"] == "indicator")
        .flat_map(|e| e["aliases"].as_array().unwrap())
        .map(|v| {
            v.as_str()
                .unwrap()
                .trim_start_matches("indicator.")
                .replace('_', "")
        })
        .collect();
    assert_eq!(indicator_aliases, registered_indicator_aliases);
    let mut methods = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for file in std::fs::read_dir(root.join("methods")).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|s| s != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        for line in text.lines() {
            if line.starts_with("impl") {
                if let Some((_, tail)) = line.split_once(" Method for ") {
                    methods.insert(
                        tail.chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .collect::<String>(),
                    );
                }
            }
            if let Some(tail) = line.strip_prefix("pub type ") {
                aliases.insert(
                    tail.split([' ', '<', '='])
                        .next()
                        .unwrap()
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    let actual: BTreeSet<_> = entries
        .iter()
        .filter(|e| e["kind"] == "method")
        .map(|e| e["symbol"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(methods, actual);
    let registered: BTreeSet<_> = entries
        .iter()
        .filter(|e| e["kind"] == "method")
        .flat_map(|e| e["aliases"].as_array().unwrap())
        .map(|v| {
            v.as_str()
                .unwrap()
                .trim_start_matches("method.")
                .replace('_', "")
        })
        .collect();
    assert_eq!(aliases, registered);
    for entry in entries {
        let id = entry["id"].as_str().unwrap();
        let kind = entry["inputs"][0].as_str().unwrap();
        let req = request(id, kind);
        let expected = serde_json::to_value(native::calculate(&req).unwrap().results).unwrap();
        for alias in entry["aliases"].as_array().unwrap() {
            let mut req = req.clone();
            req.operations[0].id = alias.as_str().unwrap().into();
            assert_eq!(
                serde_json::to_value(native::calculate(&req).unwrap().results).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn every_native_operation_handles_constant_and_zero_volume_inputs() {
    for entry in native::catalog().unwrap()["entries"].as_array().unwrap() {
        for kind in entry["inputs"].as_array().unwrap() {
            let mut req = request(entry["id"].as_str().unwrap(), kind.as_str().unwrap());
            match &mut req.data {
                Input::Bars(v) => {
                    for b in v {
                        b.candle.open = 100.0;
                        b.candle.high = 100.0;
                        b.candle.low = 100.0;
                        b.candle.close = 100.0;
                        b.candle.volume = 0.0;
                    }
                }
                Input::Scalars(v) => {
                    for p in v {
                        p.value = 0.0;
                    }
                }
                Input::Pairs(v) => {
                    for p in v {
                        p.value = (0.0, 0.0);
                    }
                }
                Input::Values(v) => {
                    for p in v {
                        p.value = Value::Null;
                    }
                }
            }
            let result =
                native::calculate(&req).unwrap_or_else(|e| panic!("{}: {e}", req.operations[0].id));
            assert_eq!(result.results[0].rows.len(), 64);
            assert!(result.results[0].rows.iter().all(|r| [
                "native_output",
                "undefined_result",
                "no_output"
            ]
            .contains(&r.status)));
        }
    }
}

#[test]
fn parameters_time_output_limits_and_cancellation_are_enforced() {
    let base = request("method.sma", "scalars");
    for params in [
        json!({"period":255}),
        json!({"period":256}),
        json!({"period":2.5}),
        json!({"period":0}),
        json!({"command":"x"}),
    ] {
        let mut r = base.clone();
        r.operations[0].params = params;
        assert_eq!(
            native::calculate(&r).unwrap_err().code,
            ErrorCode::InvalidParameter
        );
    }
    let mut r = base.clone();
    r.as_of_ms = 5;
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidTime
    );
    let mut r = base.clone();
    if let Input::Scalars(v) = &mut r.data {
        v[1] = v[0].clone();
    }
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::DuplicateOrUnorderedBar
    );
    let mut r = base.clone();
    r.operations[0].id = "method.missing".into();
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::UnsupportedProfile
    );
    let mut r = base.clone();
    r.operations = vec![r.operations[0].clone(); 17];
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    let mut r = request("method.renko", "bars");
    r.operations[0].params = json!({"size":1e-9});
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    for code in [ErrorCode::Cancelled, ErrorCode::TimedOut] {
        let mut calls = 0;
        assert_eq!(
            native::calculate_controlled(&base, || {
                calls += 1;
                if calls == 70 {
                    Err(roze_ta::error::TaError::new(code, "test"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            code
        );
    }
    let mut r = request("indicator.macd", "bars");
    r.operations[0].params = json!({"signal":{"ema":5}});
    assert!(native::calculate(&r).is_ok());
    r.operations[0].params = json!({"signal":{"exec":5}});
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::InvalidParameter
    );
    let mut r = request("method.vwma", "pairs");
    if let Input::Pairs(v) = &mut r.data {
        for p in v {
            p.value.1 = 0.0;
        }
    }
    assert!(native::calculate(&r).unwrap().results[0]
        .rows
        .iter()
        .all(|row| row.status == "undefined_result"));
}

#[test]
fn seeded_values_pending_and_parameter_changes_are_explicit() {
    let mut r = request("method.sma", "scalars");
    r.operations[0].params = json!({"period":3});
    if let Input::Scalars(v) = &mut r.data {
        v.truncate(3);
        for (i, p) in v.iter_mut().enumerate() {
            p.value = i as f64 + 1.0;
        }
    }
    let result = native::calculate(&r).unwrap();
    let values: Vec<_> = result.results[0]
        .rows
        .iter()
        .map(|r| {
            serde_json::to_value(&r.output).unwrap()["value"]
                .as_f64()
                .unwrap()
        })
        .collect();
    for (actual, expected) in values.into_iter().zip([1.0, 4.0 / 3.0, 2.0]) {
        assert!((actual - expected).abs() < 1e-12);
    }
    assert!(result.quality_flags.contains(&"warmup_not_certified"));
    let hash = result.results[0].parameter_hash.clone();
    r.operations[0].params = json!({"period":4});
    assert_ne!(
        hash,
        native::calculate(&r).unwrap().results[0].parameter_hash
    );
    let mut r = request("method.collapse_timeframe", "bars");
    r.operations[0].params = json!({"period":3});
    let result = native::calculate(&r).unwrap();
    assert_eq!(result.results[0].rows[0].status, "no_output");
    assert_eq!(result.results[0].rows[2].status, "native_output");
    assert_eq!(result.results[0].rows[2].at_ms, 3);
    let mut r = request("method.past", "values");
    r.operations[0].params = json!({"period":1});
    let result = native::calculate(&r).unwrap();
    assert_eq!(
        serde_json::to_value(&result.results[0].rows[1].output).unwrap()["value"]["index"],
        0
    );
}

#[test]
fn generic_history_and_retained_output_have_byte_budgets() {
    let mut r = request("method.past", "values");
    if let Input::Values(v) = &mut r.data {
        v[0].value = json!("x".repeat(native::MAX_JSON_VALUE_BYTES));
    }
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    r.data = Input::Values(
        (1..=300)
            .map(|i| Timed {
                at_ms: i,
                available_at_ms: i + 1,
                value: json!("x".repeat(1000)),
            })
            .collect(),
    );
    r.operations = vec![r.operations[0].clone(); 12];
    assert_eq!(
        native::calculate(&r).unwrap_err().code,
        ErrorCode::LimitExceeded
    );
    r.output = OutputMode::Latest;
    assert_eq!(native::calculate(&r).unwrap().results.len(), 12);
}
