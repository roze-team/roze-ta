use roze_ta::{
    catalog::Candle,
    engine::{ClosedBar, SeriesIdentity, Stream},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let identity = SeriesIdentity {
        series_id: "demo".into(),
        instrument: "TEST".into(),
        timeframe: "1m".into(),
        source: "example".into(),
        data_version: "1".into(),
    };
    let mut stream = Stream::new(identity.clone(), "ema.5")?;
    for i in 1..=10 {
        let close = 100.0 + i as f64;
        let bar = ClosedBar {
            candle: Candle {
                closed_at_ms: i * 60_000,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 100.0,
            },
            available_at_ms: i * 60_000,
        };
        stream.update(&bar, i * 60_000)?;
    }
    let snapshot = stream.snapshot()?;
    let restored = Stream::restore(&snapshot, &identity, "ema.5")?;
    assert_eq!(stream.latest(), restored.latest());
    println!("{}", serde_json::to_string_pretty(&restored.latest())?);
    Ok(())
}
