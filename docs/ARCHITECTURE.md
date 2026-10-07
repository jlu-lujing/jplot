# jplot 架构与方法论

> 状态：v0 内核成型。本文是模块划分、数据管线、验证方法与已知偏差的**唯一权威说明**，随重构同步更新。

## 1. 设计基线

- **目标**：对齐 R ggplot2 4.0.3 的 Grammar of Graphics 语义（aes/stat/scale/position/coord/theme/guides），功能与参数一致；视觉默认采用 **jplot 自有风格**（`ThemeKind::Jplot`：白底、Okabe-Ito 色盲安全离散色板、viridis 连续色、水平浅网格、黑字），ggplot2 的 grey/bw/minimal/classic 保留为可选项。
- **验收**：统一光栅器（resvg + Arial 字体库）下，42 张 ggplot2 参考图逐张 **< 2%**、整体均值 **< 1%**（当前 0.391%）。
- **铁律**：每次代码变更必须过 `./scripts/check_regress.sh`。纯重构要求与冻结基线（`compare/baseline_diffs.txt`）byte-identical（ZERO-CHANGE）；功能新增过均值门禁并更新基线。
- **识图不可省**：像素 diff 会漏判语义错误（segment 事故：坏图也"填满面板"只有 2.63%，但第三段线段已丢）。diff 数字只是线索，**>1% 或 PANEL-MISMATCH 必须看图**（`compare/diff/*_side.png`）。

## 2. 模块管线

```
spec.rs (PlotSpec JSON DSL)
  └─ build.rs            编排: Dataset→Frame→stat→position→scale 训练→BuiltPlot
       ├─ data.rs        Column/Range（f64 数值 + 因子 levels）
       ├─ stat.rs        bin_breaks_bins/width (R bin.R 移植), count, boxplot
       ├─ position.rs    group 识别 + dodge/stack/jitter
       ├─ scale.rs       ContinuousScale/DiscreteScale/NumScale/DiscreteColourScale
       │    ├─ breaks.rs Wilkinson extended_breaks + label_number 格式化
       │    └─ color.rs  Color/HCL(LCh-uv)/Lab(D65) 渐变/hue/grey 调色板
       ├─ theme.rs       主题（含 jplot 风格 + okabe_ito/viridis）
       └─ layout.rs      Viewport/gutter/轴/标题装配 → scene.rs
            ├─ geom.rs   28 个 per-geom 画家 + glyph/linetype
            └─ guides.rs 图例构建与绘制（含连续色条 bar guide）
  → scene.rs（设备像素矢量场景，层序 = grob 树）
  → jplot-render（SVG writer / resvg PNG；SVG 亦是展示层的出口）
  → jplot-cli / jplot-ffi（Python/R 绑定在计划中）
```

## 3. 知识体系（体系化的产出）

| 资产 | 位置 | 作用 |
|---|---|---|
| **探针常数** | `core/src/probes.rs` | 所有 ggplot2/svglite 校准值 + 测法出处，重校准收敛到单文件 |
| **回归门禁** | `scripts/check_regress.sh` + `compare/baseline_diffs.txt` | 渲染→同光栅器 diff→与冻结基线比对 |
| **方法论文档** | `compare/README.md` | 对比管线（svglite→resvg 为什么成立） |
| API 对齐矩阵 | `docs/API_PARITY.md` + `api_inventory.json` + `compare/extract_api.R` | 参数级覆盖率追踪，缺口=路线图 |
| 领域规划 | `docs/SCALE_PARITY.md`、`docs/TRANSFORM_PLAN.md`、`docs/GEOM_PARAM_AUDIT.md` | scale/transform/geom 参数级设计 |

### 单位事实（勿再重复推导）
- 画布 72dpi ⇒ **pt ≡ px**。ggplot2 `size`/`linewidth` 单位是 **毫米**；换算族常数在 `theme.rs::geom_defaults`（svglite 实测：grid 0.53、geom 1.07、点描边 0.71 @ 0.5mm）。
- 点半径：`r = 16/15·size_mm + stroke_px/2`（area_pal：`size=lo+(hi−lo)·√t`，range 1..6mm）。
- 离散 scale 面板范围 `c(1−0.6, n+0.6)`；连续 `mult=0.05` 作用于**变换空间**宽度。
- linetype 为 grid lty 语义：digit 串是 on/off 对（`22`,`42`,`1343`），unit = `stroke·4/3`。
- 默认连续色 = `#132B43→#56B1F7` **Lab** 插值（非 sRGB、非 viridis——那是 jplot 风格的选择）。

## 4. 已知偏差登记（不追，但必须知道）

1. **svglite ↔ cairo 固有差**（2026-10 修正）：svglite **有**双层网格（major 1.07px + minor 0.53px），已双层实现；resvg **严格执行** `textLength`+`lengthAdjust`，刻度标签已按 R stringWidth 钉死。剩余文字差 = 字形 AA，结构性 ~0.1–0.3%。
2. **Wilkinson 刻度 tie-break**：extended_breaks 的浮点并列解偶有选择不同（如 freqpoly 标签 "2.0" vs "2"），导致 gutter 差 1–2px；24_freqpoly 另有 stat-bin 扩展端点差异（freqpoly 双侧空 bin 规则未完全对齐，残差 ~1.8%，单图低优先）。
3. **jitter RNG** 无法与 R 的 Mersenne-Twister 对齐：散点散布的随机位置不可逐位复现；离散图验证用固定 seed 或关闭。
4. **重算法保真度**（loess/hexbin/contour）：按"大体一致"验收，允许亚像素/轻微数值差。

5. **饱和底色放大 AA 残差**（44_thm_bg）：网格线的亚像素位置漂移在灰底上低于阈值，在纯蓝 panel 上白线 AA 混合差被放大到 ~1%；视觉一致（side 图核对），属字形/线 AA 类的已知结构性残差。

## 4b. 主题参数体系（对齐 ggplot2 `theme()`）

`ThemeSpec::Custom { base, elements }` = ggplot2 `theme_*() + theme(...)`：
- **元素命名与 R 完全一致**（`panel.grid`、`axis.text`、`legend.position`、
  `panel.background`…），值形态来自 R 导出（`#RRGGBBAA` 颜色、`{"type":"str"}`
  位置、jsonlite 的 null 三形态 `{}`/`{"type":"null"}`/null = `element_blank()`）；
- 继承链近似实现：`text` 为底 → `axis.text`/`axis.title`/`plot.title` merge；
  未识别元素静默忽略（与 ggplot2 宽容行为一致）；
- 已支持元素：`text`(size/colour/face/angle, base_size 联动)、`axis.text`、
  `axis.title`、`plot.title`、`panel.background`(fill)、`panel.border`(blank)、
  `panel.grid*`(blank/colour)、`axis.ticks`(blank)、`plot.background`、
  `legend.position`(none/left/top/bottom/right)；
- 验证：43(nogrid)/44(蓝 panel)/45(axis.text)/46(legend=none) 全 <1.1%。

## 5. 约定

- **R-first**：任何几何/默认值改动前，先用 `Rscript` 探针（`layer_data`/`ggplot_build`/svg 几何 grep）取真值，禁止从源码推断后直接写死。
- Frame 列命名：映射后统一物化到 canonical aes 名（`size`/`alpha`/`shape`/`linetype`），派生列 `xmin/xmax/ymin/ymax`；**scale 范围训练必须覆盖全部派生列**（xend/yend 事故）。
- `aes` 参数与 `...` 参数分流以 `aes_registry.rs` 为唯一词表；未知键一律进 `GeomArgs` 泛型 map。
