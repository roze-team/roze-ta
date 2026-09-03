# 统一引擎 V2 契约

本契约覆盖本轮 P1 首批实现，关联 FR-CALC-001～004、FR-EVD-001～005、FR-MCP-001～004。
尚未完成的逐指标公式审核、周期缺口/交易日历及多周期对齐见 [实施状态](../implementation-status.md)。

## 版本与兼容

- 新 Rust API：`roze_ta::engine`，结果 schema_version=2，Profile 版本=1。
- MCP 新工具：`indicator_batch_calculate_v2`；原有 `indicator_catalog` 和 `indicator_batch_calculate` 保留。
- V1 的请求、结果状态和哈希协议继续保留；其计算通过新流式引擎回放。回归测试固定全部 35 个 Profile 的旧/新数值一致。
- V2 状态为 ready、warming_up、undefined_result；公共输入错误返回结构化 TaError，
  不伪造 invalid_input/unsupported 的数值结果。未知 Profile 拒绝整个请求。
- V2 request 和 result 使用各自 Schema。不能将 V1 的 insufficient_data/failed 与 V2 状态直接混用。
- serde_json 启用 float_roundtrip。重新解析高精度小数时以准确的 f64 舍入为准，不复刻旧解析器可能的精度损失。
- 原生 Yata 重导出和旧 EMA/RSI/ATR helper 仍可访问；推荐用 V2 获取时间校验及明确错误。
  旧 helper 现在拒绝非有限/过大输入；EMA 拒绝 255（上游 u8 的 period+1 会溢出），而不是在构造时 panic。

## 输入和时间

SeriesIdentity 必须包含 series_id、instrument、timeframe、source、data_version，每字段 1～128 UTF-8 字节。
同一 Stream 只代表这个不可变身份，恢复时需由调用方提供期望身份与 Profile。
快照修正应更改 data_version 或 series_id，然后重建状态。

ClosedBar 包含 candle 和 available_at_ms；candle.closed_at_ms 为 UTC 毫秒。
要求 0 < closed_at_ms <= available_at_ms <= as_of_ms，收盘时间严格递增，可知时间不倒退。
重复、乱序、尚未收盘或尚不可知数据在改变状态前拒绝。
OHLCV 数值需有限、绝对值不超过 1e100；low > 0，OHLC 区间一致，volume >= 0。
负价格/价差输入尚未支持。volume 必须由调用方提供，当前尚无真实量/tick量/缺失量类型。
timeframe 是身份字段，本轮尚不解析周期、识别缺口、补齐数据或接入交易日历。

## 计算、预热与状态

Stream 提供 new(identity, profile_id)、update(bar, as_of)、latest、reset、snapshot、restore。
新建状态尚无 latest。首次 update 延迟初始化，初始化/首根更新顺序与既有 Profile 一致。
SMA 使用首值填充；EMA 使用首值为种子；RSI/ATR 保留现有变体，参见规格卡。
所有 35 个已登记 Profile 均走同一个流式计算实现，MCP 不复制公式。

无效更新及上游异常不会改变原状态。每次更新先复制有界的指标状态；复杂度增加 O(窗口状态大小)，不宣称全部 O(1)。
预热不足时 values 为空、status=warming_up；成熟后非有限值/输出数量不匹配则为 undefined_result，values 仍为空。
数学未定义会消费该 bar、保留真实内部状态；不会用上一值替代。后续是否恢复取决于指标，调用方可 reset 重建。
19 个复杂默认 Profile 暂保留原型 256 根预热限制，并附 conservative_warmup_not_audited。
这是计算状态，不代表公式或预热审核已完成。零量 bar 另附 zero_volume 标记。

批量请求 output=latest（默认）或 series。行顺序与输入一致，Profile 顺序与请求一致。
require_all_ready=true 时，任何选中输出行非 ready，整个请求返回 not_ready；
因此 series 模式包含预热行时严格模式也会失败。
普通模式允许不同 Profile 各自处于预热/未定义状态。
公共输入校验在计算前完成，不返回部分无效请求。

## 快照

快照 envelope 的 schema_version=1，包含实现版本、参数哈希、payload_hex 与 checksum。
payload 使用 bincode 2.0.1 serde standard 编码（little endian、variable integer），再编码为小写十六进制。
内部保存序列身份、Profile、完整指标状态、样本数、最后消费/可知时间、latest。
f64 按二进制位保存，包括数学运算中可能出现的非有限内部值，不经 JSON 数字重编码，不保存历史 bar 重放替代内部状态。
payload 最大 1 MiB，拒绝尾随字节、无效十六进制、解码失败、版本/参数/身份差异和不一致的状态元数据。
恢复后同一 bar 会被重复消费检查拒绝。当前只承诺同一固定实现/工具链的恢复格式，不承诺跨版本或跨平台逐位一致。
checksum 用于损坏检测，不是认证。原生快照应来自受信任存储；MCP 不接受任意快照上传。

Chaikin 的默认 windowless ADI 在 Yata 0.7.0 中包含空 Window，而该 Window 的 Deserialize 会拒绝空缓冲区。
外层用相同 CLV、累计加法与上游 EMA(3/10) 保存等价状态；不修改 vendor，且与原始数值回归及独立公式样本对照。
该适配只输出当前 Profile 定义的 oscillator，不伪称恢复了未暴露的上游信号状态。

## 哈希协议 roze-ta-canonical-v1

所有哈希为 SHA-256、小写 hex，编码以 UTF-8 字节 `roze-ta-canonical-v1` 后接一个零字节开头。
其后编码已完成类型转换和校验的值：

| 类型 | 编码 |
| --- | --- |
| null | ASCII n |
| false/true | ASCII f/t |
| 字符串 | ASCII s + 8 字节大端 UTF-8 字节长度 + UTF-8 内容 |
| 整数 | ASCII i + 8 字节大端十进制文本长度 + 无多余零的十进制文本 |
| f64 | ASCII d + 8 字节大端 IEEE-754 bits；-0.0 统一为 +0.0 |
| 数组 | ASCII a + 8 字节大端元素数 + 逐项编码 |
| 对象 | ASCII o + 8 字节大端字段数 + 按键排序的字符串键和值 |

对象按 Rust 字符串字典顺序排列，不进行 Unicode 正规化。整数和浮点类型不同；API f64 字段即使写 JSON 整数也先反序列化成 f64。
计算前拒绝非有限输入，结果状态不输出非有限值；仅内部二进制快照保留这些位。
input_hash 覆盖整个已类型化的 V2 请求（包括身份、快照、as-of、输出模式、Profile 顺序和默认选项）。
parameter_hash 覆盖 [实现版本, Profile ID, 固定参数]。
output_hash 覆盖 [schema_version, 实现版本, input_hash, series]。
快照 checksum 覆盖 [快照版本, 实现版本, parameter_hash, payload_hex]。

独立手工编码、.NET SHA-256 计算的测试向量：

| 输入 | SHA-256 |
| --- | --- |
| null | 7210b59b67acd9f3597e415599517dc54241d58480022abeb804fbfd45542641 |
| f64 +0.0/-0.0 | fd9cf5a6d78e9b96fe781e0cd832d144fd4c8a1f52a8fe3d2b92a988b22cfdac |
| 字符串 TA | 1b79f2038e3d3cdf3128f6684ffba9dfd8f9fab7801a1e1792c0c64eed388652 |

哈希证明内容一致性，不证明行情来源真实。结果不包含系统当前时间、耗时或随机值。

## MCP 资源与取消

继续使用官方 rmcp 3.2.0，stdio stdout 仅用于协议。
单帧请求最多 1 MiB（换行符不计），超限导致传输关闭；由 SDK 处理正常 JSON-RPC 编解码。
最多 4096 根 bar、64 个 Profile、65536 个选中输出行；重复 Profile 拒绝。
完整工具响应（包括 SDK text/structuredContent）最大 8 MiB；过大时返回 limit_exceeded。
每进程最多 2 个并行计算任务，超过立即拒绝；工作任务结束前持有 permit。
计算运行在 spawn_blocking 中，每根 bar 检查 SDK 请求取消 token 和 10 秒计算期限。
取消/超时为协作式，最迟下一个检查点返回，不能宣称强制中断正在进行的一次运算。
V2 序列化/响应限额检查也在工作线程；SDK 输出发送不属于 10 秒计算期限。
核心库的 calculate_controlled 使用调用方 checkpoint，不隐式读取时钟或引入异步运行时。

稳定领域错误码见 error-v2.json；SDK 本身的协议参数错误仍按 JSON-RPC 返回。
当前仅 stdio；本轮不包含 HTTP 服务验收。

## Schema

运行 `rtk cargo run -p roze-ta --example export_schema --features schema --locked` 更新 schema/ 中的五个文件。
Schema 负责类型/枚举/未知字段约束，跨字段时间、字节/行数及资源校验由实现执行。

