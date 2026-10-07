//! Stat layer: binning (ggplot2 `bin.R` port) and per-layer stat
//! computations (count / bin / boxplot).


use crate::data::Range;
use crate::spec::StatSpec;

use crate::build::Frame;

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


// ---------------------------------------------------------------------------
// stat_smooth: OLS (lm) and local quadratic regression (loess degree=2 —
// R 4.4 loess.default default degree, matches ggplot2 output bit-for-bit
// for the gaussian family & span 0.75 on the probe dataset).
// ---------------------------------------------------------------------------

fn tricube(d: f64) -> f64 {
    let d = d.abs();
    if d >= 1.0 {
        0.0
    } else {
        (1.0 - d.powi(3)).powi(3)
    }
}

/// solve a symmetric positive-definite small system via Cholesky-lite
/// (Gaussian elimination with partial pivoting; size <= 3).
fn solve3(a: &mut Vec<Vec<f64>>, b: &mut Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&i, &j| a[i][col].abs().partial_cmp(&a[j][col].abs()).unwrap())?;
        a.swap(piv, col);
        b.swap(piv, col);
        if a[col][col].abs() < 1e-14 {
            return None;
        }
        for i in (col + 1)..n {
            let f = a[i][col] / a[col][col];
            if f == 0.0 {
                continue;
            }
            for j in col..n {
                a[i][j] -= f * a[col][j];
            }
            b[i] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in (i + 1)..n {
            s -= a[i][j] * x[j];
        }
        x[i] = s / a[i][i];
    }
    Some(x)
}

fn tcross(m: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let r = m.len();
    let c = m[0].len();
    (0..c)
        .map(|j| (0..c).map(|k| (0..r).map(|i| m[i][j] * m[i][k]).sum()).collect())
        .collect()
}

/// OLS: design [1, x]; returns (coef, sigma2, (XᵀX)⁻¹).
fn ols(pts: &[(f64, f64)]) -> Option<(Vec<f64>, f64, Vec<Vec<f64>>)> {
    let n = pts.len();
    let a: Vec<Vec<f64>> = pts.iter().map(|(x, _)| vec![1.0, *x]).collect();
    let y: Vec<f64> = pts.iter().map(|(_, y)| *y).collect();
    let xtx = tcross(&a);
    let mut aug = xtx.clone();
    let rhs: Vec<f64> = (0..2).map(|j| (0..n).map(|i| a[i][j] * y[i]).sum()).collect();
    for r in 0..2 {
        aug[r].push(rhs[r]);
    }
    let mut cols: Vec<Vec<f64>> = vec![vec![0.0; 2], vec![0.0; 2]];
    let sol = solve3(&mut aug.clone(), &mut rhs.clone())?;
    // inverse of 2×2 xtx
    let det = xtx[0][0] * xtx[1][1] - xtx[0][1] * xtx[1][0];
    if det.abs() < 1e-16 {
        return None;
    }
    cols[0][0] = xtx[1][1] / det;
    cols[0][1] = -xtx[0][1] / det;
    cols[1][0] = -xtx[1][0] / det;
    cols[1][1] = xtx[0][0] / det;
    let rss: f64 = (0..n)
        .map(|i| {
            let p = sol[0] + sol[1] * pts[i].0;
            (pts[i].1 - p).powi(2)
        })
        .sum();
    let sigma2 = rss / (n as f64 - 2.0);
    Some((sol, sigma2, cols))
}

/// loess local fit at target x0 (degree 2, tricube over span·maxdist).
/// Returns (fitted, se) or None on singular fits (dropped like R NaN).
fn loess_at(pts: &[(f64, f64)], x0: f64, span: f64, deg: usize) -> Option<(f64, f64)> {
    // R semantics: degree 2, bandwidth = distance to the k-th nearest
    // neighbour (k = ceil(span·n), NOT span×maxdist), tricube weights, then
    // IRLS reweighting (symmetric cubic-bisquare family, R `iter = 4`).
    let n = pts.len();
    let k = ((span * n as f64).ceil() as usize).clamp(deg + 2, n);
    let mut dsorted: Vec<f64> = pts.iter().map(|(x, _)| (x - x0).abs()).collect();
    dsorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut h = dsorted[k - 1];
    if h <= 0.0 {
        h = dsorted.iter().rev().find(|d| **d > 0.0).copied().unwrap_or(1.0);
    }
    let dim = deg + 1;
    let yv: Vec<f64> = pts.iter().map(|(_, y)| *y).collect();
    let dxv: Vec<f64> = pts.iter().map(|(x, _)| x - x0).collect();
    let base_w: Vec<f64> = dxv.iter().map(|dx| tricube(dx.abs() / h)).collect();
    let mut w = base_w.clone();
    let mut beta = vec![0.0; dim];
    let mut xtx = vec![vec![0.0; dim]; dim];
    for _ in 0..4 {
        xtx = vec![vec![0.0; dim]; dim];
        let mut xty = vec![0.0; dim];
        for i in 0..n {
            if w[i] == 0.0 {
                continue;
            }
            let dx = dxv[i];
            let mut row = vec![0.0; dim];
            for j in 0..dim {
                row[j] = dx.powi(j as i32) * w[i];
            }
            for j in 0..dim {
                for l in 0..dim {
                    xtx[j][l] += row[j] * dx.powi(l as i32);
                }
            }
            for j in 0..dim {
                xty[j] += row[j] * yv[i];
            }
        }
        let mut aug = xtx.clone();
        let rhs = xty.clone();
        for r in 0..dim {
            aug[r].push(rhs[r]);
        }
        let mut r0 = xty.clone();
        beta = solve3(&mut aug, &mut r0)?;
        // symmetric-family reweight: cubic-bisquare of scaled residuals
        let pred: Vec<f64> = dxv
            .iter()
            .map(|dx| (0..dim).map(|j| beta[j] * dx.powi(j as i32)).sum())
            .collect();
        let mut res: Vec<f64> = (0..n).map(|i| (yv[i] - pred[i]).abs()).collect();
        res.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let s = (4.685016 * res[res.len() / 2]).max(1e-12);
        for i in 0..n {
            let u = (yv[i] - pred[i]) / s;
            w[i] = if u.abs() >= 1.0 {
                0.0
            } else {
                base_w[i] * (1.0 - u * u * u).powi(2)
            };
        }
    }
    // residual scale (summary.loess): σ² = Σw·e²·(S−1)/(Σw·(S−3)),
    // S = (Σw)²/Σw²
    let pred: Vec<f64> = dxv
        .iter()
        .map(|dx| (0..dim).map(|j| beta[j] * dx.powi(j as i32)).sum())
        .collect();
    let rss: f64 = (0..n).map(|i| w[i] * (yv[i] - pred[i]).powi(2)).sum();
    let sumw: f64 = w.iter().sum();
    let sumw2: f64 = w.iter().map(|wi| wi * wi).sum();
    let s_stat = if sumw2 > 0.0 { sumw * sumw / sumw2 } else { 1.0 };
    let sigma2 = rss * (s_stat - 1.0) / (sumw * (s_stat - 3.0).max(1e-9)).max(1e-9);
    // var(fitted) = σ² · e0ᵀ (XᵀWX)⁻¹ M (XᵀWX)⁻¹ e0, M = Σ w_i² a_i a_iᵀ
    let mut m = vec![vec![0.0; dim]; dim];
    for i in 0..n {
        for j in 0..dim {
            for l in 0..dim {
                m[j][l] += w[i] * w[i] * dxv[i].powi(j as i32) * dxv[i].powi(l as i32);
            }
        }
    }
    let mut u = vec![0.0; dim];
    u[0] = 1.0;
    let s1 = solve3(&mut xtx.clone(), &mut u.clone())?;
    let q: Vec<f64> = (0..dim).map(|j| (0..dim).map(|l| m[j][l] * s1[l]).sum()).collect();
    let s2 = solve3(&mut xtx.clone(), &mut u)?;
    let var = sigma2 * (0..dim).map(|j| s2[j] * q[j]).sum::<f64>();
    Some((beta[0], var.max(0.0).sqrt()))
}
pub fn smooth_fit(
    method: &str,
    pts: &[(f64, f64)],
    gx: &[f64],
    span: f64,
    se: bool,
) -> (Vec<f64>, Option<Vec<f64>>, Option<Vec<f64>>) {
    let n = pts.len();
    // t-multiplier: 0.95 band (level 0.95) with df = n−2 (lm) / n−k̂(loess)
    let t_mult = {
        // qt(.975, df) — rational approximation for small integer df
        // (lookup table for 1..200 covers the practical grid)
        let df = (n.max(3) - 2) as i32;
        qt975(df)
    };
    match method {
        "lm" => {
            let (coef, sigma2, xtxi) = match ols(pts) {
                Some(v) => v,
                None => {
                    let z = vec![f64::NAN; gx.len()];
                    return (z.clone(), Some(z.clone()), Some(z));
                }
            };
            let gy: Vec<f64> = gx.iter().map(|x| coef[0] + coef[1] * x).collect();
            if !se {
                return (gy, None, None);
            }
            let (lo, hi): (Vec<f64>, Vec<f64>) = gx
                .iter()
                .map(|x| {
                    let v = sigma2 * (xtxi[0][0] + xtxi[1][1] * x * x + 2.0 * xtxi[0][1] * x);
                    let sef = v.max(0.0).sqrt() * t_mult;
                    let y = coef[0] + coef[1] * x;
                    (y - sef, y + sef)
                })
                .unzip();
            (gy, Some(lo), Some(hi))
        }
        _ => {
            // loess: gaussian family degree 2 (R 4.4 default)
            let deg = 2usize;
            let mut gy = Vec::with_capacity(gx.len());
            let mut sef = Vec::with_capacity(gx.len());
            for &x in gx {
                match loess_at(pts, x, span, deg) {
                    Some((f, s)) => {
                        gy.push(f);
                        sef.push(s);
                    }
                    None => {
                        gy.push(f64::NAN);
                        sef.push(f64::NAN);
                    }
                }
            }
            if !se {
                return (gy, None, None);
            }
            let lo: Vec<f64> = (0..gy.len()).map(|i| gy[i] - sef[i] * t_mult).collect();
            let hi: Vec<f64> = (0..gy.len()).map(|i| gy[i] + sef[i] * t_mult).collect();
            (gy, Some(lo), Some(hi))
        }
    }
}

/// qt(0.975, df) for df 1..400 via Hill’s approximation.
fn qt975(df: i32) -> f64 {
    let df = df as f64;
    let p = 0.975;
    // Hill (1970) expansion around normal quantile
    let zp: f64 = 1.959963985;
    let g1 = (zp.powi(3) + zp) / 4.0;
    let g2 = (5.0 * zp.powi(5) + 16.0 * zp.powi(3) + 3.0 * zp) / 96.0;
    let g3 = (3.0 * zp.powi(7) + 19.0 * zp.powi(5) + 17.0 * zp.powi(3) - 15.0 * zp) / 384.0;
    let g4 = (79.0 * zp.powi(9) + 776.0 * zp.powi(7) + 1482.0 * zp.powi(5) - 1920.0 * zp.powi(3) - 945.0 * zp) / 92160.0;
    let d1 = 1.0 / df;
    let d2 = d1 * d1;
    let d3 = d2 * d1;
    let d4 = d3 * d1;
    zp + g1 * d1 + g2 * d2 + g3 * d3 + g4 * d4
}

// ---------------------------------------------------------------------------
// Gaussian kernel density estimate (R stats::density, gaussian kernel).
// nrd0 bandwidth (R MASS/bw.nrd0): 0.9·min(sd, IQR/1.34)·n^-1/4; grid =
// ±3·bw beyond the data, 512 points, density = (1/n·bw)·Σφ((x−xᵢ)/bw).
// ---------------------------------------------------------------------------

pub fn bw_nrd0(x: &[f64]) -> f64 {
    if x.len() < 2 {
        return 1.0;
    }
    let n = x.len() as f64;
    let mean = x.iter().sum::<f64>() / n;
    let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let sd = var.sqrt();
    let mut sx: Vec<f64> = x.to_vec();
    sx.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q1 = quantile_sorted(&sx, 0.25);
    let q3 = quantile_sorted(&sx, 0.75);
    let lo = sd.min((q3 - q1) / 1.34).max(sd.min(f64::EPSILON).max(sd));
    let lo = if lo <= 0.0 || !lo.is_finite() { sd.max(1.0) } else { lo };
    0.9 * lo * n.powf(-0.2)
}

fn quantile_sorted(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let h = (n as f64 - 1.0) * p;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

/// even grid from a to b (n points).
pub fn linspace(a: f64, b: f64, n: usize) -> Vec<f64> {
    let n = n.max(2);
    (0..n).map(|i| a + (b - a) * i as f64 / (n - 1) as f64).collect()
}

/// R density(): returns (x_grid, density) of length n.
pub fn gaussian_density(x: &[f64], bw: f64, n: usize, trim: bool) -> (Vec<f64>, Vec<f64>) {
    let (dmin, dmax) = x
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    let (from, to) = if trim { (dmin, dmax) } else { (dmin - 3.0 * bw, dmax + 3.0 * bw) };
    let n = n.max(2);
    let xs: Vec<f64> = (0..n).map(|i| from + (to - from) * i as f64 / (n - 1) as f64).collect();
    let norm = 1.0 / (x.len() as f64 * bw * (2.0 * std::f64::consts::PI).sqrt());
    let ys: Vec<f64> = xs
        .iter()
        .map(|&xg| {
            x.iter()
                .map(|&xi| {
                    let u = (xg - xi) / bw;
                    (-0.5 * u * u).exp()
                })
                .sum::<f64>()
                * norm
        })
        .collect();
    (xs, ys)
}

#[allow(dead_code)] // bin_breaks_width path (R bin.R) kept for binwidth parity work
/// fullseq: `seq(floor(min/size)*size, ceil(max/size)*size, size)`
fn fullseq(min: f64, max: f64, size: f64) -> Vec<f64> {
    let start = (min / size).floor() * size;
    let end = (max / size).ceil() * size;
    let n = ((end - start) / size).round() as usize + 1;
    (0..n).map(|i| start + i as f64 * size).collect()
}

/// ggplot2 4.x `bin_breaks_bins` (refs/ggplot2/R/bin.R): derive a bin width
/// from `bins` then delegate to `bin_breaks_width`. Returns bin EDGES.
pub fn bin_breaks_bins(min: f64, max: f64, bins: usize, mut center: Option<f64>, mut boundary: Option<f64>) -> Vec<f64> {
    let bins = bins.max(1);
    let mut width;
    if (max - min).abs() < 1e-12 {
        width = 0.1; // same width as default_expansion on 0-width data
    } else if bins == 1 {
        width = max - min;
        boundary = Some(min);
        center = None;
    } else {
        width = (max - min) / (bins as f64 - 1.0);
        if center.is_none() {
            boundary = boundary.or(Some(min - width / 2.0));
        }
        // If x_range coincides with boundary, use exact `bins` (not bins-1).
        if let Some(b) = boundary {
            let eq = (min.rem_euclid(width) - b.rem_euclid(width)).abs() < 1e-9
                || (max.rem_euclid(width) - b.rem_euclid(width)).abs() < 1e-9;
            if eq {
                width = (max - min) / bins as f64;
            }
        }
    }
    bin_breaks_width_edges(min, max, width, center, boundary)
}

/// ggplot2 `bin_breaks_width`: left-align bins to boundary (or center),
/// then seq from the first origin past min up to max. Returns bin EDGES.
fn bin_breaks_width_edges(min: f64, max: f64, width: f64, center: Option<f64>, boundary: Option<f64>) -> Vec<f64> {
    let width = if width <= 0.0 || !width.is_finite() { 1.0 } else { width };
    let boundary = boundary.unwrap_or_else(|| match center {
        Some(c) => c - width / 2.0,
        None => width / 2.0, // tile-layer default: min/max in outer half-bins
    });
    let shift = ((min - boundary) / width).floor();
    let origin = boundary + shift * width;
    let max_x = max + (1.0 - 1e-8) * width;
    let n_breaks = (((max_x - origin) / width).floor() as usize) + 1;
    (0..n_breaks.max(2)).map(|i| origin + i as f64 * width).collect()
}

/// ggplot2 4.x `bin_breaks_bins` + `bin_breaks_width` (refs/ggplot2/R/bin.R).
/// Returns bin CENTRES over `bins` equal-width bins spanning [min, max], with
/// boundary offset so min/max sit in the outer half-bins.
pub fn default_bins(min: f64, max: f64, bins: usize) -> (Vec<f64>, f64) {
    let edges = bin_breaks_bins(min, max, bins, None, None);
    let width = if edges.len() >= 2 { edges[1] - edges[0] } else { 1.0 };
    (edges.windows(2).map(|e| (e[0] + e[1]) / 2.0).collect(), width)
}

/// ggplot2 bin_breaks: index of the bin for v.
/// `closed == "right"`: (a, b] bins (edge → the bin ending at it);
/// `closed == "left"`: [a, b) bins (edge → the bin starting at it).
fn bin_index(v: f64, origin: f64, width: f64, n_bins: usize, closed_right: bool) -> usize {
    let rel = (v - origin) / width;
    let on_edge = (rel - rel.round()).abs() < 1e-9;
    let idx = if on_edge {
        if closed_right {
            rel.round() as i64 - 1
        } else {
            rel.round() as i64
        }
    } else if closed_right {
        rel.floor() as i64
    } else {
        rel.ceil() as i64 - 1
    };
    (idx.max(0) as usize).min(n_bins - 1)
}

/// Stat computation per layer (M1 subset).
pub fn compute_stat(f: &Frame, spec: &StatSpec) -> Frame {
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
        StatSpec::Bin { bins, breaks, binwidth, center, boundary, closed } => {
            let xs = f.get("x").cloned().unwrap_or_default();
            let finite: Vec<f64> = xs.iter().cloned().filter(|v| v.is_finite()).collect();
            let (lo, hi) = match Range::from_values(&finite) {
                Some(r) => (r.min, r.max),
                None => (0.0, 1.0),
            };
            // ggplot2 StatBin$compute_group precedence: explicit `breaks`
            // wins; then `binwidth` (bin_breaks_width) beats `bins`
            // (bin_breaks_bins). center/boundary align the grid either way.
            let edges: Vec<f64> = match breaks {
                Some(b) if b.len() >= 2 => b.clone(),
                _ => match binwidth {
                    Some(w) => bin_breaks_width_edges(lo, hi, *w, *center, *boundary),
                    None => bin_breaks_bins(lo, hi, *bins, *center, *boundary),
                },
            };
            let closed_right = closed.as_deref() != Some("left");
            let origin = edges[0];
            let width = if edges.len() >= 2 { edges[1] - edges[0] } else { 1.0 };
            let centres: Vec<f64> = edges.windows(2).map(|e| (e[0] + e[1]) / 2.0).collect();
            let mut counts = vec![0.0f64; centres.len()];
            let nb = counts.len();
            for &v in &finite {
                if nb > 0 {
                    counts[bin_index(v, origin, width, nb, closed_right)] += 1.0;
                }
            }
            let mut out = Frame::new();
            let xmin: Vec<f64> = edges.windows(2).map(|e| e[0]).collect();
            let xmax: Vec<f64> = edges.windows(2).map(|e| e[1]).collect();
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
        StatSpec::Smooth { method, span, se, n, fullrange: _fullrange } => {
            let xs = f.get("x").cloned().unwrap_or_default();
            let ys = f.get("y").cloned().unwrap_or_default();
            let m = method.as_deref().unwrap_or("loess");
            let nn = n.unwrap_or(80);
            let sp = span.unwrap_or(0.75);
            // group by ALL discrete aes columns (colour/fill/… × discrete x)
            // — ggplot2 groups smooths by every mapped discrete aesthetic.
            let gcols: Vec<&'static str> = ["colour", "fill", "linetype"]
                .into_iter()
                .filter(|k| f.cat.contains_key(*k))
                .collect();
            let xcats: Option<Vec<String>> = f.cat.get("x").cloned();
            let discrete_x = xcats.is_some();
            let mut keys: Vec<(Vec<String>, Option<f64>)> = Vec::new();
            for i in 0..xs.len() {
                if !(ys[i].is_finite()) {
                    continue;
                }
                let sig: Vec<String> = gcols
                    .iter()
                    .map(|k| f.cat.get(*k).and_then(|v| v.get(i)).cloned().unwrap_or_default())
                    .collect();
                let ordx = if discrete_x { Some(xs[i]) } else { None };
                if !keys.iter().any(|(s, o)| *s == sig && *o == ordx) {
                    keys.push((sig, ordx));
                }
            }
            let mut out = Frame::new();
            let mut oy: Vec<f64> = Vec::new();
            let mut ox: Vec<f64> = Vec::new();
            let mut oymin: Vec<f64> = Vec::new();
            let mut oymax: Vec<f64> = Vec::new();
            let mut ocats: Vec<Vec<String>> = vec![Vec::new(); gcols.len()];
            let mut oxcats: Vec<String> = Vec::new();
            for (sig, ordx) in &keys {
                let pts: Vec<(f64, f64)> = (0..xs.len())
                    .filter(|&i| {
                        ys[i].is_finite()
                            && gcols.iter().enumerate().all(|(j, k)| {
                                f.cat.get(*k).and_then(|v| v.get(i)).cloned() == Some(sig[j].clone())
                            })
                            && ordx.map_or(true, |o| (xs[i] - o).abs() < 1e-9)
                    })
                    .map(|i| (xs[i], ys[i]))
                    .collect();
                    if pts.len() < 3 {
                        continue;
                    }
                let (gx, gy, ylo, yhi) = if let Some(o) = ordx {
                    // discrete x group: fit on index positions, grid over slot
                    let pp: Vec<(f64, f64)> = pts
                        .iter()
                        .enumerate()
                        .map(|(k, &(_, yv))| (k as f64, yv)) // x positions are the ordinal slot; all pts share it → use 0..n index for spread? ggplot2 fits y~index-in-group; we use 0..n
                        .collect();
                    let _ = pp;
                    // For discrete x, ggplot2's lm fits y ~ numeric index of x
                    // within the group (all equal) → degenerate; it actually
                    // fits y ~ x where x is the ordinal — a vertical line
                    // impossible; instead ggplot2 fits per group on x=index in
                    // the DATA (only one x value!) → skips. Reproduce: skip
                    // discrete-x smooths (they warn/behave as constants).
                    // Our reference 56 maps colour groups on CONTINUOUS x, so
                    // discrete x only arises for smooth on factor x: rare.
                    // Emit nothing to stay safe.
                    continue;
                } else {
                    let lo = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                    let hi = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
                    let gx = linspace(lo, hi, nn);
                    let (gy, ylo, yhi) = smooth_fit(m, &pts, &gx, sp, *se);
                    (gx, gy, ylo, yhi)
                };
                let nnn = gx.len();
                ox.extend(gx);
                oy.extend(gy);
                if *se {
                    oymin.extend(ylo.unwrap_or_else(|| vec![f64::NAN; nnn]));
                    oymax.extend(yhi.unwrap_or_else(|| vec![f64::NAN; nnn]));
                }
                for (j, v) in ocats.iter_mut().enumerate() {
                    v.extend(std::iter::repeat(sig[j].clone()).take(nnn));
                }
                if discrete_x {
                    let lab = (0..xs.len())
                        .find(|&i| {
                            ordx.map_or(false, |o| (xs[i] - o).abs() < 1e-9)
                                && f.cat.get("x").is_some()
                        })
                        .and_then(|i| xcats.as_ref().unwrap().get(i).cloned())
                        .unwrap_or_default();
                    oxcats.extend(std::iter::repeat(lab).take(nnn));
                }
            }
            out.set("x", ox);
            out.set("y", oy);
            if *se {
                out.set("ymin", oymin);
                out.set("ymax", oymax);
            }
            for (j, k) in gcols.iter().enumerate() {
                if !ocats[j].is_empty() {
                    out.set_cat(k, ocats[j].clone());
                }
            }
            if discrete_x && !oxcats.is_empty() {
                out.set_cat("x", oxcats.clone());
                for k in ["x"] {
                    if let Some(decl) = f.levels.get(k) {
                        out.set_levels(k, decl.clone());
                    }
                }
            }
            for (k, v) in &f.levels {
                if out.cat.contains_key(k.as_str()) && !out.levels.contains_key(k.as_str()) {
                    out.set_levels(k, v.clone());
                }
            }
            out
        }
        StatSpec::Density { bw, adjust, n, trim: _trim } => {
            let xs = f.get("x").cloned().unwrap_or_default();
            let x: Vec<f64> = xs.into_iter().filter(|v| v.is_finite()).collect();
            if x.len() < 2 {
                return Frame::new();
            }
            let mut b = bw.unwrap_or_else(|| bw_nrd0(&x));
            b *= adjust.unwrap_or(1.0);
            if b <= 0.0 || !b.is_finite() {
                b = 1.0;
            }
            // ggplot2 4.0.3 layer_data: the 512-pt grid spans the DATA range
            // (min..max); the ±3·bw padding lives inside the estimate, not the
            // output grid. R's `trim` additionally drops nothing (same range).
            let (gx, gy) = gaussian_density(&x, b, n.unwrap_or(512), true);
            let mut out = Frame::new();
            out.set("x", gx);
            out.set("y", gy);
            out
        }
        StatSpec::Ydensity { bw, adjust, n, trim: _trim, scale } => {
            let xs = f.get("x").cloned().unwrap_or_default();
            let ys = f.get("y").cloned().unwrap_or_default();
            // factor labels keyed by the ordinal x values (stat runs BEFORE
            // ordinalise; labels must survive, like StatBoxplot's set_cat)
            let xcats: Option<Vec<String>> = f.cat.get("x").cloned();
            let mut out = Frame::new();
            // group by discrete x values (1-based ordinal already mapped)
            let mut uniq: Vec<f64> = xs.iter().cloned().filter(|v| v.is_finite()).collect();
            uniq.sort_by(|a, b| a.partial_cmp(b).unwrap());
            uniq.dedup();
            let mut per_group: Vec<(f64, Vec<f64>, Vec<f64>, usize)> = Vec::new();
            for &ux in &uniq {
                let vals: Vec<f64> = (0..xs.len())
                    .filter(|&i| (xs[i] - ux).abs() < 1e-9 && ys[i].is_finite())
                    .map(|i| ys[i])
                    .collect();
                if vals.len() < 2 {
                    continue;
                }
                let mut b = bw.unwrap_or_else(|| bw_nrd0(&vals));
                b *= adjust.unwrap_or(1.0);
                if b <= 0.0 || !b.is_finite() {
                    b = 1.0;
                }
                let (gx, gy) = gaussian_density(&vals, b, n.unwrap_or(512), true);
                // keep only points within the group's data range (shell clipped
                // to min..max, matching ggplot2 4.x geom_violin output)
                let (vmin, vmax) = vals.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, &v| (a.0.min(v), a.1.max(v)));
                let gx2: Vec<f64> = gx.iter().cloned().filter(|y| *y >= vmin && *y <= vmax).collect();
                let gy2: Vec<f64> = gx
                    .iter()
                    .zip(gy.iter())
                    .filter(|(y, _)| **y >= vmin && **y <= vmax)
                    .map(|(_, d)| *d)
                    .collect();
                per_group.push((ux, gx2, gy2, vals.len()));
            }
            // "area" (default): violinwidth = density / global max density;
            // "count": also × n/max(n); "width": per-group scaled to 1.
            let gmax = per_group
                .iter()
                .flat_map(|(_, _, gy, _)| gy.iter().cloned())
                .fold(0.0f64, f64::max);
            let nmax = per_group.iter().map(|(_, _, _, nn)| *nn).max().unwrap_or(1) as f64;
            let mode = scale.as_deref().unwrap_or("area");
            let mut xo: Vec<f64> = Vec::new();
            let mut yo: Vec<f64> = Vec::new();
            let mut vw: Vec<f64> = Vec::new();
            let mut gn: Vec<f64> = Vec::new();
            let mut wid: Vec<f64> = Vec::new();
            let mut xlab: Vec<String> = Vec::new();
            for (ux, gx, gy, nn) in &per_group {
                let nnf = *nn as f64;
                for (j, &yv) in gy.iter().enumerate() {
                    xo.push(*ux);
                    if let Some(cats) = &xcats {
                        // label for this ordinal = the label on ANY input row
                        // whose ordinal x matches (row order ≠ level order!)
                        let lab = (0..xs.len())
                            .find(|&i| (xs[i] - ux).abs() < 1e-9)
                            .and_then(|i| cats.get(i).cloned());
                        xlab.push(lab.unwrap_or_default());
                    }
                    yo.push(gx[j]);
                    let w = match mode {
                        "count" => yv / gmax * (nnf / nmax),
                        "width" => yv / gy.iter().cloned().fold(0.0f64, f64::max),
                        _ => yv / gmax,
                    };
                    vw.push(w);
                    gn.push(nnf);
                    wid.push(0.9);
                }
            }
            out.set("x", xo);
            out.set("y", yo);
            out.set("violinwidth", vw);
            out.set("n", gn);
            out.set("width", wid);
            if !xlab.is_empty() {
                out.set_cat("x", xlab);
                if let Some(decl) = f.levels.get("x") {
                    out.set_levels("x", decl.clone());
                }
            }
            out
        }
        StatSpec::Boxplot { coef } => {
            let coef = coef.unwrap_or(1.5);
            let _xs = f.get("x").cloned().unwrap_or_default();
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
