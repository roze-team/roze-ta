# Reference 指标使用

`reference_catalog` 返回580个入口的构造参数、输入/输出类型和公式规格。参数全部显式提供，示例不是其他库的默认值。

调用 `reference_batch_calculate`：

```json
{
  "schema_version": 1,
  "identity": {"series_id":"demo","instrument":"TEST","timeframe":"1d","source":"caller","data_version":"v1"},
  "operation": {"id":"wickra.Sma","params":{"period":3}},
  "as_of_ms": 4,
  "samples": [
    {"at_ms":1,"available_at_ms":2,"value":2.0},
    {"at_ms":2,"available_at_ms":3,"value":5.0},
    {"at_ms":3,"available_at_ms":4,"value":11.0}
  ]
}
```

最后一行SMA为6，前两行预热。实际时间使用UTC毫秒。

`reference_stream` 的 `action` 为 `create`、`advance`、`inspect`、`reset`。create提供identity/operation；其余动作附上返回的snapshot，advance再提供as_of_ms/samples。客户端保存快照，服务端不维护交易或用户会话。快照包含受限输入历史，恢复时重新校验并重放。

Rust调用 `roze_ta::reference_all::{catalog, calculate, Stream}`；原生算法类型位于 `roze_ta::wickra_all`。低层类型不包含wire层全部时间与资源校验。原50个固定Profile继续使用现有指标工具。

输入与限制见[契约](../contracts/reference-all-v1.md)，跨库兼容状态见[逐项映射](../reference-indicator-mapping.json)。
