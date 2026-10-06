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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub color: Color,
    pub width: f64, // px
}

impl Line {
    pub fn solid(color: Color, width: f64) -> Self {
        Line { color, width }
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
    },
    /// axis tick labels: `y` is the TEXT CENTRE for horizontal text
    Text {
        content: String,
        x: f64,
        y: f64,
        style: TextStyle,
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
