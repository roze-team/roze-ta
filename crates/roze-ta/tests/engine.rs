use roze_ta::{
    catalog::{catalog, Candle},
    engine::*,
    error::ErrorCode,
};

fn identity() -> SeriesIdentity {
    SeriesIdentity {
        series_id: "TEST-1m".into(),
        instrument: "TEST".into(),
        timeframe: "1m".into(),
        source: "fixture".into(),
        data_version: "1".into(),
    }
}
fn bars(count: usize) -> Vec<ClosedBar> {
    (1..=count)
        .map(|i| {
            let close = 100.0 + (i as f64 * 0.13).sin() * 7.0 + i as f64 / 23.0;
            ClosedBar {
                candle: Candle {
                    closed_at_ms: i as i64 * 60_000,
                    open: close + 0.1,
                    high: close + 2.0,
                    low: close - 1.0,
                    close,
                    volume: 100.0 + (i % 7) as f64,
                },
                available_at_ms: i as i64 * 60_000 + 2,
            }
        })
        .collect()
}
fn request() -> BatchRequest {
    BatchRequest {
        schema_version: SCHEMA_VERSION,
        snapshot_id: "snapshot-1".into(),
        identity: identity(),
        as_of_ms: 100_000_000,
        bars: bars(320),
        profiles: catalog().into_iter().map(|p| p.id).collect(),
        output: OutputMode::Series,
        require_all_ready: false,
    }
}
#[test]
fn every_profile_streams_batches_and_resumes_bit_exactly() {
    let request = request();
    let batch = calculate(&request).unwrap();
    for series in batch.series {
        for split in [0, 1, 14, 257] {
            let mut original = Stream::new(identity(), &series.profile_id).unwrap();
            for bar in &request.bars[..split] {
                original.update(bar, request.as_of_ms).unwrap();
            }
            let snapshot = original.snapshot().unwrap();
            // Persistence transports only metadata and hex, never floating-point state via JSON.
            let snapshot: Snapshot =
                serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
            let mut restored = Stream::restore(&snapshot, &identity(), &series.profile_id)
                .unwrap_or_else(|e| panic!("{} split {split}: {e}", series.profile_id));
            assert_eq!(original.latest(), restored.latest());
            for (index, bar) in request.bars.iter().enumerate().skip(split) {
                let expected = original.update(bar, request.as_of_ms).unwrap();
                let actual = restored.update(bar, request.as_of_ms).unwrap();
                assert_eq!(
                    actual, expected,
                    "{} split {split} row {index}",
                    series.profile_id
                );
                assert_eq!(actual, series.rows[index]);
                for (key, value) in &actual.values {
                    assert_eq!(value.to_bits(), expected.values[key].to_bits());
                }
            }
        }
    }
}
#[test]
fn independently_calculated_ema_sma_rsi_atr_reference_values() {
    let mut r = request();
    r.profiles = vec![
        "ema.5".into(),
        "sma.5".into(),
        "rsi.14".into(),
        "atr.14".into(),
    ];
    r.bars = bars(32);
    let result = calculate(&r).unwrap();
    let closes: Vec<_> = r.bars.iter().map(|b| b.candle.close).collect();
    let mut ema = closes[0];
    for &close in &closes[1..] {
        ema = ema + (close - ema) / 3.0;
    }
    assert!((result.series[0].rows.last().unwrap().values["ema"] - ema).abs() < 1e-12);
    let sma = closes[closes.len() - 5..].iter().sum::<f64>() / 5.0;
    assert!((result.series[1].rows.last().unwrap().values["sma"] - sma).abs() < 1e-12);
    let first = closes[1] - closes[0];
    let (mut gain, mut loss) = (first.max(0.0), (-first).max(0.0));
    for pair in closes[1..].windows(2) {
        let delta = pair[1] - pair[0];
        gain = (13.0 * gain + delta.max(0.0)) / 14.0;
        loss = (13.0 * loss + (-delta).max(0.0)) / 14.0;
    }
    let rsi = 100.0 * gain / (gain + loss);
    assert!((result.series[2].rows.last().unwrap().values["rsi"] - rsi).abs() < 1e-10);
    let mut atr = r.bars[0].candle.high - r.bars[0].candle.low;
    for pair in r.bars.windows(2) {
        let b = &pair[1].candle;
        let previous = pair[0].candle.close;
        let tr = (b.high - b.low)
            .max((b.high - previous).abs())
            .max((b.low - previous).abs());
        atr = (13.0 * atr + tr) / 14.0;
    }
    assert!((result.series[3].rows.last().unwrap().values["atr"] - atr).abs() < 1e-12);
}
#[test]
fn bad_updates_are_transactional_and_reset_is_fresh() {
    let data = bars(10);
    let mut stream = Stream::new(identity(), "ema.5").unwrap();
    stream.update(&data[0], 100_000_000).unwrap();
    let before = stream.snapshot().unwrap().checksum;
    assert_eq!(
        stream.update(&data[0], 100_000_000).unwrap_err().code,
        ErrorCode::DuplicateOrUnorderedBar
    );
    let mut invalid = data[1].clone();
    invalid.candle.close = f64::NAN;
    assert_eq!(
        stream.update(&invalid, 100_000_000).unwrap_err().code,
        ErrorCode::InvalidBar
    );
    assert_eq!(
        stream
            .update(&data[1], data[1].candle.closed_at_ms)
            .unwrap_err()
            .code,
        ErrorCode::InvalidTime
    );
    assert_eq!(before, stream.snapshot().unwrap().checksum);
    stream.reset().unwrap();
    assert_eq!(stream.samples_seen(), 0);
    assert!(stream.latest().is_none());
    let mut fresh = Stream::new(identity(), "ema.5").unwrap();
    for bar in data {
        assert_eq!(
            stream.update(&bar, 100_000_000).unwrap(),
            fresh.update(&bar, 100_000_000).unwrap()
        );
    }
}
#[test]
fn snapshots_reject_corruption_versions_profiles_and_sequence_mismatch() {
    let mut stream = Stream::new(identity(), "sma.5").unwrap();
    for bar in bars(20) {
        stream.update(&bar, 100_000_000).unwrap();
    }
    let snapshot = stream.snapshot().unwrap();
    let mut bad = snapshot.clone();
    bad.payload_hex.replace_range(10..11, "z");
    assert_eq!(
        Stream::restore(&bad, &identity(), "sma.5")
            .unwrap_err()
            .code,
        ErrorCode::CorruptSnapshot
    );
    let mut bad = snapshot.clone();
    bad.schema_version += 1;
    assert_eq!(
        Stream::restore(&bad, &identity(), "sma.5")
            .unwrap_err()
            .code,
        ErrorCode::IncompatibleSnapshot
    );
    assert_eq!(
        Stream::restore(&snapshot, &identity(), "sma.10")
            .unwrap_err()
            .code,
        ErrorCode::IncompatibleSnapshot
    );
    let mut other = identity();
    other.data_version = "2".into();
    assert_eq!(
        Stream::restore(&snapshot, &other, "sma.5")
            .unwrap_err()
            .code,
        ErrorCode::CorruptSnapshot
    );
    let mut bad = snapshot;
    bad.payload_hex = "0".repeat(MAX_SNAPSHOT_BYTES * 2 + 1);
    assert_eq!(
        Stream::restore(&bad, &identity(), "sma.5")
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
}
#[test]
fn warmup_undefined_and_strict_output_are_explicit() {
    let mut r = request();
    r.profiles = vec!["sma.5".into()];
    r.bars = bars(4);
    r.output = OutputMode::Latest;
    let result = calculate(&r).unwrap();
    let row = &result.series[0].rows[0];
    assert_eq!(row.status, Status::WarmingUp);
    assert!(row.values.is_empty());
    r.require_all_ready = true;
    assert_eq!(calculate(&r).unwrap_err().code, ErrorCode::NotReady);
    r.require_all_ready = false;
    r.profiles = vec!["cmf.default".into()];
    r.bars = bars(300);
    for b in &mut r.bars {
        b.candle.volume = 0.0;
    }
    let result = calculate(&r).unwrap();
    let row = &result.series[0].rows[0];
    assert_eq!(row.status, Status::UndefinedResult);
    assert!(row.values.is_empty());
}
#[test]
fn future_append_does_not_change_past_and_instances_are_isolated() {
    let mut short = request();
    short.bars.truncate(100);
    let mut full = short.clone();
    full.bars = bars(300);
    let a = calculate(&short).unwrap();
    let b = calculate(&full).unwrap();
    for (a, b) in a.series.iter().zip(&b.series) {
        assert_eq!(a.rows, b.rows[..100]);
    }
    let mut x = Stream::new(identity(), "ema.5").unwrap();
    let mut y = x.clone();
    for bar in bars(5) {
        x.update(&bar, 100_000_000).unwrap();
    }
    assert!(y.latest().is_none());
    for bar in bars(5) {
        y.update(&bar, 100_000_000).unwrap();
    }
    assert_eq!(x.latest(), y.latest());
}
#[test]
fn request_limits_identity_availability_and_output_limits_are_enforced() {
    let mut r = request();
    r.bars[1].available_at_ms = r.as_of_ms + 1;
    assert_eq!(calculate(&r).unwrap_err().code, ErrorCode::InvalidTime);
    let mut r = request();
    r.identity.source.clear();
    assert_eq!(calculate(&r).unwrap_err().code, ErrorCode::InvalidIdentity);
    let mut r = request();
    r.profiles = vec!["sma.5".into(), "sma.5".into()];
    assert_eq!(calculate(&r).unwrap_err().code, ErrorCode::DuplicateProfile);
    let mut r = request();
    r.bars = bars(4096);
    assert_eq!(calculate(&r).unwrap_err().code, ErrorCode::LimitExceeded);
    let mut r = request();
    r.profiles = vec!["execute_order".into()];
    assert_eq!(
        calculate(&r).unwrap_err().code,
        ErrorCode::UnsupportedProfile
    );
}
#[test]
fn hash_canonicalizes_signed_zero_and_binds_identity() {
    let mut r = request();
    r.output = OutputMode::Latest;
    r.profiles = vec!["ema.5".into()];
    r.bars[0].candle.volume = 0.0;
    let a = calculate(&r).unwrap();
    r.bars[0].candle.volume = -0.0;
    let b = calculate(&r).unwrap();
    assert_eq!(a.input_hash, b.input_hash);
    assert_eq!(a.output_hash, b.output_hash);
    r.identity.data_version = "2".into();
    assert_ne!(a.output_hash, calculate(&r).unwrap().output_hash);
}

#[test]
fn chaikin_windowless_adapter_matches_independent_accumulator_reference() {
    let mut r = request();
    r.profiles = vec!["chaikin.default".into()];
    r.output = OutputMode::Latest;
    let (mut sum, mut short, mut long) = (0.0, 0.0, 0.0);
    for bar in &mut r.bars {
        bar.candle.open = 12.0;
        bar.candle.high = 14.0;
        bar.candle.low = 10.0;
        bar.candle.close = 13.0;
        bar.candle.volume = 20.0;
        // CLV=(2*13-14-10)/(14-10)=0.5, hence money flow=10.
        sum += 10.0;
        short += (sum - short) * 0.5;
        long += (sum - long) * (2.0 / 11.0);
    }
    let actual = calculate(&r).unwrap().series[0].rows[0].values["chaikin"];
    assert!((actual - (short - long)).abs() < 1e-10);
}

#[test]
fn cancellation_is_cooperative_and_does_not_return_partial_results() {
    use roze_ta::error::TaError;
    let r = request();
    let mut calls = 0;
    let error = calculate_controlled(&r, || {
        calls += 1;
        if calls > r.bars.len() + 12 {
            Err(TaError::new(ErrorCode::Cancelled, "cancelled"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Cancelled);
    assert_eq!(calls, r.bars.len() + 13);
}
#[test]
fn raw_helpers_reject_invalid_data_without_poisoning_or_overflow() {
    assert!(roze_ta::EmaState::new(255).is_none());
    assert!(roze_ta::EmaState::new(256).is_none());
    assert!(roze_ta::ema(&[1.0, f64::NAN, 2.0], 5).is_none());
    assert!(roze_ta::rsi(&[1.0, f64::INFINITY, 2.0], 14).is_none());
    let mut a = roze_ta::EmaState::new(5).unwrap();
    a.next(10.0);
    let mut b = a.clone();
    assert!(a.next(f64::NAN).is_none());
    assert_eq!(a.next(12.0), b.next(12.0));
}
#[test]
fn degenerate_series_can_be_snapshotted_without_losing_nonfinite_internal_bits() {
    for profile in catalog() {
        let mut stream = Stream::new(identity(), &profile.id).unwrap();
        let mut data = bars(270);
        for b in &mut data {
            b.candle.open = 100.0;
            b.candle.high = 100.0;
            b.candle.low = 100.0;
            b.candle.close = 100.0;
            b.candle.volume = 0.0;
        }
        for bar in &data {
            stream.update(bar, 100_000_000).unwrap();
        }
        let snapshot = stream.snapshot().unwrap();
        let mut restored = Stream::restore(&snapshot, &identity(), &profile.id).unwrap();
        let mut next = bars(271).pop().unwrap();
        next.candle.closed_at_ms = 271 * 60_000;
        next.available_at_ms = next.candle.closed_at_ms + 2;
        assert_eq!(
            stream.update(&next, 100_000_000).unwrap(),
            restored.update(&next, 100_000_000).unwrap(),
            "{}",
            profile.id
        );
    }
}
