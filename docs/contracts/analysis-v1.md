# 概率与统计 S1 契约 v1

状态：首批实现，2026-09-03。对应 FR-STAT-001～009 基础部分、FR-PROB-001～004。
Rust API 为 `roze_ta::analysis::{calculate, calculate_controlled, infer_beta}`；
MCP 为 `analysis_batch_calculate`。共享公式、校验、错误类型和版本。
后续新增的七类操作、九类分布当前总范围与边界见 [论文公式契约](paper-models-v1.md)。
JSON Schema 见 `schema/analysis-request-v1.json`、`analysis-result-v1.json`、`beta-artifact-v1.json`。

## 数据、时间及哈希

请求明确 `schema_version=1`、SeriesIdentity、input_kind、units、as_of_ms、fit_cutoff_ms、points、events、operations。
要求 `0 < fit_cutoff <= as_of`。核心无 I/O，不读系统时间、不抓行情、不推测日历或年化因子。
points 按正的 at_ms 严格递增；available_at_ms >= at_ms。x/y 有限且绝对值 <= 1e50。
只选 available_at_ms <= fit_cutoff 的行；未来行保留在完整 input_hash 中，但不影响所选统计结果。
缺失值不填充；Pair 要求每个所选行都有 y。同一 Point 的 x/y 由调用方明确按时间配对，
available_at_ms 必须是两者均可知的时刻。输入含价格还是收益由 input_kind 声明，不自动转换。

结果包含身份、截止、所选点数和范围、selection_hash、完整规格和方法版本。
input_hash 使用现有 canonical-v1 编码；output_hash 对将自身字段设为空字符串的完整结果计算。
概率结果另外记录事件所选 ID、范围和 selection_hash。
哈希可核对数据/参数和意外损坏，不证明来源真实，不提供签名或防篡改认证。

## 方法规格卡

| method | 定义与参数 | 最少样本、单位及特殊行为 |
| --- | --- | --- |
| describe | ddof=0/1，方差为中心平方和/(n-ddof)，Welford 累计；quantiles 最多 64 个 [0,1]；trim_fraction ∈ [0,0.5)；interval_level ∈ [0.5,0.999] | 至少 1 行；方差自由度不足为 insufficient_data；标准差同输入单位、方差为平方单位 |
| describe 形态 | R type 7 线性插值；ECDF 返回唯一值及 <= 的比例，兼作分位排名；skew 为总体中心三阶矩/σ³，kurt 为总体中心四阶矩/σ⁴−3 | 非偏差修正估计；常量序列形态为 undefined；分位概率/偏度/峰度无量纲 |
| describe 稳健 | MAD 为未缩放绝对中位差；IQR=Q75−Q25；截尾丢弃两端各 floor(n×trim_fraction)；Winsorize 按 type 7 的 Q(trim)、Q(1−trim) 夹取，保持原时间顺序 | 返回处理前 n、截尾后 n 和缩尾后 n；均值、MAD、IQR 同输入单位 |
| describe 均值区间 | 双侧 Student-t，df=n−1，SE=s(ddof=1)/√n，不随描述方差的 ddof 改变 | 至少 2 行；明确 IID 正态误差假设，无时间依赖修正；frequentist_confidence 字段 |
| transform | kind=lag/difference/simple_return/log_return/cumulative_return；lag 为 1..4096 个所选观测 | lag/diff/普通收益前 lag 行 insufficient_data；收益输入严格正；simple=x/x_lag−1，log=ln(x)−ln(x_lag)，cumulative=x/x_first−1 且只允许 lag=1，首行为 0 |
| rolling_zscore | window=2..4096，包含当前行的历史窗口；普通 Z=(x−mean)/std(ddof)；robust=true 时 (x−median)/(1.482602218505602×MAD)，要求 ddof=0 | 未满窗口 insufficient_data；零标准差/MAD 为 undefined；输出无量纲，每行含依赖的最晚 available_at |
| pair | ddof=0/1；window=null 为整段，或 2..4096 的历史滚动；Pearson；平均秩处理 ties 的 Spearman；2×2 covariance，顺序 x,y；OLS y=intercept+beta×x | 至少 2 行；滚动未满窗口 insufficient_data；常量 x 回归奇异；常量 x/y 相关和 R² undefined；beta=cov(x,y)/var(x)；无隐式滞后 |
| pair 回归 | 返回斜率、截距、R² 和残差；完整回归返回每行残差，滚动返回窗口最后一行残差；自由度 n−2 | 描述性拟合，非预测模型产物；beta 单位 y/x，截距/残差同 y；协方差为乘积单位；相关/R² 无量纲 |

窗口按所选观测数计数；缺口不被视为固定时间间隔，调用方应明确清洗/对齐规则。
整段回归残差属于 fit_cutoff 可知的拟合诊断，不能将它们当作当时已知的历史交易信号。
标准化与 Winsorize 阈值只使用所选训练期或当前历史窗口。
空 transform 返回空序列，空 describe/pair/empirical distribution/probability 报 insufficient_data。
标量状态区分 ready、insufficient_data、undefined；非有限标量结果以明确原因返回 undefined，不编码成 null 或 0。

## 分布与采样

`distribution` 操作的 task 明确 distribution、evaluate_at、quantiles、sampling。
Normal、StudentT、Beta 由 statrs 0.19.1 求 PDF/CDF；Binomial 求 PMF/CDF；经验分布为离散质量。
参数范围：Normal/Student-t 的位置绝对值 <= 1e12，标准差/尺度 1e-12..1e12；
Student-t 自由度与 Beta 形状 0.05..10000；Binomial trials=0..1,000,000，probability=[0,1]。
evaluate_at 有限且绝对值 <= 1e50，每组最多 256；quantiles 每组最多 256，p∈[0,1]。
二项 PMF 在负数/非整数处为 0，CDF 对实数先 floor；经验 PMF 按相等观测计数，CDF 为 <= 的比例。
经验分位为广义反函数（取使 ECDF>=p 的最小观测，p=0 取最小值），与 describe 的 type 7 不同。

为约束依赖中的迭代，Beta 采用最多 256 次二分；Student-t 最多 128 次括界扩展后再二分。
每步支持 checkpoint；CDF 残差要求 <= 1e-12×min(p,1−p)。无法达到精度或括界时返回 numerical_failure，
不声称所有极端参数均可数值求解。有限计算域以外的无穷分位/PDF 返回 undefined。
Normal 使用依赖的非迭代反函数，Binomial 使用有限离散区间搜索。

采样仅在 sampling={seed,samples} 明确提供时执行。samples=1..4096；
固定 ChaCha8/rand_chacha 0.9.0 + rand 0.9.5 Open01，通过上述反函数生成，绝不调用系统随机源。
结果保存种子、算法、请求/实际样本数。失败/取消返回错误，没有冒充完成的部分结果。
同一固定工具链/平台及版本内复现；没有跨平台 f64 逐位一致承诺。

## 概率任务、后验与隔离

`probability` 为显式拟合：定义 event_definition、conditioning、horizon_ms、label_definition_version、
interval_level、prior(alpha,beta) 和 assumption=`iid_after_nonoverlap_selection`。
先验每个形状限 0.05..10000。语义字段由调用方负责准确描述，核心不解析自然语言来生成标签。

事件要求唯一 id、`0 < condition_known_at <= start < end <= available_at`，区间为半开 [start,end)，
每个 end−start 必须等于 horizon_ms。按 (start,end,id) 排序后依次筛选：

1. available_at > fit_cutoff：排除尚未成熟标签，不读取其 hit。
2. condition_matches=false：排除不符合条件的事件。
3. 已成熟且符合条件必须有 hit；start < 上一个入选事件的 end：排除重叠。
4. 剩余事件统计 n/k。相邻区间可以同时选入；相同区间的重复事件即使 ID 不同也不会重复计数。

结果保留三类排除计数、入选 ID、n、k、p=k/n、Wilson score 区间。
非重叠不能证明独立；effective_sample_count=null 且 method 明示未估计。
Wilson 明示 IID 假设，用入选 n，不伪造时间依赖调整后的有效样本量。
零命中/全命中时区间仍非退化，经验 p 的 0/1 不被当成确定性预测。

Beta-Binomial 后验为 alpha'=alpha+k，beta'=beta+n−k。返回显式先验、后验及不可变 BetaArtifact：
版本、输入 schema、身份、定义、训练截止、范围、完整输入/入选哈希、n/k 和产物哈希。
该入口对完整去重训练集重新拟合，不提供消费增量数据的隐式更新接口；重试是幂等纯计算。
先验不能偷偷替换成上一后验并重用原始数据；调用方若这样操作属于改变模型规格，无法由本库推断意图。

`infer_beta(artifact,as_of)` 或 batch 的 `infer_beta` 操作只验证/读取产物，不接受训练标签、不重新拟合。
拒绝不兼容版本、损坏哈希、不一致 n/k/后验和 as_of < fit_cutoff。
返回单次同定义 Bernoulli 事件的后验预测均值 alpha'/(alpha'+beta')；
另一个字段为潜在参数的双侧等尾 bayesian_credible 区间，不能解释成下一次 0/1 标签的区间。
原始标签定义随产物保留，不适用于随意更换事件、条件、期限或成本口径。

## 成本与服务边界

原生和 MCP 同样限制 4096 points、4096 events、1..16 operations、65536 预计输出标量、
2,000,000 调度 work units。滚动排序按 n×window×16 估算，分布按输出数×384+n×16；
这是保守调度权重，不是 CPU 指令计数。超限在重计算前拒绝。
原生提供 checkpoint，调用方可实现 deadline/cancel；MCP 复用现有 2 并发、10 秒截止、
1 MiB 请求帧和 8 MiB 响应限额，CPU 工作在线程池执行。取消为协作式，不是进程级强制终止。
目录增加 capability_kind 和数据/拟合/随机/在线状态；复用 stat_* 标识，不把 6 个分析入口计成新增技术指标数量。

## 本批边界及参考

本批提供 2 序列协方差矩阵，不支持任意维矩阵；OLS 只有基础描述诊断，无参数区间/异方差修正。
分析窗口为批量历史窗口，未提供 S1 统计流式快照、拟合 Z-score/Winsorize 产物和通用 walk-forward 切分器。
以上是 S1 批次历史边界；后续检验、Bootstrap、VaR/ES 及模型实现以[当前覆盖](../formula-coverage.md)和[工作流契约](inference-workflows-v1.md)为准。完整 S2/S3 验收仍未完成。

参考来源：[R quantile](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/quantile.html)、
[NIST Wilson interval](https://www.itl.nist.gov/div898/handbook/prc/section2/prc241.htm)、
[statrs 0.19.1](https://docs.rs/statrs/0.19.1/statrs/)。数值测试以小样本手算与闭式分布为独立依据，
不是只拿封装后的自身输出相互比较。测试映射见 `../evidence/2026-09-03-s1-analysis.md`。
