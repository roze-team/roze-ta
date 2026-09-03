# 基础 Profile 规格卡

版本：1。固定参数只从 catalog 获取，调用方不能覆盖参数；修改公式需新 Profile/实现版本。
本轮独立参考测试位于 crates/roze-ta/tests/engine.rs，不使用另一个 Yata 调用作为基础公式参考。

| 类别 | 固定周期 | 输入/输出 | 初始化与公式 | 最少样本/状态 |
| --- | --- | --- | --- | --- |
| ema | 5,8,13,21,34,55,89,144 | close → ema，price | y0=x0，alpha=2/(p+1)，yt=yt-1+alpha*(xt-yt-1) | p；O(1) |
| sma | 5,10,20,50,100,200 | close → sma，price | 初始窗口用 x0 填满，之后滚动平均 | p；O(p) |
| rsi.14 | 14 | close → rsi，0..100 | 第一次差分作为 gain/loss 种子；后续 RMA alpha=1/14；两者都零返回50 | 15；O(1) |
| atr.14 | 14 | high,low,close → atr，price | 首根 high-low；后续 max(high-low,abs(high-prev_close),abs(low-prev_close))，RMA alpha=1/14 | 15（沿用原型）；O(1) |

RSI 是 roze-ta 既有 RMA 变体，非 Yata 默认 EMA/0..1 变体，也非以首 p 个差分平均为种子的 Wilder 标准初始化。
ATR 也是以首 TR 为种子，非首 p 根 TR 的均值初始化。最少样本是展示门槛，非误差已收敛保证。
独立测试用 sin 生成非平凡价格序列，普通标量算术按上述递推计算参考值；EMA/SMA/ATR 容差 1e-12，RSI 1e-10。
同实现状态恢复测试另外采用逐位比较，不把同路径比较用作公式参考。

常量、单调、交替、空输入、周期上限和非有限值应按各自 API 契约校验。
当前已有常量/零量与非有限/乱序/短序列等自动测试；不将这些测试误认为所有市场输入已完成验收。

## Chaikin 默认配置恢复适配

固定 ma1=EMA(3)、ma2=EMA(10)、window=0，输出为两条 EMA 差值，单位为成交量加权累积差。
CLV 和均线仍调用上游方法。初始累计值=0、两条 EMA 种子=0；每根 bar 累加 CLV*volume 再更新两条 EMA。
之所以单独保存累计量，是 Yata 空 Window 无法反序列化；源码与版本化快照说明见 engine-v2.md。
已有全部 Profile 的旧/新等值测试，复杂公式的全面独立参考与边界审核仍在待办中。

