# Wickra 整仓导入与原生补充

用户要求把 Wickra 全部迁入。本次范围固定为提交 `7ed1504c805cb45fff7a317a659eb3ad559a80a7`，完整 Git tree 共2639个文件，均已导入 `vendor/wickra-full/` 并逐字节验证原始Git blob。

## 来源与范围

- 完整保留核心、数据层、门面crate、基准、Rust示例、所有语言绑定、测试、golden数据、文档、脚本与许可证。
- `wickra-core` 全部514个导出算法在 `roze_ta::wickra_all` 编译，全部进入Reference目录及MCP。其524个维护文件和补丁仍由[核心迁移清单](wickra-full-migration.md)单独验证。
- 新增 `roze_ta::wickra_data::{aggregator,resample}`，直接维护原始纯内存逐笔聚合和K线重采样算法及单元测试。
- 原始全目录属性测试迁入 `tests/wickra_invariants.rs`，514项算法加1项目录完整性测试。原7个模块内proptest恢复原始随机生成和参考断言，固定开发依赖proptest 1.11.0，仅启用std。
- 原生并行方法保留为可选 `wickra-parallel` feature，依赖固定rayon 1.12.0。默认关闭；不改变原有MCP计算接口及限额。

Python、Node、WASM、C/C++、C#、Go绑定和CSV/交易所数据源客户端均保留完整原始源码，未编入roze-ta核心或作为本项目多语言发行包发布。核心仍无隐式I/O；上游工作流和发布脚本仅是原始资料，不会在本项目自动执行。整仓源码已导入不等于每个外部语言工具链都已构建验收。

## 最小维护差异

两个数据模块仅修改路径、版权来源文件头和fmt。错误类型由本地纯数据模块定义，保留InvalidTimeframe/Core/Malformed，不引入其CSV、HTTP、WebSocket依赖。
属性测试仅修改库路径、文件自引用和来源文件头/fmt，保留所有算法构造、随机生成策略、断言和覆盖检查。

使用上游MIT许可；原始MIT/Apache-2.0全文均在vendor保留。新增[三文件派生清单](../evidence/wickra-repository-derived.json)和[补丁](wickra-repository.patch)，原始来源身份不随维护修改更新。

## 重现与校验

`import-wickra-repository.py` 在写入前验证归档全部成员：拒绝路径越界、链接、未登记文件、重复文件、错误blob或与已有基线不同的内容。
`verify-wickra-repository.py` 检查完整2639文件集合和Git blob，核对派生摘要并在工作区独立临时目录重放补丁。`--record`仅用于明确审查后的派生变更，不更新原始tree。
`migrate-wickra-supplement.py`生成两个数据维护模块和完整属性测试；随后fmt再记录补丁。原核心的`migrate-wickra-full.py`保留独立来源验证。

升级时固定新的独立提交与tree，核对514个导出名及所有参数/输入输出，重跑全目录属性、Reference/MCP、并行和旧快照测试。不得仅替换tree摘要掩盖本地原始基线变化。
回滚范围为本次新增整仓文件、三个补充维护文件、模块导出和开发/可选依赖；恢复本轮前核心生成脚本和派生补丁，不删除原有R1或Yata内容。
