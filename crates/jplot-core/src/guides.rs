//! Legend guides: build (levels/glyphs from scales) and draw (key columns,
//! stacked blocks, gradient bars). Probed against ggplot2 4.0.3 svglite refs.

use crate::build::BuiltPlot;
use crate::probes;
use crate::scale::Color;
use crate::scene::{layer, Line, Paint, Primitive, Scene, TextAlign, TextStyle};
use crate::text::measure;
use crate::theme::geom_defaults::{self, geom_lw, point_r_px, point_stroke};
use crate::theme::Theme;

use crate::layout::Viewport;

/// One legend key glyph: the mark drawn next to a level label, mirroring
/// ggplot2's per-geom `draw_key_*`.
#[derive(Debug, Clone)]
pub enum KeyGlyph {
    Circle(Color),
    CircleR(f64, Color), // custom radius (size/alpha guides), colour
    Segment(Color),
    SegmentD(Option<Vec<f64>>, Color), // dashed segment (linetype guide)
    Rect(Color),
    Pch(i32, Color, Color), // shape, colour, fill
}

/// A legend guide: title + levels + per-level key glyphs, or a gradient bar.
pub struct Guide {
    pub title: String,
    pub levels: Vec<String>,
    pub glyphs: Vec<KeyGlyph>,
    /// continuous colour bar: (data_lo, data_hi, ramp low, ramp high)
    pub bar: Option<(f64, f64, Vec<String>)>,
    /// bar guide: normalised tick positions (t of each level label)
    pub bar_ticks: Vec<f64>,
}

/// Collect one guide per mapped discrete aesthetic (fill, colour, shape),
/// using ggplot2's guide order (fill, colour, shape).
pub fn build_guides(bp: &BuiltPlot, ls: &TextStyle) -> Vec<Guide> {
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


// ---------------------------------------------------------------------------

/// Stacked guide blocks (fill → colour → shape). Geometry probed from
/// ggplot2 4.0.3 svglite refs:
///   single guide: title bl 204.43, keys 222.14/237.98/253.82
///   (title→first key 17.7, pitch 15.84); two guides: last key→next title 35.8;
///   key label baseline = key centre + 3.15; label column right-aligned at
///   width - 10.96; title centred over the key column.
pub fn draw_legends(sc: &mut Scene, guides: &[Guide], theme: &Theme, width: f64, vp: &Viewport) {
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
