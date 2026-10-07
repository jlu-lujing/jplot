//! Probed reference constants.
//!
//! Every value here was measured against a ggplot2 4.0.3 / R 4.4.3 rendering
//! (svglite SVG geometry or cairo PNG pixel probes — see `compare/`). They are
//! collected here rather than inlined so that:
//!   1. the *provenance* (which probe established it) lives next to the value;
//!   2. re-calibration is a single, auditable file edit;
//!   3. layout.rs reads declarative names instead of magic floats.
//!
//! Units: layout/legend offsets are device px on the 720×480 @ 72 dpi canvas
//! (pt = px); glyph ratios are dimensionless multiples of the point radius r.

/// Gutter / gtable column anatomy (px). Panel origin = sum of left columns.
pub mod gutter {
    /// plot margin on every side (ggplot2 `plot.margin` = 0.5·half_line·rel…
    /// probes: constant 5.48 = 11/2·0.9964 ≈ half_line at base 11).
    pub const PLOT_MARGIN: f64 = 5.48;
    /// axis cell = widest tick label + this internal padding
    /// (tick 2.75 + svglite label margin ≈ 2.1).
    pub const AXIS_LABEL_PAD: f64 = 4.85;
    /// rotated y-label column width (probe 07_line, left 41.2 with title).
    pub const YLAB_COL: f64 = 13.06;
    /// bottom gutter when the x-axis title is present (panel y1 = 448.5).
    pub const BOTTOM_WITH_TITLE: f64 = 31.5;
    /// bottom gutter with no x-axis title.
    pub const BOTTOM_NO_TITLE: f64 = 23.0;
    /// top gutter: plot title adds 17.7 (probe 08_col y0 = 23.18).
    pub const TITLE_H: f64 = 17.7;
    /// subtitle adds 14.5.
    pub const SUBTITLE_H: f64 = 14.5;
}

/// Axis tick/label text placement (svglite baselines).
pub mod axis {
    /// x tick label baseline = panel y1 + tick + this + ascent·0.76·size…
    /// expressed as the fixed pad above the label's own baseline.
    pub const XLABEL_PAD: f64 = 1.80;
    /// fraction of font size from centre to baseline (Arial metrics 0.76).
    pub const BASELINE_FRAC: f64 = 0.76;
    /// y tick label: horizontal inset from panel x0 past the tick.
    pub const YLABEL_INSET: f64 = 2.2;
    /// y tick label: vertical offset added to the break before baseline.
    pub const YLABEL_PAD: f64 = 0.31;
    /// x-axis title baseline measured from the canvas bottom (height − 7.80).
    pub const XTITLE_BASE: f64 = 7.80;
    /// y-axis title rotate(-90) column centre x.
    pub const YTITLE_X: f64 = 13.36;
    /// grid text "box height" factor for vjust (svglite v0/vh/v1 probe).
    pub const VJUST_BOX: f64 = 0.716;
}

/// Plot title / subtitle / caption anchors.
pub mod title {
    /// plot title baseline (size 13.2 = base·1.2).
    pub const BASELINE: f64 = 14.93;
    /// subtitle baseline offset below title (fraction of small_text).
    pub const SUB_PAD: f64 = 0.35;
}

/// Legend geometry (svglite probe: single-guide block title 204.43, keys
/// 222.14 / 237.98 / 253.82, label column right edge = width − 10.96).
pub mod legend {
    /// key width (also the square key box side).
    pub const KEY_W: f64 = 16.0;
    /// vertical pitch between consecutive key rows.
    pub const PITCH: f64 = 15.84;
    /// gap between the key column and the label column.
    pub const LABEL_GAP: f64 = 7.1;
    /// legend block right edge inset = width − 10.96.
    pub const RIGHT_EDGE: f64 = 10.96;
    /// gutter after the legend column (title overhang allowance).
    pub const BLOCK_PAD: f64 = 9.17;
    /// title overhang: half the title width beyond the key column may widen
    /// the block (probe title_gap uses this, distinct from BLOCK_PAD).
    pub const TITLE_GAP: f64 = 5.12;
    /// title→first-key baseline gap.
    pub const TITLE_TO_FIRST: f64 = 17.7;
    /// last-key→next-title gap when guides stack.
    pub const INTER_GUIDE: f64 = 35.8;
    /// key label baseline sits below the key centre by this.
    pub const LABEL_BASE: f64 = 3.15;
    /// half the title→first-key used to balance the block about panel centre.
    pub const CENTRE_ADJ: f64 = 2.11;
    /// gradient colour bar: top offset from the block title baseline.
    pub const BAR_TOP: f64 = 6.64;
    /// gradient colour bar height.
    pub const BAR_H: f64 = 79.2;
    /// gradient bar block extra height beyond title_to_first (6.64 + 79.2·≈0.72
    /// tick span; probe-fit so bar-block-centre matches the ref stack).
    pub const BAR_SPAN: f64 = 63.3;
    /// number of colour slices rendered into the gradient bar.
    pub const BAR_SLICES: usize = 48;
}

/// Point-glyph shape geometry (svglite pch polygon probes; r = outer radius).
pub mod glyph {
    /// upward triangle (pch 2/17/24): apex / base x-offsets per radius.
    pub const TRI_UP_Y: f64 = 1.556;
    pub const TRI_X: f64 = 1.348;
    pub const TRI_BASE_Y: f64 = 0.777;
    /// pch-20 ("small circle") radius factor of the outer radius.
    pub const DOT_RATIO: f64 = 2.0 / 3.0;
    /// plus/cross arm length factor (√2).
    pub const SQRT2: f64 = 1.414;
    /// cross diagonal arm factor (1/√2).
    pub const INV_SQRT2: f64 = 0.7071;
    /// outlier 17-triangle (legacy boxplot): apex-y / base-y factors of r
    /// (the ±x offsets are exactly ±r).
    pub const OUTLIER_TRI_APEX: f64 = 1.15;
    pub const OUTLIER_TRI_BASE_Y: f64 = 0.8;
}
