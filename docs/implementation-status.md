# 实施状态

## 2026-09-04 组合风险分析

新增 `portfolio_risk`：多资产敞口、HHI/有效资产数、协方差/相关性、年化波动和 Euler 风险贡献、线性压力情景。复用既有分析 MCP；当前 7 个工具、14 类分析操作。原生 36 指标/44 Method 与 Profile 流式接口保持兼容。

[契约与限制](contracts/portfolio-risk-v1.md)，[验证证据](evidence/2026-09-04-portfolio-risk.md)。账户预检、仓位/止损执行、限额与熔断仍未在本库实现。以下是各阶段历史记录。


更新：2026-09-03。需求正文保持目标规格；本页记录实现进度，不宣布整个 P1 已验收。

## 原生算法迁移

Yata 构建依赖已由 `roze-ta` 内部 core/helpers/indicators/methods/prelude 模块替代。
原始 100 文件基线保持不变并排除出 workspace；派生源码保留 Apache-2.0，迁移清单区分原始与派生摘要。
固定 f64/u8、始终启用 serde、删除可选 unsafe 分支；公式、Profile 和快照布局保持兼容。
对应 FR-ENG-006、AC-001/006/007/009，验证见 [原生迁移证据](evidence/2026-09-03-native-migration.md)。
原生低层方法保留既有前置条件与文档化 panic；批量/流式/MCP 仍使用结构化 Result 边界。

## 最新增量：全部原生算法开放 MCP

7 个 MCP 工具：新增原生目录和原生批量入口，全部 36 个原生 Indicator 与 44 个独立 Method 可调用。
支持别名、参数覆盖、数值/双序列/OHLCV/JSON 历史值、规则信号、Heikin-Ashi、Renko 和按根数聚合。
保留原生种子语义、显式未定义与无输出状态、发出时间、参数与输入输出哈希、计算和输出限额。
原生计算入口与原有 45 个 Profile / 13 类分析并存，计数不相加；本次不将算法开放等同于所有正式 Profile 审核完成。
见 [全量说明](usage/native-mcp.md)、[逐项清单](native-mcp-coverage.csv)、[验收证据](evidence/2026-09-03-all-native-mcp.md)。

## 前次增量：MCP 已实现能力覆盖

5 个只读工具覆盖现有 45 个 Profile 和 13 类分析操作；新增无服务端会话的 `indicator_stream`，
复用原生创建、更新、查看、快照恢复与重置。目录新增操作名称映射，参数 Schema 由 `tools/list` 提供。
所有 Profile 和分析操作经过协议与原生结果一致性验证，流式边界与快照限制有拒绝测试。
详见 [MCP 使用说明](usage/mcp.md) 和 [验证证据](evidence/2026-09-03-mcp-coverage.md)。
这不增加指标数量，也不改变上游依赖、公式审计和后续需求的完成状态。

## S2A 验证与重采样

新增冻结二元概率评估（Brier/Log Loss/可靠性分桶/ECE/独立基准）、均值 IID/移动块 Bootstrap，
以及按真实标签结束和可知时间清除跨界样本的训练/校准/验证分区。三类均为只读批量操作。
对应 FR-PROB-006/012/013 与时间隔离要求；不宣称完成校准器拟合、任意统计量 Bootstrap 或完整 S2。
见 [validation-v1 契约](contracts/validation-v1.md)、[S2A 证据](evidence/2026-09-03-validation-s2a.md)。

## E1 分析公式

新增绩效/风险、平仓交易统计、成熟标签截面因子评估三类批量操作，复用只读 analysis_batch_calculate。
明确净收益、年化、MAR、历史 ES 边界权重、HAC 与时间选择；不增加指标族计数。
接口和公式见 [evaluation-v1](contracts/evaluation-v1.md)，范围见 [公式映射](formula-coverage.md)，
验证见 [E1 evidence](evidence/2026-09-03-evaluation-e1.md)。本批不是完整 S2 或全部公式交付。

## 指标 A1

统一目录现为 **33 类 / 45 个 Profile**，新增 WMA、RMA、DEMA、TEMA、VWMA、ROC、Momentum、A/D、OBV、Williams %R。
10 项均有独立公式参考、逐项预热、统一 batch/stream/restore、零量/常量恢复、时间隔离和 MCP 一致性测试。
目录新增 family_id、formula_variant、warmup_rule、verification_status；指标族与不同参数 Profile 分开计数。
旧版真实生成的 35 个快照在新版本继续输出一致，原 Profile 公式与参数哈希保持兼容。

本批是 A 的增量，不代表 40 类 A 批完成。A 批未接入的 7 类为：supertrend、stoch_rsi、true_strength、
relative_vigor、vwap、stat_stddev、stat_zscore。后两项已有 S1 批量基础，但不能算完成指标流式/恢复验收。
旧 19 个复杂默认 Profile 的 256 根规则仍明确标为待逐项审核；后续须逐项用规格和参考证据替换，不能直接批量降低门槛。
Linux 与生产级压力验收尚未完成；B/C 批及 S2/S3 保留原计划。

规格卡：[A1](contracts/expansion-a1.md)；证据：[A1 验证记录](evidence/2026-09-03-expansion-a1.md)；
覆盖：[上游 36 模块映射](upstream-indicator-coverage.csv)、[Method 说明](upstream-method-coverage.md)。
下面 P1/S1 章节保留各阶段历史记录，阶段内的 23/35 指对应交付时点。

工作期间需求已扩展至 V0.2：60 类核心指标、形态/结构及概率统计 A/B/C、S1/S2/S3 批次。
沿用最新需求和已有扩展清单；本轮先交付这些后续能力共用的计算/状态基础，不将扩展批次计为已实现。

## 本轮完成

- 35 个固定 Profile 的统一 Stream/new/update/latest/reset；V1 批量和 V2 latest/series 共用引擎。
- 完整二进制状态快照、实现/参数/序列身份校验、重复消费拒绝；所有 Profile 在多个切点恢复的逐位一致性测试。
- V2 明确预热/数学未定义状态、稳定领域错误、数据版本/source/available-at 与输入校验。
- 版本化规范编码哈希、独立编码的测试向量、-0.0 一致性、追加未来数据不改变已产生行。
- V2 MCP 工具、原生一致性、只读声明、帧/响应/并发限额、协作式取消与计算期限。
- 上游空 Window 恢复问题的外层 Chaikin 适配；上游源码无修改。
- EMA/SMA/RSI/ATR 独立递推参考值；旧 EMA 的 255 周期溢出边界和旧 helper 非有限输入处理修复。

## 验收映射

| 需求/验收 | 当前证据 | 剩余工作 |
| --- | --- | --- |
| FR-CALC-001～004 / AC-001、007 | 所有登记 Profile 批量/流式/恢复测试；快照身份、版本、损坏拒绝 | 跨平台容差、长期兼容/迁移策略 |
| FR-EVD / AC-003、004 | 规范哈希向量、常量零量、非有限/乱序/未来输入、事务性更新 | 市场特定价格域、量类型、缺口/日历、外部时间戳信任 |
| AC-002、005 | 四类基础公式独立参考 + 全部登记 Profile 旧/新数值回归 | 其余公式独立参考与逐 Profile 规格卡；全部上游 36 模块和基础 Method 覆盖审计 |
| AC-006 | 官方 SDK 握手/发现/V1/V2 调用、原生一致性与并发限额测试 | 协议取消/超时的完整黑盒压力验证；Linux smoke |
| AC-009 | 固定上游 tag/commit、100 文件 SHA-256 | 持续 CI 平台矩阵 |
| FR-CALC-005 | 单序列可知时间与追加未来不回写过去 | 多周期/绘图位移/Pivot 确认时间完整对齐 |
| AC-008、010 | 未实施 | roze-quant 兼容接入、release 基准、产物/发布验收 |

复杂默认 Profile 的 256 根预热只是保留原型策略，V2 明示未审核标记。
本轮只覆盖当前登记的 23 类/35 个 Profile，未以方法或不同周期虚增指标类别，也未宣称全部 Yata 模块接入。

## 后续实施顺序

1. 补齐 36 个上游模块及基础 Method 覆盖表、每项规格卡与独立参考样本，逐个替换保守预热限制。
2. 完善量类型、缺口/日历策略及多周期时间对齐。
3. 增加 Linux CI、取消/超时黑盒测试与 release 性能基线，再进入 P2 特征组合和 P3 集成。

## S1 概率与统计首批（2026-09-03）

已增加 `analysis` 核心模块、统一批量 MCP、版本化 Schema 和不可变 Beta 后验产物。
没有增加技术指标族计数；stat_* 能力复用关系在目录中登记。

| 需求 | 本批覆盖 | 未完成范围 |
| --- | --- | --- |
| FR-STAT-001～003 | ddof 描述统计、type 7、ECDF/排名、矩偏度/超额峰度、MAD/IQR、截尾/缩尾计数 | 统计流式快照、独立训练/推断的缩放与缩尾产物 |
| FR-STAT-004～005 | 收益/差分/滞后、历史滚动普通和稳健 Z-score | 市场日历与缺口对齐自动化 |
| FR-STAT-006～007 | Pearson/Spearman、2×2 协方差、OLS/滚动回归与 Beta、残差/R² | 任意维矩阵、回归区间、Ridge/稳健回归、独立回归模型产物 |
| FR-STAT-009 | IID 正态假设的 t 均值区间、Wilson 比例区间 | 回归参数和相关误差修正区间 |
| FR-PROB-001～004 | 五类分布、显式种子采样、条件事件去重/成熟/区间不重叠筛选、Wilson、Beta-Binomial 拟合与只读推断 | 增量去重状态和通用时间切分/验证集隔离器 |
| AC-STAT-001～004、007、009 | 手算/闭式分布参考、标签隔离、重复样本拒绝、产物校验 | 更广泛参考矩阵和跨平台证据 |
| AC-STAT-006、010 | 基础分布固定种子、预算、协作取消、依赖许可与 release timing smoke | 重采样/路径模拟、完整黑盒超时压力、长期基准 |
| AC-STAT-005、008 / S2、S3 | 未实施 | 统计检验、校准、Bootstrap/Monte Carlo、HMM 等 |

规格卡见 [analysis-v1](contracts/analysis-v1.md)，验证记录见
[S1 evidence](evidence/2026-09-03-s1-analysis.md)。以上不构成完整 S1 或 P1 验收完成声明。
