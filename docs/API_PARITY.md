# ggplot2 API 对齐矩阵 (生成自 api_inventory.json，勿手改)

来源：`compare/extract_api.R` 从已安装 ggplot2 4.0.3 + refs/ggplot2 机械抽取。
✓ = jplot 已实现该参数; · = ggplot2 有、jplot 登记待实现 (spec 已可透传则记 ○)

| ggplot2 | 具名参数 (mapping/data/stat/position 之外) | jplot 覆盖 |
|---|---|---|
| `geom_abline` | `slope`, `intercept`, `na.rm`, `show.legend`, `inherit.aes` | 3/5 |
| `geom_area` | `orientation`, `outline.type`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 4/8 |
| `geom_bar` | `just`, `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/6 |
| `geom_bin2d` | `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/5 |
| `geom_bin_2d` | `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/5 |
| `geom_blank` | `show.legend`, `inherit.aes` | 2/2 |
| `geom_boxplot` | `outliers`, `outlier.colour`, `outlier.color`, `outlier.fill`, `outlier.shape`, `outlier.size`, `outlier.stroke`, `outlier.alpha`, `whisker.colour`, `whisker.color`, `whisker.linetype`, `whisker.linewidth`, `staple.colour`, `staple.color`, `staple.linetype`, `staple.linewidth`, `median.colour`, `median.color`, `median.linetype`, `median.linewidth`, `box.colour`, `box.color`, `box.linetype`, `box.linewidth`, `notch`, `notchwidth`, `staplewidth`, `varwidth`, `na.rm`, `orientation`, `show.legend`, `inherit.aes` | 4/32 |
| `geom_col` | `just`, `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/6 |
| `geom_contour` | `bins`, `binwidth`, `breaks`, `arrow`, `arrow.fill`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/11 |
| `geom_contour_filled` | `bins`, `binwidth`, `breaks`, `rule`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/10 |
| `geom_count` | `na.rm`, `show.legend`, `inherit.aes` | 3/3 |
| `geom_crossbar` | `middle.colour`, `middle.color`, `middle.linetype`, `middle.linewidth`, `box.colour`, `box.color`, `box.linetype`, `box.linewidth`, `fatten`, `na.rm`, `orientation`, `show.legend`, `inherit.aes` | 4/13 |
| `geom_curve` | `curvature`, `angle`, `ncp`, `arrow`, `arrow.fill`, `lineend`, `na.rm`, `show.legend`, `inherit.aes` | 3/9 |
| `geom_density` | `outline.type`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/7 |
| `geom_density2d` | `contour_var`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/7 |
| `geom_density2d_filled` | `contour_var`, `na.rm`, `show.legend`, `inherit.aes` | 3/4 |
| `geom_density_2d` | `contour_var`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/7 |
| `geom_density_2d_filled` | `contour_var`, `na.rm`, `show.legend`, `inherit.aes` | 3/4 |
| `geom_dotplot` | `binwidth`, `binaxis`, `method`, `binpositions`, `stackdir`, `stackratio`, `dotsize`, `stackgroups`, `origin`, `right`, `width`, `drop`, `na.rm`, `show.legend`, `inherit.aes` | 3/15 |
| `geom_errorbar` | `orientation`, `lineend`, `na.rm`, `show.legend`, `inherit.aes` | 4/5 |
| `geom_errorbarh` | `orientation` | 1/1 |
| `geom_freqpoly` | `na.rm`, `show.legend`, `inherit.aes` | 3/3 |
| `geom_function` | `arrow`, `arrow.fill`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/8 |
| `geom_hex` | `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/6 |
| `geom_histogram` | `binwidth`, `bins`, `orientation`, `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 4/8 |
| `geom_hline` | `yintercept`, `na.rm`, `show.legend`, `inherit.aes` | 3/4 |
| `geom_jitter` | `width`, `height`, `na.rm`, `show.legend`, `inherit.aes` | 3/5 |
| `geom_label` | `parse`, `label.padding`, `label.r`, `label.size`, `border.colour`, `border.color`, `text.colour`, `text.color`, `size.unit`, `na.rm`, `show.legend`, `inherit.aes` | 3/12 |
| `geom_line` | `orientation`, `arrow`, `arrow.fill`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 4/9 |
| `geom_linerange` | `orientation`, `lineend`, `na.rm`, `show.legend`, `inherit.aes` | 4/5 |
| `geom_map` | `map`, `na.rm`, `show.legend`, `inherit.aes` | 3/4 |
| `geom_path` | `arrow`, `arrow.fill`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/8 |
| `geom_point` | `na.rm`, `show.legend`, `inherit.aes` | 3/3 |
| `geom_pointrange` | `orientation`, `fatten`, `lineend`, `na.rm`, `show.legend`, `inherit.aes` | 4/6 |
| `geom_polygon` | `rule`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/7 |
| `geom_qq` | `geom`, `distribution`, `dparams`, `na.rm`, `show.legend`, `inherit.aes` | 3/6 |
| `geom_qq_line` | `geom`, `distribution`, `dparams`, `line.p`, `fullrange`, `na.rm`, `show.legend`, `inherit.aes` | 3/8 |
| `geom_quantile` | `arrow`, `arrow.fill`, `lineend`, `linejoin`, `linemitre`, `na.rm`, `show.legend`, `inherit.aes` | 3/8 |
| `geom_raster` | `interpolate`, `hjust`, `vjust`, `na.rm`, `show.legend`, `inherit.aes` | 3/6 |
| `geom_rect` | `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/5 |
| `geom_ribbon` | `orientation`, `lineend`, `linejoin`, `linemitre`, `outline.type`, `na.rm`, `show.legend`, `inherit.aes` | 4/8 |
| `geom_rug` | `lineend`, `sides`, `outside`, `length`, `na.rm`, `show.legend`, `inherit.aes` | 3/7 |
| `geom_segment` | `arrow`, `arrow.fill`, `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/7 |
| `geom_sf` | `na.rm`, `show.legend`, `inherit.aes` | 3/3 |
| `geom_sf_label` | `parse`, `label.padding`, `label.r`, `label.size`, `border.colour`, `border.color`, `text.colour`, `text.color`, `na.rm`, `show.legend`, `inherit.aes`, `fun.geometry` | 3/12 |
| `geom_sf_text` | `parse`, `check_overlap`, `na.rm`, `show.legend`, `inherit.aes`, `fun.geometry` | 3/6 |
| `geom_smooth` | `method`, `formula`, `se`, `na.rm`, `orientation`, `show.legend`, `inherit.aes` | 4/7 |
| `geom_spoke` | `arrow`, `arrow.fill`, `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/7 |
| `geom_step` | `orientation`, `lineend`, `linejoin`, `linemitre`, `arrow`, `arrow.fill`, `direction`, `na.rm`, `show.legend`, `inherit.aes` | 4/10 |
| `geom_text` | `parse`, `check_overlap`, `size.unit`, `na.rm`, `show.legend`, `inherit.aes` | 3/6 |
| `geom_tile` | `lineend`, `linejoin`, `na.rm`, `show.legend`, `inherit.aes` | 3/5 |
| `geom_violin` | `trim`, `bounds`, `quantile.colour`, `quantile.color`, `quantile.linetype`, `quantile.linewidth`, `draw_quantiles`, `scale`, `na.rm`, `orientation`, `show.legend`, `inherit.aes` | 4/12 |
| `geom_vline` | `xintercept`, `na.rm`, `show.legend`, `inherit.aes` | 3/4 |

## scale_* 构造函数 (152)

`scale_alpha` · `scale_alpha_binned` · `scale_alpha_continuous` · `scale_alpha_date` · `scale_alpha_datetime` · `scale_alpha_discrete` · `scale_alpha_identity` · `scale_alpha_manual` · `scale_alpha_ordinal` · `scale_apply` · `scale_backward_compatibility` · `scale_color_binned` · `scale_color_brewer` · `scale_color_continuous` · `scale_color_date` · `scale_color_datetime` · `scale_color_discrete` · `scale_color_distiller` · `scale_color_fermenter` · `scale_color_gradient` · `scale_color_gradient2` · `scale_color_gradientn` · `scale_color_grey` · `scale_color_hue` · `scale_color_identity` · `scale_color_manual` · `scale_color_ordinal` · `scale_color_steps` · `scale_color_steps2` · `scale_color_stepsn` · `scale_color_viridis_b` · `scale_color_viridis_c` · `scale_color_viridis_d` · `scale_colour_binned` · `scale_colour_brewer` · `scale_colour_continuous` · `scale_colour_date` · `scale_colour_datetime` · `scale_colour_discrete` · `scale_colour_distiller` · `scale_colour_fermenter` · `scale_colour_gradient` · `scale_colour_gradient2` · `scale_colour_gradientn` · `scale_colour_grey` · `scale_colour_hue` · `scale_colour_identity` · `scale_colour_manual` · `scale_colour_ordinal` · `scale_colour_qualitative` · `scale_colour_steps` · `scale_colour_steps2` · `scale_colour_stepsn` · `scale_colour_viridis_b` · `scale_colour_viridis_c` · `scale_colour_viridis_d` · `scale_continuous_identity` · `scale_description` · `scale_discrete_identity` · `scale_discrete_manual` · `scale_fill_binned` · `scale_fill_brewer` · `scale_fill_continuous` · `scale_fill_date` · `scale_fill_datetime` · `scale_fill_discrete` · `scale_fill_distiller` · `scale_fill_fermenter` · `scale_fill_gradient` · `scale_fill_gradient2` · `scale_fill_gradientn` · `scale_fill_grey` · `scale_fill_hue` · `scale_fill_identity` · `scale_fill_manual` · `scale_fill_ordinal` · `scale_fill_qualitative` · `scale_fill_steps` · `scale_fill_steps2` · `scale_fill_stepsn` · `scale_fill_viridis_b` · `scale_fill_viridis_c` · `scale_fill_viridis_d` · `scale_flip_axis` · `scale_flip_position` · `scale_linetype` · `scale_linetype_binned` · `scale_linetype_continuous` · `scale_linetype_discrete` · `scale_linetype_identity` · `scale_linetype_manual` · `scale_linewidth` · `scale_linewidth_binned` · `scale_linewidth_continuous` · `scale_linewidth_date` · `scale_linewidth_datetime` · `scale_linewidth_discrete` · `scale_linewidth_identity` · `scale_linewidth_manual` · `scale_linewidth_ordinal` · `scale_override_call` · `scale_radius` · `scale_shape` · `scale_shape_binned` · `scale_shape_continuous` · `scale_shape_discrete` · `scale_shape_identity` · `scale_shape_manual` · `scale_shape_ordinal` · `scale_size` · `scale_size_area` · `scale_size_binned` · `scale_size_binned_area` · `scale_size_continuous` · `scale_size_date` · `scale_size_datetime` · `scale_size_discrete` · `scale_size_identity` · `scale_size_manual` · `scale_size_ordinal` · `scale_type` · `scale_type.Date` · `scale_type.POSIXt` · `scale_type.character` · `scale_type.default` · `scale_type.double` · `scale_type.factor` · `scale_type.hms` · `scale_type.integer` · `scale_type.list` · `scale_type.logical` · `scale_type.numeric` · `scale_type.ordered` · `scale_type.sfc` · `scale_x_binned` · `scale_x_continuous` · `scale_x_date` · `scale_x_datetime` · `scale_x_discrete` · `scale_x_log10` · `scale_x_reverse` · `scale_x_sqrt` · `scale_x_time` · `scale_y_binned` · `scale_y_continuous` · `scale_y_date` · `scale_y_datetime` · `scale_y_discrete` · `scale_y_log10` · `scale_y_reverse` · `scale_y_sqrt` · `scale_y_time`

## position_* 构造函数 (8)

`position_dodge` · `position_dodge2` · `position_fill` · `position_jitter` · `position_jitterdodge` · `position_margin` · `position_nudge` · `position_stack`

## coord_* 构造函数 (12)

`coord_cartesian` · `coord_equal` · `coord_fixed` · `coord_flip` · `coord_map` · `coord_munch` · `coord_polar` · `coord_quickmap` · `coord_radial` · `coord_sf` · `coord_trans` · `coord_transform`

## facet_* 构造函数 (3)

`facet_grid` · `facet_null` · `facet_wrap`

## stat_* 构造函数 (36)

`stat_align` · `stat_bin` · `stat_bin2d` · `stat_bin_2d` · `stat_bin_hex` · `stat_binhex` · `stat_boxplot` · `stat_connect` · `stat_contour` · `stat_contour_filled` · `stat_count` · `stat_density` · `stat_density2d` · `stat_density2d_filled` · `stat_density_2d` · `stat_density_2d_filled` · `stat_ecdf` · `stat_ellipse` · `stat_function` · `stat_identity` · `stat_manual` · `stat_qq` · `stat_qq_line` · `stat_quantile` · `stat_sf` · `stat_sf_coordinates` · `stat_smooth` · `stat_spoke` · `stat_sum` · `stat_summary` · `stat_summary2d` · `stat_summary_2d` · `stat_summary_bin` · `stat_summary_hex` · `stat_unique` · `stat_ydensity`

**参数覆盖率(geom 具名参数): 168/380**  aes 词汇: jplot 51 vs ggplot2 31
