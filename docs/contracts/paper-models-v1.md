# 论文增强公式契约 V1

对应 FR-STAT-006～013、016～018，FR-PROB-001、005～008、013，FR-MCP-001～004、006。
来源文档版本见 [SHA-256 记录](../evidence/paper-source.json)。
核心库新增 research、formulas、regression、allocation、dynamics、stochastic、inference 七个模块，
各有 `roze-ta-…-v1` 实现版本。随机方法固定 ChaCha8 / rand_chacha 0.9.0。

## 调用、时间与范围

入口仍为 `analysis::calculate` / `calculate_controlled` 和只读 MCP `analysis_batch_calculate`。
共有 21 种分析操作，七种新操作内有 **69 个任务变体**，不计作 69 个技术指标。
字段见 [请求 Schema](schema/analysis-request-v1.json)，完整参数见 [72 个示例](../usage/paper-requests-v1.json)。

| method | input_kind | 数据/时间 |
|---|---|---|
| research | excess_simple_return、p_value、loss_difference、candidate_performance、benchmark_loss_advantage | points 或 task.rows；有序数据仅选 cutoff 前可知的连续前缀；多重检验选可知 p 值集合 |
| formula | explicit_formula_parameters | spec.task；spec.available_at_ms 声明整包数据和参数可知时间 |
| regression | regression_matrix | spec.rows 可知前缀；第一列 y，其余 X；PCA 全为特征 |
| allocation | allocation_parameters | 同 formula |
| dynamics | dynamics_parameters | 同 formula；初始状态早于第一条观测 |
| stochastic | stochastic_parameters | 同 formula；路径时间为模型模拟时间 |
| inference | inference_samples | 同 formula；全部样本在声明时间已可知 |

显式参数型操作要求 points/events 为空；research 不接受 events，标量 points 不接受 y。
整包 available_at_ms 必须 >0 且 ≤fit_cutoff；调用方负责外部数据的真实可知时间。
所有数据、参数进入父请求哈希，选样型结果另有 selection_hash。同版本、数据、参数、种子可复现。
不隐式填补或重排数据；参数拟合必须通过显式 `inference.dynamics_fit` 请求。各结果 assumptions 列出当前公式变体与单位。

文档的系统建议、数据表和交易验收章节是设计资料，不是运行数据库、部署或交易的指令。
大数定律/中心极限定理是适用条件，有限样本程序不宣称证明这些命题。

## 研究检验

| task | 输出和约定 |
|---|---|
| sharpe_inference | 单周期 Sharpe、Lo 聚合、PSR、MinTRL、可选 DSR；输入为已扣同周期 rf 的收益，units=ratio |
| multiple_testing | BH step-up / Bonferroni 校正 p 值，原输入次序；alpha 在 (0,1)，units=probability |
| diebold_mariano | x=loss1−loss2；Bartlett HAC、DM 和双侧渐近正态 p；不自动推断 horizon 或作小样本校正 |
| pbo | CSCV 全部平衡组合及补集；mean/Sharpe；最小索引赢 IS 并列，OOS 平均升序秩，logit≤0 计过拟合 |
| reality_check_spa | 联合环形块 bootstrap，White RC 与 Hansen consistent SPA；p=(1+exceedances)/(B+1) |

Sharpe SD 除 n−1，偏度/非超额峰度用中心矩除 n；Lo 自协方差除 n，权重 1−k/q。
q=1..4096，PSR 至少三样本；零方差/不合法渐近方差返回 undefined。
PSR 是 IID 渐近统计量，不是贝叶斯后验；Lo 自相关修正不自动修正 PSR。
MinTRL 在估计 Sharpe≤基准时未定义。DSR 接受完整试验摘要、有效独立次数和原始单周期
Sharpe 均值/样本 SD；有效次数=1 使用均值基准，否则须在 [2,total_trials]，不自动估计独立次数。
BH 需独立或适当正相关条件；Bonferroni 不需该条件。
PBO 2..32 候选，偶数块数 2..10，样本整除非空等长块。
RC/SPA 至少三样本、2..32 候选、99..4096 replicates、显式 seed/block/HAC；
SPA 用固定 Bartlett HAC SD 与 −sqrt(2 log log n) 负均值重心修正。
任一候选长期方差非正时 SPA 未定义，仍可返回 RC；不静默剔除候选。

## 金融、执行和信息公式

| task | 公式、单位或约定 |
|---|---|
| cashflow_return / roll_yield | (price+cashflow)/previous−1；多头 (near−far)/near |
| kelly / continuous_kelly | 二元净赔率 Kelly；(mean−rf)/variance 近似；显式 fraction，保留负值 |
| trade_expectancy | 净期望、含成本盈亏平衡胜率、赔率；盈亏成本同币种，平衡概率可大于 1 |
| futures | 含 costs 的线性净 PnL、abs(entry) 名义价值、近似保证金、权益杠杆；允许负结算价格 |
| stop_sizing | floor(equity*risk_fraction/(abs(entry−stop)*multiplier+cost_per_contract)) |
| carry | 基差 F−S；相对、简单年化和对数年化；连续持有成本理论期价，years 为年 |
| hedge | rho*sigma_spot/sigma_futures，及 exposure/contract_value 缩放的张数 |
| turnover | sum(abs(current−previous)) 与一半的单边换手；不自动校正权重漂移 |
| cost_model | gross−turnover*(commission+half_spread+slippage+impact)/10000；每项 bps 只计一次 |
| execution_quality | 成交率/均价、延迟/交易/机会成本、费用与总 implementation shortfall；买入为正、卖出反向 |
| almgren_chriss | 连续线性临时冲击的库存/速度；风险厌恶=0 使用 TWAP 极限；稳定指数形式 |
| avellaneda_stoikov | 保留价、总点差、bid/ask；gamma=0 点差极限 2/k；不自动 tick 舍入 |
| order_flow_imbalance | 首条报价作基线；同价保留两项数量变化；每事件值及总量 |
| black_scholes | 欧式 call/put、delta/gamma/vega/theta/rho；连续股息；vega/rho 每 1.0 变化，theta 每年 |
| discrete_probability / bayes | 全期望/全方差，二元证据与后验；概率和为 1，零概率证据 undefined |
| losses / information | MSE、MAE、Huber；熵/KL 使用自然对数 nats，p>0 且 q=0 的无限 KL 为 undefined |

formula 标量有限且绝对值≤1e30，数组一般≤4096，权重≤16。
除明确允许零/负的参数外要求正数。执行损失必须满足延迟+交易+机会+费用=总损失；
报价和库存路径是数值模型，不包含订单或账户操作。

## 回归和组合

| task | 方法与约束 |
|---|---|
| regression.ols | 重正交 Modified Gram-Schmidt QR；可选截距；R²/调整 R²、IID 同方差 SE/t 检验；秩不足失败 |
| regression.ridge | SSE+lambda*L2，增广设计 QR，截距不惩罚 |
| regression.lasso | SSE/(2n)+lambda*L1，循环坐标下降；注意 lambda 归一化约定 |
| regression.logistic | 平均 log loss+L2/2，全局 Lipschitz 步长；截距不惩罚；不自动特征缩放 |
| regression.pca | 中心化协方差 n−1、Jacobi 分解；降序特征值/载荷/均值，不自动标准化 |
| allocation.minimum_variance | 完全投资解析最小方差，可选目标收益等式，允许做空 |
| allocation.mean_variance / maximum_sharpe | 无约束 Sigma^−1*mu/delta；或正归一化完全投资切点组合 |
| allocation.risk_parity | 正权重等风险预算，单坐标精确更新；检查 converged |
| allocation.inverse_volatility / target_volatility | 反波动权重；目标/当前波动率并受显式杠杆上限约束 |
| allocation.ledoit_wolf | 中心化 n 分母，缩放单位阵目标的 2004 JMA 估计，不是常数相关目标 |
| allocation.black_litterman | delta*Sigma*w 先验；观点空间线性解；均值不确定性单列；无约束后验权重 |
| allocation.cvar_optimize | Rockafellar–Uryasev 经验 CVaR，long-only unit simplex；投影次梯度；最优已见目标、凸支撑下界与 gap |

回归≤16 特征/4096 行，选样行数多于列数；迭代≤1000、tolerance=1e-12..1e-3。
fitted/residuals 是样本内数据，不是历史样本外预测。组合≤8 资产/观点；协方差优化需 SPD。
CVaR≤4096 次，confidence=[0.5,0.999)；不附加收益、行业等未声明约束。
Lasso/Logistic/风险平价/CVaR 的未收敛结果保留 converged=false，不冒充最优解。

## 时序、随机、推断

| task | 定义与初始化 |
|---|---|
| dynamics.ar1_moments | abs(phi)<1 平稳均值/方差，仅正 phi 有单调半衰期 |
| dynamics.arima | 显式 AR/MA，p/q≤32、d≤2；MA 初始残差零；首 p 个差分值作为历史；一步反差分预测 |
| dynamics.ewma / garch | EWMA 零均值；ARCH 1..32、GARCH 0..32；非负系数和<1，历史 newest-first；每行方差在该行残差之前 |
| dynamics.dcc | 显式标准化残差、Qbar/Qinitial；a,b≥0 且和<1；输出消费该行后的下一期相关矩阵，行优先展开 |
| dynamics.har_rv | 显式 1/5/22 日系数，输入 realized variance；不混淆方差/标准差 |
| dynamics.realized_variance / parkinson | 对数收益平方和；高低价周期方差及年化波动率 |
| dynamics.ornstein_uhlenbeck | 精确条件均值/方差、ln(2)/k 半衰期 |
| dynamics.kalman | 多元预测后更新，Joseph 协方差；状态/观测维数≤8；只过滤不平滑 |
| dynamics.hawkes | 显式指数核，空初始历史；事件前强度、解析积分似然、终点强度；谱半径上下界可保持不确定 |
| stochastic.paths / ito | Brownian/GBM 精确网格转移，seed/steps/paths 显式，终值 Monte Carlo SE；给定导数的 Itô 变换 |
| stochastic.heston | 特征函数 Fourier/Simpson，双网格/双上界检查；xi=0 用精确确定性方差极限 |
| inference.one_sample_t / welch / correlation_test | 单样本、Welch–Satterthwaite、Pearson 零相关的双侧检验 |
| inference.maximum_likelihood | IID normal/lognormal/Poisson/exponential 闭式估计；正态方差除 n；零方差退化边界显式说明 |
| inference.normal_mean_posterior | 已知观测方差、正态均值先验；后验均值/方差/MAP；空样本返回先验 |
| inference.adf / engle_granger | 显式滞后/确定项 ADF；EG 含截距一阶段 OLS、无截距残差 ADF，回归方向明确 |
| inference.johansen | 降秩回归，显式 lagged differences 和可选无限制常数；根、trace/max-root、beta、alpha；rank 由调用方提供 |

新增 `inference.adf_gaussian_calibration`、`engle_granger_gaussian_calibration`、`vecm`、`dynamics_fit`，
分别提供高斯零假设有限样本模拟校准、完整条件 VECM 拟合与多步预测、四种模型有界拟合。
详细假设、参数顺序及限制见[工作流契约](inference-workflows-v1.md)。
已进一步加入 MacKinnon 响应面、Johansen 自动选秩和 Heston 多报价校准，见[标准校准契约](standard-calibration-v1.md)。
这是**公式求值和部分模型拟合已实现、完整统计平台尚未实现**的边界，不宣称所有 S2/S3 需求已验收。
Heston 的积分差异是经验检查而非严格误差上界，须查看 converged。

## 分布、限额和验收

distribution 新增 Bernoulli、Poisson、Exponential、LogNormal，合计九类。
Poisson rate=1e-12..10000，Exponential rate=1e-12..1e12，
LogNormal log_mean±100、log_sd=0.05..10。无界分位端点返回 undefined，Poisson 逆分位有界二分。

父限制不变：≤16 operations、2,000,000 work units、65,536 数值输出；
先校验和计费再计算，超限不截断。维数/迭代数有独立上限，预算可进一步限制样本规模。
非法输入、样本不足、秩不足/数值失败和数学未定义分别用领域错误或 Scalar 状态表达。
各循环有界并响应父取消，MCP 继续执行现有超时/并发/字节限额。

独立测试包含手工样本、标准 Black–Scholes 值、Riccati ODE Heston 参考、2×2 行列式 Johansen 参考，
另有 72 个请求的原生/目录/MCP 一致性测试。见 [证据](../evidence/2026-09-04-paper-models.md)
及 [逐节映射](../paper-formula-coverage.csv)。

## 参考来源

- [DSR 原论文](https://www.davidhbailey.com/dhbpapers/deflated-sharpe.pdf)
- [PBO 原论文](https://www.davidhbailey.com/dhbpapers/backtest-prob.pdf)
- [DCC 原论文](https://archive.nyu.edu/handle/2451/26879)
- [Ledoit–Wolf 缩放单位阵估计](https://www.ledoit.net/Well-conditioned2004.pdf)
- [Rockafellar–Uryasev CVaR](https://sites.math.washington.edu/~rtr/papers/rtr179-CVaR1.pdf)
- [Hawkes 综述](https://arxiv.org/abs/1502.04592)
- [Heston 特征函数分析](https://arxiv.org/abs/0902.2154)
- [arch 官方 SPA 重心修正实现](https://bashtage.github.io/arch/_modules/arch/bootstrap/multiple_comparison.html)

自有 Rust 公式实现；参考资料用于数学核对，未修改 Yata 原始/派生审计文件。
