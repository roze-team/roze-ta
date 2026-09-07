# Wickra 纯数据变换 V1

原生入口：`roze_ta::wickra_data::aggregator::{Timeframe, TickAggregator}`、`roze_ta::wickra_data::resample::Resampler`。来源及维护差异见[整仓迁移记录](../patches/wickra-repository-migration.md)。这些是数据变换，不计入514个指标，也未新建MCP工具。

`Timeframe::new(bucket)`要求正整数；单位与输入timestamp一致。`minutes/hours/days`使用秒，`millis/one_minute_ms`使用毫秒。桶起点为向下取整的时间边界；i64极限使用原生饱和规则。

`TickAggregator::push(Tick)`按桶聚合：open为第一笔、high/low为极值、close为最后一笔、volume为体积之和。只有新桶输入到达才发出之前的桶，时间戳为桶起点。支持同时间戳的多笔成交，拒绝桶内或跨桶倒序。

`Resampler::push(Candle)`把细粒度K线合成较粗的桶，OHLCV按相同规则归并。调用方保证原始K线已收盘、输入顺序正确，目标周期是源周期的整数倍；原生实现不验证源周期，也不拒绝同桶内倒序，不能当作严格as-of引擎使用。

默认跳过空桶；显式`with_gap_fill(true)`才发出前收盘价、零成交量的合成K线，每次最多1,000,000个填充桶。合成K线不是观测行情，调用方须区分其来源。无缺口时更新和状态O(1)；填充g个桶时输出时间/空间O(g)。原生输入类型应通过校验构造器建立，不把new_unchecked当作数据校验入口。

`flush()`返回仍在累计的开放桶并清空它，不证明该桶已经在市场收盘。累计量溢出为结构化错误；沿用上游flush消费当前桶的错误语义。统一时间隔离与快照契约仍属于Reference/Profile接口，不自动附着在这些低层辅助类型上。

验证使用上游手工OHLCV样本、边界跨桶、负时间取整、时间溢出、乱序、零量、显式缺口及填充上限测试。原始数值测试与维护代码分别保留，可重放来源补丁。
