# MCP 已实现能力覆盖

基线：`aa8fba81448c88ff73dff705e7d33869bf2aae01`；2026-09-03。
对应 FR-MCP-001～004、006、FR-CALC-001～004 和 AC-006。

本次新增 stateless `indicator_stream`，复用原生 Stream/new/update/latest/snapshot/restore/reset，
增加目录 methods 映射。没有新增公式、复制公式或修改 `vendor/yata`。

## 验证内容

- 官方 rmcp 客户端握手、工具发现和实际调用，现有 4 个工具加流式工具，共 5 个。
- 45 个 Profile 全部经过 V1/V2 批量协议调用，与原生输出逐项比较。
- 每个 Profile 使用 300 根非恒定价量数据，在第 143 根切分，比较 MCP 创建/续算与原生的完整新行、最新值和状态快照。
- 所有 Profile 的重复调用、查看和重置，确认调用方状态模型和原生快照一致。
- S1 七类、E1 三类、S2A 三类操作经真实协议调用，与原生 JSON 完全一致。
- 目录声明、Operation JSON Schema 中的变体与实际调用的 13 类方法集合相等，避免新增类型时遗漏目录/协议覆盖。
- MCP 流式拒绝损坏快照、不同序列版本、重复 bar、未来快照、错误版本、空续算、超大状态/批次、并发超限。
- 协作检查点注入取消/超时，验证私有工作状态被丢弃、调用方快照仍能恢复；不将此声明为真实 SDK 取消/超时压力验证。
- 已有单行输入与完整响应限额测试继续运行。

## 执行结果

Windows / Rust 1.98.0，在独立临时 checkout 验证后应用到本地 roze-ta。

| 命令 | 结果 |
| --- | --- |
| `rtk cargo test -p roze-ta-mcp --locked` | 9 个测试通过，含原有协议与限额测试 |
| `rtk cargo test --workspace --locked` | RTK 汇总 270 个测试通过，10 个 suite |
| `rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过 |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps -- -D warnings` | 自有 crate 门禁通过；上游既有 qualification/lifetime 警告保留 |
| `rtk proxy powershell -NoProfile -File scripts/verify-upstream.ps1` | 上游 100 个文件 SHA-256 匹配固定 commit |
| `rtk git diff --check` | 通过 |

MCP crate 仅将已有传递依赖 serde / schemars 声明为直接依赖，用于新请求 DTO 的序列化与 Schema；
Cargo.lock 无依赖版本升级。验证主要代码位于 `crates/roze-ta-mcp/src/coverage_tests.rs`。

## 边界

“覆盖”指当前目录的 33 类指标 / 45 个 Profile 和 13 类分析操作，不代表完整需求已交付。
公式审计、19 个复杂默认 Profile 的保守预热、上游未注册 Method、未来 S2/S3 能力维持原状态。
本轮不宣称 Linux、真实协议取消/超时压力或生产性能通过。
