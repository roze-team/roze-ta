# 金融数据分析的数学基础

日期：2026-09-05。本文补全六类数学主题中的缺失公式，作为需求阅读的背景材料。
公式不表示项目已有对应 API；当前能力以[公式覆盖说明](formula-coverage.md)、版本化契约及测试证据为准。
关联需求：协方差对应 FR-STAT-006，贝叶斯更新对应 FR-PROB-004，隐状态模型对应 FR-PROB-011；
需求定义见[概率与统计分析需求](probability-statistics.md)。本文不新增算法交付承诺。

## 1. 线性代数与张量分析（Linear Algebra & Tensor Analysis）

### 张量运算与神经网络

Transformer、LSTM 等模型使用矩阵和张量变换。以单个注意力头为例，输入
$X\in\mathbb R^{L\times d}$，投影参数为
$W_Q,W_K\in\mathbb R^{d\times d_k}$、$W_V\in\mathbb R^{d\times d_v}$：

$$
Q=XW_Q,\qquad K=XW_K,\qquad V=XW_V
$$

$$
\operatorname{Attention}(Q,K,V)
=\operatorname{softmax}\left(\frac{QK^\top}{\sqrt{d_k}}\right)V
$$

$L$ 为序列长度，$d_k>0$ 为键向量维度，softmax 按行计算；输出形状为 $L\times d_v$。
这是未加掩码的基本公式；因果预测需要在 softmax 前屏蔽未来位置。

### 资产协方差矩阵（Covariance Matrix）

设 $\mathbf r$ 为 $n$ 个资产的同频收益率向量，$\boldsymbol\mu=\mathbb E[\mathbf r]$，二阶矩有限：

$$
\Sigma_{ij}=\operatorname{Cov}(r_i,r_j)
=\mathbb E[(r_i-\mu_i)(r_j-\mu_j)]
$$

对于 $T\geq2$ 期完整且按时间对齐的观测，样本协方差与组合方差为：

$$
\bar{\mathbf r}=\frac1T\sum_{t=1}^T\mathbf r_t,\qquad
\widehat\Sigma=\frac1{T-1}\sum_{t=1}^T
(\mathbf r_t-\bar{\mathbf r})(\mathbf r_t-\bar{\mathbf r})^\top,
\qquad \sigma_p^2=\mathbf w^\top\Sigma\mathbf w
$$

$\mathbf w$ 为固定组合权重。协方差衡量共同变动，不表示因果关系；缺失值策略需单独声明。

### 主成分分析（PCA）与因子降维

对中心化特征矩阵 $X_c\in\mathbb R^{T\times n}$ 的样本协方差进行特征分解：

$$
\widehat\Sigma=\frac{X_c^\top X_c}{T-1},\qquad
\widehat\Sigma\mathbf v_k=\lambda_k\mathbf v_k,
\qquad \lambda_1\geq\cdots\geq\lambda_n\geq0
$$

取前 $m$ 个正交单位特征向量构成 $V_m=[\mathbf v_1,\ldots,\mathbf v_m]$：

$$
Z=X_cV_m,\qquad
\frac{Z^\top Z}{T-1}=\operatorname{diag}(\lambda_1,\ldots,\lambda_m)
$$

主成分彼此不相关，通常不保证统计独立，也不自动对应市场、行业或风格因子。
不同量纲的特征需要明确是否标准化；中心、尺度和投影只能用训练期数据拟合。

## 2. 概率论与数理统计（Probability Theory & Mathematical Statistics）

### 时间序列分析

ARMA 可描述平稳序列的线性均值动态，GARCH 可描述条件方差和波动聚集。
能否有效预测需通过样本外验证，不能仅由模型名称推断。

### 贝叶斯推断（Bayesian Inference）

设 $U$ 表示指定未来区间内收益率大于零，$E$ 表示当前已知的新闻或财报信息：

$$
P(U\mid E)=\frac{P(E\mid U)P(U)}{P(E)}
=\frac{P(E\mid U)P(U)}
{P(E\mid U)P(U)+P(E\mid U^c)P(U^c)}
$$

要求 $P(E)>0$。$P(U)$ 为先验，$P(U\mid E)$ 为后验；连续观测使用相应似然密度。
新闻情绪分数不能直接当作似然或上涨概率，需要明确事件、估计模型及校准方法。

### 假设检验与因子显著性

原文缺失的检验名称为 $t$ 检验。对于 $T\geq2$ 个超额收益观测 $e_t$，检验 $H_0:\mathbb E[e_t]=0$：

$$
\bar e=\frac1T\sum_{t=1}^T e_t,\qquad
s_e^2=\frac1{T-1}\sum_{t=1}^T(e_t-\bar e)^2,\qquad
t=\frac{\bar e}{s_e/\sqrt T}
$$

要求 $s_e>0$；在独立同分布正态假设下，零假设中的统计量服从自由度 $T-1$ 的 Student-t 分布。
存在异方差、自相关时需要相应稳健标准误；多策略筛选需考虑多重比较。
卡方检验用于相应类别或分布假设，IC/ICIR 用于描述因子关联及稳定性，均不能单独证明存在可交易 Alpha。

## 3. 随机过程与随机微积分（Stochastic Processes & Stochastic Calculus）

### 马尔可夫决策过程（MDP）

强化学习可将决策建模为当前状态 $s_t$、动作 $a_t$、下一步奖励 $R_{t+1}$，
并用策略 $\pi(a\mid s)$ 选择动作。无限时域折扣目标为：

$$
\pi^\star=\arg\max_\pi\mathbb E_\pi
\left[\sum_{t=0}^{\infty}\gamma^tR_{t+1}\right],\qquad 0\leq\gamma<1
$$

需要奖励有界或满足相应可积条件；金融状态是否满足马尔可夫假设须另行验证。
奖励需明确成本及风险口径，动作示例可为买入、卖出或观望。

### 几何布朗运动（GBM）与随机微分方程（SDE）

$$
dS_t=\mu S_t\,dt+\sigma S_t\,dW_t
$$

$S_t$ 为资产价格，$\mu$ 为常数漂移率，$\sigma$ 为常数波动率，$W_t$ 为标准布朗运动。
在无股息标的的风险中性模型中，漂移率取无风险利率 $r$。
相应 Black–Scholes 方程为：

$$
\frac{\partial V}{\partial t}
+\frac12\sigma^2S^2\frac{\partial^2V}{\partial S^2}
+rS\frac{\partial V}{\partial S}-rV=0
$$

$V(S,t)$ 为衍生品价值；欧式看涨期权在到期时刻 $T$、执行价 $K$ 的终端条件为：

$$
V(S,T)=\max(S-K,0)
$$

该方程采用常数利率和波动率、无摩擦连续交易等模型假设。
Deep SDE 与 PINN 可用于研究相关数值求解问题，具体方法及误差需单独验证。

### 隐马尔可夫模型（HMM）

HMM 以隐状态和观测分布描述体制切换，可研究牛市、熊市或震荡状态。
隐状态的经济含义需解释；实时分析使用截至当前的过滤概率，全序列平滑使用后续信息。

## 4. 最优化理论与凸优化（Optimization Theory & Convex Optimization）

### 梯度下降法（Gradient Descent）

神经网络训练以最小化损失为目标，反向传播计算梯度：

$$
\theta^\star=\arg\min_\theta\mathcal L(\theta),\qquad
\theta_{t+1}=\theta_t-\eta_t\nabla_\theta\mathcal L(\theta_t)
$$

$\theta$ 为参数，$\eta_t>0$ 为学习率。上式为基本梯度下降；SGD 使用随机梯度估计，
Adam 还维护梯度矩估计。非凸模型训练不保证找到全局最优解。

### 二次规划（QP）与均值—方差组合

$$
\mathbf w^\star=\arg\max_{\mathbf w}
\left[\boldsymbol\mu^\top\mathbf w
-\frac\lambda2\mathbf w^\top\Sigma\mathbf w\right]
$$

$\boldsymbol\mu$ 为期望收益向量，$\Sigma$ 为相同期限的收益协方差矩阵，$\lambda>0$ 为风险厌恶系数。
常见的预算、只做多、单资产上限、行业暴露和权重调整约束为：

$$
\begin{aligned}
\mathbf1^\top\mathbf w&=1,\\
\mathbf0\leq\mathbf w&\leq\mathbf w_{\max},\\
\mathbf l\leq B\mathbf w&\leq\mathbf u,\\
\|\mathbf w-\mathbf w_{\mathrm{prev}}\|_1&\leq\tau.
\end{aligned}
$$

$B$ 为行业暴露映射矩阵，$\mathbf l,\mathbf u$ 为暴露上下限，$\tau\geq0$ 为权重调整上限。
若换手率采用半和口径，则为 $\frac12\|\mathbf w-\mathbf w_{\mathrm{prev}}\|_1$，不能与全和口径混用。
当 $\Sigma$ 半正定时，绝对值约束经辅助变量线性化后可写为凸 QP。
最大回撤依赖收益路径，需额外定义情景与财富动态，不能直接视为普通均值—方差 QP 的标准约束。

## 5. 图论与拓扑数据分析（Graph Theory & Topological Data Analysis）

- **图神经网络（GCN/GAT）**：以股票为节点，以供应链、行业或持股关系为边，研究关联特征与滞后传导。关系数据须按当时可知时间构图；图关联不自动证明因果传导。
- **持久同调（Persistent Homology）**：研究高维数据在不同尺度下的拓扑结构，可作为市场状态变化的候选特征。是否具有危机预警价值需独立样本外验证。

## 6. 信息论与信号处理（Information Theory & Signal Processing）

- **互信息与香农熵**：互信息可度量包括非线性在内的统计依赖，熵描述指定概率分布的不确定性；估计需明确离散化或密度方法、样本量和偏差处理。
- **小波与傅里叶变换**：用于时序的尺度或频率分析及滤波。高频不必然是噪声，低频不必然是有效趋势；实时分析必须明确滤波边界与延迟，防止使用未来数据。

## 文档验收

本次范围仅为公式与解释补全：注意力投影、协方差、PCA、贝叶斯、t 检验、MDP、GBM、Black–Scholes、梯度下降及组合优化均已给出符号和条件。
验收证据见[补全记录](evidence/2026-09-05-mathematical-foundations.md)。数值 API 的验收仍按 AC-002、AC-STAT-001、AC-STAT-004、AC-STAT-005 等相应要求另行执行。
