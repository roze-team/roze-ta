use roze_ta::{
    catalog::{catalog, Candle},
    engine::*,
    error::ErrorCode,
};
const IDS: [&str; 10] = [
    "wma.14",
    "rma.14",
    "dema.14",
    "tema.14",
    "vwma.14",
    "roc.14",
    "momentum.14",
    "ad.cumulative",
    "obv.zero_seed",
    "williams_r.14",
];
fn identity() -> SeriesIdentity {
    SeriesIdentity {
        series_id: "a1".into(),
        instrument: "TEST".into(),
        timeframe: "1s".into(),
        source: "manual_fixture".into(),
        data_version: "1".into(),
    }
}
fn bars(n: usize) -> Vec<ClosedBar> {
    (0..n)
        .map(|i| {
            let close = 100. + ((i * 7) % 19) as f64;
            ClosedBar {
                candle: Candle {
                    closed_at_ms: (i + 1) as i64 * 1000,
                    open: close,
                    high: close + ((i % 3) + 1) as f64,
                    low: close - 2.,
                    close,
                    volume: ((i % 5) + 1) as f64 / 10.,
                },
                available_at_ms: (i + 1) as i64 * 1000 + 2,
            }
        })
        .collect()
}
fn request(n: usize) -> BatchRequest {
    BatchRequest {
        schema_version: 2,
        snapshot_id: "a1".into(),
        identity: identity(),
        as_of_ms: 1_000_000,
        bars: bars(n),
        profiles: IDS.iter().map(|s| (*s).into()).collect(),
        output: OutputMode::Series,
        require_all_ready: false,
    }
}
fn near(actual: f64, expected: f64, id: &str, i: usize) {
    assert!(
        (actual - expected).abs() <= 1e-10 * (1. + expected.abs()),
        "{id} at {i}: {actual} vs {expected}"
    );
}

#[test]
fn ten_families_have_explicit_versions_and_warmups() {
    let profiles = catalog();
    assert_eq!(profiles.len(), 45);
    let families: std::collections::BTreeSet<_> = profiles.iter().map(|p| &p.family_id).collect();
    assert_eq!(families.len(), 33);
    for (id, minimum) in IDS.into_iter().zip([14, 14, 27, 40, 14, 15, 15, 1, 2, 14]) {
        let p = profiles.iter().find(|p| p.id == id).unwrap();
        assert_eq!(p.minimum_bars, minimum);
        assert_eq!(p.parameters["formula_variant"], p.formula_variant);
        assert_eq!(p.capability_kind, "indicator");
        assert!(p.verification_status.starts_with("a1_windows"));
        assert!(!p.warmup_rule.is_empty());
        assert!(Stream::new(identity(), &format!("{}.255", p.family_id)).is_err());
    }
    assert_eq!(
        profiles
            .iter()
            .find(|p| p.id == "trix.default")
            .unwrap()
            .formula_variant,
        "yata_absolute_smoothed_change"
    );
}

#[test]
fn each_new_profile_matches_independent_formulas_at_every_ready_bar() {
    let req = request(90);
    let result = calculate(&req).unwrap();
    let cs: Vec<_> = req.bars.iter().map(|b| b.candle.close).collect();
    let (mut e1, mut e2, mut e3, mut rma) = (cs[0], cs[0], cs[0], cs[0]);
    let (mut ad, mut obv) = (0., 0.);
    for i in 0..cs.len() {
        let b = &req.bars[i].candle;
        e1 += (cs[i] - e1) * 2. / 15.;
        e2 += (e1 - e2) * 2. / 15.;
        e3 += (e2 - e3) * 2. / 15.;
        rma = (13. * rma + cs[i]) / 14.;
        ad += ((b.close - b.low) - (b.high - b.close)) / (b.high - b.low) * b.volume;
        if i > 0 {
            if cs[i] > cs[i - 1] {
                obv += b.volume;
            } else if cs[i] < cs[i - 1] {
                obv -= b.volume;
            }
        }
        let left = (i + 1).saturating_sub(14);
        let window = &req.bars[left..=i];
        let volume: f64 = window.iter().map(|b| b.candle.volume).sum();
        let high = window
            .iter()
            .map(|b| b.candle.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = window
            .iter()
            .map(|b| b.candle.low)
            .fold(f64::INFINITY, f64::min);
        let wma = cs[left..=i]
            .iter()
            .enumerate()
            .map(|(j, x)| (j + 1) as f64 * x)
            .sum::<f64>()
            / 105.;
        let vwma = window
            .iter()
            .map(|b| b.candle.close * b.candle.volume)
            .sum::<f64>()
            / volume;
        let old = cs[i.saturating_sub(14)];
        let references = [
            wma,
            rma,
            2. * e1 - e2,
            3. * e1 - 3. * e2 + e3,
            vwma,
            (cs[i] - old) / old,
            cs[i] - old,
            ad,
            obv,
            -100. * (high - cs[i]) / (high - low),
        ];
        for (series, expected) in result.series.iter().zip(references) {
            let row = &series.rows[i];
            if i + 1 < row.minimum_samples {
                assert_eq!(row.status, Status::WarmingUp);
                assert!(row.values.is_empty());
            } else {
                assert_eq!(row.status, Status::Ready, "{} at {i}", series.profile_id);
                near(
                    *row.values.values().next().unwrap(),
                    expected,
                    &series.profile_id,
                    i,
                );
                assert_eq!(row.available_at_ms, req.bars[i].available_at_ms);
                assert!(!row.quality_flags.iter().any(|f| f.contains("not_audited")));
            }
        }
    }
}

#[test]
fn zeros_ties_and_undefined_windows_recover_after_restore() {
    let mut data = bars(50);
    for b in &mut data {
        b.candle.close = 100.;
        b.candle.open = 100.;
        b.candle.high = 100.;
        b.candle.low = 100.;
        b.candle.volume = 0.;
    }
    for id in IDS {
        let mut s = Stream::new(identity(), id).unwrap();
        for b in &data[..42] {
            s.update(b, 1_000_000).unwrap();
        }
        let row = s.latest().unwrap();
        if matches!(id, "vwma.14" | "williams_r.14") {
            assert_eq!(row.status, Status::UndefinedResult);
        } else {
            assert_eq!(row.status, Status::Ready);
        }
        if matches!(
            id,
            "obv.zero_seed" | "ad.cumulative" | "roc.14" | "momentum.14"
        ) {
            near(*row.values.values().next().unwrap(), 0., id, 42);
        }
        let snap = s.snapshot().unwrap();
        let mut restored = Stream::restore(&snap, &identity(), id).unwrap();
        let mut next = data[42].clone();
        next.candle.high = 102.;
        next.candle.close = 101.;
        next.candle.volume = 10.;
        let expected = s.update(&next, 1_000_000).unwrap();
        let actual = restored.update(&next, 1_000_000).unwrap();
        assert_eq!(expected, actual);
        assert_eq!(actual.status, Status::Ready);
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
    let mut data = bars(30);
    for b in &mut data[1..] {
        b.candle.volume = 0.;
    }
    let mut s = Stream::new(identity(), "vwma.14").unwrap();
    for (i, b) in data.iter().enumerate() {
        let row = s.update(b, 1_000_000).unwrap();
        if i >= 14 {
            assert_eq!(row.status, Status::UndefinedResult);
        }
    }
}

#[test]
fn invalid_updates_and_future_data_cannot_change_a1_history() {
    let req = request(60);
    let full = calculate(&req).unwrap();
    let mut prefix = req.clone();
    prefix.bars.truncate(45);
    let part = calculate(&prefix).unwrap();
    for (whole, partial) in full.series.iter().zip(&part.series) {
        assert_eq!(&whole.rows[..45], partial.rows);
    }
    for id in IDS {
        let mut s = Stream::new(identity(), id).unwrap();
        s.update(&req.bars[0], 1_000_000).unwrap();
        let checksum = s.snapshot().unwrap().checksum;
        assert_eq!(
            s.update(&req.bars[0], 1_000_000).unwrap_err().code,
            ErrorCode::DuplicateOrUnorderedBar
        );
        assert_eq!(
            s.update(&req.bars[1], 1001).unwrap_err().code,
            ErrorCode::InvalidTime
        );
        let mut invalid = req.bars[1].clone();
        invalid.candle.volume = -1.;
        assert!(s.update(&invalid, 1_000_000).is_err());
        assert_eq!(s.snapshot().unwrap().checksum, checksum);
    }
}

#[test]
fn actual_pre_a1_snapshots_keep_old_values_and_layout() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        profile_id: String,
        identity: SeriesIdentity,
        snapshot: Snapshot,
        next_bar: ClosedBar,
        expected: Measurement,
    }
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/pre-a1-snapshots.json")).unwrap();
    assert_eq!(fixtures.len(), 35);
    for f in fixtures {
        let mut s = Stream::restore(&f.snapshot, &f.identity, &f.profile_id).unwrap();
        assert_eq!(
            s.update(&f.next_bar, 1_000_000).unwrap(),
            f.expected,
            "{}",
            f.profile_id
        );
    }
}
