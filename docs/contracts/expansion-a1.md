# A1 指标规格卡

2026-09-03。A 批 40 类中的首个增量：10 类、10 个固定 Profile。
目录合计 33 类、45 个 Profile；A 批仍有 7 类未接入，既有复杂原型尚待逐项审核。
以下固定参数都可通过 `indicator_catalog` 读取；不能通过修改 JSON 参数任意改变窗口。
新增实现版本 `roze-ta-a1-v1`，每个参数对象含 `formula_variant` 与 `algorithm_version=a1-v1`。

## 公共契约

- 全部使用现有 Stream/V2 batch/MCP 同一计算路径；无 I/O、订单流或服务运行时依赖。
- 输入为已收盘 OHLCV，价格有限且严格正、绝对值 <= 1e100；volume 有限、非负且 <= 1e100。
- 统一校验 closed_at 严格递增、closed_at <= available_at <= as_of，available_at 不倒退。
- 新 Profile 参数固定：周期型 n=14。其他周期 ID 当前为 unsupported_profile，不把周期计为新指标族。
- 表中最少样本之前 warming_up 且无数值；完成预热但零分母为 undefined_result 且无数值。
- 每个新 Profile 只有表中一个输出，units 字段精确适用于该输出。输出可知时间等于当前输入 available_at。
- 价格缺口不自动插值；周期是观测根数，不推测交易日历；拆股、复权、成交量单位变化需新 data_version/stream。
- 窗口算法只保存最后 14 个输入或上游有限状态，不存整段历史。V2 更新为事务式校验后推进；非法输入不消费。
- 全量 V2 上限仍为 4096 bars、64 Profiles、65536 输出行；超限须拆分，不提高服务限额来容纳目录增长。

## 逐项规格

| 家族 / Profile | 公式与初始化 | 最少样本与理由 | 输出 / 单位 / 复杂度 |
| --- | --- | --- | --- |
| wma / wma.14 | 线性权重，最旧到最新为 1..14；WMA=Σ(j×Cj)/105。直接封装 Yata WMA；首值填充内部窗口 | 14；完全替换填充历史 | wma / price；正输入时在窗口价格区间；O(1) 算法、O(14) 状态 |
| rma / rma.14 | RMA_t=(C_t+13×RMA_prev)/14；直接封装 Yata RMA，首收盘价种子 | 14；发布策略为一个完整观测周期，首值影响以 (13/14)^k 衰减，并非完全消失 | rma / price；正输入时正；O(1) 算法/状态 |
| dema / dema.14 | E1=EMA(C), E2=EMA(E1), DEMA=2E1−E2；alpha=2/15，各级首收盘价种子，直接封装 Yata DEMA | 27=2×(14−1)+1；按两级平滑观察跨度设发布门槛，不等同 SMA 种子平台 | dema / price；可能超出原价格范围；O(1) 算法/状态 |
| tema / tema.14 | E3=EMA(E2), TEMA=3E1−3E2+E3；同上种子，直接封装 Yata TEMA | 40=3×(14−1)+1；三级平滑观察跨度，初值影响仍可能存在 | tema / price；可能超出原价格范围；O(1) 算法/状态 |
| vwma / vwma.14 | Σ(C×V)/ΣV，最近 14 根真实 bar，不填充成交量；外层 bounded window 每步重算，消除滚动减法的残留权重 | 14；完整权重窗口；全窗口零成交量 undefined，随后正成交量可恢复 | vwma / price；正权重时在窗口价格区间；O(14) 算法/状态 |
| roc / roc.14 | (C_t−C_t−14)/C_t−14，直接封装 Yata ROC | 15；需要实际 14 期前价格，禁止使用上游早期填充值输出 | roc / ratio；>-1，无上界；不是 0..100 百分数；O(1) 算法、O(14) 状态 |
| momentum / momentum.14 | C_t−C_t−14，直接封装 Yata Momentum Method | 15；同上 | momentum / price；有正负；O(1) 算法、O(14) 状态 |
| ad / ad.cumulative | AD_0=0，每根加 CLV×V；CLV=((C−L)−(H−C))/(H−L)，H=L 时 CLV=0；含第一根贡献。复用上游 CLV、外层累计 | 1；从显式序列起点累计；不用无法反序列化的上游空 Window | ad / caller_volume；有正负，依赖累计起点；O(1) 算法/状态 |
| obv / obv.zero_seed | 第一根仅保存价格、OBV=0；之后涨加当前 V、跌减当前 V、平盘不变 | 2；需要一次真实价格比较；首根只 warming_up | obv / caller_volume；有正负，依赖累计起点；O(1) 算法/状态 |
| williams_r / williams_r.14 | −100×(HH14−C)/(HH14−LL14)，包含当前 bar；全窗口 H=L 时 undefined | 14；完整极值窗口 | williams_r / −100..0；无量纲百分刻度；O(14) 算法/状态 |

WMA/RMA/DEMA/TEMA 的早期数值遵循明确的 Yata 首值种子变体。
DEMA/TEMA 发布门槛不声称达到某个固定收敛精度，不能与不同种子的实现做无条件逐位对比。
上表 O(1) 为算法更新；现有引擎为保证更新事务性会复制有限状态，窗口型端到端更新为 O(14)。

## 成交量与异常

VWMA、A/D、OBV 的 volume 必须由调用方声明并在同一流中保持单位一致；本接口沿用 Bar 的单 volume 字段，
尚没有逐 bar 的 volume_type 类型合同，因此目录明确使用 caller_volume，不把它解释为主动买卖量。
VWMA 为收盘价加权的 bar 指标，不是逐笔精确 VWAP；VWAP 的会话/时区/锚定契约留在后续 A 批。
零量本身是合法输入，Measurement 保留 zero_volume 标记；WMA 等价格指标不因零量停止。
flat bar 的 A/D 贡献为 0（指定变体），OBV 平盘贡献为 0；不能将这些约定推广为所有订单流算法的规则。

## 兼容、快照与目录

不改旧 35 个 Profile 的参数、最少样本、单位或公式；旧 TRIX 在目录明确为 yata_absolute_smoothed_change。
新增目录字段 family_id、capability_kind、formula_variant、warmup_rule、verification_status。
旧 Profile 状态仍标 partial_audit，19 个复杂默认 Profile 的 legacy_256_pending_individual_audit 明确保留待办。
不会把元数据标签当作旧公式已审核或将 256 改成未经证明的较小数字。

Kernel 新扩展仅追加在旧二进制枚举之后；现有引擎快照 v1/v2.0 布局和旧参数哈希保持兼容。
新算法由新 Profile ID 与 a1-v1 参数辨识；旧二进制不能计算新 ID，应返回不支持。
测试载入改动前真实生成的 35 个历史快照并继续一根，结果与旧实现保存的预期 Measurement 完全相等。
新 Profile 也经过未初始化、预热中、就绪、未定义及恢复后的状态路径测试。

## 来源与证据

数值参考不依赖调用封装本身：`tests/expansion_a1.rs` 按原始价格/量窗口求和和独立递推，核对每一根就绪输出。
全部目录批量/流式/多切点恢复测试自动覆盖新 10 项；MCP 对 10 项完整序列与原生 JSON 对比。

来源：固定 vendor/yata/src/methods/{wma,rma,ema,rate_of_change,momentum,vwma,adi}.rs 与 core/ohlcv.rs；
[Fidelity Williams %R](https://www.fidelity.com/learning-center/trading-investing/technical-analysis/technical-indicator-guide/williams-r)、
[Fidelity OBV](https://www.fidelity.com/learning-center/trading-investing/technical-analysis/technical-indicator-guide/obv)。
验收记录见 [A1 evidence](../evidence/2026-09-03-expansion-a1.md)。
