//! Colour science: sRGB `Color`, R-compatible HCL (LCh-uv), CIE Lab (D65)
//! ramps, and discrete palettes (hue/grey). Ported from R grDevices::hcl2rgb
//! and scales::pal_grad / hue_pal / grey_pal — probe-verified against
//! ggplot2 4.0.3 (#F8766D hue first colour, #132B43→#56B1F7 Lab midpoints).

// ---------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------

/// ggplot2 `col_mix(ink, paper, p)`: linear mix ink→paper.
pub fn col_mix(p: f64) -> Color {
    let v = (p * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::rgb(v, v, v)
}

/// sRGB colour with alpha 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 1.0 }
    }

    pub fn black() -> Self {
        Color::rgb(0, 0, 0)
    }
    pub fn white() -> Self {
        Color::rgb(255, 255, 255)
    }
    pub fn transparent() -> Self {
        Color { r: 0, g: 0, b: 0, a: 0.0 }
    }

    /// Parse `#rgb`, `#rrggbb`, named subset. Returns None on failure.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix('#') {
            let digits = hex.as_bytes();
            let v = |c: u8| -> Option<u32> {
                match c {
                    b'0'..=b'9' => Some((c - b'0') as u32),
                    b'a'..=b'f' => Some((c - b'a' + 10) as u32),
                    b'A'..=b'F' => Some((c - b'A' + 10) as u32),
                    _ => None,
                }
            };
            match digits.len() {
                3 => {
                    let r = v(digits[0])?;
                    let g = v(digits[1])?;
                    let b = v(digits[2])?;
                    Some(Color::rgb((r * 17) as u8, (g * 17) as u8, (b * 17) as u8))
                }
                6 => {
                    let p = |i: usize| -> Option<u32> { Some(v(digits[i])? * 16 + v(digits[i + 1])?) };
                    Some(Color::rgb(p(0)? as u8, p(2)? as u8, p(4)? as u8))
                }
                // R/ggplot2 rgb()/scales always emit #RRGGBBAA (e.g.
                // "#0000FFFF" = opaque blue) — must round-trip or overrides
                // from the R export silently drop.
                8 => {
                    let p = |i: usize| -> Option<u32> { Some(v(digits[i])? * 16 + v(digits[i + 1])?) };
                    let c = Color::rgb(p(0)? as u8, p(2)? as u8, p(4)? as u8);
                    Some(Color { a: p(6)? as f64 / 255.0, ..c })
                }
                _ => None,
            }
        } else {
            match s.to_ascii_lowercase().as_str() {
                "black" => Some(Color::black()),
                "white" => Some(Color::white()),
                "red" => Some(Color::rgb(255, 0, 0)),
                "blue" => Some(Color::rgb(0, 0, 255)),
                "green" => Some(Color::rgb(0, 255, 0)),
                "grey90" | "gray90" => {
                    let v = (0.9f64 * 255.0).round() as u8;
                    Some(Color::rgb(v, v, v))
                }
                "transparent" | "none" => Some(Color::transparent()),
                _ => None,
            }
        }
    }

    /// `#rrggbb` or `#rrggbbaa` (premultiplied-style alpha hex as ggplot2 does).
    pub fn to_hex(&self) -> String {
        if self.a >= 1.0 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            let a = (self.a.clamp(0.0, 1.0) * 255.0).round() as u8;
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, a)
        }
    }

    pub fn with_alpha(&self, a: f64) -> Color {
        let mut c = *self;
        c.a = a;
        c
    }
}

// ---------------------------------------------------------------------------
// HCL — R's grDevices::hcl() is **CIE LCh-uv (LUV)**, not LCh-AB. Ported
// verbatim from R-4.4.3 src/library/grDevices/src/colors.c::hcl2rgb (MIT):
// gamma 2.4 threshold 0.00304, WHITE_Y=100, u*/v* white D65, clip.
// ---------------------------------------------------------------------------

const WHITE_Y: f64 = 100.000;
const WHITE_U: f64 = 0.1978398;
const WHITE_V: f64 = 0.4683363;
const GAMMA: f64 = 2.4;

fn gtrans(u: f64) -> f64 {
    if u > 0.00304 {
        1.055 * u.powf(1.0 / GAMMA) - 0.055
    } else {
        12.92 * u
    }
}

/// R-compatible hcl(h, c, l) → sRGB (with R's integer rounding `(int)(255*x+.5)`
/// and clamp, i.e. `FixupColor`).
pub fn hcl_to_rgb(h: f64, c: f64, l: f64) -> Color {
    if l <= 0.0 {
        return Color::rgb(0, 0, 0);
    }
    let h_rad = h.to_radians();
    let big_u = c * h_rad.cos();
    let big_v = c * h_rad.sin();

    let y = WHITE_Y * if l > 7.999592 { ((l + 16.0) / 116.0).powi(3) } else { l / 903.3 };
    let u = big_u / (13.0 * l) + WHITE_U;
    let v = big_v / (13.0 * l) + WHITE_V;
    let x = 9.0 * y * u / (4.0 * v);
    let z = -x / 3.0 - 5.0 * y + 3.0 * y / v;

    let r = gtrans((3.240479 * x - 1.537150 * y - 0.498535 * z) / WHITE_Y);
    let g = gtrans((-0.969256 * x + 1.875992 * y + 0.041556 * z) / WHITE_Y);
    let b = gtrans((0.055648 * x - 0.204043 * y + 1.057311 * z) / WHITE_Y);

    let q = |v: f64| ((255.0 * v + 0.5) as i32).clamp(0, 255) as u8;
    Color::rgb(q(r), q(g), q(b))
}

// ---------------------------------------------------------------------------
// CIE Lab (D65) — scales::pal_grad space="Lab" for continuous colour scales
// ---------------------------------------------------------------------------

const D65: [f64; 3] = [0.95047, 1.0, 1.08883];

fn lin8(v: f64) -> f64 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}
fn delin8(v: f64) -> f64 {
    if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}
fn labf(t: f64) -> f64 {
    if t > 216.0 / 24389.0 { t.cbrt() } else { 841.0 / 108.0 * t + 4.0 / 29.0 }
}
fn labfi(t: f64) -> f64 {
    if t > 6.0 / 29.0 { t * t * t } else { (t - 4.0 / 29.0) * 108.0 / 841.0 }
}

/// sRGB → CIE Lab (D65).
pub fn rgb_to_lab(r: u8, g: u8, b: u8) -> [f64; 3] {
    let (rl, gl, bl) = (lin8(r as f64 / 255.0), lin8(g as f64 / 255.0), lin8(b as f64 / 255.0));
    let x = 0.4124564 * rl + 0.3575761 * gl + 0.1804375 * bl;
    let y = 0.2126729 * rl + 0.7151522 * gl + 0.0721750 * bl;
    let z = 0.0193339 * rl + 0.1191920 * gl + 0.9503041 * bl;
    let (fx, fy, fz) = (labf(x / D65[0]), labf(y / D65[1]), labf(z / D65[2]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIE Lab (D65) → sRGB, clamped with R-style rounding.
pub fn lab_to_rgb(lab: [f64; 3]) -> Color {
    let [l, a, bb] = lab;
    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - bb / 200.0;
    let x = D65[0] * labfi(fx);
    let y = D65[1] * if l > 7.999592 { labfi(fy) } else { l / 903.3 };
    let z = D65[2] * labfi(fz);
    let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let b = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
    let q = |v: f64| ((delin8(v.clamp(0.0, 1.0)) * 255.0 + 0.5) as i32).clamp(0, 255) as u8;
    Color::rgb(q(r), q(g), q(b))
}

/// Lab-space interpolation between two sRGB hex colours (scales::pal_grad).
pub fn lab_ramp(hex_lo: &str, hex_hi: &str, t: f64) -> Color {
    let lo = Color::parse(hex_lo).unwrap_or(Color::black());
    let hi = Color::parse(hex_hi).unwrap_or(Color::white());
    let a = rgb_to_lab(lo.r, lo.g, lo.b);
    let b = rgb_to_lab(hi.r, hi.g, hi.b);
    let t = t.clamp(0.0, 1.0);
    lab_to_rgb([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t])
}

/// Multi-stop ramp (viridis, gradient2, …): Lab-interpolate between the two
/// adjacent stops around t. 0/1 stops → the ggplot2 default gradient.
pub fn multi_ramp(stops: &[String], t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    if stops.is_empty() {
        return lab_ramp("#132B43", "#56B1F7", t);
    }
    if stops.len() == 1 {
        return Color::parse(&stops[0]).unwrap_or(Color::black());
    }
    let seg = t * (stops.len() - 1) as f64;
    let i = (seg as usize).min(stops.len() - 2);
    lab_ramp(&stops[i], &stops[i + 1], seg - i as f64)
}

// ---------------------------------------------------------------------------
// Palettes
// ---------------------------------------------------------------------------

/// ggplot2 default discrete colour palette: `scales::hue_pal(c = 100, l = 65)`
/// with h = c(15, 375) — hues 15, 15+360/n, ... (endpoint excluded).
pub fn hue_palette(n: usize) -> Vec<Color> {
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let h = 15.0 + 360.0 * (i as f64) / (n as f64);
            hcl_to_rgb(h % 360.0, 100.0, 65.0)
        })
        .collect()
}

/// scales::grey_pal — grey(n) equivalent: equally spaced greys, darkest first.
pub fn grey_palette(n: usize) -> Vec<Color> {
    grey_range(n, 0.2, 0.8)
}

/// grey palette math (scales::grey_pal): `grey(seq(start, end, length = n))`
/// where grey(v) = rgb(v,v,v) — v is the LIGHTNESS, not its complement.
/// ggplot2 scale_fill_grey defaults start = .7, end = .2 (dark = first level).
pub fn grey_range(n: usize, start: f64, end: f64) -> Vec<Color> {
    if n == 0 {
        return vec![];
    }
    (0..n)
        .map(|i| {
            let v = if n == 1 {
                (start + end) / 2.0
            } else {
                start + (end - start) * i as f64 / (n as f64 - 1.0)
            };
            let byte = (v * 255.0).round() as u8; // grey(v) = rgb(v,v,v)
            Color::rgb(byte, byte, byte)
        })
        .collect()
}

/// Named discrete palettes (RColorBrewer / viridisLite, generated tables).
/// `n>8` recycles the 8-colour row (R warns identically); brewer names may
/// carry a `brewer:` prefix. Returns uppercase hex strings.
pub fn named_palette(name: &str, n: usize) -> Option<Vec<String>> {
    let (kind, rest) = match name.split_once(':') {
        Some(("brewer", nm)) => ("brewer", nm),
        Some(("viridis", nm)) => ("viridis", nm),
        other => ("viridis", match other {
            Some((_, v)) => {
                let _ = v;
                name
            }
            None => name,
        }),
    };
    // plain viridis family names also accepted bare
    let bare = matches!(name, "viridis" | "magma" | "plasma" | "inferno" | "cividis");
    let (kind, rest) = if bare { ("viridis", name) } else { (kind, rest) };
    let table: &[(&str, &[(u32, [&str; 8])])] = match kind {
        "brewer" => &crate::palettes::BREWER,
        "viridis" => &crate::palettes::VIRIDIS,
        _ => return None,
    };
    let entry = table.iter().find(|(nm, _)| nm.eq_ignore_ascii_case(rest))?;
    let n = n.max(1);
    let row = entry
        .1
        .iter()
        .find(|(k, _)| *k == n as u32)
        .or_else(|| entry.1.iter().find(|(k, _)| *k == 8))
        .map(|(_, r)| r)?;
    Some((0..n).map(|i| row[i % row.len().min(8)].to_string()).collect())
}


// ---------------------------------------------------------------------------
// Breaks: Wilkinson extended, ported from labeling 0.4.3 (MIT)
