# E1 绩效、风险与因子评估契约

版本：`roze-ta-evaluation-v1`；2026-09-03。纯 Rust、调用方供数、只读批量分析。
复用 [analysis-v1](analysis-v1.md) 的 Request、Scalar、领域错误、时间选择与规范哈希。
这是三个新增 operation；外层 schema_version=1 与 S1 方法版本保持兼容，每个 E1 输出另带 method_version。
不修改旧 Beta 产物、指标 Profile 或 Yata 公式。E1 不增加 60+12+12 指标扩展清单的计数。

## 输入、时间与预算

- 时间为正整数毫秒，`0 < fit_cutoff_ms <= as_of_ms`；仅使用 cutoff 前已可知的数据。
- 单次最多 16 operation；外层 points/events、单个 trade/factor 数据集分别最多 4096 条。
- 所有数值必须有限且绝对值不超过 1e50；ddof 只能为 0 或 1。文本字段为 1..1024 UTF-8 字节。
- 共用 2,000,000 work units / 65,536 output values；performance 预算 `n*(32+lags)`，
  trade `16*n`，factor `64*n`。超限返回 limit_exceeded。循环提供协作取消检查点。
- MCP 共用两路计算并发、1 MiB 请求、8 MiB 响应、10 秒计算期限；算法只在核心库实现。
- 不从交易日历推断周期、不补齐缺失数据；金融序列身份及可知时间由调用方保证。

## eval_performance / method=performance

数据：外层 points.x 为已经扣除成本的单周期简单收益，`input_kind=simple_return`、`units=ratio`。
`return_basis=net_of_costs`、`spacing=consecutive_trading_periods` 必填。有效收益必须 > -1；
破产、负权益和外部现金流调整不属于此模型。至少两个已选择样本；不设指标式预热或隐藏 256 根阈值。
选择结果必须为原始有序序列的可知前缀，不能跳过延迟披露的内部收益后继续复利。

参数：`periods_per_year=A` 为 1..1e6，不默认 252；`risk_free_per_period=f` 与 `mar_per_period=M`
均为与收益同周期的简单收益率且 > -1。`tail_level=c` 为 [0.5,0.999]；`include_gaussian_tail` 必填；
`hac_lags=L` 为 0..128 且 L<n。可选 benchmark_id 与每条已选择 points.y 必须同时提供或同时缺省；
y 为已对齐、同口径、同周期的基准简单收益 > -1。不会按名字下载或对齐基准。

记 m=mean(r)、s²=sum((r-m)²)/(n-ddof)，财富初值 W0=1。

| 输出 | 公式与单位 |
| --- | --- |
| arithmetic_mean_return | m，ratio |
| cumulative_return | product(1+r)-1，以 sum(log1p(r)) 计算，ratio |
| annualized_return | exp(sum(log1p(r))*A/n)-1，ratio |
| annualized_volatility | s*sqrt(A)，ratio |
| annualized_sharpe | (m-f)*sqrt(A)/s，无量纲 |
| annualized_sortino | (m-M)*sqrt(A)/sqrt(mean(min(r-M,0)²))，无量纲 |
| annualized_tracking_error | std(r-y,ddof)*sqrt(A)，ratio |
| annualized_information_ratio | mean(r-y)*sqrt(A)/std(r-y,ddof)，无量纲 |
| maximum_drawdown | max(1-Wt/max(W0..Wt))，正的损失比例 |
| maximum_underwater_periods | 从最近峰值到尚未恢复的采样点之间，最长连续水下样本数 |
| calmar | annualized_return/maximum_drawdown，无量纲 |

权益曲线的每行可知时间是所有依赖收益可知时间的最大值。财富和峰值用对数存储；
当 log_equity 距 log_peak 不超过 `8*EPSILON*(1+abs(log_equity)+abs(log_peak))`，
按数值恢复到峰值处理、回撤置零，避免数学上恢复却因舍入仍被算为水下。
时间长度按采样周期计数，不解释为自然日。累计/年化溢出返回 Scalar undefined；其他可计算风险量仍返回。
零波动、零下行偏差、零跟踪误差、零回撤分别返回有原因的 undefined，不输出无穷大。
Sortino 分子也使用 MAR；若参考文档以 rf 为分子、MAR 为分母，本实现明确采用另一种口径。
sqrt(A) 年化具有平稳性和相关结构假设；它并未自动被下述 HAC 替换。

### VaR / ES

损失 l=-r，以单周期净简单收益的负值定义；不将负风险估计强行截为零。
历史 VaR 是升序损失的第 ceil(c*n) 个值（从 1 计数，经验逆 CDF）。
ES 是最高 `(1-c)*n` 个样本质量的加权均值：完整样本权重为 1，边界样本只计剩余质量。
例如 r=[0.1,-0.2,0.25]、c=0.5：VaR=-0.1，ES=(0.2-0.1*0.5)/1.5=0.1。
该变体等价于对经验分位数的上尾积分，不能换成简单 `loss >= VaR` 平均。

显式请求 Gaussian 时另返回 z=Phi_inverse(c)、VaR=-m+s*z、ES=-m+s*phi(z)/(1-c)。
该输出不证明收益正态，不外推多周期，不计算未知尾部置信区间；历史估计很小样本时也必须保留样本数。

### HAC 均值诊断

gamma_k=sum((r_t-m)*(r_(t-k)-m))/n；rho_k=gamma_k/gamma_0。
Bartlett 权重 w_k=1-k/(L+1)；长期方差 V=gamma_0+2*sum(w_k*gamma_k)。
输出均值超额收益标准误 sqrt(V/n)、t=(m-f)/SE、有效样本数 n_eff=n*gamma_0/V。
固定分母 n，无小样本修正，无自动选阶。V=0 时 SE=0、t/n_eff 未定义；V<0 时全部未定义。
负序列相关时 n_eff 可能大于 n，不能硬截成 n；它是均值精度近似，不是新增观测。
不返回未经校准的 p 值或“显著”结论。方法定义参考
[statsmodels HAC 文档](https://www.statsmodels.org/dev/generated/statsmodels.stats.sandwich_covariance.cov_hac.html)，
本项目只实现此处指定的均值模型和归一化规则。

## eval_trades / method=trade_summary

每条 Trade 提供唯一 id、严格递增 closed_at_ms、available_at_ms>=closed_at_ms、gross_pnl 与非负 costs。
currency 与 cost_definition 必填；同币种金额才可求和。时间并列的多笔成交须由上层先按交易定义归并；
此版本不擅自决定并列顺序。至少一笔已可知的平仓交易；可知选择必须为前缀，避免未知中间交易造成虚假连胜。

net=gross-costs；net>0 为赢、net<0 为输、net=0 为平。输出交易数、赢输平计数、毛收益/成本/净收益总额、
胜率、平均净收益、平均盈利、平均亏损绝对值、payoff_ratio、profit_factor、最长连赢/连输。
胜率=赢/n，平局计入分母并中断连胜/连败。payoff=平均盈利/平均亏损绝对值；
profit_factor=正净收益总额/负净收益绝对值总额。缺少某类样本时平均值或 payoff 为 insufficient_data；
没有亏损时 profit_factor 为 undefined(no_losing_trades)，不输出无限利润因子。
金额单位为 currency，比例无量纲，连胜败为交易笔数。成本仅扣一次；不从重叠持仓的交易记录推导资金净值。
输出 selected_ids/range/hash 与未可知排除数，用于审计此次选择。

## eval_factor / method=factor_evaluation

每条记录给出 asset_id、at_ms、factor_available_at_ms、label_end_ms、label_available_at_ms、score、future_return。
要求 factor_available_at_ms<=at_ms，label_end_ms=at_ms+horizon_ms（检查整数溢出），label_available_at_ms>=label_end_ms。
同资产/at 不得重复。factor_id、universe_id、label_definition 必填；horizon_ms>0；
`2<=minimum_cross_section<=universe_size<=4096`。universe 为调用方声明的固定样本池大小。
score 单位由因子定义说明，future_return 由 label_definition 定义；IC 对线性尺度不敏感。

按 at 分组、资产 ID 排序。任一已提供标签未成熟则整组排除，禁止只保留当时已成熟的资产。
成熟标签缺失返回 invalid_sample；不足 minimum_cross_section 的组计数后排除。
按时间先后选取互不重叠的标签区间 [at,label_end)；后一组 at<前组 end 时排除并计数。
单个截面不得超过声明 universe_size。不存在可用截面时返回 insufficient_data。

- IC：同一截面 score 与 forward label 的 Pearson correlation，范围 [-1,1]。
- RankIC：双方平均秩后 Pearson，同值共享平均秩；不把多个时点混成时间序列相关。
- coverage=组内记录数/universe_size；不能验证未提供资产的真实成员资格或幸存者偏差。
- direction_hit_rate：排除 score=0 或 label=0 后同符号比例，另报有效/零方向计数；它是描述性命中率。
- 汇总分别对有效 IC/RankIC 求均值、ddof 标准差及 ICIR=mean/std，**不年化**；保留 valid_groups。
  常量截面的相关为 undefined；汇总缺少自由度为 insufficient_data；零 IC 标准差时 ICIR undefined。
- 输出每行 at/available、各类排除组数、规范 selection_hash。这是固定 cutoff 的可重复批量评估；
  变更 cutoff 会重新进行成熟及不重叠选择，不维护或回写已发布历史结果。

## 复现、边界与实现状态

外层 input_hash/specifications 保留完整请求（包括调用方提供的未来记录）；output_hash 覆盖整个响应，
所以追加未来记录可改变请求/响应哈希与排除计数。外层 selection_hash 只指 points；
trade/factor 自己的 selection_hash 指实际使用的交易/截面。固定 cutoff 未选中数值不参与分析结果。
as_of 校验不能证明调用方的历史数据和时间戳真实，也不能检测标签计算内部的泄漏。

三类能力均 `online_update=false`，无拟合产物、随机性或持久化状态；复现方式为重放同一规范请求。
不宣称完成流式状态恢复或全部 S2。现有 S1 和指标快照仍走原路径并由全量回归检查。
参考值、常量、尾部边界、短样本、重复、延迟可知、标签隔离、预算/取消见 `tests/evaluation.rs`；
官方 MCP SDK 与原生结果逐字段一致性见 MCP 的 evaluation 测试。
