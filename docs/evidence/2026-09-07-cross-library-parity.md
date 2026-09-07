# 跨库逐项数值审计

本文件记录首轮基线。后续原106项差异、8项异常已完成范围明确的验收，当前状态见 [来源验收](source-compat-parity.json) 和 [最新审计](cross-library-parity-audit.json)。

需求 FR-IND-009 / AC-IND-009。针对原368条未验收映射运行独立参考。

| 分类 | 项数 | 含义 |
| --- | ---: | --- |
| TA-Lib verified_default_cases | 201 | 默认参数、三组256条输入、全部输出逐行与固定提交C版一致 |
| 其他库 verified_selected_cases | 53 | 明示参数、三组输入、选定输出一致；不覆盖其他getter或全部选项 |
| numeric_mismatch | 106 | 数值、预热、单位或输出结构存在差异，不能替换源API |
| reference_exception | 8 | Python/R参考在零振幅零成交量输入抛出异常，未认定兼容 |

总计368项、1104个配置/数据案例。完整来源与首差异见
`cross-library-parity-audit.json`。没有忽略预热行、跳过失败项或用本地实现生成期望值。
Python/R输出字段的明示重命名在脚本中；结构不一致也记为差异。
flat特指OHLC相等且volume=0；不代表所有平盘量价组合。

## 来源与复验

`cross-library-sources.json` 固定四个Git提交、归档SHA256和逐文件Git blob。

1. `prepare-cross-library-sources.py` 解包固定来源并校验路径、摘要。
2. `build-reference-talib.py` 编译独立TA-Lib C参考，`check-talib-parity.py` 生成603个独立期望值。
3. `check-python-parity.py` 直接导入固定ta/talipp源码，记录309个案例。
4. `build-reference-ttr.py` 编译固定TTR原生C，`check-ttr-parity.py` 经R参考生成192个案例。
5. `build-reference-mapping.py` 应用7处已证实的绑定修正；`build-parity-report.py` 要求368项各有三组记录。
6. Rust `talib_c_parity.rs` 和 `cross_library_parity.rs` 使用提交的参考值，无需Python/R/C运行时。

TTR参考使用本地提取的R4.5.2和`ttr-oracle-packages.csv`记录的依赖。
所有TTR R函数和C例程均来自固定Git提交；新DLL注册符号显式覆盖旧包符号。
MSVC编译R头文件采用int枚举与未使用的legacy complex布局兼容设置，未修改TTR公式。
GPL的TTR仅作独立进程测试参考，不复制到MIT核心，也不链接到产品。

## 变更与边界

迁入官方TA-Lib Rust201个函数，保留BSD-3-Clause与219原始文件，派生补丁独立记录。
`verify-talib.py` 校验原始Git blob和213个派生文件摘要。新增适配器复用统一校验、时序和恢复协议。
Reference目录781项；MCP工具数量与原50个严格Profile保持既有契约。
适配器逐次重放完整前缀，最多4096条，未承诺O(1)更新。

7处映射修正列在`reference-parity-bindings.json`：TTR样本协方差/方差/标准差、MAD缩放、TTR/talipp OBV初值、ta日收益周期。
尚未修复的差异显式保留；本轮不能宣称全部跨库兼容需求完成。

## 已执行门禁

- `cargo fmt -p roze-ta -p roze-ta-mcp -- --check` 与工作区all-targets/all-features Clippy通过。
- 核心4524单元测试、515条Wickra性质测试、既有集成测试、MCP11单元测试及stdio测试通过。
- TA-Lib603例、其他53项159例独立参考回归通过；目录全781项的构造、恢复、MCP可调用性通过。
- verify-upstream、verify-native、verify-reference-indicators、verify-wickra-repository、verify-wickra-full、verify-talib通过。
- 全量1179个文档示例通过，0失败；使用低并发、无debug信息的测试配置运行，耗时597.93秒。第一次高并发文档运行因资源争用被主动停止，未把中断视为通过。

典型未解决差异包括：ta的EMA种子不同；talipp多输出在预热期间逐字段就绪，而部分本地公式整项返回None；TTR的ROC默认连续复利单位不同。
TTR默认DPO把均线向过去平移，会使用后续样本；不能为了数值一致向实时接口引入未来数据。这类批量/因果契约差异需要单独定义接口或变体。
