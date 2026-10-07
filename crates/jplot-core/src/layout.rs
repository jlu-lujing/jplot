//! Layout: BuiltPlot → Scene (device px @72dpi → pt-equal).
//!
//! Conventions calibrated against ggplot2 4.0.3 cairo (compare/probe.R):
//! panel grey is grey92 (0.9216), canvas 720×480 for a scatter with default
//! gutters left≈36px top≈9px; axis-tick length 0.25cm=7.1pt; half_line=5.5pt.
//! Text boxes are positioned by their *centre*; halign shifts horizontally.

use crate::build::{BuiltPlot, BuiltScale};
use crate::scene::{layer, Line, Paint, Primitive, Scene, TextAlign, TextStyle};
use crate::scale::Color;
use crate::text::{left_edge, measure};
use crate::theme::{geom_defaults, Theme};
use crate::theme::geom_defaults::mm_to_px;

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
}

pub fn layout(bp: &BuiltPlot) -> Scene {
    let spec = &bp.plot;
    let theme = Theme::new(crate::theme::ThemeKind::Grey);
    let width = spec.width;
    let height = spec.height;
    let hl = theme.half_line(); // 5.5
    let mut sc = Scene::new(width, height);
    sc.background = Color::white();

    let (x_breaks, x_labels) = axis_breaks(&bp.x_scale);
    let (y_breaks, y_labels) = axis_breaks(&bp.y_scale);
    let label_style = TextStyle {
        size: theme.small_text(),
        color: Color::rgb(77, 77, 77), // col_mix(ink, paper, 0.302)
        ..Default::default()
    };
    let tick_line = Line::solid(Color::rgb(51, 51, 51), mm_to_px(0.5)); // #333
    let x_lab = spec.labels.x.clone().or_else(|| auto_label_of(bp, "x")).unwrap_or_default();
    let y_lab = spec.labels.y.clone().or_else(|| auto_label_of(bp, "y")).unwrap_or_default();

    // --- gutters: calibrated against ggplot2 4.0.3 cairo PNG probes -------
    // (compare/refs pixel probes: panel x0 = 31..36 px, y1 = 446 px on a
    //  720x480 canvas for default theme_grey scatter/bar/hist/box)
    let tick = theme.tick_length_pt;
    // ggplot2 4.x gtable left column anatomy (probe_layout.R):
    //   5.48 plot-margin | 13.74 rotated-ytitle (if present) | axis cell
    // axis cell = widest tick label (DejaVu metrics = renderer's font) +
    // 4.85 internal padding (tick 2.75 + label margin ~2.1).
    let mut left = {
        let w = y_labels.iter().map(|l| measure(l, &label_style).width).fold(0.0, f64::max);
        w + 4.85
    };
    if !y_lab.is_empty() {
        left += 13.74;
    }
    left += 5.48;
    // svglite: panel bottom = 448.5 → 31.5 gutter with x-axis title
    let mut bottom = {
        let _ = hl;
        if !x_lab.is_empty() { 31.5 } else { 23.0 }
    };
    if spec.labels.caption.is_some() {
        bottom += hl + theme.small_text();
    }
    // svglite: y0=5.48; +17.7 with plot title (probe 08_col y0=23.18)
    let mut top = 5.48;
    if spec.labels.title.is_some() {
        top += 17.7;
    }
    if spec.labels.subtitle.is_some() {
        top += 14.5;
    }
    let mut right = 5.48;
    let legend_width;
    {
        if let Some(cs) = bp.colour_scale.as_ref().or(bp.fill_scale.as_ref()) {
            let title = bp
                .plot
                .labels
                .guides
                .get(if bp.fill_scale.as_ref().map(|s| std::ptr::eq(s, cs)).unwrap_or(false) { "fill" } else { "colour" })
                .or(cs.name.as_ref())
                .cloned()
                .unwrap_or_default();
            let ts = TextStyle { size: theme.base_size, ..label_style.clone() };
            // title centred over key column (width 16) -> may overhang left by half its width
            let title_gap = measure(&title, &ts).width / 2.0 - 16.0 / 2.0 + 5.12;
            let title_gap = title_gap.max(0.0);
            let maxlabel = cs
                .levels
                .iter()
                .map(|l| measure(l, &label_style).width)
                .fold(0.0f64, f64::max);
            legend_width = 16.0 + 7.1 + maxlabel + 9.17;
            right += legend_width.max(title_gap) + 10.96;
        } else {
            legend_width = 0.0;
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
    if theme.panel_grid_major {
        let gc = if matches!(theme.kind, crate::theme::ThemeKind::Grey) {
            Color::white()
        } else {
            Color::rgb(229, 229, 229)
        };
        let gl = Line::solid(gc, mm_to_px(0.5));
        for &b in &x_breaks {
            let x = vp.map_x(&bp.x_scale, b);
            if x > vp.x0 && x < vp.x1 {
                sc.layer(layer::GRID).push(Primitive::Segment { x1: x, y1: vp.y0, x2: x, y2: vp.y1, stroke: gl });
            }
        }
        for &b in &y_breaks {
            let y = vp.map_y(&bp.y_scale, b);
            if y > vp.y0 && y < vp.y1 {
                sc.layer(layer::GRID).push(Primitive::Segment { x1: vp.x0, y1: y, x2: vp.x1, y2: y, stroke: gl });
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
            stroke: Some(Line::solid(Color::black(), mm_to_px(0.5))),
        });
    }

    // --- data ---------------------------------------------------------------
    let mut data = Vec::new();
    for l in &bp.layers {
        draw_layer(&mut data, l, bp, &vp);
    }
    for p in data {
        sc.layer(layer::DATA).push(p);
    }
    sc.layer(layer::DATA).clip = Some((vp.x0, vp.y0, vp.x1 - vp.x0, vp.y1 - vp.y0));

    // --- axes ---------------------------------------------------------------
    {
        let al = tick_line;
        let mut push_label = |s: &mut Scene, content: &str, x: f64, cy: f64, st: TextStyle| {
            let m = measure(content, &st);
            let x = left_edge(&m, &st, x);
            s.layer(layer::AXES).push(Primitive::Text { content: content.into(), x, y: cy, style: st });
        };
        for (&b, lab) in x_breaks.iter().zip(x_labels.iter()) {
            let x = vp.map_x(&bp.x_scale, b);
            if x < vp.x0 - 0.5 || x > vp.x1 + 0.5 {
                continue;
            }
            sc.layer(layer::AXES).push(Primitive::Segment { x1: x, y1: vp.y1, x2: x, y2: vp.y1 + tick, stroke: al });
            push_label(&mut sc, lab, x, vp.y1 + tick + hl + label_style.size / 2.0, label_style.clone());
        }
        for (&b, lab) in y_breaks.iter().zip(y_labels.iter()) {
            let y = vp.map_y(&bp.y_scale, b);
            if y < vp.y0 - 0.5 || y > vp.y1 + 0.5 {
                continue;
            }
            sc.layer(layer::AXES).push(Primitive::Segment { x1: vp.x0 - tick, y1: y, x2: vp.x0, y2: y, stroke: al });
            let st = TextStyle { halign: TextAlign::Right, ..label_style.clone() };
            let cy = y;
            let m = measure(lab, &st);
            let x = left_edge(&m, &st, vp.x0 - tick - hl);
            sc.layer(layer::AXES).push(Primitive::Text { content: lab.clone(), x, y: cy, style: st });
        }
        if !x_lab.is_empty() {
            let m = measure(&x_lab, &label_style);
            let x = (vp.x0 + vp.x1) / 2.0 - m.width / 2.0;
            sc.layer(layer::TITLES).push(Primitive::Text { content: x_lab.clone(), x, y: height - hl - label_style.size * 1.2, style: label_style.clone() });
        }
        if !y_lab.is_empty() {
            sc.layer(layer::TITLES).push(Primitive::Text {
                content: y_lab.clone(),
                x: hl + label_style.size / 2.0,
                y: (vp.y0 + vp.y1) / 2.0,
                style: TextStyle { angle: 90.0, ..label_style.clone() },
            });
        }
    }

    // --- titles / caption ----------------------------------------------------
    if let Some(t) = &spec.labels.title {
        let ts = TextStyle { size: theme.large_text(), halign: TextAlign::Left, ..Default::default() };
        sc.layer(layer::TITLES).push(Primitive::Text { content: t.clone(), x: vp.x0, y: theme.title_space_pt() + ts.size / 2.0, style: ts });
    }
    if let Some(t) = &spec.labels.subtitle {
        let ts = TextStyle { size: theme.small_text(), halign: TextAlign::Left, ..Default::default() };
        let y = theme.title_space_pt() + theme.large_text() + hl / 2.0 + ts.size / 2.0;
        sc.layer(layer::TITLES).push(Primitive::Text { content: t.clone(), x: vp.x0, y, style: ts });
    }
    if let Some(c) = &spec.labels.caption {
        let ts = TextStyle { size: theme.small_text(), halign: TextAlign::Right, ..Default::default() };
        sc.layer(layer::TITLES).push(Primitive::Text { content: c.clone(), x: vp.x1, y: height - hl - ts.size / 2.0, style: ts });
    }

    // --- legend --------------------------------------------------------------
    if let Some(cs) = bp.colour_scale.as_ref().or(bp.fill_scale.as_ref()) {
        draw_legend(&mut sc, bp, cs, &theme, width, &vp);
    }

    sc
}

fn s_axis_tick(s: &mut Scene, pos: f64, along1: f64, along2: f64, al: Line) {
    // x-axis: constant x (along = y); y-axis: constant y (along = x)
    // detect by whether along1/along2 differ in the axis-perpendicular way —
    // callers pass (vp.y1, x, y1+tick) for x-ticks and (y, x0-tick, y) for y.
    // We are told via argument order: (s, pos, from, to, line). pos is the
    // fixed coordinate, [from,to] the extent. But which axis? encode via a
    // sentinel: x-ticks pass pos==y1 (a y), y-ticks pass pos==y (a y) too.
    // Simplify: caller distinguishes by which pair is horizontal vs vertical;
    // here the "fixed" coord is `pos`, and the segment runs perpendicular.
    // To keep the signature unambiguous, callers pass explicit endpoints.
    let _ = (pos, along1, along2, al, s);
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
            matches!(l.geom, GeomSpec::Bar | GeomSpec::Histogram { .. }) && !l.aes.contains_key("y")
        });
        if count_stat {
            return Some("count".into());
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

fn draw_layer(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    use crate::spec::GeomSpec;
    match l.geom {
        GeomSpec::Point { .. } => draw_points(ops, l, bp, vp),
        GeomSpec::Line => draw_lines(ops, l, bp, vp),
        GeomSpec::Col | GeomSpec::Bar | GeomSpec::Histogram { .. } => draw_bars(ops, l, bp, vp),
        GeomSpec::Boxplot => draw_boxplot(ops, l, bp, vp),
    }
}

fn point_colour_of(l: &crate::build::BuiltLayer, i: usize, bp: &BuiltPlot) -> Color {
    let alpha = l.args.alpha.unwrap_or(1.0);
    if let Some(c) = &l.args.colour {
        if let Some(c) = Color::parse(c) {
            return c.with_alpha(alpha);
        }
    }
    if l.aes.contains_key("colour") {
        if let Some(cs) = &bp.colour_scale {
            if let Some(vals) = l.frame.cat.get("colour") {
                if i < vals.len() && !vals[i].is_empty() {
                    return cs.map(&vals[i]).with_alpha(alpha);
                }
            }
            if let Some(vals) = l.frame.get("colour") {
                if i < vals.len() {
                    return gradient_color(vals[i], vals, alpha);
                }
            }
        }
    }
    Color::black().with_alpha(alpha)
}

fn fill_colour_of(l: &crate::build::BuiltLayer, i: usize, bp: &BuiltPlot) -> Color {
    let alpha = l.args.alpha.unwrap_or(1.0);
    if let Some(c) = &l.args.fill {
        if let Some(c) = Color::parse(c) {
            return c.with_alpha(alpha);
        }
    }
    if l.aes.contains_key("fill") {
        if let Some(cs) = &bp.fill_scale {
            if let Some(vals) = l.frame.cat.get("fill") {
                if i < vals.len() && !vals[i].is_empty() {
                    return cs.map(&vals[i]).with_alpha(alpha);
                }
            }
        }
    }
    // ggplot2 geom_bar default fill = col_mix(ink, paper, 0.35) ≈ 0.35 grey
    Color::rgb(89, 89, 89).with_alpha(alpha)
}

fn gradient_color(v: f64, all: &[f64], alpha: f64) -> Color {
    let lo = all.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = all.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let t = if hi == lo { 0.5 } else { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) };
    let (r0, g0, b0) = (68u8, 1u8, 84u8);
    let (r1, g1, b1) = (253u8, 231u8, 37u8);
    let m = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
    Color::rgb(m(r0, r1), m(g0, g1), m(b0, b1)).with_alpha(alpha)
}

fn draw_points(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let size_px = l.args.size.unwrap_or(geom_defaults::point_size_px(11.0));
    let (xs, ys) = (l.frame.get("x").cloned().unwrap_or_default(), l.frame.get("y").cloned().unwrap_or_default());
    let r = size_px * 0.5;
    for i in 0..xs.len().min(ys.len()) {
        if !xs[i].is_finite() || !ys[i].is_finite() {
            continue;
        }
        let c = point_colour_of(l, i, bp);
        // ggplot2 default shape 19: solid, colour-filled, no border
        ops.push(Primitive::Circle {
            cx: vp.map_x(&bp.x_scale, xs[i]),
            cy: vp.map_y(&bp.y_scale, ys[i]),
            r,
            fill: Some(Paint::new(c)),
            stroke: None,
        });
    }
}

fn draw_lines(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let width_px = l.args.linewidth.unwrap_or(mm_to_px(0.5));
    let (xs, ys) = (l.frame.get("x").cloned().unwrap_or_default(), l.frame.get("y").cloned().unwrap_or_default());
    let n_groups = l.group_ids.iter().cloned().max().map_or(0, |m| m + 1);
    for g in 0..n_groups.max(1) {
        let mut idx: Vec<usize> = (0..xs.len())
            .filter(|&i| n_groups <= 1 || l.group_ids.get(i) == Some(&g))
            .collect();
        if idx.is_empty() {
            continue;
        }
        // colour from this group's first data row (not always row 0)
        let rep = idx[0];
        idx.sort_by(|&a, &b| xs[a].partial_cmp(&xs[b]).unwrap());
        let pts: Vec<(f64, f64)> = idx
            .into_iter()
            .filter(|&i| xs[i].is_finite() && ys[i].is_finite())
            .map(|i| (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ys[i])))
            .collect();
        if pts.len() < 2 {
            continue;
        }
        ops.push(Primitive::Polyline {
            points: pts,
            stroke: Some(Line::solid(point_colour_of(l, rep, bp), width_px)),
            fill: None,
        });
    }
}

fn draw_bars(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let xs = l.frame.get("x").cloned().unwrap_or_default();
    let ys = l.frame.get("y").cloned().unwrap_or_default();
    // ggplot2 geom_bar/col/histogram default colour = NA (no outline).
    let has_outline = l.aes.contains_key("colour") || l.args.colour.is_some();
    let (xa_v, xb_v) = (l.frame.get("xmin").cloned(), l.frame.get("xmax").cloned());
    let w = l.frame.get("width").cloned().unwrap_or_else(|| vec![0.9; xs.len()]);
    let ys_bottom = l.frame.get("ymin").cloned().unwrap_or_else(|| vec![0.0; xs.len()]);
    for i in 0..xs.len().min(ys.len()).min(w.len()) {
        if !xs[i].is_finite() || !ys[i].is_finite() {
            continue;
        }
        let (xl, xr) = match (&xa_v, &xb_v) {
            (Some(a), Some(b)) if i < a.len() && a[i].is_finite() => (a[i], b[i]),
            _ => (xs[i] - w[i] / 2.0, xs[i] + w[i] / 2.0),
        };
        let (xa, xb) = (vp.map_x(&bp.x_scale, xl), vp.map_x(&bp.x_scale, xr));
        let (ya, yb) = (vp.map_y(&bp.y_scale, ys[i]), vp.map_y(&bp.y_scale, ys_bottom[i]));
        let c = fill_colour_of(l, i, bp);
        ops.push(Primitive::Rect {
            x: xa.min(xb),
            y: ya.min(yb),
            w: (xb - xa).abs(),
            h: (yb - ya).abs(),
            fill: Some(Paint::new(c)),
            stroke: if has_outline {
                Some(Line::solid(point_colour_of(l, i, bp), mm_to_px(0.5)))
            } else {
                None
            },
        });
    }
}

fn draw_boxplot(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let xs = g.get("x").cloned().unwrap_or_default();
    let w = g.get("width").cloned().unwrap_or_else(|| vec![0.75; xs.len()]);
    let xmv = g.get("xmin").cloned();
    let xxv = g.get("xmax").cloned();
    // ggplot2 4.x: boxplot colour #333, linewidth 0.5 pt (0.5/0.75 px→0.667)
    let wp = mm_to_px(0.5);
    let line = Line::solid(Color::rgb(51, 51, 51), wp);
    for i in 0..xs.len() {
        let (q1, med, q3) = (g.get("ylower").map(|v| v[i]).unwrap_or(f64::NAN), g.get("ymiddle").map(|v| v[i]).unwrap_or(f64::NAN), g.get("yupper").map(|v| v[i]).unwrap_or(f64::NAN));
        let (wlo, whi) = (g.get("ymin").map(|v| v[i]).unwrap_or(f64::NAN), g.get("ymax").map(|v| v[i]).unwrap_or(f64::NAN));
        let (xb0, xb1) = match (&xmv, &xxv) {
            (Some(a), Some(b)) if i < a.len() => (vp.map_x(&bp.x_scale, a[i]), vp.map_x(&bp.x_scale, b[i])),
            _ => (vp.map_x(&bp.x_scale, xs[i] - w[i] / 2.0), vp.map_x(&bp.x_scale, xs[i] + w[i] / 2.0)),
        };
        let xc = (xb0 + xb1) / 2.0;
        let (yq1, yq3) = (vp.map_y(&bp.y_scale, q1), vp.map_y(&bp.y_scale, q3));
        let (ymed, ylo, yhi) = (vp.map_y(&bp.y_scale, med), vp.map_y(&bp.y_scale, wlo), vp.map_y(&bp.y_scale, whi));
        ops.push(Primitive::Rect { x: xb0, y: yq1.min(yq3), w: xb1 - xb0, h: (yq3 - yq1).abs(), fill: Some(Paint::new(Color::white())), stroke: Some(line) });
        ops.push(Primitive::Segment { x1: xb0, y1: ymed, x2: xb1, y2: ymed, stroke: line });
        ops.push(Primitive::Segment { x1: xc, y1: yq1, x2: xc, y2: ylo, stroke: line });
        ops.push(Primitive::Segment { x1: xc, y1: yq3, x2: xc, y2: yhi, stroke: line });
        ops.push(Primitive::Segment { x1: xb0, y1: ylo, x2: xb1, y2: ylo, stroke: line });
        ops.push(Primitive::Segment { x1: xb0, y1: yhi, x2: xb1, y2: yhi, stroke: line });
    }
    if let (Some(ox), Some(oy)) = (g.get("outlier_x"), g.get("outlier_y")) {
        let r = geom_defaults::point_size_px(11.0) * 0.5;
        for i in 0..ox.len() {
            ops.push(Primitive::Circle { cx: vp.map_x(&bp.x_scale, ox[i]), cy: vp.map_y(&bp.y_scale, oy[i]), r, fill: Some(Paint::new(Color::white())), stroke: Some(Line::solid(Color::rgb(51, 51, 51), mm_to_px(0.5))) });
        }
    }
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

fn draw_legend(sc: &mut Scene, bp: &BuiltPlot, cs: &crate::scale::DiscreteColourScale, theme: &Theme, width: f64, vp: &Viewport) {
    // Geometry probed from ggplot2 4.0.3 cairo refs:
    //   content right edge ≈ 720 - 5.48 - 5.48; keys 16px wide; label gap 7.1;
    //   vertical: block centred on panel middle; title centred over key column.
    let aes = if bp.fill_scale.as_ref().map(|s| std::ptr::eq(s, cs)).unwrap_or(false) { "fill" } else { "colour" };
    let ls = TextStyle { size: theme.small_text(), color: Color::black(), halign: TextAlign::Left, ..Default::default() };
    let key_w = 16.0;
    let key_pitch = 17.0;
    let title_h = ls.size;
    let gap = 7.1;
    let maxlabel = cs.levels.iter().map(|l| measure(l, &ls).width).fold(0.0f64, f64::max);
    let right_edge = width - 5.48 - 5.48;
    let label_left = right_edge - maxlabel;
    let key_left = label_left - gap - key_w;
    let title = bp
        .plot
        .labels
        .guides
        .get(aes)
        .or(cs.name.as_ref())
        .cloned()
        .unwrap_or_else(|| aes.to_string());
    // ggplot2 legend.title: rel(1) of base = 11pt, plain, black
    let tstyle = TextStyle {
        size: theme.base_size,
        color: Color::black(),
        ..ls.clone()
    };
    let n = cs.levels.len().max(1);
    let block_h = title_h + 4.0 + (n as f64) * key_pitch;
    let block_top = (vp.y0 + vp.y1) / 2.0 - block_h / 2.0;
    // title centred over the key column
    let tm = measure(&title, &tstyle);
    let tcx = key_left + key_w / 2.0;
    sc.layer(layer::LEGEND).push(Primitive::Text {
        content: title,
        x: tcx - tm.width / 2.0,
        y: block_top + title_h / 2.0,
        style: tstyle,
    });
    let keys_top = block_top + title_h + 4.0;
    let is_fill = aes == "fill";
    for (i, lvl) in cs.levels.iter().enumerate() {
        let cy = keys_top + i as f64 * key_pitch + key_pitch / 2.0 - 0.5;
        if is_fill {
            // rect key
            sc.layer(layer::LEGEND).push(Primitive::Rect {
                x: key_left,
                y: keys_top + i as f64 * key_pitch,
                w: key_w,
                h: 16.0,
                fill: Some(Paint::new(cs.map(lvl))),
                stroke: None,
            });
        } else {
            sc.layer(layer::LEGEND).push(Primitive::Circle {
                cx: key_left + key_w / 2.0,
                cy,
                r: geom_defaults::point_size_px(11.0) * 0.5,
                fill: Some(Paint::new(cs.map(lvl))),
                stroke: None,
            });
        }
        sc.layer(layer::LEGEND).push(Primitive::Text {
            content: lvl.clone(),
            x: label_left,
            y: cy,
            style: ls.clone(),
        });
    }
}

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
