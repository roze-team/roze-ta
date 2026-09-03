# S2A 概率评估、时间切分与重采样契约

2026-09-03。对应 FR-PROB-006、012、013 与需求第 7 节；是 S2 的增量交付。
三类 operation 沿用 [analysis-v1](analysis-v1.md) 的 Request、Scalar、错误、时间和规范哈希，
不改变既有 S1/E1 或指标快照。外层 schema_version=1；各类结果带独立方法版本。
核心不执行 I/O、不持久化、不训练模型，MCP 直接调用相同核心库。

## 1. eval_calibration / calibration_evaluation

方法版本 `roze-ta-calibration-evaluation-v1`。评估冻结的二元预测概率，不将分类分数转换成概率，
也不拟合 Platt/isotonic 校准器。

### 输入与样本隔离

CalibrationSpec 必填 model_id、model_fit_cutoff_ms、event_definition、label_definition_version、
horizon_ms、evaluation_start_ms/end_ms、baseline_probability、baseline_fit_cutoff_ms、bin_edges。
log_loss_clip 可为 null，forecasts 为 1..4096 条。概率和基准概率必须在 [0,1] 且有限。
分桶边界包含 0 和 1、严格递增、共 2..33 个，使用调用方预先固定的边界，不读取评估标签拟合分桶。

时间均为正整数毫秒。horizon>0，评估窗口为预测发生时间的 [start,end)，
model/baseline fit cutoff 必须严格早于 start。每条 Forecast 有唯一 ID、严格递增 at_ms，
且 `model_fit_cutoff < prediction_available_at <= at`；`label_end=at+horizon`，
`label_available_at>=label_end`。检查 horizon 加法溢出。上述检查也适用于未选中记录。

先排除窗口外记录，再排除 fit_cutoff 前尚未可知的标签；成熟 outcome=None 返回 invalid_sample。
剩余样本按时序贪心选择不重叠的 [at,label_end) 区间，排除数单独报告。
至少一个有效样本才能评估。标签可在评估窗口结束后成熟，只要在此次 fit_cutoff 前可知。
模型与基准截止时间是调用方声明，核心不能检查真实训练过程或历史数据真伪。

### 评分、基准与可靠性数据

对 n 个选择样本，p 为原始概率、y 为 0/1 标签：

| 输出 | 公式/解释 |
| --- | --- |
| model.brier_score | mean((p-y)²)，二元未乘 2 口径，范围 [0,1] |
| model.log_loss | -mean(y*ln(p)+(1-y)*ln(1-p))，自然对数；分支计算避免 0*ln(0) |
| baseline | 使用调用方提供的固定基准概率，按相同样本和评分口径计算 |
| brier_skill_score | 1-model_brier/baseline_brier；基准 Brier 为 0 则 undefined，可为负 |
| expected_calibration_error | sum(bin_n/n * abs(bin_mean_probability-bin_hit_rate)) |
| maximum_calibration_error | 非空桶 absolute_gap 的最大值 |

Log Loss 不默认裁剪。p=0 却命中、p=1 却未命中时，返回 undefined(infinite_log_loss)，
并计数 impossible_outcomes；正确的确定预测损失为 0。显式 clip=e 要求 0<e<0.5 且 1-e 在浮点中小于 1；
评分概率变为 clamp(p,e,1-e)，model/baseline 各自报告 clipped_probabilities。
原始 Brier、可靠性分桶和 impossible_outcomes 计数不受裁剪影响。

桶使用 [lower,upper)，最后一个桶包含 1；边界上的概率进入右侧桶。
输出 count/hits/mean_probability/observed_frequency/absolute_gap；空桶保持 count=0，
三个估计字段为 insufficient_data，不虚构 0 概率或命中率。
所有评分、概率和差距无物理量单位；Log Loss 单位为自然对数信息量。

输出数据选择 ID、范围、selection_hash、依赖标签的最大 available_at，以及事件/模型/基准定义。
不提供显著性检验或校准置信区间。Brier/Log Loss 同时反映校准与区分能力；
ECE 对分桶和样本量敏感，不能单独证明模型已校准。定义参考
[scikit-learn 概率校准文档](https://scikit-learn.org/stable/modules/calibration.html)，本项目边界/裁剪按上述显式口径。

## 2. prob_bootstrap_mean / bootstrap_mean

版本 `roze-ta-bootstrap-mean-v1/chacha8-rand_chacha-0.9.0/rand-0.9.5`。
使用外层已选择 points.x，至少两个；输出单位与输入 x 相同，方差不作为单独输出。
只重采样样本均值，暂不支持任意统计量、BCa、studentized 或平稳 Bootstrap。

BootstrapSpec 必须指定 scheme、seed:u64、replicates=2..4096、interval_level=[0.5,0.999]：

- `iid { assume_independent:true }`：从 n 个选择样本等概率、有放回抽 n 次。
- `moving_block { block_length:L, assume_stationary:true }`：L=1..n，从 n-L+1 个不环绕的重叠块
  均匀抽取起点，保留块内顺序，拼接至 n，截断最后一块。没有默认块长或默认 IID。
  选择样本必须是原始序列的可知前缀，不能跳过内部未知值后把两侧误当相邻。
  调用方负责提供连续采样观测与适合块采样的平稳序列；核心不识别交易日历。

每次调用独立初始化 ChaCha8Rng::seed_from_u64，均匀起点用固定 u64 范围采样；
无共享 RNG 或隐藏 seed。返回全部 replicate_means（顺序为生成顺序）、实际 completed_replicates、
算法版本、种子、原始均值、重采样均值、估计偏差 mean(bootstrap)-original。

standard_error 是重采样均值的样本标准差，分母 B-1；频率学 percentile 区间
取 replicate_means 的 type 7 分位数 [(1-level)/2,(1+level)/2]。
`monte_carlo_standard_error_of_bootstrap_mean=standard_error/sqrt(B)` 只度量模拟均值精度，
不代表区间端点误差，也不是增加了原始独立样本量。
固定 L 的非环绕块抽样可能产生边界偏差，结果保留 estimated_bias。

常量数据或 L=n 时可得到零宽重采样分布，这是算法条件下的退化结果，不证明真实不确定性为零。
有限 B 的 percentile 区间不保证名义覆盖率；此实现不自动纠正偏差或寻找最佳块长。
块方法族参考 [R boot 时间序列重采样文档](https://stat.ethz.ch/R-manual/R-patched/library/boot/html/tsboot.html)，
本项目固定为上面明确说明的非环绕移动块，未宣称与 R 的所有参数组合逐位一致。

## 3. stat_temporal_split / temporal_split

版本 `roze-ta-temporal-split-v1`。输出训练/校准/验证分区的样本 ID 计划，不执行拟合或评分。
必须指定 dataset_id、label_definition、purge_gap_ms>=0、minimum_partition_samples=1..4096，
1..32 个 FoldWindow 与 1..4096 条 TimedLabel。

FoldWindow 定义 `0<train_start<train_end<calibration_end<validation_end`；
相邻 fold 的 train_end/calibration_end/validation_end 严格向前移动，train_start 可以保持或增加，
因此可表达 expanding、rolling 和 walk-forward。purge_gap 必须小于训练和校准窗口各自宽度。
没有随机打乱、自动窗口默认值或重叠 fold 独立性假设。

每条记录 ID 唯一，`0<features_available_at<=at<label_end<=label_available_at`；
支持同一时刻多条记录，按 at、ID 排序，输入顺序不影响分区选择哈希。
先按 at 归入三个半开窗口，再按真实标签时间筛选：

| 分区 | at 窗口 | 标签须严格可知于 |
| --- | --- | --- |
| train | [train_start,train_end) | train_end-purge_gap |
| calibration | [train_end,calibration_end) | calibration_end-purge_gap |
| validation | [calibration_end,validation_end) | validation_end |

还要求标签已经在此次 fit_cutoff 前可知。严格 `<边界` 消除同毫秒拟合/预测先后顺序歧义。
标签结束时间不得越过相应边界；这会清除预测期跨界或延迟披露的样本，不是仅按行数留 gap。
排除计数按顺序分为窗口外、cutoff 时未知、跨界/间隔，彼此互斥。
每个分区保留 feature_window、labels_known_before、selected_ids/hash 和 sufficient_samples；
任何分区不足 minimum 时 fold.usable=false，不抛弃该 fold 或用 0 分数冒充。
usable 仅表示此次选择满足最小样本数，不保证窗口完整、统计独立或模型可成功拟合。

允许同分区标签彼此重叠；多个 fold 也可重用历史样本，不能将其分数当独立重复实验。
这是按时间向前的标签清除与前置间隔，不是完整 combinatorial purged CV 或测试期之后的 embargo。
模型训练者仍须确保所有预处理/特征选择/校准仅使用对应分区。

## 4. 资源、复现与验收边界

沿用单请求 16 operations、2,000,000 work units、65,536 output values。
新增计费：calibration `64*n`；bootstrap `n*B`；temporal `8*n*folds`，合计超限返回 limit_exceeded。
循环提供协作取消检查点；取消不返回声称完成的随机样本。MCP 仍为四个只读工具，
共用两路并发、1 MiB 请求、8 MiB 响应和 10 秒期限。
核心与现有模块共用 rand/statrs 等锁定依赖，本批无新增依赖。

不设置指标式预热；最少样本按每项契约。三类均 online_update=false，无快照或拟合产物，
通过规范请求重放复现。随机结果只承诺已验证 Windows 平台和固定算法版本的重复性。
外层 selection_hash 只代表 points；calibration/temporal 自带样本或分区哈希。
完整请求/输出哈希包含未来输入和排除计数，不能将其与仅选择样本的哈希混为一谈。
改变 cutoff 会重新选样；固定 cutoff 的未选中数值不会进入计算。

独立参考见 `tests/validation.rs`：人工 Brier/Log Loss、端点/分桶、两点分布穷举期望、
块顺序与截断、真实标签跨界、已知时间、种子/预算/取消；MCP 官方 SDK 验证同批原生一致性。
本批覆盖 AC-STAT-006/007/008 的部分要求；独立校准模型拟合、一般统计量重采样、
协议黑盒压力、跨平台数值和 S2 其余模型仍待实现，不宣称整个 S2 完成。
