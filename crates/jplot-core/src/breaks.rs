//! Axis-tick math: Wilkinson-style `extended_breaks` (ported from R
//! `labeling::extended()`, constants verified against refs/scales) and
//! `format_breaks` (scales::label_number semantics: common decimals, space
//! thousands separator, scientific outside [1e-4, 1e6)).


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

    let (dmin, dmax) = (dmin.min(dmax), dmin.max(dmax));

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
                break 'outer; // j <- Inf in R: loop ends immediately
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

