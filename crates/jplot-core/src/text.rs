//! Core text metrics: deterministic, backend-independent text extents so that
//! *layout* (gutters, alignment, legend sizing) is identical between backends.
//! Metrics are modelled on DejaVu Sans / Helvetica proportions at equal
//! point sizes (grDevices' default sans), calibrated against ggplot2 cairo.

use crate::scene::TextStyle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
}

impl TextMetrics {
    pub fn height(&self) -> f64 {
        self.ascent + self.descent
    }
    /// y offset from the *centre* of the text box to the baseline (draw y).
    pub fn baseline_from_center(&self) -> f64 {
        self.ascent - self.height() / 2.0
    }
}

fn char_advance(ch: char) -> f64 {
    // Adobe Helvetica (Nimbus Sans) metrics /1000 em.
    // Arial (Helvetica-compatible) advances /1000 em — this is BOTH the face
    // resvg renders (svg font-family: Arial, svglite's default family) and
    // the face ggplot2's cairo uses (its "sans" maps to Nimbus Sans =
    // Helvetica metrics). Layout metrics MUST match the rasteriser face or
    // gutters mis-size.
    match ch {
        ' ' => 0.278,
        '.' | ',' | ':' | ';' => 0.278,
        '\'' => 0.222,
        '!' | '|' => 0.278,
        '(' | ')' => 0.333,
        '/' => 0.278,
        '-' => 0.333,
        '+' | '=' | '<' | '>' => 0.584,
        'i' | 'l' | 'j' => 0.222,
        'f' | 't' | 'r' => 0.333,
        '{' | '}' | '[' | ']' => 0.333,
        'm' => 0.833,
        'w' => 0.5,
        'M' => 0.833,
        'W' => 0.944,
        'I' => 0.278,
        'J' => 0.5,
        'L' => 0.556,
        'T' => 0.611,
        ch if ch.is_ascii_digit() => 0.556,
        ch if ch.is_ascii_uppercase() => 0.667,
        ch if ch.is_ascii_lowercase() => 0.556,
        ch if (ch as u32) > 0x7f => 0.556,
        _ => 0.556,
    }
}

pub fn measure(text: &str, style: &TextStyle) -> TextMetrics {
    let advance: f64 = text.chars().map(char_advance).sum();
    let width = advance * style.size * if style.bold { 1.05 } else { 1.0 };
    TextMetrics {
        width,
        ascent: 0.76 * style.size,
        descent: 0.24 * style.size,
    }
}

/// X of the left edge given halign and anchor x (centre for rotated text).
pub fn left_edge(m: &TextMetrics, style: &TextStyle, x: f64) -> f64 {
    match style.halign {
        crate::scene::TextAlign::Left => x,
        crate::scene::TextAlign::Center => x - m.width / 2.0,
        crate::scene::TextAlign::Right => x - m.width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_monospace() {
        let s = TextStyle::default();
        assert_eq!(measure("123", &s).width, measure("456", &s).width);
    }

    #[test]
    fn longer_wider() {
        let s = TextStyle::default();
        assert!(measure("hello", &s).width > measure("hell", &s).width);
    }
}
