fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        serde_json::to_string_pretty(&roze_ta::reference_all::catalog()?)?
    );
    Ok(())
}
