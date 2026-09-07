# 106 项差异、8 项参考异常处理证据

对应需求 FR-IND-009 / AC-IND-009。新增 114 个显式来源变体，Reference 目录共 895 项。

当前 368 项跨库审计：201 项 TA-Lib 默认配置、53 项既有选定配置、111 项来源变体、3 项明确实时对齐变体。
原 `numeric_mismatch=106`、`reference_exception=8` 的记录保存在
`cross-library-parity-baseline.json`；当前审计这两类未解决项为零。

新增验收共 342 个案例：334 个正常数值案例逐行逐字段相符，8 个参考异常转为结构化错误且失败更新不改变状态。
三个数据集分别为混合行情、平盘零成交量、趋势；Python 每组 256 条，TTR 每组 512 条。
浮点统一容差为 `2e-9 * (1 + abs(expected))`，缺值与字段结构必须完全一致。
其中 KST/ta、DPO/TTR、ZigZag/TTR 的时间约定见 [来源契约](../contracts/source-parity-v1.md)，不声称与前视或回填的完整历史图表逐时点相同。

## 本轮通过的检查

- `cargo test -p roze-ta --test source_compat_parity --test reference_all --locked --offline`：114 项独立对照、8 项异常事务、真实时间戳/恢复、全目录构造与恢复。
- `cargo test --workspace --lib --tests --all-features --locked --offline`：工作区单元、集成、MCP 与 stdio 测试全部通过。
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`。
- `cargo fmt --all -- --check`、`git diff --check`。
- `verify-upstream.ps1`：100 个 Yata 原始文件。
- `verify-native.ps1`：90 个原生迁移源文件及补丁重放、许可证、45 个旧快照。
- `verify-reference-indicators.ps1`：原始 blob、派生补丁与 886 个来源映射。
- `verify-wickra-full.py`、`verify-wickra-repository.py`：524 个维护 Rust 文件、2639 个完整原始仓库文件。
- `verify-talib.py`：219 个原始文件、213 个已记录派生文件。

执行命令均通过 `rtk` / `rtk proxy`。冻结的算法基线未改写。
许可证声明增补通过 `native-notices-additions.json` 从原始固定哈希的声明文件重建，保留明确差异，不以新哈希替换原始基线。

独立数值文件的 SHA-256、逐项案例、参数和状态见 `source-compat-parity.json`。
验收限定于这些明确配置与输出；不扩张为来源所有参数组合、所有 getter 或逐位数值相同。
