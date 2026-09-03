# V2 使用方式

MCP 流式创建、续算、查看和重置的参数见 [完整 MCP 接口](mcp.md)；本页示例为 V2 批量工具。

原生可运行示例：

```powershell
rtk cargo run -p roze-ta --example stream --locked
```

Stream::new 接收固定序列身份和目录 Profile。每根已收盘 bar 调用 update；
snapshot() 保存内部状态，restore() 必须给出期望身份与 Profile；reset() 清空已消费状态。

MCP 客户端直接启动构建出的 roze-ta-mcp.exe。新增工具 indicator_batch_calculate_v2 示例参数：

```json
{
  "schema_version": 2,
  "snapshot_id": "demo-snapshot",
  "identity": {
    "series_id": "TEST-1m",
    "instrument": "TEST",
    "timeframe": "1m",
    "source": "example",
    "data_version": "1"
  },
  "as_of_ms": 120001,
  "profiles": ["ema.5"],
  "output": "series",
  "require_all_ready": false,
  "bars": [
    {
      "candle": {"closed_at_ms": 60000, "open": 100, "high": 102, "low": 99, "close": 101, "volume": 100},
      "available_at_ms": 60001
    },
    {
      "candle": {"closed_at_ms": 120000, "open": 101, "high": 103, "low": 100, "close": 102, "volume": 120},
      "available_at_ms": 120001
    }
  ]
}
```

只有两根 bar，因此 ema.5 返回 warming_up 和空 values；补足五根后才返回 ready。
latest 模式只选最后一行，series 返回每根 bar 对应的状态。
V1 工具继续使用原请求格式，不接受上面的 V2 结构。目录工具返回两个版本的信息和运行限额。

