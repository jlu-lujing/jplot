use serde::{Deserialize, Serialize};

/// A column of data: numeric, categorical (string) or logical-as-category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Column {
    Numeric {
        values: Vec<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Categorical {
        values: Vec<String>,
        /// Fixed level order; if None, order of first appearance.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        levels: Option<Vec<String>>,
    },
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::Numeric { values, .. } => values.len(),
            Column::Categorical { values, .. } => values.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn numeric(&self) -> Option<&[f64]> {
        match self {
            Column::Numeric { values, .. } => Some(values),
            _ => None,
        }
    }
    pub fn categorical(&self) -> Option<&[String]> {
        match self {
            Column::Categorical { values, .. } => Some(values),
            _ => None,
        }
    }
    pub fn is_numeric(&self) -> bool {
        matches!(self, Column::Numeric { .. })
    }
    pub fn unique_levels(&self) -> Vec<String> {
        match self {
            Column::Numeric { .. } => Vec::new(),
            Column::Categorical { levels, .. } => levels.clone().unwrap_or_else(|| {
                let mut seen: Vec<String> = Vec::new();
                for v in self.categorical().unwrap() {
                    if !seen.iter().any(|s| s == v) {
                        seen.push(v.clone());
                    }
                }
                seen
            }),
        }
    }
}

/// Column-oriented dataset, ggplot2 `data` frame equivalent.
///
/// Serde supports two shapes: the canonical `{"columns": [[name, col], ...]}`
/// and the friendlier map `{"x": {...}, "y": {...}}` (used by R/Python
/// generators — JSON objects preserve key order per RFC 8259 / serde_json).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dataset {
    pub columns: Vec<(String, Column)>,
}

impl<'de> Deserialize<'de> for Dataset {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{MapAccess, SeqAccess, Visitor};
        struct DS;
        impl<'de> Visitor<'de> for DS {
            type Value = Dataset;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("dataset as map or {columns: [[name, col]]}")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Dataset, A::Error> {
                let mut cols: Vec<(String, Column)> = Vec::new();
                let mut first = true;
                while let Some(k) = m.next_key::<String>()? {
                    if k == "columns" && first {
                        // canonical form
                        let pairs: Vec<(String, Column)> = m.next_value()?;
                        return Ok(Dataset { columns: pairs });
                    }
                    first = false;
                    let col: Column = m.next_value()?;
                    cols.push((k, col));
                }
                Ok(Dataset { columns: cols })
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut s: A) -> Result<Dataset, A::Error> {
                // [[name, col], ...]
                let mut cols: Vec<(String, Column)> = Vec::new();
                while let Some(pair) = s.next_element::<(String, Column)>()? {
                    cols.push(pair);
                }
                Ok(Dataset { columns: cols })
            }
        }
        d.deserialize_any(DS)
    }
}

impl Serialize for Dataset {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.columns.len()))?;
        for (k, c) in &self.columns {
            m.serialize_entry(k, c)?;
        }
        m.end()
    }
}

impl Dataset {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, name: impl Into<String>, col: Column) -> &mut Self {
        self.columns.push((name.into(), col));
        self
    }
    pub fn get(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|(n, _)| n == name).map(|(_, c)| c)
    }
    pub fn nrow(&self) -> usize {
        self.columns.first().map_or(0, |(_, c)| c.len())
    }
    /// Numeric values of a column; categorical columns are ordinalised by level order (1-based, ggplot2 style).
    pub fn numeric_of(&self, name: &str) -> Option<Vec<f64>> {
        match self.get(name)? {
            Column::Numeric { values, .. } => Some(values.clone()),
            Column::Categorical { .. } => {
                let levels = self.get(name).unwrap().unique_levels();
                let vals: Vec<String> = self.get(name).unwrap().categorical().unwrap().to_vec();
                Some(
                    vals.iter()
                        .map(|v| {
                            levels
                                .iter()
                                .position(|l| l == v)
                                .map_or(f64::NAN, |i| (i + 1) as f64)
                        })
                        .collect(),
                )
            }
        }
    }
}

/// A numeric range (min, max) with sensible expansion.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

impl Range {
    pub fn from_values(vals: &[f64]) -> Option<Range> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for &v in vals {
            if v.is_finite() {
                min = min.min(v);
                max = max.max(v);
            }
        }
        if min.is_finite() {
            Some(Range { min, max })
        } else {
            None
        }
    }
    pub fn expand(&mut self, mult: f64) {
        if self.min == self.max {
            let half = if self.min == 0.0 { 0.5 } else { self.min.abs() * mult };
            self.min -= half;
            self.max += half;
        } else {
            let d = (self.max - self.min) * mult;
            self.min -= d;
            self.max += d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(v: Vec<f64>) -> Column {
        Column::Numeric { values: v, label: None }
    }
    fn cat(v: Vec<&str>) -> Column {
        Column::Categorical { values: v.iter().map(|s| s.to_string()).collect(), levels: None }
    }

    #[test]
    fn ordinalises_categorical_1based() {
        let mut ds = Dataset::new();
        ds.add("g", cat(vec!["b", "a", "b", "c"]));
        // levels in order of first appearance: b,a,c
        assert_eq!(ds.numeric_of("g").unwrap(), vec![1.0, 2.0, 1.0, 3.0]);
    }

    #[test]
    fn range_expansion() {
        let mut r = Range::from_values(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!((r.min, r.max), (1.0, 3.0));
        r.expand(0.05);
        assert!((r.min - 0.9).abs() < 1e-9 && (r.max - 3.1).abs() < 1e-9);
    }

    #[test]
    fn range_from_empty_or_all_nan_is_none() {
        assert!(Range::from_values(&[]).is_none());
        assert!(Range::from_values(&[f64::NAN]).is_none());
    }

    #[test]
    fn serde_roundtrip() {
        let mut ds = Dataset::new();
        ds.add("x", num(vec![1.0, 2.0]));
        let s = serde_json::to_string(&ds).unwrap();
        let ds2: Dataset = serde_json::from_str(&s).unwrap();
        assert_eq!(ds, ds2);
    }
}
