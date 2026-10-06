# jplot — 设计文档

> Rust 实现的 Grammar of Graphics 绘图引擎（ggplot2 风格），对外提供 Python / R 绑定。
> 全生态规划见 `ECOSYSTEM.md`（v2 愿景：官方重建 ggplot2 生态关键包位）。

## 0. 目标与非目标

**目标**
- 分层语法：`data + aes + geom + stat + scale + coord + facet + theme + guides`
- 单内核多前端：Rust core 编译成静态库 / cdylib，Python 与 R 只是薄壳
- 输出：SVG（首选，矢量、可嵌入 notebook）、PNG、PDF、HTML（后续）
- 零拷贝数据接入：Arrow 作为跨语言数据 interchange
- 可复现、可测试：同一 spec 在任何平台产出字节稳定的 SVG

**非目标（v1）**
- 不做交互式图表（plotly 的领域）、不做 WebGL 加速
- 不追求 ggplot2 100% API 兼容；语义对齐即可
- 不做 3D / 地图投影（预留扩展点）

**先行参考**：`gramplot`（Rust 版 grammar of graphics）、Lets-Plot（JetBrains，spec 驱动）、plotnine（Python ggplot2）、`plotters`（Rust，但非分层语法）。这些都不作为依赖，仅作语义参考。

---

## 1. 核心架构决策

### 1.1 Spec-first（关键决策）

Rust 里做 `+` 运算符和可变参数（`aes(x=, y=, colour=)`）很别扭，而跨语言绑定最难的是"重复表达同一套语义"。

解法：**把 plot 定义做成可序列化的 `PlotSpec`（serde + JSON）**。

```
Rust 原生:  jplot::ggplot(df, aes().x("mpg").y("wt")) + geom_point()   ← 构建 PlotSpec
Python:     jp.ggplot(df, jp.aes(x="mpg")) + jp.geom_point()           ← 构建 dict → JSON
R:          jplot(mtcars, aes(x=mpg, y=wt)) + geom_point()             ← 构建 list → JSON
                                        │
                                        ▼
                        jplot-ffi: jplot_render_json(spec, device) → bytes
```

好处：
- FFI 面极小（一个 JSON 入、字节流出），Python/R 绑定不需要复制 core 的类型系统
- spec 可 diff、可存档、可单测、可跨进程/网络发送
- 调试友好：出问题时贴 JSON 即可复现
- Rust 用户仍享受完整类型化 builder；其它语言想接只需实现 JSON 生成器

同时保留 **typed FFI**（handle-based）作为高性能路径：`jplot_plot_new()` → `jplot_add_layer()` → `jplot_render()`，避免大数据反复走 JSON。

### 1.2 数据层：Arrow 而非 polars 依赖

内部列式结构基于 `arrow`（或更轻的 `array2`），提供：
- Python：`pyarrow` → `FFI_ArrowArray` 零拷贝
- R：`nanoarrow` / `arrow` 包 → 同一 FFI 结构
- 可选 feature `polars`：给需要 lazy / join / 复杂 groupby 的统计层用

不自建行列式 `DataFrame`（会重复造 arrow），也不把 polars 做成硬依赖（编译重、绑定侧不需要）。

### 1.3 渲染层：场景图 + 多后端

```
PlotSpec ──build()──▶ Vec<BuildItem> ──layout()──▶ Scene(图元树) ──▶ backend
```
`Scene` 是与输出无关的图元：`Text{content,font,anchor,rot}`、`Path{geom,stroke,fill}`、`Rect`、`Circle`、`Clip`、`Group{opacity}`、`Raster`。

| 后端 | 依赖 | 用途 |
|---|---|---|
| SVG | 自研 writer（`xmlparser`-free，直接写） | 默认，notebook/R Markdown |
| PNG | `tiny-skia`（纯 Rust） | 快速预览、测试基准 |
| PDF | `print`/`genpdf` 或 SVG-in-PDF | 论文出图 |

不用 `skia-bindings`（Skia C++，体积大、交叉编译痛）。字体用 `ab_glyph` + 内置默认字体（DejaVu Sans 子集），保证无系统字体依赖与输出可复现；可选 `fontdb` feature 走系统字体。

文本度量（换行、axis label 占位、legend 对齐）必须在 core 里做，不能让后端临时决定——否则 SVG/PNG 版面会飘。

### 1.4 构建流水线（对齐 ggplot2 `ggplot_build`）

1. **resolve data**：每层数据 + plot 级数据继承，解析 `aes` 列名 → 列向量
2. **stat compute**：`stat_bin`/`stat_identity`/`stat_smooth` … 产出 layer data
3. **position adjust**：`identity`/`dodge`/`dodge2`/`stack`/`fill`/`jitter`
4. **scale train + map**：discrete 序数化、continuous 域→值域、调色板、breaks/labels，产出 guide 数据
5. **coord transform**：`cartesian`/`flip`/`log`/`fixed`，计算 panel 视口
6. **facet layout**：`wrap`/`grid` 分面与 strip 标签
7. **guides/theme**：图例、坐标轴、标题、panel 背景（theme element 继承链）
8. **render**：Scene → 后端

每步都是纯函数 + 显式中间类型，便于分层单测和 snapshot 测试。

---

## 2. Workspace 结构

```
jplot/
├─ Cargo.toml                  # [workspace] members + 统一依赖/版本/lints
├─ crates/
│  ├─ jplot-core/              # 语法与构建，无 IO、无渲染
│  │  ├─ data/     column.rs, dataset.rs, arrow.rs, aes.rs (AesMap, ColumnKey)
│  │  ├─ scale/    scale.rs, continuous.rs, discrete.rs, palette.rs, breaks.rs, transform.rs
│  │  ├─ stat/     stat.rs, bin.rs, smooth.rs, count.rs, ydensity.rs, identity.rs
│  │  ├─ geom/     geom.rs, point.rs, line.rs, path.rs, bar.rs, col.rs, ribbon.rs, text.rs, boxplot.rs, density.rs, histogram.rs, stepfun.rs
│  │  ├─ position/ position.rs, dodge.rs, stack.rs, jitter.rs, identity.rs, nudge.rs
│  │  ├─ coord/    coord.rs, cartesian.rs, flip.rs, trans.rs
│  │  ├─ facet/    facet.rs, wrap.rs, grid.rs, strip.rs
│  │  ├─ guide/    guide.rs, legend.rs, axis.rs, colorbar.rs
│  │  ├─ theme/    theme.rs, element.rs, spec.rs (继承链), defaults.rs
│  │  ├─ build/    plot.rs (Plot/Layer/Spec), build.rs (流水线), layout.rs, scene.rs (图元), mtext.rs (文本度量)
│  │  └─ error.rs  thiserror
│  ├─ jplot-render/            # feature: svg / png / pdf
│  ├─ jplot-ffi/               # C ABI: handle registry + jplot_render_json
│  ├─ jplot-python/            # PyO3 + maturin；产出 python/ 包
│  └─ jplot-r/                 # extendr；产出 R/ 包 + NAMESPACE
├─ bindings/
│  ├─ python/  jplot/ (__init__.py, ggplot.py, aes.py, geom.py, scale.py, display.py, pyproject.toml)
│  └─ r/       jplot/ (R/, man/, tests/testthat/, DESCRIPTION)
├─ examples/                   # rust 示例 + gallery（同时用于视觉回归）
├─ data/                       # 内置示例数据集（mpg, diamonds 等价物；自采或公有领域，勿照抄 ggplot2 数据）
└─ docs/
```

先只做 `core` + `render` + `python`，`r` 与 `ffi` 在 M3 进入。

---

## 3. Rust API 草案

```rust
use jplot::prelude::*;

let p = ggplot(data)
    + aes(&[("x", col("mpg")), ("y", col("wt")), ("colour", col("cyl"))])
    + geom_point()
    + geom_smooth(Method::Loess)
    + scale_colour_viridis(Option::D)
    + labs("Weight vs MPG", colour = "Cylinders")
    + theme_bw()
    + facet_wrap(["cyl"]);

let svg = p.render(Device::svg(800, 500))?;   // Vec<u8>
p.save("fig.png", 300.dpi())?;
```

设计要点：
- `impl Add<Layer> for Plot` / `Add<Theme>` …：复刻 `+` 手感，`Plot` 保持 `Clone + Debug + Send + Sync`（图层内部 `Arc<dyn Geom>`）
- `aes()` 只接受列名字符串 / `col()` 表达式，**不引入 DSL 求值器**（v1）；`after_stat(count)` 用 `stat("count")` 显式写法
- `Geom`/`Stat`/`Scale`/`Position`/`Coord`/`Facet` 均为 trait + `#[enum_dispatch]`（比 `Box<dyn>` 快，仍可 object-safe 化给 spec 用）
- 所有 trait 实现 `#[typetag::serde]` 或 `#[serde(tag="kind")]` 手工 enum 判别，使 `PlotSpec` 与类型化 API 等价

Theme 继承链（`element_tree` 语义）：`elem.text → elem.title → elem.axis.text → elem.axis.text.x`，用 `Option<T>` 逐层 `or()` 合并；这是 ggplot2 最容易被低估的复杂度，v1 就要设计对，否则返工。

---

## 4. 绑定层

### Python（PyO3 + maturin）
```python
import jplot as jp, pandas as pd
df = pd.read_parquet(...)
p = jp.ggplot(df, jp.aes(x="mpg", y="wt", colour="cyl")) + jp.geom_point() + jp.theme_bw()
p.save("f.png", dpi=300); p          # Jupyter: _repr_svg_()  →  零配置富展示
```
- DataFrame 接入：`pyarrow`（zero-copy）；`pandas`/`polars` 走 `.to_arrow()`
- 渲染策略：默认惰性——`_repr_svg_` 时才 render，尺寸从 `display.max_img_size` / 用户 `width=` 取
- `__add__` 返回新对象（不可变），与 Rust 侧一致
- 分发：`maturin` 打 abi3 wheel（`manylinux`/`musllinux`/`macos arm+x86`/`win`）

### R（extendr）
```r
library(jplot)
jplot(mtcars, aes(x = mpg, y = wt, colour = factor(cyl))) + geom_point() + theme_bw()
```
- `aes()` 用 NSE 捕获列名（`substitute` → deparse），转成 spec 字符串
- 数据：`data.frame` → `nanoarrow` 导出 → 零拷贝入 Rust；避免在 R 侧复制大表
- S3 `print.jplot` / `knit_print` 输出 SVG ⇒ R Markdown、Shinystudio、Plots pane 天然可用
- 尺寸：沿用 R 图形设备尺寸；提供 `ggsave`-风格的 `jpsave()`
- 打包：`extendr` 生成 `.Call` 胶水，CRAN 需 `cargo vendor` + `Makevars` 编静态库（评审重点，成本不低）

### C ABI（jplot-ffi）
```c
JpError* jplot_last_error(void);
int32_t  jplot_render_json(const char* spec_json, const char* opts_json,
                           JpBuffer** out);              // 通用无状态路径
uint64_t jplot_plot_from_arrow(const char* spec_json, uintptr_t ffi_array);
int32_t  jplot_plot_add_json(uint64_t handle, const char* layer_json);
int32_t  jplot_plot_render(uint64_t handle, const char* opts_json, JpBuffer** out);
void     jplot_buffer_free(JpBuffer*);
```
错误经 `JpError{code,message}` 返回，绑定层各自转 Python `RuntimeError` / R `rlang::abort`。

---

## 5. 质量与工程约定（对齐全局 CLAUDE.md）

- 全类型 `Debug + Clone + Send + Sync`；值语义即所有权转移
- 覆盖率 ≥80%/模块：
  - `insta` snapshot：Scene 与 SVG 文本
  - `proptest`：scale 逆变换往返、dodge 不重叠且等宽、bin 计数和 = n、palette 色域合法
  - 视觉回归：`examples/gallery` 渲染 PNG，`image` crate 逐像素带阈值比对（golden 入库）
- `cargo clippy -D warnings` + `rustfmt`；workspace 级 `[lints]`
- 数据/字体等资源随 crate 内置（`include_bytes!`），保证跨机可复现
- 并行开发：worktree agent 各自 commit 不 push，合并后 main 上跑 `cargo test`；`crates/*` 按模块划归，不交叉改文件
- 独立实现，不复制 ggplot2/R 源码；短函数名语义对齐（`geom_point` 等）

## 6. 里程碑

| 阶段 | 交付 | 验收 |
|---|---|---|
| **M1 骨架** | workspace、`PlotSpec`、`ggplot`+`aes`、`geom_point/line/bar`、linear+discrete scale、SVG+PNG、默认字体、`theme_grey` | `cargo test`；散点/折线/柱状 3 个 gallery 图 |
| **M2 语法完备** | stat(bin/smooth/count/ydensity)、position(dodge/stack/jitter)、facet wrap/grid、color+fill+size scale+viridis/manual、guides+legend、theme 继承链、coord_flip/log | snapshot 全绿；gallery ≥15 图 |
| **M3 跨语言** | `jplot-ffi` C ABI、Python(maturin, wheel, `_repr_svg_`)、R(extendr, nanoarrow, S3 print) | 三语言同 spec 渲染出同一张图（像素级一致） |
| **M4 发布** | PDF/HTML、CI 三平台 wheel + R tarball、`cargo-release`、文档站 | 公开可安装 |

## 7. 待定决策（需确认）

1. `arrow` vs `array2` 作为内部列式实现？（倾向 `arrow`：生态与 FFI 现成）
2. PNG 用 `tiny-skia` 还是让 `plotters` 只做光栅？（倾向 `tiny-skia`，自研 Scene→draw）
3. 是否 v1 就支持 `after_stat()` / 表达式求值？（倾向 v1 不支持，v2 引入 `jplot-expr`（`datafusion-expr` 或 `bacon_string` 级别））
4. R 侧是否需要 `ggplot2` 对象转换入口（`as_jplot(ggobj)`）？还是仅平行 API
5. 示例数据集来源与许可（不可直接搬 ggplot2 的 `mpg`/`diamonds`）

## 8. 扩展与注册 API（Geom/Stat/Coord 开放）

> 结论来自附录 A 的生态调研：ggplot2 的垂直生态（ggtree、ggsci、EnhancedVolcano…）几乎全部长在它的 6 个 ggproto 扩展点上，core 本身从不做大而全。jplot 要能被生信等垂直领域接受，扩展点必须是**公开、稳定、可注册**的，而不是内部 trait。

### 8.1 第一方扩展（Rust 编译期）

核心 6 trait 全部公开，配注册宏，注册后即可参与 spec 序列化：

```rust
pub trait Geom: typetag::serde {           // Stat / Scale / Position / Coord / Facet 同构
    fn required_aes(&self) -> &[AesKey];
    fn draw_layer(&self, data: &LayerData, resolved: &ResolvedAes,
                  panel: &Viewport, out: &mut dyn Painter) -> Result<()>;
}

// 第三方 crate 一行注册，jplot 的 `geom_xxx()` 工厂与 JSON spec 同时生效
jplot::register_geom!(VolcanoGeom);        // spec 中 kind = "volcano"
```

- `Coord` 特别要求可自定义投影（见 8.3），其余 trait 语义对齐 ggplot2 的 `Geom`/`Stat`/`Position`/`Scale`/`Facet`。
- 内置图型与第三方图型走同一条代码路径（自举验证扩展点是否够用）。

### 8.2 动态扩展（插件 crate，`jenite`/`libloading`，v2）

面向"发二进制而不重编译宿主"的场景（类似 R 装包）。ABI 只暴露 `register_*` + 6 个 vtable trait；MVP 阶段不做，但 trait 设计时就按 object-safe 约束，避免 v2 返工。

### 8.3 为 ggtree 类图形预留的能力

ggtree（系统发育树 + 多层注释 + 径向布局）是对抽象压力最大的真实用例，要求：

1. `Coord` 支持非线性投影（笛卡尔 ↔ 径向/三角树布局），`CoordRadialTransform` 作为内置示例实现验证接口
2. 多层数据注释沿树叠加 = 已有 per-layer `data` + `Coord` 共享即可覆盖
3. 自定义 `Scale`（如进化距离色标）注册

只要 8.1 的 trait + 8.3 的 coord 投影做对了，ggtree/gggenes 级别的图可在 Rust 或第三方绑定侧复现，无需改 core。

### 8.4 绑定侧扩展策略

Python/R 侧 v1 只开放**spec 级**扩展：用户用本语言实现 `compute_stat(spec) -> data` / `draw(geom_json, painter)` 回调，经回调式 FFI 注入（`jplot_register_stat_py(callback)`）。不做完整 ggproto 式"任何语言任意扩展"——成本高、可复现性差；R 侧真正的垂直生态留给社区在 jplot-R 之上封装（如 `jplot_volcano()`、`scale_colour_nature()` 只是预设 spec 的函数）。

### 8.5 内置的"生态级"默认件

调研中反复出现、值得 core 直接提供的最小集（避免每个用户自己造）：
- 期刊配色 scale：`scale_colour_nature()/science()/cell()/lancet()`（ggsci 风格，配色需自研或选公有领域色板，不可抄 ggsci 数据）
- `geom_label_repel()`（ggrepel 式防重叠标签，生信火山图刚需）
- 显著性星号 / p 值区间标注辅助函数
- `after_stat` / `after_scale` 表达式（见 §7.3，v2）
- volcano、GSEA running-score、enrichment dotplot 作为 `examples/gallery` 案例图（不 core 化，证明扩展点足够即达标）

## 附录 A：ggplot2 生态调研（2026-10）

> 网络受限，基于既有知识整理；包名与定位可靠，个别包的新旧版本状态以 CRAN/Bioconductor 为准。

### A.1 扩展机制

ggplot2 以 `ggproto` 暴露 6 个扩展点：`Geom` / `Stat` / `Scale` / `Position` / `Coord` / `Facet`。第三方包通常只实现其中一两个即可造新图型——这是其生态爆炸的直接原因，也是本设计 §8 的依据。

### A.2 tidyverse 通用配套

- 数据管道：dplyr、tidyr、forcats、scales
- 拼接出版：patchwork、cowplot、ggpubr::ggarrange
- 标签防重叠：ggrepel（生信标配）、directlabels、ggtext（HTML/Markdown 富文本标签）、ggfittext
- 主题/配色：ggthemes（JAMA/Economist/Stata 风格）、ggsci（Nature/Science/Cell/Lancet/NEJM 期刊配色）、viridis/scico、ggdark
- 几何增强：ggforce（圆/弧/样条/zoom）、ggridges（山脊图）、ggbeeswarm、ggbump、ggstar（显著性星号）
- 动画/交互：gganimate、plotly::ggplotly、ggiraph

### A.3 生物信息学垂直生态（Bioconductor 为主）

| 方向 | 代表包 | 说明 |
|---|---|---|
| 富集分析 | clusterProfiler + enrichplot | dotplot、cnetplot、emapplot、GSEA ridge plot、enrichment map |
| 差异表达火山图 | EnhancedVolcano、ggvolcano | ggplot2 + repel + 阈值注释，DESeq2/edgeR 结果标准展示 |
| 单细胞 | Seurat（DimPlot 即 ggplot2）、dittoSeq、scater、SCpubr、iSEE | R scRNA-seq 生态几乎全部输出 ggplot 对象 |
| 系统发育/宏进化 | ggtree、ggtreeExtra、gggenes、ggpicrust2、treeio | ggplot2 扩展最成功范例：自定义极坐标/径向 + 多层 geom 注释 |
| 序列/比对 | ggmsa、ggseqlogo | Biostrings 数据直连 |
| 基因组轨迹 | ggbio（GRanges autoplot、circos/ideogram）；Gviz/trackViewer 为 grid 自绘非 ggplot2 | |
| 流式细胞 | ggcyto（flowCore 生态）、CytoExploreR | |
| 微生物组 | phyloseq（plot_ordination/plot_richness）、ANCOMBC、MicrobiomeProfiler | |
| 蛋白质组/代谢组 | DEP、MSnbase、pRoloc、MetaboAnalystR、normr | |
| GSEA/通路 | fgsea(plotEnrichment)、ReactomePA、ggkegg | |
| 生存/临床 | survminer(ggsurvplot)、gtsummary | |
| 贝叶斯诊断 | bayesplot、tidybayes、ggdist（区间/等级图） | |

### A.4 关键启示

1. 这些包全部**复用 ggplot2 对象**（`+` 组合输出 ggplot 对象），不各自实现渲染 → 印证 spec-first + 可扩展 trait 路线。
2. 生信高频需求 = 差异化清单：volcano+repel 标注、GSEA running-score 曲线、enrichment dotplot（size+colour+离散分类多 aesthetic 同时用）、ggtree 式径向多层坐标、期刊配色预设。
3. "core 小而扩展点开"的路径已被验证 20 年；jplot 的 `Coord` 投影抽象是扩展点里最关键的约束（§8.3），必须在 M2 前设计对。
