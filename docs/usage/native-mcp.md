# 全量原生指标与 Method 的 MCP 接入

当前 **36 个原生指标模块、44 个独立 Method 全部可调用**，包括此前未进入固定 Profile 目录的能力。
每个原生类型都有可发现的 ID；别名映射到同一类型，不重复计数。覆盖清单见 [native-mcp-coverage.csv](../native-mcp-coverage.csv)。
已有 45 个固定 Profile 和 13 类分析操作保留原接口；这些分类有重叠，不能把数量直接相加当成指标家族数。
示例模块 `indicators::example` 仅是教学模板，不计作算法；内部状态类型、迭代器结果类型和 Rust 辅助函数不另注册为算法。

## 两个新工具

| 工具 | 用途 |
| --- | --- |
| `native_catalog` | 返回全部原生条目、别名、默认参数、参数 JSON Schema、输入类型、源码文档和限额 |
| `native_batch_calculate` | 通过 ID 与参数调用全部原生指标或 Method，返回 latest 或完整 series |

加上原有 5 个工具，MCP 现为 **7 个只读工具**。原生入口与 `indicator_catalog` 中的固定 Profile 入口同时存在。
原生 API 是 `roze_ta::native::{catalog, calculate, calculate_controlled}`，MCP 只调用此 API，不维护另一套公式。

## 调用示例

先调用 `native_catalog`。以下内容是 `native_batch_calculate` 的 `arguments`：

```json
{
  "schema_version": 1,
  "identity": {
    "series_id": "demo", "instrument": "TEST", "timeframe": "1ms",
    "source": "manual", "data_version": "1"
  },
  "as_of_ms": 4,
  "data": {
    "kind": "scalars",
    "samples": [
      {"at_ms": 1, "available_at_ms": 2, "value": 100.0},
      {"at_ms": 2, "available_at_ms": 3, "value": 102.0},
      {"at_ms": 3, "available_at_ms": 4, "value": 101.0}
    ]
  },
  "operations": [
    {"id": "method.sma", "params": {"period": 3}},
    {"id": "method.st_dev", "params": {"period": 3}},
    {"id": "method.upper_reversal_signal", "params": {"first": 1, "second": 1}}
  ],
  "output": "series"
}
```

同一个请求中的操作共用输入类型与数据；不同输入类型分开调用。
`params` 可以省略或传 `{}`，以目录中的默认参数为基础覆盖所给字段。未知参数、整数截断、超范围参数被拒绝。
除了 JSON Schema，原生构造器还校验窗口关系、均线组合等约束。出错不会返回部分计算结果。

| `data.kind` | `samples` 元素 | 适用能力 |
| --- | --- | --- |
| `bars` | `{"candle":{"closed_at_ms":1,"open":100,"high":102,"low":99,"close":101,"volume":10},"available_at_ms":2}` | 全部原生 Indicator、TR、ADI、Renko、Heikin-Ashi、CollapseTimeframe、Past |
| `scalars` | `{"at_ms":1,"available_at_ms":2,"value":100.0}` | 均线、偏差、变化率、极值、索引、反转等 Method |
| `pairs` | `{"at_ms":1,"available_at_ms":2,"value":[100.0,10.0]}` | Cross / CrossAbove / CrossUnder、VWMA、Past |
| `values` | `{"at_ms":1,"available_at_ms":2,"value":{"label":"sample"}}` | Past 的结构化 JSON 历史值 |

VWMA 的 pair 是 `(value, weight)`，Cross 的 pair 是被比较的两个值。
`Past` 支持上述四种表示，不能通过 JSON 传入任意 Rust 类型或执行代码。

## 参数与特殊输出

- 普通窗口 Method：`{"period":14}`；窗口上限 254。窗口 0 只在原生实现允许时使用，例如 ADI / Integral。
- Conv：`{"weights":[1.0,2.0,3.0]}`，1～254 项、有限且总和不为零。
- TSI：`{"first":2,"second":3}`，分别按原生 `(period1, period2)` 顺序传递；实际顺序说明见目录原生文档。
- ReversalSignal / UpperReversalSignal / LowerReversalSignal：`first=left`、`second=right`，均为正数且和加一不超过 254。
- Renko：`{"size":0.01,"source":"close"}`。size 是 **相对比例**，范围为 `(0,1)`，不是绝对价格砖块宽度。
- CollapseTimeframe：`{"period":3}`，按输入顺序每 3 根合并；不自动处理交易日历或墙上时钟分桶，尾部不完整组保持 `no_output`。
- 原生 Indicator 使用其配置字段，例如 MACD 可覆盖 `{"signal":{"ema":5}}`。默认参数来自对应 Rust `Default`，而非 MCP 硬编码。

所有字段说明、窗口关系、输出顺序和原生单位位于每项 `documentation` / `parameter_documentation` / `source`。
`Source` 枚举为 close/open/high/low/hl2/tp/volume/volumed_price；组合均线支持原生 MA 的全部 15 种构造类型。

每行 `output.kind` 为 scalar、index、signal、indicator、candle、candles、data 或 pending。
Indicator 的 `values` / `signals` 保留原生数组顺序；信号使用 `analog` 和 `ratio`，它们是规则结果，不是订单。
Renko 可能在一根输入产生多个 `candles`，也可能为空；不完整的周期聚合返回 pending。

## 初始化与时间语义

原生入口严格采用 **`new(first)`，然后对包括 first 的每个样本调用 `next`**，与原生 `over` 一致。
例如原生 SMA 会用首值填充窗口，因此可以在完整窗口前给出数值；不能把它误认为固定 Profile 的正式 `ready`。

响应附带 `native_seeded_output`、`warmup_not_certified`、`signals_are_not_orders`、`timestamps_are_emission_times`。
行状态为 `native_output`、`undefined_result` 或 `no_output`。非有限结果转为 null 并明确标为 undefined，不用零值代替。
固定 Profile 的预热、量纲和公式审核仍沿用既有契约，本次全量原生接入不改写其审核状态。

`at_ms` 和 `available_at_ms` 是当前输出的触发/可知时间。反转信号和合成 K 线不会回填为过去已知的数据；
有绘图位移的指标也保留当前发出时刻，不自动改成图表显示位置。没有新 bar 的输出状态仍保留当前输入时间。
输入必须递增、已完成且在 `as_of_ms` 可知，bar 沿用正价格、有限 OHLC 和非负 volume 校验。

返回结果包含身份、as-of、实现版本、规范 input/output 哈希，以及每个操作的规范 ID、完整参数、参数哈希和 rows。
哈希用于重放一致性，不证明来源真实性。相同请求可复现；更换参数或数据版本改变相应哈希。

## 资源与兼容

- 单次 1～4096 个样本、1～16 个操作；`output` 默认 latest，也可选 series。
- 最多生成 65536 项输出；Renko 单个输入最多 4096 块，超限明确失败，不静默截断。
- `values` 中每个 JSON 值最多 1024 编码字节，限制 Past 初始化时的窗口复制。
- 保留的行 JSON 合计最多 3 MiB；超限建议使用 latest 或缩小批次。
- MCP 继续使用单帧 1 MiB、工具响应 8 MiB、共享 2 个计算名额和 10 秒协作期限；支持取消检查点。
- 原生新入口当前是有界批量计算，不保存服务端会话、不提供原生类型的快照协议。现有 45 个 Profile 的流式快照工具保持可用。
- 原生错误使用既有结构化错误码；原生实现意外 panic 隔离为本次请求失败，不返回部分结果。
- 不引入新依赖、网络取数、文件读写或交易权限；尚未实现的需求算法不会出现在目录中。

目录文档摘自项目维护的 Yata 派生模块，保留 Apache-2.0 来源，见仓库第三方声明。
验收证据见 [全量 MCP 验证](../evidence/2026-09-03-all-native-mcp.md)。
