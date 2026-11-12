//! Serializable workbook model exchanged between the host (XLSX I/O) and the
//! engine (WASM in the webview, native on save).

use serde::{Deserialize, Serialize};

use crate::addr::{CellRef, Rect};
use crate::value::{ErrorKind, Value};
use crate::workbook::{Cell, DefinedName, Input, Sheet, TableDef, Workbook};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum CellValueModel {
    Bool(bool),
    Number(f64),
    Text(String),
    Error { e: String },
}

// Hand-written: `#[serde(untagged)]` tries each variant and builds an error
// per miss, which dominated loading large workbooks.
impl<'de> Deserialize<'de> for CellValueModel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = CellValueModel;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a number, string, boolean or {\"e\": code}")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(CellValueModel::Bool(v))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
                Ok(CellValueModel::Number(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                Ok(CellValueModel::Number(v as f64))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                Ok(CellValueModel::Number(v as f64))
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(CellValueModel::Text(v.to_string()))
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(CellValueModel::Text(v))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut m: A,
            ) -> Result<Self::Value, A::Error> {
                let mut e = None;
                while let Some((k, v)) = m.next_entry::<String, serde_json::Value>()? {
                    if k == "e" {
                        e = v.as_str().map(String::from);
                    }
                }
                e.map(|e| CellValueModel::Error { e })
                    .ok_or_else(|| serde::de::Error::custom("expected {\"e\": code}"))
            }
        }
        d.deserialize_any(V)
    }
}

impl CellValueModel {
    pub fn from_value(v: &Value) -> Option<Self> {
        Some(match v {
            Value::Number(n) => Self::Number(*n),
            Value::Bool(b) => Self::Bool(*b),
            Value::Text(t) => Self::Text(t.to_string()),
            Value::Error(e) => Self::Error { e: e.code().into() },
            Value::Array(a) => return a.data.first().and_then(Self::from_value),
            _ => return None,
        })
    }

    pub fn to_value(&self) -> Value {
        match self {
            Self::Number(n) => Value::Number(*n),
            Self::Bool(b) => Value::Bool(*b),
            Self::Text(t) => Value::text(t.clone()),
            Self::Error { e } => Value::Error(ErrorKind::parse(e).unwrap_or(ErrorKind::Value)),
        }
    }
}

fn no(b: &bool) -> bool {
    !*b
}

/// Visual cell formatting (one per cell XF, parallel to `num_formats`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CellStyle {
    #[serde(default, skip_serializing_if = "no")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "no")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "no")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "no")]
    pub strike: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_name: Option<String>,
    /// `#RRGGBB`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// left | center | right | justify | fill | general
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_align: Option<String>,
    /// top | center | bottom
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_align: Option<String>,
    #[serde(default, skip_serializing_if = "no")]
    pub wrap: bool,
    /// Border bitmask: 1 left, 2 right, 4 top, 8 bottom.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub border: u8,
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

impl CellStyle {
    pub fn has_font(&self) -> bool {
        self.bold
            || self.italic
            || self.underline
            || self.strike
            || self.font_size.is_some()
            || self.font_name.is_some()
            || self.color.is_some()
    }
}

/// An embedded chart (DrawingML `c:chartSpace`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDef {
    /// column | bar | line | area | pie | scatter
    pub kind: String,
    #[serde(default)]
    pub title: String,
    /// Source data: first row = series names, first column = categories.
    pub data: Rect,
    /// Cells the chart covers (top-left … bottom-right).
    pub anchor: Rect,
    /// Read from the file: its parts are kept as-is on save.
    #[serde(default, skip_serializing_if = "no")]
    pub from_file: bool,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellModel {
    pub r: u32,
    pub c: u32,
    /// Formula text without the leading `=`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f: Option<String>,
    /// Literal value, or the cached result of a formula.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<CellValueModel>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub s: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SheetModel {
    pub name: String,
    #[serde(default)]
    pub cells: Vec<CellModel>,
    #[serde(default)]
    pub col_widths: Vec<(u32, f64)>,
    #[serde(default)]
    pub row_heights: Vec<(u32, f64)>,
    #[serde(default)]
    pub merges: Vec<Rect>,
    #[serde(default)]
    pub frozen: (u32, u32),
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub tables: Vec<TableDef>,
    #[serde(default)]
    pub charts: Vec<ChartDef>,
    /// Index of the worksheet in the source file this sheet came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookModel {
    pub sheets: Vec<SheetModel>,
    #[serde(default)]
    pub names: Vec<DefinedName>,
    /// Number-format code per style (cell XF) index.
    #[serde(default)]
    pub num_formats: Vec<String>,
    /// Visual style per style index (may be shorter than `num_formats`).
    #[serde(default)]
    pub styles: Vec<CellStyle>,
    #[serde(default)]
    pub date1904: bool,
}

impl WorkbookModel {
    pub fn blank() -> Self {
        Self {
            sheets: vec![SheetModel {
                name: "Sheet1".into(),
                ..Default::default()
            }],
            num_formats: vec!["General".into()],
            ..Default::default()
        }
    }
}

impl Workbook {
    /// Build an engine workbook from a model (marks all formulas dirty; call `recalc`).
    pub fn from_model(m: &WorkbookModel) -> Self {
        let mut wb = Self::from_model_cells(m);
        wb.mark_all_dirty();
        wb
    }

    /// Cells and sheets only (no dependency graph yet).
    pub fn from_model_cells(m: &WorkbookModel) -> Self {
        let mut wb = Workbook {
            num_formats: if m.num_formats.is_empty() {
                vec!["General".into()]
            } else {
                m.num_formats.clone()
            },
            date1904: m.date1904,
            names: m.names.clone(),
            styles: m.styles.clone(),
            ..Default::default()
        };
        for sm in &m.sheets {
            let mut s = Sheet::new(sm.name.clone());
            s.col_widths = sm.col_widths.iter().copied().collect();
            s.row_heights = sm.row_heights.iter().copied().collect();
            s.merges = sm.merges.clone();
            s.frozen = sm.frozen;
            s.hidden = sm.hidden;
            s.tables = sm.tables.clone();
            s.charts = sm.charts.clone();
            s.source = sm.source;
            wb.sheets.push(s);
        }
        if wb.sheets.is_empty() {
            wb.sheets.push(Sheet::new("Sheet1"));
        }
        wb.reparse_names();
        // filled-down formulas share one parsed tree
        let mut shared: crate::fxhash::FxHashMap<
            String,
            (std::sync::Arc<crate::parser::Expr>, CellRef),
        > = Default::default();
        for (si, sm) in m.sheets.iter().enumerate() {
            for c in &sm.cells {
                let at = CellRef::new(c.r, c.c);
                let cached = c.v.as_ref().map(|v| v.to_value()).unwrap_or_default();
                let input = match &c.f {
                    Some(f) => {
                        let key =
                            crate::parser::relative_key(f, at).map(|k| format!("{si}\u{1}{k}"));
                        match key.as_ref().and_then(|k| shared.get(k)) {
                            Some((expr, base)) => Input::Formula {
                                expr: expr.clone(),
                                base: *base,
                            },
                            None => match crate::parser::parse(f, at) {
                                Ok(e) => {
                                    let expr = std::sync::Arc::new(e);
                                    if let Some(k) = key {
                                        shared.insert(k, (expr.clone(), at));
                                    }
                                    Input::Formula { expr, base: at }
                                }
                                Err(_) => Input::Value(Value::text(format!("={f}"))),
                            },
                        }
                    }
                    None => Input::Value(cached.clone()),
                };
                let is_formula = matches!(input, Input::Formula { .. });
                let sheet = &mut wb.sheets[si];
                sheet.index_formula(at, is_formula);
                sheet.touch();
                sheet.cells.insert(
                    at,
                    Cell {
                        input,
                        value: cached,
                        style: c.s,
                    },
                );
            }
        }
        wb
    }

    pub fn to_model(&self) -> WorkbookModel {
        WorkbookModel {
            sheets: self
                .sheets
                .iter()
                .map(|s| SheetModel {
                    name: s.name.clone(),
                    cells: s
                        .cells
                        .iter()
                        .map(|(k, c)| match &c.input {
                            f @ Input::Formula { .. } => CellModel {
                                r: k.row,
                                c: k.col,
                                f: f.formula_text(*k),
                                v: CellValueModel::from_value(&c.value),
                                s: c.style,
                            },
                            Input::Value(v) => CellModel {
                                r: k.row,
                                c: k.col,
                                f: None,
                                v: CellValueModel::from_value(v),
                                s: c.style,
                            },
                        })
                        .collect(),
                    col_widths: s.col_widths.iter().map(|(a, b)| (*a, *b)).collect(),
                    row_heights: s.row_heights.iter().map(|(a, b)| (*a, *b)).collect(),
                    merges: s.merges.clone(),
                    frozen: s.frozen,
                    hidden: s.hidden,
                    tables: s.tables.clone(),
                    charts: s.charts.clone(),
                    source: s.source,
                })
                .collect(),
            names: self.names.clone(),
            num_formats: self.num_formats.clone(),
            styles: self.styles.clone(),
            date1904: self.date1904,
        }
    }
}
