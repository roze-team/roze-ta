# Yata 算法原生迁移

需求：FR-ENG-001/002/006；验收：AC-001/006/007/009。
原始版本：Yata v0.7.0，commit `5030e2349cedde60b0e367a9de9400d466ff644f`。
迁移前项目提交：`aa8fba81448c88ff73dff705e7d33869bf2aae01`。

## 目的与边界

按用户要求将算法改为 Roze 项目直接维护的原生模块，取消 Yata Cargo 依赖。
`vendor/yata` 与 `UPSTREAM.json` 保留原件和原始摘要；不再进入 workspace。
核心仍是同步纯 Rust，MCP 仍调用核心库，未引入服务运行时、I/O 或交易能力。

## 文件映射与最小行为变更

`vendor/yata/src/{core,helpers,indicators,methods}/**` 映射到
`crates/roze-ta/src/` 下同名目录；`src/lib.rs` 的 prelude 提取为 `prelude.rs`。
原 README、benches、crate 根文档和 Cargo 文件保留在原始基线；原方法单元测试与文档示例随模块迁移。

- 派生文件添加 Apache-2.0 SPDX、原作者、原路径、修改声明；文档示例导入改为 `roze_ta`。
- 固定现有生产组合 `ValueType=f64`、`PeriodType=u8`；serde 属性变为无条件启用。
- 删除 Window/SMM 的可选 unsafe 路径，保留原安全分支；整个 crate 受 `forbid(unsafe_code)` 约束。
- 维护副本按项目 rustfmt 格式化，修正生命周期写法及 Clippy 文档/风格问题；原件不改动。
- 原有公式、单位、Profile ID、参数、状态字段次序保持不变；没有新增指标。
- `roze_ta::{core,helpers,indicators,methods,prelude}` 直接导出本地类型。
  `roze_ta::yata::*` 作为模块兼容路径保留；不再存在独立 yata crate 类型身份。

原始与派生文件 SHA-256、逐文件完整 unified diff、许可证和快照基线摘要由
`native-migration.json` / `yata-native.patch` 记录。`scripts/verify-native.ps1`
校验文件集合、原始摘要、派生摘要、补丁摘要、版权声明及许可文件；不得只改摘要掩盖算法变更。
维护补丁时同时审查差异、更新证据并运行验收，原始 `UPSTREAM.json` 永远不随派生代码改变。

## 兼容性

引擎与目录中的 `yata-0.7.0@...` 字符串继续表示公式/序列化兼容版本和来源，
不是 Cargo 依赖声明。迁移源码身份另由 manifest 和项目 Git 提交定位。
保留该版本是为了旧快照、参数哈希和结果哈希兼容；未来语义变化必须显式升级版本。
迁移前真实生成的 45 个 Profile 快照被冻结，原有 pre-A1 的 35 个快照也继续验证。
新代码的快照内容、checksum 和恢复输出须与迁移前一致。

此前直接依赖外部 `yata` 并跨 crate 传递其类型的调用方需改用 `roze_ta` 导出的类型。
原生模块不提供 value_type_f32、period_type_u16/u32/u64、unsafe_performance 或可关闭 serde 的 features；
这些组合不是现有统一引擎已验证的契约。低层 Method/Window 保留原有前置条件和文档化 panic，
本次未把全部低层 API 改为 fallible；面向调用方的批量、流式和 MCP 边界继续采用结构化 Result。

## 升级与回滚

升级上游时先建立新的独立原始基线，再审查算法差异并迁移有需要的改动；
不能重新复制整个目录覆盖本地派生实现。原生模块纳入 fmt、clippy、单元与文档测试。
本次先在忽略的 `target/native-migration-stage` 完成验证，再核对原文件未变后落地。
回滚恢复迁移前 Cargo/文档与锁文件，移除本次新增原生模块、测试和迁移记录，恢复原 Yata 路径依赖；
已有上游目录无需恢复。Git 提交后可通过撤销该迁移提交回滚。
