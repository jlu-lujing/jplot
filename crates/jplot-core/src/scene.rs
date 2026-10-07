//! Scene: renderer-agnostic layered vector scene in device px.
//! Layers are drawn back-to-front; order matches ggplot2 grob tree:
//! background < panel_bg < grid < data < axes < titles < legend.

use crate::scale::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    pub color: Color,
}

impl Paint {
    pub fn new(c: Color) -> Self {
        Paint { color: c }
    }
    pub fn none() -> Self {
        Paint { color: Color::transparent() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub color: Color,
    pub width: f64, // px
    /// grid-linetype dash pattern in px (e.g. "22" → [8,8]); None = solid
    pub dash: Option<Vec<f64>>,
}

impl Line {
    pub fn solid(color: Color, width: f64) -> Self {
        Line { color, width, dash: None }
    }
    pub fn dashed(color: Color, width: f64, dash: &[f64]) -> Self {
        Line { color, width, dash: Some(dash.to_vec()) }
    }
}

/// Resolve a ggplot2 linetype to a px dash pattern (None = solid).
/// R grid semantics (probe-verified at stroke 1.07/2.13/4.27): unit =
/// stroke × 4/3 px per pattern digit; bare lty numbers map to the classic
/// patterns 2=(4,4) 3=(1,3) 4=(1,3,4,3) 5=(8,4) 6=(2,2,6,2); digit strings
/// like ggplot2's discrete seq ("22","42","13","1343") are on/off pairs.
pub fn dash_for(linetype: &str, stroke_px: f64) -> Option<Vec<f64>> {
    let u = stroke_px * (4.0 / 3.0);
    let scale = |p: &[f64]| p.iter().map(|x| x * u).collect::<Vec<_>>();
    let digits = |s: &str| -> Option<Vec<f64>> {
        let d: Vec<f64> = s.chars().map(|c| (c as u32 - '0' as u32) as f64).collect();
        Some(scale(&d))
    };
    match linetype.trim() {
        "" | "blank" | "0" => None,
        "1" | "solid" => None,
        "2" | "dashed" => Some(scale(&[4., 4.])),
        "3" | "dotted" => Some(scale(&[1., 3.])),
        "4" | "dotdash" => Some(scale(&[1., 3., 4., 3.])),
        "5" | "longdash" => Some(scale(&[8., 4.])),
        "6" | "twodash" => Some(scale(&[2., 2., 6., 2.])),
        other => {
            if !other.is_empty() && other.chars().all(|c| c.is_ascii_digit()) {
                digits(other)
            } else {
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f64, // px, = pt @ 72dpi
    pub color: Color,
    pub halign: TextAlign,
    /// rotation CCW degrees about anchor point
    pub angle: f64,
    pub bold: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            size: 11.0,
            color: Color::black(),
            halign: TextAlign::Center,
            angle: 0.0,
            bold: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: Option<Paint>,
        stroke: Option<Line>,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: Option<Paint>,
        stroke: Option<Line>,
    },
    Polyline {
        points: Vec<(f64, f64)>,
        stroke: Option<Line>,
        fill: Option<Paint>,
        /// close the path (SVG <polygon>); false = open <polyline>
        closed: bool,
    },
    /// axis tick labels: `y` is the TEXT CENTRE for horizontal text
    Text {
        content: String,
        x: f64,
        y: f64,
        style: TextStyle,
        /// svglite emits `textLength` + `lengthAdjust="spacingAndGlyphs"`
        /// (R stringWidth metrics); resvg honours it — reproducing it makes
        /// rasterised text metrics identical to the reference.
        text_length: Option<f64>,
    },
    Segment {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke: Line,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub clip: Option<(f64, f64, f64, f64)>, // x,y,w,h
    pub primitives: Vec<Primitive>,
}

impl Layer {
    pub fn new() -> Self {
        Layer { clip: None, primitives: Vec::new() }
    }
    pub fn push(&mut self, p: Primitive) {
        self.primitives.push(p);
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub background: Color,
    pub layers: Vec<Layer>,
}

impl Scene {
    pub fn new(width: f64, height: f64) -> Self {
        Scene {
            width,
            height,
            background: Color::white(),
            layers: Vec::new(),
        }
    }
    pub fn layer(&mut self, idx: usize) -> &mut Layer {
        while self.layers.len() <= idx {
            self.layers.push(Layer::new());
        }
        &mut self.layers[idx]
    }
}

/// Standard layer indices (draw order).
pub mod layer {
    pub const PANEL_BG: usize = 0;
    pub const GRID: usize = 1;
    pub const DATA: usize = 2;
    pub const AXES: usize = 3;
    pub const TITLES: usize = 4;
    pub const LEGEND: usize = 5;
}

#[cfg(test)]
mod tests {
    use super::dash_for;
    // R grid semantics, unit = stroke*4/3; probe at stroke 1.07 → unit 1.4265
    #[test]
    fn ggplot_digit_seq_patterns() {
        let u = 1.07 * (4.0 / 3.0);
        let d = dash_for("22", 1.07).unwrap(); // on2 off2 -> 2.85,2.85
        assert!((d[0] - 2.0 * u).abs() < 1e-9 && (d[1] - 2.0 * u).abs() < 1e-9, "{d:?}");
        let d = dash_for("42", 1.07).unwrap(); // 5.69,2.85
        assert!((d[0] - 4.0 * u).abs() < 1e-9 && (d[1] - 2.0 * u).abs() < 1e-9, "{d:?}");
        let d = dash_for("13", 1.07).unwrap(); // 1.42,4.27
        assert!((d[0] - 1.0 * u).abs() < 1e-9 && (d[1] - 3.0 * u).abs() < 1e-9, "{d:?}");
        let d = dash_for("1343", 1.07).unwrap(); // 1.42,4.27,5.69,4.27
        assert_eq!(d.len(), 4);
        assert!(dash_for("solid", 1.07).is_none());
    }
}
