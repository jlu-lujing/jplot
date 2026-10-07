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
use crate::theme::geom_defaults::{geom_lw, point_r_px, point_stroke, theme_lw};

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

/// One legend key glyph: the mark drawn next to a level label, mirroring
/// ggplot2's per-geom `draw_key_*`.
#[derive(Debug, Clone)]
enum KeyGlyph {
    Circle(Color),
    CircleR(f64, Color), // custom radius (size/alpha guides), colour
    Segment(Color),
    SegmentD(Option<Vec<f64>>, Color), // dashed segment (linetype guide)
    Rect(Color),
    Pch(i32, Color, Color), // shape, colour, fill
}

/// A legend guide: title + levels + per-level key glyphs.
struct Guide {
    title: String,
    levels: Vec<String>,
    glyphs: Vec<KeyGlyph>,
}

/// Collect one guide per mapped discrete aesthetic (fill, colour, shape),
/// using ggplot2's guide order (fill, colour, shape).
fn build_guides(bp: &BuiltPlot, ls: &TextStyle) -> Vec<Guide> {
    let mut guides: Vec<Guide> = Vec::new();
    let title_of = |aes: &str, name: Option<&String>| {
        bp.plot
            .labels
            .guides
            .get(aes)
            .or(name)
            .or(bp.guide_sources.get(aes))
            .cloned()
            .unwrap_or_else(|| aes.to_string())
    };
    let key_is_line = |aes: &str| {
        bp.layers
            .iter()
            .find(|l| l.aes.contains_key(aes))
            .map(|l| matches!(l.geom, crate::spec::GeomSpec::Line))
            .unwrap_or(false)
    };
    let key_is_bar = |aes: &str| {
        bp.layers
            .iter()
            .find(|l| l.aes.contains_key(aes))
            .map(|l| matches!(l.geom, crate::spec::GeomSpec::Col | crate::spec::GeomSpec::Bar | crate::spec::GeomSpec::Histogram { .. }))
            .unwrap_or(false)
    };
    for (aes, cs) in [("fill", &bp.fill_scale), ("colour", &bp.colour_scale)] {
        if let Some(cs) = cs {
            let glyphs = cs
                .levels
                .iter()
                .map(|l| {
                    let c = cs.map(l);
                    if key_is_line(aes) {
                        KeyGlyph::Segment(c)
                    } else if key_is_bar(aes) || aes == "fill" {
                        KeyGlyph::Rect(c)
                    } else {
                        KeyGlyph::Circle(c)
                    }
                })
                .collect();
            guides.push(Guide { title: title_of(aes, cs.name.as_ref()), levels: cs.levels.clone(), glyphs });
        }
    }
    if let Some((levels, pchs)) = bp.shape_scale.as_ref() {
        let colour = bp
            .layers
            .iter()
            .find(|l| l.aes.contains_key("shape"))
            .and_then(|l| l.args.colour_(&["colour", "color"]))
            .unwrap_or(Color::black());
        let _ = ls;
        let glyphs = pchs.iter().map(|&s| KeyGlyph::Pch(s as i32, colour, colour)).collect();
        guides.push(Guide { title: title_of("shape", None), levels: levels.clone(), glyphs });
    }
    // continuous size / alpha guides: keys are default-point circles whose
    // radius (size) or fill-opacity (alpha) varies with the break value.
    for (aes, ns, size_key) in [("size", &bp.size_scale, true), ("alpha", &bp.alpha_scale, false)] {
        if let Some(ns) = ns {
            let brk = ns.breaks();
            let levels = crate::scale::format_breaks(&brk);
            let colour = bp
                .layers
                .iter()
                .find(|l| l.aes.contains_key(aes))
                .and_then(|l| l.args.colour_(&["colour", "color"]))
                .unwrap_or(Color::black());
            let glyphs = brk
                .iter()
                .map(|&b| {
                    if size_key {
                        let mm = ns.map(b);
                        KeyGlyph::CircleR(point_r_px(mm) + point_stroke(0.5) * 0.5, colour)
                    } else {
                        KeyGlyph::CircleR(point_r_px(1.5), colour.with_alpha(ns.map(b)))
                    }
                })
                .collect();
            guides.push(Guide { title: ns.name.clone().unwrap_or_else(|| aes.into()), levels, glyphs });
        }
    }    // linetype guide: key = short line segment carrying the lty dash pattern
    if let Some((levels, lty)) = bp.linetype_scale.as_ref() {
        let colour = bp
            .layers
            .iter()
            .find(|l| l.aes.contains_key("linetype"))
            .and_then(|l| l.args.colour_(&["colour", "color"]))
            .unwrap_or(Color::black());
        let glyphs = lty
            .iter()
            .map(|t| KeyGlyph::SegmentD(crate::scene::dash_for(t, geom_lw(0.5)), colour))
            .collect();
        guides.push(Guide { title: bp.guide_sources.get("linetype").cloned().unwrap_or_else(|| "linetype".into()), levels: levels.clone(), glyphs });
    }

    guides
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
    let tick_line = Line::solid(Color::rgb(51, 51, 51), theme_lw(0.5)); // #333
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
        left += 13.06; // svglite gtable ylab-l column (probe 07_line)
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
    let guides: Vec<Guide> = build_guides(bp, &label_style);
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
                .map(|g| measure(&g.title, &ts).width / 2.0 - 16.0 / 2.0 + 5.12)
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
        let gl = Line::solid(gc, theme_lw(0.5));
        for &b in &x_breaks {
            let x = vp.map_x(&bp.x_scale, b);
            if x > vp.x0 && x < vp.x1 {
                sc.layer(layer::GRID).push(Primitive::Segment { x1: x, y1: vp.y0, x2: x, y2: vp.y1, stroke: gl.clone() });
            }
        }
        for &b in &y_breaks {
            let y = vp.map_y(&bp.y_scale, b);
            if y > vp.y0 && y < vp.y1 {
                sc.layer(layer::GRID).push(Primitive::Segment { x1: vp.x0, y1: y, x2: vp.x1, y2: y, stroke: gl.clone() });
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
        draw_layer(&mut data, l, bp, &vp);
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
        let mut push_label = |s: &mut Scene, content: &str, cx: f64, baseline: f64, st: TextStyle| {
            s.layer(layer::AXES).push(Primitive::Text { content: content.into(), x: cx, y: baseline, style: st });
        };
        for (&b, lab) in x_breaks.iter().zip(x_labels.iter()) {
            let x = vp.map_x(&bp.x_scale, b);
            if x < vp.x0 - 0.5 || x > vp.x1 + 0.5 {
                continue;
            }
            sc.layer(layer::AXES).push(Primitive::Segment { x1: x, y1: vp.y1, x2: x, y2: vp.y1 + tick, stroke: al.clone() });
            push_label(&mut sc, lab, x, vp.y1 + tick + 2.42 + 0.76 * label_style.size, label_style.clone());
        }
        for (&b, lab) in y_breaks.iter().zip(y_labels.iter()) {
            let y = vp.map_y(&bp.y_scale, b);
            if y < vp.y0 - 0.5 || y > vp.y1 + 0.5 {
                continue;
            }
            sc.layer(layer::AXES).push(Primitive::Segment { x1: vp.x0 - tick, y1: y, x2: vp.x0, y2: y, stroke: al.clone() });
            let st = TextStyle { halign: TextAlign::Right, ..label_style.clone() };
            sc.layer(layer::AXES).push(Primitive::Text { content: lab.clone(), x: vp.x0 - tick - hl, y: y + 0.31 + 0.76 * st.size, style: st });
        }
        if !x_lab.is_empty() {
            // svglite: "disp" baseline = 472.20 (= height - 7.80), size 11, centred
            let ts = TextStyle { size: theme.base_size, ..label_style.clone() };
            sc.layer(layer::TITLES).push(Primitive::Text { content: x_lab.clone(), x: (vp.x0 + vp.x1) / 2.0, y: height - 7.80, style: ts });
        }
        if !y_lab.is_empty() {
            // svglite: translate(13.36, 226.99) rotate(-90) anchor=middle
            let ts = TextStyle { size: theme.base_size, angle: 90.0, ..label_style.clone() };
            sc.layer(layer::TITLES).push(Primitive::Text {
                content: y_lab.clone(),
                x: 13.36,
                y: (vp.y0 + vp.y1) / 2.0,
                style: ts,
            });
        }
    }

    // --- titles / caption ----------------------------------------------------
    if let Some(t) = &spec.labels.title {
        let ts = TextStyle { size: theme.large_text(), halign: TextAlign::Left, ..Default::default() };
        // svglite title baseline = 14.93 (size 13.2)
        sc.layer(layer::TITLES).push(Primitive::Text { content: t.clone(), x: vp.x0, y: 14.93, style: ts });
    }
    if let Some(t) = &spec.labels.subtitle {
        let ts = TextStyle { size: theme.small_text(), halign: TextAlign::Left, ..Default::default() };
        let y = 14.93 + theme.large_text() + ts.size * 0.35;
        sc.layer(layer::TITLES).push(Primitive::Text { content: t.clone(), x: vp.x0, y, style: ts });
    }
    if let Some(c) = &spec.labels.caption {
        let ts = TextStyle { size: theme.small_text(), halign: TextAlign::Right, ..Default::default() };
        sc.layer(layer::TITLES).push(Primitive::Text { content: c.clone(), x: vp.x1, y: height - hl - ts.size / 2.0, style: ts });
    }

    // --- legend --------------------------------------------------------------
    if !guides.is_empty() {
        draw_legends(&mut sc, &guides, &theme, width, &vp);
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
            matches!(l.geom, GeomSpec::Bar | GeomSpec::Histogram { .. } | GeomSpec::Freqpoly { .. })
                && !l.aes.contains_key("y")
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
        GeomSpec::Line | GeomSpec::Freqpoly { .. } => draw_lines(ops, l, bp, vp),
        GeomSpec::Step => draw_step(ops, l, bp, vp),
        GeomSpec::Col | GeomSpec::Bar | GeomSpec::Histogram { .. } => draw_bars(ops, l, bp, vp),
        GeomSpec::Boxplot => draw_boxplot(ops, l, bp, vp),
        GeomSpec::Hline => draw_hline(ops, l, bp, vp),
        GeomSpec::Vline => draw_vline(ops, l, bp, vp),
        GeomSpec::Text => draw_text(ops, l, bp, vp),
        GeomSpec::Area => draw_area(ops, l, bp, vp),
        GeomSpec::Errorbar => draw_errorbar(ops, l, bp, vp),
        GeomSpec::Ribbon => draw_ribbon(ops, l, bp, vp),
    }
}

/// geom_errorbar: a vertical line from ymin to ymax at each x, with horizontal
/// caps of width `width` (data units, default 0.9) at both ends.
fn draw_errorbar(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let xs = l.frame.get("x").cloned().unwrap_or_default();
    let ymin = l.frame.get("ymin").cloned().unwrap_or_default();
    let ymax = l.frame.get("ymax").cloned().unwrap_or_default();
    let width = l.args.f64_("width").unwrap_or(0.9);
    let lw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let n = xs.len().min(ymin.len()).min(ymax.len());
    for i in 0..n {
        if !xs[i].is_finite() || !ymin[i].is_finite() || !ymax[i].is_finite() {
            continue;
        }
        let (px, p0, p1) = (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ymin[i]), vp.map_y(&bp.y_scale, ymax[i]));
        let (cl, cr) = (vp.map_x(&bp.x_scale, xs[i] - width / 2.0), vp.map_x(&bp.x_scale, xs[i] + width / 2.0));
        let c = point_colour_of(l, i, bp);
        let ln = Line::solid(c, lw);
        ops.push(Primitive::Segment { x1: px, y1: p0, x2: px, y2: p1, stroke: ln.clone() });
        ops.push(Primitive::Segment { x1: cl, y1: p0, x2: cr, y2: p0, stroke: ln.clone() });
        ops.push(Primitive::Segment { x1: cl, y1: p1, x2: cr, y2: p1, stroke: ln.clone() });
    }
}

/// geom_ribbon: a filled band between ymin and ymax across x, per group.
fn draw_ribbon(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let xs = l.frame.get("x").cloned().unwrap_or_default();
    let ymin = l.frame.get("ymin").cloned().unwrap_or_default();
    let ymax = l.frame.get("ymax").cloned().unwrap_or_default();
    let n_groups = l.group_ids.iter().cloned().max().map_or(0, |m| m + 1);
    for g in 0..n_groups.max(1) {
        let mut idx: Vec<usize> = (0..xs.len()).filter(|&i| n_groups <= 1 || l.group_ids.get(i) == Some(&g)).collect();
        if idx.is_empty() {
            continue;
        }
        let rep = idx[0];
        idx.sort_by_key(|&i| (xs[i] * 1e6).round() as i64);
        let top: Vec<(f64, f64)> = idx
            .iter()
            .filter(|&&i| i < ymax.len() && xs[i].is_finite() && ymax[i].is_finite())
            .map(|&i| (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ymax[i])))
            .collect();
        let bot: Vec<(f64, f64)> = idx
            .iter()
            .filter(|&&i| i < ymin.len() && xs[i].is_finite() && ymin[i].is_finite())
            .rev()
            .map(|&i| (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ymin[i])))
            .collect();
        if top.len() < 2 {
            continue;
        }
        let alpha = l.args.f64_("alpha").unwrap_or(1.0);
        let c = if l.aes.contains_key("fill") || l.args.s("fill").is_some() {
            fill_colour_of(l, rep, bp)
        } else {
            Color::rgb(51, 51, 51)
        }
        .with_alpha(alpha);
        let mut poly = top;
        poly.extend(bot);
        ops.push(Primitive::Polyline { points: poly, stroke: None, fill: Some(Paint::new(c)), closed: true });
    }
}

/// geom_text: labels at (x, y). size is MILLIMETRES (ggplot2 text unit);
/// svglite renders it at size * 72.27/25.4 pt. hjust/vjust 0..1 (default .5).
fn draw_text(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let labels = l.frame.cat.get("label").cloned().unwrap_or_default();
    if labels.is_empty() {
        return;
    }
    let size_mm = l.args.f64_("size").unwrap_or(11.0 / (72.27 / 25.4));
    let size_px = size_mm * (72.27 / 25.4);
    let hjust = l.args.f64_("hjust").unwrap_or(0.5);
    let vjust = l.args.f64_("vjust").unwrap_or(0.5);
    let alpha = l.args.f64_("alpha").unwrap_or(1.0);
    let colour = l.args.colour_(&["colour", "color"]).unwrap_or(Color::black()).with_alpha(alpha);
    let xs = l.frame.get("x").cloned().unwrap_or_default();
    let ys = l.frame.get("y").cloned().unwrap_or_default();
    let style = TextStyle { size: size_px, color: colour, halign: TextAlign::Left, ..Default::default() };
    for i in 0..labels.len().min(xs.len()).min(ys.len()) {
        if !xs[i].is_finite() || !ys[i].is_finite() {
            continue;
        }
        let (px, py) = (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ys[i]));
        let m = measure(&labels[i], &style);
        // grid text justification, probed from svglite geom_text(vjust=0/0.5/1):
        // vjust=0 puts the baseline at the anchor, vjust=1 shifts it down by
        // 0.716*size (the grid "text box" height for Arial at this size);
        // hjust=0 left edge at x, =0.5 centred, =1 right edge at x.
        let x = px - hjust * m.width;
        let baseline = py + vjust * 0.716 * size_px;
        ops.push(Primitive::Text { content: labels[i].clone(), x, y: baseline, style: style.clone() });
    }
}

/// geom_area: per-group filled polygon between y=0 (or ymin) and y.
fn draw_area(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let xs = l.frame.get("x").cloned().unwrap_or_default();
    let ys = l.frame.get("y").cloned().unwrap_or_default();
    let base = l.frame.get("ymin").cloned().unwrap_or_else(|| vec![0.0; ys.len()]);
    let n_groups = l.group_ids.iter().cloned().max().map_or(0, |m| m + 1);
    for g in 0..n_groups.max(1) {
        let mut idx: Vec<usize> = (0..xs.len()).filter(|&i| n_groups <= 1 || l.group_ids.get(i) == Some(&g)).collect();
        if idx.is_empty() {
            continue;
        }
        let rep = idx[0];
        idx.sort_by_key(|&i| (xs[i] * 1e6).round() as i64);
        let pts: Vec<(f64, f64)> = idx
            .iter()
            .filter(|&&i| xs[i].is_finite() && ys[i].is_finite())
            .map(|&i| (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ys[i])))
            .collect();
        let base_pts: Vec<(f64, f64)> = idx
            .iter()
            .filter(|&&i| xs[i].is_finite() && base[i].is_finite())
            .map(|&i| (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, base[i])))
            .collect();
        if pts.len() < 2 {
            continue;
        }
        // geom_area default fill = col_mix(ink, paper, 0.2) = #333 (differs
        // from geom_bar's #595959); honour an explicit fill aes/arg otherwise.
        let c = if l.aes.contains_key("fill") || l.args.s("fill").is_some() {
            fill_colour_of(l, rep, bp)
        } else {
            Color::rgb(51, 51, 51)
        };
        let mut poly = pts.clone();
        poly.extend(base_pts.iter().rev().copied());
        ops.push(Primitive::Polyline { points: poly, stroke: None, fill: Some(Paint::new(c)), closed: true });
    }
}

/// geom_step direction="hv": hold y, then step vertically at the next x.
fn draw_step(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let width_px = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let (xs, ys) = (l.frame.get("x").cloned().unwrap_or_default(), l.frame.get("y").cloned().unwrap_or_default());
    let n_groups = l.group_ids.iter().cloned().max().map_or(0, |m| m + 1);
    for g in 0..n_groups.max(1) {
        let mut idx: Vec<usize> = (0..xs.len()).filter(|&i| n_groups <= 1 || l.group_ids.get(i) == Some(&g)).collect();
        if idx.is_empty() {
            continue;
        }
        let rep = idx[0];
        idx.sort_by_key(|&i| (xs[i] * 1e6).round() as i64);
        let pts: Vec<(f64, f64)> = idx
            .into_iter()
            .filter(|&i| xs[i].is_finite() && ys[i].is_finite())
            .map(|i| (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ys[i])))
            .collect();
        if pts.len() < 2 {
            continue;
        }
        // insert a horizontal carry point (x[i+1], y[i]) before each vertical jump
        let mut stair: Vec<(f64, f64)> = vec![pts[0]];
        for w in pts.windows(2) {
            stair.push((w[1].0, w[0].1));
            stair.push(w[1]);
        }
        ops.push(Primitive::Polyline {
            points: stair,
            stroke: Some(Line::solid(point_colour_of(l, rep, bp), width_px)),
            fill: None,
            closed: false,
        });
    }
}

/// geom_hline / geom_vline: constant lines across the panel. Intercept comes
/// from the mapped aes column ("y"/"x") or the yintercept/xintercept param.
fn draw_hline(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let width_px = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let ys = l
        .frame
        .get("y")
        .cloned()
        .or_else(|| l.args.f64_("yintercept").map(|v| vec![v]))
        .unwrap_or_default();
    let n = l.frame.n.max(1);
    for (i, &y) in ys.iter().enumerate() {
        if !y.is_finite() {
            continue;
        }
        // constant lines span the panel regardless of y_scale censoring
        let py = vp.map_y_plain(&bp.y_scale, y);
        let c = point_colour_of(l, i.min(n - 1), bp);
        ops.push(Primitive::Segment { x1: vp.x0, y1: py, x2: vp.x1, y2: py, stroke: Line::solid(c, width_px) });
    }
}

fn draw_vline(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let width_px = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let xs = l
        .frame
        .get("x")
        .cloned()
        .or_else(|| l.args.f64_("xintercept").map(|v| vec![v]))
        .unwrap_or_default();
    let n = l.frame.n.max(1);
    for (i, &x) in xs.iter().enumerate() {
        if !x.is_finite() {
            continue;
        }
        // constant lines span the panel regardless of x_scale censoring
        let px = vp.map_x_plain(&bp.x_scale, x);
        let c = point_colour_of(l, i.min(n - 1), bp);
        ops.push(Primitive::Segment { x1: px, y1: vp.y0, x2: px, y2: vp.y1, stroke: Line::solid(c, width_px) });
    }
}

/// dash pattern for a row's linetype (mapped column / arg / solid).
fn linetype_of(l: &crate::build::BuiltLayer, i: usize, stroke_px: f64) -> Option<Vec<f64>> {
    let name = l
        .frame
        .cat
        .get("linetype")
        .and_then(|v| v.get(i).cloned())
        .or_else(|| l.args.s("linetype").map(|s| s.to_string()));
    match name {
        Some(n) => crate::scene::dash_for(&n, stroke_px),
        None => None,
    }
}

fn point_colour_of(l: &crate::build::BuiltLayer, i: usize, bp: &BuiltPlot) -> Color {
    let alpha = l.args.f64_("alpha").unwrap_or(1.0);
    if let Some(c) = l.args.colour_(&["colour", "color"]) {
        return c.with_alpha(alpha);
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
    let alpha = l.args.f64_("alpha").unwrap_or(1.0);
    if let Some(c) = l.args.colour_(&["fill"]) {
        return c.with_alpha(alpha);
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

fn point_fill_of(l: &crate::build::BuiltLayer, i: usize, bp: &BuiltPlot) -> Color {
    let alpha = l.args.f64_("alpha").unwrap_or(1.0);
    if let Some(c) = l.args.colour_(&["fill"]) {
        return c.with_alpha(alpha);
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
    // ggplot2 default fill aesthetic is NA → transparent interior
    Color::transparent()
}

/// Push one pch glyph centred at (cx,cy). `r` = outer radius (size-derived,
/// including half the border stroke), `stk` = border width px.
/// shapes: 0 square, 1 circle, 2 tri, 3 plus, 4 cross, 5 diamond (all hollow
/// stroke=col); 15 sq,16 cir,17 tri,18 diamond solid(fill=col); 19 cir,20 small
/// cir solid; 21 cir,22 sq,23 dia,24 tri-up,25 tri-down fillable(fill=fill,stroke=col).
fn push_glyph(ops: &mut Vec<Primitive>, shape: i32, cx: f64, cy: f64, r: f64, stk: f64, col: Color, fill: Color) {
    let fillable = (21..=25).contains(&shape);
    let solid = (15..=20).contains(&shape);
    let interior = if fillable { fill } else if solid { col } else { Color::transparent() };
    let border = if solid { Color::transparent() } else { col };
    let stroke = Line::solid(border, stk);
    let tri_up = |rr: f64| vec![(cx, cy - 1.556 * rr), (cx - 1.348 * rr, cy + 0.777 * rr), (cx + 1.348 * rr, cy + 0.777 * rr)];
    let tri_dn = |rr: f64| vec![(cx, cy + 1.556 * rr), (cx - 1.348 * rr, cy - 0.777 * rr), (cx + 1.348 * rr, cy - 0.777 * rr)];
    match shape {
        0 | 15 | 22 => ops.push(Primitive::Rect { x: cx - r, y: cy - r, w: 2.0 * r, h: 2.0 * r, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, stroke: if border.a == 0.0 { None } else { Some(stroke) } }),
        20 => ops.push(Primitive::Circle { cx, cy, r: r * 2.0 / 3.0, fill: Some(Paint::new(col)), stroke: None }),
        s if matches!(s, 1 | 16 | 19 | 21) => ops.push(Primitive::Circle { cx, cy, r, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, stroke: if border.a == 0.0 { None } else { Some(stroke) } }),
        2 | 17 | 24 => ops.push(Primitive::Polyline { points: tri_up(r), stroke: if border.a == 0.0 { None } else { Some(stroke) }, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, closed: true }),
        25 => ops.push(Primitive::Polyline { points: tri_dn(r), stroke: Some(stroke), fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, closed: true }),
        5 | 18 | 23 => ops.push(Primitive::Polyline { points: vec![(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)], stroke: if border.a == 0.0 { None } else { Some(stroke) }, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, closed: true }),
        3 => {
            let a = 1.414 * r;
            ops.push(Primitive::Segment { x1: cx - a, y1: cy, x2: cx + a, y2: cy, stroke });
            ops.push(Primitive::Segment { x1: cx, y1: cy - a, x2: cx, y2: cy + a, stroke: Line::solid(border, stk) });
        }
        4 | 8 => {
            let a = 1.414 * r * 0.7071;
            ops.push(Primitive::Segment { x1: cx - a, y1: cy - a, x2: cx + a, y2: cy + a, stroke: Line::solid(border, stk) });
            ops.push(Primitive::Segment { x1: cx - a, y1: cy + a, x2: cx + a, y2: cy - a, stroke: Line::solid(border, stk) });
            if shape == 8 {
                let b = 1.414 * r;
                ops.push(Primitive::Segment { x1: cx - b, y1: cy, x2: cx + b, y2: cy, stroke: Line::solid(border, stk) });
                ops.push(Primitive::Segment { x1: cx, y1: cy - b, x2: cx, y2: cy + b, stroke: Line::solid(border, stk) });
            }
        }
        // default: solid circle (theme pointshape 19)
        _ => ops.push(Primitive::Circle { cx, cy, r, fill: Some(Paint::new(col)), stroke: None }),
    }
}

fn draw_points(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let size = l.args.f64_("size").unwrap_or(1.5);
    let stroke = l.args.f64_("stroke").unwrap_or(0.5);
    let default_shape = l.args.f64_("shape").unwrap_or(19.0) as i32;
    let (xs, ys) = (l.frame.get("x").cloned().unwrap_or_default(), l.frame.get("y").cloned().unwrap_or_default());
    let shapes = l.frame.get("shape").cloned();
    let sizes = l.frame.get("size").cloned();
    let alphas = l.frame.get("alpha").cloned();
    let arg_alpha = l.args.f64_("alpha").unwrap_or(1.0);
    for i in 0..xs.len().min(ys.len()) {
        if !xs[i].is_finite() || !ys[i].is_finite() {
            continue;
        }
        // mapped size (mm) / alpha override the defaults per row
        let size = sizes.as_ref().and_then(|v| v.get(i).copied()).unwrap_or(size);
        let alpha = alphas.as_ref().and_then(|v| v.get(i).copied()).unwrap_or(arg_alpha);
        let r = point_r_px(size) + point_stroke(stroke) * 0.5;
        let stk = point_stroke(stroke);
        let shape = shapes.as_ref().and_then(|v| v.get(i).copied()).map(|s| s as i32).unwrap_or(default_shape);
        let mut c = point_colour_of(l, i, bp);
        let mut f = point_fill_of(l, i, bp);
        c.a *= alpha;
        f.a *= alpha;
        let (cx, cy) = (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ys[i]));
        push_glyph(ops, shape, cx, cy, r, stk, c, f);
    }
}

fn draw_lines(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let width_px = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
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
        let lt = linetype_of(l, rep, width_px);
        let mut stroke = Line::solid(point_colour_of(l, rep, bp), width_px);
        stroke.dash = lt;
        ops.push(Primitive::Polyline {
            points: pts,
            stroke: Some(stroke),
            fill: None,
            closed: false,
        });
    }
}

fn draw_bars(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let xs = l.frame.get("x").cloned().unwrap_or_default();
    let ys = l.frame.get("y").cloned().unwrap_or_default();
    // ggplot2 geom_bar/col/histogram default colour = NA (no outline).
    let has_outline = l.aes.contains_key("colour")
        || l.args.s("colour").is_some()
        || l.args.s("color").is_some();
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
                let blw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
                Some(Line::solid(point_colour_of(l, i, bp), blw))
            } else {
                None
            },
        });
    }
}

fn draw_boxplot(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let a = &l.args;
    let xs = g.get("x").cloned().unwrap_or_default();
    let w = g.get("width").cloned().unwrap_or_else(|| vec![0.75; xs.len()]);
    let xmv = g.get("xmin").cloned();
    let xxv = g.get("xmax").cloned();

    // ggplot2 GeomBoxplot defaults (refs/ggplot2 + api_inventory.json):
    //   colour = col_mix(ink,paper,0.2) = #333;  linewidth = borderwidth (0.5mm)
    //   fill = paper(white);  median linewidth = linewidth * fatten(2)
    //   outliers: shape 19, size = pointsize, stroke 0.5, fill NA
    //   staplewidth = 0 → NO whisker caps drawn
    //   notch = FALSE, notchwidth = 0.5 ; varwidth handled in build()
    // user-supplied *linewidth values are in MILLIMETRES (ggplot2 semantics)
    // and must go through geom_lw(); the 0.5 default is already a nominal mm.
    let base_colour = Color::rgb(51, 51, 51);
    let lw = a.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let box_colour = a.colour_(&["box.colour", "box.color"]).unwrap_or(base_colour);
    let box_lw = a.f64_("box.linewidth").map(geom_lw).unwrap_or(lw);
    let whisker_colour = a.colour_(&["whisker.colour", "whisker.color"]).unwrap_or(base_colour);
    let whisker_lw = a.f64_("whisker.linewidth").map(geom_lw).unwrap_or(lw);
    let staple_colour = a.colour_(&["staple.colour", "staple.color"]).unwrap_or(base_colour);
    let staple_lw = a.f64_("staple.linewidth").map(geom_lw).unwrap_or(lw);
    let median_colour = a.colour_(&["median.colour", "median.color"]).unwrap_or(base_colour);
    let median_lw = a.f64_("median.linewidth").map(geom_lw).unwrap_or(lw * 2.0); // fatten = 2
    let notch = a.bool_("notch").unwrap_or(false);
    let notchwidth = a.f64_("notchwidth").unwrap_or(0.5);
    let staplewidth = a.f64_("staplewidth").unwrap_or(0.0);
    let box_line = Line::solid(box_colour, box_lw);
    let median_line = Line::solid(median_colour, median_lw);
    let whisker_line = Line::solid(whisker_colour, whisker_lw);
    let staple_line = Line::solid(staple_colour, staple_lw);

    for i in 0..xs.len() {
        let (q1, med, q3) = (
            g.get("ylower").map(|v| v[i]).unwrap_or(f64::NAN),
            g.get("ymiddle").map(|v| v[i]).unwrap_or(f64::NAN),
            g.get("yupper").map(|v| v[i]).unwrap_or(f64::NAN),
        );
        let (wlo, whi) = (
            g.get("ymin").map(|v| v[i]).unwrap_or(f64::NAN),
            g.get("ymax").map(|v| v[i]).unwrap_or(f64::NAN),
        );
        let (xb0, xb1) = match (&xmv, &xxv) {
            (Some(a), Some(b)) if i < a.len() => (vp.map_x(&bp.x_scale, a[i]), vp.map_x(&bp.x_scale, b[i])),
            _ => (vp.map_x(&bp.x_scale, xs[i] - w[i] / 2.0), vp.map_x(&bp.x_scale, xs[i] + w[i] / 2.0)),
        };
        let xc = (xb0 + xb1) / 2.0;
        let (yq1, yq3) = (vp.map_y(&bp.y_scale, q1), vp.map_y(&bp.y_scale, q3));
        let (ymed, ylo, yhi) = (vp.map_y(&bp.y_scale, med), vp.map_y(&bp.y_scale, wlo), vp.map_y(&bp.y_scale, whi));

        let has_notch = notch
            && g.get("notchlower").map(|v| v[i]).map(|v| v.is_finite()).unwrap_or(false)
            && g.get("notchupper").map(|v| v[i]).map(|v| v.is_finite()).unwrap_or(false);
        if has_notch {
            // GeomCrossbar box polygon (refs/ggplot2/R/geom-crossbar.R)
            let nlo = g.get("notchlower").unwrap()[i];
            let nhi = g.get("notchupper").unwrap()[i];
            let ind = (1.0 - notchwidth) * (xb1 - xb0) / 2.0;
            let (ynlo, ynhi) = (vp.map_y(&bp.y_scale, nlo), vp.map_y(&bp.y_scale, nhi));
            let pts = vec![
                (xb0, yq3), (xb0, ynhi), (xb0 + ind, ymed), (xb0, ynlo), (xb0, yq1),
                (xb1, yq1), (xb1, ynlo), (xb1 - ind, ymed), (xb1, ynhi), (xb1, yq3),
            ];
            ops.push(Primitive::Polyline { points: pts, stroke: Some(box_line.clone()), fill: Some(Paint::new(Color::white())), closed: true });
            ops.push(Primitive::Segment { x1: xb0 + ind, y1: ymed, x2: xb1 - ind, y2: ymed, stroke: median_line.clone() });
        } else {
            ops.push(Primitive::Rect {
                x: xb0, y: yq3.min(yq1), w: xb1 - xb0, h: (yq1 - yq3).abs(),
                fill: Some(Paint::new(Color::white())), stroke: Some(box_line.clone()),
            });
            ops.push(Primitive::Segment { x1: xb0, y1: ymed, x2: xb1, y2: ymed, stroke: median_line.clone() });
        }
        // whiskers: vertical centre line, box→whisker extreme (both ends)
        ops.push(Primitive::Segment { x1: xc, y1: yq3, x2: xc, y2: yhi, stroke: whisker_line.clone() });
        ops.push(Primitive::Segment { x1: xc, y1: yq1, x2: xc, y2: ylo, stroke: whisker_line.clone() });
        // staples only when staplewidth != 0
        if staplewidth != 0.0 {
            let half = (xb1 - xb0) * staplewidth / 2.0;
            ops.push(Primitive::Segment { x1: xc - half, y1: yhi, x2: xc + half, y2: yhi, stroke: staple_line.clone() });
            ops.push(Primitive::Segment { x1: xc - half, y1: ylo, x2: xc + half, y2: ylo, stroke: staple_line.clone() });
        }
    }
    // outliers (only if not dropped via outliers=FALSE)
    if let (Some(ox), Some(oy)) = (g.get("outlier_x"), g.get("outlier_y")) {
        let shape = a.f64_("outlier.shape").unwrap_or(19.0);
        let r = a.f64_("outlier.size").map(point_r_px).unwrap_or(geom_defaults::point_size_px(11.0) * 0.5);
        let stroke_w = a.f64_("outlier.stroke").unwrap_or(point_stroke(0.5));
        let o_colour = a.colour_(&["outlier.colour", "outlier.color"]).unwrap_or(base_colour);
        let o_fill = a.colour_(&["outlier.fill"]);
        let alpha = a.f64_("outlier.alpha");
        let o_colour = match alpha { Some(al) => o_colour.with_alpha(al), None => o_colour };
        for i in 0..ox.len() {
            let (cx, cy) = (vp.map_x(&bp.x_scale, ox[i]), vp.map_y(&bp.y_scale, oy[i]));
            // shapes: 19 solid circle; 17 solid triangle; 1/0 open circle; 21 fill+stroke
            match shape as i32 {
                17 | 2 => {
                    let pts = vec![(cx, cy - r * 1.15), (cx - r, cy + r * 0.8), (cx + r, cy + r * 0.8)];
                    let solid = shape as i32 == 17;
                    ops.push(Primitive::Polyline {
                        points: pts,
                        stroke: Some(Line::solid(o_colour, if solid { stroke_w } else { point_stroke(0.5) })),
                        fill: if solid { Some(Paint::new(o_fill.unwrap_or(o_colour))) } else { o_fill.map(Paint::new) },
                        closed: true,
                    });
                }
                21 => ops.push(Primitive::Circle { cx, cy, r, fill: Some(Paint::new(o_fill.unwrap_or(Color::white()))), stroke: Some(Line::solid(o_colour, stroke_w)) }),
                0 | 1 => ops.push(Primitive::Circle { cx, cy, r, fill: o_fill.map(Paint::new), stroke: Some(Line::solid(o_colour, point_stroke(0.5))) }),
                _ => ops.push(Primitive::Circle { cx, cy, r, fill: Some(Paint::new(o_colour)), stroke: None }),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

/// Stacked guide blocks (fill → colour → shape). Geometry probed from
/// ggplot2 4.0.3 svglite refs:
///   single guide: title bl 204.43, keys 222.14/237.98/253.82
///   (title→first key 17.7, pitch 15.84); two guides: last key→next title 35.8;
///   key label baseline = key centre + 3.15; label column right-aligned at
///   width - 10.96; title centred over the key column.
fn draw_legends(sc: &mut Scene, guides: &[Guide], theme: &Theme, width: f64, vp: &Viewport) {
    let ls = TextStyle { size: theme.small_text(), color: Color::black(), halign: TextAlign::Left, ..Default::default() };
    let ts = TextStyle { size: theme.base_size, color: Color::black(), halign: TextAlign::Left, ..Default::default() };
    let key_w = 16.0;
    let pitch = 15.84;
    let title_to_first = 17.7;
    let inter = 35.8;
    let right_edge = width - 5.48 - 5.48;
    let maxlabel = guides
        .iter()
        .flat_map(|g| g.levels.iter())
        .map(|l| measure(l, &ls).width)
        .fold(0.0f64, f64::max);
    let label_left = right_edge - maxlabel;
    let key_left = label_left - 7.1 - key_w;
    let key_cx = key_left + key_w / 2.0;

    // vertical layout: first-title baseline centred on the panel middle so
    // the whole stack's baseline span is balanced (probe: +2.11 offset).
    let total: f64 = guides
        .iter()
        .map(|g| title_to_first + (g.levels.len().max(1) as f64 - 1.0) * pitch)
        .sum::<f64>()
        + inter * (guides.len().saturating_sub(1)) as f64;
    let mut title_bl = (vp.y0 + vp.y1) / 2.0 - total / 2.0 + 2.11;

    for g in guides {
        let tm = measure(&g.title, &ts);
        sc.layer(layer::LEGEND).push(Primitive::Text {
            content: g.title.clone(),
            x: key_cx - tm.width / 2.0,
            y: title_bl,
            style: ts.clone(),
        });
        for (i, lvl) in g.levels.iter().enumerate() {
            let key_bl = title_bl + title_to_first + i as f64 * pitch;
            let cy = key_bl - 3.15;
            let r = geom_defaults::point_r_px(1.5);
            match &g.glyphs[i] {
                KeyGlyph::Segment(c) | KeyGlyph::SegmentD(None, c) => sc.layer(layer::LEGEND).push(Primitive::Segment {
                    x1: key_left,
                    y1: cy,
                    x2: key_left + key_w,
                    y2: cy,
                    stroke: Line::solid(*c, geom_lw(0.5)),
                }),
                KeyGlyph::SegmentD(Some(d), c) => {
                    let mut ln = Line::solid(*c, geom_lw(0.5));
                    ln.dash = Some(d.clone());
                    sc.layer(layer::LEGEND).push(Primitive::Segment {
                        x1: key_left,
                        y1: cy,
                        x2: key_left + key_w,
                        y2: cy,
                        stroke: ln,
                    });
                }
                KeyGlyph::Rect(c) => sc.layer(layer::LEGEND).push(Primitive::Rect {
                    x: key_left,
                    y: cy - 8.0,
                    w: key_w,
                    h: 16.0,
                    fill: Some(Paint::new(*c)),
                    stroke: None,
                }),
                KeyGlyph::Circle(c) => sc.layer(layer::LEGEND).push(Primitive::Circle {
                    cx: key_cx,
                    cy,
                    r: geom_defaults::point_r_px(1.5),
                    fill: Some(Paint::new(*c)),
                    stroke: Some(Line::solid(*c, point_stroke(0.5))),
                }),
                KeyGlyph::CircleR(r, c) => sc.layer(layer::LEGEND).push(Primitive::Circle {
                    cx: key_cx,
                    cy,
                    r: *r,
                    fill: Some(Paint::new(*c)),
                    stroke: Some(Line::solid(*c, point_stroke(0.5))),
                }),
                KeyGlyph::Pch(s, c, f) => {
                    let mut glyph: Vec<Primitive> = Vec::new();
                    push_glyph(
                        &mut glyph,
                        *s,
                        key_cx,
                        cy,
                        r + point_stroke(0.5) * 0.5,
                        point_stroke(0.5),
                        *c,
                        *f,
                    );
                    for p in glyph {
                        sc.layer(layer::LEGEND).push(p);
                    }
                }
            }
            sc.layer(layer::LEGEND).push(Primitive::Text {
                content: lvl.clone(),
                x: label_left,
                y: key_bl,
                style: ls.clone(),
            });
        }
        title_bl += title_to_first + (g.levels.len().max(1) as f64 - 1.0) * pitch + inter;
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
