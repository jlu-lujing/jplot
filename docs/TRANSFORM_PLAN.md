# Transform + expand + oob + na.value 实现方案（TRANSFORM_PLAN）

来源证据：
- ggplot2 4.0.3：`refs/ggplot2/R/scale-.R`、`scale-expansion.R`、`scale-continuous.R`、`scale-colour.R`、`scale-view.R`、`coord-cartesian-.R`、`plot-build.R`
- scales：本地 refs 无该包，取 CRAN **scales 1.4.0**（ggplot2 DESCRIPTION 要求 `scales (>= 1.4.0)`，refs/ggplot2/DESCRIPTION:5）源码核对；文件:行 均指 scales-1.4.0 `R/*.R`（行号与 GitHub main 当前一致，两处都标）。
- jplot 现状：`crates/jplot-core/src/scale.rs`、`build.rs`、`layout.rs`；`docs/SCALE_PARITY.md`。

纯设计文档，不改任何源码。

---

## 1. scales 包各变换的精确语义

`new_transform()` 默认值（scales-1.4.0 `R/transform.R:31–41`，GitHub main `transform.R:31–61`）：

```r
new_transform(name, transform, inverse, ...,
  breaks       = extended_breaks(),          # 默认 breaks：数据空间，n=5
  minor_breaks = regular_minor_breaks(),     # 默认 minor：变换空间，n=2（由 ggplot2 传入）
  format       = format_format(),            # 默认 format：format(x, trim=TRUE, justify="left")
  domain       = c(-Inf, Inf))
```

- `format_format` 定义：`labels-retired.R:156–169`（plain `format()`，非 10^ 表达式）。
- `extended_breaks` = `breaks_extended`，默认 **n = 5**：`breaks.R:92`（main `:105–106`）、别名 `breaks.R:108`。

各变换（`transform-numeric.R`，本地 1.4.0 = main 行号）：

| 变换 | transform | inverse | domain | breaks | minor_breaks | format | 行号 |
|---|---|---|---|---|---|---|---|
| `transform_identity()` | `force` | `force` | `(-Inf, Inf)` | 默认 extended(n=5) | 默认 regular | 默认 | :275–283 |
| `transform_log(base)` | `log(x, base)` | `base^x` | **`(1e-100, Inf)`** | `log_breaks(base)` | 默认 regular | 默认 | :313–324 |
| `transform_log10()` | = `transform_log(10)` | | | | | | :327–329 |
| `transform_log2()` | = `transform_log(2)` | | | | | | :333–335 |
| `transform_reverse()` | `-x` | `-x` | `(-Inf, Inf)` | 默认 extended(n=5)（数据空间） | `regular_minor_breaks(reverse=TRUE)` | 默认 | :454–463 |
| `transform_sqrt()` | `sqrt` | `ifelse(x<0, NA_real_, x^2)` | **`(0, Inf)`** | 默认 extended(n=5) | 默认 regular | 默认 | :477–486 |

### log breaks：在哪个空间算？

**breaks 函数收数据空间 limits，返回数据空间 breaks；内部在 log 空间搜索。** 双重证据：

1. ggplot2 `ScaleContinuous$get_breaks`（scale-.R:1168–1209）：
   - limits（变换空间）先 squish 进 domain：`domain <- transform(trans$domain); limits <- oob_squish(limits, domain)`（:1180–1184，防 log 下限 −Inf，#980）；
   - `limits <- trans$inverse(limits)`（:1193）→ **breaks 函数在数据空间求值**（:1195/1197）；
   - 结果 `trans$transform(breaks)` 变回变换空间（:1208）。
2. `breaks_log`（= `log_breaks`）本身（scales `breaks-log.R:46–81`，main `:52–87`）：
   - `rng <- log(raw_rng, base)`（:55）— 在 log 空间取 `min=floor(rng[1])`、`max=ceiling(rng[2])`；
   - `by <- floor((max-min)/n)+1; breaks <- base^seq(min, max, by=by)`（:63–64）— **整数幂**；
   - 相关 breaks 数 `>= n-2` 即返回（:66）；否则 `by` 递减到 1（:72–76）；
   - 仍不足 → `log_sub_breaks`（:175–207）：贪心加中间步（base=10 即 3、5 的幂次倍数，`outer(base^seq(min,max), steps)`，:194）；
   - 终极兜底：`extended_breaks(n = n)(base^rng)`（:206）— **小跨度退回数据空间线性 breaks**。
   - 默认 `n = 5, base = 10`（:46）。

**n.breaks 默认**：scale 层 `n.breaks = NULL`（scale-.R:112；binned :359）；NULL 时用 transform breaks 函数自身默认 n=5。若用户给 `n.breaks` 而 breaks 函数签名无 `n` 参数（`support_nbreaks`，scale-.R:1934–1939）则 warn 忽略（:1196–1204）。

**minor breaks**：`get_breaks_minor(n = 2, ...)`（scale-.R:1211–1213），waiver → `trans$minor_breaks(b, limits, n=2)`（:1236–1241）——**在变换空间**于主刻度之间插 `n-1=1` 个（`regular_minor_breaks`，scales `minor_breaks.R:65–100`：`seq(a,b,length.out=n+1)[-(n+1)]`，:85–92；reverse 版反向补边界 :78–82）；越界 minor 丢弃（`discard(breaks, limits)`，scale-.R:1261）。

---

## 2. expand：expand_range4 精确公式与流水线顺序

### expansion() 4 元素向量

`expansion(mult, add)`（scale-expansion.R:37–52）→ `c(mult[1], add[1], mult[2], add[2])`（:51），长度 1 自动复制成 2（:49–50）。

### expand_range4（scale-expansion.R:73–93）

- 非有限 limits → `c(-Inf, Inf)`（:80–82）；
- 2 元素旧语法复制成 4（:86–88）；
- 分侧调用 `expand_range(limits, expand[c(1,3)], expand[c(2,4)])`（:92），即下侧用 `(mult_l, add_l)`、上侧用 `(mult_r, add_r)`。
- `scales::expand_range`（scales `bounds.R:352–360`）：

```
width  = if zero_range(range) then zero_width (=1) else hi - lo
lo'    = lo - (width * mult_l + add_l)
hi'    = hi + (width * mult_r + add_r)
```

### default_expansion（scale-expansion.R:105–129）

- `discrete = expansion(add = 0.6)`（:107）、`continuous = expansion(mult = 0.05)`（:108）；
- `scale$expand %|W|% if (scale$is_discrete()) discrete else continuous`（:115–116）——**按 is_discrete 选默认**；
- `expand` 逻辑向量可分别关掉下/上侧（:122–127）。
- 离散路径：`expand_limits_discrete_trans`（:264–328）→ limits=c(1,n) → add=0.6 → `c(1-0.6, n+0.6)`，与 jplot `range_for` 探针一致（scale.rs:571–583）。

### 顺序证据：transform → expand → breaks → inverse

1. **数据先 transform 再进 scale/stat**：`plot-build.R:84` `data <- lapply(data, scales$transform_df)`；`default_transform`（scale-.R:1082–1086）+ `check_transformation` 警告非有限值（scale-.R:1911–1930）。
2. **limits 构造时即存变换空间**：`continuous_scale()` `limits <- transform$transform(limits); sort(limits)`（scale-.R:161–165）。
3. **expand 在变换坐标空间做**：`view_scales_from_scale`（coord-cartesian-.R:196–202）→ `expand_limits_scale`（scale-expansion.R:156–182）→ `expand_limits_continuous_trans`（:222–262）：
   - `continuous_range_coord <- trans$transform(limits)`（:232，limits 已在变换空间，此处对 coord_limits 兜底）；
   - 降序 limits（reverse/reciprocal）：`rev(expand_range4(rev(...)))`（:236–242）；否则直接 `expand_range4`（:244）；
   - `final_scale_limits <- trans$inverse(continuous_range_coord)`（:247）；
   - **inverse 出非有限值 → 该端回退未 expand 的 limits**（:252–256，sqrt+负端经典案例）；
   - 返回 `sort()` 后范围（:258–261）。
4. **breaks 从 expanded range 算**：`view_scale_primary`（scale-view.R:14–48）对 `sort(continuous_range)` 调 `get_breaks`（:22），再 `censor(breaks, range, only.finite=FALSE)` 删越界刻度（:23）；minor 同理（:31）。
5. **labels 最后 inverse**：`get_labels`（scale-.R:1267–1306）：`breaks <- trans$inverse(breaks)`（:1274）→ waiver 用 `trans$format`（:1287）。

> **jplot 现状差距**：`ContinuousScale::train`（scale.rs:489–494）在**数据空间**做对称 expand（mult/add 同值双端），identity 下等价，log/sqrt 下 mult=0.05 应对**变换后宽度**取比例，须挪进变换空间（见 §4）。

---

## 3. oob 行为、limits 超界→NA、na.value 默认差异

### oob 函数（scales `bounds.R:275–334`）

| 函数 | 行为 | 行号 |
|---|---|---|
| `oob_censor(x, range, only.finite=TRUE)` | 越界**有限值**→`NA_real_`；Inf 保留 | :275–283 |
| `oob_censor_any` | 同上但 `only.finite=FALSE`，Inf 也→NA | :285–289 |
| `oob_squish` | 越界有限值压回最近端点 | :299–307 |
| `oob_squish_any` | Inf 也压回 | :309–312 |
| `oob_squish_infinite` | 仅 Inf→最近端点 | :314–322 |
| `oob_keep` | 原样返回（= coord 缩放不删数据） | :324–334 |

### 默认与位置 scale 的"超界→NA"

- 连续构造默认 `oob = censor`（scale-.R:116；`ScaleContinuous` 字段 :1098）；**binned 默认 `squish`**（:358；字段 :1621，非位置 map 处 :1654、breaks 处 `oob_discard` :1726）。
- **位置 scale 的 map 只做 oob 不做 rescale**：`ScaleContinuousPosition$map`（scale-continuous.R:184–190）：

```r
scaled <- as.numeric(self$oob(x, limits))
if (!anyNA(scaled)) return(scaled)
vec_assign(scaled, is.na(scaled), self$na.value)
```

  limits 外的数据 → censor→NA → 替换成 `na.value`（位置默认 NA_real_）→ geom 丢弃该行（快照：tests/testthat/_snaps/scale-continuous.md:1–7 "Removed N rows … outside the scale range"）。
- 刻度同理：`break_info` 用 `oob_censor_any(major, range)`（scale-.R:1327）再删 NA 主刻度及其标签（:1329–1336）；binned 的 limits 先 `oob_squish` 进 domain（:1184）。
- 非位置 `ScaleContinuous$map`（scale-.R:1123–1135）：`rescale(oob(x, limits), limits)` → palette → NA 处 `vec_assign(..., self$na.value)`（:1134）。

### na.value 默认差异

| 位置 | 默认 | 行号 |
|---|---|---|
| `Scale` 基类 | `NA` | scale-.R:543 |
| `ScaleContinuous` | `NA_real_` | :1096 |
| `ScaleDiscrete` | `NA`（且受 `na.translate` 开关控制，:1402/:1418–1421） | :1374 |
| `ScaleBinned` | `NA_real_` | :1619 |
| `scale_x/y_continuous()` 形参 | `NA_real_` | scale-continuous.R:91/:136 |
| colour/fill（离散+连续全系） | **`"grey50"`** | scale-colour.R:84/:119/:154/:189/:260/:294 |

---

## 4. Rust 设计与 scale.rs 最小侵入接入点

### 4.1 数据结构（新，全部 derive `Debug+Clone+Copy+PartialEq+Serialize+Deserialize`，enum tag 风格与 `ScaleSpec` 一致）

```rust
/// scales::transform_* 对应物。用 enum 而非 trait object：可序列化、可 Copy。
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformSpec {
    Identity,
    Log10,
    Log2,
    Log { base: f64 },
    Sqrt,
    Reverse,
}

impl TransformSpec {
    pub fn transform(&self, x: f64) -> f64;   // log: x<=0→NaN（x∈domain 外）; sqrt: x<0→NaN; reverse: -x
    pub fn inverse(&self, t: f64) -> f64;     // base^t; sqrt: t<0→NaN; reverse: -t
    pub fn domain(&self) -> (f64, f64);       // identity/reverse: (-inf,inf); log*: (1e-100,inf); sqrt: (0,inf)
    pub fn breaks(&self, lo_data: f64, hi_data: f64, n: usize) -> Vec<f64>;
        // 一律输入/输出数据空间（对齐 get_breaks 的 inverse→算→transform 三步）
        // log 族 → breaks_log(n=5,base)+log_sub_breaks+extended 兜底；其余 → extended_breaks(n)
    pub fn minor_breaks(&self, majors_t: &[f64], range_t: [f64;2], n: usize) -> Vec<f64>;
        // 变换空间 regular_minor_breaks；reverse 用反向边界版
    pub fn format_label(&self, b_data: f64) -> String; // 复用现有 format_break（对齐 format_format）
}

/// expansion(mult=c(l,r), add=c(l,r)) → {mult_l,add_l,mult_r,add_r}
pub struct ExpandSpec { pub mult_l: f64, pub mult_r: f64, pub add_l: f64, pub add_r: f64 }
impl ExpandSpec {
    pub const DEFAULT_CONTINUOUS: Self = Self { mult_l: 0.05, mult_r: 0.05, add_l: 0.0, add_r: 0.0 };
    pub const DEFAULT_DISCRETE:   Self = Self { mult_l: 0.0,  mult_r: 0.0,  add_l: 0.6, add_r: 0.6 };
    pub fn expansion(mult: [f64;2], add: [f64;2]) -> Self;
    /// scales::expand_range + ggplot2 rev 技巧（在调用方保证 limits 已升序；
    /// reverse 降序场景由 train() 先 rev 再 expand 再 rev）
    pub fn expand_range4(&self, lo: f64, hi: f64) -> (f64, f64);
}

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Oob { Censor, CensorAny, Squish, SquishInfinite, Keep }
impl Oob { pub fn apply(&self, v: f64, range: [f64;2]) -> f64; }
```

### 4.2 ScaleSpec::Continuous 增字段（scale.rs:400–411，全部 `#[serde(default)]` 向后兼容）

```rust
Continuous {
    limits, breaks, labels, name,          // 既有
    #[serde(default, skip_serializing_if="Option::is_none")]
    transform: Option<TransformSpec>,      // None = Identity
    #[serde(default, skip_serializing_if="Option::is_none")]
    expand4:   Option<ExpandSpec>,         // 既有 [f64;2] 旧字段保留 = mult/add 同值双端
    #[serde(default, skip_serializing_if="Option::is_none")]
    oob:       Option<Oob>,                // None = Censor（连续默认）
    #[serde(default, skip_serializing_if="Option::is_none")]
    na_value:  Option<f64>,                // None = NaN（位置 scale：丢弃；colour 侧用 Color 版）
    #[serde(default, skip_serializing_if="Option::is_none")]
    n_breaks:  Option<usize>,              // ggplot2 n.breaks，None → transform 默认 5
}
```

### 4.3 ContinuousScale 增内部状态（scale.rs:450–458）

```rust
pub struct ContinuousScale {
    pub limits: Range,      // 语义改为：**数据空间** limits（inverse 后），供轴标题/调试
    pub range:  Range,      // 数据空间 inverse(变换空间 expand 结果)，非有限端回退未 expand 端
    pub t_limits: Range,    // 新：变换空间 limits
    pub t_range:  Range,    // 新：变换空间 expanded range —— map 用它
    pub transform: TransformSpec,
    pub oob: Oob,
    pub na_value: f64,
    pub breaks: Vec<f64>,   // 存**变换空间**值（与 t_range 同域，censor 后）
    pub labels: Vec<String>,// 对 inverse(breaks) 格式化
    pub minor_breaks: Vec<f64>, // 新：变换空间，供次刻度
    pub name: Option<String>,
}
```

### 4.4 精确公式（train 内，对齐 R 顺序）

```
t_lo,t_hi        = transform(limits)                       // scale-.R:161–165 / :84
（训练数据同样先 transform；非有限值剔除 ≈ ggplot2 的 warning+下游 censor）
t_lo..t_hi 降序（reverse）→ rev 后再 expand（scale-expansion.R:236–242）
width_t          = if zero_range(t) { 1.0 } else { t_hi - t_lo }   // bounds.R:352–360
t_range          = [t_lo - (width_t*mult_l + add_l),
                    t_hi + (width_t*mult_r + add_r)]               // :92 → bounds.R:359
data_range       = inverse(t_range)，非有限端 ← t 端点回退原 limits 端  // expansion.R:247–256
domain_t         = sort(transform(domain))                          // scale-.R:1180–1182
breaks_data      = trans.breaks(inverse(squish(t_range, domain_t))…, n = n_breaks.unwrap_or(5))
                                                                   // :1184,1193–1197
breaks_t         = transform(breaks_data)                            // :1208
breaks_t         = oob_censor_any(breaks_t, t_range) 去 NA           // scale-view.R:23 / scale-.R:1327
labels           = format_label(inverse(breaks_data))                // :1274,:1287
```

### 4.5 scale.rs / build.rs / layout.rs 最小侵入接入点清单

| # | 位置 | 改法 |
|---|---|---|
| 1 | `scale.rs:396–434` `ScaleSpec::Continuous` | 加 6 个可选字段（§4.2）；文件头新增 `TransformSpec/ExpandSpec/Oob` 三类型 + `breaks_log` 算法移植（~150 行，独立可单测） |
| 2 | `scale.rs:463–507` `ContinuousScale::train` | **主改造点**。顺序：spec→transform 训练值与 limits→zero-range pad（变换空间）→ `expand_range4`（含 rev 技巧）→ domain-squish→inverse 算 breaks→transform 回→censor→labels。旧 `expand.unwrap_or([0.05,0.])` 分支保留映射为 `ExpandSpec` 旧语法 |
| 3 | `scale.rs:509–511` `map` | 一行改多行：`let t = tr.transform(v); let t = oob.apply(t, [t_range.min,t_range.max]); if t.is_nan() { return f64::NAN } (t - t_range.min)/(t_range.max - t_range.min)`；调用方（layout.rs `Viewport::t_x`:24–28）**零改动**，NaN 自然被 geom 路径过滤（≈ ggplot2 删行） |
| 4 | `scale.rs:514–521` `breaks_in_range` | 比较域不变（breaks 与 range 同在变换空间），仅需把 `self.range` 换成 `self.t_range`；返回 (变换值→数据值, label)：`inverse(b)` 供画轴位置之外的调试/二次轴 |
| 5 | `build.rs:608–649` `train()` 闭包 | 收集 `cont` 后不动（保持数据空间）——transform 由 `ContinuousScale::train` 内部做（决策见 §5 注）；仅需把 `ScaleSpec` 新字段透传 |
| 6 | `layout.rs:255–264` `axis_breaks` | 不变（labels 已预格式化）；若要画次刻度：新增读取 `cs.minor_breaks` + `cs.map` |
| 7 | `scale.rs:624–630` `DiscreteColourScale::map` 与 `layout.rs:345` `gradient_color` | na.value 侧：miss 时返回 `Color::parse("grey50")`（scale-colour.R:84）而非 black/静默 |

不改动：`DiscreteScale`（离散 expand 语义已由 `range_for` 探针对齐）、hist 分箱（`build.rs:112–140`，见 §5 决策）。

### 4.6 语义决策（与 ggplot2 的已知偏差，写入 doc comment）

- **transform 时机**：ggplot2 在 stat 之前 transform 数据（plot-build.R:84），`scale_x_log10()+geom_histogram()` 的 bin 在 log 空间（bin.R:147 用 `scale$dimension()`）。jplot v1 只在 position scale 训练/映射边界 transform，stats 仍在线性空间 —— log+hist 组合会偏差，明确记录。
- **非有限数据**：log 遇 x≤0 → transform 得 NaN，从训练集中剔除（≈ ggplot2 warning + −Inf/NaN 被 oob censor 成 NA 删行的净效果）；limits 显式含域外值时按 domain squish 处理刻度（get_breaks），**不**改数据映射。
- **expand 作用于变换空间**：这是对现有 identity-only 行为的兼容性收紧，非 identity 时轴范围将首次正确对齐 R。

---

## 5. 验证计划（compare/ 对比图 spec + 预期）

沿用 `compare/export_specs.R` 管线（`spec(..., scales=...)` → `specs/*.json` + `refs/*.png` @72dpi，`compare_images.py` 打分）。新增 3 个 spec：

1. **`21_scatter_log10`** — `ggplot(mtcars, aes(disp, mpg)) + geom_point() + scale_y_log10()`
   预期：y 主刻度 = `breaks_log(n=5)` 在 [~79,~472]：`rng=[1.898,2.674]→min=1,max=3,by=1→{10,100,1000}` 均相关(3 ≥ n-2=3) → 刻度 **10/100/1000 等距分布**（面板上 log 等距），点垂直位置 log 压缩（低 mpg 点更聚拢）；范围外无刻度；expand 5% 作用在 log10 宽度上（≈ 上下各 0.026 个 decade）。
2. **`22_scatter_yreverse`** — `... + scale_y_reverse()`
   预期：y 轴 **max 在下、min 在上**；刻度为对数据空间 limits 做 extended 再取负（如 10,15,20,25,30 反向）；expand 在 −y 空间：`rev(expand_range4)` 后数据离轴上下各 5%（与 identity 图镜像对称）；次刻度若渲染则用 reverse 版 regular_minor_breaks。
3. **`23_scatter_log10_limits`** — `... + scale_y_log10(limits = c(10, 30))`（低于下限的点存在）
   预期：**oob=censor** 生效——mpg<10 或 >30 的点变 NA **整行删除**（对照 R 控制台快照 "Removed N rows … outside the scale range"），面板只画范围内点；刻度只保留 [10,30]（`oob_censor_any(breaks, range)`，scale-view.R:23）；数据空间 limits=[10,30] inverse 后 range 与 limits 相同再各加变换空间 5%。

单元级金样（scale.rs tests，无需渲染）：
- `ExpandSpec::expand_range4`：`[0,14] × mult .05` → `[-0.7, 14.7]`（现有探针保持）；`[1800,2000]` zero_width 分支。
- `breaks_log(5,10)`：`(1,1e3)→{10,100,1000}`；`(2000,9000)→sub-breaks {3000,5000,…}` 邻域；`(1800,2000)` 退回 extended；`n=5` 默认。
- log10 `transform/inverse/domain` 往返；sqrt inverse 负端 → NaN；reverse minor `reverse=TRUE` 边界行为。
