# 统计拟合与预测工作流规格卡

对应来源第 8.5–8.7、8.10、19.5、19.7、19.21 节，以及 FR-ENG-003、FR-MCP-001/004、AC-002–006。
这些实现不代表完整 S2/S3 或生产验收完成。核心版本 `roze-ta-inference-v1.2`；
继续使用 `analysis_batch_calculate` 的 `inference` 操作、`inference_samples` 输入类型。
完整示例见 [72 个请求](../usage/paper-requests-v1.json)，其中四个请求覆盖 `dynamics_fit` 的四种模型。

所有输入是调用方显式冻结的数据，`available_at_ms` 必须不晚于 `fit_cutoff_ms`；
父结果保存 identity、参数、输入/输出哈希和截止时间。样本顺序是时间顺序，不隐式重排、
填补或读取未来数据。预测 horizon 是最后一个输入观测之后的等间隔步数，不自动推断交易日历。

## ADF / Engle–Granger 高斯零假设校准

- `adf_gaussian_calibration`：samples、lags、trend、replicates、seed。
- `engle_granger_gaussian_calibration`：dependent、independent、lags、replicates、seed。
- `replicates=99..4095`，受父 2,000,000 work units 进一步约束。每次模拟可取消。
- 固定长度、滞后及确定项，模拟从零开始、无漂移、IID 标准正态增量的随机游走。
  无确定项 ADF 要求实际首样本为零；有截距时初始水平可由截距吸收。
- EG 每次模拟两个独立随机游走，并重新做含截距的一阶段回归，再做无截距残差 ADF。
- 左尾 `gaussian_null_p_value=(1+count(T_null<=T_observed))/(B+1)`，同时给出分辨率
  `1/(B+1)`；1%、5%、10% 临界值采用模拟样本逆 ECDF（ceil(B*alpha) 次序统计量）。
- 随机数为显式种子的 ChaCha8 / Box–Muller；同环境同输入确定复现。样本/临界统计量无单位。
- 校准只针对所声明的高斯零假设。不是 MacKinnon 响应面、序列相关/异方差稳健 bootstrap，
  也不包括根据同一数据自动选阶后的检验。退化回归或非有限统计量明确失败，不剔除模拟失败样本。

## VECM 条件拟合与预测

`vecm` 接受 observations、lagged_differences、include_constant、rank、steps。
维数 2..4，差分滞后 0..8，rank=0..维数，steps=1..64；样本数及矩阵秩要求沿用 Johansen。
仅支持无常数或协整关系外的无限制常数，暂不支持受限常数、季节项或外生回归变量。

`Δx_t = alpha beta' x_(t-1) + c + Σ Gamma_j Δx_(t-j) + ε_t`。
先估计 Johansen alpha/beta，再将去掉长期校正项的差分对常数及滞后差分做 QR 回归，
得到短期系数。rank=0 令长期项为零，rank=维数等价于对应无约束 VAR。

父结果保留 beta、alpha、根和检验统计量；`vecm` 输出对象包含：

| 字段 | 含义 |
|---|---|
| short_run | Gamma1..Gammap；每个矩阵行是方程、列是解释变量 |
| constant | 各方程截距；无常数时为零 |
| innovation_covariance | 残差交叉积除有效观测数，Gaussian ML 除数 |
| lagged_levels | 最后 p+1 个水平观测，旧到新 |
| forecasts | 1..steps 的条件均值水平预测 |
| forecast_covariances | 等价 VAR 冲击响应累积的预测误差协方差 |
| effective_observations | 排除初始滞后的有效行数 |

预测令未来创新均值为零；协方差仅包含创新不确定性，不包括参数估计、rank 选择或结构突变风险。
均值单位同原序列，协方差为对应变量单位之积；不自动选择 rank 或输出 Johansen 校准 p 值。
非有限预测/矩阵拒绝返回，不能以零填补。完整拟合结果包含在父哈希契约内；
当前接口在一次请求中完成拟合与预测，不提供独立序列化模型恢复工具。

## 有界模型参数拟合

`dynamics_fit` 输入初始 `model`（已有 dynamics 参数对象）、lower、upper、max_iterations、tolerance。
可拟合 ARCH/GARCH、ARIMA、DCC、指数核 Hawkes，最多 12 个参数；上下界有限、严格有序、
包含初值且绝对值不超过 1e12。样本至少参数数+2（ARIMA 按有效残差数）。
iterations=1..1000；归一化步长容差 1e-8..0.01。规模同时受父工作量及输出限额限制。

| 模型 | 参数顺序 | 目标与固定量 |
|---|---|---|
| GARCH | omega, alpha_0.., beta_0.. | 条件高斯负对数似然；残差由调用方去均值；初始冲击与方差冻结；omega>0、alpha/beta>=0、系数和<1 |
| ARIMA | intercept, ar_0.., ma_0.. | 条件残差平方和；p/d/q 固定，MA 初始残差零；截距位于差分尺度；不自动选阶，不保证平稳/可逆 |
| DCC | a, b | Gaussian correlation quasi-likelihood（省略常数项）；标准化残差、Qbar/Q0 冻结；a,b>=0、a+b<1；使用每个观测发生前的相关矩阵 |
| Hawkes | baseline 向量、alpha 行优先、beta 行优先 | 有限观测区间事件负对数似然；初始历史为空；baseline/beta>0、alpha>=0；平稳性只作诊断 |

拟合复用核心 dynamics 递推，MCP 不复制算法。优化使用按用户 bounds 缩放的确定性坐标搜索，
初始归一化步长 0.25；每轮检查所有正负坐标方向，接受最佳严格改善；无改善时步长减半。
`converged` 只表示搜索步长降至容差，不保证全局最优或参数可识别；达到迭代上限返回最佳可行参数及 false。
结果包含初始/最终目标、迭代/评估数、最终步长、命中上下界的参数、拟合后的完整 model 和递推结果。
边界截断不冒充无约束解。初始目标未定义时失败；试探点数值失败视为不可行，但取消/超限错误必须向上传播。

## 验收依据与剩余范围

`inference_workflows`：独立 VAR(2) 正规方程参考与生产 Johansen+QR 的多步预测对比、
rank=0 手算随机游走、ARIMA 截距闭式均值、独立 GARCH 似然递推、DCC 初始相关时序、
Hawkes 事件双重求和参考、模拟重现/变换不变性、超限、取消及边界拒绝。
目录与 MCP 测试执行全部 72 个示例并与核心结果比较。

Heston 多报价校准、Johansen 自动选秩与 MacKinnon 响应面已在[标准校准契约](standard-calibration-v1.md)补齐。
仍缺独立模型恢复工具、概率校准器训练、自动模型选阶/诊断流程及完整跨平台生产验收。

参考：[VECM 官方公式与参数定义](https://www.statsmodels.org/stable/generated/statsmodels.tsa.vector_ar.vecm.VECMResults.html)、
[MacKinnon 模拟检验研究](https://www.econ.queensu.ca/research/working-papers/1027)、
[DCC 原文](https://archive.nyu.edu/handle/2451/26879)、
[Hawkes 综述](https://arxiv.org/abs/1502.04592)。自有 Rust 实现，无新增运行时依赖。
