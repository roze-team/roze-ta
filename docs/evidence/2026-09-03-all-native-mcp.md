# 全部已实现原生指标与 Method 开放 MCP

日期：2026-09-03；基线：`f5b4831fb0289c03c2959aa774941e72e4004189`，保留本地此前的 stdio 测试和文档增量。
平台：Windows，Rust 1.98.0。对应 FR-MCP-001～004/006、FR-CALC-005/006、FR-ENG-001/002 与 AC-006/009。

## 交付

- 全部 36 个原生 Indicator 模块、44 个独立 Method 通过统一核心 API 和 MCP 开放。
- 新增 `native_catalog` 与 `native_batch_calculate`，总计 7 个 MCP 工具；原有 45 个 Profile 和 13 类分析操作保持兼容。
- 单一原生清单记录 ID、符号、别名、输入种类、参数字段和源码文档；构造与计算调用既有原生类型，不复制公式。
- 支持全部 4 个 Indicator 类型别名及全部 Method 类型别名；不把别名、状态/结果类型、教学示例和辅助函数计作额外算法。
- 参数覆盖、枚举均线配置、四种输入类型、原生数值/索引/信号/合成 bar/多 bar/JSON 输出。
- 明确原生种子、未认证预热、非有限/无输出状态、发出时间与哈希；不把原生接口开放等同于固定 Profile 的公式审核。
- 限制输入、操作数、窗口、输出项、Renko 砖块、Past 单个 JSON 值和保留行字节数；沿用 MCP 并发、超时、取消及传输限额。

## 验证

`crates/roze-ta/tests/native_all.rs`：

- 每个 Indicator 与对应 Rust `Default().over()` 的完整输出比较。
- 每个 Method 与对应 Rust `new/next` 比较，覆盖初始化、数组顺序、信号、合成 bar 和变长输出。
- 从源码扫描真实 Indicator 模块、Method 实现和两类公开类型别名，与目录集合逐项比较，防止漏接。
- 所有默认能力经过常量和零成交量输入；非有限值显式标记，不假造零值。
- 独立 SMA 种子小样本、参数哈希变化、聚合 pending/发出时间和结构化 Past 验证。
- 非法参数、未来/重复样本、未知 ID、批量规模、Renko 过量输出、Past 内存放大、返回字节规模以及取消/超时检查点验证。

`crates/roze-ta-mcp/src/native_tests.rs`：

- 使用官方 SDK 握手并发现全量目录；实际调用每个条目、每个别名及每种宣告支持的输入表示，与核心 API JSON 比较。
- 验证原生工具与现有计算入口共用并发名额，非法参数与 Renko 超限返回稳定错误码。

`crates/roze-ta-mcp/tests/stdio.rs`：

- 实际启动二进制并调用全部 7 个工具，验证 stdout 协议格式和原有快照跨进程恢复。

| 门禁 | 结果 |
| --- | --- |
| `rtk cargo test --workspace --locked --offline` | 281 个测试通过，RTK 汇总 11 个 suite |
| `rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过 |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps --offline -- -D warnings` | 无问题 |
| `verify-upstream.ps1` | 100 个原始文件匹配 |
| `verify-native.ps1` | 90 项原生映射、补丁、许可证和 45 个旧快照验证通过 |
| `rtk git diff --check` | 通过 |

Cargo 验证使用隔离 checkout，并通过临时 `--target-dir` 复用构建缓存；两个脚本通过 `rtk proxy powershell -NoProfile -File` 执行。
未修改 `vendor/yata`、90 个迁移算法文件、迁移哈希或 Cargo 依赖。

## 范围

本次“全部”指当前已实现的实际原生指标/Method 加已有 Profile 和分析操作，不包含尚未实现的需求算法。
新原生入口为有界批量；原生类型的快照/增量协议不在本次交付内，原有 Profile 流式接口保持可用。
验证针对本机 stdio，未宣称 Linux、HTTP、多租户或生产压力验收。
用法见 [native-mcp](../usage/native-mcp.md)，完整清单见 [CSV](../native-mcp-coverage.csv)。
