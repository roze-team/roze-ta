use roze_ta::{
    engine::SeriesIdentity,
    reference_all::{self, Operation, Sample, Stream},
};
use serde_json::json;

fn identity() -> SeriesIdentity {
    serde_json::from_value(json!({"series_id":"test-series","instrument":"TEST","timeframe":"1d","source":"independent-test","data_version":"v1"})).unwrap()
}
fn sample(kind: &str, i: usize) -> Sample {
    let t = 1_700_000_000_000_i64 + i as i64 * 86_400_000;
    let close = 100.0 + (i as f64 * 0.7).sin() * 2.0 + i as f64 * 0.03;
    let value = match kind {
        "f64" => json!(close),
        "(f64, f64)" => json!([close, 98.0 + (i as f64 * 0.9).cos()]),
        "Candle" => {
            json!({"open":close-0.1,"high":close+0.5,"low":close-0.5,"close":close,"volume":100.0+(i%7) as f64,"timestamp":t})
        }
        "OrderBook" => {
            json!({"bids":[{"price":close-0.1,"size":3.0},{"price":close-0.2,"size":5.0}],"asks":[{"price":close+0.1,"size":4.0},{"price":close+0.2,"size":6.0}]})
        }
        "Trade" => {
            json!({"price":close,"size":2.0+(i%3) as f64,"side":if i.is_multiple_of(2) {"Buy"} else {"Sell"},"timestamp":t})
        }
        "TradeQuote" => {
            json!({"trade":{"price":close+0.02,"size":2.0,"side":"Buy","timestamp":t},"mid":close})
        }
        "DerivativesTick" => {
            json!({"funding_rate":0.0001+(i%3) as f64*0.00001,"mark_price":close,"index_price":close-0.1,"futures_price":close+0.3,"open_interest":1000.0+i as f64,"long_size":600.0,"short_size":400.0,"taker_buy_volume":50.0,"taker_sell_volume":40.0,"long_liquidation":5.0,"short_liquidation":3.0,"timestamp":t})
        }
        "CrossSection" => {
            json!({"members":[{"change":1.0,"volume":100.0,"new_high":true,"new_low":false,"above_ma":true,"on_buy_signal":true},{"change":-1.0,"volume":110.0,"new_high":false,"new_low":true,"above_ma":false,"on_buy_signal":false}],"timestamp":t})
        }
        _ => panic!("missing test input type {kind}"),
    };
    Sample {
        at_ms: t,
        available_at_ms: t + 1,
        value,
    }
}

#[test]
fn every_constructor_and_input_kind_is_callable_and_restores() {
    let catalog = reference_all::catalog().unwrap();
    let entries = catalog["entries"].as_array().unwrap();
    let source_names = roze_ta::wickra_all::FAMILIES
        .iter()
        .flat_map(|(_, names)| names.iter().map(|name| format!("wickra.{name}")))
        .collect::<std::collections::BTreeSet<_>>();
    let registered_names = entries
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .filter(|id| id.starts_with("wickra."))
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        registered_names, source_names,
        "every upstream export must be callable"
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e["id"].as_str().unwrap().starts_with("wickra."))
            .count(),
        514
    );
    let mut failures = Vec::new();
    for entry in entries {
        let id = entry["id"].as_str().unwrap();
        let params = if id == "wickra.TdRei" {
            json!({"period":5})
        } else {
            entry["example_params"].clone()
        };
        let operation = Operation {
            id: id.into(),
            params,
        };
        let mut stream = match Stream::new(identity(), operation.clone()) {
            Ok(stream) => stream,
            Err(e) => {
                failures.push(format!("{id} params {}: {e}", operation.params));
                continue;
            }
        };
        for i in 0..16 {
            let mut s = sample(entry["input_type"].as_str().unwrap(), i);
            if id == "extra.VariableSma" {
                s.value[1] = json!(3);
            }
            stream
                .update(&s, s.available_at_ms)
                .unwrap_or_else(|e| panic!("{id} sample {i}: {e}"));
        }
        let snapshot = stream.snapshot().unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();
        let decoded = serde_json::from_str(&serialized).unwrap();
        let mut restored = Stream::restore(&decoded, &identity(), &operation)
            .unwrap_or_else(|e| panic!("restore {id}: {e}"));
        assert_eq!(stream.latest(), restored.latest(), "{id}");
        let mut s = sample(entry["input_type"].as_str().unwrap(), 16);
        if id == "extra.VariableSma" {
            s.value[1] = json!(3);
        }
        assert_eq!(
            stream.update(&s, s.available_at_ms).unwrap(),
            restored.update(&s, s.available_at_ms).unwrap(),
            "{id}"
        );
        stream.reset().unwrap();
        assert!(stream.latest().is_none());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn reference_sma_matches_manual_values_and_transactions_preserve_state() {
    let operation = Operation {
        id: "wickra.Sma".into(),
        params: json!({"period":3}),
    };
    let mut stream = Stream::new(identity(), operation.clone()).unwrap();
    for (i, x) in [2.0, 5.0, 11.0, 2.0].into_iter().enumerate() {
        let mut s = sample("f64", i);
        s.value = json!(x);
        let row = stream.update(&s, s.available_at_ms).unwrap();
        if i >= 2 {
            assert_eq!(row.value, Some(json!(6.0)));
        }
    }
    let before = serde_json::to_value(stream.snapshot().unwrap()).unwrap();
    let s = sample("f64", 4);
    assert!(stream.update(&s, s.at_ms).is_err());
    let mut bad = s.clone();
    bad.value = json!({"unexpected":true});
    assert!(stream.update(&bad, bad.available_at_ms).is_err());
    assert!(stream
        .update_with_checkpoint(&s, s.available_at_ms, &mut || Err(
            roze_ta::error::TaError::new(roze_ta::error::ErrorCode::Cancelled, "test")
        ))
        .is_err());
    assert_eq!(
        before,
        serde_json::to_value(stream.snapshot().unwrap()).unwrap()
    );
    let mut corrupt = stream.snapshot().unwrap();
    corrupt.history[0].value = json!(9.0);
    assert!(Stream::restore(&corrupt, &identity(), &operation).is_err());
}

#[test]
fn rich_input_validation_rejects_bypassing_constructors_and_times() {
    for (id, params, kind) in [
        ("wickra.OrderBookImbalanceTop1", json!({}), "OrderBook"),
        ("wickra.AdvanceDecline", json!({}), "CrossSection"),
        ("wickra.FundingRate", json!({}), "DerivativesTick"),
    ] {
        let mut stream = Stream::new(
            identity(),
            Operation {
                id: id.into(),
                params,
            },
        )
        .unwrap();
        let mut s = sample(kind, 0);
        match kind {
            "OrderBook" => s.value["asks"][0]["price"] = json!(1.0),
            "CrossSection" => s.value["members"][0]["volume"] = json!(-1.0),
            _ => s.value["timestamp"] = json!(s.at_ms + 1),
        }
        assert!(stream.update(&s, s.available_at_ms).is_err(), "{id}");
        assert!(stream.latest().is_none());
    }
}

fn extra_rows(
    name: &str,
    params: serde_json::Value,
    values: Vec<serde_json::Value>,
) -> Vec<reference_all::Row> {
    let mut stream = Stream::new(
        identity(),
        Operation {
            id: format!("extra.{name}"),
            params,
        },
    )
    .unwrap();
    values
        .into_iter()
        .enumerate()
        .map(|(i, value)| {
            let t = 1000 + i as i64;
            let mut value = value;
            if value.is_object() {
                value["timestamp"] = json!(t);
            }
            stream
                .update(
                    &Sample {
                        at_ms: t,
                        available_at_ms: t + 1,
                        value,
                    },
                    10_000,
                )
                .unwrap()
        })
        .collect()
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10 * (1.0 + expected.abs()),
        "{actual} != {expected}"
    );
}

#[test]
fn elementary_operations_have_independent_closed_form_values() {
    for (name, x, want) in [
        ("Acos", 1.0, 0.0),
        ("Asin", 0.0, 0.0),
        ("Atan", 0.0, 0.0),
        ("Ceil", 1.1, 2.0),
        ("Cos", 0.0, 1.0),
        ("Cosh", 0.0, 1.0),
        ("Exp", 0.0, 1.0),
        ("Floor", 1.9, 1.0),
        ("Ln", std::f64::consts::E, 1.0),
        ("Log10", 100.0, 2.0),
        ("Sin", 0.0, 0.0),
        ("Sinh", 0.0, 0.0),
        ("Sqrt", 9.0, 3.0),
        ("Tan", 0.0, 0.0),
        ("Tanh", 0.0, 0.0),
    ] {
        close(
            extra_rows(name, json!({}), vec![json!(x)])[0]
                .value
                .as_ref()
                .unwrap()
                .as_f64()
                .unwrap(),
            want,
        );
    }
    for (name, want) in [
        ("Add", 10.0),
        ("Subtract", 6.0),
        ("Multiply", 16.0),
        ("Divide", 4.0),
    ] {
        close(
            extra_rows(name, json!({}), vec![json!([8.0, 2.0])])[0]
                .value
                .as_ref()
                .unwrap()
                .as_f64()
                .unwrap(),
            want,
        );
    }
    assert_eq!(
        extra_rows("Divide", json!({}), vec![json!([1.0, 0.0])])[0].status,
        reference_all::Status::UndefinedResult
    );
    assert_eq!(
        extra_rows("Sqrt", json!({}), vec![json!(-1.0)])[0].status,
        reference_all::Status::UndefinedResult
    );
}

#[test]
fn rolling_and_regression_variants_match_hand_calculations() {
    for (name, want) in [
        ("Sum", 18.0),
        ("Minimum", 2.0),
        ("Maximum", 11.0),
        ("MinIndex", 0.0),
        ("MaxIndex", 2.0),
        ("MeanDeviation", 10.0 / 3.0),
        ("RelativeVolume", 11.0 / 6.0),
    ] {
        let rows = extra_rows(
            name,
            json!({"period":3}),
            vec![json!(2.0), json!(5.0), json!(11.0)],
        );
        close(rows[2].value.as_ref().unwrap().as_f64().unwrap(), want);
    }
    let w = extra_rows(
        "WilderSum",
        json!({"period":3}),
        [2.0, 5.0, 11.0, 2.0].map(|v| json!(v)).to_vec(),
    );
    close(w[3].value.as_ref().unwrap().as_f64().unwrap(), 14.0);
    let r = extra_rows(
        "SingleFactorModel",
        json!({"period":3}),
        vec![json!([3.0, 1.0]), json!([5.0, 2.0]), json!([7.0, 3.0])],
    );
    assert_eq!(
        r[2].value,
        Some(json!({"alpha":1.0,"beta":2.0,"r_squared":1.0}))
    );
    let r = extra_rows(
        "VariableSma",
        json!({"period":3}),
        vec![json!([2.0, 2.0]), json!([5.0, 3.0]), json!([11.0, 2.0])],
    );
    close(r[2].value.as_ref().unwrap().as_f64().unwrap(), 8.0);
    let r = extra_rows(
        "TrendDetectionIndex",
        json!({"period":2}),
        (1..=6).map(|v| json!(v)).collect(),
    );
    assert_eq!(r[5].value, Some(json!({"tdi":0.0,"di":4.0})));
    let r = extra_rows(
        "PriceBands",
        json!({"period":3}),
        (1..=5).map(|v| json!(v)).collect(),
    );
    assert_eq!(
        r[4].value,
        Some(json!({"lower":4.0,"center":4.0,"upper":4.0}))
    );
}

#[test]
fn composed_volume_volatility_and_long_warmup_formulas() {
    let bars = vec![10.0, 12.0, 11.0]
        .into_iter()
        .map(|x| json!({"open":x,"high":x+0.5,"low":x-0.5,"close":x,"volume":2.0,"timestamp":0}))
        .collect::<Vec<_>>();
    let sfx = extra_rows("Sfx", json!({"period":2}), bars.clone());
    assert_eq!(
        sfx[2].value,
        Some(json!({"atr":1.625,"std_dev":0.5,"ma_std_dev":0.75}))
    );
    let snr = extra_rows("SignalNoiseRatio", json!({"period":2}), bars.clone());
    close(
        snr[2].value.as_ref().unwrap().as_f64().unwrap(),
        1.0 / 1.625,
    );
    let sobv = extra_rows("SmoothedObv", json!({"period":2}), bars.clone());
    close(sobv[1].value.as_ref().unwrap().as_f64().unwrap(), 1.0);
    close(sobv[2].value.as_ref().unwrap().as_f64().unwrap(), 1.0);
    let range = extra_rows("AverageBarRange", json!({"period":2}), bars);
    close(range[1].value.as_ref().unwrap().as_f64().unwrap(), 1.0);
    let guppy = extra_rows("Guppy", json!({}), vec![json!(2.0); 60]);
    assert_eq!(guppy[59].value, Some(json!(vec![2.0; 12])));
    let dvi = extra_rows("Dvi", json!({}), vec![json!(10.0); 357]);
    assert!(dvi[355].value.is_none());
    assert_eq!(
        dvi[356].value,
        Some(json!({"magnitude":1.0,"stretch":1.0,"dvi":1.0}))
    );
    let cmo = extra_rows(
        "SmoothedCmo",
        json!({"period":2}),
        vec![json!(10.0), json!(12.0), json!(11.0)],
    );
    close(
        cmo[2].value.as_ref().unwrap().as_f64().unwrap(),
        100.0 / 3.0,
    );
    let pvo = extra_rows(
        "PercentageVolumeOscillator",
        json!({}),
        vec![json!(10.0); 34],
    );
    assert_eq!(
        pvo[33].value,
        Some(json!({"pvo":0.0,"signal":0.0,"histogram":0.0}))
    );
}

#[test]
fn sample_statistics_and_lags_have_independent_values() {
    for (name, expected) in [
        ("SampleVariance", 21.0),
        ("SampleStdDev", 21.0_f64.sqrt()),
        ("ScaledMedianDeviation", 4.4478),
    ] {
        let rows = extra_rows(
            name,
            json!({"period":3}),
            vec![json!(2), json!(5), json!(11)],
        );
        close(rows[2].value.as_ref().unwrap().as_f64().unwrap(), expected);
    }
    let rows = extra_rows(
        "SampleCovariance",
        json!({"period":3}),
        vec![json!([1, 3]), json!([2, 5]), json!([3, 7])],
    );
    close(rows[2].value.as_ref().unwrap().as_f64().unwrap(), 2.0);
    let rows = extra_rows(
        "Lags",
        json!({"period":2}),
        vec![json!(2), json!(5), json!(11)],
    );
    assert_eq!(rows[2].value, Some(json!([5.0, 2.0])));
    let rows = extra_rows(
        "Growth",
        json!({}),
        vec![json!([100, 1]), json!([110, 0]), json!([121, -1])],
    );
    close(rows[1].value.as_ref().unwrap().as_f64().unwrap(), 1.1);
    close(rows[2].value.as_ref().unwrap().as_f64().unwrap(), 1.1);
}

#[test]
fn logarithmic_prices_reject_zero_without_advancing_state() {
    let mut stream = Stream::new(
        identity(),
        Operation {
            id: "wickra.LogReturn".into(),
            params: json!({"period":1}),
        },
    )
    .unwrap();
    let mut s = sample("f64", 0);
    s.value = json!(0);
    assert!(stream.update(&s, s.available_at_ms).is_err());
    assert!(stream.latest().is_none());
}
