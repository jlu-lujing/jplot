//! Layout: BuiltPlot → Scene (device px @72dpi → pt-equal).
//!
//! Conventions calibrated against ggplot2 4.0.3 cairo (compare/probe.R):
//! panel grey is grey92 (0.9216), canvas 720×480 for a scatter with default
//! gutters left≈36px top≈9px; axis-tick length 0.25cm=7.1pt; half_line=5.5pt.
//! Text boxes are positioned by their *centre*; halign shifts horizontally.

use crate::build::{BuiltPlot, BuiltScale};
pub use crate::geom;
use crate::scene::{layer, Line, Paint, Primitive, Scene, TextAlign, TextStyle};
use crate::scale::Color;
use crate::text::{left_edge, measure};
use crate::probes;
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

/// A legend guide: title + levels + per-level key glyphs, or a gradient bar.
struct Guide {
    title: String,
    levels: Vec<String>,
    glyphs: Vec<KeyGlyph>,
    /// continuous colour bar: (data_lo, data_hi, ramp low, ramp high)
    bar: Option<(f64, f64, Vec<String>)>,
    /// bar guide: normalised tick positions (t of each level label)
    bar_ticks: Vec<f64>,
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
            if cs.levels.is_empty() && cs.data_hi != cs.data_lo {
                // continuous colour: gradient bar guide (probe: bar h 79.2,
                // block-centre span title_to_first + 63.3)
                let (lo, hi) = (cs.data_lo, cs.data_hi);
                let brk = crate::scale::extended_breaks(lo, hi, 5)
                    .into_iter()
                    .filter(|b| *b >= lo && *b <= hi)
                    .collect::<Vec<_>>();
                let levels = crate::scale::format_breaks(&brk);
                guides.push(Guide {
                    title: title_of(aes, cs.name.as_ref()),
                    levels,
                    glyphs: vec![],
                    bar: Some((lo, hi, cs.ramp.clone())),
                    bar_ticks: brk.iter().map(|b| (b - lo) / (hi - lo)).collect(),
                });
                continue;
            }
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
            guides.push(Guide { title: title_of(aes, cs.name.as_ref()), levels: cs.levels.clone(), glyphs, bar: None, bar_ticks: vec![] });
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
        guides.push(Guide { title: title_of("shape", None), levels: levels.clone(), glyphs, bar: None, bar_ticks: vec![] });
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
            guides.push(Guide { title: ns.name.clone().unwrap_or_else(|| aes.into()), levels, glyphs, bar: None, bar_ticks: vec![] });
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
        guides.push(Guide { title: bp.guide_sources.get("linetype").cloned().unwrap_or_else(|| "linetype".into()), levels: levels.clone(), glyphs, bar: None, bar_ticks: vec![] });
    }

    guides
}

pub fn layout(bp: &BuiltPlot) -> Scene {
    let spec = &bp.plot;
    let theme = Theme::new(spec.theme.kind());
    let width = spec.width;
    let height = spec.height;
    let hl = theme.half_line(); // 5.5
    let mut sc = Scene::new(width, height);
    sc.background = Color::white();

    let (x_breaks, x_labels) = axis_breaks(&bp.x_scale);
    let (y_breaks, y_labels) = axis_breaks(&bp.y_scale);
    let label_style = TextStyle {
        size: theme.small_text(),
        color: theme.axis_text,
        ..Default::default()
    };
    let tick_line = Line::solid(Color::rgb(51, 51, 51), theme_lw(0.5)); // #333
    let x_lab = spec.labels.x.clone().or_else(|| auto_label_of(bp, "x")).unwrap_or_default();
    let y_lab = spec.labels.y.clone().or_else(|| auto_label_of(bp, "y")).unwrap_or_default();

    // --- gutters: calibrated against ggplot2 4.0.3 probes (see probes.rs) --
    let tick = theme.tick_length_pt;
    let mut left = {
        let w = y_labels.iter().map(|l| measure(l, &label_style).width).fold(0.0, f64::max);
        w + probes::gutter::AXIS_LABEL_PAD
    };
    if !y_lab.is_empty() {
        left += probes::gutter::YLAB_COL;
    }
    left += probes::gutter::PLOT_MARGIN;
    // svglite: panel bottom = 448.5 → 31.5 gutter with x-axis title
    let mut bottom = {
        let _ = hl;
        if !x_lab.is_empty() {
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
                .map(|g| {
                    measure(&g.title, &ts).width / 2.0
                        - probes::legend::KEY_W / 2.0
                        + probes::legend::TITLE_GAP
                })
                .fold(0.0f64, f64::max);
            legend_width =
                probes::legend::KEY_W + probes::legend::LABEL_GAP + maxlabel + probes::legend::BLOCK_PAD;
            right += legend_width.max(title_gap) + probes::legend::RIGHT_EDGE;
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
        let gl = Line::solid(theme.panel_grid, theme_lw(0.5));
        for &b in &x_breaks {
            if !theme.grid_x { continue; }
            let x = vp.map_x(&bp.x_scale, b);
            if x > vp.x0 && x < vp.x1 {
                sc.layer(layer::GRID).push(Primitive::Segment { x1: x, y1: vp.y0, x2: x, y2: vp.y1, stroke: gl.clone() });
            }
        }
        for &b in &y_breaks {
            if !theme.grid_y { continue; }
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
        geom::draw_layer(&mut data, l, bp, &vp);
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
            push_label(
                &mut sc,
                lab,
                x,
                vp.y1 + tick + probes::axis::XLABEL_PAD + probes::axis::BASELINE_FRAC * label_style.size,
                label_style.clone(),
            );
        }
        for (&b, lab) in y_breaks.iter().zip(y_labels.iter()) {
            let y = vp.map_y(&bp.y_scale, b);
            if y < vp.y0 - 0.5 || y > vp.y1 + 0.5 {
                continue;
            }
            sc.layer(layer::AXES).push(Primitive::Segment { x1: vp.x0 - tick, y1: y, x2: vp.x0, y2: y, stroke: al.clone() });
            let st = TextStyle { halign: TextAlign::Right, ..label_style.clone() };
            sc.layer(layer::AXES).push(Primitive::Text {
                content: lab.clone(),
                x: vp.x0 - tick - probes::axis::YLABEL_INSET,
                y: y + probes::axis::YLABEL_PAD + probes::axis::BASELINE_FRAC * st.size,
                style: st,
            });
        }
        if !x_lab.is_empty() {
            // svglite: "disp" baseline = 472.20 (= height - 7.80), size 11, centred
            let ts = TextStyle { size: theme.base_size, ..label_style.clone() };
            sc.layer(layer::TITLES).push(Primitive::Text {
                content: x_lab.clone(),
                x: (vp.x0 + vp.x1) / 2.0,
                y: height - probes::axis::XTITLE_BASE,
                style: ts,
            });
        }
        if !y_lab.is_empty() {
            // svglite: translate(13.36, 226.99) rotate(-90) anchor=middle
            let ts = TextStyle { size: theme.base_size, angle: 90.0, ..label_style.clone() };
            sc.layer(layer::TITLES).push(Primitive::Text {
                content: y_lab.clone(),
                x: probes::axis::YTITLE_X,
                y: (vp.y0 + vp.y1) / 2.0,
                style: ts,
            });
        }
    }

    // --- titles / caption ----------------------------------------------------
    if let Some(t) = &spec.labels.title {
        let ts = TextStyle { size: theme.large_text(), halign: TextAlign::Left, ..Default::default() };
        // svglite title baseline (size 13.2)
        sc.layer(layer::TITLES).push(Primitive::Text {
            content: t.clone(),
            x: vp.x0,
            y: probes::title::BASELINE,
            style: ts,
        });
    }
    if let Some(t) = &spec.labels.subtitle {
        let ts = TextStyle { size: theme.small_text(), halign: TextAlign::Left, ..Default::default() };
        let y = probes::title::BASELINE + theme.large_text() + ts.size * probes::title::SUB_PAD;
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
    let key_w = probes::legend::KEY_W;
    let pitch = probes::legend::PITCH;
    let title_to_first = probes::legend::TITLE_TO_FIRST;
    let inter = probes::legend::INTER_GUIDE;
    let right_edge = width - 2.0 * probes::gutter::PLOT_MARGIN;
    let maxlabel = guides
        .iter()
        .flat_map(|g| g.levels.iter())
        .map(|l| measure(l, &ls).width)
        .fold(0.0f64, f64::max);
    let label_left = right_edge - maxlabel;
    let key_left = label_left - probes::legend::LABEL_GAP - key_w;
    let key_cx = key_left + key_w / 2.0;

    // vertical layout: first-title baseline centred on the panel middle so
    // the whole stack's baseline span is balanced (probe: +2.11 offset).
    let guide_h = |g: &Guide| -> f64 {
        if g.bar.is_some() {
            title_to_first + probes::legend::BAR_SPAN
        } else {
            title_to_first + (g.levels.len().max(1) as f64 - 1.0) * pitch
        }
    };
    let total: f64 = guides.iter().map(guide_h).sum::<f64>()
        + inter * (guides.len().saturating_sub(1)) as f64;
    let mut title_bl = (vp.y0 + vp.y1) / 2.0 - total / 2.0 + probes::legend::CENTRE_ADJ;

    for g in guides {
        let tm = measure(&g.title, &ts);
        sc.layer(layer::LEGEND).push(Primitive::Text {
            content: g.title.clone(),
            x: key_cx - tm.width / 2.0,
            y: title_bl,
            style: ts.clone(),
        });
        if let Some((lo, hi, stops)) = &g.bar {
            let _ = (lo, hi);
            let bar_top = title_bl + probes::legend::BAR_TOP;
            let bar_h = probes::legend::BAR_H;
            let bar_w = probes::legend::KEY_W;
            let slices = probes::legend::BAR_SLICES;
            for k in 0..slices {
                let t = (k as f64 + 0.5) / slices as f64; // 0 bottom .. 1 top
                let y = bar_top + bar_h * (1.0 - (k as f64 + 1.0) / slices as f64);
                let hh = bar_h / slices as f64 + 0.05;
                let col = crate::scale::multi_ramp(stops, t);
                sc.layer(layer::LEGEND).push(Primitive::Rect {
                    x: key_cx - bar_w / 2.0,
                    y,
                    w: bar_w,
                    h: hh,
                    fill: Some(Paint::new(col)),
                    stroke: None,
                });
            }
            let _ = (lo, hi);
            for (i, lvl) in g.levels.iter().enumerate() {
                let t = match g.bar_ticks.get(i).copied() {
                    Some(tv) => tv,
                    None => continue,
                };
                let ty = bar_top + bar_h * (1.0 - t) + 3.1;
                sc.layer(layer::LEGEND).push(Primitive::Text {
                    content: lvl.clone(),
                    x: label_left,
                    y: ty,
                    style: ls.clone(),
                });
            }
            title_bl += guide_h(g) + inter;
            continue;
        }
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
                    crate::geom::push_glyph(
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
        title_bl += guide_h(g) + inter;
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
