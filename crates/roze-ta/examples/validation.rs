fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request = serde_json::from_str(include_str!(
        "../../../docs/usage/validation-request-v1.json"
    ))?;
    let result = roze_ta::analysis::calculate(&request)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
