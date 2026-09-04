# MCP 接口与能力覆盖

本页对应 FR-MCP-001～004、006 和 AC-006。现有 **7 个工具**，包含 **33 类指标的 45 个固定 Profile、21 类分析操作**，
并通过新增入口开放 **全部 36 个原生指标模块、44 个独立 Method**。别名不重复计数，原生算法与 Profile 有重叠。
全量入口及特殊输出见 [原生 MCP 说明](native-mcp.md)；需求中尚未实现的未来算法不计入已接入能力。
算法已迁入 roze-ta 原生模块，Cargo 不再依赖 `yata`；`vendor/yata` 仅保留为原始审计基线。
派生算法保留 Yata 来源和 Apache-2.0 许可，详见 [原生迁移说明](../patches/yata-native-migration.md)。

## 启动与发现

```powershell
rtk cargo build -p roze-ta-mcp --locked
```

支持 stdio 的 MCP 客户端配置示例，路径按实际仓库位置调整：

```json
{
  "mcpServers": {
    "roze-ta": {
      "command": "D:\\Alion\\roze-ta\\target\\debug\\roze-ta-mcp.exe",
      "args": []
    }
  }
}
```

客户端直接启动二进制；stdout 仅承载 MCP 协议。使用官方 Rust SDK rmcp。
通过 `tools/list` 获取每个工具的完整 `inputSchema`，通过 `indicator_catalog` 获取 Profile、
参数、单位、预热规则、验证状态、分析 `methods` 名称、版本和限额。固定 Profile 的参数不能任意改写。

| 工具 | 能力 | 参数说明 |
| --- | --- | --- |
| `indicator_catalog` | 目录、方法到接口的映射、版本和限额 | 无参数 |
| `indicator_batch_calculate` | 45 个 Profile 的兼容 V1 批量计算 | [V1 说明](../../README.md)，保留旧请求和字符串错误格式 |
| `indicator_batch_calculate_v2` | 45 个 Profile 的 latest / 完整 series、数据身份、可知时间和规范哈希 | [V2 示例](engine-v2.md) |
| `indicator_stream` | 任意已注册 Profile 的创建、续算、查看、重置与快照恢复 | 本页下方 |
| `analysis_batch_calculate` | 下表所有统计、概率、评估和验证操作 | [S1](analysis-v1.md)、[E1](evaluation-v1.md)、[S2A](validation-v1.md) |
| `native_catalog` | 全部原生指标与 Method 的目录、别名、默认参数和参数 Schema | [全量原生说明](native-mcp.md) |
| `native_batch_calculate` | 36 个原生指标和 44 个 Method 的参数化批量调用 | [全量原生说明](native-mcp.md) |

多个指标共享有界批量入口，无需为每个周期建立单独工具。新增 Profile 自动进入目录与计算入口。

| `operations[].method` | 功能 |
| --- | --- |
| `describe` | 描述与稳健统计、分位数、均值区间 |
| `transform` | 简单/对数/累计收益、差分、滞后 |
| `rolling_zscore` | 普通或稳健滚动 Z-score |
| `pair` | Pearson/Spearman、协方差、OLS、Beta、滚动回归 |
| `distribution` | 正态、Student-t、Beta、二项、经验分布；求值/分位数/显式种子采样 |
| `probability` | 成熟条件事件频率、Wilson 区间、Beta 后验拟合 |
| `infer_beta` | 读取冻结 Beta 产物进行推断，不重新拟合 |
| `performance` | 净收益、回撤、风险与年化统计 |
| `trade_summary` | 已平仓交易及显式费用统计 |
| `factor_evaluation` | 成熟标签的截面因子评估 |
| `calibration_evaluation` | 冻结二元预测的 Brier/Log Loss、可靠性分箱等 |
| `bootstrap_mean` | 显式种子的 IID / 移动块均值 Bootstrap |
| `temporal_split` | 按标签结束和可知时间清除跨界样本的时间分区 |

分析方法属于同一请求内的独立计算；`infer_beta` 的产物先由一次 `probability` 调用返回，
再在后续请求中传入 `artifact`。`calibration_evaluation` 是评分，不是校准模型拟合。

## 流式契约 v1

一次请求处理一个 Profile，服务不保存会话；状态由调用方携带。
工具的 `schema_version: 1` 与原生引擎的 `engine_schema_version: 2` 分别版本化。
下面 JSON 是 `tools/call` 中的 `arguments`：

```json
{
  "schema_version": 1,
  "identity": {
    "series_id": "TEST-1m", "instrument": "TEST", "timeframe": "1m",
    "source": "example", "data_version": "1"
  },
  "profile_id": "ema.5",
  "as_of_ms": 120001,
  "action": {
    "kind": "create",
    "output": "series",
    "bars": [
      {
        "candle": {"closed_at_ms": 60000, "open": 100, "high": 102, "low": 99, "close": 101, "volume": 100},
        "available_at_ms": 60001
      }
    ]
  }
}
```

返回 `structuredContent` 中包含 `schema_version`、`engine_schema_version`、`implementation_version`、
`identity`、`profile_id`、`as_of_ms`、`samples_seen`、`latest`、`rows`、`snapshot`、`previous_snapshot_checksum`。

| `action.kind` | 必填字段 | 行为 |
| --- | --- | --- |
| `create` | `bars` | 建立新状态；`bars: []` 可生成空快照；可选 `output` 默认 `latest` |
| `advance` | `snapshot`、非空 `bars` | 恢复状态后消费严格更新的数据；可选 `output` 默认 `latest` |
| `inspect` | `snapshot` | 校验快照，返回其最新值及同一状态快照 |
| `reset` | `snapshot` | 校验后生成同身份、同 Profile 的空状态，`samples_seen: 0` |

续算时保持 `identity` / `profile_id`，把上次响应的完整 `snapshot` 原样放入 `action.snapshot`，
将 `kind` 改成 `advance`，传入后续 bars 和新的 `as_of_ms`。每次响应都返回可用于下一次调用的快照。
重启 MCP 进程后同样可以恢复；相同请求可重放得到相同结果。

`output: series` 的 `rows` 只包含本次新消费的行，`latest` 始终表示整个状态的最新值；
`output: latest` 的 `rows` 最多一行。空状态的 `latest` 为 null，`rows` 为空。
预热与 undefined 状态沿用原生引擎，不填造数值。`inspect/reset` 不接受 bars/output。

快照版本、实现版本、参数与序列身份必须匹配；快照最新可知时间也必须不晚于请求 `as_of_ms`。
所有 bars 先调用原生校验，再开始更新。失败或取消不返回部分状态，也不会改变调用方原快照。
`reset` 只返回一个新值，不删除外部文件或其他调用方的状态。

快照 checksum 用于损坏检测和重放关联，不是签名，也不证明行情来源。
流式响应使用原生快照 checksum 和 measurement 参数哈希；不把它冒充 V2 批量的 input/output 哈希。

## 限额与错误

- stdio 单行请求上限 1 MiB，完整工具响应上限 8 MiB；客户端应缩小批次。
- 全部计算入口共用 2 个并发名额，CPU 工作在 blocking worker；计算期限 10 秒，支持协作式取消。
- 每次最多 4096 根 bar；批量最多 64 个 Profile、65536 行结果。
- 流式输入和输出快照的紧凑 JSON 均不得超过 256 KiB，为后续请求预留空间；仍须满足整帧 1 MiB 上限。
- 分析最多 4096 个点和 4096 个事件、16 个操作、200 万 work units、65536 个输出值；以目录和各分析契约为准。
- 不接收代码、命令、路径或外部数据源。所有工具标记只读，交易与行情采集由业务服务处理。

V2、流式和分析的领域失败返回 `isError: true`，`structuredContent.error` 包含 `code` / `message`，
例如 `invalid_time`、`duplicate_or_unordered_bar`、`corrupt_snapshot`、`incompatible_snapshot`、
`unsupported_profile`、`limit_exceeded`、`cancelled`、`timed_out`。
JSON/参数类型或未知字段错误由 SDK 作为协议参数错误处理；超大 stdio 帧终止当前传输，需要重连。
协议取消由 SDK 处理，客户端不应依赖取消后一定收到工具结果。

新增工具和目录字段保持现有四个工具兼容；未来破坏性变更需新契约版本。
本机 stdio 的验证不代表 HTTP、多租户服务、生产压力或跨平台验收。

验证记录见 [MCP 覆盖证据](../evidence/2026-09-03-mcp-coverage.md)。
原生迁移后的完整回归、真实 stdio 调用和跨进程恢复见 [最新版运行验证](../evidence/2026-09-03-native-mcp-verification.md)。

## 组合风险分析

`analysis_batch_calculate` 包含 `portfolio_risk`，目录 ID 为 `risk_portfolio`。输入包含同币种有符号权重、正净资产、对齐收益率和显式压力情景。可在没有历史样本时计算敞口及情景，协方差等结果明确标为样本不足。见 [公式/单位/时间/限额](../contracts/portfolio-risk-v1.md) 和 [完整请求](portfolio-request-v1.json)。7 个工具名称保持兼容。

## 论文增强公式

通过同一个 `analysis_batch_calculate` 调用 `research`、`formula`、`regression`、`allocation`、`dynamics`、`stochastic`、`inference`。
共 69 个任务变体，目录可查询各组 `tasks`；[示例数组](paper-requests-v1.json) 的每个元素都是一个可独立调用的完整请求。
参数、单位、时间、限额、迭代收敛和未提供的拟合/统计工作流见 [公式契约](../contracts/paper-models-v1.md)。
原生与 MCP 共用实现，无新增工具权限、数据库操作或交易执行。
