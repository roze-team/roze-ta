# Official TA-Lib Rust migration

原始提交：`2f0426d4a3e7d5b153c83ce7e6c4b9b05d8ca9c7`。
原始 219 文件位于 `vendor/ta-lib-rust`，Git blob 摘要在
`docs/evidence/talib-rust-source.json`，上游归档与完整来源在
`docs/evidence/cross-library-sources.json`。保留 BSD-3-Clause 与全部作者声明。

最小派生变更由 `scripts/migrate-talib-rust.py` 表达：模块路径改为
`crate::talib`，示例改为 `roze_ta::talib`，README include 路径适配，
原始 runtime FMA macro 改为直接调用 portable 实现，避免引入带 unsafe
展开的 dispatch 依赖。保留原始 mul_add 算法和源文件的 lint 配置。
项目接口位于独立 `reference_all/talib.rs`，不修改上游公式。

验证：独立 C 版全部 201 函数三组输入的逐行数值对照、回归夹具、目录和恢复测试。
升级需要重新固定原始摘要、审查生成代码和重跑 C 参考；不得用更新摘要掩盖原始文件改动。
回滚可移除 talib 注册、模块和对应新增证据，既有 Wickra 变体不依赖本模块。
