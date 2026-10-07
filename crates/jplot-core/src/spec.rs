//! Plot specification & typed builder API (`ggplot(...) + aes(...) + geom_*`).
//! Everything serialises to JSON (`PlotSpec`) so Python/R/CLI share one wire
//! format with the Rust-native builder.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::data::Dataset;
use crate::scale::ScaleSpec;

/// Aesthetic channels supported in v1.
pub type AesKey = String;

/// Mapping of aesthetics to data columns (string) — ggplot2 `aes()`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AesSpec {
    /// aesthetic name -> column name, e.g. {"x": "mpg", "colour": "cyl"}
    #[serde(default, deserialize_with = "crate::serde_util::map_or_empty")]
    pub map: HashMap<AesKey, String>,
}

impl AesSpec {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(mut self, key: impl Into<AesKey>, col: impl Into<String>) -> Self {
        self.map.insert(key.into(), col.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeomSpec {
    Point {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shape: Option<f64>,
    },
    Line,
    /// geom_col: bars whose height is the raw y value
    Col,
    /// geom_bar: counts (stat = count by default)
    Bar,
    /// geom_histogram: binning
    Histogram {
        bins: usize,
    },
    /// geom_boxplot
    Boxplot,
    /// geom_freqpoly: histogram counts drawn as a line over bin centres
    Freqpoly {
        bins: usize,
    },
    /// geom_step: piecewise-constant line (hist / up / down)
    Step,
    /// geom_hline / geom_vline: constant lines (intercept from aes or param)
    Hline,
    Vline,
    /// geom_text: labels at (x, y)
    Text,
    /// geom_area: filled area to y=0 (stacks by default)
    Area,
    /// geom_errorbar: vertical line + caps at each x between ymin..ymax
    Errorbar,
    /// geom_ribbon: filled band between ymin..ymax per group
    Ribbon,
    /// geom_segment: x,y -> xend,yend
    Segment,
    /// geom_path: line in data order (no x-sort)
    Path,
    /// geom_rect: xmin/xmax/ymin/ymax boxes
    Rect,
    /// geom_tile: centred rectangles (width/height from spacing)
    Tile,
    /// geom_linerange: vertical segment ymin..ymax (no caps)
    Linerange,
    /// geom_pointrange: linerange + point at y
    Pointrange,
    /// geom_crossbar: box ymin..ymax + median line + caps
    Crossbar,
    /// geom_errorbarh: horizontal errorbar (x = value, xmin..xmax at y)
    Errorbarh,
    /// geom_abline: y = slope*x + intercept
    Abline,
    /// geom_point with position jitter
    Jitter,
    /// geom_label: text with background box
    Label,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatSpec {
    Identity,
    /// count per group/bin of x
    Count {
        /// width of the bin on the discrete x scale (ggplot2 default 1 → .9 for bar)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<f64>,
    },
    Bin {
        bins: usize,
        /// optional explicit bin edges
        #[serde(default, skip_serializing_if = "Option::is_none")]
        breaks: Option<Vec<f64>>,
        /// explicit bin width (overrides bins-derived width)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        binwidth: Option<f64>,
        /// bin centres must sit at center + k*width
        #[serde(default, skip_serializing_if = "Option::is_none")]
        center: Option<f64>,
        /// bin edges must pass through boundary
        #[serde(default, skip_serializing_if = "Option::is_none")]
        boundary: Option<f64>,
        /// "right" (default) | "left"
        #[serde(default, skip_serializing_if = "Option::is_none")]
        closed: Option<String>,
    },
    /// boxplot summary: quantile(0.25), median, quantile(0.75), whiskers, outliers
    Boxplot {
        /// whisker multiplier of IQR (ggplot2 default 1.5)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        coef: Option<f64>,
    },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PositionSpec {
    #[default]
    Identity,
    Dodge {
        width: f64,
    },
    /// ggplot2 position_stack / "stack": bars at the same x accumulate.
    Stack,
    Jitter {
        width: f64,
        height: f64,
        seed: u64,
    },
}

/// Aesthetic values passed to a geom as `...` (`geom_point(colour = "red")`,
/// `geom_boxplot(outlier.colour = "red", notch = TRUE)`).
///
/// All ggplot2 `...` names are captured generically (R jsonlite serialises
/// the empty list as `[]`, tolerated via the helper); typed accessors expose
/// colour/size/bool params. The complete knob vocabulary is machine-extracted
/// into `api_inventory.json` — docs/API_PARITY.md tracks per-geom coverage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeomArgs {
    /// all ggplot2 `...` params: name -> value (hex strings, numbers,
    /// "TRUE"/"FALSE" as R encodes them)
    pub map: HashMap<String, serde_json::Value>,
}

impl Serialize for GeomArgs {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.map.len()))?;
        for (k, v) in &self.map {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// `[]` (empty R list) or `{...}` object.
#[derive(Deserialize)]
#[serde(untagged)]
enum GeomArgsHelper {
    Seq(Vec<serde_json::Value>),
    Map(HashMap<String, serde_json::Value>),
}

impl<'de> Deserialize<'de> for GeomArgs {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match GeomArgsHelper::deserialize(d)? {
            GeomArgsHelper::Seq(_) => GeomArgs::default(),
            GeomArgsHelper::Map(m) => GeomArgs { map: m },
        })
    }
}

impl GeomArgs {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(mut self, k: impl Into<String>, v: serde_json::Value) -> Self {
        self.map.insert(k.into(), v);
        self
    }
    /// string param (colour hex, linetype name, …)
    pub fn s(&self, key: &str) -> Option<&str> {
        self.map.get(key).and_then(|v| v.as_str())
    }
    pub fn f64_(&self, key: &str) -> Option<f64> {
        self.map.get(key).and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.parse().ok(),
            serde_json::Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        })
    }
    pub fn bool_(&self, key: &str) -> Option<bool> {
        self.map.get(key).and_then(|v| match v {
            serde_json::Value::Bool(b) => Some(*b),
            serde_json::Value::String(s) => match s.as_str() {
                "TRUE" | "true" => Some(true),
                "FALSE" | "false" => Some(false),
                _ => None,
            },
            serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0),
            _ => None,
        })
    }
    /// first present colour param among keys (ggplot2 colour/color aliases)
    pub fn colour_(&self, keys: &[&str]) -> Option<crate::scale::Color> {
        keys.iter()
            .find_map(|k| self.s(k))
            .and_then(crate::scale::Color::parse)
    }
    /// numeric vector param (e.g. histogram `breaks`, `limits`) — a JSON
    /// array of numbers, or a single number. R exports `[]`/single scalars.
    pub fn nums(&self, key: &str) -> Option<Vec<f64>> {
        match self.map.get(key)? {
            serde_json::Value::Array(a) => {
                let v: Vec<f64> =
                    a.iter().filter_map(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).collect();
                if v.is_empty() {
                    None
                } else {
                    Some(v)
                }
            }
            serde_json::Value::Number(n) => n.as_f64().map(|f| vec![f]),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSpec {
    pub geom: GeomSpec,
    #[serde(default)]
    pub stat: Option<StatSpec>,
    #[serde(default)]
    pub position: PositionSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Dataset>,
    #[serde(default)]
    pub mapping: AesSpec,
    #[serde(default)]
    pub args: GeomArgs,
    /// inherit aes from plot level (default true)
    #[serde(default = "default_true")]
    pub inherit_aes: bool,
}

fn default_true() -> bool {
    true
}

impl Default for LayerSpec {
    fn default() -> Self {
        LayerSpec {
            geom: GeomSpec::Point { shape: None },
            stat: None,
            position: PositionSpec::Identity,
            data: None,
            mapping: AesSpec::default(),
            args: GeomArgs::default(),
            inherit_aes: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LabelsSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<String>,
    /// legend titles by aesthetic
    #[serde(default, deserialize_with = "crate::serde_util::map_or_empty", skip_serializing_if = "HashMap::is_empty")]
    pub guides: HashMap<AesKey, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThemeSpec {
    Grey,
    Bw,
    Minimal,
    Classic,
}

impl Default for ThemeSpec {
    fn default() -> Self {
        ThemeSpec::Grey
    }
}

/// Scales keyed by aesthetic (`x`, `y`, `colour`, `fill`, ...).
pub type ScalesSpec = HashMap<AesKey, ScaleSpec>;

/// HashMap alias tolerant to `[]`.
fn scales_spec_de<'de, D: serde::Deserializer<'de>>(d: D) -> Result<ScalesSpec, D::Error> {
    crate::serde_util::map_or_empty(d)
}

/// The complete plot: what `ggplot2::ggplotGrob` consumes, serialised.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotSpec {
    #[serde(default)]
    pub data: Dataset,
    #[serde(default)]
    pub mapping: AesSpec,
    #[serde(default)]
    pub layers: Vec<LayerSpec>,
    #[serde(default, deserialize_with = "scales_spec_de")]
    pub scales: ScalesSpec,
    #[serde(default)]
    pub labels: LabelsSpec,
    #[serde(default)]
    pub theme: ThemeSpec,
    #[serde(default = "default_width")]
    pub width: f64,
    #[serde(default = "default_height")]
    pub height: f64,
}

fn default_width() -> f64 {
    720.0
}
fn default_height() -> f64 {
    480.0
}

impl Default for PlotSpec {
    fn default() -> Self {
        PlotSpec {
            data: Dataset::default(),
            mapping: AesSpec::default(),
            layers: Vec::new(),
            scales: ScalesSpec::default(),
            labels: LabelsSpec::default(),
            theme: ThemeSpec::default(),
            width: default_width(),
            height: default_height(),
        }
    }
}

// ---------------------------------------------------------------------------
// Typed builder API
// ---------------------------------------------------------------------------

/// Builder accumulating via `+`, ggplot2-style.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plot {
    pub spec: PlotSpec,
}

pub fn ggplot(data: Dataset) -> Plot {
    Plot {
        spec: PlotSpec {
            data,
            ..Default::default()
        },
    }
}

pub fn aes(map: HashMap<AesKey, String>) -> AesSpec {
    AesSpec { map }
}

pub trait AddLayer {
    fn add(self, p: Plot) -> Plot;
}

impl<F: FnOnce(Plot) -> Plot> AddLayer for F {
    fn add(self, p: Plot) -> Plot {
        self(p)
    }
}

impl std::ops::Add<Plot> for Plot {
    type Output = Plot;
    fn add(mut self, rhs: Plot) -> Plot {
        self.spec.layers.extend(rhs.spec.layers);
        self
    }
}

/// `p + geom_point()` — layer constructors are `impl Into<LayerSpec>` closures.
impl std::ops::Add<LayerSpec> for Plot {
    type Output = Plot;
    fn add(mut self, layer: LayerSpec) -> Plot {
        self.spec.layers.push(layer);
        self
    }
}

impl std::ops::Add<AesSpec> for Plot {
    type Output = Plot;
    fn add(mut self, a: AesSpec) -> Plot {
        for (k, v) in a.map {
            if self.spec.mapping.map.contains_key(&k) && !self.spec.mapping.map.is_empty() {
                // keep plot-level; sub-layer aes wins at build time
            }
            self.spec.mapping.map.insert(k, v);
        }
        self
    }
}

impl std::ops::Add<ThemeSpec> for Plot {
    type Output = Plot;
    fn add(mut self, t: ThemeSpec) -> Plot {
        self.spec.theme = t;
        self
    }
}

impl std::ops::Add<ScalesSpec> for Plot {
    type Output = Plot;
    fn add(mut self, s: ScalesSpec) -> Plot {
        self.spec.scales.extend(s);
        self
    }
}

impl std::ops::Add<LabelsSpec> for Plot {
    type Output = Plot;
    fn add(mut self, l: LabelsSpec) -> Plot {
        if l.title.is_some() {
            self.spec.labels.title = l.title;
        }
        if l.x.is_some() {
            self.spec.labels.x = l.x;
        }
        if l.y.is_some() {
            self.spec.labels.y = l.y;
        }
        for (k, v) in l.guides {
            self.spec.labels.guides.insert(k, v);
        }
        self
    }
}

// convenience geom/layer helpers ------------------------------------------------

pub fn geom_point() -> LayerSpec {
    LayerSpec {
        geom: GeomSpec::Point { shape: None },
        stat: Some(StatSpec::Identity),
        ..Default::default()
    }
}

pub fn geom_line() -> LayerSpec {
    LayerSpec {
        geom: GeomSpec::Line,
        stat: Some(StatSpec::Identity),
        ..Default::default()
    }
}

pub fn geom_col() -> LayerSpec {
    LayerSpec {
        geom: GeomSpec::Col,
        stat: Some(StatSpec::Identity),
        ..Default::default()
    }
}

pub fn geom_bar() -> LayerSpec {
    LayerSpec {
        geom: GeomSpec::Bar,
        stat: Some(StatSpec::Count { width: None }),
        ..Default::default()
    }
}

pub fn geom_histogram(bins: usize) -> LayerSpec {
    LayerSpec {
        geom: GeomSpec::Histogram { bins },
        // stat derived at build time from geom + args (binwidth/center/breaks)
        stat: None,
        ..Default::default()
    }
}

pub fn geom_boxplot() -> LayerSpec {
    LayerSpec {
        geom: GeomSpec::Boxplot,
        stat: Some(StatSpec::Boxplot { coef: None }),
        position: PositionSpec::Dodge { width: 0.75 },
        ..Default::default()
    }
}

pub trait PlotRender {
    fn render_svg(&self) -> Result<String, crate::error::JplotError>;
    fn render_png(&self) -> Result<Vec<u8>, crate::error::JplotError>;
}

impl Plot {
    pub fn spec(&self) -> &PlotSpec {
        &self.spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Column;
    use std::collections::HashMap;

    #[test]
    fn builder_plus_serialises() {
        let mut d = Dataset::new();
        d.add(
            "x",
            Column::Numeric { values: vec![1.0, 2.0, 3.0], label: None },
        );
        let mut m = HashMap::new();
        m.insert("x".to_string(), "x".to_string());
        m.insert("y".to_string(), "x".to_string());
        let p = ggplot(d) + aes(m) + geom_point();
        let j = serde_json::to_string(&p.spec).unwrap();
        let p2: PlotSpec = serde_json::from_str(&j).unwrap();
        assert_eq!(p.spec.layers.len(), 1);
        assert_eq!(p2, p.spec);
    }
}
