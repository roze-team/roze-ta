//! Reproducible release timing smoke, not a cross-platform performance promise.
use roze_ta::{analysis::*, engine::SeriesIdentity};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request = Request {
        schema_version: 1,
        identity: SeriesIdentity {
            series_id: "budget".into(),
            instrument: "TEST".into(),
            timeframe: "1ms".into(),
            source: "fixture".into(),
            data_version: "1".into(),
        },
        input_kind: "synthetic".into(),
        units: "unit".into(),
        as_of_ms: 10_000,
        fit_cutoff_ms: 10_000,
        points: vec![],
        events: vec![],
        operations: vec![Operation::Distribution {
            task: DistributionTask {
                distribution: Distribution::Beta {
                    alpha: 2.,
                    beta: 3.,
                },
                evaluate_at: vec![],
                quantiles: vec![],
                sampling: Some(Sampling {
                    seed: 42,
                    samples: 4096,
                }),
            },
        }],
    };
    let mut checkpoints = 0usize;
    let start = std::time::Instant::now();
    let result = calculate_controlled(&request, || {
        checkpoints += 1;
        Ok(())
    })?;
    println!(
        "{}",
        serde_json::json!({"case":"beta_inverse_sampling_4096","elapsed_ms":start.elapsed().as_secs_f64()*1000.,"checkpoints":checkpoints,"output_hash":result.output_hash,"method_version":result.method_version,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH})
    );
    Ok(())
}
