# 统计和概率调用

契约及参数边界见 [analysis-v1](../contracts/analysis-v1.md)。这是 S1 首批实现，剩余项在实施状态中记录。

原生完整例子：`crates/roze-ta/examples/analysis.rs`。

```powershell
rtk cargo run -p roze-ta --example analysis --locked
rtk cargo run -p roze-ta --features schema --example export_schema --locked
```

MCP 工具 `analysis_batch_calculate` 接收和 Rust `analysis::Request` 相同的 JSON。
例如对 [1,3] 计算样本均值/方差：

```json
{
  "schema_version": 1,
  "identity": {"series_id":"demo","instrument":"TEST","timeframe":"1ms","source":"manual","data_version":"1"},
  "input_kind": "price", "units": "unit", "as_of_ms": 10, "fit_cutoff_ms": 10,
  "points": [
    {"at_ms":1,"available_at_ms":1,"x":1.0,"y":null},
    {"at_ms":2,"available_at_ms":2,"x":3.0,"y":null}
  ],
  "events": [],
  "operations": [{"method":"describe","ddof":1,"quantiles":[0.5],"trim_fraction":0.0,"interval_level":0.95}]
}
```

均值应为 2、样本方差为 2。结果包含完整规格与哈希；MCP structured_content 与原生结果一致。
`distribution` 的显式参数不需要 points，经验分布需要所选 points。
`probability` 要求成熟事件区间，标签格式可参考原生例子；结果包含不可变后验产物。
后续推断用 `{ "method":"infer_beta", "artifact": <原始完整产物> }` 放进 operations，
as_of_ms 不得早于训练截止。结果不会读取请求里的新标签来更新该产物。
复现时保存完整请求、输出、Cargo.lock 及方法版本；新的数据/参数重新拟合会产生新产物哈希。
