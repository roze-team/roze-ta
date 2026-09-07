use roze_ta::{
    catalog::{self, Candle},
    engine::*,
    wickra::{self, Indicator},
};
const IDS: [&str; 5] = [
    "alma.9",
    "supertrend.10_3",
    "stoch_rsi.14_14",
    "vortex.14",
    "ulcer.14",
];
fn identity() -> SeriesIdentity {
    SeriesIdentity {
        series_id: "r1".into(),
        instrument: "TEST".into(),
        timeframe: "1ms".into(),
        source: "independent".into(),
        data_version: "1".into(),
    }
}
fn bars(n: usize) -> Vec<ClosedBar> {
    (0..n)
        .map(|i| {
            let p = 100. + ((i * 7) % 19) as f64;
            ClosedBar {
                candle: Candle {
                    closed_at_ms: i as i64 + 1,
                    open: p,
                    high: p + 2.,
                    low: p - 2.,
                    close: p,
                    volume: 0.,
                },
                available_at_ms: i as i64 + 2,
            }
        })
        .collect()
}
fn request(data: Vec<ClosedBar>) -> BatchRequest {
    BatchRequest {
        schema_version: 2,
        snapshot_id: "r1".into(),
        identity: identity(),
        as_of_ms: 10000,
        bars: data,
        profiles: IDS.iter().map(|s| s.to_string()).collect(),
        output: OutputMode::Series,
        require_all_ready: false,
    }
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10 * (1. + expected.abs()),
        "{actual} != {expected}"
    );
}
fn raw(high: f64, low: f64, close: f64) -> wickra::Candle {
    wickra::Candle::new(close, high, low, close, 0., 0).unwrap()
}

#[test]
fn manual_values_and_reversals() {
    let mut alma = wickra::Alma::new(3, 0.5, 6.).unwrap();
    assert_eq!(alma.update(1.), None);
    assert_eq!(alma.update(1.), None);
    // weights exp(-2),1,exp(-2); the last sample contributes 0.10650697891920076.
    near(alma.update(2.).unwrap(), 1.1065069789192008);
    let mut v = wickra::Vortex::new(2).unwrap();
    assert!(v.update(raw(10., 8., 9.)).is_none());
    assert!(v.update(raw(12., 9., 11.)).is_none());
    let value = v.update(raw(13., 11., 12.)).unwrap();
    near(value.plus, 1.6);
    near(value.minus, 0.4);
    let mut u = wickra::UlcerIndex::new(2).unwrap();
    assert!(u.update(10.).is_none());
    assert!(u.update(8.).is_none());
    near(u.update(12.).unwrap(), 200_f64.sqrt());
    near(u.update(9.).unwrap(), 312.5_f64.sqrt());
    let mut st = wickra::SuperTrend::new(2, 1.).unwrap();
    assert!(st.update(raw(11., 9., 10.)).is_none());
    for (bar, line, direction) in [
        (raw(12., 10., 11.), 9., 1.),
        (raw(9., 7., 8.), 11., -1.),
        (raw(13., 11., 12.), 8., 1.),
    ] {
        let actual = st.update(bar).unwrap();
        near(actual.value, line);
        assert_eq!(actual.direction, direction);
    }
    let mut s = wickra::StochRsi::new(2, 2).unwrap();
    for p in [10., 12., 11.] {
        assert!(s.update(p).is_none());
    }
    // RSI: 66.666... then 85.714...; latest at top of its two-value window.
    near(s.update(13.).unwrap(), 100.);
    near(s.update(10.).unwrap(), 0.);
}

#[test]
fn fixed_profiles_match_independent_history_formulas() {
    let req = request(bars(120));
    let output = calculate(&req).unwrap();
    let closes: Vec<_> = req.bars.iter().map(|b| b.candle.close).collect();
    // Reference arrays recomputed from historical observations, not migrated helpers.
    let changes: Vec<_> = closes.windows(2).map(|w| w[1] - w[0]).collect();
    let mut rsi = vec![None; closes.len()];
    let (mut gain, mut loss) = (0., 0.);
    for i in 14..closes.len() {
        if i == 14 {
            gain = changes[..14].iter().map(|x| x.max(0.)).sum::<f64>() / 14.;
            loss = changes[..14].iter().map(|x| (-x).max(0.)).sum::<f64>() / 14.;
        } else {
            gain = (13. * gain + changes[i - 1].max(0.)) / 14.;
            loss = (13. * loss + (-changes[i - 1]).max(0.)) / 14.;
        }
        rsi[i] = Some(100. * gain / (gain + loss));
    }
    let mut ranges = Vec::new();
    let (mut atr, mut upper, mut lower, mut direction) = (0., 0., 0., 1.);
    for (i, bar) in req.bars.iter().enumerate() {
        let b = &bar.candle;
        let prev = closes[i.saturating_sub(1)];
        ranges.push(
            (b.high - b.low)
                .max((b.high - prev).abs())
                .max((b.low - prev).abs()),
        );
        if i >= 9 {
            atr = if i == 9 {
                ranges[..10].iter().sum::<f64>() / 10.
            } else {
                (9. * atr + ranges[i]) / 10.
            };
            let bu = (b.high + b.low) / 2. + 3. * atr;
            let bl = (b.high + b.low) / 2. - 3. * atr;
            if i == 9 {
                upper = bu;
                lower = bl;
            } else {
                if bu < upper || prev > upper {
                    upper = bu;
                }
                if bl > lower || prev < lower {
                    lower = bl;
                }
                if direction < 0. && b.close > upper {
                    direction = 1.;
                } else if direction > 0. && b.close < lower {
                    direction = -1.;
                }
            }
            let row = &output.series[1].rows[i];
            near(
                row.values["supertrend"],
                if direction > 0. { lower } else { upper },
            );
            assert_eq!(row.values["direction"], direction);
        }
        if i >= 8 {
            let weights: Vec<_> = (0..9)
                .map(|k| (-0.5 * ((k as f64 - 0.85 * 8.) / (9. / 6.)).powi(2)).exp())
                .collect();
            let expected = weights
                .iter()
                .zip(&closes[i - 8..=i])
                .map(|(w, p)| w * p)
                .sum::<f64>()
                / weights.iter().sum::<f64>();
            near(output.series[0].rows[i].values["alma"], expected);
        }
        if i >= 14 {
            let pairs = &req.bars[i - 14..=i];
            let total = ranges[i - 13..=i].iter().sum::<f64>();
            let plus = pairs
                .windows(2)
                .map(|w| (w[1].candle.high - w[0].candle.low).abs())
                .sum::<f64>()
                / total;
            let minus = pairs
                .windows(2)
                .map(|w| (w[1].candle.low - w[0].candle.high).abs())
                .sum::<f64>()
                / total;
            near(output.series[3].rows[i].values["vi_plus"], plus);
            near(output.series[3].rows[i].values["vi_minus"], minus);
        }
        if i >= 26 {
            let squares = (i - 13..=i)
                .map(|j| {
                    let max = closes[j - 13..=j].iter().copied().reduce(f64::max).unwrap();
                    (100. * (closes[j] - max) / max).powi(2)
                })
                .sum::<f64>();
            near(
                output.series[4].rows[i].values["ulcer"],
                (squares / 14.).sqrt(),
            );
        }
        if i >= 27 {
            let window: Vec<_> = rsi[i - 13..=i].iter().map(|v| v.unwrap()).collect();
            let low = window.iter().copied().reduce(f64::min).unwrap();
            let high = window.iter().copied().reduce(f64::max).unwrap();
            near(
                output.series[2].rows[i].values["stoch_rsi"],
                100. * (rsi[i].unwrap() - low) / (high - low),
            );
        }
        for (series, minimum) in output.series.iter().zip([9, 10, 28, 15, 27]) {
            assert_eq!(
                series.rows[i].status,
                if i + 1 < minimum {
                    Status::WarmingUp
                } else {
                    Status::Ready
                }
            );
            assert_eq!(series.rows[i].available_at_ms, bar.available_at_ms);
        }
    }
}

#[test]
fn every_split_restores_exactly_and_reset_is_fresh() {
    let data = bars(60);
    let expected = calculate(&request(data.clone())).unwrap();
    for (j, id) in IDS.iter().enumerate() {
        for split in 0..=30 {
            let mut stream = Stream::new(identity(), id).unwrap();
            for (i, b) in data[..split].iter().enumerate() {
                assert_eq!(stream.update(b, 10000).unwrap(), expected.series[j].rows[i]);
            }
            let mut restored =
                Stream::restore(&stream.snapshot().unwrap(), &identity(), id).unwrap();
            for (i, b) in data.iter().enumerate().skip(split) {
                assert_eq!(
                    restored.update(b, 10000).unwrap(),
                    expected.series[j].rows[i]
                );
            }
            restored.reset().unwrap();
            assert_eq!(
                restored.snapshot().unwrap().checksum,
                Stream::new(identity(), id)
                    .unwrap()
                    .snapshot()
                    .unwrap()
                    .checksum
            );
        }
    }
}

#[test]
fn undefined_windows_expire_and_constant_states_restore() {
    let mut data = bars(70);
    for b in &mut data[..42] {
        b.candle.open = 100.;
        b.candle.close = 100.;
        b.candle.high = 100.;
        b.candle.low = 100.;
    }
    for id in IDS {
        let mut stream = Stream::new(identity(), id).unwrap();
        for b in &data[..42] {
            stream.update(b, 10000).unwrap();
        }
        assert_eq!(
            stream.latest().unwrap().status,
            if id == "stoch_rsi.14_14" || id == "vortex.14" {
                Status::UndefinedResult
            } else {
                Status::Ready
            }
        );
        let mut restored = Stream::restore(&stream.snapshot().unwrap(), &identity(), id).unwrap();
        for b in &data[42..] {
            assert_eq!(
                stream.update(b, 10000).unwrap(),
                restored.update(b, 10000).unwrap()
            );
        }
        assert_eq!(restored.latest().unwrap().status, Status::Ready);
    }
    // Positive range followed by zero range must not retain a rounding residue.
    let mut s = Stream::new(identity(), "vortex.14").unwrap();
    let mut data = bars(45);
    for b in &mut data[1..] {
        b.candle.open = 100.;
        b.candle.close = 100.;
        b.candle.high = 100.;
        b.candle.low = 100.;
    }
    for (i, b) in data.iter().enumerate() {
        let row = s.update(b, 10000).unwrap();
        if i >= 15 {
            assert_eq!(row.status, Status::UndefinedResult);
        }
    }
}

#[test]
fn future_and_invalid_input_do_not_modify_state_or_history() {
    let data = bars(80);
    let full = calculate(&request(data.clone())).unwrap();
    let prefix = calculate(&request(data[..40].to_vec())).unwrap();
    for (a, b) in full.series.iter().zip(prefix.series) {
        assert_eq!(&a.rows[..40], b.rows);
    }
    for id in IDS {
        let mut stream = Stream::new(identity(), id).unwrap();
        stream.update(&data[0], 10000).unwrap();
        let checksum = stream.snapshot().unwrap().checksum;
        for value in [f64::NAN, f64::INFINITY, -1., 1e101] {
            let mut bad = data[1].clone();
            bad.candle.close = value;
            assert!(stream.update(&bad, 10000).is_err());
        }
        assert!(stream.update(&data[0], 10000).is_err());
        assert!(stream.update(&data[1], 2).is_err());
        assert_eq!(checksum, stream.snapshot().unwrap().checksum);
    }
}

#[test]
fn migrated_constructor_bounds_and_metadata() {
    assert!(wickra::Alma::new(0, 0.85, 6.).is_err());
    assert!(wickra::Alma::new(4097, 0.85, 6.).is_err());
    assert!(wickra::Alma::new(9, 0.85, f64::MAX).is_err());
    assert!(wickra::Alma::new(9, f64::NAN, 6.).is_err());
    assert!(wickra::StochRsi::new(0, 14).is_err());
    assert!(wickra::StochRsi::new(14, 4097).is_err());
    assert!(wickra::Vortex::new(4097).is_err());
    assert!(wickra::UlcerIndex::new(4097).is_err());
    assert!(wickra::SuperTrend::new(10, f64::INFINITY).is_err());
    let profiles = catalog::catalog();
    for (id, n) in IDS.iter().zip([9, 10, 28, 15, 27]) {
        let p = profiles.iter().find(|p| &p.id == id).unwrap();
        assert_eq!(p.minimum_bars, n);
        assert_eq!(p.parameters["formula_variant"], p.formula_variant);
    }
}

#[test]
fn legacy_batch_keeps_undefined_profile_local_and_propagates_cancellation() {
    let mut data = bars(35);
    for b in &mut data {
        b.candle.open = 100.;
        b.candle.close = 100.;
        b.candle.high = 100.;
        b.candle.low = 100.;
    }
    let req = catalog::BatchRequest {
        snapshot_id: "r1-legacy".into(),
        symbol: "TEST".into(),
        timeframe: "1ms".into(),
        as_of_ms: 1000,
        bars: data.into_iter().map(|b| b.candle).collect(),
        profiles: vec!["stoch_rsi.14_14".into(), "alma.9".into()],
    };
    let output = catalog::calculate(&req).unwrap();
    assert_eq!(output.results[0].status, "failed");
    assert!(output.results[0].values.is_empty());
    assert_eq!(
        output.results[0].error.as_deref(),
        Some("undefined_indicator_result")
    );
    assert_eq!(output.results[1].status, "ready");
    assert_eq!(
        catalog::calculate_controlled(&req, || Err("cancelled".into())).unwrap_err(),
        "cancelled"
    );
}
