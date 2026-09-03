# 绩效与因子分析用法

运行可复制的合成样本：

```powershell
rtk cargo run -p roze-ta --example evaluation --locked
```

示例读取 [evaluation-request-v1.json](evaluation-request-v1.json)，一次请求包含 performance、trade_summary、
factor_evaluation。为便于手算，样本使用虚构数据与 A=3，不能作为真实市场年化参数。
预期：累计收益 0.1、最大回撤 0.2、Calmar 0.5；交易净额 50 CNY、profit factor 2、胜率 1/3；
两截面 IC 分别 +1/-1、IC 均值 0、coverage=0.75。

原生 Rust：

```rust
let request: roze_ta::analysis::Request = serde_json::from_str(json)?;
let result = roze_ta::analysis::calculate(&request)?;
```

MCP 启动 `target/debug/roze-ta-mcp.exe`，通过 indicator_catalog 获取 eval_performance、eval_trades、eval_factor。
调用 `analysis_batch_calculate`，将示例 JSON 原样用作 arguments。仍然只有四个只读工具。
适配器直接调用相同核心计算，不会从交易账户读取收益，也不会下载行情。

真实数据接入须明确：净简单收益及基准的周期/成本口径、A/rf/MAR、平仓交易金额币种及成本、
固定因子样本池、标签定义/周期、特征可知时间和标签可知时间。只做交易或因子分析时 points 可以为空，
其数据位于 operation.spec；identity、input_kind、units 仍作为此次证据包的元数据提供。
调用方应按 Scalar.status 处理 ready、insufficient_data、undefined；非法请求则返回稳定领域错误。

公式、单位、限制和时间选择见 [契约](../contracts/evaluation-v1.md)；
覆盖与后续公式见 [公式实现映射](../formula-coverage.md)。
