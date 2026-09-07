# Wickra 完整核心迁移

来源：wickra-lib/wickra，commit `7ed1504c805cb45fff7a317a659eb3ad559a80a7`；[527文件原始 blob 清单](../evidence/wickra-full-source.json)。本次选择上游双许可证中的 MIT，逐文件保留 kingchenc 与 Wickra contributors 版权，完整许可证位于原始和维护目录。

原始文件保存在 `vendor/wickra-full`，不参与构建。524个 Rust 文件迁入 `crates/roze-ta/src/wickra_all`，以公共原始类型名提供访问；没有替换 R1 严格实现或 Yata 派生算法。

## 维护差异

1. `crate::` 路径改为 `crate::wickra_all::`；Rust 文档例子使用本库路径，加入来源文件头。
2. 输入和输出值类型派生 serde；内部算法状态不派生反序列化。wire 层通过受限历史重放恢复，逐项校验丰富输入，不直接信任私有状态。
3. 源workspace的rayon接口通过可选`wickra-parallel`启用，默认关闭，固定rayon 1.12.0。核心使用thiserror 2.0.20；独立上游断言使用approx 0.5.1。
4. 整仓补充后，七个模块内proptest已恢复原始随机策略和独立参考断言，另迁入515项全目录属性测试；固定开发依赖proptest 1.11.0（std）。此前确定性替代不再是当前维护版本。
5. 两处奇偶检查改为Rust 1.98 Clippy要求的is_multiple_of；派生代码按本项目fmt格式化。原始基线未格式化或改写。
6. 动态构造参数、边界检查、目录、输入/输出编码、快照和 MCP 适配全部位于自有 reference_all 模块，源码内的数学公式保持其命名来源变体。

[派生摘要](../evidence/wickra-full-derived.json) 与原始 blob 分开记录；[完整补丁](wickra-full.patch) 可从原始文件重放，不能用派生摘要覆盖原始来源身份。

## 重现与升级

`migrate-wickra-full.py` 先验证完整原始输入，再生成维护副本；`generate-reference-registry.py` 从明确的类型、构造器和输入/输出声明生成 dispatch，未知签名会拒绝；fmt 后才记录派生补丁。

`verify-wickra-full.py` 默认只验证；`--record` 是显式维护操作，只记录已审查派生差异，仍先校验全部原始 blob。验证在自己的绝对临时目录复制原始文件、重放补丁、核对全部派生摘要。

升级时重新获取并独立固定新提交，核对全部514导出、参数/输入/输出契约、预热和退化规则；重跑全部来源测试、原始属性测试、每项wire/MCP调用、快照及旧45 Profile兼容测试。不能假定更换上游提交后旧reference-all快照自动兼容，应增加实现版本。

本次不是上游所有低层函数的错误/资源审计。低层原生类型继续保留文档化前置条件；统一 wire 层提供不同的数据域和资源限额，详见 Reference V1 契约。
