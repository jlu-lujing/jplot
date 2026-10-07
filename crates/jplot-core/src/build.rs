//! Build pipeline: resolve aes → compute stats → train scales → adjust
//! positions → produce resolved layer frames ready for layout.
//!
//! Numerical conventions (verified against refs/ggplot2 @ 4.0.3):
//! - `pos_dodge`: `x + width * ((i - 0.5)/n - 0.5)`, bin width `/n`.
//! - `geom_col` default width 0.9; `geom_bar` count width 1 (dodge 0.9).
//! - hist: `bins=30`, fullseq of `extended_breaks(n=15)`, width = bin width.
//! - quantile type-7 (R default).
//! - boxplot coef 1.5, whiskers = most extreme datum within fence.

use std::collections::HashMap;

use crate::data::{Column, Range};
use crate::error::JplotError;
use crate::scale::{
    column_is_discrete, extended_breaks, hcl_to_rgb, ContinuousScale, DiscreteColourScale,
    DiscreteScale,
};
use crate::spec::{AesSpec, GeomArgs, GeomSpec, LayerSpec, PlotSpec, PositionSpec, StatSpec};
pub use crate::data::Dataset;

/// A resolved numeric/categorical table for one layer after stats.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub n: usize,
    pub num: HashMap<String, Vec<f64>>,
    pub cat: HashMap<String, Vec<String>>,
    /// declared factor levels per aes (for ordering)
    pub levels: HashMap<String, Vec<String>>,
}

impl Frame {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_levels(&mut self, key: &str, v: Vec<String>) {
        self.levels.insert(key.into(), v);
    }
    pub fn set(&mut self, key: &str, v: Vec<f64>) {
        self.n = self.n.max(v.len());
        self.num.insert(key.into(), v);
    }
    pub fn set_cat(&mut self, key: &str, v: Vec<String>) {
        self.n = self.n.max(v.len());
        self.cat.insert(key.into(), v);
    }
    pub fn get(&self, key: &str) -> Option<&Vec<f64>> {
        self.num.get(key)
    }
    pub fn col(&self, key: &str) -> Option<&Vec<String>> {
        self.cat.get(key)
    }

    fn from_dataset(ds: &Dataset) -> Result<Frame, JplotError> {
        let mut f = Frame::new();
        for (name, c) in &ds.columns {
            match c {
                Column::Numeric { .. } => {
                    f.set(name, c.numeric().unwrap().to_vec());
                }
                Column::Categorical { levels, .. } => {
                    f.set(name, ds.numeric_of(name).unwrap());
                    f.set_cat(name, c.categorical().unwrap().to_vec());
                    if let Some(l) = levels {
                        f.set_levels(name, l.clone());
                    }
                }
            }
        }
        Ok(f)
    }
}

/// aes resolution: layer mapping wins, then plot mapping; `after_stat` cols
/// already in the frame win over both.
fn resolve_aes(layer: &LayerSpec, plot: &AesSpec) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    if layer.inherit_aes {
        for (k, v) in &plot.map {
            out.insert(k.clone(), v.clone());
        }
    }
    for (k, v) in &layer.mapping.map {
        out.insert(k.clone(), v.clone());
    }
    out
}

/// ggplot2 quantile(type = 7).
pub fn quantile7(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 {
        return sorted[0];
    }
    let h = (n as f64 - 1.0) * p;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

/// fullseq: `seq(floor(min/size)*size, ceil(max/size)*size, size)`
fn fullseq(min: f64, max: f64, size: f64) -> Vec<f64> {
    let start = (min / size).floor() * size;
    let end = (max / size).ceil() * size;
    let n = ((end - start) / size).round() as usize + 1;
    (0..n).map(|i| start + i as f64 * size).collect()
}

/// ggplot2 4.x `bin_breaks_bins` + `bin_breaks_width` (refs/ggplot2/R/bin.R).
/// Returns bin CENTRES over `bins` equal-width bins spanning [min, max], with
/// boundary offset so min/max sit in the outer half-bins.
pub fn default_bins(min: f64, max: f64, bins: usize) -> (Vec<f64>, f64) {
    let bins = bins.max(1);
    let (width, boundary) = if max - min < 1e-12 {
        (0.1, min)
    } else if bins == 1 {
        (max - min, min)
    } else {
        let mut width = (max - min) / (bins as f64 - 1.0);
        let mut boundary = min - width / 2.0;
        // R: if any(x_range %% width == boundary %% width) use /bins
        let eq = (min.rem_euclid(width) - boundary.rem_euclid(width)).abs() < 1e-9
            || (max.rem_euclid(width) - boundary.rem_euclid(width)).abs() < 1e-9;
        if eq {
            width = (max - min) / bins as f64;
            boundary = min - width / 2.0;
        }
        (width, boundary)
    };
    let shift = ((min - boundary) / width).floor();
    let origin = boundary + shift * width;
    let max_x = max + (1.0 - 1e-8) * width;
    let n_breaks = (((max_x - origin) / width).floor() as usize) + 1;
    let edges: Vec<f64> = (0..n_breaks.max(2)).map(|i| origin + i as f64 * width).collect();
    let centres = edges.windows(2).map(|e| (e[0] + e[1]) / 2.0).collect();
    (centres, width)
}

/// ggplot2 bin_breaks (closed = "right"): index of the (a, b] bin for v.
fn bin_index(v: f64, origin: f64, width: f64, n_bins: usize) -> usize {
    let rel = (v - origin) / width;
    let idx = if (rel - rel.round()).abs() < 1e-9 {
        // exact edge → right-closed: the bin whose right edge == v
        rel.round() as i64 - 1
    } else {
        rel.floor() as i64
    };
    (idx.max(0) as usize).min(n_bins - 1)
}

/// Stat computation per layer (M1 subset).
fn compute_stat(f: &Frame, spec: &StatSpec) -> Frame {
    match spec {
        StatSpec::Identity => f.clone(),
        StatSpec::Count { width } => {
            // ggplot2 StatCount: group by every categorical / ordinal mapped column
            let _ = width;
            let mut out = Frame::new();
            let mut keys: Vec<String> = Vec::new();
            for k in ["x", "colour", "fill", "linetype", "shape", "group"] {
                if f.cat.contains_key(k) || f.num.contains_key(k) {
                    keys.push(k.into());
                }
            }
            if keys.is_empty() {
                out.set("y", vec![f.n as f64]);
                return out;
            }
            let n = f.n;
            let cats: Vec<Vec<String>> = keys.iter().map(|k| f.cat.get(k).cloned().unwrap_or_else(|| vec![String::new(); n])).collect();
            let mut groups: Vec<(usize, usize)> = Vec::new(); // (first row, count)
            for i in 0..n {
                let match_ = groups.iter().position(|&(r, _)| {
                    (0..keys.len()).all(|j| cats[j].get(i) == cats[j].get(r))
                });
                match match_ {
                    Some(g) => groups[g].1 += 1,
                    None => groups.push((i, 1)),
                }
            }
            let ys: Vec<f64> = groups.iter().map(|g| g.1 as f64).collect();
            for k in keys.iter() {
                if let Some(catv) = f.cat.get(k) {
                    out.set_cat(k, groups.iter().map(|g| catv[g.0].clone()).collect());
                }
                // carry declared levels through
                if let Some(l) = f.levels.get(k) {
                    out.set_levels(k, l.clone());
                }
            }
            out.set("y", ys);
            out
        }
        StatSpec::Bin { bins, breaks } => {
            let xs = f.get("x").cloned().unwrap_or_default();
            let finite: Vec<f64> = xs.iter().cloned().filter(|v| v.is_finite()).collect();
            let (lo, hi) = match Range::from_values(&finite) {
                Some(r) => (r.min, r.max),
                None => (0.0, 1.0),
            };
            let (centres, width) = match breaks {
                Some(b) if b.len() >= 2 => {
                    let w = b.windows(2).map(|x| x[1] - x[0]).sum::<f64>() / (b.len() - 1) as f64;
                    (b.windows(2).map(|e| (e[0] + e[1]) / 2.0).collect(), w)
                }
                _ => default_bins(lo, hi, *bins),
            };
            let origin = centres[0] - width / 2.0;
            let mut counts = vec![0.0f64; centres.len()];
            let nb = counts.len();
            for &v in &finite {
                counts[bin_index(v, origin, width, nb)] += 1.0;
            }
            let mut out = Frame::new();
            let xmin: Vec<f64> = centres.iter().map(|c| c - width / 2.0).collect();
            let xmax: Vec<f64> = centres.iter().map(|c| c + width / 2.0).collect();
            out.set("x", centres.clone());
            out.set("y", counts);
            out.set("xmin", xmin);
            out.set("xmax", xmax);
            out.set("width", vec![width; centres.len()]);
            // carry colour/fill groups if present: keep first (M1 simplification)
            if f.cat.contains_key("colour") {
                out.set_cat("colour", f.cat["colour"].iter().take(centres.len()).cloned().collect());
            }
            out
        }
        StatSpec::Boxplot { coef } => {
            let coef = coef.unwrap_or(1.5);
            let xs = f.get("x").cloned().unwrap_or_default();
            let ys = f.get("y").cloned().unwrap_or_default();
            // group by x label (discrete) or single group
            let xlabels: Option<&Vec<String>> = f.cat.get("x");
            let groups: Vec<usize> = match xlabels {
                Some(lab) => lab.clone().into_iter().map(|l| lab.iter().position(|x| *x == l).unwrap()).collect(),
                None => vec![0; ys.len()],
            };
            let keys: Vec<usize> = {
                let mut k = Vec::new();
                for &g in &groups {
                    if !k.contains(&g) {
                        k.push(g);
                    }
                }
                k
            };
            let mut out = Frame::new();
            let (mut lower, mut middle, mut upper, mut ymin, mut ymax, mut outliers) =
                (vec![], vec![], vec![], vec![], vec![], vec![]);
            let (mut weight, mut nlower, mut nupper) = (vec![], vec![], vec![]);
            let mut xlab_out = Vec::new();
            let mut out_labels: Vec<String> = Vec::new();
            for (gi, &g) in keys.iter().enumerate() {
                let mut vals: Vec<f64> = (0..ys.len())
                    .filter(|i| groups[*i] == g)
                    .map(|i| ys[i])
                    .filter(|v| v.is_finite())
                    .collect();
                vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if vals.is_empty() {
                    continue;
                }
                let n = vals.len() as f64;
                let q1 = quantile7(&vals, 0.25);
                let q2 = quantile7(&vals, 0.5);
                let q3 = quantile7(&vals, 0.75);
                let iqr = q3 - q1;
                let lo_lim = q1 - coef * iqr;
                let hi_lim = q3 + coef * iqr;
                let inliers: Vec<f64> = vals.iter().cloned().filter(|v| *v >= lo_lim && *v <= hi_lim).collect();
                let w_lo = inliers.iter().cloned().fold(f64::INFINITY, f64::min);
                let w_hi = inliers.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                lower.push(q1);
                middle.push(q2);
                upper.push(q3);
                ymin.push(w_lo);
                ymax.push(w_hi);
                // ggplot2 StatBoxplot: notch = middle ± 1.58*iqr/sqrt(n);
                // weight = n; relvarwidth = sqrt(n) (used by geom varwidth)
                weight.push(n);
                let notch = 1.58 * iqr / n.sqrt();
                nlower.push(q2 - notch);
                nupper.push(q2 + notch);
                let lab = match xlabels {
                    Some(lab) => lab[g].clone(),
                    None => gi.to_string(),
                };
                xlab_out.push(lab.clone());
                for v in vals.iter().filter(|v| **v < lo_lim || **v > hi_lim) {
                    outliers.push(*v);
                    out_labels.push(lab.clone());
                }
            }
            out.set("ylower", lower);
            out.set("ymiddle", middle.clone());
            out.set("yupper", upper);
            out.set("ymin", ymin.clone());
            out.set("ymax", ymax.clone());
            out.set("notchlower", nlower);
            out.set("notchupper", nupper);
            out.set("weight", weight);
            out.set("y", middle);
            out.set_cat("x", xlab_out.clone());
            if let Some(decl) = f.levels.get("x") {
                out.set_levels("x", decl.clone());
            }
            if !outliers.is_empty() {
                out.set_cat("outlier_label", out_labels);
                out.set("outlier_y", outliers);
            }
            out.set("width", vec![0.75; xlab_out.len().max(1)]);
            out
        }
    }
}

/// Order discrete levels: declared factor levels first, else ggplot2 sort
/// (numeric-looking levels by numeric value, otherwise lexicographic).
fn order_levels(levels: &mut Vec<String>, declared: Option<&Vec<String>>) {
    let mut seen: Vec<String> = Vec::new();
    levels.retain(|l| {
        if seen.iter().any(|s| s == l) {
            false
        } else {
            seen.push(l.clone());
            true
        }
    });
    if let Some(decl) = declared {
        levels.sort_by_key(|l| (decl.iter().position(|d| d == l).unwrap_or(usize::MAX), l.clone()));
    } else {
        let all_num = levels.iter().all(|l| l.parse::<f64>().is_ok());
        if all_num {
            levels.sort_by(|a, b| a.parse::<f64>().unwrap().partial_cmp(&b.parse::<f64>().unwrap()).unwrap());
        } else {
            levels.sort();
        }
    }
}

/// Group key: aesthetic columns that create groups in order
/// (group > colour > fill > linetype > shape) — ggplot2 make_groups subset.
fn group_cols(f: &Frame) -> Vec<&'static str> {
    ["colour", "fill", "linetype"]
        .into_iter()
        .filter(|k| f.cat.contains_key(*k))
        .collect()
}

/// Group ids ordered by the trained discrete scale levels (ggplot2 orders
/// dodge slots by scale level, NOT first appearance in the data).
fn group_ids(
    f: &Frame,
    groups: &[&str],
    level_order: &HashMap<&str, Vec<String>>,
) -> Vec<usize> {
    if groups.is_empty() {
        return vec![0; f.n];
    }
    let mut ids = Vec::with_capacity(f.n);
    let mut seen: Vec<Vec<String>> = Vec::new();
    for i in 0..f.n {
        let key: Vec<String> = groups.iter().map(|g| f.cat[*g][i].clone()).collect();
        match seen.iter().position(|k| *k == key) {
            Some(p) => ids.push(p),
            None => {
                seen.push(key);
                ids.push(seen.len() - 1);
            }
        }
    }
    // reorder by declared level tuples (Cartesian product order, first aes varies slowest)
    if groups.iter().all(|g| level_order.contains_key(*g)) {
        let rank = |k: &Vec<String>| -> Vec<usize> {
            k.iter()
                .map(|v| groups.iter().enumerate().find_map(|(gi, g)| {
                    if f.cat[*g][0] == *v || f.cat[*g].contains(v) {
                        level_order[*g].iter().position(|l| l == v)
                    } else {
                        None
                    }
                }).unwrap_or(usize::MAX))
                .collect()
        };
        let mut order: Vec<usize> = (0..seen.len()).collect();
        order.sort_by_key(|&i| rank(&seen[i]));
        let mut new_ids = vec![0usize; ids.len()];
        for (new_g, old_g) in order.iter().enumerate() {
            for (i, g) in ids.iter().enumerate() {
                if *g == *old_g {
                    new_ids[i] = new_g;
                }
            }
        }
        return new_ids;
    }
    ids
}

fn apply_position(
    f: &mut Frame,
    pos: &PositionSpec,
    level_order: &HashMap<&str, Vec<String>>,
    default_width: f64,
) {
    match pos {
        PositionSpec::Identity => {}
        PositionSpec::Dodge { width } => {
            let width = *width;
            if f.get("x").is_none() {
                return;
            }
            // ggplot2 pos_dodge: global group index / global n (NOT per x)
            let xs = f.get("x").unwrap().clone();
            let groups = group_cols(f);
            let gids = group_ids(f, &groups, level_order);
            let n = gids.iter().cloned().max().map_or(0, |m| m + 1);
            if n <= 1 {
                let d = f.get("width").cloned().unwrap_or_else(|| vec![1.0; f.n]);
                f.set("xmin", xs.iter().enumerate().map(|(i, &x)| x - d[i] / 2.0).collect());
                f.set("xmax", xs.iter().enumerate().map(|(i, &x)| x + d[i] / 2.0).collect());
                return;
            }
            let d_width: Vec<f64> = f.get("width").cloned().unwrap_or_else(|| vec![default_width; f.n]);
            // n per x-position: groups present AT that x (pos_dodge n param)
            let new_x: Vec<f64> = xs
                .iter()
                .enumerate()
                .map(|(i, &x)| x + width * ((gids[i] as f64 + 0.5) / n as f64 - 0.5))
                .collect();
            // d_width / n where n = groups at this x position (count distinct gids per x)
            let n_at_x: Vec<f64> = xs
                .iter()
                .map(|&x| {
                    let mut gs: Vec<usize> = Vec::new();
                    for (j, &xj) in xs.iter().enumerate() {
                        if (xj - x).abs() < 1e-9 && !gs.contains(&gids[j]) {
                            gs.push(gids[j]);
                        }
                    }
                    gs.len() as f64
                })
                .collect();
            let new_xmin: Vec<f64> = new_x.iter().enumerate().map(|(i, &x)| x - d_width[i] / n_at_x[i] / 2.0).collect();
            let new_xmax: Vec<f64> = new_x.iter().enumerate().map(|(i, &x)| x + d_width[i] / n_at_x[i] / 2.0).collect();
            f.set("x", new_x);
            f.set("xmin", new_xmin);
            f.set("xmax", new_xmax);
        }
        PositionSpec::Jitter { width, height, seed } => {
            // deterministic LCG jitter, seed-dependent, ggplot2-ish spread:
            // uniform ±(w, h) * 0.5 scaled by 0.4? ggplot2: runif(2n, -w, h)/2?
            // jitter(): x + runif(n, -width, width)/2? No: uniform * w where w scaled by
            // sd*0.4? Simplify: uniform in [-w/2, w/2].
            let mut state = *seed ^ 0x5DEECE66D;
            let mut rnd = move || -> f64 {
                state = (state.wrapping_mul(6364136223846793005) + 1442695040888963407) >> 17;
                (state as f64) / (i64::MAX as f64)
            };
            let xs = f.get("x").cloned().unwrap_or_default();
            let ys = f.get("y").cloned().unwrap_or_default();
            let mut jx: Vec<f64> = xs.clone();
            for v in jx.iter_mut() {
                *v += (rnd() - 0.5) * width;
            }
            if !xs.is_empty() {
                f.set("x", jx);
            }
            let mut jy: Vec<f64> = ys.clone();
            for v in jy.iter_mut() {
                *v += (rnd() - 0.5) * height;
            }
            if !ys.is_empty() {
                f.set("y", jy);
            }
        }
    }
}

/// Resolve data range of all layers (for scale training), per aesthetic.
pub struct BuiltPlot {
    pub layers: Vec<BuiltLayer>,
    pub x_scale: BuiltScale,
    pub y_scale: BuiltScale,
    pub colour_scale: Option<DiscreteColourScale>,
    pub fill_scale: Option<DiscreteColourScale>,
    /// aes name mapped on the plot/layer ("colour"/"fill") → the data column
    /// (R variable name) it maps from, e.g. colour -> "g". ggplot2 uses it as
    /// the default guide title when no scale name is set.
    pub guide_sources: HashMap<String, String>,
    pub aes_defaults: HashMap<String, AesDefault>,
    pub plot: PlotSpec,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AesDefault {
    Discrete,
    Continuous,
}

#[derive(Debug)]
pub enum BuiltScale {
    Continuous(ContinuousScale),
    Discrete(DiscreteScale),
}

pub struct BuiltLayer {
    pub frame: Frame,
    pub geom: GeomSpec,
    pub args: GeomArgs,
    pub aes: HashMap<String, String>,
    pub group_ids: Vec<usize>,
}

fn column_from_spec(ds: &Dataset, name: &str, args: &GeomArgs, aes_col: Option<&String>) -> Option<Column> {
    if let Some(c) = aes_col {
        if let Some(col) = ds.get(c) {
            return Some(col.clone());
        }
    }
    // constants from args are applied later at draw time
    let _ = args;
    None
}

pub fn build(spec: &PlotSpec) -> Result<BuiltPlot, JplotError> {
    if spec.layers.is_empty() {
        return Err(JplotError::InvalidSpec("plot has no layers".into()));
    }
    let base = Frame::from_dataset(&spec.data)?;

    // 1. resolve per-layer frames
    let mut frames: Vec<Frame> = Vec::new();
    let mut aes_map: Vec<HashMap<String, String>> = Vec::new();
    for l in &spec.layers {
        let layer_data = match &l.data {
            Some(ds) => Frame::from_dataset(ds)?,
            None => base.clone(),
        };
        let aes = resolve_aes(l, &spec.mapping);
        // restrict frame columns to mapped aes + defaults we need
        let mut f = Frame::new();
        for (aes_key, col) in &aes {
            if let Some(c) = layer_data.get(col) {
                f.set(aes_key, c.clone());
            }
            if let Some(c) = layer_data.col(col) {
                f.set_cat(aes_key, c.clone());
            }
            if let Some(l) = layer_data.levels.get(col) {
                f.set_levels(aes_key, l.clone());
            }
            // constant-only aes (colour given via args) handled at draw
        }
        if f.n == 0 {
            return Err(JplotError::EmptyData);
        }
        let _ = column_from_spec; // keep import for future constants path
        let stat = l.stat.clone().unwrap_or(match l.geom {
            GeomSpec::Bar => StatSpec::Count { width: None },
            GeomSpec::Histogram { bins } => StatSpec::Bin { bins, breaks: None },
            GeomSpec::Boxplot => StatSpec::Boxplot { coef: None },
            _ => StatSpec::Identity,
        });
        let mut f = compute_stat(&f, &stat);
        // geom default width for bars/hist: 0.9 (col) / bin width (hist) / 1.0 (bar count)
        match l.geom {
            GeomSpec::Col => {
                let w = l.args.f64_("width").unwrap_or(0.9);
                f.set("width", vec![w; f.n]);
                let xs = f.get("x").cloned().unwrap_or_default();
                f.set("xmin", xs.iter().map(|x| x - w / 2.0).collect());
                f.set("xmax", xs.iter().map(|x| x + w / 2.0).collect());
                f.set("ymin", vec![0.0; f.n]); // ggplot2: bars extend to y=0
            }
            GeomSpec::Bar | GeomSpec::Boxplot => {
                let w = l.args.f64_("width").unwrap_or(if matches!(l.geom, GeomSpec::Bar) { 0.9 } else { 0.75 });
                if f.get("width").is_none() {
                    f.set("width", vec![w; f.n]);
                    let xs = f.get("x").cloned().unwrap_or_default();
                    f.set("xmin", xs.iter().map(|x| x - w / 2.0).collect());
                    f.set("xmax", xs.iter().map(|x| x + w / 2.0).collect());
                }
                if matches!(l.geom, GeomSpec::Bar) {
                    f.set("ymin", vec![0.0; f.n]);
                }
            }
            GeomSpec::Histogram { .. } => {
                let xs = f.get("x").cloned().unwrap_or_default();
                let w = f.get("width").map(|v| v[0]).unwrap_or(1.0);
                f.set("xmin", xs.iter().map(|x| x - w / 2.0).collect());
                f.set("xmax", xs.iter().map(|x| x + w / 2.0).collect());
                f.set("ymin", vec![0.0; f.n]);
            }
            GeomSpec::Point { .. } | GeomSpec::Line => {}
        }
        frames.push(f);
        aes_map.push(aes);
    }

    // 2. train scales. Discrete wins if ANY layer uses categorical for that aes
    //    (ggplot2: the first scale for the aesthetic determines it; discrete
    //    bars require discrete x even if another layer is continuous).
    let mut x_scale = None;
    let mut y_scale = None;
    let mut colour_scale = None;
    let mut fill_scale = None;

    let train = |name: &str| -> Option<BuiltScale> {
        // aes -> frame column names to scan (boxplot emits ylower/…/outlier_y)
        let aux: &[&str] = match name {
            "y" => &["ymin", "ymax", "ylower", "ymiddle", "yupper", "outlier_y"],
            "x" => &["xmin", "xmax", "outlier_x", "x_lower"],
            _ => &[],
        };
        let mut disc_levels: Vec<String> = Vec::new();
        let mut cont: Vec<f64> = Vec::new();
        for f in &frames {
            if let Some(v) = f.get(name) {
                cont.extend(v.iter().filter(|v| v.is_finite()).cloned());
            }
            for a in aux {
                if let Some(v) = f.get(a) {
                    cont.extend(v.iter().filter(|v| v.is_finite()).cloned());
                }
            }
            if let Some(v) = f.col(name) {
                for s in v {
                    if !disc_levels.iter().any(|l| l == s) {
                        disc_levels.push(s.clone());
                    }
                }
            }
        }
        let spec = spec.scales.get(name).cloned().unwrap_or_default();
        if !disc_levels.is_empty() {
            let declared = frames.iter().find_map(|f| f.levels.get(name)).cloned();
            order_levels(&mut disc_levels, declared.as_ref());
            // a manual scale may override
            if let crate::scale::ScaleSpec::DiscreteManual { values, .. } = &spec {
                if !values.is_empty() {
                    disc_levels = values.clone();
                }
            }
            Some(BuiltScale::Discrete(DiscreteScale::train(disc_levels, &spec)))
        } else if !cont.is_empty() {
            let r = Range::from_values(&cont)?;
            Some(BuiltScale::Continuous(ContinuousScale::train(r, &spec)))
        } else {
            None
        }
    };
    x_scale = train("x");
    y_scale = train("y");
    if x_scale.is_none() && y_scale.is_none() {
        return Err(JplotError::MissingAes("x or y".into()));
    }
    let _ = (column_is_discrete, hcl_to_rgb);

    let scale_levels = |name: &str| -> Option<Vec<String>> {
        match (name, &x_scale, &y_scale) {
            ("x", Some(BuiltScale::Discrete(d)), _) => Some(d.levels.clone()),
            ("y", _, Some(BuiltScale::Discrete(d))) => Some(d.levels.clone()),
            _ => None,
        }
    };
    let train_colour = |name: &str| -> Option<DiscreteColourScale> {
        let mut levels: Vec<String> = Vec::new();
        let mut cont_vals: Vec<f64> = Vec::new();
        let mut declared: Option<Vec<String>> = None;
        for f in &frames {
            if let Some(v) = f.col(name) {
                for s in v.iter().filter(|s| !s.is_empty()) {
                    if !levels.iter().any(|l| l == s) {
                        levels.push(s.clone());
                    }
                }
            }
            if declared.is_none() {
                declared = f.levels.get(name).cloned();
            }
            if let Some(v) = f.get(name) {
                cont_vals.extend(v.iter().filter(|v| v.is_finite()).cloned());
            }
        }
        let spec = spec.scales.get(name).cloned().unwrap_or_default();
        if !levels.is_empty() {
            order_levels(&mut levels, declared.as_ref());
            Some(DiscreteColourScale::train(levels, &spec))
        } else if cont_vals.len() > 1 {
            // continuous colour: gradient spec; default endpoints
            Some(DiscreteColourScale::train(vec![], &spec))
        } else {
            None
        }
    };
    colour_scale = train_colour("colour");
    fill_scale = train_colour("fill");

    let mut level_order: HashMap<&'static str, Vec<String>> = HashMap::new();
    if let Some(BuiltScale::Discrete(d)) = &x_scale {
        level_order.insert("x", d.levels.clone());
    }
    if let Some(BuiltScale::Discrete(d)) = &y_scale {
        level_order.insert("y", d.levels.clone());
    }
    if let Some(c) = &colour_scale {
        level_order.insert("colour", c.levels.clone());
    }
    if let Some(c) = &fill_scale {
        level_order.insert("fill", c.levels.clone());
    }
    let level_order_ref = &level_order;
    // 2b. ordinalise categorical aes against the trained scale levels
    for (fi, f) in frames.iter_mut().enumerate() {
        for name in ["x", "y"] {
            if let Some(levels) = scale_levels(name) {
                if let Some(cats) = f.col(name).cloned() {
                    let ord: Vec<f64> = cats
                        .iter()
                        .map(|c| levels.iter().position(|l| l == c).map_or(f64::NAN, |i| (i + 1) as f64))
                        .collect();
                    f.set(name, ord);
                }
            }
        }
        if let (Some(levels), Some(labs)) = (scale_levels("x"), f.col("outlier_label").cloned()) {
            let ox: Vec<f64> = labs
                .iter()
                .map(|c| levels.iter().position(|l| l == c).map_or(f64::NAN, |i| (i + 1) as f64))
                .collect();
            f.set("outlier_x", ox);
        }
        // rebuild box/bar xmin/xmax around the now-numeric x, then dodge.
        // boxplot: width = args$width %||% 0.75; varwidth scales by
        // sqrt(n)/max(sqrt(n)) (ggplot2 GeomBoxplot setup_data).
        let geom = spec.layers[fi].geom.clone();
        let is_barlike = matches!(geom, GeomSpec::Col | GeomSpec::Bar | GeomSpec::Histogram { .. } | GeomSpec::Boxplot);
        if is_barlike {
            if let Some(xs) = f.get("x").cloned() {
                let mut w = f.get("width").cloned().unwrap_or_else(|| vec![0.9; xs.len()]);
                if matches!(geom, GeomSpec::Boxplot) {
                    let base = spec.layers[fi].args.f64_("width").unwrap_or(0.75);
                    let varwidth = spec.layers[fi].args.bool_("varwidth").unwrap_or(false);
                    if varwidth {
                        if let Some(ns) = f.get("weight").cloned() {
                            let rel: Vec<f64> = ns.iter().map(|n| n.max(0.0).sqrt()).collect();
                            let mx = rel.iter().cloned().fold(f64::EPSILON, f64::max);
                            w = rel.iter().map(|r| base * r / mx).collect();
                        }
                    } else {
                        w = vec![base; xs.len()];
                    }
                    // outliers = FALSE → drop the outlier layer entirely
                    if spec.layers[fi].args.bool_("outliers") == Some(false) {
                        f.num.remove("outlier_y");
                        f.cat.remove("outlier_label");
                        f.num.remove("outlier_x");
                    }
                }
                let xmin: Vec<f64> = xs.iter().enumerate().map(|(i, &x)| x - w.get(i).copied().unwrap_or(0.9) / 2.0).collect();
                let xmax: Vec<f64> = xs.iter().enumerate().map(|(i, &x)| x + w.get(i).copied().unwrap_or(0.9) / 2.0).collect();
                f.set("width", w);
                f.set("xmin", xmin);
                f.set("xmax", xmax);
            }
        }
        let default_w = match spec.layers[fi].geom {
            crate::spec::GeomSpec::Col => 0.9,
            crate::spec::GeomSpec::Bar | crate::spec::GeomSpec::Histogram { .. } => 0.9,
            crate::spec::GeomSpec::Boxplot => 0.75,
            _ => 1.0,
        };
        apply_position(f, &spec.layers[fi].position, level_order_ref, default_w);
    }

    let layers = frames
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            let groups = group_cols(&f);
            BuiltLayer {
                group_ids: group_ids(&f, &groups, level_order_ref),
                frame: f,
                geom: spec.layers[i].geom.clone(),
                args: spec.layers[i].args.clone(),
                aes: aes_map[i].clone(),
            }
        })
        .collect();

    let mut guide_sources: HashMap<String, String> = HashMap::new();
    for aes in &aes_map {
        for k in ["colour", "fill"] {
            if let Some(v) = aes.get(k) {
                guide_sources.entry(k.to_string()).or_insert_with(|| v.clone());
            }
        }
    }
    Ok(BuiltPlot {
        layers,
        x_scale: x_scale.unwrap(),
        y_scale: y_scale.unwrap(),
        colour_scale,
        fill_scale,
        guide_sources,
        aes_defaults: HashMap::new(),
        plot: spec.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Column;
    use crate::spec::{aes, geom_bar, geom_col, geom_histogram, geom_point, ggplot};

    fn num(v: Vec<f64>) -> Column {
        Column::Numeric { values: v, label: None }
    }
    fn cat(v: Vec<&str>) -> Column {
        Column::Categorical { values: v.iter().map(|s| s.to_string()).collect(), levels: None }
    }

    #[test]
    fn quantile7_matches_r() {
        // quantile(c(1,2,3,4,5), c(.25,.5,.75), type=7) = 2,3,4
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(quantile7(&v, 0.25), 2.0);
        assert_eq!(quantile7(&v, 0.5), 3.0);
        assert_eq!(quantile7(&v, 0.75), 4.0);
        // r(1:4)[2.5] type 7 = 1.75
        let v = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(quantile7(&v, 0.25), 1.75);
    }

    #[test]
    fn count_grouping() {
        let mut d = Dataset::new();
        d.add("g", cat(vec!["a", "b", "a", "c", "a", "b"]));
        let mut m = HashMap::new();
        m.insert("x".into(), "g".into());
        let p = ggplot(d) + aes(m) + geom_bar();
        let b = build(&p.spec).unwrap();
        let ys = b.layers[0].frame.get("y").unwrap();
        assert_eq!(ys, &vec![3.0, 2.0, 1.0]);
        assert_eq!(b.layers[0].frame.cat.get("x").unwrap(), &vec!["a", "b", "c"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn dodge_two_groups() {
        // two bars per level, width .9: centers at k ± .9/4 → x + .9*((g+0.5)/2-.5)
        let mut d = Dataset::new();
        d.add("g", cat(vec!["a", "b", "a", "b"]));
        d.add("v", cat(vec!["x", "x", "y", "y"]));
        d.add("c", cat(vec!["p", "p", "q", "q"]));
        let mut m = HashMap::new();
        m.insert("x".into(), "g".into());
        m.insert("fill".into(), "c".into());
        let mut l = geom_bar();
        l.position = PositionSpec::Dodge { width: 0.9 };
        l.mapping = aes(m.clone());
        let p = ggplot(d) + l;
        let b = build(&p.spec).unwrap();
        let f = &b.layers[0].frame;
        // groups: (a,p),(b,p),(a,q),(b,q) → n=2; order a,p / b,p / a,q / b,q
        let xs = f.get("x").unwrap();
        assert_eq!(xs.len(), 4);
        // group index of first two = 0 → x + .9*((0.5)/2-.5)= x-.225
        assert!((xs[0] - (1.0 - 0.225)).abs() < 1e-9);
        assert!((xs[2] - (1.0 + 0.225)).abs() < 1e-9);
    }

    #[test]
    fn histogram_bins_default() {
        let mut d = Dataset::new();
        d.add("x", num((0..100).map(|i| i as f64).collect()));
        let mut m = HashMap::new();
        m.insert("x".into(), "x".into());
        let p = ggplot(d) + aes(m) + geom_histogram(30);
        let b = build(&p.spec).unwrap();
        let ys = b.layers[0].frame.get("y").unwrap();
        // ggplot2 default on 0..99: breaks extended(n=15)->width; counts should sum to 100
        assert_eq!(ys.iter().sum::<f64>(), 100.0);
    }

    #[test]
    fn geom_col_train_continuous_x() {
        let mut d = Dataset::new();
        d.add("x", num(vec![1.0, 2.0, 3.0]));
        d.add("y", num(vec![5.0, 2.0, 8.0]));
        let mut m = HashMap::new();
        m.insert("x".into(), "x".into());
        m.insert("y".into(), "y".into());
        let p = ggplot(d) + aes(m) + geom_col();
        let b = build(&p.spec).unwrap();
        assert!(matches!(b.x_scale, BuiltScale::Continuous(_)));
    }
}
