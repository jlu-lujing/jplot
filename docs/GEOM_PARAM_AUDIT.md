# jplot geom 具名参数（非 aes）生效审计

> 纯静态审计，未修改任何源码。基线：`refs/ggplot2` @ 4.0.3。
> 审计对象：`crates/jplot-core/src/spec.rs` 中 `GeomSpec` 已实现的 6 个 geom，
> 及其在 `build.rs`（compute_stat / 宽度与 position）和 `layout.rs`（draw_*）
> 中实际读取的参数。与 `api_inventory.json`、`docs/API_PARITY.md` 交叉核对。
>
> 图例：✓ = 已生效；◐ = 部分生效；✗ = 被忽略（未读取）。

---

## 0. 结论速览

- jplot 只实现了 **6 个 geom 变体**：`point` / `line` / `col` / `bar` / `histogram` / `boxplot`。
- `GeomSpec::Point { shape }` 的 **`shape` 字段是死字段** —— 全仓库没有任何地方解构读取它
  （`layout.rs:298` 用 `{ .. }` 丢弃，`build.rs:594` 同样）。`draw_points` 无条件画圆。
- 渲染层存在**结构性缺口**，导致一批参数即便被读取也无法渲染：
  - `scene::Line` 只有 `color + width` 两个字段，**没有 linetype/dash**；
    `jplot-render` 也从不输出 `stroke-dasharray`（`crates/jplot-render/src/lib.rs` 全文无 dash）。
    → 所有 `linetype` 相关参数（含 boxplot 的 4 个组件 linetype）全部不可实现。
  - `draw_points` 只会 `Primitive::Circle`，没有 glyph 概念 → `shape`/`stroke`/`fill` 无落点。
  - 没有 arrow 图元 → `arrow`/`arrow.fill` 无落点。
  - Polyline 渲染不输出 `stroke-linecap`/`stroke-linejoin`（`Segment` 只写死 `linecap=butt`），
    所以 `lineend`/`linejoin`/`linemitre` 无落点；且 polyline 的 SVG 默认 `linejoin=miter`
    与 ggplot2 `GeomPath` 默认 `linejoin="round"` **不一致**（默认即错，非仅参数被忽略）。
- `aes_registry.rs:39-60` 的 `GEOM_PARAMS` 登记了 **25 个从未被消费的名字**：
  `na.rm, show.legend, inherit.aes, orientation, shape, stroke, linetype, lineend, linejoin,
  linealpha, just, binwidth, boundary, center, closed, {box,whisker,median,staple}.linetype,
  label, parse, nudge_x, nudge_y, check_overlap, draw_baseline`。
  这些在 `API_PARITY.md` 里可能被算作"覆盖"，但实际不影响像素。
- 另一个**默认值错误**：boxplot 宽度。ggplot2 `GeomBoxplot$default_aes$width = 0.9`
  （`refs/ggplot2/R/geom-boxplot.R:447`），经默认 `position_dodge2(padding=0.1)` 后
  有效宽度约 `0.9*0.9 = 0.81`；jplot `build.rs:742` 用 `unwrap_or(0.75)`，
  且 compare spec 用的是 `position=identity`。每张箱线图都会系统性偏窄。

---

## 1. 已实现的 GeomSpec 变体

来源：`crates/jplot-core/src/spec.rs:35-51`

| GeomSpec 变体 | 字段 | 对应 ggplot2 | 渲染入口 |
|---|---|---|---|
| `Point { shape: Option<f64> }` | `shape`（**死字段**） | `GeomPoint` | `layout.rs:355 draw_points` |
| `Line` | — | `GeomLine`（也复用为 path 语义） | `layout.rs:375 draw_lines` |
| `Col` | — | `GeomCol` / `GeomBar` | `layout.rs:406 draw_bars` |
| `Bar` | — | `GeomBar` + `StatCount` | `layout.rs:406 draw_bars` |
| `Histogram { bins }` | `bins` | `GeomBar` + `StatBin` | `layout.rs:406 draw_bars` |
| `Boxplot` | — | `GeomBoxplot` | `layout.rs:442 draw_boxplot` |

构造器：`spec.rs:393-440`。compare 目录仅覆盖这 6 个（`compare/specs/01..10`）。
`API_PARITY.md` 里出现的 errorbar / smooth / violin / tile / text / density 等
**并未实现**，其"覆盖"来自 `aes_registry::GEOM_PARAMS` 的字符串登记，不是可渲染 geom。

---

## 2. 全局/共享具名参数

ggplot2 `layer()`（`refs/ggplot2/R/layer.R:87-99`）把所有 `...` 分成三堆：
- 命中 aesthetic 名的 → 常量 aes（`colour, fill, size, linewidth, linetype, alpha, shape, stroke`）；
- 命中 `geom$parameters()` 的 → geom 参数；
- 命中 `stat$parameters()` 的 → stat 参数。

`na.rm` 单独处理（`layer.R:88`，默认 `FALSE`），`show.legend` / `inherit.aes` 在 layer 层消费。

| 参数 | R 默认 | jplot 状态 | 位置/说明 |
|---|---|---|---|
| `na.rm` | `FALSE` | ✗ | 无任何 `bool_("na.rm")`；NA 行被 draw_* 静默跳过（像素无差，仅缺 warning） |
| `show.legend` | `NA` | ✗ | 图例只看 colour/fill scale 是否存在，无法用 `FALSE` 关闭 |
| `inherit.aes` | `TRUE` | ◐ | `LayerSpec.inherit_aes` 字段生效（`build.rs:78`）；但 `args` 里的 `inherit.aes` 键被忽略 |
| `orientation` | `NA` | ✗ | 全仓库无读取；line 永远按 x 排序，bar/hist/box 永远竖直 |
| `alpha` | `NA`(不透明) | ✓ | `point_colour_of`/`fill_colour_of`（`layout.rs:306,328`）；boxplot 箱体不读 |
| `colour`/`color` | 各 geom 不同 | ◐ | point/line/bar 生效；boxplot 只认 `box.colour` 等专用名 |
| `fill` | 各 geom 不同 | ◐ | bar/hist 生效；point 恒用 colour 填充；boxplot 箱体恒白 |
| `size` | 各 geom 不同 | ◐ | 仅 point 读（`layout.rs:356`）；line 已迁移 linewidth；boxplot 走 `outlier.size` |
| `linewidth` | 主题 0.5mm | ◐ | line/boxplot 生效；bar/col/hist 的描边宽度写死 0.5mm |
| `linetype` | solid | ✗ | **渲染层无 dash，无法实现** |
| `shape` | pointshape(19) | ✗ | point 恒圆；`GeomSpec::Point.shape` 死字段 |
| `stroke` | borderwidth 0.5mm | ✗ | point 恒无描边 |

---

## 3. 逐 geom 审计

### 3.1 `geom_point`（`refs/ggplot2/R/geom-point.R`）

`draw_panel` 读取：`shape, colour, fill, alpha, size, stroke`。
`default_aes`：`shape=19, colour=#000, fill=NA, size=1.5mm, alpha=NA, stroke=0.5mm`。
无 `setup_params`。

jplot 读取点：`layout.rs:355-373`（`draw_points`）+ `layout.rs:305`（`point_colour_of`）。

| 参数 | R 默认 | jplot | 影响 |
|---|---|---|---|
| `size` | 1.5mm | ✓ | `f64_("size")` |
| `colour`/`color` | `#000000` | ✓ | `colour_(["colour","color"])` |
| `alpha` | `NA` | ✓ | 应用于填充色 |
| `shape` | `19` | ✗ | **恒画圆**；连 typed 字段都不读 |
| `fill` | `NA` | ✗ | 恒 `fill=colour`，shape 21–25 的填充无效 |
| `stroke` | 0.5mm | ✗ | 圆无描边 |
| `na.rm` / `show.legend` | — | ✗ | 见全局 |

> `fill`/`stroke` 单独设通常无可见差（因为 shape 也是 19），但 `geom_point(shape=21,
> fill="red", colour="black", stroke=2)` 这类组合完全失真。

### 3.2 `geom_line`（`refs/ggplot2/R/geom-path.R` `GeomLine`/`GeomPath`）

`draw_panel` 读取：`arrow, arrow.fill, lineend="butt", linejoin="round", linemitre=10, na.rm`；
aes：`colour, linewidth, linetype, alpha`。
`setup_data`：按 `PANEL, group, x` 排序。`extra_params = c("na.rm","orientation")`。
`default_aes`：`colour=ink, linewidth=主题, linetype=主题(solid), alpha=NA`。

jplot 读取点：`layout.rs:375-404`（`draw_lines`）。

| 参数 | R 默认 | jplot | 影响 |
|---|---|---|---|
| `linewidth` | 0.5mm | ✓ | `f64_("linewidth")` |
| `colour`/`color` | `#000000` | ✓ | 逐 group |
| `alpha` | `NA` | ✓ | — |
| `linetype` | `solid` | ✗ | 渲染层无 dash |
| `lineend` | `"butt"` | ✗ | 粗线端帽无变化 |
| `linejoin` | `"round"` | ✗ | **默认 miter≠round**，粗线拐角可见 |
| `linemitre` | `10` | ✗ | 尖角阈值 |
| `arrow` / `arrow.fill` | `NULL` | ✗ | 无箭头图元 |
| `orientation` | `NA` | ✗ | 无法画水平/翻转折线；且 jplot 恒按 x 排序（`geom_path` 原始顺序语义也缺失） |
| `na.rm` / `show.legend` | — | ✗ | 见全局 |

> 注：`geom_path` 的"按数据行序连接"和 `geom_line` 的"按 x 排序"在 jplot 里是同一实现，
> 无法区分；`orientation="y"` 亦不支持。

### 3.3 `geom_col` / `geom_bar`（`refs/ggplot2/R/geom-bar.R` + `geom-rect.R`）

`setup_data`（`geom-bar.R:25-45`）：`width`（`default_aes$width=0.9`）、
`just = params$just %||% 0.5`、`xmin = x - width*just`、`xmax = x + width*(1-just)`、
`ymin/ymax = pmin/pmax(y,0)`。
`draw_panel`（`geom-rect.R:56-109`）读取：`lineend="butt", linejoin="mitre"`；
aes：`colour=NA, fill=col_mix(ink,paper,0.35)≈#595959, linewidth=0.5mm, linetype=solid, alpha=NA`。
`extra_params = c("just","na.rm","orientation")`。

jplot 读取点：`build.rs:566-586`（宽度/xmin/xmax/ymin）+ `layout.rs:406-440`（`draw_bars`）。

| 参数 | R 默认 | jplot | 影响 |
|---|---|---|---|
| `width` | `0.9` | ✓ | `build.rs:568` |
| `fill` | `#595959` | ✓ | `fill_colour_of` |
| `alpha` | `NA` | ✓ | 作用于填充 |
| `colour`/`color` | `NA`(无边框) | ◐ | 只决定"是否描边"+描边色；**描边宽度写死 0.5mm** |
| `linewidth` | 0.5mm | ✗ | 边框恒 0.5mm（`layout.rs:434`） |
| `linetype` | `solid` | ✗ | 渲染层无 dash |
| `just` | `0.5` | ✗ | 柱恒居中（`build.rs:571-572` 写死 ±w/2） |
| `lineend`/`linejoin` | butt/mitre | ✗ | 矩形无落点 |
| `orientation` | `NA` | ✗ | 无法画水平柱 |
| `na.rm` / `show.legend` | — | ✗ | 见全局 |

> `geom_bar` 与 `geom_col` 共用 `draw_bars`，区别仅在 `build.rs` 的默认 stat。
> `GeomBar` 的 `pmin/pmax(y,0)` 语义 jplot 用 `ymin=0` 近似（负值柱会画错方向）。

### 3.4 `geom_histogram`（`refs/ggplot2/R/geom-histogram.R` + `stat-bin`）

`geom_histogram` 把 `binwidth, bins, orientation` 传给 `stat_bin`；
`stat_bin` 参数：`binwidth, bins=30, center, boundary, closed="right", pad=FALSE, breaks`。
几何部分完全复用 `GeomBar`（见 3.3）。

jplot：`bins` 经 typed `GeomSpec::Histogram{bins}` → `StatSpec::Bin`（`build.rs:197-230`）
生效；`StatSpec::Bin.breaks` 存在但 **没有从 `GeomArgs` 到它的路径**。

| 参数 | R 默认 | jplot | 影响 |
|---|---|---|---|
| `bins` | `30` | ✓ | typed 字段（`args` 里的 `bins` 键反而被忽略） |
| `binwidth` | `NULL` | ✗ | 改分箱宽度/数量，**整张直方图变化** |
| `center` | `NULL` | ✗ | 平移分箱网格 |
| `boundary` | `NULL` | ✗ | 平移分箱网格 |
| `breaks` | `NULL` | ✗ | 显式分箱边界（仅显式 `StatSpec::Bin.breaks` 可用） |
| `closed` | `"right"` | ✗ | 边界归属（半个 bin 的计数） |
| `pad` | `FALSE` | ✗ | 两端空 bin |
| `just` | `0.5` | ✗ | 同 bar |
| `colour`/`linewidth`/`linetype` | 同 bar | ◐/✗ | 同 3.3 |
| `orientation` | `NA` | ✗ | 同 bar |

> `default_bins`（`build.rs:115`）只实现了 ggplot2 4.x 的 `bins=` 路径，
> 未实现 `binwidth=` / `center=` / `boundary=` 三条优先级分支。

### 3.5 `geom_boxplot`（`refs/ggplot2/R/geom-boxplot.R` + `geom-crossbar.R`）

`draw_group` 读取（`geom-boxplot.R:307-434`）：
`lineend="butt", linejoin="mitre", fatten=2, outlier_gp, whisker_gp, staple_gp,
median_gp, box_gp, notch=FALSE, notchwidth=0.5, staplewidth=0, varwidth=FALSE, flipped_aes`。
各 `*_gp` 读取 `colour, linetype, linewidth`；`outlier_gp` 读取
`colour, fill, shape, size, stroke, alpha`（默认 shape 19 / size 1.5mm / stroke 0.5mm）。
`setup_data` 读 `width`（`default_aes$width=0.9`）、`varwidth`、`outliers`。
`default_aes`：`colour=col_mix(ink,paper,0.2)=#333, fill=paper(白), linewidth=borderwidth,
linetype=bordertype, alpha=NA, width=0.9, weight=1`。
`setup_params`：`fatten`（默认 2，已 deprecated，映射到 `median.linewidth`）。

jplot 读取点：`build.rs:736-765`（width/varwidth/outliers）+ `layout.rs:442-554`。

| 参数 | R 默认 | jplot | 影响 |
|---|---|---|---|
| `notch` | `FALSE` | ✓ | `bool_("notch")` |
| `notchwidth` | `0.5` | ✓ | — |
| `staplewidth` | `0` | ✓ | — |
| `varwidth` | `FALSE` | ✓ | `build.rs:743` |
| `outliers` | `TRUE` | ✓ | `FALSE` 时删离群点 |
| `outlier.colour`/`.color` | `NULL` | ✓ | — |
| `outlier.fill` | `NULL` | ✓ | — |
| `outlier.shape` | `19` | ◐ | 仅支持 0/1/2/17/21，其余回落圆 |
| `outlier.size` | `1.5mm` | ✓ | — |
| `outlier.stroke` | `0.5` | ✓ | — |
| `outlier.alpha` | `NULL` | ✓ | — |
| `box.colour`/`.color` | `NULL` | ✓ | — |
| `box.linewidth` | 继承 | ✓ | — |
| `whisker.colour`/`.color` | `NULL` | ✓ | — |
| `whisker.linewidth` | 继承 | ✓ | — |
| `staple.colour`/`.color` | `NULL` | ✓ | — |
| `staple.linewidth` | 继承 | ✓ | — |
| `median.colour`/`.color` | `NULL` | ✓ | — |
| `median.linewidth` | 继承 | ✓ | — |
| `width` | `0.9`(→有效≈0.81) | ✗(错) | jplot 用 `0.75`（`build.rs:742`），系统性偏窄 |
| `fatten` | `2` | ✗ | 硬编码 `median_lw = lw*2`；默认一致，显式 `fatten=` 无效 |
| `box.linetype` | 继承 | ✗ | 渲染层无 dash |
| `whisker.linetype` | 继承 | ✗ | 同上 |
| `median.linetype` | 继承 | ✗ | 同上 |
| `staple.linetype` | 继承 | ✗ | 同上 |
| `colour`（通用） | `#333` | ✗ | 只认 `box.colour` 等专用名；`geom_boxplot(colour="blue")` 无效 |
| `fill`（通用） | 白 | ✗ | 箱体恒白（`layout.rs:510,505`）；`geom_boxplot(fill="red")` 无效 |
| `alpha`（通用） | `NA` | ✗ | 箱体恒不透明；只读 `outlier.alpha` |
| `linewidth`（通用） | 0.5mm | ✓ | 作为 box/whisker/staple 基准，median×2 |
| `orientation` | `NA` | ✗ | 无法画水平箱线图 |
| `lineend`/`linejoin` | butt/mitre | ✗ | 无落点 |
| `na.rm` / `show.legend` | — | ✗ | 见全局 |

> 注意 `geom_boxplot(fill=...)` / `colour=...` 是最常见的用法之一，jplot 完全忽略，
> 但 compare spec 09/10 只用了 `box.colour`/`median.colour` 等专用名，所以现有对比没暴露。

---

## 4. Top 20 未生效参数（按"改一张对比图能否立刻看出差别"排序）

排序依据：像素改变幅度 × 常用度 × 演示成本。`结构` 表示需先补渲染层能力。

| # | 参数 | geom | R 默认 | jplot | 一眼可见度 | 备注 |
|---|---|---|---|---|---|---|
| 1 | `shape` | point | `19` | ✗ | ★★★★★ | 换整个标记字形；连 typed 字段都未读 |
| 2 | `binwidth` | histogram | `NULL` | ✗ | ★★★★★ | 重排全部 bin，直方图整体变样 |
| 3 | `linetype` | line | `solid` | ✗ 结构 | ★★★★★ | 实线↔虚线；渲染层无 dash |
| 4 | `just` | bar/col/hist | `0.5` | ✗ | ★★★★☆ | 柱子整体左/右移半个宽度 |
| 5 | `center` | histogram | `NULL` | ✗ | ★★★★☆ | 平移分箱网格 |
| 6 | `boundary` | histogram | `NULL` | ✗ | ★★★★☆ | 平移分箱网格 |
| 7 | `breaks` | histogram | `NULL` | ✗ | ★★★★☆ | 任意分箱边界 |
| 8 | `colour`（通用） | boxplot | `#333` | ✗ | ★★★★☆ | 整条箱线换色（专用名才有用） |
| 9 | `fill`（通用） | boxplot | 白 | ✗ | ★★★★☆ | 箱体填色 |
| 10 | `linewidth` | bar/col/hist | `0.5mm` | ✗ | ★★★★☆ | 有 `colour` 时边框粗细 |
| 11 | `fill` | point | `NA` | ✗ | ★★★☆☆ | shape 21–25 的填充（需 #1 一起） |
| 12 | `stroke` | point | `0.5mm` | ✗ | ★★★☆☆ | shape 21–25 的边框（需 #1 一起） |
| 13 | `alpha`（通用） | boxplot | `NA` | ✗ | ★★★☆☆ | 箱体透明度 |
| 14 | `orientation` | line/bar/hist/box | `NA` | ✗ | ★★★☆☆ | 整图翻转为水平 |
| 15 | `box.linetype` | boxplot | 继承 | ✗ 结构 | ★★★☆☆ | 箱体虚线 |
| 16 | `whisker.linetype` | boxplot | 继承 | ✗ 结构 | ★★★☆☆ | 须线虚线 |
| 17 | `median.linetype` | boxplot | 继承 | ✗ 结构 | ★★★☆☆ | 中位线虚线 |
| 18 | `arrow` / `arrow.fill` | line | `NULL` | ✗ | ★★★☆☆ | 折线加箭头 |
| 19 | `lineend` | line | `"butt"` | ✗ | ★★☆☆☆ | 粗线端帽 round/square |
| 20 | `show.legend` | 全部 | `NA` | ✗ | ★★☆☆☆ | `FALSE` 隐藏图例 |
| — | `linejoin` | line | `"round"` | ✗ | ★★☆☆☆ | 默认 miter≠round，粗线拐角已可见 |
| — | `fatten` | boxplot | `2` | ✗ | ★☆☆☆☆ | 默认一致，仅显式值失效 |
| — | `pad` | histogram | `FALSE` | ✗ | ★☆☆☆☆ | 两端空 bin |
| — | `closed` | histogram | `"right"` | ✗ | ★☆☆☆☆ | 边界归属 |
| — | `linemitre` | line | `10` | ✗ | ★☆☆☆☆ | 尖角阈值 |
| — | `na.rm` | 全部 | `FALSE` | ✗ | 无 | 仅影响 warning，不影响像素 |

---

## 5. 建议的修复顺序（研究结论，未实施）

1. **补渲染层基础能力**（否则一整批参数无处落地）：
   `scene::Line` 增加 `linetype`（dash 数组）并在 renderer 输出 `stroke-dasharray`；
   Polyline 输出 `stroke-linecap`/`stroke-linejoin`/`stroke-miterlimit`；
   增加点 glyph（至少 shape 0–25 的圆/方/三角/十字族）。
2. **point**：读 `shape`（含 typed 字段）、`fill`、`stroke`。
3. **histogram**：把 `binwidth/center/boundary/breaks/closed/pad` 接入 `StatSpec::Bin`。
4. **bar/col/hist**：读 `just`、描边 `linewidth`/`linetype`。
5. **boxplot**：通用 `colour`/`fill`/`alpha` 下沉为默认，专用名覆盖；修正 `width` 默认 0.75→0.9
   （并核对 dodge2 padding）；`fatten` 兼容。
6. **全局**：`orientation`、`show.legend`、`na.rm`。

## 6. 核对方法

- `spec.rs:35-51`（GeomSpec）、`spec.rs:134-172`（GeomArgs 访问器）。
- `build.rs:155-318`（compute_stat）、`build.rs:566-595 / 736-773`（宽度、varwidth、outliers、position）。
- `layout.rs:305-343`（colour 解析）、`layout.rs:355-554`（四个 draw_*）。
- `refs/ggplot2/R/{geom-point,geom-path,geom-bar,geom-rect,geom-histogram,geom-boxplot,geom-crossbar,layer}.R`。
- `api_inventory.json`（`geom_ctor_args` 只列 geom 专属参数，不含共享 aes 常量）。
- `crates/jplot-render/src/lib.rs`（确认无 dash/linejoin 输出）。
