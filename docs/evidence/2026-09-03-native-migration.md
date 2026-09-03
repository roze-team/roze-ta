# 原生算法迁移验证

日期：2026-09-03；平台：Windows x86_64-pc-windows-msvc；Rust 1.98.0。
依据 roze-skills、FR-ENG-001/002/006，以及 AC-001/006/007/009。
迁移前提交：`aa8fba81448c88ff73dff705e7d33869bf2aae01`。

## 实施范围

- 90 个 Rust 文件映射到 roze-ta 内部 core/helpers/indicators/methods/prelude。
- 移除 Yata Cargo 依赖与 workspace 成员；保留 vendor/yata 的全部 100 个原始文件和原始 UPSTREAM.json。
- 固定 f64/u8、始终启用 serde，移除 Window/SMM 可选 unsafe 路径。
- 派生模块保留版权和 Apache-2.0；自有代码保留 MIT，组合 crate 标注 MIT AND Apache-2.0。
- 保留原生公开路径与 roze_ta::yata 模块兼容路径；公式和状态字段布局不变。

实施先在 `target/native-migration-stage` 临时 checkout 验证。
Cargo 命令添加 `--target-dir D:/Alion/roze-ta/target` 复用构建缓存，均使用本地锁定依赖，无需联网。

## 验收证据

| 命令或检查 | 结果 |
| --- | --- |
| `rtk cargo test --workspace --all-features --locked --offline` | 独立迁移 268 passed；合并并发 MCP 改动后 272 passed；含移入的 132 个单元测试、69 个文档测试和新增 2 个迁移测试 |
| `rtk cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过；只处理维护副本 |
| `rtk cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --all-features --no-deps --locked --offline -- -D warnings` | 通过，无警告 |
| `rtk proxy powershell -NoProfile -File scripts/verify-native.ps1` | 90 个映射、完整补丁重放、许可证及快照基线摘要通过；同时核验 100 个上游原件 |
| `rtk proxy powershell -NoProfile -File scripts/verify-expansion.ps1` | 原生目录中的全部 36 个指标模块与 84 个目标的覆盖表一致 |
| Cargo metadata（locked/offline） | workspace 仅 roze-ta、roze-ta-mcp；解析图无 yata package；组合许可证字段正确 |
| `rtk cargo check --workspace --all-features --locked --offline` | 落地后的实际工作目录编译通过，仅构建两个项目 crate |

落地后再次读取并校验：原始 100 文件及 90 个派生映射、补丁重放、许可证、快照摘要均通过。

## 快照与错误检测

迁移前在上述真实提交运行保存的 [生成器](../../crates/roze-ta/tests/fixtures/pre-native-generator.rs.txt)，
冻结全部 45 个 Profile 的快照、后续 bar 与预期输出。
[基线记录](native-migration-baseline.json) 包含生成提交、工具链及文件摘要。
新测试从相同 280 根 bar 建立原生状态，核对完整 payload_hex、checksum、implementation_version，
再分别用原生状态与旧快照恢复后消费第 281 根 bar，核对所有输出字段。
现有 pre-A1 的 35 个历史快照继续通过。

旧 `roze_ta::yata::methods::EMA` 路径与新的 `roze_ta::methods::EMA` 在编译期接受同一可变引用，
并使用手算 EMA 值验证调用；兼容模块未重新引入独立依赖。

在临时 checkout 进行三次故意破坏，每次均在 finally 恢复原文件：

1. 在派生 EMA 文件追加未记录的注释：校验拒绝 `Native file differs`。
2. 同时更新该文件的目标摘要，保持补丁不变：校验拒绝 `Patch does not reconstruct native source`。
3. 在原始 vendor EMA 文件追加注释：校验拒绝 `Original file differs`。

恢复后完整校验再次通过。原始摘要未修改；[迁移清单](../patches/native-migration.json)
和 [逐文件补丁](../patches/yata-native.patch) 独立记录维护副本的变化。

## 兼容限制与回滚

落地前发现工作区同时新增了 MCP 流式接口和覆盖测试。保留该项工作，合并 README、
implementation-status 与 Cargo.lock 的交叉修改，再对合并结果验证；未覆盖其源码、文档或证据。

这是源码归属和构建边界迁移，不是所有底层 API 的错误处理重写或新公式验收。
低层 Method/Window 的前置条件与文档化 panic 保持既有行为；统一引擎、分析和 MCP 仍使用结构化 Result。
直接使用外部 yata 类型的调用方须改用 roze_ta 类型；不提供上游其他数值宽度或 unsafe features。
未验证 Linux、跨平台浮点一致性、长期性能或生产运行；未修改 roze-quant、部署或执行交易。
完整兼容说明与 Git 回滚范围见 [迁移说明](../patches/yata-native-migration.md)。
