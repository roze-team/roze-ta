# 原生迁移后的 MCP 验证

基线提交：`f5b4831fb0289c03c2959aa774941e72e4004189`，与用户提供的远程 HEAD 一致。
日期：2026-09-03。平台：Windows；工具链：Rust 1.98.0。
对应 FR-MCP-001～004、006、FR-ENG-006、AC-006/007/009。

此提交已经合入 5 个 MCP 工具和完整目录覆盖，不需要重复建立接口。
Cargo 不再依赖 `yata`，现有计算转而调用 roze-ta 原生模块；来源代码与许可证仍保留。
本次仅修正旧依赖说明并新增实际二进制的 stdio 集成测试，不修改公式、参数、快照格式或工具接口。

## 覆盖与实际运行

- 现有官方 SDK 协议测试覆盖 45 个 Profile 的 V1/V2、流式续算/查看/重置，以及全部 13 类分析操作。
- 新增 `crates/roze-ta-mcp/tests/stdio.rs`，直接启动 Cargo 构建的 MCP 可执行文件。
- 测试 JSON-RPC 初始化、工具发现及全部 5 个工具的实际调用；批量与分析响应和原生结果比较。
- 消费前 3 根 bar 后关闭 stdin，验证进程正常退出；启动全新进程并传入快照消费剩余 3 根，结果与原生完整批量计算一致。
- stdout 按 JSON-RPC 逐行解析，杂音会使测试失败；请求等待、测试输入读取和进程退出均有上限，测试结束清理子进程。
- Windows 子进程使用 CREATE_NO_WINDOW。未增加依赖或版本升级。

## 验证结果

验证在独立本地 checkout 完成；下列 Cargo 命令使用共享的临时 `--target-dir` 复用编译缓存。

| 检查 | 结果 |
| --- | --- |
| `cargo test -p roze-ta-mcp --test stdio --locked --offline` | 1 个真实进程测试通过 |
| `cargo test --workspace --locked --offline` | RTK 汇总 273 个测试通过，10 个 suite |
| `cargo fmt -p roze-ta -p roze-ta-mcp -- --check` | 通过 |
| `cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps --offline -- -D warnings` | 无问题 |
| `scripts/verify-upstream.ps1` | 原始基线 100 个文件匹配 |
| `scripts/verify-native.ps1` | 90 项源码映射、补丁重放、许可证和 45 个迁移前快照通过 |
| `git diff --check` | 通过 |

所有 shell 命令通过 RTK 执行；PowerShell 脚本通过 `rtk proxy powershell -NoProfile -File` 执行。

## 完成范围

当前目录的 33 类指标 / 45 个固定 Profile 和 13 类分析操作已支持 MCP。
未登记的原生 Method、待开发的指标与统计模型仍按原需求推进，本次不将它们算作已接入。
仅验证本机 stdio；不宣称 HTTP、Linux、生产压力或真实协议取消/超时压力验证通过。
