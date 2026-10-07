//! Grouping & position adjustments: ggplot2-style group identification and
//! the position family (dodge / stack / jitter).

use std::collections::HashMap;

use crate::spec::PositionSpec;

use crate::build::Frame;


/// Order discrete levels: declared factor levels first, else ggplot2 sort
/// (numeric-looking levels by numeric value, otherwise lexicographic).
pub fn order_levels(levels: &mut Vec<String>, declared: Option<&Vec<String>>) {
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
pub fn group_cols(f: &Frame) -> Vec<&'static str> {
    ["colour", "fill", "linetype"]
        .into_iter()
        .filter(|k| f.cat.contains_key(*k))
        .collect()
}

/// Group ids ordered by the trained discrete scale levels (ggplot2 orders
/// dodge slots by scale level, NOT first appearance in the data).
pub fn group_ids(
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

pub fn apply_position(
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
        PositionSpec::Stack => {
            if f.get("y").is_none() {
                return;
            }
            // pos_stack: for rows sharing an x, order by group and accumulate
            // y from 0 (ggplot2: y = cumsum(count) within each x).
            let xs = f.get("x").cloned().unwrap_or_default();
            let ys = f.get("y").cloned().unwrap_or_default();
            let groups = group_cols(f);
            let gids = group_ids(f, &groups, level_order);
            let mut new_ys = ys.clone();
            let mut new_ymin = vec![0.0; ys.len()];
            // process each distinct x independently
            let mut seen: Vec<f64> = Vec::new();
            for i in 0..xs.len() {
                if seen.contains(&xs[i]) || !xs[i].is_finite() {
                    continue;
                }
                seen.push(xs[i]);
                let mut idx: Vec<usize> = (0..xs.len()).filter(|&j| (xs[j] - xs[i]).abs() < 1e-9).collect();
                // ggplot2 pos_stack accumulates by DESCENDING group: the
                // highest group id ends up on the bottom of the stack.
                idx.sort_by(|&a, &b| gids[b].cmp(&gids[a]));
                let mut cum = 0.0;
                for &j in &idx {
                    new_ymin[j] = cum;
                    cum += ys[j];
                    new_ys[j] = cum; // top = cumulative sum
                }
            }
            f.set("y", new_ys);
            f.set("ymin", new_ymin);
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
