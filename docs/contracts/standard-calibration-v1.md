# 标准统计校准与 Heston 参数校准规格卡

对应原文 8.6、8.7、19.7、19.22，FR-ENG-003、FR-MCP-001/004、AC-002–006/009。
本次增加四个任务变体；论文扩展合计 69 个变体、72 个可执行示例。
入口仍是原生 `analysis::calculate` 和现有 `analysis_batch_calculate` MCP 工具。
结果、参数、身份、fit cutoff 继续进入父结果哈希；时间可知性和纯 Rust 无 I/O 边界不变。

## MacKinnon 响应面

| 任务 | 输入 | 校准配置 |
|---|---|---|
| inference.adf_mac_kinnon | samples、lags、trend | N=1；none/constant/linear 三种确定项；临界值样本数为 n−lags−1 |
| inference.engle_granger_mac_kinnon | dependent、independent、lags | 一阶段含截距，残差 ADF 无截距；N=2、constant 响应面；临界值样本数为原 n−1 |

两者沿用 `inference_samples`。先计算现有 ADF/EG tau 统计量，再返回 `mackinnon`：
1994 近似渐近 p 值、2010 有限样本临界值（1%、5%、10%）、采用的 case、样本数及方法版本。
原生另提供 `inference::standard_calibration::calibrate_statistic`，用于明确已知 tau 的纯数值校准。
该函数要求有限统计量和样本数 1..4096；样本数不代表小样本近似的准确性保证。

左/右尾按来源阈值截为 0/1，中间区间使用分段多项式后取标准正态 CDF。
临界值为 c0+c1/n+c2/n²+c3/n³；p 值是渐近近似，不是把有限样本临界值反演得到。
不支持 EG 多解释变量、趋势平方项、断点修正或同样本自动选阶带来的选择偏差校准。
原 Gaussian 零假设模拟校准任务继续保留，二者方法版本与输出明确分开。

## Johansen 自动选秩与预测

`inference.johansen_rank` 输入 observations、lagged_differences、include_constant、
significance、test、forecast_steps。significance 为 ten_percent/five_percent/one_percent；
test 为 trace/max_eigenvalue。forecast_steps 可省略/null，或指定 1..64。

使用既有 Johansen 降秩回归，维数 2..4、差分滞后 0..8，确定项仅无确定项或无限制常数。
按 rank=0,1,.. 顺序比较统计量与对应剩余维数的临界值，严格大于临界值才拒绝；
遇到首次不拒绝即停止，全部拒绝则选择满秩。
返回 `rank_selection` 中的所选秩、实际检验过的秩、统计量、临界值、拒绝标记和表格版本。
alpha/beta 与所选秩一致；指定 forecast_steps 时同时执行该秩的 VECM 拟合和多步预测。

选秩采用 MacKinnon–Haug–Michelis 表格（statsmodels 0.14.6 的无确定项/常数子集），
不提供伪造的连续 Johansen p 值。表格为渐近校准；不保证小样本 rank 正确，
不自动选择差分滞后、外生项或更复杂确定项。预测误差协方差仍不含参数估计和选秩不确定性。

## Heston 多报价参数校准

`stochastic.heston_calibrate` 使用 `stochastic_parameters`。
输入 initial（v0、kappa、theta、xi、rho）、quotes、parameters（活动参数列表）、lower/upper、
max_iterations、fit_tolerance、integration_limit、intervals、pricing_tolerance。

每条 quote 明确 spot、strike、years、rate、dividend_yield、kind(call/put)、price、weight。
价格单位与 spot 一致，时间单位年，方差/均值回复参数遵循既有 Heston 定价契约。
全部报价、利率及参数在 available_at_ms 已知，不能混入 fit cutoff 后的报价。

- 活动参数可选 initial_variance、reversion、long_run_variance、vol_of_variance、correlation，
  1..5 项，不可重复；未激活参数保持 initial 值。边界依活动列表顺序。
- 报价数为活动参数数..16；weight>0，price 满足有限 European 无套利上下界。
- 方差及 xi 下界非负，kappa 下界正，rho 边界在 [-1,1]；所有边界包含初值。
- 目标是 sum(weight*(model_price−quote_price)²)/sum(weight)，同一组参数拟合全部报价。
- 复用既有 Heston 积分/确定方差极限定价。每个被接受的参数点必须通过 pricing_tolerance
  的经验积分收敛检查；初值定价失败则返回数值错误，失败试探点不参与改善。
- 确定性坐标搜索使用归一化边界和初始步长 0.25，接受最佳严格改善；无改善则减半。
  fit_tolerance=1e-8..0.01；iterations=1..1000。达到迭代上限返回当前最优点且 converged=false。
- 返回模型参数、活动参数、初始/最终 weighted MSE、RMSE、拟合价格、残差、积分差异、
  迭代/评估数、步长及命中边界的参数。可将输出模型用于下一次显式校准或现有 Heston 定价。

父工作预算仍为 2,000,000：以每报价 intervals×256，乘最大可能的目标评估次数计费。
预算联合限制报价数、分辨率、活动参数数与迭代数；不能将各字段上限同时当作可运行规模。
例如五个活动参数、五条报价、64 intervals 的两轮搜索已接近限额；大规模曲面拟合需调用方
显式分阶段 warm start，当前接口不会偷偷延长计算或将未收敛标记成成功。
局部收敛不证明全局最优或参数可识别；未强制 Feller 条件，不输出参数置信区间、隐含波动率曲面插值
或真实市场有效性结论。报价权重与定价容差应由调用方按报价精度指定。

## 来源、许可与验证

系数来源固定 statsmodels v0.14.6，见 [原始来源哈希](../evidence/statistical-table-sources.json)。
选取的系数以 BSD-3-Clause 保留在独立 Rust 表格模块中；完整许可证和版权位于
`crates/roze-ta/LICENSE-STATSMODELS` 与 `STATISTICAL-TABLE-NOTICES.md`。
组合 crate 声明 MIT AND Apache-2.0 AND BSD-3-Clause，原 Yata 校验基线与迁移补丁均不变。

独立参考覆盖 24 个 tau/case 组合（Python erfc 与直接幂多项式，p 容差 1e-10、临界值 1e-12）、
Johansen 两种检验/两个显著性等级与预测衔接、Black–Scholes 极限参数恢复、
独立 Riccati ODE 报价恢复非零 xi、五参数多报价加权目标、预算/非法边界/中途取消。
测试与工程门禁见 [验证记录](../evidence/2026-09-04-standard-calibration.md)。

参考：[MacKinnon 系数及方法](https://github.com/statsmodels/statsmodels/blob/v0.14.6/statsmodels/tsa/adfvalues.py)、
[Johansen 临界值表](https://github.com/statsmodels/statsmodels/blob/v0.14.6/statsmodels/tsa/coint_tables.py)、
[statsmodels 许可证](https://github.com/statsmodels/statsmodels/blob/v0.14.6/LICENSE.txt)、
[Heston 特征函数](https://arxiv.org/abs/0902.2154)。
