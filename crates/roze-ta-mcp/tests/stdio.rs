//! Exercise the executable boundary, including state recovery after a process restart.
use anyhow::{anyhow, Context};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Client {
    child: Child,
    input: Option<ChildStdin>,
    messages: Receiver<anyhow::Result<Value>>,
    reader: Option<JoinHandle<()>>,
    id: u64,
}

impl Client {
    fn start() -> anyhow::Result<Self> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_roze-ta-mcp"));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = command.spawn()?;
        let input = child.stdin.take();
        let stdout = child.stdout.take().context("missing stdout pipe")?;
        let (sender, messages) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                // Bound the test harness too, so unexpected output cannot allocate indefinitely.
                match stdout.by_ref().take(2 * 1024 * 1024).read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let message = serde_json::from_str(&line).map_err(anyhow::Error::from);
                        let invalid = message.is_err();
                        if sender.send(message).is_err() || invalid {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error.into()));
                        break;
                    }
                }
            }
        });
        let mut client = Self {
            child,
            input,
            messages,
            reader: Some(reader),
            id: 0,
        };
        let initialized = client.request(
            "initialize",
            json!({
                "protocolVersion":"2025-03-26", "capabilities":{},
                "clientInfo":{"name":"roze-ta-stdio-test","version":"1"}
            }),
        )?;
        assert!(initialized["capabilities"]["tools"].is_object());
        client.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))?;
        Ok(client)
    }

    fn send(&mut self, message: Value) -> anyhow::Result<()> {
        let input = self.input.as_mut().context("stdin already closed")?;
        serde_json::to_writer(&mut *input, &message)?;
        input.write_all(b"\n")?;
        input.flush()?;
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.id += 1;
        self.send(json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params}))?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))??;
            assert_eq!(message["jsonrpc"], "2.0");
            if message.get("id").is_none() {
                continue;
            }
            assert_eq!(message["id"], self.id);
            if let Some(error) = message.get("error") {
                return Err(anyhow!("{error}"));
            }
            return message.get("result").cloned().context("missing result");
        }
    }

    fn tool(&mut self, name: &str, arguments: Value) -> anyhow::Result<Value> {
        let result = self.request("tools/call", json!({"name":name,"arguments":arguments}))?;
        assert_eq!(result["isError"], false, "{result}");
        result
            .get("structuredContent")
            .cloned()
            .context("missing structuredContent")
    }

    fn shutdown(mut self) -> anyhow::Result<()> {
        drop(self.input.take());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait()? {
                assert!(status.success(), "server failed on EOF: {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(anyhow!("server did not exit on stdin EOF"));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        drop(self.input.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[test]
fn all_tools_work_over_stdio_and_snapshots_survive_restart() -> anyhow::Result<()> {
    let mut client = Client::start()?;
    let tools = client.request("tools/list", json!({}))?;
    let names: std::collections::BTreeSet<_> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "indicator_catalog",
            "indicator_batch_calculate",
            "indicator_batch_calculate_v2",
            "indicator_stream",
            "analysis_batch_calculate",
            "native_catalog",
            "native_batch_calculate"
        ]
        .into_iter()
        .collect()
    );
    let catalog = client.tool("indicator_catalog", json!({}))?;
    assert_eq!(catalog["profiles"].as_array().unwrap().len(), 45);
    let native_catalog = client.tool("native_catalog", json!({}))?;
    assert_eq!(
        native_catalog["counts"],
        json!({"indicators":36,"methods":44})
    );
    let identity = json!({"series_id":"stdio-test","instrument":"TEST","timeframe":"1ms","source":"fixture","data_version":"1"});
    let bars: Vec<_> = (1..=6)
        .map(|i| {
            json!({"closed_at_ms":i,"open":100.0+i as f64,
        "high":102.0+i as f64,"low":99.0+i as f64,"close":101.0+i as f64,"volume":100.0})
        })
        .collect();
    let closed: Vec<_> = bars
        .iter()
        .map(|bar| json!({"candle":bar,"available_at_ms":bar["closed_at_ms"]}))
        .collect();
    let legacy = json!({"snapshot_id":"stdio","symbol":"TEST","timeframe":"1ms","as_of_ms":10,"bars":bars,"profiles":["sma.5"]});
    let request: roze_ta::catalog::BatchRequest = serde_json::from_value(legacy.clone())?;
    assert_eq!(
        client.tool("indicator_batch_calculate", legacy)?,
        serde_json::to_value(roze_ta::catalog::calculate(&request).map_err(anyhow::Error::msg)?)?
    );
    let batch = json!({"schema_version":2,"snapshot_id":"stdio","identity":identity,"as_of_ms":10,
        "bars":closed,"profiles":["sma.5"],"output":"latest"});
    let request: roze_ta::engine::BatchRequest = serde_json::from_value(batch.clone())?;
    let expected = serde_json::to_value(roze_ta::engine::calculate(&request)?)?;
    assert_eq!(
        client.tool("indicator_batch_calculate_v2", batch)?,
        expected
    );
    let validation: Value = serde_json::from_str(include_str!(
        "../../../docs/usage/validation-request-v1.json"
    ))?;
    let request: roze_ta::analysis::Request = serde_json::from_value(validation.clone())?;
    assert_eq!(
        client.tool("analysis_batch_calculate", validation)?,
        serde_json::to_value(roze_ta::analysis::calculate(&request)?)?
    );
    let native_args = json!({"schema_version":1,"identity":identity,"as_of_ms":10,
        "data":{"kind":"bars","samples":closed},"operations":[{"id":"method.heikin_ashi","params":{}}],"output":"series"});
    let native_request: roze_ta::native::Request = serde_json::from_value(native_args.clone())?;
    assert_eq!(
        client.tool("native_batch_calculate", native_args)?,
        serde_json::to_value(roze_ta::native::calculate(&native_request)?)?
    );
    let first = client.tool(
        "indicator_stream",
        json!({"schema_version":1,"identity":identity,"profile_id":"sma.5",
        "as_of_ms":10,"action":{"kind":"create","bars":&closed[..3]}}),
    )?;
    client.shutdown()?;
    let mut restarted = Client::start()?;
    let resumed = restarted.tool("indicator_stream", json!({"schema_version":1,"identity":identity,"profile_id":"sma.5",
        "as_of_ms":10,"action":{"kind":"advance","snapshot":first["snapshot"],"bars":&closed[3..]}}))?;
    assert_eq!(resumed["latest"], expected["series"][0]["rows"][0]);
    assert_eq!(resumed["samples_seen"], 6);
    restarted.shutdown()?;
    Ok(())
}
