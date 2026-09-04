# 2026-09-04 组合风险验证

基线：`f5b4831fb0289c03c2959aa774941e72e4004189` 及此前尚未提交的全原生 MCP 改动。
平台：Windows，本地固定 Rust 工具链，Cargo.lock 未改变。
在隔离目录验证后按文件摘要保护应用到 roze-ta，未修改 roze-quant 源码/依赖。

- `cargo test --workspace --locked --offline`：287 passed，12 suites。
- `cargo fmt -p roze-ta -p roze-ta-mcp -- --check`：通过。
- `cargo clippy -p roze-ta -p roze-ta-mcp --all-targets --no-deps --locked --offline -- -D warnings`：通过。
- `verify-upstream.ps1`：100 个原始文件通过。
- `verify-native.ps1`：90 个派生源映射及补丁重放、许可证、45 个迁移前快照通过。
- 新增 6 项核心测试：独立手算、对冲/零敞口、样本不足、时间/数据选择、非法输入/限额、哈希/取消/极端损失。
- 既有全分析 MCP 测试新增组合请求，官方 SDK 发现/调用全部 14 个方法，逐结果与核心库一致；7 个工具不变。

实现是只读线性组合分析，无账户或订单权限；不宣称账户风控、非线性压力测试、远程 MCP 部署或生产验收通过。
