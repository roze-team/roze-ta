# 组合风险分析 V1

对应 FR-MCP-001/004、FR-EVD-001/003、FR-ENG-003。实现版本 `roze-ta-portfolio-risk-v1`。
入口：Rust `analysis::calculate` / `calculate_controlled`；MCP `analysis_batch_calculate`，
operation.method=`portfolio_risk`；目录 ID=`risk_portfolio`。

## 输入与可知时间

外层 Schema 1，`input_kind=portfolio_simple_return`、`units=ratio`，points/events 留空。
数据放在 spec 中；完整示例见 [请求](../usage/portfolio-request-v1.json)。
每个资产有唯一 id 和有符号权重 `w=线性敞口/组合净资产`，不自动归一化。
净资产 equity 必须正且不超过 1e30；currency 显式给出，全部敞口/收益率必须由调用方转换为同币种。
权重绝对值不超过 100，允许空头和杠杆；这些是计算资源/数值边界，不是交易限额。

观测是资产顺序固定、无缺失的同期简单总收益率向量；调用方保证等间隔交易周期。
at_ms 严格递增且为正，available_at_ms>=at_ms。
只选择 fit_cutoff_ms 前可知的连续前缀；延迟中间点后仍有已知点时拒绝，不跳过、不插值。
未来后缀也执行维度和数值校验，但不参与估计。
权重与情景的 available_at_ms 必须为正且不晚于 fit_cutoff_ms。
收益率和冲击均为有限 ratio，范围 [-1,100]，-1 表示资产价值全部损失。
压力情景必须显式给出所有资产的冲击，没有未指定资产默认零的规则。

这是将给定权重应用于历史协方差的风险估计，并非历史实际持仓回测。
权重无需早于所有历史观测，但必须在分析截止时可知。
外层 selected_points 描述通用 points，因此为零；组合有效样本、范围和 selection_hash 位于该项结果内。
外层 input_hash 仍包含完整 spec、数据、权重、情景；output_hash 覆盖实现与结果。

## 公式、单位与异常

设资产数 m，选中样本数 n，显式 ddof 为 0 或 1，年化周期数 A∈[1,1000000]。

| 输出 | 定义 | 单位 |
|---|---|---|
| gross_exposure_ratio | Σ abs(w_i) | 净资产倍数 |
| net_exposure_ratio | Σ w_i | 净资产倍数 |
| signed_exposure | equity × w_i | currency |
| gross_weight_share | abs(w_i) / gross | ratio |
| concentration_hhi | Σ gross_weight_share_i² | ratio |
| effective_asset_count | 1/HHI | 数量 |
| covariance_per_period | Σ_t(r_ti-mean_i)(r_tj-mean_j)/(n-ddof) | ratio²/期 |
| correlation | cov_ij / (sd_i × sd_j) | ratio |
| annualized_volatility | sqrt(A) × sqrt(w'Σw) | 年化收益波动 ratio |
| volatility_contribution_i | sqrt(A) × w_i × (Σw)_i / sqrt(w'Σw) | 同上；允许负贡献 |
| diversification_ratio | Σ abs(w_i)sd_i / sqrt(w'Σw) | ratio |
| 情景 pnl_by_asset_i | equity × w_i × shock_i | currency |
| 情景 pnl / loss | Σ pnl_i / -pnl | currency；盈利时 loss 可负 |
| 情景 equity_after | equity + pnl | currency；可为负，不截断破产 |

组合方差实现为加权偏差的平方和，减少 w'Σw 相消导致负值的误差。
年化采用平方根时间假设；存在序列相关时不应将其视为精确年化风险。
协方差至少需要两条选中样本，ddof=0 也不例外；不足时返回 InsufficientData，
敞口及压力情景仍可计算。零资产波动使相关系数未定义；零组合波动使风险贡献和
分散化比率未定义；零 gross 使份额、HHI、有效资产数未定义。
这些返回 Scalar 状态，不用零冒充有效比率；零组合波动本身可为有效零。
集中度按输入资产 ID 计算，不自动穿透基金、行业或关联发行人。

## 资源与执行边界

1..16 个资产、最多 4096 条观测、最多 64 个情景，复用全请求 16 operations、
200 万 work units、65536 output values 限制；多个组合操作累计计费。
时间 O(n m² + scenario_count m)，内存 O(n + m² + scenario_count m)。
复用 MCP 1 MiB 请求、8 MiB 响应、2 并发、10 秒协作超时和取消控制。
批量只读、无状态、无隐式 I/O；本版本不提供组合流式快照。

情景是显式假设，不是概率预测。V1 使用固定线性敞口，不覆盖非线性衍生品、
资金费、交易成本、滑点、保证金、强平、流动性或订单生命周期。
VaR/CVaR、回撤、Sharpe/Sortino 继续使用已有 `performance`，要求独立声明的净收益序列；
不把本模块的假设组合收益冒充真实净收益。

账户限额、仓位取整、最小下单量、挂单占用、止损执行、最终放行和熔断仍属于
`risk-execution-service`。本模块没有订单放行标志，不能替代执行前确定性风控。

## 参考与验收

- [MathWorks 风险贡献定义](https://www.mathworks.com/help/finance/portfolioriskcontribution.html)：组合波动风险贡献与权重、协方差的关系。
- [BIS 压力测试原则](https://www.bis.org/bcbs/publ/d450.pdf)：情景分析的治理背景；本实现不声明满足监管压力测试要求。
- 独立手算：收益 [[-.1,0],[0,.1],[.1,-.1]]，权重 [.6,.4]，ddof=1，
  协方差 [[.01,-.005],[-.005,.01]]，组合方差 .0028，HHI .52。
  equity=1000、冲击 [-.2,-.1] 时 pnl=-160，剩余净资产 840。
- `tests/portfolio_risk.rs` 覆盖手算、多空对冲、缺样本、可知时间、维度/参数、
  零波动、损失超过净资产、哈希和协作取消；MCP 覆盖测试通过官方 SDK 调用并比对核心输出。
