# 概率统计公式实现映射

2026-09-03。参考用户提供的 `C:\Users\xFc\Downloads\量化交易概率统计数学公式大全.md`。
该文档是公式候选资料，公式须固定参数、单位、数据与时间条件后才能作为接口契约。
以下是能力级范围映射，不宣称整份文档逐公式验收完成。

| 范围 | 当前实现 | 尚待实现/审核 |
| --- | --- | --- |
| 收益和基础统计 | S1 收益/差分、描述统计、分位数、偏峰度、MAD/IQR、滚动 Z-score | 统计流式恢复、训练/推理缩放产物 |
| 概率和基础推断 | S1 五类分布、显式种子抽样、Wilson、Beta-Binomial、IID t 均值区间 | 通用假设检验、校准、多重检验 |
| 线性关系 | S1 Pearson/Spearman、成对 OLS、滚动 Beta | 多元/稳健回归、正则化、模型产物 |
| 策略绩效 | E1 复利/年化、波动、Sharpe、Sortino、IR、TE、最大回撤、持续期、Calmar | 现金流调整、负权益/破产、滚动窗口绩效、PSR/DSR |
| 交易统计 | E1 胜率、净收益期望、平均盈亏、赔率、利润因子、连续盈亏 | 并列平仓时间排序、跨币种转换、重叠仓位净值 |
| 尾部风险 | E1 历史 VaR/ES、显式正态 VaR/ES | EVT、带不确定性的尾部拟合、多周期风险、回测检验 |
| 序列依赖 | E1 指定滞后的自相关、Bartlett HAC 均值标准误/t、近似有效样本量 | ADF/KPSS、AR/ARIMA/GARCH、Kalman、HMM |
| 因子评估与时间验证 | E1 截面 IC/RankIC/ICIR、覆盖率、方向命中率；S2A 显式 rolling/expanding 时间分区及标签清除 | 分层组合收益、换手成本、多空组合、完整 Purged CV、拟合/验证执行器 |
| 组合与资金管理 | 未实施 | 协方差矩阵、组合优化、风险贡献、Kelly 与参数不确定性 |
| 随机模拟 | S1 基础分布抽样；S2A 样本均值 IID/非环绕移动块 Bootstrap、percentile 区间 | 任意统计量/BCa/平稳 Bootstrap、路径 Monte Carlo、随机过程 |
| 衍生品/交易微观结构 | 未实施 | Black-Scholes/Greeks、合约乘数/保证金、资金费率、盘口专用数据 |
| 预测与机器学习诊断 | S2A 冻结概率 Brier/Log Loss、可靠性分桶/ECE、独立基准对照 | Logistic、Platt/isotonic 校准器拟合、可靠性区间、一般损失函数 |

E1 是分析功能增量，和 60 核心指标 +12 形态 +12 结构清单分别计数。
它不等同于全部 S1/S2/S3 验收；已交付范围见 [E1 契约](contracts/evaluation-v1.md)。
S2A 已加入概率评估、时间分区与均值重采样，见 [validation-v1](contracts/validation-v1.md)。
后续补独立校准模型、统计检验与一般重采样，再实现依赖拟合与状态产物的时序模型、组合分析。
所有新增模型均需要独立参考值、数值稳定性、预算/取消与固定 cutoff 证据。
