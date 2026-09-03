# 本轮依赖与许可证

- Yata：固定 0.7.0 和上游 commit，Apache-2.0；vendor/yata 原文与许可证保持完整。
- 自有扩展：沿用根目录 MIT。
- 新增 bincode =2.0.1：MIT，仅启用 std、serde；用于完整 f64 位模式的内部快照，禁用默认 derive。
  新增传递依赖 unty 0.0.4：MIT。版本与校验和固定于 Cargo.lock。
- serde_json：沿用现有依赖，增加 float_roundtrip，用于公共 JSON f64 的准确反序列化。
- MCP 继续使用官方 rmcp =3.2.0，无自制协议分发、公式副本或新服务运行时。

本地源码核对了 bincode serde 编码限制和 Yata 的 serde 属性。
来源：[bincode 2.0.1](https://docs.rs/bincode/2.0.1/bincode/)、[Yata v0.7.0](https://github.com/amv-dev/yata/tree/v0.7.0)。
以上是本轮依赖变更记录，不代替发布时的完整 SBOM 或持续漏洞检查。

## S1 数值依赖（2026-09-03）

- statrs =0.19.1：MIT；禁用默认 features，不启用 nalgebra、依赖自带随机生成或 KDE。
  复用经过测试的分布 PDF/CDF；Beta/Student-t 反函数采用外层有界二分，避免依赖内部无显式迭代上限的求逆循环。
- rand =0.9.5、rand_chacha =0.9.0：MIT OR Apache-2.0；禁用默认 features，仅启用 std。
  使用明确 seed 的 ChaCha8 和 Open01，不调用 OS 随机数。算法和映射版本写入结果。
- 新增解析依赖包含 approx 0.5.1（Apache-2.0）、libm 0.2.16（MIT）、
  rand_core/ppv-lite86（MIT OR Apache-2.0）、zerocopy（BSD-2-Clause OR Apache-2.0 OR MIT）。
  Cargo.lock 同时包含条件平台包；r-efi 可选 MIT/Apache-2.0，未选用其 LGPL 选项。
- 详细版本、features 与许可证来自已下载包的 Cargo metadata，见
  [s1-dependencies.json](evidence/s1-dependencies.json)。该清单含解析图中的同名既有版本，
  不表示所有条件平台依赖都在 Windows 编译，也不是完整 SBOM。

这批算法属于独立库纯数值计算，没有新增服务基础设施或 Roze 运行时依赖。
