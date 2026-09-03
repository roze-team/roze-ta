use roze_ta::{analysis, catalog, engine, error};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/contracts/schema");
    std::fs::create_dir_all(&root)?;
    let schemas = [
        (
            "batch-request-v1.json",
            schemars::schema_for!(catalog::BatchRequest),
        ),
        (
            "batch-request-v2.json",
            schemars::schema_for!(engine::BatchRequest),
        ),
        (
            "batch-result-v2.json",
            schemars::schema_for!(engine::BatchResult),
        ),
        ("snapshot-v1.json", schemars::schema_for!(engine::Snapshot)),
        ("error-v2.json", schemars::schema_for!(error::TaError)),
        (
            "analysis-request-v1.json",
            schemars::schema_for!(analysis::Request),
        ),
        (
            "analysis-result-v1.json",
            schemars::schema_for!(analysis::ResultSet),
        ),
        (
            "beta-artifact-v1.json",
            schemars::schema_for!(analysis::BetaArtifact),
        ),
    ];
    for (name, schema) in schemas {
        std::fs::write(
            root.join(name),
            format!("{}\n", serde_json::to_string_pretty(&schema)?),
        )?;
    }
    println!("Exported 8 schemas.");
    Ok(())
}
