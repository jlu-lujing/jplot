//! Theme definitions. Layout constants follow ggplot2's classic box layout
//! (theme_grey default). Distances are in *points* (1 px = 0.75 pt), matching
//! grDevices's device units. Constants calibrated against ggplot2 4.x output
//! (see compare/calibrate.R).

use crate::scale::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeKind {
    Grey,
    Bw,
    Minimal,
    Classic,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub kind: ThemeKind,
    /// base font size in pt (ggplot2 base_size = 11)
    pub base_size: f64,
    pub panel_bg: Color,
    pub panel_grid: Color,
    /// major grid lines drawn? (grey: yes; bw: major yes; minimal: yes; classic: no)
    pub panel_grid_major: bool,
    /// minor grid lines (bw/minimal/classic: no)
    pub panel_grid_minor: bool,
    /// panel border line (bw: yes)
    pub panel_border: bool,
    /// axis lines (classic: yes)
    pub axis_line: bool,
    /// axis ticks length in pt (3.2pt? ggplot2 axis.ticks.length = unit(0.25,"cm")=7.1pt)
    /// axis.ticks.length = rel(0.5) × half_line (ggplot2 4.x theme_grey)
    pub tick_length_pt: f64,
    pub ink: Color,
    pub paper: Color,
}

impl Theme {
    pub fn new(kind: ThemeKind) -> Theme {
        let base_size = 11.0;
        let ink = Color::black();
        let paper = Color::white();
        match kind {
            ThemeKind::Grey => Theme {
                kind,
                base_size,
                panel_bg: Color::rgb(235, 235, 235), // ggplot2 4.x grey: 0.9216 → 235
                panel_grid: paper.with_alpha(1.0),
                panel_grid_major: true,
                panel_grid_minor: false,
                panel_border: false,
                axis_line: false,
                tick_length_pt: 2.75, // rel(0.5) * half_line
                ink,
                paper,
            },
            ThemeKind::Bw => Theme {
                kind,
                base_size,
                panel_bg: paper,
                panel_grid: Color::rgb(229, 229, 229),
                panel_grid_major: true,
                panel_grid_minor: false,
                panel_border: true,
                axis_line: false,
                tick_length_pt: 2.75,
                ink,
                paper,
            },
            ThemeKind::Minimal => Theme {
                kind,
                base_size,
                panel_bg: paper,
                panel_grid: Color::rgb(229, 229, 229),
                panel_grid_major: true,
                panel_grid_minor: false,
                panel_border: false,
                axis_line: false,
                tick_length_pt: 2.75,
                ink,
                paper,
            },
            ThemeKind::Classic => Theme {
                kind,
                base_size,
                panel_bg: paper,
                panel_grid: paper,
                panel_grid_major: false,
                panel_grid_minor: false,
                panel_border: false,
                axis_line: true,
                tick_length_pt: 2.75,
                ink,
                paper,
            },
        }
    }

    pub fn half_line(&self) -> f64 {
        self.base_size / 2.0
    }
    pub fn small_text(&self) -> f64 {
        self.base_size * 0.8 // axis.text, legend.text
    }
    pub fn large_text(&self) -> f64 {
        self.base_size * 1.2 // title
    }
    pub fn title_space_pt(&self) -> f64 {
        // title margin t = half_line/2 = 5.5pt
        self.half_line() / 2.0
    }
    pub fn panel_padding_pt(&self) -> f64 {
        self.half_line() * 0.5 // axis.ticks etc use half_line; panel margin 0
    }
}

/// Default geom constants from ggplot2 source (refs/ggplot2/R/geom-*.R).
pub mod geom_defaults {
    /// ggplot2 linewidth/size unit is MILLIMETRES; cairo @72dpi draws
    /// pt=1px and 1mm = 72.27/25.4 pt (ggplot2's internal .pt constant).
    pub fn mm_to_px(v_mm: f64) -> f64 {
        v_mm * 72.27 / 25.4
    }
    /// .pt <- 72.27 / 25.4 ; .stroke <- 96 / 25.4
    pub const PT_PER_MM: f64 = 72.27 / 25.4;
    pub const STROKE_RATIO: f64 = (96.0 / 25.4) / PT_PER_MM; // 96/72.27
    /// ggplot2 point `size` is in MILLIMETRES: 1.5mm at 72dpi device
    /// (R cairo uses 72.27pt/inch; grid points size = pointsize/72 inch)
    /// → diameter_px = size_mm/25.4 * 72 ≈ 4.25px for the default 1.5.
    pub fn point_size_px(base_size: f64) -> f64 {
        ((base_size / 11.0) * 1.5) / 25.4 * 72.0
    }
    /// border width = base_line_size = base/22 pt → px
    pub fn border_width_px(base_size: f64) -> f64 {
        (base_size / 22.0) / 0.75
    }
    pub fn ink_alpha_point_stroke(base_size: f64) -> f64 {
        border_width_px(base_size)
    }
}
