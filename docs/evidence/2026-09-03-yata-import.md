# Yata 来源与导入验证

- 日期：2026-09-03。
- 官方仓库：https://github.com/amv-dev/yata
- 标签：v0.7.0
- 提交：5030e2349cedde60b0e367a9de9400d466ff644f
- 平台：Windows x86_64-pc-windows-msvc。
- Rust：1.98.0 (88d9e12ae 2026-08-18)。
- Cargo：1.98.0 (797e8a9bc 2026-08-05)。

## 方法与结果

本轮从官方 GitHub 仓库下载指定标签，在临时目录用 git archive 导出 100 个受版本控制文件。
以每文件 SHA-256 与现有 `D:\Alion\roze-ta\vendor\yata` 比对：100/100 一致，数量一致。
目标目录已有另一批导入文件，故沿用现有 vendor/yata、UPSTREAM.json、workspace 和扩展代码，没有覆盖或重复导入。

临时 workspace 使用原始 Yata 与其 Cargo.lock，执行：

- `cargo test --workspace --locked --offline`：132 个单元测试、69 个文档测试通过，总计 201，无失败。
- `cargo check --workspace --locked --offline`：通过。
- SHA-256 完整性：测试前后均 100 个文件一致。

上游产生 7 条编译警告：4 条 unnecessary qualification、3 条 lifetime syntax 提示。
保留上游原文，未运行自动修复。原始日志见本目录的 yata-cargo-test.log 与 yata-cargo-check.log。
日志来自临时导入 workspace，因此其中 vendor 路径为 yata-0.7.0，与目标目录名不同但内容一致。

## 证据范围

以上结果仅覆盖官方 Yata 基线与来源完整性。
不代表外层原型已满足需求 AC-001～AC-010，不代表 Linux、nightly benchmark 或生产性能验收通过。
用户粘贴的 V0.1 需求与当前 docs/requirements.md 在统一换行并去除末尾空白后完全一致。

