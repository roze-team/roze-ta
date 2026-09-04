# 概率统计公式实现映射

2026-09-04，依据用户提供的《量化交易概率统计数学公式大全_论文增强版.md》。
[来源 SHA-256](evidence/paper-source.json) 固定本次核对版本。

建立了 **131 个数学章节条目的映射**：128 个公式求值条目、3 个理论说明条目。
同一算法可能对应多个章节，不能将这些条目计作新增算法数。
新增七类分析操作、69 个任务变体，以及四类分布；分析操作合计 21 类。

| 范围 | 入口 | 实现层级 |
|---|---|---|
| 收益、描述统计、相关 | describe / transform / rolling_zscore / pair | 既有 S1 与含现金流收益扩展 |
| 概率、分布、贝叶斯 | distribution / probability / inference | 九类分布、闭式 MLE/正态均值后验、Beta-Binomial |
| 回归、正则化、PCA | regression | OLS/Ridge/Lasso/Logistic/PCA，显式收敛状态 |
| 绩效、交易、尾部风险 | performance / trade_summary / formula | 既有 E1 与期货、成本、执行质量 |
| 回测可信度 | research | Lo/PSR/DSR/MinTRL/PBO/RC/SPA/BH/Bonferroni/DM |
| 组合 | portfolio_risk / allocation | 固定风险、解析优化、风险平价、LW、BL、受限 CVaR |
| 时序 | dynamics / inference | 显式参数递推与有界拟合、Kalman、ADF/EG 校准、Johansen、VECM 多步预测 |
| 随机与期权 | stochastic / formula | Brownian/GBM、Itô、Black–Scholes、Heston Fourier |
| 微观结构 | formula / dynamics | AC、AS、OFI、Hawkes 显式模型求值 |
| 因子与预测诊断 | factor_evaluation / calibration_evaluation / formula | IC/ICIR、方向命中、换手、损失和信息量 |

完整清单：[逐节 CSV](paper-formula-coverage.csv)，含源行号、API、限制和证据分类。
参数、单位、变体、时间、限额与错误见 [契约](contracts/paper-models-v1.md)，
可运行参数见 [72 个请求](usage/paper-requests-v1.json)。

新增[统计工作流](contracts/inference-workflows-v1.md)：ARIMA/GARCH/DCC/Hawkes 显式有界拟合、
VECM 拟合与多步预测、ADF/EG 高斯零假设有限样本模拟校准。
又新增[标准校准](contracts/standard-calibration-v1.md)：MacKinnon 响应面、Johansen 选秩与 Heston 多报价参数校准。
**公式求值覆盖不等于完整模型训练或生产验收。** 尚未提供
任意组合约束优化、交易平台数据库或实盘执行。CSV 对相关条目逐项标注边界。
原 S1/E1/S2A 契约继续适用，不宣称所有概率统计 S2/S3 需求已完成。
