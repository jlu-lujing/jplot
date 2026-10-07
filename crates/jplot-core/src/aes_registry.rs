//! Aesthetic registry — the single source of truth for every aesthetic name
//! jplot's pipeline recognises (data→stat, stat→geom, geom→draw) and every
//! geom `...` parameter (ggplot2's per-geom extra args). Cross-checked against
//! `api_inventory.json` (extracted from ggplot2 itself) by
//! `tests/api_parity.rs`, so "does jplot understand this ggplot2 knob" is a
//! mechanical yes/no, not a memory test.

/// Aesthetics flowing data → stat → geom (ggplot2 `aes()` vocabulary).
pub const AES: &[&str] = &[
    // positions & range endpoints
    "x", "y", "xmin", "xmax", "ymin", "ymax", "xend", "yend", "xintercept", "yintercept",
    "slope", "intercept",
    // box/density interval stats vocabulary (stat → geom handoff)
    "lower", "upper", "middle", "lwd", "notchupper", "notchlower", "x_lower", "x_middle", "x_upper",
    "y_lower", "y_middle", "y_upper", "outlier_label", "outlier_x", "outlier_y", "flipped_aes",
    // geometry shape
    "width", "height", "angle", "just",
    // appearance
    "colour", "color", "fill", "size", "linewidth", "linetype", "shape", "alpha",
    "stroke",
    // text
    "label", "family", "fontface", "hjust", "vjust",
    // bookkeeping
    "group", "weight", "order", "PANEL", "subgroup",
];

/// Stat-computed variables reachable via `after_stat()` (ggplot2 names).
pub const STAT_VARIABLES: &[&str] = &[
    "count", "n", "density", "ncount", "prop", "ndensity",
    "after_stat_count", "after_stat_density", "after_stat_ncount", "after_stat_prop",
    "bin", "width", "breaks",
    "xmin", "xmax", "ymin", "ymax",
    "lower", "middle", "upper", "outliers", "lwd",
];

/// ggplot2 `...` parameters per geom family (subset consumed by jplot so far;
/// names copied verbatim from ggplot2 signatures — see api_inventory.json).
/// Anything present in the inventory but absent here is on the roadmap.
pub const GEOM_PARAMS: &[&str] = &[
    // shared
    "na.rm", "show.legend", "inherit.aes", "orientation",
    // point
    "shape", "size", "stroke", "colour", "fill", "alpha",
    // line/path
    "linetype", "linewidth", "lineend", "linejoin", "linealpha",
    // bar/col/rect/hist
    "width", "just", "binwidth", "bins", "boundary", "center", "closed",
    // boxplot
    "outliers", "outlier.colour", "outlier.color", "outlier.fill", "outlier.shape",
    "outlier.size", "outlier.stroke", "outlier.alpha",
    "whisker.colour", "whisker.color", "whisker.linetype", "whisker.linewidth",
    "staple.colour", "staple.linetype", "staple.linewidth", "staplewidth",
    "median.colour", "median.color", "median.linetype", "median.linewidth",
    "box.colour", "box.color", "box.linetype", "box.linewidth",
    "notch", "notchwidth", "varwidth",
    // text/label
    "label", "parse", "nudge_x", "nudge_y", "check_overlap",
    // ribbons
    "draw_baseline",
];

/// Aesthetic keys used for grouping (ggplot2 `make_groups`).
pub const GROUP_AES: &[&str] = &["group", "colour", "color", "fill", "linetype", "shape", "alpha"];

pub fn is_aes(name: &str) -> bool {
    AES.contains(&name) || name.starts_with("..")
}

pub fn is_param(name: &str) -> bool {
    GEOM_PARAMS.contains(&name)
}
