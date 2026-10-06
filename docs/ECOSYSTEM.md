# jplot 全生态规划（v2 愿景）

> 本文档基于 DESIGN.md（core 设计），把"重写整个 ggplot2 生态"作为目标展开：core 之外，官方重建 ggplot2 生态中被验证过的每一个关键包位。策略不变——**core 永远小而稳，一切垂直功能长在扩展点上**；区别只是这次扩展点上长的东西由我们自己官方实现。

## 1. 生态地图总览

```
                      ┌─────────────────────────────────────┐
  用户                │  bindings:  jplot-py   jplot-r      │
                      ├─────────────────────────────────────┤
  垂直生态            │  jplot-bio   jplot-phylo  jplot-clin │
  (ggplot2 生态位)    │  jplot-heat  jplot-sci  jplot-repel  │
                      ├─────────────────────────────────────┤
  通用增强            │  jplot-force jplot-compose jplot-anim│
                      │  jplot-text  jplot-stats            │
                      ├─────────────────────────────────────┤
  工具链              │  jplot-cli  jplot-wasm  jplot-lsp   │
                      ├─────────────────────────────────────┤
  Core                │  jplot-core  jplot-render  jplot-ffi│
                      └─────────────────────────────────────┘
```

原则：
1. **生态包只准依赖 `jplot-core` 的公开扩展 API**（§8 of DESIGN.md），禁止 `#[path]` 式私有心智依赖；core 的 CI 用全部生态包做下游编译测试（防止 trait 破坏性变更）。
2. 每个生态包 = 一个 Rust crate + 自动派生的 spec 工厂函数（`kind` 注册）+ 三语言薄绑定 + 至少 3 个 gallery 图。缺任何一项不许进 workspace。
3. 配色/数据集不抄 ggplot2/ggsci/ComplexHeatmap 的数据本体，只对齐语义；函数名对齐生态惯例（`jplot_volcano`、`scale_colour_nature`）。

## 2. 包位清单（对照 ggplot2 生态逐个重写）

### 2.1 通用增强层

| jplot 包 | 对标 | 内容 | 备注 |
|---|---|---|---|
| `jplot-repel` | ggrepel | 标签/锚点防重叠（模拟退火 + quadtree，Rust 里比 R 快 1~2 个数量级） | 生信火山图刚需，生态优先级 P0 |
| `jplot-compose` | patchwork / cowplot / ggpubr::ggarrange | 多图拼接：`/`、`|`、`+` 运算符、主题继承、共享 legend、tag、brise 布局 | spec 层新增 `LayoutSpec` 节点，可嵌套 |
| `jplot-sci` | ggsci + ggthemes + viridis/scico | 期刊配色（Nature/Science/Cell/Lancet/NEJM/JAMA…，色值自采或公开色板）、经典主题（Economist/Stata/Minerals） | P0：绑定侧最先能感知的差异化 |
| `jplot-text` | ggtext + ggfittext | 富文本标签（HTML 子集/Markdown → Scene 图元）、自动缩字 | 依赖 core 的文本度量层 |
| `jplot-force` | ggforce + ggridges + ggbump + ggbeeswarm | 圆/椭圆/弧/样条、zoom 焦点区域、山脊图、bump、beeswarm | 纯 Geom/Stat 扩展 |
| `jplot-anim` | gganimate | 补间 + 缓动 + `transition_states()/manual()`，输出 GIF/MP4/WebM（`image`+`ffmpeg` sidecar）| Scene 时序化：`render_frame(t)` 是 core 的确定性函数 |
| `jplot-stats` | ggstats + tidybayes + ggdist | 半小提琴/eye plot/CCDF 区间图、boot CI、多种平滑（loess/gam/quantile） | 统计扩展全部走 `Stat` trait |
| `jplot-annotate` | ggstar + ggsignif + rectify | 显著性星号、p 值括号、brackets、箭头族 | P1，生信高频 |

### 2.2 生物信息学垂直层（本项目差异化主战场）

| jplot 包 | 对标 | 内容 |
|---|---|---|
| `jplot-bio` | EnhancedVolcano + enrichplot + fgsea::plotEnrichment | `volcano`（log2FC×−log10p，repel 标注内置）、GSEA running-score 曲线与 ridge、enrichment dotplot（size+colour+离散分类）、cnet/emap 简化版 |
| `jplot-phylo` | **ggtree + ggtreeExtra + treeio + gggenes** | 树数据模型（newick/nexus/phyloxml 读入 → `Tree` 类型）、rectangular/slanted/radical/fan 布局（作为 `Coord` 投影实现）、多层注释轨道、geom_tree/geom_tiplabel/geom_cladelab/geom_fan、基因特征箭头图（gggenes） |
| `jplot-heat` | ComplexHeatmap + pheatmap + htmly | 复杂热图：行/列聚类轨道、discrete+continuous 色标、多热图拼接、side bar——**grid 自绘体系，spec 里作为复合 `CompositeSpec` 组件实现**，与 core 共享 scale/theme/字体 |
| `jplot-flow` | ggcyto | 流式细胞：flowJo 风格密度脊流图、gates 叠加（FCM 解析 feature，可后置） |
| `jplot-genome` | ggbio(autoplot) + trackViewer 的 ggplot 化 | GRanges 式区间轨道：exon/CNV/coverage track、ideogram、circos 简化（复用 phylo 的径向 coord） |
| `jplot-scrna` | Seurat DimPlot/FeaturePlot + dittoSeq + SCpubr | embedding 散点（UMAP/t-SNE 由调用方算好传入）、feature/violin/ridge split、DotPlot、marker heat；10x/AnnData 列约定适配器 |
| `jplot-clin` | survminer + gtsummary | KM 曲线+风险表（风险表作为 panel 下附组件）、森林图、ROC |

依赖关系：`jplot-bio`/`jplot-scrna`/`jplot-clin` 依赖 `jplot-repel`；`jplot-genome` 径向部分依赖 `jplot-phylo` 的 coord；其余互相独立。

### 2.3 工具链层

| 包 | 说明 |
|---|---|
| `jplot-cli` | `jplot render fig.json -o fig.svg`；管道友好，spec 即文件格式；`jplot watch` 重渲染 |
| `jplot-wasm` | core 编译 wasm32 → 浏览器内实时渲染 SVG + 在线 spec playground（文档站交互来源）；为 HTML 交互后端（hover/zoom）铺路 |
| `jplot-lsp` | spec JSON 的 language server：schema 补全、错误高亮（编辑器体验即"RStudio 对 ggplot2"的替代） |
| `jplot-datasets` | 内置示例数据集（公有领域/自生成）：mpg/diamonds 等价物 + 生信样例（DE 结果、GSEA、树、UMAP 坐标），绑定侧 `jp.data.mpg()` 直接拿 |
| `jplot-convert` | R 侧 `as_jplot(ggobj)` 转换器（ggplot2 对象 → spec，覆盖常用 geom/scale；不可转的显式报错）——降低迁移成本的关键 |

### 2.4 语言侧包

- **Python**：`jplot`（core）、`jplot.bio`、`jplot.heatsc`（热图）、`jplot.compose`；DataFrame 适配（pandas/polars/pyarrow/AnnData）；`pyproject` 入口点注册第三方 `jp_geom` 插件（Python 侧 spec 级扩展，§8.4 of DESIGN.md）
- **R**：`jplot`、`jplotScale`（期刊配色）、`jplotBio`（volcano/GSEA/dotplot）、`jplotTree`（ggtree 风格 API：`jstree(newick) %>+% geom_cladelab(...)`）、`jplotHeat`；全部 S3 `print`→SVG；`as_jplot.ggplot2.ggplot` 迁移入口
- R 侧 API 哲学：签名尽量兼容 ggplot2（`aes`/`labs`/`theme`/`ggsave` 同名同参），让"换 import 不换代码"成为可行迁移路径。

## 3. Workspace 扩展结构

```
jplot/
├─ crates/                      # core 三件套 + ffi（DESIGN.md §2）
├─ ecosystem/
│  ├─ jplot-repel/    jplot-sci/        jplot-compose/    jplot-text/
│  ├─ jplot-force/    jplot-anim/       jplot-stats/      jplot-annotate/
│  ├─ jplot-bio/      jplot-phylo/      jplot-heat/       jplot-scrna/
│  ├─ jplot-genome/   jplot-clin/       jplot-flow/
│  └─ jplot-datasets/
├─ tools/
│  ├─ jplot-cli/    jplot-wasm/    jplot-lsp/
├─ bindings/  (python/ r/ 各自是 mini-monorepo，按包发 wheel/CRAN tarball)
├─ gallery/                     # 跨包统一画廊：每包 ≥3 例，CI 像素回归
└─ docs/
```

版本策略：workspace 统一 `version = "0.x.y"` 一次性 bump（同 core 同生态锁步发版），trait 面 1.0 前允许破坏性变更但 changelog 强制写迁移提示。

## 4. 重写里程碑（取代 DESIGN.md §6，core 部分不变）

| 阶段 | 交付 | 验收 |
|---|---|---|
| M1–M2 | 不变：core 语法完备 + snapshot | 同 DESIGN.md |
| M3 | ffi + Python/R 双绑定 + `jplot-sci`(期刊配色/主题) + `jplot-repel` + `jplot-compose` + `jplot-cli` | 三语言同 spec 同图；notebook/R Markdown 可发布；ggplot2 用户可感知"能用" |
| M4 | **生态第一波**：`jplot-bio`(volcano/GSEA/dotplot) + `jplot-heat` + `jplot-annotate` + `jplot-stats` + `jplot-datasets` + R `as_jplot` 转换器 | 一篇真实 DE 分析文章的 4 张主图全部 jplot 出品（gallery 收录流程文档） |
| M5 | **生态第二波**：`jplot-phylo`(树+径向 coord) + `jplot-scrna` + `jplot-genome` + `jplot-clin` + `jplot-text` + `jplot-force` | ggtree 风格多层注释图可复现（抽象压力测试通过） |
| M6 | **生态第三波 + 平台**：`jplot-anim` + `jplot-wasm`(playground/HTML 交互) + `jplot-lsp` + `jplot-flow` + 插件 ABI（jenite 动态注册）+ 发布 CRAN/PyPI/cargo | 生态包全部 ≥80% 覆盖；CI 含生态包下游兼容矩阵 |

每波结束 core 冻结一次 trait 面（semver 承诺），为插件 ABI 做准备。

## 5. 治理与风险

- **范围爆炸**是本规划唯一真正的风险。防线：core 里程碑（M1–M3）不许夹带生态功能；生态包各自独立 crate、独立 CI job，任一延期不阻塞主线发布。
- trait 面稳定性：每个生态包 PR 必须附"用到了哪些扩展点"清单；积累 ≥3 个反例才改 core，且走 RFC issue。
- 人力：波次制交付，用户可提前用已发布波次（M3 起即有完整可用的通用绘图库）。
- 许可：生态包统一 `MIT OR Apache-2.0`；配色数据/示例数据逐个登记来源与许可（`jplot-datasets/LICENSES.md`）。
- 与现有生态共存而非替代：R 绑定优先兼容层做厚（同名 API + 转换器），Python 侧接受与 matplotlib/plotnine 并存，差异化立足点是**性能（repel/热图/树布局 Rust 化）+ 跨语言像素一致 + 单二进制部署**。
