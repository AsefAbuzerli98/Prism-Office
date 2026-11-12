//! Serializable workbook model exchanged between the host (XLSX I/O) and the
//! engine (WASM in the webview, native on save).

use serde::{Deserialize, Serialize};

use crate::addr::{CellRef, Rect};
use crate::value::{ErrorKind, Value};
use crate::workbook::{Cell, DefinedName, Input, Sheet, TableDef, Workbook};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CellValueModel {
    Bool(bool),
    Number(f64),
    Text(String),
    Error { e: String },
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
            s.source = sm.source;
            wb.sheets.push(s);
        }
        if wb.sheets.is_empty() {
            wb.sheets.push(Sheet::new("Sheet1"));
        }
        wb.reparse_names();
        for (si, sm) in m.sheets.iter().enumerate() {
            for c in &sm.cells {
                let at = CellRef::new(c.r, c.c);
                let cached = c.v.as_ref().map(|v| v.to_value()).unwrap_or_default();
                let input = match &c.f {
                    Some(f) => match crate::parser::parse(f, at) {
                        Ok(e) => Input::Formula {
                            text: f.clone(),
                            expr: std::sync::Arc::new(e),
                        },
                        Err(_) => Input::Value(Value::text(format!("={f}"))),
                    },
                    None => Input::Value(cached.clone()),
                };
                let is_formula = matches!(input, Input::Formula { .. });
                let sheet = &mut wb.sheets[si];
                sheet.index_formula(at, is_formula);
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
        wb.mark_all_dirty();
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
                            Input::Formula { text, .. } => CellModel {
                                r: k.row,
                                c: k.col,
                                f: Some(text.clone()),
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
