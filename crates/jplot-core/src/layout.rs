//! Layout: BuiltPlot → Scene (device px @72dpi → pt-equal).
//!
//! Conventions calibrated against ggplot2 4.0.3 cairo (compare/probe.R):
//! panel grey is grey92 (0.9216), canvas 720×480 for a scatter with default
//! gutters left≈36px top≈9px; axis-tick length 0.25cm=7.1pt; half_line=5.5pt.
//! Text boxes are positioned by their *centre*; halign shifts horizontally.

use crate::build::{BuiltPlot, BuiltScale};
pub use crate::geom;
pub use crate::guides;
use crate::scene::{layer, Line, Paint, Primitive, Scene, TextAlign, TextStyle};
use crate::scale::Color;
use crate::text::measure;
use crate::probes;
use crate::theme::geom_defaults::geom_lw;

pub struct Viewport {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Viewport {
    fn t_x(s: &BuiltScale, v: f64) -> f64 {
        match s {
            BuiltScale::Continuous(cs) => cs.map(v),
            BuiltScale::Discrete(ds) => (v - ds.range.min) / (ds.range.max - ds.range.min),
        }
    }
    pub fn map_x(&self, s: &BuiltScale, v: f64) -> f64 {
        self.x0 + Self::t_x(s, v) * (self.x1 - self.x0)
    }
    pub fn map_y(&self, s: &BuiltScale, v: f64) -> f64 {
        self.y1 - Self::t_x(s, v) * (self.y1 - self.y0)
    }
    /// Derived geometry (errorbar caps, area/ribbon corners, bar edges):
    /// no oob censor — these may extend past the panel and are clipped by it.
    pub fn map_x_plain(&self, s: &BuiltScale, v: f64) -> f64 {
        let t = match s {
            BuiltScale::Continuous(cs) => cs.map_plain(v),
            BuiltScale::Discrete(ds) => (v - ds.range.min) / (ds.range.max - ds.range.min),
        };
        self.x0 + t * (self.x1 - self.x0)
    }
    pub fn map_y_plain(&self, s: &BuiltScale, v: f64) -> f64 {
        let t = match s {
            BuiltScale::Continuous(cs) => cs.map_plain(v),
            BuiltScale::Discrete(ds) => (v - ds.range.min) / (ds.range.max - ds.range.min),
        };
        self.y1 - t * (self.y1 - self.y0)
    }
}

pub fn layout(bp: &BuiltPlot) -> Scene {
    let spec = &bp.plot;
    let theme = spec.theme.resolve();
    let width = spec.width;
    let height = spec.height;
    let hl = theme.half_line(); // 5.5
    let mut sc = Scene::new(width, height);
    sc.background = theme.plot_bg.unwrap_or(Color::white());

    let (x_breaks, x_labels) = axis_breaks(&bp.x_scale);
    let (y_breaks, y_labels) = axis_breaks(&bp.y_scale);
    let label_style = TextStyle {
        size: theme.axis_text_ov.size.unwrap_or(theme.small_text()),
        color: theme.axis_text_ov.colour.unwrap_or(theme.axis_text),
        bold: theme.axis_text_ov.bold,
        ..Default::default()
    };
    let tick_line = Line::solid(Color::rgb(51, 51, 51), geom_lw(0.5)); // #333 @ 0.5mm→1.07 (ref probe)
    let flipped = spec.coord == "flip";
    let x_lab = spec.labels.x.clone().or_else(|| auto_label_of(bp, "x")).unwrap_or_default();
    let y_lab = spec.labels.y.clone().or_else(|| auto_label_of(bp, "y")).unwrap_or_default();

    // --- gutters: calibrated against ggplot2 4.0.3 probes (see probes.rs) --
    let tick = theme.tick_length_pt;
    // coord_flip: the LEFT axis carries the x-aesthetic (labels + title swap)
    let (l_axis_labels, l_axis_title) = if flipped {
        (&x_labels, &x_lab)
    } else {
        (&y_labels, &y_lab)
    };
    let mut left = {
        let w = l_axis_labels
            .iter()
            .map(|l| measure(l, &label_style).width)
            .fold(0.0, f64::max);
        w + probes::gutter::AXIS_LABEL_PAD
    };
    if !l_axis_title.is_empty() {
        left += probes::gutter::YLAB_COL;
    }
    left += probes::gutter::PLOT_MARGIN;
    // svglite: panel bottom = 448.5 → 31.5 gutter with x-axis title
    let b_axis_title = if flipped { &y_lab } else { &x_lab };
    let mut bottom = {
        let _ = hl;
        if !b_axis_title.is_empty() {
            probes::gutter::BOTTOM_WITH_TITLE
        } else {
            probes::gutter::BOTTOM_NO_TITLE
        }
    };
    if spec.labels.caption.is_some() {
        bottom += hl + theme.small_text();
    }
    let mut top = probes::gutter::PLOT_MARGIN;
    if spec.labels.title.is_some() {
        top += probes::gutter::TITLE_H;
    }
    if spec.labels.subtitle.is_some() {
        top += probes::gutter::SUBTITLE_H;
    }
    let mut right = probes::gutter::PLOT_MARGIN;
    let guides: Vec<guides::Guide> = if theme.legend_pos == crate::theme::LegendPos::None {
        Vec::new()
    } else {
        guides::build_guides(bp, &label_style)
    };
    {
        if !guides.is_empty() {
            // all guides share one column; width sized by the widest label
            let maxlabel = guides
                .iter()
                .flat_map(|g| g.levels.iter())
                .map(|l| measure(l, &label_style).width)
                .fold(0.0f64, f64::max);
            let ts = TextStyle { size: theme.base_size, ..label_style.clone() };
            let title_gap = guides
                .iter()
                .map(|g| {
                    measure(&g.title, &ts).width / 2.0
                        - probes::legend::KEY_W / 2.0
                        + probes::legend::TITLE_GAP
                })
                .fold(0.0f64, f64::max);
            let legend_width =
                probes::legend::KEY_W + probes::legend::LABEL_GAP + maxlabel + probes::legend::BLOCK_PAD;
            right += legend_width.max(title_gap) + probes::legend::RIGHT_EDGE;
        }
    }
    let _ = tick;

    let vp = Viewport { x0: left, y0: top, x1: width - right, y1: height - bottom };

    // --- panel background ----------------------------------------------------
    if theme.panel_bg != Color::white() {
        sc.layer(layer::PANEL_BG).push(Primitive::Rect {
            x: vp.x0,
            y: vp.y0,
            w: vp.x1 - vp.x0,
            h: vp.y1 - vp.y0,
            fill: Some(Paint::new(theme.panel_bg)),
            stroke: None,
        });
    }

    // --- grid ---------------------------------------------------------------
    // Ref probe (svglite): major grid = 0.5mm → 1.07px, minor grid = 0.25mm →
    // 0.53px, both #FFFFFF; minor lines sit at midpoints between majors and
    // half-steps past the outer majors (R regular_minor_breaks, n = 2).
    if theme.panel_grid_major {
        let gl = Line::solid(theme.panel_grid, geom_lw(0.5));
        for &b in &x_breaks {
            if !theme.grid_x { continue; }
            if flipped {
                let y = vp.y1 - Viewport::t_x(&bp.x_scale, b) * (vp.y1 - vp.y0);
                if y > vp.y0 && y < vp.y1 {
                    sc.layer(layer::GRID).push(Primitive::Segment { x1: vp.x0, y1: y, x2: vp.x1, y2: y, stroke: gl.clone() });
                }
            } else {
                let x = vp.map_x(&bp.x_scale, b);
                if x > vp.x0 && x < vp.x1 {
                    sc.layer(layer::GRID).push(Primitive::Segment { x1: x, y1: vp.y0, x2: x, y2: vp.y1, stroke: gl.clone() });
                }
            }
        }
        for &b in &y_breaks {
            if !theme.grid_y { continue; }
            if flipped {
                let x = vp.map_x(&bp.y_scale, b);
                if x > vp.x0 && x < vp.x1 {
                    sc.layer(layer::GRID).push(Primitive::Segment { x1: x, y1: vp.y0, x2: x, y2: vp.y1, stroke: gl.clone() });
                }
            } else {
                let y = vp.map_y(&bp.y_scale, b);
                if y > vp.y0 && y < vp.y1 {
                    sc.layer(layer::GRID).push(Primitive::Segment { x1: vp.x0, y1: y, x2: vp.x1, y2: y, stroke: gl.clone() });
                }
            }
        }
    }
    if theme.panel_grid_minor {
        let gl = Line::solid(theme.panel_grid, geom_lw(0.25));
        // minor gridlines exist only for CONTINUOUS scales (ggplot2 discrete
        // scales carry no minors; na.value ticks are not minors)
        let (xlo, xhi) = bp.x_scale.range();
        let (ylo, yhi) = bp.y_scale.range();
        let x_minor = matches!(&bp.x_scale, crate::build::BuiltScale::Continuous(_));
        let y_minor = matches!(&bp.y_scale, crate::build::BuiltScale::Continuous(_));
        for &b in &minor_breaks_of(&x_breaks, xlo, xhi) {
            if !theme.grid_x || !x_minor {
                continue;
            }
            if flipped {
                // x aesthetic → vertical axis (reversed, like its majors)
                let y = vp.y1 - Viewport::t_x(&bp.x_scale, b) * (vp.y1 - vp.y0);
                if y > vp.y0 && y < vp.y1 {
                    sc.layer(layer::GRID)
                        .push(Primitive::Segment { x1: vp.x0, y1: y, x2: vp.x1, y2: y, stroke: gl.clone() });
                }
            } else {
                let x = vp.map_x(&bp.x_scale, b);
                if x > vp.x0 && x < vp.x1 {
                    sc.layer(layer::GRID)
                        .push(Primitive::Segment { x1: x, y1: vp.y0, x2: x, y2: vp.y1, stroke: gl.clone() });
                }
            }
        }
        for &b in &minor_breaks_of(&y_breaks, ylo, yhi) {
            if !theme.grid_y || !y_minor {
                continue;
            }
            if flipped {
                // y aesthetic → bottom axis, same direction as the majors
                let x = vp.map_x(&bp.y_scale, b);
                if x > vp.x0 && x < vp.x1 {
                    sc.layer(layer::GRID)
                        .push(Primitive::Segment { x1: x, y1: vp.y0, x2: x, y2: vp.y1, stroke: gl.clone() });
                }
            } else {
                let y = vp.map_y(&bp.y_scale, b);
                if y > vp.y0 && y < vp.y1 {
                    sc.layer(layer::GRID)
                        .push(Primitive::Segment { x1: vp.x0, y1: y, x2: vp.x1, y2: y, stroke: gl.clone() });
                }
            }
        }
    }
    if theme.panel_border {
        sc.layer(layer::GRID).push(Primitive::Rect {
            x: vp.x0,
            y: vp.y0,
            w: vp.x1 - vp.x0,
            h: vp.y1 - vp.y0,
            fill: None,
            stroke: Some(Line::solid(Color::black(), geom_lw(0.5))),
        });
    }

    // --- data ---------------------------------------------------------------
    let mut data = Vec::new();
    for l in &bp.layers {
        geom::draw_layer(&mut data, l, bp, &vp);
    }
    if flipped {
        for p in &mut data {
            transpose(p, &vp);
        }
    }
    for p in data {
        sc.layer(layer::DATA).push(p);
    }
    sc.layer(layer::DATA).clip = Some((vp.x0, vp.y0, vp.x1 - vp.x0, vp.y1 - vp.y0));

    // --- axes ---------------------------------------------------------------
    {
        let al = tick_line;
        // Text `y` is now the BASELINE and `x` the centre (renderer does
        // text-anchor:middle); svglite baselines:
        //   x-label = y1 + tick + 2.42 + ascent ; y-label = break + 0.31 + ascent
        //   x-title = 472.20 (fixed), y-title centred translate(13.36, mid)
        let push_label = |s: &mut Scene, content: &str, cx: f64, baseline: f64, st: TextStyle| {
            // R/svglite writes textLength from stringWidth; our Helvetica
            // metrics are the same source, so pin resvg to it (removes
            // natural-kerning drift → the dominant "bottom"/"left-gutter"
            // residuals in triage).
            let w = measure(content, &st).width;
            s.layer(layer::AXES).push(Primitive::Text { content: content.into(), x: cx, y: baseline, style: st, text_length: Some(w) });
        };
        for (&b, lab) in x_breaks.iter().zip(x_labels.iter()) {
            if flipped {
                // x aesthetic now runs down the LEFT vertical axis; the data
                // transpose put level 1 at the BOTTOM (reversed discrete
                // axis), so the tick label must use the same reversal.
                let py = vp.y1 - Viewport::t_x(&bp.x_scale, b) * (vp.y1 - vp.y0);
                if py < vp.y0 - 0.5 || py > vp.y1 + 0.5 {
                    continue;
                }
                sc.layer(layer::AXES)
                    .push(Primitive::Segment { x1: vp.x0 - tick, y1: py, x2: vp.x0, y2: py, stroke: al.clone() });
                let st = TextStyle { halign: TextAlign::Right, ..label_style.clone() };
                let w = measure(lab, &st).width;
                sc.layer(layer::AXES).push(Primitive::Text {
                    content: lab.into(),
                    x: vp.x0 - tick - probes::axis::YLABEL_INSET,
                    y: py + probes::axis::VJUST_BOX / 2.0 * st.size,
                    style: st,
                    text_length: Some(w),
                });
            } else {
                let x = vp.map_x(&bp.x_scale, b);
                if x < vp.x0 - 0.5 || x > vp.x1 + 0.5 {
                    continue;
                }
                sc.layer(layer::AXES).push(Primitive::Segment { x1: x, y1: vp.y1, x2: x, y2: vp.y1 + tick, stroke: al.clone() });
                push_label(
                    &mut sc,
                    lab,
                    x,
                    vp.y1 + tick + probes::axis::XLABEL_PAD + probes::axis::BASELINE_FRAC * label_style.size,
                    label_style.clone(),
                );
            }
        }
        for (&b, lab) in y_breaks.iter().zip(y_labels.iter()) {
            if flipped {
                // y aesthetic now runs across the BOTTOM axis
                let px = vp.x0 + Viewport::t_x(&bp.y_scale, b) * (vp.x1 - vp.x0);
                if px < vp.x0 - 0.5 || px > vp.x1 + 0.5 {
                    continue;
                }
                sc.layer(layer::AXES)
                    .push(Primitive::Segment { x1: px, y1: vp.y1, x2: px, y2: vp.y1 + tick, stroke: al.clone() });
                push_label(
                    &mut sc,
                    lab,
                    px,
                    vp.y1 + tick + probes::axis::XLABEL_PAD + probes::axis::BASELINE_FRAC * label_style.size,
                    label_style.clone(),
                );
            } else {
                let y = vp.map_y(&bp.y_scale, b);
                if y < vp.y0 - 0.5 || y > vp.y1 + 0.5 {
                    continue;
                }
                sc.layer(layer::AXES).push(Primitive::Segment { x1: vp.x0 - tick, y1: y, x2: vp.x0, y2: y, stroke: al.clone() });
                let st = TextStyle { halign: TextAlign::Right, ..label_style.clone() };
                let yw = measure(lab, &st).width;
                sc.layer(layer::AXES).push(Primitive::Text {
                    content: lab.clone(),
                    x: vp.x0 - tick - probes::axis::YLABEL_INSET,
                    // vertical-centre on the tick (vjust=0.5): baseline sits half a
                    // grid text-box below the tick, matching svglite (triage: the
                    // old 0.31+0.76·size pushed labels ~4px low).
                    y: y + probes::axis::VJUST_BOX / 2.0 * st.size,
                    style: st, text_length: Some(yw)
                });
            }
        }
        if flipped {
            if !x_lab.is_empty() {
                let ts = TextStyle {
                    size: theme.axis_title_ov.size.unwrap_or(theme.base_size),
                    color: theme.axis_title_ov.colour.unwrap_or(theme.axis_text),
                    angle: 90.0,
                    ..label_style.clone()
                };
                sc.layer(layer::TITLES).push(Primitive::Text {
                    content: x_lab.clone(),
                    x: probes::axis::YTITLE_X,
                    y: (vp.y0 + vp.y1) / 2.0,
                    style: ts, text_length: None
                });
            }
            if !y_lab.is_empty() {
                let ts = TextStyle {
                    size: theme.axis_title_ov.size.unwrap_or(theme.base_size),
                    color: theme.axis_title_ov.colour.unwrap_or(theme.axis_text),
                    ..label_style.clone()
                };
                sc.layer(layer::TITLES).push(Primitive::Text {
                    content: y_lab.clone(),
                    x: (vp.x0 + vp.x1) / 2.0,
                    y: height - probes::axis::XTITLE_BASE,
                    style: ts, text_length: None
                });
            }
        } else if !x_lab.is_empty() {
            // svglite: "disp" baseline = 472.20 (= height - 7.80), size 11, centred
            let ts = TextStyle {
                size: theme.axis_title_ov.size.unwrap_or(theme.base_size),
                color: theme.axis_title_ov.colour.unwrap_or(theme.axis_text),
                bold: theme.axis_title_ov.bold,
                ..label_style.clone()
            };
            sc.layer(layer::TITLES).push(Primitive::Text {
                content: x_lab.clone(),
                x: (vp.x0 + vp.x1) / 2.0,
                y: height - probes::axis::XTITLE_BASE,
                style: ts, text_length: None
            });
        }
        if !flipped && !y_lab.is_empty() {
            // svglite: translate(13.36, 226.99) rotate(-90) anchor=middle
            let ts = TextStyle {
                size: theme.axis_title_ov.size.unwrap_or(theme.base_size),
                color: theme.axis_title_ov.colour.unwrap_or(theme.axis_text),
                bold: theme.axis_title_ov.bold,
                angle: 90.0,
                ..label_style.clone()
            };
            sc.layer(layer::TITLES).push(Primitive::Text {
                content: y_lab.clone(),
                x: probes::axis::YTITLE_X,
                y: (vp.y0 + vp.y1) / 2.0,
                style: ts, text_length: None
            });
        }
    }

    // --- titles / caption ----------------------------------------------------
    if let Some(t) = &spec.labels.title {
        let ts = TextStyle {
            size: theme.title_ov.size.unwrap_or(theme.large_text()),
            color: theme.title_ov.colour.unwrap_or(Color::black()),
            bold: theme.title_ov.bold,
            halign: if theme.title_ov.hjust == Some(0.5) || theme.title_ov.hjust.is_none() {
                TextAlign::Left
            } else {
                theme.title_ov.hjust.map_or(TextAlign::Left, |h| if h <= 0.25 { TextAlign::Left } else if h >= 0.75 { TextAlign::Right } else { TextAlign::Center })
            },
            ..Default::default()
        };
        // svglite title baseline (size 13.2)
        sc.layer(layer::TITLES).push(Primitive::Text {
            content: t.clone(),
            x: vp.x0,
            y: probes::title::BASELINE,
            style: ts, text_length: None
        });
    }
    if let Some(t) = &spec.labels.subtitle {
        let ts = TextStyle { size: theme.small_text(), halign: TextAlign::Left, ..Default::default() };
        let y = probes::title::BASELINE + theme.large_text() + ts.size * probes::title::SUB_PAD;
        sc.layer(layer::TITLES).push(Primitive::Text { content: t.clone(), x: vp.x0, y, style: ts , text_length: None });
    }
    if let Some(c) = &spec.labels.caption {
        let ts = TextStyle { size: theme.small_text(), halign: TextAlign::Right, ..Default::default() };
        sc.layer(layer::TITLES).push(Primitive::Text { content: c.clone(), x: vp.x1, y: height - hl - ts.size / 2.0, style: ts , text_length: None });
    }

    // --- legend --------------------------------------------------------------
    if !guides.is_empty() {
        guides::draw_legends(&mut sc, &guides, &theme, width, &vp);
    }

    sc
}


/// Mirror a primitive about the panel diagonal (coord_flip data transform).
fn transpose(p: &mut Primitive, vp: &Viewport) {
    let w = vp.x1 - vp.x0;
    let h = vp.y1 - vp.y0;
    // coord_flip data transform: new px = px0 + (y1 − py)·w/h (the panel's
    // TOP edge becomes the LEFT edge ⇒ continuous y-data keeps ascending
    // left→right: ref probe 50 cyl=8 (low mpg) box at x=166, cyl=4 at 389);
    // new py = y1 − (px − x0)·h/w (old left → bottom ⇒ level 1 at the bottom,
    // cyl 4@369 / 8@92 as in the reference).
    let tx = |x: f64, y: f64| -> (f64, f64) {
        (vp.x0 + (vp.y1 - y) * (w / h), vp.y1 - (x - vp.x0) * (h / w))
    };
    match p {
        Primitive::Rect { x, y, w: rw, h: rh, .. } => {
            // image of the old BOTTOM-RIGHT corner is the new top-left
            let (nx, ny) = tx(*x + *rw, *y + *rh);
            let (nw, nh) = (*rh * (w / h), *rw * (h / w));
            *x = nx;
            *y = ny;
            *rw = nw;
            *rh = nh;
        }
        Primitive::Circle { cx, cy, .. } => {
            let (a, b) = tx(*cx, *cy);
            *cx = a;
            *cy = b;
        }
        Primitive::Segment { x1, y1, x2, y2, .. } => {
            let (a, b) = tx(*x1, *y1);
            let (c, d) = tx(*x2, *y2);
            *x1 = a;
            *y1 = b;
            *x2 = c;
            *y2 = d;
        }
        Primitive::Polyline { points, .. } => {
            for (x, y) in points.iter_mut() {
                let (a, b) = tx(*x, *y);
                *x = a;
                *y = b;
            }
        }
        Primitive::Text { x, y, .. } => {
            let (a, b) = tx(*x, *y);
            *x = a;
            *y = b;
        }
    }
}

/// Minor ticks matching R regular_minor_breaks (n=2): one line at each
/// midpoint between consecutive majors, plus one half-step past the LAST
/// major when it still fits inside the panel range (R starts the sequence at
/// the first major, so nothing precedes it; both ends are range-clipped).
fn minor_breaks_of(majors: &[f64], lo: f64, hi: f64) -> Vec<f64> {
    if majors.len() < 2 {
        return vec![];
    }
    let step = (majors[1] - majors[0]) / 2.0;
    let mut out: Vec<f64> = Vec::new();
    for w in majors.windows(2) {
        let m = (w[0] + w[1]) / 2.0;
        if m > lo && m < hi {
            out.push(m);
        }
    }
    let past = majors[majors.len() - 1] + step;
    if past > lo && past < hi {
        out.push(past);
    }
    out
}

fn axis_breaks(s: &BuiltScale) -> (Vec<f64>, Vec<String>) {
    match s {
        BuiltScale::Continuous(cs) => {
            let b = cs.breaks_in_range();
            (b.iter().map(|(v, _)| *v).collect(), b.iter().map(|(_, l)| l.clone()).collect())
        }
        BuiltScale::Discrete(ds) => {
            let b: Vec<f64> = (1..=ds.levels.len()).map(|i| i as f64).collect();
            (b, ds.levels.clone())
        }
    }
}

fn auto_label_of(bp: &BuiltPlot, aes: &str) -> Option<String> {
    // count-based stats get ggplot2's default label
    if aes == "y" {
        let count_stat = bp.layers.iter().any(|l| {
            use crate::spec::GeomSpec;
            matches!(l.geom, GeomSpec::Bar | GeomSpec::Histogram { .. } | GeomSpec::Freqpoly { .. })
                && !l.aes.contains_key("y")
        });
        if count_stat {
            return Some("count".into());
        }
        let dens_stat = bp.layers.iter().any(|l| {
            use crate::spec::GeomSpec;
            matches!(l.geom, GeomSpec::Density) && !l.aes.contains_key("y")
        });
        if dens_stat {
            return Some("density".into());
        }
    }
    let col = bp
        .layers
        .iter()
        .find_map(|l| l.aes.get(aes).cloned())
        .or_else(|| bp.plot.mapping.map.get(aes).cloned())?;
    if col.is_empty() {
        None
    } else {
        Some(col)
    }
}

// ---------------------------------------------------------------------------
// Layers
// ---------------------------------------------------------------------------


#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Column, Dataset};
    use crate::spec::{aes, geom_boxplot, geom_line, ggplot};
    use std::collections::HashMap;

    fn boxplot_scene() -> Scene {
        let mut d = Dataset::new();
        d.add("g", Column::Categorical { values: vec!["a".into(); 10], levels: Some(vec!["a".into()]) });
        d.add("v", Column::Numeric { values: (1..=10).map(|i| i as f64).collect(), label: None });
        let mut m = HashMap::new();
        m.insert("x".into(), "g".into());
        m.insert("y".into(), "v".into());
        let p = ggplot(d) + aes(m) + geom_boxplot();
        let b = crate::build::build(&p.spec).unwrap();
        layout(&b)
    }

    #[test]
    fn boxplot_box_rects_have_positive_height() {
        // regression: SVG drops negative-width/height rects; the box body must
        // be normalised (yq1 > yq3 in px space since y is flipped).
        let sc = boxplot_scene();
        let rects: Vec<_> = sc
            .layers
            .iter()
            .flat_map(|l| l.primitives.iter())
            .filter_map(|p| match p {
                Primitive::Rect { w, h, fill, .. } if fill.is_some() && *fill != Some(Paint::new(Color::white())) => Some((w, h)),
                Primitive::Rect { x, w, h, stroke, .. } if *stroke != Some(Line::solid(Color::rgb(51, 51, 51), 0.6666667)) => None,
                _ => None,
            })
            .collect();
        // simpler: inspect ALL rects, none may have negative w/h
        for l in &sc.layers {
            for p in &l.primitives {
                if let Primitive::Rect { w, h, .. } = p {
                    assert!(*w >= 0.0 && *h >= 0.0, "rect with negative extent: w={w} h={h}");
                }
            }
        }
    }

    #[test]
    fn boxplot_emits_box_body_rects() {
        let sc = boxplot_scene();
        let boxes = sc
            .layers
            .iter()
            .flat_map(|l| &l.primitives)
            .filter(|p| matches!(p, Primitive::Rect { fill, stroke, w, h, .. }
                if fill == &Some(Paint::new(Color::white())) && stroke.is_some() && *w > 10.0 && *h > 10.0))
            .count();
        assert_eq!(boxes, 1, "expected one box body per group");
    }

    #[test]
    fn lines_are_coloured_per_group() {
        let mut d = Dataset::new();
        d.add("x", Column::Numeric { values: vec![1.0, 2.0, 1.0, 2.0], label: None });
        d.add("y", Column::Numeric { values: vec![1.0, 2.0, 3.0, 4.0], label: None });
        d.add("g", Column::Categorical { values: vec!["a".into(), "a".into(), "b".into(), "b".into()], levels: Some(vec!["a".into(), "b".into()]) });
        let mut m = HashMap::new();
        m.insert("x".into(), "x".into());
        m.insert("y".into(), "y".into());
        m.insert("colour".into(), "g".into());
        let p = ggplot(d) + aes(m) + geom_line();
        let b = crate::build::build(&p.spec).unwrap();
        let sc = layout(&b);
        let colors: Vec<_> = sc
            .layers
            .iter()
            .flat_map(|l| &l.primitives)
            .filter_map(|p| match p {
                Primitive::Polyline { stroke, .. } => Some(stroke.as_ref().map(|s| s.color)),
                _ => None,
            })
            .collect();
        assert_eq!(colors.len(), 2);
        assert_ne!(colors[0], colors[1], "two groups must not share a colour");
    }
}
