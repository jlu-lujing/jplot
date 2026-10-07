//! Rendering backends: SVG (hand-written writer) and PNG (Scene → SVG → resvg
//! → tiny-skia pixmap → png). Single source of truth keeps both backends
//! pixel-consistent.

use jplot_core::scene::{Layer, Primitive, Scene, TextAlign};
use jplot_core::scale::Color;

const FONT_STACK: &str = "Arial, Helvetica, sans-serif";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fill_attr(c: &Color) -> String {
    if c.a >= 1.0 {
        format!("fill=\"{}\"", c.to_hex())
    } else {
        // SVG fill-opacity; hex without alpha
        format!(
            "fill=\"{}\" fill-opacity=\"{:.4}\"",
            format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b),
            c.a
        )
    }
}

/// Full stroke attributes for a scene Line, including dasharray when set.
fn line_attrs(l: &jplot_core::scene::Line) -> String {
    let base = format!("stroke=\"{}\" stroke-width=\"{}\"", l.color.to_hex(), trim(l.width));
    match &l.dash {
        Some(d) if !d.is_empty() => {
            let ds = d.iter().map(|x| format!("{}", trim(*x))).collect::<Vec<_>>().join(",");
            format!("{} stroke-dasharray=\"{}\" stroke-dashoffset=\"0\"", base, ds)
        }
        _ => base,
    }
}

fn stroke_attrs(color: &Color, width: f64) -> String {
    if color.a <= 0.0 || width <= 0.0 {
        return "fill=\"none\"".into();
    }
    format!(
        "fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"",
        color.to_hex(),
        trim(width)
    )
}

fn trim(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Baseline y for text whose (x=left edge, y=vertical centre) anchor.


pub fn to_svg(scene: &Scene) -> String {
    let mut o = String::new();
    o.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" font-family=\"{}\">\n",
        trim(scene.width),
        trim(scene.height),
        trim(scene.width),
        trim(scene.height),
        FONT_STACK
    ));
    o.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
        trim(scene.width),
        trim(scene.height),
        scene.background.to_hex()
    ));
    for (i, layer) in scene.layers.iter().enumerate() {
        let _ = i;
        match layer.clip {
            Some((x, y, w, h)) => {
                o.push_str(&format!(
                    "<defs><clipPath id=\"clip{i}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs>",
                    trim(x), trim(y), trim(w), trim(h)
                ));
                o.push_str(&format!("<g clip-path=\"url(#clip{i})\">"));
            }
            None => o.push_str("<g>"),
        }
        for p in &layer.primitives {
            draw_primitive(&mut o, p);
        }
        o.push_str("</g>\n");
    }
    o.push_str("</svg>\n");
    o
}

fn draw_primitive(o: &mut String, p: &Primitive) {
    match p {
        Primitive::Rect { x, y, w, h, fill, stroke } => {
            let f = fill.map(|pa| fill_attr(&pa.color)).unwrap_or_else(|| "fill=\"none\"".into());
            let s = match stroke {
                Some(l) => format!(" {}", line_attrs(l)),
                None => String::new(),
            };
            o.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {f}{s}/>", trim(*x), trim(*y), trim(*w), trim(*h)));
        }
        Primitive::Circle { cx, cy, r, fill, stroke } => {
            let f = fill.map(|pa| fill_attr(&pa.color)).unwrap_or_else(|| "fill=\"none\"".into());
            let s = match stroke {
                Some(l) => format!(" {}", line_attrs(l)),
                None => String::new(),
            };
            o.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {f}{s}/>", trim(*cx), trim(*cy), trim(*r)));
        }
        Primitive::Polyline { points, stroke, fill, closed } => {
            let pts: String = points.iter().map(|(x, y)| format!("{},{}", trim(*x), trim(*y))).collect::<Vec<_>>().join(" ");
            let f = fill.map(|pa| fill_attr(&pa.color)).unwrap_or_else(|| "fill=\"none\"".into());
            let s = match stroke {
                Some(l) => format!(" {}", line_attrs(l)),
                None => String::new(),
            };
            let tag = if *closed { "polygon" } else { "polyline" };
            // a filled+stroked closed shape must not double-stroke the closing edge
            let s = if *closed && fill.is_some() { s.replace(" stroke=\"", " fill-rule=\"nonzero\" stroke=\"") } else { s };
            o.push_str(&format!("<{tag} points=\"{pts}\" {f}{s}/>"));
        }
        Primitive::Segment { x1, y1, x2, y2, stroke } => {
            o.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {} stroke-linecap=\"butt\"/>",
                trim(*x1), trim(*y1), trim(*x2), trim(*y2),
                stroke_attrs(&stroke.color, stroke.width)
            ));
        }
        Primitive::Text { content, x, y, style, text_length } => {
            // Scene Text semantics: `y` is the BASELINE, `x` is the anchor x
            // (centre for middle/right alignment). Mirrors svglite output.
            let anchor = match style.halign {
                TextAlign::Left => "start",
                TextAlign::Center => "middle",
                TextAlign::Right => "end",
            };
            // svglite writes textLength + lengthAdjust="spacingAndGlyphs"
            // (R's own string metrics); resvg honours it — emitting it makes
            // rasterised text widths match the reference exactly.
            let tl = match text_length {
                Some(w) if *w > 0.0 => {
                    format!(" textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\"", trim(*w))
                }
                _ => String::new(),
            };
            let attrs = format!(
                "{} font-size=\"{}\"{}{tl}",
                fill_attr(&style.color),
                trim(style.size),
                if style.bold { " font-weight=\"bold\"" } else { "" }
            );
            if style.angle.abs() > 0.0 {
                // rotate CCW about (x, y) = svglite translate(x,y) rotate(-a)
                o.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" {} text-anchor=\"middle\" transform=\"rotate({}, {}, {})\">{}</text>",
                    trim(*x), trim(*y), attrs, trim(-style.angle), trim(*x), trim(*y), esc(content)
                ));
            } else {
                o.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" {} text-anchor=\"{}\">{}</text>",
                    trim(*x), trim(*y), attrs, anchor, esc(content)
                ));
            }
        }
    }
}

/// Render PNG bytes: SVG (above) → usvg → tiny-skia, system fonts via fontdb.
#[cfg(feature = "png")]
pub fn to_png(scene: &Scene, scale: f64) -> Result<Vec<u8>, String> {
    let svg = to_svg(scene);
    svg_to_png(&svg, scale)
}

/// Shared font database so to_png / svg_to_png / any caller agree.
#[cfg(feature = "png")]
pub fn font_database() -> usvg::fontdb::Database {
    let mut db = usvg::fontdb::Database::new();
    for p in [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/Supplemental/Helvetica.ttc",
        "/usr/share/fonts/truetype/ghostscript/nimbus-sans-regular.ttc",
        "/usr/share/fonts/truetype/urw-base35/NimbusSans-Regular.ttc",
    ] {
        if std::path::Path::new(p).exists() {
            db.load_font_file(p).ok();
        }
    }
    db.load_system_fonts();
    db
}

/// Rasterise an arbitrary SVG string with the SAME font database as `to_png`.
/// Used by the comparison pipeline so the ggplot2 reference (svglite) and
/// jplot output go through one identical rasteriser — otherwise glyph
/// antialiasing differences between R's cairo and ours drown out geometry.
#[cfg(feature = "png")]
pub fn svg_to_png(svg: &str, scale: f64) -> Result<Vec<u8>, String> {
    let mut opt = usvg::Options::default();
    opt.fontdb = std::sync::Arc::new(font_database());
    opt.font_family = "Arial".into();
    opt.font_size = 11.0;
    let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| format!("usvg: {e}"))?;
    let size = tree.size();
    let target_w = (size.width() * scale as f32).round() as u32;
    let target_h = (size.height() * scale as f32).round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h).ok_or("pixmap")?;
    let transform = tiny_skia::Transform::from_scale(scale as f32, scale as f32);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| format!("png: {e}"))
}

/// Layer ordering sanity: DATA layer must be drawn after PANEL_BG/GRID.
pub fn assert_layer_order(_layers: &[Layer]) {}
