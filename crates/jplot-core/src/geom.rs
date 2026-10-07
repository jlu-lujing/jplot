//! Geom drawing: BuiltLayer → Scene primitives, one function per geom.
//!
//! Split from layout.rs so the layer vocabulary (draw_layer dispatch + per-geom
//! painters + shared colour/linestyle helpers) lives in one place.

use crate::build::{BuiltPlot, BuiltScale};
use crate::probes;
use crate::scale::Color;
use crate::scene::{Line, Paint, Primitive, TextAlign, TextStyle};
use crate::text::measure;
use crate::theme::geom_defaults::{self, geom_lw, point_r_px, point_stroke};

use crate::layout::Viewport;

pub fn draw_layer(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
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
        GeomSpec::Segment => draw_segment(ops, l, bp, vp),
        GeomSpec::Path => draw_lines(ops, l, bp, vp),
        GeomSpec::Rect => draw_rect(ops, l, bp, vp),
        GeomSpec::Tile => draw_tile(ops, l, bp, vp),
        GeomSpec::Linerange => draw_linerange(ops, l, bp, vp),
        GeomSpec::Pointrange => draw_pointrange(ops, l, bp, vp),
        GeomSpec::Crossbar => draw_crossbar(ops, l, bp, vp),
        GeomSpec::Errorbarh => draw_errorbarh(ops, l, bp, vp),
        GeomSpec::Abline => draw_abline(ops, l, bp, vp),
        GeomSpec::Jitter => draw_points(ops, l, bp, vp),
        GeomSpec::Label => draw_label(ops, l, bp, vp),
        GeomSpec::Density => draw_lines(ops, l, bp, vp),
        GeomSpec::Violin => draw_violin(ops, l, bp, vp),
    }
}

/// geom_violin (stat ydensity): mirrored polygon x ± width/2·violinwidth at
/// y, plus the median bar (colour #333, linewidth 0.5·fatten? ggplot2 draws
/// the median as a thick bar by default).
fn draw_violin(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let xs = g.get("x").cloned().unwrap_or_default();
    let ys = g.get("y").cloned().unwrap_or_default();
    let vw = g.get("violinwidth").cloned().unwrap_or_default();
    let wv = g.get("width").cloned().unwrap_or_default();
    let n = xs.len().min(ys.len()).min(vw.len());
    // group by x value (each violin is one x group; x constant per group)
    let mut groups: Vec<(f64, Vec<(f64, f64)>)> = Vec::new(); // (x, [(y, half-width data-units)])
    for i in 0..n {
        if !(xs[i].is_finite() && ys[i].is_finite() && vw[i].is_finite()) {
            continue;
        }
        let w = wv.get(i).copied().unwrap_or(0.9);
        let half = w / 2.0 * vw[i];
        match groups.iter_mut().find(|(gx, _)| (*gx - xs[i]).abs() < 1e-9) {
            Some((_, pts)) => pts.push((ys[i], half)),
            None => groups.push((xs[i], vec![(ys[i], half)])),
        }
    }
    for (gx, pts) in groups {
        if pts.len() < 2 {
            continue;
        }
        let fillc = l.args.colour_(&["fill"]).unwrap_or(Color::white());
        let stroke = line_style_of(l, 0, bp, geom_lw(l.args.f64_("linewidth").unwrap_or(0.5)));
        // right side ascending, left side descending (ggplot2 GeomPolygon order)
        let mut poly: Vec<(f64, f64)> = pts
            .iter()
            .map(|(y, hw)| (vp.map_x(&bp.x_scale, gx + hw), vp.map_y(&bp.y_scale, *y)))
            .collect();
        poly.extend(pts.iter().rev().map(|(y, hw)| (vp.map_x(&bp.x_scale, gx - hw), vp.map_y(&bp.y_scale, *y))));
        poly.push(poly[0]);
        ops.push(Primitive::Polyline { points: poly, stroke: Some(stroke), fill: Some(Paint::new(fillc)), closed: true });
        // (ggplot2 4.x geom_violin draws no median bar by default)
    }
}

/// geom_segment: x,y -> xend,yend
fn draw_segment(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let (xs, ys) = (g.get("x").cloned().unwrap_or_default(), g.get("y").cloned().unwrap_or_default());
    let (xe, ye) = (g.get("xend").cloned().unwrap_or_default(), g.get("yend").cloned().unwrap_or_default());
    let lw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let n = xs.len().min(ys.len()).min(xe.len()).min(ye.len());
    for i in 0..n {
        if !(xs[i].is_finite() && ys[i].is_finite() && xe[i].is_finite() && ye[i].is_finite()) {
            continue;
        }
        ops.push(Primitive::Segment {
            x1: vp.map_x(&bp.x_scale, xs[i]),
            y1: vp.map_y(&bp.y_scale, ys[i]),
            x2: vp.map_x(&bp.x_scale, xe[i]),
            y2: vp.map_y(&bp.y_scale, ye[i]),
            stroke: line_style_of(l, i, bp, lw),
        });
    }
}

/// dash + colour line for a row (linetype aes/args).
fn line_style_of(l: &crate::build::BuiltLayer, i: usize, bp: &BuiltPlot, lw: f64) -> Line {
    let mut ln = Line::solid(point_colour_of(l, i, bp), lw);
    ln.dash = linetype_of(l, i, lw);
    ln
}

/// geom_rect / geom_tile: boxes from xmin/xmax/ymin/ymax (rect) or centred
/// tiles with inferred spacing.
fn draw_rect(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let (xa, xb) = (g.get("xmin").cloned().unwrap_or_default(), g.get("xmax").cloned().unwrap_or_default());
    let (ya, yb) = (g.get("ymin").cloned().unwrap_or_default(), g.get("ymax").cloned().unwrap_or_default());
    let lw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let n = xa.len().min(xb.len()).min(ya.len()).min(yb.len());
    for i in 0..n {
        if !(xa[i].is_finite() && xb[i].is_finite() && ya[i].is_finite() && yb[i].is_finite()) {
            continue;
        }
        let (x0, x1) = (vp.map_x(&bp.x_scale, xa[i]), vp.map_x(&bp.x_scale, xb[i]));
        let (y0, y1) = (vp.map_y(&bp.y_scale, ya[i]), vp.map_y(&bp.y_scale, yb[i]));
        ops.push(Primitive::Rect {
            x: x0.min(x1),
            y: y0.min(y1),
            w: (x1 - x0).abs(),
            h: (y1 - y0).abs(),
            fill: Some(Paint::new(fill_colour_of(l, i, bp))),
            stroke: if has_stroke(l) { Some(line_style_of(l, i, bp, lw)) } else { None },
        });
    }
}

fn has_stroke(l: &crate::build::BuiltLayer) -> bool {
    l.aes.contains_key("colour") || l.args.s("colour").is_some() || l.args.s("color").is_some()
}

fn draw_tile(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let (_xs, _ys) = (g.get("x").cloned().unwrap_or_default(), g.get("y").cloned().unwrap_or_default());
    // infer per-axis spacing from unique coordinate gaps (ggplot2 resolution)
    let sp = |vals: &[f64]| -> f64 {
        let mut u: Vec<f64> = vals.iter().cloned().filter(|v| v.is_finite()).collect();
        u.sort_by(|a, b| a.partial_cmp(b).unwrap());
        u.dedup();
        if u.len() > 1 {
            u.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min)
        } else {
            1.0
        }
    };
    // ggplot2 resolution(): minimum spacing among ALL unique coords of the
    // whole layer (global, not local); tile default fill = #333 (col_mix .2)
    let _ = sp;
    let g = &l.frame;
    let (xa, xb) = (g.get("xmin").cloned().unwrap_or_default(), g.get("xmax").cloned().unwrap_or_default());
    let (ya, yb) = (g.get("ymin").cloned().unwrap_or_default(), g.get("ymax").cloned().unwrap_or_default());
    let fill = if l.aes.contains_key("fill") || l.args.s("fill").is_some() {
        None
    } else {
        Some(Color::rgb(51, 51, 51))
    };
    let n = xa.len().min(xb.len()).min(ya.len()).min(yb.len());
    for i in 0..n {
        if !(xa[i].is_finite() && xb[i].is_finite() && ya[i].is_finite() && yb[i].is_finite()) {
            continue;
        }
        let (x0, x1) = (vp.map_x(&bp.x_scale, xa[i]), vp.map_x(&bp.x_scale, xb[i]));
        let (y0, y1) = (vp.map_y(&bp.y_scale, ya[i]), vp.map_y(&bp.y_scale, yb[i]));
        ops.push(Primitive::Rect {
            x: x0.min(x1),
            y: y0.min(y1),
            w: (x1 - x0).abs(),
            h: (y1 - y0).abs(),
            fill: Some(Paint::new(fill.unwrap_or_else(|| fill_colour_of(l, i, bp)))),
            stroke: None,
        });
    }
}

/// geom_linerange / geom_pointrange / geom_crossbar (ymin..ymax at x).
fn draw_linerange(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let (xs, ya, yb) = (g.get("x").cloned().unwrap_or_default(), g.get("ymin").cloned().unwrap_or_default(), g.get("ymax").cloned().unwrap_or_default());
    let lw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let n = xs.len().min(ya.len()).min(yb.len());
    for i in 0..n {
        if !(xs[i].is_finite() && ya[i].is_finite() && yb[i].is_finite()) {
            continue;
        }
        let (px, p0, p1) = (vp.map_x(&bp.x_scale, xs[i]), vp.map_y(&bp.y_scale, ya[i]), vp.map_y(&bp.y_scale, yb[i]));
        ops.push(Primitive::Segment { x1: px, y1: p0, x2: px, y2: p1, stroke: line_style_of(l, i, bp, lw) });
    }
}

fn draw_pointrange(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    draw_linerange(ops, l, bp, vp);
    draw_points(ops, l, bp, vp);
}

/// geom_crossbar: rectangle ymin..ymax + median line (default mid) + end caps
/// of width w (ggplot2 width param, default 0.5 scaled).
fn draw_crossbar(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let (xs, ya, yb) = (g.get("x").cloned().unwrap_or_default(), g.get("ymin").cloned().unwrap_or_default(), g.get("ymax").cloned().unwrap_or_default());
    let lw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let w = l.args.f64_("width").unwrap_or(0.5);
    let n = xs.len().min(ya.len()).min(yb.len());
    for i in 0..n {
        if !(xs[i].is_finite() && ya[i].is_finite() && yb[i].is_finite()) {
            continue;
        }
        let (x0, x1) = (vp.map_x(&bp.x_scale, xs[i] - w / 2.0), vp.map_x(&bp.x_scale, xs[i] + w / 2.0));
        let (y0, y1) = (vp.map_y(&bp.y_scale, ya[i]), vp.map_y(&bp.y_scale, yb[i]));
        let ln = line_style_of(l, i, bp, lw);
        let (ymed, m0, m1) = (
            vp.map_y(&bp.y_scale, (ya[i] + yb[i]) / 2.0),
            vp.map_x(&bp.x_scale, xs[i] - w / 2.0),
            vp.map_x(&bp.x_scale, xs[i] + w / 2.0),
        );
        let fillc = l.args.colour_(&["fill"]).unwrap_or(Color::white());
        ops.push(Primitive::Rect { x: x0.min(x1), y: y0.min(y1), w: (x1 - x0).abs(), h: (y1 - y0).abs(), fill: Some(Paint::new(fillc)), stroke: Some(ln.clone()) });
        ops.push(Primitive::Segment { x1: m0, y1: ymed, x2: m1, y2: ymed, stroke: ln });
    }
}

/// geom_errorbarh: horizontal errorbar (x = value, xmin..xmax at y).
fn draw_errorbarh(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let g = &l.frame;
    let (ys, xa, xb) = (g.get("y").cloned().unwrap_or_default(), g.get("xmin").cloned().unwrap_or_default(), g.get("xmax").cloned().unwrap_or_default());
    let lw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let h = l.args.f64_("height").unwrap_or(0.9);
    let n = ys.len().min(xa.len()).min(xb.len());
    for i in 0..n {
        if !(ys[i].is_finite() && xa[i].is_finite() && xb[i].is_finite()) {
            continue;
        }
        let (py, p0, p1) = (vp.map_y(&bp.y_scale, ys[i]), vp.map_x(&bp.x_scale, xa[i]), vp.map_x(&bp.x_scale, xb[i]));
        let (c0, c1) = (vp.map_y(&bp.y_scale, ys[i] - h / 2.0), vp.map_y(&bp.y_scale, ys[i] + h / 2.0));
        let ln = line_style_of(l, i, bp, lw);
        ops.push(Primitive::Segment { x1: p0, y1: py, x2: p1, y2: py, stroke: ln.clone() });
        ops.push(Primitive::Segment { x1: p0, y1: c0, x2: p0, y2: c1, stroke: ln.clone() });
        ops.push(Primitive::Segment { x1: p1, y1: c0, x2: p1, y2: c1, stroke: ln });
    }
}

/// geom_abline: y = slope·x + intercept across the panel.
fn draw_abline(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    let slope = l.args.f64_("slope").unwrap_or(1.0);
    let inter = l.args.f64_("intercept").unwrap_or(0.0);
    let lw = l.args.f64_("linewidth").map(geom_lw).unwrap_or(geom_lw(0.5));
    let xs = l.frame.get("slope").cloned().unwrap_or_default();
    let is = l.frame.get("intercept").cloned().unwrap_or_default();
    let (lo, hi) = (bp.x_scale.range().0, bp.x_scale.range().1);
    let n = xs.len().max(1);
    for i in 0..n {
        let s = xs.get(i).copied().unwrap_or(slope);
        let b = is.get(i).copied().unwrap_or(inter);
        let ln = line_style_of(l, i.min(0), bp, lw);
        ops.push(Primitive::Segment {
            x1: vp.x0,
            y1: vp.map_y(&bp.y_scale, s * lo + b),
            x2: vp.x1,
            y2: vp.map_y(&bp.y_scale, s * hi + b),
            stroke: ln,
        });
    }
}

/// geom_label: text on a rounded background box (box first, then text).
fn draw_label(ops: &mut Vec<Primitive>, l: &crate::build::BuiltLayer, bp: &BuiltPlot, vp: &Viewport) {
    draw_text(ops, l, bp, vp);
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
        let baseline = py + vjust * probes::axis::VJUST_BOX * size_px;
        ops.push(Primitive::Text { content: labels[i].clone(), x, y: baseline, style: style.clone() , text_length: None });
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
                    // continuous colour ramp from the scale (multi-stop viridis
                    // under house theme; manual colours/limits override)
                    let t = if cs.data_hi == cs.data_lo {
                        0.5
                    } else {
                        ((vals[i] - cs.data_lo) / (cs.data_hi - cs.data_lo)).clamp(0.0, 1.0)
                    };
                    return cs.ramp_color(t).with_alpha(alpha);
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

/// data range of a built scale (for abline span)
impl BuiltScale {
    pub fn range(&self) -> (f64, f64) {
        match self {
            BuiltScale::Continuous(cs) => (cs.range.min, cs.range.max),
            BuiltScale::Discrete(ds) => (ds.range.min, ds.range.max),
        }
    }
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
pub fn push_glyph(ops: &mut Vec<Primitive>, shape: i32, cx: f64, cy: f64, r: f64, stk: f64, col: Color, fill: Color) {
    let fillable = (21..=25).contains(&shape);
    let solid = (15..=20).contains(&shape);
    let interior = if fillable { fill } else if solid { col } else { Color::transparent() };
    let border = if solid { Color::transparent() } else { col };
    let stroke = Line::solid(border, stk);
    let tri_up = |rr: f64| {
        vec![
            (cx, cy - probes::glyph::TRI_UP_Y * rr),
            (cx - probes::glyph::TRI_X * rr, cy + probes::glyph::TRI_BASE_Y * rr),
            (cx + probes::glyph::TRI_X * rr, cy + probes::glyph::TRI_BASE_Y * rr),
        ]
    };
    let tri_dn = |rr: f64| {
        vec![
            (cx, cy + probes::glyph::TRI_UP_Y * rr),
            (cx - probes::glyph::TRI_X * rr, cy - probes::glyph::TRI_BASE_Y * rr),
            (cx + probes::glyph::TRI_X * rr, cy - probes::glyph::TRI_BASE_Y * rr),
        ]
    };
    match shape {
        0 | 15 | 22 => ops.push(Primitive::Rect { x: cx - r, y: cy - r, w: 2.0 * r, h: 2.0 * r, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, stroke: if border.a == 0.0 { None } else { Some(stroke) } }),
        20 => ops.push(Primitive::Circle { cx, cy, r: r * probes::glyph::DOT_RATIO, fill: Some(Paint::new(col)), stroke: None }),
        s if matches!(s, 1 | 16 | 19 | 21) => ops.push(Primitive::Circle { cx, cy, r, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, stroke: if border.a == 0.0 { None } else { Some(stroke) } }),
        2 | 17 | 24 => ops.push(Primitive::Polyline { points: tri_up(r), stroke: if border.a == 0.0 { None } else { Some(stroke) }, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, closed: true }),
        25 => ops.push(Primitive::Polyline { points: tri_dn(r), stroke: Some(stroke), fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, closed: true }),
        5 | 18 | 23 => ops.push(Primitive::Polyline { points: vec![(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)], stroke: if border.a == 0.0 { None } else { Some(stroke) }, fill: if interior.a == 0.0 { None } else { Some(Paint::new(interior)) }, closed: true }),
        3 => {
            let a = probes::glyph::SQRT2 * r;
            ops.push(Primitive::Segment { x1: cx - a, y1: cy, x2: cx + a, y2: cy, stroke });
            ops.push(Primitive::Segment { x1: cx, y1: cy - a, x2: cx, y2: cy + a, stroke: Line::solid(border, stk) });
        }
        4 | 8 => {
            let a = probes::glyph::SQRT2 * r * probes::glyph::INV_SQRT2;
            ops.push(Primitive::Segment { x1: cx - a, y1: cy - a, x2: cx + a, y2: cy + a, stroke: Line::solid(border, stk) });
            ops.push(Primitive::Segment { x1: cx - a, y1: cy + a, x2: cx + a, y2: cy - a, stroke: Line::solid(border, stk) });
            if shape == 8 {
                let b = probes::glyph::SQRT2 * r;
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
                    let pts = vec![
                        (cx, cy - r * probes::glyph::OUTLIER_TRI_APEX),
                        (cx - r, cy + r * probes::glyph::OUTLIER_TRI_BASE_Y),
                        (cx + r, cy + r * probes::glyph::OUTLIER_TRI_BASE_Y),
                    ];
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
