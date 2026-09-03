# 概率与统计分析需求

版本：V0.2 需求草案；2026-09-03。

实现进度：已有 S1 基础能力、E1 绩效/风险/因子分析，以及 S2A 概率评估、时间切分和均值重采样；详见
[实施状态](implementation-status.md) 与 [公式实现映射](formula-coverage.md)。以下仍为目标需求。

## 1. 定位

`roze-ta` 除传统指标外，提供可复现的统计分析、概率估计和不确定性量化能力。
目标是支持证据包中的“估计值、适用条件、样本量和不确定性”，而非只提供一个指标分数。
以下为目标需求；当前初始原型尚未实现本专项的大部分能力。

纯 Rust 核心不依赖 Python 运行时。数值算法及随机数依赖在详细设计阶段评审，
优先复用经过测试的 Rust 库；实现来源、算法版本和许可证进入版本清单。

## 2. 与技术指标的关系

```text
行情与对齐序列
  ├─ 技术指标：EMA、RSI、ATR 等
  ├─ 描述统计：分位数、偏度、滚动相关等
  ├─ 统计推断：区间估计、假设检验、回归诊断等
  └─ 概率模型：条件概率、贝叶斯更新、情景模拟等
       → 数值结果 + 数据区间 + 适用条件 + 不确定性 + 版本
       → 证据包 / 原生 API / 只读 MCP
```

计算分为三个类别：确定性变换、需要拟合的统计模型、使用随机数的模拟。
随机算法必须显式提供种子；拟合过程必须与推断分开，并保存训练数据截止时刻。
主指标清单中的 `stat_stddev/stat_zscore/stat_linreg/stat_corr/stat_beta/stat_percentile/stat_skew/stat_kurt`
在本专项复用，不重复计算为新增指标族。

## 3. 统计能力清单

| 编号 | 能力 | 目标内容 | 优先级 |
| --- | --- | --- | --- |
| FR-STAT-001 | 描述统计 | 样本数、均值、中位数、极值、方差、标准差 | S1 |
| FR-STAT-002 | 分布形态 | 分位数、分位排名、偏度、峰度、经验分布 ECDF | S1 |
| FR-STAT-003 | 稳健统计 | MAD、IQR、截尾均值、Winsorize；保留处理前后计数 | S1 |
| FR-STAT-004 | 收益变换 | 简单/对数收益、累计收益、滞后、差分；明确定义及非法输入 | S1 |
| FR-STAT-005 | 标准化 | 滚动 Z-score、稳健 Z-score；常量窗口明确状态 | S1 |
| FR-STAT-006 | 相关与协方差 | Pearson、Spearman、协方差矩阵、滚动 Beta | S1 |
| FR-STAT-007 | 回归 | OLS、滚动回归、斜率、残差、R²；补充 Ridge 和稳健回归 | S1 基础 / S2 扩展 |
| FR-STAT-008 | 时序结构 | ACF、PACF、滞后相关、滚动自相关 | S2 |
| FR-STAT-009 | 区间估计 | 均值、比例、回归参数的区间；明确频率学或贝叶斯解释 | S1 基础 / S2 扩展 |
| FR-STAT-010 | 分布与残差检验 | 正态性、分布拟合、残差自相关；包含 Jarque-Bera、KS、Ljung-Box 等经审核实现 | S2 |
| FR-STAT-011 | 平稳性 | ADF、KPSS，返回趋势项、滞后选择、统计量与 p 值方法 | S2 |
| FR-STAT-012 | 多序列关系 | Engle-Granger 协整、对冲回归、价差残差；禁止用全样本拟合历史信号 | S2 |
| FR-STAT-013 | 多重比较 | Bonferroni、Benjamini-Hochberg 等；保存检验集合 | S2 |
| FR-STAT-014 | 变点检测 | CUSUM、Page-Hinkley、均值/方差变化；分在线检测与离线分段 | S2 |
| FR-STAT-015 | 状态估计 | Kalman 过滤与状态协方差；平滑结果单独标为使用后续数据 | S3 |
| FR-STAT-016 | 模型选择与诊断 | 残差、拟合失败、参数稳定性及 AIC/BIC 的适用条件 | S2 |
| FR-STAT-017 | 统计尾部风险 | 历史/模型 VaR、Expected Shortfall；固定收益或损失符号、分位数及预测期限 | S2 |
| FR-STAT-018 | 时间序列特征 | 波动聚集、趋势持续性、熵等候选；每种估计方法单独评审 | S3 |

“支持检验”必须包含适用条件、样本限制和参考实现验证，不仅返回一个数字。
不同方法及参数可属于同一能力族，不能把每个窗口大小计为新增统计技术。

## 4. 概率能力清单

| 编号 | 能力 | 目标内容 | 优先级 |
| --- | --- | --- | --- |
| FR-PROB-001 | 分布基础 | PDF/PMF、CDF、分位函数、采样；首先支持正态、Student-t、Beta、二项分布与经验分布 | S1 |
| FR-PROB-002 | 经验与条件概率 | 明确事件、条件、观察期、命中数、样本数、有效样本量及区间 | S1 |
| FR-PROB-003 | 概率区间 | 二项比例 Wilson 区间；边界样本与零事件行为 | S1 |
| FR-PROB-004 | 贝叶斯更新 | Beta-Binomial 等可解释共轭模型；显式先验、后验和更新数据 | S1 |
| FR-PROB-005 | 分布拟合 | 参数估计、拟合诊断和失败状态；禁止默认所有收益服从正态 | S2 |
| FR-PROB-006 | Bootstrap | IID、移动块/平稳 Bootstrap；统计量分布和区间 | S2 |
| FR-PROB-007 | 随机化检验 | 置换/重采样策略可配置，保留时间或分组约束 | S2 |
| FR-PROB-008 | 蒙特卡洛 | 基于指定模型/经验数据的情景采样、路径统计、分位数与抽样误差 | S2 |
| FR-PROB-009 | 越界与首达概率 | 给定模型下 H 期内触及阈值、首次到达时间的估计 | S3 |
| FR-PROB-010 | 状态转移 | 离散 Markov 转移计数、平滑和转移概率 | S2 |
| FR-PROB-011 | 隐状态模型 | 可选 HMM，输出过滤状态概率；训练与推断分离 | S3 |
| FR-PROB-012 | 概率校准 | Brier Score、Log Loss、可靠性图数据、校准误差；后续支持独立校准模型 | S2 |
| FR-PROB-013 | 不确定性拆分 | 尽可能区分抽样不确定性、参数不确定性和模型假设，无法量化时明确说明 | S2 |

概率必须在 0～1，分布和阈值需明确单位。不可把 RSI/ADX 值、相似度、模型自报 confidence
或未经校准的分类分数直接命名为“上涨概率”或“交易胜率”。

## 5. 概率任务定义

每个估计请求必须定义：

- `event_definition`：例如“未来 H 根已收盘 bar 的累计收益大于阈值 θ”。
- `conditioning`：使用哪些在 as-of 时刻已知的特征、分桶及过滤规则。
- `horizon`：预测期限；区别期末超过阈值与期间任意时点触达阈值。
- `label_definition_version`：价格源、收益定义、缺失数据、成本口径及标签成熟时间。
- `as_of`、`fit_cutoff`、`data_version`、训练样本范围及样本选择哈希。
- `method`、超参数、先验、随机种子及实现版本。

预测未来收益方向与估计策略净盈利概率是不同任务。后者需要上层提供完整交易标签及成本口径，
不能只用价格上涨次数推导。没有完整定义或有效样本时返回 insufficient_data/unsupported 等明确状态。

## 6. 统计语义与数值约定

1. 区分总体与样本方差，显式 `ddof`；峰度注明原始峰度还是超额峰度。
2. 分位数注明插值算法；相关注明相关类型、收益/价格输入及是否滞后。
3. 成对序列按时间对齐；缺失值处理策略和每对有效样本数写入结果。
4. 标准化、异常值阈值、分桶边界和缩尾阈值只能用训练期或当前历史窗口估计。
5. 结果同时提供估计值、样本量、有效样本量及其估计方法（如适用）。
6. `p_value` 不能解释为原假设为真的概率，也不能替代效应量或预测概率。
7. 频率学置信区间与贝叶斯可信区间使用不同字段/标识，不混用解释。
8. 相关系数不命名为因果效应；协整检验不能被简化为高相关判断。
9. 回归区间应说明误差模型；异方差、自相关修正属于显式选项并有验证。
10. 数学未定义、奇异矩阵、不收敛、自由度不足、数值溢出均返回可区分错误。
11. 年化因子、频率、窗口和市场日历显式配置；不默认所有市场一年相同交易天数。
12. 不在静态库中内置保证通过的显著性或胜率阈值，输出事实供上层策略治理使用。

## 7. 时间序列与样本隔离

- 随机打乱时间序列不能作为默认训练/验证拆分。
- 支持 rolling / expanding / walk-forward 时间切分，记录训练、校准、验证的边界。
- 预测期重叠的标签必须按事件区间排除重叠或使用相应分组/依赖处理；仅按行号留间隔不足以证明隔离。
- 训练数据只能包含在 fit_cutoff 前已成熟、已可知的标签。
- 普通 IID Bootstrap 不得默认为相关收益序列的有效区间方法；须显式选择块采样或说明独立假设。
- Kalman smoothing、离线变点、HMM 全序列平滑结果必须标注后验可用时间；在线证据使用过滤结果。
- 在多个特征、指标、阈值上筛选后报告检验结果时，保留试验集合并支持多重比较处理。

## 8. 拟合、推断与随机模拟接口

建议按职责提供接口族：

```text
transform(series, specification) -> StatisticalSeries
summarize(window, specification) -> StatisticalEstimate
fit(training_dataset, specification) -> FittedArtifact
infer(fitted_artifact, as_of_features) -> ProbabilityEstimate
resample(dataset, specification, seed) -> SamplingEstimate
simulate(fitted_artifact, scenario, budget, seed) -> ScenarioEstimate
evaluate(mature_predictions_and_labels, specification) -> EvaluationReport
```

这些是需求层接口，详细设计后再确定 Rust trait/类型；当前不声明已有同名可调用 API。

模型产物须保存训练截止、参数、算法、数据哈希、状态版本及适用输入 Schema。
统计推断不得隐式重新拟合；调用方要重新拟合必须显式请求并产生新模型版本。

模拟必须约束路径数、步数、采样次数、内存和耗时，并支持取消。返回实际完成样本数、
随机数算法和种子；提前停止不能伪装为完成全部预算。
相同种子在明确限定的平台与算法版本下可复现，不承诺所有平台浮点逐位相同。

## 9. 结果契约

在通用证据字段之外，按结果类型增加：

```text
analysis_kind / method / method_version / assumptions
estimate / effect_size / standard_error
interval_type / interval_level / lower / upper
sample_count / effective_sample_count / degrees_of_freedom
test_statistic / null_hypothesis / alternative / p_value / adjusted_p_value
event_definition / horizon / probability / prior / posterior
fit_cutoff / calibration_period / evaluation_period / model_artifact_hash
rng_algorithm / seed / requested_samples / completed_samples / convergence
```

不同结果使用类型化结构，不能要求无关字段统一填 0；不适用与无法估计要区分。
统计结果不得携带账户下单权限。上层确定性风控可引用版本化估计，但估计本身不直接修改风险限额。

## 10. MCP 与计算成本

目录增加 `capability_kind`：indicator、statistics、probability、simulation、evaluation。
增加所需数据类型、是否拟合、是否随机、预算上限及是否支持在线更新等能力描述。
保留少量受控目录/批量入口，避免为每种算法单独暴露一个工具。

轻量描述统计可同步执行；拟合、大规模重采样和模拟须通过明确的预算与取消机制执行。
原生 API 与 MCP 必须复用同一计算实现和错误类型。远程产物/训练集访问需另行设计权限。

## 11. 专项验收

| 编号 | 验收内容 | 通过条件 |
| --- | --- | --- |
| AC-STAT-001 | 基础统计 | 已知小样本验证均值、方差、分位数、相关及回归；处理常量和缺失输入 |
| AC-STAT-002 | 分布函数 | CDF 单调有界、分位反演、边界参数及独立参考值通过 |
| AC-STAT-003 | 条件概率 | 命中数/样本数/区间可复核；零事件和全事件不产生错误的绝对确定结论 |
| AC-STAT-004 | 贝叶斯更新 | 先验+样本到后验有手工可验样例；不静默重复消费样本 |
| AC-STAT-005 | 统计检验 | 参考数据、自由度、滞后和临界值来源可核对；适用条件不满足时明确报告 |
| AC-STAT-006 | 重采样/模拟 | 固定种子复现、预算与取消有效；估计误差随样本变化有验证 |
| AC-STAT-007 | 前视隔离 | 拟合/校准不读取未来标签；在线结果与截断历史回放一致 |
| AC-STAT-008 | 概率校准 | Brier/Log Loss 与人工样例一致；校准拟合和评估数据分离，包含基准概率对照 |
| AC-STAT-009 | 模型产物 | 数据与参数可追溯；不兼容模型、坏状态和奇异输入拒绝 |
| AC-STAT-010 | 依赖与性能 | Rust 实现与许可证审查完成；独立参考值、边界测试和预算性能通过 |

## 12. 分期交付

- S1（并入 P1）：描述/稳健统计、收益变换、相关与基础回归、基础分布、经验概率、Wilson 区间、Beta-Binomial。
- S2（并入 P2）：统计检验、协整、重采样、蒙特卡洛、Markov、多重比较、概率校准和尾部统计。
- S3（P2 后续）：HMM、状态过滤、首达概率、复杂时序特征；逐项确认模型和数据条件。

与指标 A/B/C 批次并行规划，但不能以增加名称数量替代数值、样本隔离和校准验收。
所有概率与统计功能的当前实现状态都应在后续任务追踪中单独标识。
