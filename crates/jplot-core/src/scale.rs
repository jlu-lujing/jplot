//! Scales: continuous & discrete, breaks (Wilkinson extended), labels, colour
//! palettes. Breaks algorithm ported from R `labeling::extended()` (MIT) with
//! constants verified against refs/ggplot2 and refs/scales.

use serde::{Deserialize, Serialize};

use crate::data::{Column, Range};

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
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            // grey(seq(...)) darkest .2 to lightest .9 for n>1; n=1 -> .5? R: grey(0.5)
            let v = if n == 1 {
                0.5
            } else {
                0.2 + 0.7 * (i as f64) / (n as f64 - 1.0)
            };
            let byte = ((1.0f64 - v) * 255.0f64).round() as u8; // grey(x) = black of intensity 1-x
            Color::rgb(byte, byte, byte)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Breaks: Wilkinson extended, ported from labeling 0.4.3 (MIT)
// ---------------------------------------------------------------------------

const Q_DEFAULT: [f64; 6] = [1.0, 5.0, 2.0, 2.5, 4.0, 3.0];
const W_DEFAULT: [f64; 4] = [0.25, 0.2, 0.5, 0.05];
const EPS: f64 = f64::EPSILON * 100.0;

fn simplicity(q: f64, qset: &[f64], j: f64, lmin: f64, lmax: f64, lstep: f64) -> f64 {
    let n = qset.len() as f64;
    let i = qset
        .iter()
        .position(|x| (x - q).abs() < EPS)
        .map(|p| p as f64 + 1.0)
        .unwrap_or(qset.len() as f64);
    let lm = lmin.rem_euclid(lstep);
    let v = if (lm < EPS || lstep - lm < EPS) && lmin <= 0.0 && lmax >= 0.0 {
        1.0
    } else {
        0.0
    };
    1.0 - (i - 1.0) / (n - 1.0) - j + v
}

fn simplicity_max(q: f64, qset: &[f64], j: f64) -> f64 {
    let n = qset.len() as f64;
    let i = qset
        .iter()
        .position(|x| (x - q).abs() < EPS)
        .map(|p| p as f64 + 1.0)
        .unwrap_or(qset.len() as f64);
    1.0 - (i - 1.0) / (n - 1.0) - j + 1.0
}

fn coverage(dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let range = dmax - dmin;
    1.0 - 0.5 * ((dmax - lmax).powi(2) + (dmin - lmin).powi(2)) / ((0.1 * range).powi(2))
}

fn coverage_max(dmin: f64, dmax: f64, span: f64) -> f64 {
    let range = dmax - dmin;
    if span > range {
        let half = (span - range) / 2.0;
        1.0 - 0.5 * (half * half + half * half) / ((0.1 * range).powi(2))
    } else {
        1.0
    }
}

fn density(k: f64, m: f64, dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let r = (k - 1.0) / (lmax - lmin);
    let rt = (m - 1.0) / (lmax.max(dmax) - dmin.min(lmin));
    2.0 - (r / rt).max(rt / r)
}

fn density_max(k: f64, m: f64) -> f64 {
    if k >= m {
        2.0 - (k - 1.0) / (m - 1.0)
    } else {
        1.0
    }
}

/// Wilkinson's extended breaks algorithm — equivalent to
/// `labeling::extended(dmin, dmax, m)` which scales/ggplot2 use by default.
pub fn extended_breaks(dmin: f64, dmax: f64, m: usize) -> Vec<f64> {
    let m = m.max(3) as f64;
    let qset = &Q_DEFAULT[..];
    let w = &W_DEFAULT[..];

    let (mut dmin, mut dmax) = (dmin.min(dmax), dmin.max(dmax));

    if dmax - dmin < EPS {
        return linspace(dmin, dmax, m as usize);
    }
    if dmax - dmax.hypot(0.0) > f64::MAX.sqrt() || dmax - dmin > f64::MAX.sqrt() {
        return linspace(dmin, dmax, m as usize);
    }

    let mut best_score = -2.0_f64;
    let mut best: Option<(f64, f64, f64)> = None; // (lmin, lmax, lstep)

    let mut j: f64 = 1.0;
    'outer: while j < f64::INFINITY {
        for &q in qset {
            let sm = simplicity_max(q, qset, j);
            if w[0] * sm + w[1] + w[2] + w[3] < best_score {
                j = f64::INFINITY;
                break 'outer;
            }
            let mut k: f64 = 2.0;
            while k < f64::INFINITY {
                let dm = density_max(k, m);
                if w[0] * sm + w[1] + w[2] + w[3] * dm < best_score {
                    break;
                }
                let delta = (dmax - dmin) / (k + 1.0) / j / q;
                let mut z = f64::ceil(delta.log10());
                while z < f64::INFINITY {
                    let step = j * q * 10f64.powf(z);
                    let cm = coverage_max(dmin, dmax, step * (k - 1.0));
                    if w[0] * sm + w[1] * cm + w[2] * dm + w[3] < best_score {
                        break;
                    }
                    let min_start = f64::floor(dmax / step) * j - (k - 1.0) * j;
                    let max_start = f64::ceil(dmin / step) * j;
                    if min_start > max_start {
                        z += 1.0;
                        continue;
                    }
                    let mut start = min_start;
                    while start <= max_start {
                        let lmin = start * (step / j);
                        let lmax = lmin + step * (k - 1.0);
                        let lstep = step;
                        let s = simplicity(q, qset, j, lmin, lmax, lstep);
                        let c = coverage(dmin, dmax, lmin, lmax);
                        let g = density(k, m, dmin, dmax, lmin, lmax);
                        let l = 1.0; // .legibility is a constant 1 in labeling R port
                        let score = w[0] * s + w[1] * c + w[2] * g + w[3] * l;
                        if score > best_score {
                            best_score = score;
                            best = Some((lmin, lmax, lstep));
                        }
                        start += 1.0;
                    }
                    z += 1.0;
                }
                k += 1.0;
            }
        }
        j += 1.0;
    }

    match best {
        Some((lmin, lmax, lstep)) => {
            let n = ((lmax - lmin) / lstep).round() as usize + 1;
            (0..n).map(|i| lmin + i as f64 * lstep).collect()
        }
        None => linspace(dmin, dmax, m as usize),
    }
}

fn linspace(a: f64, b: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![a];
    }
    (0..n)
        .map(|i| a + (b - a) * (i as f64) / (n as f64 - 1.0))
        .collect()
}

/// Number of ticks: ggplot2 requests 5 breaks via `n.breaks` theme element.
pub fn default_break_count() -> usize {
    5
}

// ---------------------------------------------------------------------------
// Break labels (scales::label_number style)
// ---------------------------------------------------------------------------

/// Format break labels like ggplot2's scales::label_number(): the whole break
/// set shares a common number of decimal places (e.g. {5, 7.5, 10} ->
/// "5.0","7.5","10.0"), space thousands separator ("1 000"), and scientific
/// notation only for |x| >= 1e6 or non-zero |x| < 1e-4.
pub fn format_breaks(xs: &[f64]) -> Vec<String> {
    if xs.iter().any(|x| !x.is_finite()) {
        return xs.iter().map(|x| x.to_string()).collect();
    }
    // big-mark: ggplot2/scales default uses a thin space ("1 000")
    if xs.iter().any(|x| x.abs() >= 1e6 || (x != &0.0 && x.abs() < 1e-4)) {
        return xs.iter().map(|x| format_break(*x)).collect();
    }
    // common decimals: the max needed across the set (label_number rule)
    let mut dec = 0usize;
    for prec in 0..=4usize {
        if xs.iter().all(|x| format!("{x:.prec$}").parse::<f64>() == Ok(*x)) {
            dec = prec;
            break;
        }
    }
    let sep = |s: &str| {
        // insert a space every 3 digits left of the decimal point
        let (int, frac) = match s.split_once('.') {
            Some(p) => (p.0.to_string(), Some(p.1)),
            None => (s.to_string(), None),
        };
        let neg = int.starts_with('-');
        let digits = int.trim_start_matches('-');
        let mut out = String::new();
        let ds: Vec<char> = digits.chars().collect();
        for (i, c) in ds.iter().enumerate() {
            if i > 0 && (ds.len() - i) % 3 == 0 {
                out.push(' ');
            }
            out.push(*c);
        }
        let body = match frac {
            Some(f) => format!("{out}.{f}"),
            None => out,
        };
        if neg {
            format!("-{body}")
        } else {
            body
        }
    };
    xs.iter().map(|x| sep(&format!("{x:.dec$}"))).collect()
}

/// Format a single break value (scientific/edge cases kept for callers that
/// need per-value formatting).
pub fn format_break(x: f64) -> String {
    if !x.is_finite() {
        return x.to_string();
    }
    if x == 0.0 {
        return "0".into();
    }
    let ax = x.abs();
    if ax >= 1e6 || ax < 1e-4 {
        // scientific: mantissa trimmed of trailing zeros
        let e = x.log10().floor();
        let m = x / 10f64.powf(e);
        let ms = trim_num(m);
        let es = e as i64;
        if ms == "1" {
            return format!("x10^{es}");
        }
        return format!("{ms}x10^{es}");
    }
    trim_num(x)
}

fn trim_num(x: f64) -> String {
    // find the shortest decimal repr that round-trips reasonably (<=6 sig figs)
    for prec in 0..=6 {
        let s = format!("{x:.prec$}");
        if s.parse::<f64>() == Ok(x) {
            return trim_zeros(&s);
        }
    }
    trim_zeros(&format!("{x:.6}"))
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        let t = s.trim_end_matches('0').trim_end_matches('.');
        t.to_string()
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Scale specs (serialisable)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScaleSpec {
    /// Continuous identity scale with optional manual limits/breaks/labels/name.
    Continuous {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limits: Option<[f64; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breaks: Option<Vec<f64>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        labels: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expand: Option<[f64; 2]>,
    },
    /// Manual discrete scale: named levels -> values (colours etc.).
    DiscreteManual {
        values: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Manual continuous colour: fixed single value.
    Manual {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Continuous colour gradient between two hex colours. ggplot2 >= 3.5
    /// default continuous colour uses viridis; jplot v1 uses a simple linear
    /// gradient (low -> high) via `Gradient`. `default_gradient` provides the
    /// ggplot2-equivalent default (low "#132B43", high "#FCFDF4").
    Gradient {
        low: String,
        high: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

impl Default for ScaleSpec {
    fn default() -> Self {
        ScaleSpec::Continuous {
            limits: None,
            breaks: None,
            labels: None,
            name: None,
            expand: None,
        }
    }
}

/// Trained continuous scale: domain (after expansion) -> [0,1].
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuousScale {
    /// user limits (unexpanded) — breaks are computed on this
    pub limits: Range,
    /// panel range after expansion — data maps through this
    pub range: Range,
    pub breaks: Vec<f64>,
    pub labels: Vec<String>,
    pub name: Option<String>,
}

impl ContinuousScale {
    /// ggplot2 defaults: expand = c(0.05, 0); breaks = extended(limits, n=5)
    /// when no manual breaks; labels = formatted breaks.
    pub fn train(data_range: Range, spec: &ScaleSpec) -> ContinuousScale {
        let (limits, expand, breaks, labels, name) = match spec {
            ScaleSpec::Continuous {
                limits,
                breaks,
                labels,
                name,
                expand,
            } => (
                limits.map(|[a, b]| Range { min: a, max: b }),
                expand.unwrap_or([0.05, 0.0]),
                breaks.clone(),
                labels.clone(),
                name.clone(),
            ),
            _ => (None, [0.05, 0.0], None, None, None),
        };
        let mut limits = limits.unwrap_or(data_range);
        if limits.min == limits.max {
            // ggplot2 dispenses a width for zero-range (like zero_width=1)
            let half = if limits.min == 0.0 { 0.5 } else { limits.min.abs() * 0.1 };
            limits = Range {
                min: limits.min - half,
                max: limits.max + half,
            };
        }
        // ggplot2 4.x expand_range: symmetric mul/add on both ends
        // (probe: 0..14 → -0.7..14.7).
        let mut range = limits;
        let w = range.max - range.min;
        range.min -= w * expand[0] + expand[1];
        range.max += w * expand[0] + expand[1];
        // breaks are computed by extended() on the EXPANDED range, then
        // clipped to range (probes: dodge y limits 0..12 → breaks on
        // -0.6..12.6 → 0,2.5,…,12.5).
        let breaks = breaks.unwrap_or_else(|| extended_breaks(range.min, range.max, 5));
        let labels = labels.unwrap_or_else(|| format_breaks(&breaks));
        ContinuousScale {
            limits,
            range,
            breaks,
            labels,
            name,
        }
    }

    pub fn map(&self, v: f64) -> f64 {
        (v - self.range.min) / (self.range.max - self.range.min)
    }

    /// breaks clipped into the (expanded) range, sorted
    pub fn breaks_in_range(&self) -> Vec<(f64, String)> {
        self.breaks
            .iter()
            .zip(self.labels.iter())
            .filter(|(b, _)| **b >= self.range.min && **b <= self.range.max)
            .map(|(b, l)| (*b, l.clone()))
            .collect()
    }
}

/// Trained discrete scale: levels ordered; value i maps to i+1 (1-based),
/// expanded panel range = c(1 - mult/2?, ...) — ggplot2: limits c(1,n),
/// expand = c(0.6, 0) → range = c(1 - .6, n + .6).
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteScale {
    pub levels: Vec<String>,
    pub range: Range,
    pub name: Option<String>,
}

impl DiscreteScale {
    pub fn train(levels: Vec<String>, spec: &ScaleSpec) -> DiscreteScale {
        let name = match spec {
            ScaleSpec::DiscreteManual { name, values } => {
                if !values.is_empty() {
                    // manual levels win when provided
                    DiscreteScale {
                        levels: values.clone(),
                        range: range_for(values.len()),
                        name: name.clone(),
                    }
                    .into_named(name.clone())
                } else {
                    DiscreteScale {
                        levels: levels.clone(),
                        range: range_for(levels.len()),
                        name: name.clone(),
                    }
                }
            }
            _ => DiscreteScale {
                levels: levels.clone(),
                range: range_for(levels.len()),
                name: None,
            },
        };
        name
    }

    pub fn map(&self, level: &str) -> f64 {
        self.levels
            .iter()
            .position(|l| l == level)
            .map_or(f64::NAN, |i| (i + 1) as f64)
    }
}

fn range_for(n: usize) -> Range {
    // ggplot2 discrete: limits c(1,n), expand = c(.6, 0) applied via
    // expand_range(mul = .6): width = n-1 → c(1-0.6(n-1), n+0.6(n-1)).
    // n == 1: zero-width → range spans the whole panel (c(1,1) + pad).
    if n == 0 {
        Range { min: 0.0, max: 1.0 }
    } else if n == 1 {
        Range { min: 0.4, max: 1.6 }
    } else {
        // 4.x probe (panel_params$x.range on 3 levels): c(1-0.6, n+0.6)
        Range { min: 0.4, max: n as f64 + 0.6 }
    }
}

impl DiscreteScale {
    fn into_named(mut self, name: Option<String>) -> Self {
        self.name = name.or(self.name);
        self
    }
}

/// Colour mapping for a discrete aesthetic: hue palette by default,
/// or manual palette / values from spec.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteColourScale {
    pub levels: Vec<String>,
    pub colours: Vec<Color>,
    pub name: Option<String>,
}

impl DiscreteColourScale {
    pub fn train(levels: Vec<String>, spec: &ScaleSpec) -> Self {
        let colours = match spec {
            ScaleSpec::DiscreteManual { values, .. } => {
                values.iter().filter_map(|s| Color::parse(s)).collect()
            }
            _ => hue_palette(levels.len()),
        };
        let colours = if colours.len() == levels.len() {
            colours
        } else {
            hue_palette(levels.len())
        };
        let name = match spec {
            ScaleSpec::DiscreteManual { name, .. } => name.clone(),
            _ => None,
        };
        DiscreteColourScale {
            levels,
            colours,
            name,
        }
    }
    pub fn map(&self, level: &str) -> Color {
        self.levels
            .iter()
            .position(|l| l == level)
            .map(|i| self.colours[i % self.colours.len()])
            .unwrap_or_else(Color::black)
    }
}

/// Aesthetic key of a column (continuous vs discrete) resolved from data.
pub fn column_is_discrete(col: &Column) -> bool {
    matches!(col, Column::Categorical { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_breaks_round_numbers() {
        let b = extended_breaks(0.0, 10.0, 5);
        assert_eq!(b, vec![0.0, 2.5, 5.0, 7.5, 10.0]); // labeling::extended(0,10,5)
        // verified against R: labeling::extended(4.1, 9.8, 5)
        let b = extended_breaks(4.1, 9.8, 5);
        assert_eq!(b, vec![4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
    }

    #[test]
    fn extended_breaks_0_to_100() {
        assert_eq!(
            extended_breaks(0.0, 100.0, 5),
            vec![0.0, 25.0, 50.0, 75.0, 100.0]
        );
    }

    #[test]
    fn format_breaks() {
        assert_eq!(format_break(0.0), "0");
        assert_eq!(format_break(2.5), "2.5");
        assert_eq!(format_break(50.0), "50");
        assert_eq!(format_break(0.1), "0.1");
        assert_eq!(format_break(1e6), "x10^6");
        assert_eq!(format_break(2.5e-5), "2.5x10^-5");
    }

    #[test]
    fn hue_palette_known_first() {
        // scales::hue_pal()(2)[1] == "#F8766D" = rgb(248,118,109)
        let p = hue_palette(2);
        assert_eq!((p[0].r, p[0].g, p[0].b), (248, 118, 109));
        // hue_pal()(2)[2] == "#00BFC4"
        assert_eq!((p[1].r, p[1].g, p[1].b), (0, 191, 196));
    }

    #[test]
    fn parse_hex() {
        assert_eq!(Color::parse("#FF8000").unwrap(), Color::rgb(255, 128, 0));
        assert_eq!(Color::parse("#f80").unwrap(), Color::rgb(255, 136, 0));
        assert!(Color::parse("notacolor").is_none());
    }

    #[test]
    fn continuous_train_expands_and_breaks() {
        let r = Range { min: 0.0, max: 10.0 };
        let s = ContinuousScale::train(r, &ScaleSpec::default());
        assert_eq!(s.breaks, vec![0.0, 2.5, 5.0, 7.5, 10.0]);
        assert!((s.range.min + 0.5).abs() < 1e-9);
        assert!((s.range.max - 10.5).abs() < 1e-9);
        // scales::label_number keeps common decimals across the break set
        // (R verified: label_number(c(0,2.5,5,7.5,10)) -> "0.0".."10.0")
        assert_eq!(s.labels, vec!["0.0", "2.5", "5.0", "7.5", "10.0"]);
    }

    #[test]
    fn discrete_range_two_levels_matches_ggplot2() {
        // ggplot2 verified: 2 levels → 0.4..2.6 ; 3 levels → 0.4..3.6 (y.range probe)
        let s = DiscreteScale::train(vec!["a".into(), "b".into()], &ScaleSpec::default());
        assert!((s.range.min - 0.4).abs() < 1e-9);
        assert!((s.range.max - 2.6).abs() < 1e-9);
        let s3 = DiscreteScale::train(vec!["a".into(), "b".into(), "c".into()], &ScaleSpec::default());
        assert!((s3.range.min - 0.4).abs() < 1e-9);
        assert!((s3.range.max - 3.6).abs() < 1e-9);
    }
}
