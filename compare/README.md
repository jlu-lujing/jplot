# 对比管线（像素回归的机理）

**为什么可信**：不同光栅器（R 的 cairo vs jplot 的 resvg）的抗锯齿/字体差异会淹没几何差异。因此参考图不用 cairo PNG，而用 **svglite 导出 SVG**，两边再经 **jplot 自己的 resvg + 同一 Arial 字体库**光栅化——残余差异 = 纯几何/坐标差。

```
export_specs.R  ──► refs/*.svg   (svglite, pt→px 归一, linecap butt)
             └──► specs/*.json   (jplot PlotSpec, theme 固定 grey 保证可比)
check_regress.sh ──► ours/*.png + refs_png/*.svg→resvg 光栅化
compare_images.py ──► >16/>32/>64 三级 diff + *_side.png 对比图 + overview
```

- `compare_images.py` 的 `refs/`（cairo 原图）**仅作人眼参照**，不参与 diff。
- `baseline_diffs.txt` 由 `./scripts/check_regress.sh --snap` 冻结；commit 它等于签署新基线。
- 识图：看 `diff/*_side.png`（左 ref 右 ours）。**diff 小 ≠ 对**——语义错误（丢线段、错轴）可能只差 2–3%，必须看图。

新增对比图：`export_specs.R` 里加 `save()+render()` 对（spec 与 ggplot2 代码必须逐参数等价），跑 `--snap` 前先人工识图。
