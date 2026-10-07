//! Scales: continuous & discrete scale assembly (Transform/Expand/Oob,
//! ContinuousScale/DiscreteScale/NumScale/DiscreteColourScale).
//! Tick math lives in breaks.rs, colour science in color.rs.

use serde::{Deserialize, Serialize};

use crate::data::{Column, Range};

pub use crate::breaks::{default_break_count, extended_breaks, format_break, format_breaks};

// Re-export the colour-science layer so downstream `use crate::scale::Color`
// keeps working (colours live in color.rs).
pub use crate::color::{
    col_mix, grey_palette, hcl_to_rgb, hue_palette, lab_ramp, lab_to_rgb, multi_ramp,
    rgb_to_lab, Color,
};

// ---------------------------------------------------------------------------
// Transform / expand / oob (scales + ggplot2 port, docs/TRANSFORM_PLAN.md)
// ---------------------------------------------------------------------------

/// scales::transform_* port. Enum (not trait object) so it serialises + Copy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransformSpec {
    Identity,
    Log10,
    Log2,
    Log { base: f64 },
    Sqrt,
    Reverse,
}

impl TransformSpec {
    /// transform into the internal space; values outside the domain → NaN
    /// (dropped from training / censored in map, ≈ ggplot2 warning + oob→NA).
    pub fn transform(&self, x: f64) -> f64 {
        match self {
            TransformSpec::Identity => x,
            TransformSpec::Reverse => -x,
            TransformSpec::Sqrt => {
                if x < 0.0 {
                    f64::NAN
                } else {
                    x.sqrt()
                }
            }
            TransformSpec::Log10 => {
                if x <= 0.0 {
                    f64::NAN
                } else {
                    x.log10()
                }
            }
            TransformSpec::Log2 => {
                if x <= 0.0 {
                    f64::NAN
                } else {
                    x.log2()
                }
            }
            TransformSpec::Log { base } => {
                if x <= 0.0 {
                    f64::NAN
                } else {
                    x.ln() / base.ln()
                }
            }
        }
    }

    pub fn inverse(&self, t: f64) -> f64 {
        match self {
            TransformSpec::Identity => t,
            TransformSpec::Reverse => -t,
            TransformSpec::Sqrt => {
                if t < 0.0 {
                    f64::NAN
                } else {
                    t * t
                }
            }
            TransformSpec::Log10 => 10f64.powf(t),
            TransformSpec::Log2 => 2f64.powf(t),
            TransformSpec::Log { base } => base.powf(t),
        }
    }

    pub fn base(&self) -> Option<f64> {
        match self {
            TransformSpec::Log10 => Some(10.0),
            TransformSpec::Log2 => Some(2.0),
            TransformSpec::Log { base } => Some(*base),
            _ => None,
        }
    }

    /// Major breaks, **data-space in, data-space out** (scale-.R:1193–1197:
    /// get_breaks inverts the limits, runs the breaks fn on data, transforms
    /// back). Only log has a special breaks fn; identity/reverse/sqrt use
    /// extended_breaks on the expanded data range.
    pub fn breaks(&self, lo: f64, hi: f64, n: usize) -> Vec<f64> {
        match self.base() {
            None => extended_breaks(lo, hi, n),
            Some(base) => log_breaks(lo, hi, n, base),
        }
    }
}

/// expansion(mult=c(l,r), add=c(l,r)) → four per-side coefficients.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExpandSpec {
    pub mult_l: f64,
    pub add_l: f64,
    pub mult_r: f64,
    pub add_r: f64,
}

impl ExpandSpec {
    pub const DEFAULT_CONTINUOUS: Self = Self { mult_l: 0.05, mult_r: 0.05, add_l: 0.0, add_r: 0.0 };

    /// scales::expand_range4 (bounds.R:352–360). `lo`/`hi` are the (already
    /// transformed, ascending) unexpanded limits; returns the expanded pair.
    pub fn expand_range4(&self, lo: f64, hi: f64) -> (f64, f64) {
        let w = if lo == hi { 1.0 } else { hi - lo };
        (lo - (w * self.mult_l + self.add_l), hi + (w * self.mult_r + self.add_r))
    }
}

/// scales::oob_* port (bounds.R:275–334).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Oob {
    Censor,
    Squish,
    Keep,
}

impl Oob {
    pub fn apply(&self, v: f64, lo: f64, hi: f64) -> f64 {
        match self {
            Oob::Keep => v,
            Oob::Squish => v.clamp(lo, hi),
            Oob::Censor => {
                if v < lo || v > hi {
                    f64::NAN
                } else {
                    v
                }
            }
        }
    }
}

/// scales::log_breaks(n, base) + log_sub_breaks port, following the
/// decompiled 1.4.0 reference exactly (breaks-log.R:46–207): integer powers
/// first, then greedy multipliers with a ±1 padded window, then the
/// extended_breaks fallback. Input/output are data-space values.
pub fn log_breaks(lo: f64, hi: f64, n: usize, base: f64) -> Vec<f64> {
    if !lo.is_finite() || !hi.is_finite() || lo <= 0.0 || hi <= 0.0 {
        return vec![];
    }
    let llo = lo.ln() / base.ln();
    let lhi = hi.ln() / base.ln();
    let mn = llo.floor();
    let mx = lhi.ceil();
    if mx == mn {
        return vec![base.powf(mn)];
    }
    let in_rng = |b: f64| b >= lo && b <= hi;
    // integer powers of `base` from 10^mn to 10^mx, stepping the exponent
    let mk = |by: i64| -> Vec<f64> {
        let mut out = Vec::new();
        let mut e = mn;
        while e <= mx + 1e-9 {
            out.push(base.powf(e));
            e += by as f64;
        }
        out
    };
    let mut by = ((mx - mn) / n as f64).floor() as i64 + 1;
    let mut breaks = mk(by);
    if breaks.iter().filter(|b| in_rng(**b)).count() >= n.saturating_sub(2) {
        return breaks;
    }
    while by > 1 {
        by -= 1;
        breaks = mk(by);
        if breaks.iter().filter(|b| in_rng(**b)).count() >= n.saturating_sub(2) {
            return breaks;
        }
    }
    // ---- log_sub_breaks (breaks-log.R:175–207) ----
    if base > 2.0 {
        let mut steps: Vec<f64> = vec![1.0];
        let mut cand: Vec<f64> = (2..base as i64).map(|c| c as f64).collect();
        let mut all: Vec<f64> = Vec::new();
        let mut have_enough = false;
        while let Some(pos) = pick_delta(&cand, &steps, base) {
            steps.push(cand[pos]);
            cand.remove(pos);
            let mut acc: Vec<f64> = Vec::new();
            let mut e = mn;
            while e <= mx + 1e-9 {
                let p = base.powf(e);
                for &s in &steps {
                    acc.push(p * s);
                }
                e += 1.0;
            }
            acc.sort_by(|a, b| a.partial_cmp(b).unwrap());
            acc.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            if acc.iter().filter(|b| in_rng(**b)).count() >= n.saturating_sub(2) {
                all = acc;
                have_enough = true;
                break;
            }
        }
        if have_enough {
            // pad ±1 index beyond the in-range window (reference lines 27–31)
            let rel_idx: Vec<usize> = all
                .iter()
                .enumerate()
                .filter(|(_, b)| in_rng(**b))
                .map(|(i, _)| i)
                .collect();
            let lo_i = rel_idx.first().map(|i| (*i as i64 - 1).max(0) as usize).unwrap_or(0);
            let hi_i = rel_idx.last().map(|i| (*i + 1).min(all.len() - 1)).unwrap_or(0);
            return all[lo_i..=hi_i].to_vec();
        }
    }
    // final fallback: extended breaks over the raw data range
    extended_breaks(lo, hi, n)
}

/// Greedy delta pick from log_sub_breaks: choose the candidate that maximises
/// the minimum log-gap after appending it to `steps` (i.e. the sparsest).
fn pick_delta(cand: &[f64], steps: &[f64], base: f64) -> Option<usize> {
    if cand.is_empty() {
        return None;
    }
    let delta = |st: &mut Vec<f64>| -> f64 {
        // R: min(diff(log(sort(c(x, steps, base)), base))) — the fixed anchor
        // point is log_base(base) = 1.0 (not 0).
        let mut pts: Vec<f64> = st.iter().map(|s| s.ln() / base.ln()).collect();
        pts.push(1.0);
        pts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        pts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        pts.windows(2).map(|w| w[1] - w[0]).fold(f64::INFINITY, f64::min)
    };
    let mut best = 0usize;
    let mut bestv = f64::NEG_INFINITY;
    for (i, c) in cand.iter().enumerate() {
        let mut st2: Vec<f64> = steps.to_vec();
        st2.push(*c);
        let d = delta(&mut st2);
        // eps ties to R's which.max, which returns the FIRST maximum (i.e.
        // the smaller multiplier when 5 vs 6 tie at log-gap 0.222…)
        if d > bestv + 1e-12 {
            bestv = d;
            best = i;
        }
    }
    Some(best)
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transform: Option<TransformSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expand4: Option<ExpandSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        oob: Option<Oob>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        n_breaks: Option<usize>,
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
            transform: None,
            expand4: None,
            oob: None,
            n_breaks: None,
        }
    }
}

/// Trained continuous scale. All internal geometry (`t_range`, `breaks`) lives
/// in the TRANSFORMED space so log/sqrt/reverse expand and place breaks like
/// ggplot2; `map(v)` transforms the incoming data value first. `range` and
/// `limits` are reported back in DATA space (inverse) for display/debug.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuousScale {
    pub limits: Range,
    pub range: Range,
    /// transformed-space expanded range — map() normalises against this
    pub t_range: Range,
    pub transform: TransformSpec,
    pub oob: Oob,
    /// major breaks in TRANSFORMED space
    pub breaks: Vec<f64>,
    pub labels: Vec<String>,
    pub name: Option<String>,
}

impl ContinuousScale {
    /// `data_range` is the range of the layer's values **already transformed**
    /// into this scale's space by the caller (identity → unchanged); values
    /// outside the domain (log of ≤0) are filtered out by the caller.
    pub fn train(data_range: Range, spec: &ScaleSpec) -> ContinuousScale {
        let (limits, expand, expand4, breaks, labels, name, tr, oob, n_breaks) = match spec {
            ScaleSpec::Continuous {
                limits,
                breaks,
                labels,
                name,
                expand,
                expand4,
                transform,
                oob,
                n_breaks,
            } => (
                limits.map(|[a, b]| Range { min: a, max: b }),
                *expand,
                *expand4,
                breaks.clone(),
                labels.clone(),
                name.clone(),
                transform.unwrap_or(TransformSpec::Identity),
                oob.unwrap_or(Oob::Censor),
                n_breaks.unwrap_or(5),
            ),
            _ => (None, None, None, None, None, None, TransformSpec::Identity, Oob::Censor, 5),
        };
        // old symmetric `expand` [mult, add] maps to a 4-element ExpandSpec.
        let exp4 = expand4.unwrap_or_else(|| match expand {
            Some([m, a]) => ExpandSpec { mult_l: m, mult_r: m, add_l: a, add_r: a },
            None => ExpandSpec::DEFAULT_CONTINUOUS,
        });
        // unexpanded limits in transformed space (spec.limits are DATA-space)
        let mut t_limits = match limits {
            Some(l) => {
                let (a, b) = (tr.transform(l.min), tr.transform(l.max));
                let (a, b) = (a.min(b), a.max(b));
                Range { min: a, max: b }
            }
            None => data_range,
        };
        if t_limits.min == t_limits.max {
            let half = if t_limits.min == 0.0 { 0.5 } else { t_limits.min.abs() * 0.1 };
            t_limits = Range { min: t_limits.min - half, max: t_limits.max + half };
        }
        let (t_lo, t_hi) = exp4.expand_range4(t_limits.min, t_limits.max);
        let t_range = Range { min: t_lo, max: t_hi };
        // data-space view of the expanded range, ascending
        let (i0, i1) = (tr.inverse(t_lo), tr.inverse(t_hi));
        let (data_lo, data_hi) = (i0.min(i1), i0.max(i1));
        // breaks: fn runs in DATA space, results transform back (scale-.R:1193–1208).
        // Pair with labels before sorting by transformed value so reverse
        // (where the transformed order is descending) keeps labels aligned.
        let breaks_data = breaks.unwrap_or_else(|| tr.breaks(data_lo, data_hi, n_breaks));
        let labels_in = labels.clone().unwrap_or_else(|| format_breaks(&breaks_data));
        let mut pairs: Vec<(f64, String)> = breaks_data
            .iter()
            .zip(labels_in.iter())
            .map(|(b, l)| (tr.transform(*b), l.clone()))
            .filter(|(t, _)| t.is_finite() && *t >= t_range.min && *t <= t_range.max)
            .collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let (breaks_t, labels): (Vec<f64>, Vec<String>) = pairs.into_iter().unzip();
        let (d0, d1) = (tr.inverse(t_limits.min), tr.inverse(t_limits.max));
        ContinuousScale {
            limits: Range { min: d0.min(d1), max: d0.max(d1) },
            range: Range { min: data_lo, max: data_hi },
            t_range,
            transform: tr,
            oob,
            breaks: breaks_t,
            labels,
            name,
        }
    }

    /// Mapped data value → normalised [0,1], applying oob (censor → NaN so the
    /// row is dropped, like ggplot2's "Removed N rows" for explicit limits).
    pub fn map(&self, v: f64) -> f64 {
        let t = self.transform.transform(v);
        let t = self.oob.apply(t, self.t_range.min, self.t_range.max);
        (t - self.t_range.min) / (self.t_range.max - self.t_range.min)
    }

    /// Derived-geometry mapping (errorbar caps, area corners): pure rescale,
    /// NO censor — these extend past the panel and are clipped by it, so they
    /// must not be nulled at the range edge.
    pub fn map_plain(&self, v: f64) -> f64 {
        let t = self.transform.transform(v);
        (t - self.t_range.min) / (self.t_range.max - self.t_range.min)
    }

    /// visible major ticks as (DATA-space position, label), sorted.
    pub fn breaks_in_range(&self) -> Vec<(f64, String)> {
        self.breaks
            .iter()
            .zip(self.labels.iter())
            .filter(|(b, _)| **b >= self.t_range.min && **b <= self.t_range.max)
            .map(|(b, l)| (self.transform.inverse(*b), l.clone()))
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

/// Numeric→numeric scale for the `size` / `alpha` / `linewidth` aesthetics.
/// ggplot2: size = area_pal (values ∝ sqrt(rescaled), range 1..6mm),
/// alpha = linear rescale to 0.1..1; linewidth = linear 1..6mm.
#[derive(Debug, Clone, PartialEq)]
pub struct NumScale {
    pub data_lo: f64,
    pub data_hi: f64,
    pub out_lo: f64,
    pub out_hi: f64,
    pub area: bool,
    pub name: Option<String>,
}

impl NumScale {
    pub fn train(vals: &[f64], range: [f64; 2], area: bool, name: Option<String>) -> Self {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &v in vals {
            if v.is_finite() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if !lo.is_finite() || !hi.is_finite() {
            lo = 0.0;
            hi = 1.0;
        }
        NumScale { data_lo: lo, data_hi: hi, out_lo: range[0], out_hi: range[1], area, name }
    }

    /// rescale (scales::rescale, clamped) then the palette transform:
    /// area_pal is `rescale(sqrt(x), range, c(0,1))` → out = lo + (hi−lo)·√t.
    pub fn map(&self, v: f64) -> f64 {
        if !v.is_finite() {
            return self.out_lo;
        }
        let t = if self.data_hi == self.data_lo {
            1.0
        } else {
            ((v - self.data_lo) / (self.data_hi - self.data_lo)).clamp(0.0, 1.0)
        };
        if self.area {
            self.out_lo + (self.out_hi - self.out_lo) * t.sqrt()
        } else {
            self.out_lo + (self.out_hi - self.out_lo) * t
        }
    }

    /// legend breaks: extended(5) over the data range, censored inside it
    /// (verified against R 4.0.3: hp∈[52,335] → 100..300).
    pub fn breaks(&self) -> Vec<f64> {
        if self.data_hi == self.data_lo {
            return vec![self.data_lo];
        }
        extended_breaks(self.data_lo, self.data_hi, 5)
            .into_iter()
            .filter(|b| *b >= self.data_lo && *b <= self.data_hi)
            .collect()
    }
}

/// Colour mapping for a discrete aesthetic: hue palette by default,
/// or manual palette / values from spec.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteColourScale {
    pub levels: Vec<String>,
    pub colours: Vec<Color>,
    pub name: Option<String>,
    /// continuous form (colour mapped to a numeric column): data range +
    /// ramp endpoints (Lab-interpolated at draw time)
    pub data_lo: f64,
    pub data_hi: f64,
    pub ramp: Vec<String>,
}

impl DiscreteColourScale {
    pub fn train(levels: Vec<String>, spec: &ScaleSpec) -> Self {
        Self::train_with_palette(levels, spec, None, &[])
    }
    /// theme-aware: `palette` = house discrete palette (None → ggplot2 hue),
    /// `ramp` = multi-stop hex ramp for the continuous form (empty → default
    /// 2-stop gradient)
    pub fn train_with_palette(
        levels: Vec<String>,
        spec: &ScaleSpec,
        palette: Option<&Vec<Color>>,
        ramp: &[String],
    ) -> Self {
        let default_pal = palette.map_or_else(|| hue_palette(levels.len()), |p| p.clone());
        let colours = match spec {
            ScaleSpec::DiscreteManual { values, .. } => {
                values.iter().filter_map(|s| Color::parse(s)).collect()
            }
            _ => default_pal.clone(),
        };
        let colours = if colours.len() == levels.len() {
            colours
        } else {
            // resample the house palette to the level count (linear indices)
            let n = levels.len().max(1);
            match palette {
                Some(p) if !p.is_empty() => (0..n)
                    .map(|i| p[(i * p.len()) / n])
                    .collect(),
                _ => hue_palette(n),
            }
        };
        let name = match spec {
            ScaleSpec::DiscreteManual { name, .. } => name.clone(),
            ScaleSpec::Gradient { name, .. } => name.clone(),
            _ => None,
        };
        DiscreteColourScale {
            levels,
            colours,
            name,
            data_lo: 0.0,
            data_hi: 1.0,
            ramp: match spec {
                ScaleSpec::Gradient { low, high, .. } => vec![low.clone(), high.clone()],
                _ => ramp.to_vec(),
            },
        }
    }
    /// continuous ramp colour at t∈[0,1], Lab-interpolated across all stops
    /// (empty ramp → ggplot2 default gradient #132B43→#56B1F7).
    pub fn ramp_color(&self, t: f64) -> Color {
        multi_ramp(&self.ramp, t)
    }
    pub fn ramp_hex(&self) -> (&str, &str) {
        // legacy 2-stop accessor (gradient bar fallback)
        match (self.ramp.first(), self.ramp.last()) {
            (Some(a), Some(b)) if self.ramp.len() == 2 => (a.as_str(), b.as_str()),
            _ => ("#132B43", "#56B1F7"),
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


    // helper: enum variants don't support FRU, so construct full fields
    fn cont(transform: Option<TransformSpec>, expand4: Option<ExpandSpec>, limits: Option<[f64; 2]>, expand: Option<[f64; 2]>, oob: Option<Oob>) -> ScaleSpec {
        ScaleSpec::Continuous { limits, breaks: None, labels: None, name: None, expand, transform, expand4, oob, n_breaks: None }
    }

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

    #[test]
    fn log_breaks_sub_breaks_matches_scales() {
        // R: scales::log_breaks(5,10)(c(64.7,518.9)) -> 50 100 300 500 1000
        // exercises the greedy log_sub_breaks multiplier path.
        let b = log_breaks(64.7, 518.9, 5, 10.0);
        assert_eq!(b, vec![50.0, 100.0, 300.0, 500.0, 1000.0]);
    }

    #[test]
    fn log_breaks_integer_powers_matches_scales() {
        // R: scales::log_breaks(5,10)(c(1,1000)) -> 1 10 100 1000
        let b = log_breaks(1.0, 1000.0, 5, 10.0);
        assert_eq!(b, vec![1.0, 10.0, 100.0, 1000.0]);
    }

    #[test]
    fn log_breaks_narrow_falls_back_to_extended() {
        // R: scales::log_breaks(5,10)(c(1800,2000)) -> extended fallback
        // 1800 1850 1900 1950 2000 (a ~5-tick linear set over the narrow range)
        let b = log_breaks(1800.0, 2000.0, 5, 10.0);
        assert!(!b.is_empty());
        assert!(b[0] <= 1800.0 && *b.last().unwrap() >= 2000.0);
    }

    #[test]
    fn log10_transform_inverts_and_censors() {
        let t = TransformSpec::Log10;
        assert!((t.transform(100.0) - 2.0).abs() < 1e-12);
        assert!((t.inverse(2.0) - 100.0).abs() < 1e-9);
        assert!(t.transform(-5.0).is_nan());
        assert!(t.transform(0.0).is_nan());
    }

    #[test]
    fn lab_ramp_matches_scales_pal_grad() {
        // R: scale_colour_gradient breaks map 0/.25/.5/.75/1 ->
        // #132B43 #22496C #336A98 #448DC6 #56B1F7 (Lab space)
        let c = |t| crate::scale::lab_ramp("#132B43", "#56B1F7", t);
        assert_eq!(c(0.0).to_hex().to_uppercase(), "#132B43");
        assert_eq!(c(0.25).to_hex().to_uppercase(), "#22496C");
        assert_eq!(c(0.5).to_hex().to_uppercase(), "#336A98");
        assert_eq!(c(0.75).to_hex().to_uppercase(), "#448DC6");
        assert_eq!(c(1.0).to_hex().to_uppercase(), "#56B1F7");
    }

    #[test]
    fn reverse_scale_breaks_descending_labels_aligned() {
        // data [10,15,20,25,30,35] reversed: caller passes the TRANSFORMED
        // range (negated) = [-35,-10]. transformed breaks ascending (-35..-10)
        // → inverse = data 35..10, paired so each label stays with its value.
        let spec = cont(Some(TransformSpec::Reverse), None, None, None, None);
        let s = ContinuousScale::train(Range { min: -35.0, max: -10.0 }, &spec);
        let (brk, lbl): (Vec<f64>, Vec<String>) = s.breaks_in_range().into_iter().unzip();
        assert_eq!(brk, vec![35.0, 30.0, 25.0, 20.0, 15.0, 10.0]);
        assert_eq!(
            lbl,
            vec!["35", "30", "25", "20", "15", "10"]
        );
    }

    #[test]
    fn expand4_asymmetric_matches_expansion() {
        // expansion(mult = c(0, 0.1)) over [0,10]: left 0, right 1.0
        let spec = cont(None, Some(ExpandSpec { mult_l: 0.0, mult_r: 0.1, add_l: 0.0, add_r: 0.0 }), None, None, None);
        let s = ContinuousScale::train(Range { min: 0.0, max: 10.0 }, &spec);
        assert!((s.range.min - 0.0).abs() < 1e-9);
        assert!((s.range.max - 11.0).abs() < 1e-9);
    }

    #[test]
    fn oob_censor_maps_out_of_range_to_nan() {
        let spec = cont(None, None, Some([0.0, 10.0]), Some([0.0, 0.0]), Some(Oob::Censor));
        let s = ContinuousScale::train(Range { min: 0.0, max: 10.0 }, &spec);
        assert!((s.map(5.0) - 0.5).abs() < 1e-9);
        assert!(s.map(15.0).is_nan());
        assert!(s.map(-1.0).is_nan());
    }
}
