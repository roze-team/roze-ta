//! JSON-lines bridge used by independent offline compatibility oracles.
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line?;
        let result = match serde_json::from_str::<roze_ta::reference_all::Request>(&line) {
            Ok(request) => match roze_ta::reference_all::calculate(&request) {
                Ok(value) => value,
                Err(error) => {
                    serde_json::json!({"error":error.to_string(),"error_code":error.code})
                }
            },
            Err(error) => serde_json::json!({"error":error.to_string()}),
        };
        serde_json::to_writer(&mut stdout, &result)?;
        writeln!(stdout)?;
        stdout.flush()?;
    }
    Ok(())
}
