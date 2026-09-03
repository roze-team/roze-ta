# P1 首批实现验证

日期：2026-09-03。环境：Windows x86_64-pc-windows-msvc，Rust/Cargo 1.98.0。
实现版本：roze-ta-engine-v2.0/yata-0.7.0@5030e2349cedde60b0e367a9de9400d466ff644f。
本轮先在 C:\Users\xFc\AppData\Local\Temp\roze-ta-p1-20260903 实现并验证，再按文件清单写入目标项目。

## 门禁

| 检查 | 结果 |
| --- | --- |
| cargo test --workspace --locked --offline | 222 通过：核心 unit 6、引擎 integration 12、MCP 3、Yata unit 132、Yata doc 69 |
| cargo fmt -p roze-ta -p roze-ta-mcp -- --check | 通过 |
| cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps --locked --offline -- -D warnings | 自有代码通过，保留上游 7 条既有警告 |
| cargo test -p roze-ta-mcp --locked --offline | 最终取消 token 生命周期调整后 3 项通过 |
| cargo run -p roze-ta --example export_schema --features schema --offline | 导出 5 个 Schema |
| cargo run -p roze-ta --example stream --locked --offline | EMA 流式/快照/恢复示例运行通过 |
| verify-upstream.ps1 | 100 个上游文件一致 |

原始日志：[完整测试](p1-cargo-test.log)、[Clippy](p1-clippy.log)。

## 关键证据

- 35 个 Profile × 4 个恢复切点（0、1、14、257），逐行对照批量/连续状态，数值比较 f64 bits。
- 35 个 Profile 常量/零区间/零量序列的快照恢复；二进制保留可能的非有限内部值。
- EMA/SMA/RSI/ATR 独立递推参考，Chaikin 累计量+两条 EMA 手工公式参考。
- 旧批量计算路径与新引擎的 35 Profile 数值回归；该回归不替代独立公式审核。
- 非有限、非法 OHLC、重复/乱序、未来可知时间、序列身份、批量/输出限额。
- 更新失败不污染状态；快照损坏/版本/Profile/身份不匹配拒绝；reset 与新状态一致。
- 规范哈希独立字节向量、-0.0 规范化、身份绑定、追加未来数据不修改过去行。
- 官方 SDK 握手、三工具发现与只读声明，V1/V2 调用与原生结果一致，并发限制、响应/帧上限。

## 交付与边界

上游源码保持原样，未修改 roze-quant。
新增 bincode 2.0.1 / unty 0.0.4，现有依赖版本未升级；serde_json 增加 float_roundtrip。
目标目录内新近增加的 V0.2 需求/指标扩展/概率统计文档保留，不用临时目录旧版本覆盖。
落地前原文件备份在 C:\Users\xFc\AppData\Local\Temp\roze-ta-p1-backup-20260903，
其中 manifest.json 记录存在性与前后摘要，可仅恢复本轮修改。新增文件回滚按清单移除。

仓库当前文件尚未提交，验证范围由本次源文件摘要清单 p1-source-sha256.json 标识。
未完成 Linux 验证、压力下取消/超时黑盒验收、复杂公式全面审核、交易日历/多周期、性能基线及概率统计扩展。
详见 [实施状态](../implementation-status.md)。

落地后验证：目标目录执行 cargo build -p roze-ta-mcp --locked --offline 成功，
产物为 target/debug/roze-ta-mcp.exe；再次通过自有 fmt 和 100 文件上游完整性检查。
25 个自有源码/契约等文件通过严格 UTF-8 解码及 replacement-character 检查。
