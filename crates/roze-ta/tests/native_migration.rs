use roze_ta::{
    catalog::{catalog, Candle},
    engine::{ClosedBar, Measurement, SeriesIdentity, Snapshot, Stream},
    methods::EMA,
    prelude::Method,
};

#[derive(serde::Deserialize)]
struct Fixture {
    profile_id: String,
    identity: SeriesIdentity,
    snapshot: Snapshot,
    next_bar: ClosedBar,
    expected: Measurement,
}

fn bar(i: i64) -> ClosedBar {
    let close = 100. + (i as f64 * 0.2).sin() * 4. + i as f64 / 100.;
    ClosedBar {
        candle: Candle {
            closed_at_ms: i * 1000,
            open: close,
            high: close + 2.,
            low: close - 1.,
            close,
            volume: 100. + (i % 7) as f64,
        },
        available_at_ms: i * 1000 + 1,
    }
}

#[test]
fn native_engine_preserves_all_pre_migration_snapshots_and_outputs() {
    // Captured using the real external Yata implementation at aa8fba8,
    // before switching dependencies. Never regenerate from the native engine.
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/pre-native-snapshots.json")).unwrap();
    assert_eq!(fixtures.len(), 45);
    assert_eq!(
        fixtures.iter().map(|f| &f.profile_id).collect::<Vec<_>>(),
        // New profiles append after the immutable pre-migration catalog.
        catalog()
            .iter()
            .take(fixtures.len())
            .map(|p| &p.id)
            .collect::<Vec<_>>()
    );
    for f in fixtures {
        let mut native = Stream::new(f.identity.clone(), &f.profile_id).unwrap();
        for i in 1..=280 {
            native.update(&bar(i), 1_000_000).unwrap();
        }
        assert_eq!(
            serde_json::to_value(native.snapshot().unwrap()).unwrap(),
            serde_json::to_value(&f.snapshot).unwrap(),
            "snapshot bytes, checksum and version: {}",
            f.profile_id
        );
        let mut restored = Stream::restore(&f.snapshot, &f.identity, &f.profile_id).unwrap();
        assert_eq!(restored.update(&f.next_bar, 1_000_000).unwrap(), f.expected);
        assert_eq!(native.update(&f.next_bar, 1_000_000).unwrap(), f.expected);
    }
}

#[test]
fn legacy_module_paths_resolve_to_the_same_native_types() {
    let mut native = EMA::new(3, &3.).unwrap();
    let legacy: &mut roze_ta::yata::methods::EMA = &mut native;
    assert_eq!(legacy.next(&9.), 6.);
    assert_eq!(native.next(&12.), 9.);
}
