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
    /// jplot house style: white panel, Okabe-Ito colour-blind-safe discrete
    /// palette, viridis continuous ramp, horizontal-only light gridlines,
    /// black axis text — modern scientific-figure conventions.
    Jplot,
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
    /// draw vertical major gridlines (jplot style: horizontal only)
    pub grid_x: bool,
    /// draw horizontal major gridlines
    pub grid_y: bool,
    /// axis/tick text colour (jplot: black for print contrast)
    pub axis_text: Color,
    /// discrete colour palette (None = ggplot2 hue palette)
    pub discrete_palette: Option<Vec<Color>>,
    /// continuous colour ramp: hex stops, Lab-interpolated (default 2-stop)
    pub ramp: Vec<String>,
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
        // ggplot2 axis.text colour col_mix(ink,paper,0.302) ≈ #4D4D4D
        let gg_axis = Color::rgb(77, 77, 77);
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
                grid_x: true,
                grid_y: true,
                axis_text: gg_axis,
                discrete_palette: None,
                ramp: vec![],
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
                grid_x: true,
                grid_y: true,
                axis_text: gg_axis,
                discrete_palette: None,
                ramp: vec![],
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
                grid_x: true,
                grid_y: true,
                axis_text: gg_axis,
                discrete_palette: None,
                ramp: vec![],
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
                grid_x: false,
                grid_y: false,
                axis_text: gg_axis,
                discrete_palette: None,
                ramp: vec![],
                tick_length_pt: 2.75,
                ink,
                paper,
            },
            ThemeKind::Jplot => Theme {
                kind,
                base_size,
                panel_bg: paper,
                panel_grid: Color::rgb(224, 224, 224),
                panel_grid_major: true,
                panel_grid_minor: false,
                panel_border: false,
                axis_line: false,
                grid_x: false, // horizontal-only gridlines
                grid_y: true,
                axis_text: Color::black(), // print-ready contrast
                discrete_palette: Some(okabe_ito()),
                ramp: viridis_stops(),
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

/// Okabe-Ito colour-blind-safe palette (ggsci::pal_okabe_ito order).
pub fn okabe_ito() -> Vec<Color> {
    [
        (0xE6, 0x9F, 0x00),
        (0x56, 0xB4, 0xE9),
        (0x00, 0x9E, 0x73),
        (0xF0, 0xE4, 0x42),
        (0x00, 0x72, 0xB2),
        (0xD5, 0x5E, 0x00),
        (0xCC, 0x79, 0xA7),
        (0x00, 0x00, 0x00),
    ]
    .into_iter()
    .map(|(r, g, b)| Color::rgb(r, g, b))
    .collect()
}

/// viridis LUT stops (matplotlib), Lab-interpolated between neighbours.
pub fn viridis_stops() -> Vec<String> {
    [
        "#440154",
        "#482878",
        "#3E4A89",
        "#31688E",
        "#26828E",
        "#1F9E89",
        "#35B779",
        "#6DCD59",
        "#B4DE2C",
        "#FDE725",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}


/// Default geom constants from ggplot2 source (refs/ggplot2/R/geom-*.R).
pub mod geom_defaults {
    /// svglite (the reference device, 72dpi/base11) maps a *nominal* linewidth
    /// of 0.5mm to these stroke-width values depending on the element kind.
    /// Both the reference and jplot PNGs are rasterised by the SAME resvg, so
    /// emitting the identical stroke number reproduces the identical pixels.
    const GEOM_LW_PX_PER_MM: f64 = 1.07 / 0.5; // line/box/bar borders (2.14)
    const THEME_LW_PX_PER_MM: f64 = 0.53 / 0.5; // grid + axis ticks (1.06)
    const POINT_STROKE_PX_PER_MM: f64 = 0.71 / 0.5; // point/outlier stroke

    /// ggplot2 linewidth/size unit is MILLIMETRES; svglite device mapping.
    pub fn mm_to_px(v_mm: f64) -> f64 {
        v_mm * 72.27 / 25.4
    }
    /// stroke-width for a geom-level linewidth (geom_line/path/box/bar/rect).
    pub fn geom_lw(v_mm: f64) -> f64 {
        v_mm * GEOM_LW_PX_PER_MM
    }
    /// stroke-width for theme lines: panel.grid.major + axis.ticks.
    pub fn theme_lw(v_mm: f64) -> f64 {
        v_mm * THEME_LW_PX_PER_MM
    }
    /// stroke-width for point / outlier circle outlines (ggplot2 `stroke`).
    pub fn point_stroke(v_mm: f64) -> f64 {
        v_mm * POINT_STROKE_PX_PER_MM
    }
    /// .pt <- 72.27 / 25.4 ; .stroke <- 96 / 25.4
    pub const PT_PER_MM: f64 = 72.27 / 25.4;
    pub const STROKE_RATIO: f64 = (96.0 / 25.4) / PT_PER_MM; // 96/72.27
    /// ggplot2 point `size` is MILLIMETRES; svglite nominal glyph radius:
    /// r_px = 1.0667 * size_mm (probe: 1.5→1.6+0.35=1.95, 8→8.53+0.35=8.89).
    /// The DRAWN radius adds half the stroke width (see draw_points).
    pub fn point_r_px(size_mm: f64) -> f64 {
        size_mm * 16.0 / 15.0
    }
    /// ggplot2 default discrete shape sequence (solid_seq_pal): the first
    /// shapes in scales' `seq_pal` order for hollow/filled pchs.
    pub const SHAPE_SEQ: [f64; 18] =
        [16.0, 17.0, 15.0, 3.0, 7.0, 8.0, 4.0, 12.0, 13.0, 14.0, 10.0, 11.0, 5.0, 1.0, 2.0, 0.0, 6.0, 9.0];
    /// default discrete linetype sequence (R: solid,22,42,44,13,1343)
    pub const LINETYPE_SEQ: [&'static str; 6] =
        ["solid", "22", "42", "44", "13", "1343"];
    /// default point size (mm) = pointsize rel(1.5) of base 11 → 1.5mm.
    pub fn default_size_mm(base_size: f64) -> f64 {
        (base_size / 11.0) * 1.5
    }
    /// legacy helper (px, kept for compatibility): diameter.
    pub fn point_size_px(base_size: f64) -> f64 {
        point_r_px(default_size_mm(base_size)) * 2.0
    }
    /// border width = base_line_size = base/22 pt → px
    pub fn border_width_px(base_size: f64) -> f64 {
        (base_size / 22.0) / 0.75
    }
    pub fn ink_alpha_point_stroke(base_size: f64) -> f64 {
        border_width_px(base_size)
    }
}
