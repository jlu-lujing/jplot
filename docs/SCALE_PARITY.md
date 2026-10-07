# scale_* 全家族参数盘点与实现优先级（SCALE_PARITY）

来源：ggplot2 4.0.3 源码（`refs/ggplot2/R/scale-*.R`）、`api_inventory.json`（152 个 `scale_*` 构造函数 + 110 ggproto 中 Scale 系）、对照 `crates/jplot-core/src/scale.rs` + `build.rs` 当前实现。纯研究产物，不改代码。

---

## 1. 家族概览

152 个 `scale_*` 导出函数实际上坍缩为 **8 个基础构造器** + 若干 palette 变体：

| 基础构造器 | ggproto | 覆盖的导出函数（去重后每 aesthetic × 变体） |
|---|---|---|
| `continuous_scale()` | `ScaleContinuous`（位置子类 `ScaleContinuousPosition`） | `scale_{x,y}_continuous`、`scale_colour/fill_continuous`、`gradient`、`gradient2`、`gradientn`、`viridis_c`、`distiller`、`alpha`、`size`、`radius`、`linewidth`、`date/datetime/time` 位置变体 |
| `discrete_scale()` | `ScaleDiscrete`（位置子类 `ScaleDiscretePosition`） | `scale_{x,y}_discrete`、`colour/fill_discrete`、`hue`、`grey`、`brewer`、`viridis_d`、`alpha_ordinal`、`size_ordinal`、`shape`、`linetype`、`manual`、`qualitative` |
| `binned_scale()` | `ScaleBinned` | `scale_{x,y}_binned`、`colour/fill_binned`、`steps/2/n`、`fermenter`、`viridis_b`、`alpha_binned`、`size_binned`、`linetype_binned`、`shape_binned` |
| `scale_*_manual()`（离散手工） | `ScaleDiscrete`+palette | `colour/fill/alpha/shape/linetype/size/linewidth_manual` |
| `scale_*_identity()` | 各自基类 + `identity` palette、`guide="none"` | 全 aesthetic identity |
| `datetime_scale()` | `ScaleContinuousDate/Datetime` | `scale_{x,y}_{date,datetime,time}`、`alpha_date/datetime`、`size_date/datetime`、`linewidth_date/datetime`、`colour/fill_date/datetime` |
| `scale_backward_compatibility()` | 分发器 | `scale_colour/fill_{continuous,discrete,binned}` 的 `type=`/旧参数 `low/high`、`h/c/l` 路由 |
| 辅助 | — | `scale_apply`、`scale_type`（13 方法）、`scale_flip_position`、`scale_override_call`、`scale_description`、`expansion()` |

---

## 2. 基础构造器完整参数表（名称、默认值、语义）

### 2.1 `continuous_scale()` — 17 参数（scale-.R L105–126）

| 参数 | 默认 | 语义 / 渲染影响 |
|---|---|---|
| `aesthetics` | 必填 | 生效的 aesthetic 名（`x/y` 为位置轴） |
| `palette` | 必填 | 函数：连续时收 [0,1] 数值向量 → 输出值 |
| `name` | `waiver()` | 轴/图例标题；`NULL`=无标题 |
| `breaks` | `waiver()` | `NULL`=无刻度；waiver=由 transform 对象算；数值向量；函数（**输入是 limits，且位置 scale 传入的是 expand 之后的 limits**） |
| `minor_breaks` | `waiver()` | 次刻度；连续默认每个主刻度之间 1 个；离散默认无 |
| `n.breaks` | `NULL` | 提示 break 数量（仅在 breaks=waiver 且 transform 支持 n 时生效，否则 warn 忽略） |
| `labels` | `waiver()` | `NULL`=无标签；waiver=transform$format；向量（须与 breaks 等长）；函数（输入数据空间的 breaks） |
| `limits` | `NULL` | 长度 2 数值（`NA`=沿用数据端点）；函数（输入现有 limits，**数据空间**）；构造时即 `transform$transform(limits)` 存变换空间并 `sort` |
| `rescaler` | `rescale` | 连续非位置 scale 的 [0,1] 归一函数；位置 scale 忽略（恒 rescale）；`gradient2` 用 `rescale_mid(mid=0)` |
| `oob` | `censor` | 越界处理：`censor`→NA（**位置 scale 的 limits 因此是"删数据"**）；`squish`→压回；`squish_infinite` |
| `expand` | `waiver()` | `expansion(mult,add)` → 4 元 `c(mult_left, add_left, mult_right, add_right)`；默认连续 `expansion(mult=0.05)` |
| `na.value` | 构造 `NA` → ggproto `NA_real_` | NA 数据映射成的值（颜色 scale 通常 "grey50"） |
| `transform` | `"identity"` | 变换名/对象：identity, log, log10, log2, log1p, sqrt, reverse, date, time, hms, asn, atanh, boxcox, exp, logit, modulus, probability, probit, pseudo_log, reciprocal |
| `trans` | `deprecated()` | 旧名，转 `transform` |
| `guide` | `"legend"` | 构造默认；位置构造传 `waiver()` |
| `position` | `"left"` | left/right/top/bottom |
| `fallback.palette` | `NULL` | palette=NULL 且 theme 无对应 palette 时的兜底（per-aesthetic 见 §2.5） |

`ScaleContinuous` ggproto 关键字段：`na.value=NA_real_`、`rescaler=rescale`、`oob=censor`、`minor_breaks=waiver()`、`n.breaks=NULL`、`trans=transform_identity()`。

`ScaleContinuousPosition`（scale-continuous.R L175–211）覆写：
- `map = as.numeric(oob(x, limits))` — **只做 oob，不做 rescale/palette**；oob 后变 NA 的数据被 `na.value` 替换（位置即 NA）。
- `break_info` 叠加 `secondary.axis`。

**break/expand 流水线（渲染像素的决定路径）**：
1. `get_limits()`：数据 range（训练自**变换后**数据）∪ 用户 limits（`NA` 回落数据端点）。
2. `dimension(expand)` = `expand_limits_scale` → `expand_limits_continuous_trans`：**先 `trans$transform(limits)`，在坐标空间做 expand_range4（mult 对变换后宽度的比例 + add 绝对量），再 `trans$inverse` 回 scale 空间**；逆变换出非有限值时该端点回退为未 expand 的 limits（如 sqrt 变换 + 负端点）。
3. `view_scale_primary`（scale-view.R L14–48）：breaks 由 `get_breaks(sort(continuous_range))` 计算，随后 `censor(breaks, range, only.finite=FALSE)` 把 expanded 范围外的刻度删掉。
4. `get_breaks`（scale-.R L1168–1209）：limits 被 **squish 进 transform domain**（#980，防 log10 下限<0）；breaks 函数收 **数据空间** 的 limits（`trans$inverse` 后）；waiver → `trans$breaks`（= `scales::extended_breaks(n=5)`）；结果再 `trans$transform` 回变换空间。
5. `get_labels`：`trans$inverse(breaks)` → waiver 用 `trans$format`（`scales::label_number()` 风格）。
6. `get_breaks_minor`：waiver → `trans$minor_breaks(b, limits, n=2)`（每主刻度间 1 个）；越界 minor 刻度丢弃。
7. `break_info`：主刻度 `oob_censor_any(major, range)` → 刻度与标签按 NA 同步删除，再 `rescale(major, from=range)` → [0,1] 坐标。

### 2.2 `discrete_scale()` — 15 参数（scale-.R L234–319）

| 参数 | 默认 | 语义 |
|---|---|---|
| `aesthetics`/`palette`/`name`/`breaks`/`minor_breaks`/`labels`/`limits`/`expand`/`na.value`/`guide`/`position` | 同上（breaks=waiver 即 limits 本身；limits=字符向量定 level 顺序） | palette(n) 收 level 数 → 输出值 |
| `na.translate` | `TRUE` | 离散 NA 显示（位置 scale 上 NA 永远排最右） |
| `drop` | `TRUE` | 丢弃未出现的 factor level（`FALSE` 保留 factor 全 level，需 `show.legend=TRUE` 才进图例） |
| `fallback.palette` | per-aesthetic | 见 §2.5 |

`ScaleDiscrete` 关键行为：
- `map`：`palette(n)` 一次调用并**缓存**（`n.breaks.cache/palette.cache`）；palette 带名字时按名字匹配 limits，名字不在 limits 内的 palette 项 → na.value。
- `rescale(x)` = `rescale(x, match(as.character(x), limits), from=c(1, n))`。
- `dimension(expand)` = `expand_limits_discrete(map(limits))` → `expand_limits_discrete_trans`：`discrete_limits = c(1, n)` → `expand_limits_continuous_trans(c(1,n), expand)`。**即离散 expand 与连续同一公式，但作用在整数 1..n 上**。
- `get_breaks`：waiver=limits 本身；结果与 domain 取交集并携带 `pos` 属性（供 labels 向量按 pos 对位）；breaks 不在 limits 内的被丢弃。
- `get_labels`：waiver→breaks 字符/名字向量；labels 向量长度不齐时按 `pos` 属性对位。

`ScaleDiscretePosition`（scale-discrete-.R L138–232）覆写：
- 双 range：`range`（离散 level）+ `range_c`（ContinuousRange，收 jitter/boxplot 等**连续位置**数据，用 seq_len palette 后是 1..n 加 offset 的小数值）。
- `train(x)`：离散→range；数值→range_c。
- `reset()`：**不能**重置离散 range（丢值无法恢复），只重置 range_c。
- `map`：离散→`palette(length(limits))[match]`（palette 必须 ≥ n 个数值）；数值→`mapped_discrete`。
- `dimension(expand)` = `expand_limits_scale(self, expand, limits)` → `expand_limits_discrete(map(limits), expand, range_continuous=range_c$range, continuous_limits)`；`continuous.limits` 参数（构造器特有）覆盖连续范围；**同时存在离散与连续范围时：离散部分正常 expand，连续部分用 `expansion(0,0)` 不 expand，再取并集 range**（scale-expansion.R L296–327）。
- 位置构造 `scale_{x,y}_discrete`：`palette=seq_len`、`position="bottom"/"left"`、`continuous.limits=NULL`、`sec.axis=waiver()`。

### 2.3 `binned_scale()` — 17 参数（scale-.R L349–444）

| 参数 | 默认 | 语义 |
|---|---|---|
| 同 continuous 的大部分 | | |
| `oob` | **`squish`** | 分箱 scale 默认压回而非删除 |
| `n.breaks` | `NULL`（位置构造 `scale_{x,y}_binned` 默认 **10**） | 想要的分箱数 |
| `nice.breaks` | `TRUE` | 用 transform 的 nice breaks（个数可能偏离 n.breaks）；FALSE 则严格 `seq(limits, length.out=n.breaks+2)` 去端点 |
| `right` | `TRUE` | 区间右闭（`[a,b)` vs `(a,b]`） |
| `show.limits` | `FALSE` | scale 两端是否作为刻度/分箱边界显示 |
| `guide` | `"bins"` | 位置构造传 `waiver()` |
| `transform` | `"identity"` | |

`ScaleBinned` 关键行为：
- `map`：breaks = `sort(unique(c(limits[1], breaks, limits[2])))` → cut 成 bin → palette 在 **bin 中点** 求值（缓存）。
- `get_breaks`（L1695–1797）：breaks 不命中 limits 时**改写 `self$limits`**（terminal bin 等宽化：new_limits = breaks[1]+(breaks[1]-breaks[2]), …；0 宽数据回退 ±0.05）。这是个带副作用的 getter——实现时要注意。
- `get_breaks_minor()` 恒 NULL。

### 2.4 `datetime_scale()` — date/datetime/time（scale-date.R L324–500）

| 参数 | 默认 | 语义 |
|---|---|---|
| `transform` | `"date"` / `"time"` | transform 对象带日期 breaks/labels |
| `name`/`breaks`/`date_breaks`/`labels`/`date_labels`/`minor_breaks`/`date_minor_breaks`/`timezone` | waiver/NULL | `date_breaks="1 week"` 转成 breaks 函数；labels 格式化 |
| `limits`/`expand`/`oob=censor`/`na.value`/`guide`/`position`/`sec.axis` | 同 continuous | 位置版 super = `ScaleContinuousDate/Datetime` |
| 非 位置 aesthetic（alpha/size/linewidth/colour 的 date 变体） | transform 同上 | palette 同对应家族 |

### 2.5 每 aesthetic 家族的 palette 默认（fallback / 构造默认）

| aesthetic | 离散 fallback（`fallback_palette_discrete`） | 连续 fallback（`fallback_palette_continuous`） | 构造器特有参数 |
|---|---|---|---|
| colour/fill | `pal_hue()`（h=c(0,360)+15, c=100, l=65） | `pal_seq_gradient("#132B43","#56B1F7")` | `na.value="grey50"`（discrete/continuous 构造） |
| alpha | `seq(0.1, 1, n)` | `pal_rescale(c(0.1, 1))` | `range=NULL` |
| linewidth | `seq(2, 6, n)` | `pal_rescale(c(1, 6))` | `range=NULL` |
| linetype | `pal_linetype()`（6 种） | `pal_binned(pal_linetype())` | `scale_linetype(solid…)` 无 |
| shape | `pal_shape()`（6 个 16,17,15,3,7,8） | `pal_binned(pal_shape())` | `solid=NULL`（离散）、`solid=TRUE`（binned） |
| size | `sqrt(seq(4, 36, n))`（面积恒定，半径 2..6） | `pal_area()`（面积比例） | `range=NULL`；`scale_radius range=c(1,6)`（半径线性）；`scale_size_area max_size=6, rescaler=rescale_max` |
| x/y 位置 | `seq_len` | `identity` | `position`、`sec.axis`、`continuous.limits`（discrete）/ `minor_breaks`、`n.breaks`（continuous） |

导出构造器特有默认（自 api_inventory 抽取）：

| 构造器 | 特有参数与默认 |
|---|---|
| `scale_colour/fill_gradient` | `low="#132B43", high="#56B1F7", space="Lab", na.value="grey50", guide="colourbar"` |
| `scale_colour/fill_gradient2` | `low=muted("red"), mid="white", high=muted("blue"), midpoint=0, transform="identity"`（`rescaler=rescale_mid(midpoint)`；lab 端点 = #C21A01 / #2B6CB8? muted 颜色见 scales） |
| `scale_colour/fill_gradientn` | `colours`（必填）, `values=NULL, space="Lab"` |
| `scale_colour/fill_hue` | `h=c(0,360)+15, c=100, l=65, h.start=0, direction=1, na.value="grey50"` |
| `scale_colour/fill_grey` | `start=0.2, end=0.8, na.value="red"` |
| `scale_colour/fill_brewer` | `type="seq", palette=1, direction=1` |
| `scale_colour/fill_distiller` | 同上但 `direction=-1, values=NULL, guide="colourbar"` |
| `scale_colour/fill_fermenter` | 同上但 `direction=-1, guide="coloursteps"` |
| `scale_colour/fill_viridis_{d,c,b}` | `alpha=1, begin=0, end=1, direction=1, option="D"`；c 加 `values=NULL, guide="colourbar"`；b 加 `guide="coloursteps"` |
| `scale_colour/fill_steps / steps2 / stepsn` | steps=gradient 的 binned 版（guide="coloursteps"）；steps2 带 `midpoint=0`；stepsn 带 `colours, values` |
| `scale_*_manual` | `values`（必填，可命名 `c("a"="red")`）, `breaks=waiver()`, `na.value`：colour/fill="grey50"，alpha/shape/linetype/size/linewidth=`NA` |
| `scale_*_identity` | `guide="none"` |
| `scale_colour/fill_qualitative` | `type=NULL`（Okabe-Ito 等），回退 `pal_hue(h,c,l,h.start,direction)` |
| `scale_colour/fill_ordinal` | `type=getOption("ggplot2.ordinal.*")`（ordered factor 默认 viridis_d） |
| `scale_size_area / binned_area` | `max_size=6`，`rescaler=rescale_max` |
| `scale_{x,y}_binned` | `n.breaks=10, nice.breaks=TRUE, right=TRUE, show.limits=FALSE, oob=squish` |
| `scale_{x,y}_continuous` | `position="bottom"/"left"`；`sec.axis=waiver()` |
| `scale_{x,y}_date/datetime/time` | `date_breaks/date_labels/date_minor_breaks/timezone` |
| `scale_{x,y}_log10/sqrt/reverse` | 无新参，= `scale_{x,y}_continuous(transform=transform_*)` |

`scale_type()` 决定自动 scale（scale-type.R）：numeric/integer/double→continuous；factor/character/logical→discrete；ordered→c("ordinal","discrete")；Date→c("date","continuous")；POSIXt→c("datetime","continuous")；hms→"time"；list→identity；default→continuous（带提示）。

---

## 3. 与 jplot 现状的交叉对比

jplot 现状（scale.rs 705 行）：`ScaleSpec` 4 变体（Continuous / DiscreteManual / Manual / Gradient）；`ContinuousScale::train`（expand 对称 [mult,add]、extended_breaks(range,5)、labels 简化格式）、`DiscreteScale`（levels→1..n，range c(0.4, n+0.6)）、`DiscreteColourScale`（hue palette 或 manual 色值；连续 colour 仅 Gradient low/high）。build.rs：per-aes 单 scale（x/y/colour/fill），无多 aes、无 guide 系统、无 coord transform。

| # | 参数 / 机制 | R 默认 | jplot 状态 | 差距 |
|---|---|---|---|---|
| 1 | `expansion()` 4 元向量 | `c(mult_l, add_l, mult_r, add_r)` | **部分** | spec 只有 `[f64;2]`（[mult, add] 两侧对称）；无法表达 `mult=c(0,0.1)`（柱状图标准写法） |
| 2 | 连续默认 expand | `expansion(mult=0.05)` | ✅ | 默认 [0.05, 0] 对称，数值一致 |
| 3 | 连续 breaks 在 expanded range 上算 + censor | view_scale_primary | ✅ | train() 用 expand 后 range 求 breaks；`breaks_in_range` 过滤；语义吻合 |
| 4 | breaks 数 m | transform 默认 5（`extended_breaks(n=5)`） | ✅ | `default_break_count()=5` 固定 |
| 5 | `n.breaks` | NULL | **未** | 无法指定 break 数量 |
| 6 | `minor_breaks` + `get_breaks_minor` | waiver→每主刻度间 1 个 | **未** | 无 minor gridline |
| 7 | `transform`（log10/sqrt/reverse/…） | "identity" | **未** | 无任何变换；无 `scale_y_log10` |
| 8 | expand 在坐标空间做（transform 后） | expand_limits_continuous_trans | —（无 transform，暂不触发） | 实现 transform 时必须一并实现 |
| 9 | `oob` censor/squish | censor（连续位置）/squish（binned） | **未** | 无越界裁剪；limits 语义错位（见坑 2） |
| 10 | position limits = 删数据 vs coord zoom | ScaleContinuousPosition.map | **未** | jplot 的 `limits` 直接当面板范围（≈ coord 行为），数据不裁剪 → 与 R 像素不一致 |
| 11 | 离散默认 expand | `expansion(add=0.6)` → c(0.4, n+0.6) | ✅ | range_for() 实测吻合（n=1→0.4..1.6, n=2→0.4..2.6） |
| 12 | 离散 expand 可定制 + `continuous.limits` | waiver/NULL | **未** | discrete spec 无 expand/limits |
| 13 | 离散+连续混合位置（jitter/boxplot）range_c | discrete expand + continuous 不 expand 取并集 | **未**（build.rs boxplot xmin/xmax 参与连续训练） | boxplot x 训练只含序数 x，未按 R 逻辑并 range_c |
| 14 | `breaks`/`labels` 显式向量 | waiver | ✅（连续位置） | 离散位置无 breaks/labels 覆盖 |
| 15 | `limits`（连续）NA 回落/函数形式 | NULL | **部分** | 仅数值 [a,b]；无 NA 回落、无函数 |
| 16 | `limits`（离散）= level 顺序向量 | NULL | **部分** | DiscreteManual.values 可当 limits 用；但与 palette 命名匹配（`values=c("a"="red")` 命名对位）未实现 |
| 17 | `drop`/`na.translate` | TRUE/TRUE | **未** | 无 factor level 保留、无离散 NA |
| 18 | `na.value` | 颜色 "grey50"、位置 NA_real_ | **未** | NA 数据点直接丢失 |
| 19 | label 格式化 | `scales::label_number()`（precision 推断、`1e+06` 大数、千分位随 style） | **部分** | `format_break` 自研：科学计数为 `x10^6` 且阈值 1e6/1e-4 与 R 精确行为未逐值校准 |
| 20 | `name`（轴标题） | waiver | ✅（字段存在） | 渲染侧是否使用未查证 |
| 21 | `position`（top/right 轴） | bottom/left | **未** | 单轴 |
| 22 | `sec.axis` | waiver | **未** | |
| 23 | `guide` 分发（colourbar/coloursteps/bins/legend/none） | per-family | **未** | 无 guide 系统（图例另议） |
| 24 | 连续 colour 渐变 | `pal_seq_gradient("#132B43","#56B1F7")`（Lab 插值） | **部分** | `Gradient{low,high}` 存在但端点插值空间（Lab vs RGB）、build.rs 连续 colour 路径（`train_colour` 空 levels 分支）尚未接 palette |
| 25 | `gradient2` diverging + `midpoint` | mid="white", midpoint=0, `rescale_mid` | **未** | 无 diverging 色标 |
| 26 | `gradientn`/`viridis_c`（n 色插值） | values=NULL | **未** | |
| 27 | viridis_d / brewer / grey / qualitative / ordinal 默认 | 见 §2.5 | **未** | 离散 colour 只有 hue + manual |
| 28 | alpha / size / linewidth / shape / linetype scale | 各 fallback | **未** | 无独立非位置 scale 对象（alpha/size 硬编码在 geom 绘制常量里） |
| 29 | binned scale 全家（steps/fermenter/…、`scale_x_binned`） | 见 §2.3 | **未** | |
| 30 | date/datetime/time scale | transform=date/time | **未** | |
| 31 | 多 aes 同 scale（`aesthetics=c("colour","fill")`） | 构造参数 | **未** | build.rs 每 aes 一 scale |
| 32 | `zero_range` 数据兜底 | ±0.5/±0.05*? （zero_width=1） | ✅（近似） | train() 里 half=0.5 或 0.1*abs；数值来源与 R `coord` 的 zero-range 处理需 probe 校准 |
| 33 | hue palette HCL→sRGB | pal_hue(h=15+360/n*i, c=100, l=65) | ✅ | 测试对位 #F8766D/#00BFC4 |
| 34 | 离散 palette 缓存（n 变化重算） | palette.cache | 不适用 | jplot 无引用语义问题 |

统计口径（按机制不按 152 函数名）：**已对齐 6 项，部分对齐 5 项，未实现 23 项**。

---

## 4. Top 15 实现优先级（按渲染像素影响排序）

| # | 参数/机制 | R 默认 | jplot 现状 | 像素影响 | 难点 |
|---|---|---|---|---|---|
| 1 | `expand` 4 元（`mult=c(0,0.1)`） | `expansion(mult=0.05)` | [f64;2] 对称 | **高** | 改 spec serde + ContinuousScale::train 两端独立；bar/stack 的标配写法 |
| 2 | `oob` + position limits=删数据 | censor（squish for binned） | 无 | **高** | 需要"先 oob 再映射"管线 + geom 对 NA 的跳过；语义上要区分 scale.limits（删）与 coord.zoom（裁视图） |
| 3 | `transform`（log10/sqrt/reverse） | transform="identity" | 无 | **高** | Transform trait（fwd/inv/breaks/format/Domain）；breaks/labels/expand/coord 全链路联动；sqrt 需 domain [0,∞) |
| 4 | `minor_breaks` | waiver→每主刻度间 1 个 | 无 | **中**（网格线像素直接可见） | `transform_minor_breaks`；主题 minor grid 样式 |
| 5 | `n.breaks` | NULL（trans 默认 5） | 无（m 固定 5） | **中** | extended_breaks 传 m；对已有 extended_breaks 是 1 行改动 |
| 6 | 连续 colour 渐变（Lab 插值 + 默认端点） | `#132B43`→`#56B1F7`（Lab） | Gradient 存在、未接线 | **高**（colour/fill 图整体色） | sRGB↔Lab 转换；build.rs `train_colour` 连续分支接 palette |
| 7 | `na.value` | "grey50" / NA_real_ | 无 | **中**（NA 点从消失变灰色） | map 管线保留 NA 索引 |
| 8 | `gradient2`（diverging + midpoint） | mid="white", midpoint=0 | 无 | **中** | `rescale_mid`；palette 三段 Lab 插值 |
| 9 | `viridis_d/_c/_b` + ordered 默认 | option="D" | 无 | **中** | viridis LUT 常量（8 段 256 采样）；d/c/b 三态复用 |
| 10 | 离散 `limits`/`breaks`/`labels` + `drop=FALSE` | waiver/TRUE | 仅 manual.values | **中** | level 顺序（factor order > 出现序 > manual）已有 order_levels 雏形；breaks 子集过滤 + pos 对位 |
| 11 | `position`（top/right）+ `sec.axis` | bottom/left / waiver | 无 | **中**（双轴布局） | guide/axis 布局系统；sec.axis 可延后 |
| 12 | date/datetime/time 位置 scale | transform=date/time | 无 | **中-高**（时间轴数据直接废） | 日期 breaks（pretty 日历算法）/strftime labels；jplot 数据层尚无日期类型 |
| 13 | label 格式对齐（`scales::label_number`） | precision/`1e+06`/`style_*` | 自研简化 | **中** | 需对 R 逐值 diff（1e3、1e6、0.001、负数、千分位） |
| 14 | binned scale（`steps`/`scale_x_binned`） | oob=squish, n.breaks=10 | 无 | **低-中** | cut 分箱 + terminal-bin 等宽化副作用 + GuideColoursteps |
| 15 | `aesthetics=` 多 aes + `scale_*_manual` 命名值 | `"colour","fill"`；`c("a"="red")` | 无 | **低** | spec 层透传即可（`ScaleSpec` 已有 Manual/DiscreteManual 壳） |

（延后清单：`rescaler` 自定义、`fallback.palette`、theme palette 联动、`guide` 对象系统、`sec.axis`、sf、`pseudo_log/boxcox` 等冷门 transform、`scale_backward_compatibility` 路由——对像素影响小或依赖更大的子系统。）

---

## 5. 最容易踩坑的点

### 坑 1：expand 的 mult=0.05 是对 **limits（变换后）** 做的，而且先 expand 后算 breaks
`expand_limits_continuous_trans`（scale-expansion.R L222–262）：`trans$transform(limits)` → `expand_range4(…, expand)`（mult 相对**变换后**宽度）→ `trans$inverse`。所以 `scale_y_log10` 的 5% padding 是 log 值的 5%（约 ×1.12），**不是数据值的 5%**；sqrt/reverse 同理（reverse 还要处理 expand 后仍需升序的问题，R 里专门 rev() 了）。而 breaks 是在 **expand 后的 dimension** 上算的（view_scale_primary L21–24），随后 `censor(breaks, range)` 删界外刻度——jplot 现状已做对这一点（在 expand 后 range 求 breaks），但一旦实现 transform 必须保持"先变换→再 expand→再求 breaks→再逆变换 labels"的顺序，任何一处顺序颠倒都会整条轴错位。

### 坑 2：position scale 的 `limits` 是**删数据**，不是缩放；oob 在 map 时生效
`ScaleContinuousPosition$map = as.numeric(oob(x, limits))`（scale-continuous.R L182–188）：censor 把界外数据变 NA——像素上这些点**消失**，且靠 oob 的位置在 geom 内插值/路径中断处按 na.rm 处理。zoom（保数据改视图）是 `coord_cartesian(xlim=…)` 的职责。jplot 现状把 scale limits 当作面板范围直接用，等价于 coord 行为：对"用户传 limits=c(2,6)"这类 spec，渲染出的数据集不同。另外 oob 发生在 **rescale 之前**（ScaleContinuous.map L1124：`rescale(oob(x, limits), limits)`），非位置 scale 的 rescaler（如 rescale_mid）作用于 oob 后的值。

### 坑 3：discrete expand 是 **add=0.6 绝对单位**，且与连续混合范围、position_stack 的交互有专门规则
- 离散 expand 不看数据宽度：`discrete_limits=c(1,n)`，`expansion(add=0.6)` 加在两侧（n=2 → 0.4..2.6；n=30 → 0.4..30.6，比例只有 ~2%）。jplot `range_for` 已对。
- 离散+连续混合（boxplot/jitter 把数值 x 画进离散轴）：离散部分正常 expand，**连续部分用 `expansion(0,0)` 不 expand**，最终 range = 两者并集（scale-expansion.R L296–327）。jplot build.rs 目前把 boxplot 的辅助列直接并进连续训练，没有这套并集/不 expand 逻辑。
- `position_stack`：累计在 stat 之后、scale 训练之前完成（ymax 参与 y 训练 → 上限=最大累计值），y 仍是 continuous scale → 默认 mult=0.05，**0 基线下方会悬空 5%**；R 用户惯例 `expansion(mult=c(0,0.1))`，而实现顺序上必须"先 stack 后 train"，且 stack 的 `vjust`/bar 底部对齐依赖 ymin=ymin_stack 保持 0。

### 附：binned scale 的 `get_breaks` 副作用
`ScaleBinned$get_breaks`（scale-.R L1729–1769）在未显式设 limits 且算出的 breaks 不贴合 limits 时，**就地改写 `self$limits`**（terminal bin 等宽化；0 宽数据回退 ±0.05）。任何"无状态求 breaks"的实现会渲染出与 R 不同的分箱边界——实现 binned 时必须复刻这个 mutation 或者按等价顺序重排状态。

### 附：labels 大数格式
R 默认 `scales::label_number()`（precision 推断）对 1e6 量级、千分位、负数、科学计数的切换阈值与 jplot `format_break`（阈值 1e6/1e-4、`x10^6` 写法）有可见差异，属于"每根刻度文字都错"级别，建议在对齐 oob/transform 时一并逐值 diff。

---

## 6. jplot 对齐基线速查（已验证正确的三处）

1. `extended_breaks`：0..10 → 0,2.5,5,7.5,10（labeling::extended 端口，已对 R diff）。
2. 离散面板范围：`c(0.4, n+0.6)`（2 levels→0.4..2.6；3→0.4..3.6，y.range probe）。
3. 连续默认 expand 后 breaks：0..14 → -0.7..14.7，breaks 0,2.5,…（train 注释中的 probe）。
