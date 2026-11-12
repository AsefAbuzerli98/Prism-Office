//! Classic PivotTables: computing the table and the SpreadsheetML
//! pivot cache / pivot table parts.

use std::cmp::Ordering;

use crate::addr::{rect_a1, CellRef, Rect};
use crate::eval::compare;
use crate::model::PivotDef;
use crate::value::Value;
use crate::workbook::Workbook;

/// A computed pivot: the grid of cells to write plus the axis items.
#[derive(Debug, Clone, PartialEq)]
pub struct PivotTable {
    pub cells: Vec<Vec<Value>>,
    pub row_items: Vec<Value>,
    pub col_items: Vec<Value>,
    /// Rows (0-based within `cells`) to show bold: headers and grand total.
    pub bold_rows: Vec<usize>,
    pub headers: Vec<String>,
    pub data_caption: String,
}

fn key_cmp(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Empty, Value::Empty) => Ordering::Equal,
        (Value::Empty, _) => Ordering::Greater,
        (_, Value::Empty) => Ordering::Less,
        _ => compare(a, b).then_with(|| a.type_rank().cmp(&b.type_rank())),
    }
}

fn same(a: &Value, b: &Value) -> bool {
    a.type_rank() == b.type_rank() && key_cmp(a, b) == Ordering::Equal
}

pub fn function_label(f: &str) -> &'static str {
    match f {
        "average" => "Average",
        "count" => "Count",
        "max" => "Max",
        "min" => "Min",
        _ => "Sum",
    }
}

fn aggregate(f: &str, vals: &[Value]) -> Value {
    let nums: Vec<f64> = vals
        .iter()
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect();
    match f {
        "count" => Value::Number(vals.iter().filter(|v| !matches!(v, Value::Empty)).count() as f64),
        "average" if nums.is_empty() => Value::Error(crate::value::ErrorKind::Div0),
        "average" => Value::Number(nums.iter().sum::<f64>() / nums.len() as f64),
        "max" => {
            Value::Number(nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)).max_or_zero(&nums)
        }
        "min" => {
            Value::Number(nums.iter().cloned().fold(f64::INFINITY, f64::min)).max_or_zero(&nums)
        }
        _ => Value::Number(nums.iter().sum()),
    }
}

trait OrZero {
    fn max_or_zero(self, nums: &[f64]) -> Value;
}
impl OrZero for Value {
    fn max_or_zero(self, nums: &[f64]) -> Value {
        if nums.is_empty() {
            Value::Number(0.0)
        } else {
            self
        }
    }
}

fn uniques(vals: impl Iterator<Item = Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut all: Vec<Value> = vals.collect();
    all.sort_by(key_cmp);
    for v in all {
        if out.last().is_none_or_diff(&v) {
            out.push(v);
        }
    }
    out
}

trait LastDiff {
    fn is_none_or_diff(&self, v: &Value) -> bool;
}
impl LastDiff for Option<&Value> {
    fn is_none_or_diff(&self, v: &Value) -> bool {
        match self {
            None => true,
            Some(x) => !same(x, v),
        }
    }
}

/// (row key, column key, value) of one source row.
type Record = (Value, Option<Value>, Value);

/// Compute a pivot from the workbook's current values.
pub fn compute(wb: &Workbook, p: &PivotDef) -> Result<PivotTable, String> {
    let s = wb
        .sheet_index(&p.source_sheet)
        .ok_or_else(|| format!("sheet {} not found", p.source_sheet))?;
    let src = p.source;
    if src.rows() < 2 {
        return Err("the source needs a header row and data".into());
    }
    let width = src.cols();
    if p.rows >= width || p.values >= width || p.cols.is_some_and(|c| c >= width) {
        return Err("field outside the source range".into());
    }
    let get = |r: u32, c: u32| wb.value_at(s, CellRef::new(src.r0 + r, src.c0 + c));
    let headers: Vec<String> = (0..width)
        .map(|c| get(0, c).to_string())
        .map(|h| if h.is_empty() { "Column".into() } else { h })
        .collect();
    let records: Vec<Record> = (1..src.rows())
        .map(|r| (get(r, p.rows), p.cols.map(|c| get(r, c)), get(r, p.values)))
        .filter(|(rk, _, v)| !matches!(rk, Value::Empty) || !matches!(v, Value::Empty))
        .collect();
    let row_items = uniques(records.iter().map(|r| r.0.clone()));
    let col_items = if p.cols.is_some() {
        uniques(records.iter().filter_map(|r| r.1.clone()))
    } else {
        vec![]
    };
    let caption = format!(
        "{} of {}",
        function_label(&p.function),
        headers[p.values as usize]
    );
    let label = |v: &Value| {
        if matches!(v, Value::Empty) {
            Value::text("(blank)")
        } else {
            v.clone()
        }
    };
    let agg = |pred: &dyn Fn(&Record) -> bool| -> Value {
        let vals: Vec<Value> = records
            .iter()
            .filter(|r| pred(r))
            .map(|r| r.2.clone())
            .collect();
        if vals.is_empty() {
            Value::Empty
        } else {
            aggregate(&p.function, &vals)
        }
    };
    let mut cells = Vec::new();
    let mut bold_rows = vec![];
    if col_items.is_empty() {
        cells.push(vec![
            Value::text("Row Labels"),
            Value::text(caption.clone()),
        ]);
        bold_rows.push(0);
        for rk in &row_items {
            cells.push(vec![label(rk), agg(&|r| same(&r.0, rk))]);
        }
        cells.push(vec![Value::text("Grand Total"), agg(&|_| true)]);
    } else {
        let mut head = vec![Value::text(caption.clone()), Value::text("Column Labels")];
        head.resize(col_items.len() + 2, Value::Empty);
        cells.push(head);
        let mut labels = vec![Value::text("Row Labels")];
        labels.extend(col_items.iter().map(label));
        labels.push(Value::text("Grand Total"));
        cells.push(labels);
        bold_rows.extend([0, 1]);
        for rk in &row_items {
            let mut row = vec![label(rk)];
            for ck in &col_items {
                row.push(agg(&|r| {
                    same(&r.0, rk) && r.1.as_ref().is_some_and(|c| same(c, ck))
                }));
            }
            row.push(agg(&|r| same(&r.0, rk)));
            cells.push(row);
        }
        let mut total = vec![Value::text("Grand Total")];
        for ck in &col_items {
            total.push(agg(&|r| r.1.as_ref().is_some_and(|c| same(c, ck))));
        }
        total.push(agg(&|_| true));
        cells.push(total);
    }
    bold_rows.push(cells.len() - 1);
    Ok(PivotTable {
        cells,
        row_items,
        col_items,
        bold_rows,
        headers,
        data_caption: caption,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn item_xml(v: &Value) -> String {
    match v {
        Value::Number(n) => format!(r#"<n v="{}"/>"#, crate::value::format_number_roundtrip(*n)),
        Value::Bool(b) => format!(r#"<b v="{}"/>"#, *b as u8),
        Value::Empty => "<m/>".into(),
        Value::Error(e) => format!(r#"<e v="{}"/>"#, esc(e.code())),
        other => format!(r#"<s v="{}"/>"#, esc(&other.to_string())),
    }
}

const S_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// `pivotCacheDefinition` (no records; Excel refreshes on open).
pub fn cache_definition_xml(p: &PivotDef, t: &PivotTable, wb: &Workbook) -> String {
    let s = wb.sheet_index(&p.source_sheet).unwrap_or(0);
    let mut fields = String::new();
    for (i, h) in t.headers.iter().enumerate() {
        let i = i as u32;
        let items = if i == p.rows {
            Some(&t.row_items)
        } else if Some(i) == p.cols {
            Some(&t.col_items)
        } else {
            None
        };
        let shared = match items {
            Some(list) => {
                let all_num = list.iter().all(|v| matches!(v, Value::Number(_)));
                let attrs = if all_num && !list.is_empty() {
                    r#" containsSemiMixedTypes="0" containsString="0" containsNumber="1""#
                } else {
                    ""
                };
                format!(
                    r#"<sharedItems{attrs} count="{}">{}</sharedItems>"#,
                    list.len(),
                    list.iter().map(item_xml).collect::<String>()
                )
            }
            None => {
                let nums: Vec<f64> = (1..p.source.rows())
                    .filter_map(|r| {
                        if let Value::Number(n) =
                            wb.value_at(s, CellRef::new(p.source.r0 + r, p.source.c0 + i))
                        {
                            Some(n)
                        } else {
                            None
                        }
                    })
                    .collect();
                if !nums.is_empty() && nums.len() as u32 == p.source.rows() - 1 {
                    format!(
                        r#"<sharedItems containsSemiMixedTypes="0" containsString="0" containsNumber="1" minValue="{}" maxValue="{}"/>"#,
                        crate::value::format_number_roundtrip(
                            nums.iter().cloned().fold(f64::INFINITY, f64::min)
                        ),
                        crate::value::format_number_roundtrip(
                            nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                        )
                    )
                } else {
                    "<sharedItems/>".into()
                }
            }
        };
        fields.push_str(&format!(
            r#"<cacheField name="{}" numFmtId="0">{shared}</cacheField>"#,
            esc(h)
        ));
    }
    format!(
        r#"{DECL}<pivotCacheDefinition xmlns="{S_NS}" xmlns:r="{R_NS}" saveData="0" refreshOnLoad="1" createdVersion="6" refreshedVersion="6" minRefreshableVersion="3" recordCount="{}"><cacheSource type="worksheet"><worksheetSource ref="{}" sheet="{}"/></cacheSource><cacheFields count="{}">{fields}</cacheFields></pivotCacheDefinition>"#,
        p.source.rows() - 1,
        rect_a1(&p.source),
        esc(&p.source_sheet),
        t.headers.len()
    )
}

/// `pivotTableDefinition` matching the computed layout.
pub fn table_definition_xml(p: &PivotDef, t: &PivotTable, cache_id: u32) -> String {
    let n_rows = t.cells.len() as u32;
    let n_cols = t.cells.first().map_or(1, |r| r.len()) as u32;
    let loc = Rect {
        r0: p.location.row,
        c0: p.location.col,
        r1: p.location.row + n_rows - 1,
        c1: p.location.col + n_cols - 1,
    };
    let first_data_row = if p.cols.is_some() { 2 } else { 1 };
    let mut fields = String::new();
    for i in 0..t.headers.len() as u32 {
        let axis = |name: &str, list: &[Value]| {
            format!(
                r#"<pivotField axis="{name}" showAll="0"><items count="{}">{}<item t="default"/></items></pivotField>"#,
                list.len() + 1,
                (0..list.len())
                    .map(|k| format!(r#"<item x="{k}"/>"#))
                    .collect::<String>()
            )
        };
        if i == p.rows {
            fields.push_str(&axis("axisRow", &t.row_items));
        } else if Some(i) == p.cols {
            fields.push_str(&axis("axisCol", &t.col_items));
        } else if i == p.values {
            fields.push_str(r#"<pivotField dataField="1" showAll="0"/>"#);
        } else {
            fields.push_str(r#"<pivotField showAll="0"/>"#);
        }
    }
    let items = |n: usize| {
        let mut s = String::new();
        for k in 0..n {
            if k == 0 {
                s.push_str("<i><x/></i>");
            } else {
                s.push_str(&format!(r#"<i><x v="{k}"/></i>"#));
            }
        }
        s.push_str(r#"<i t="grand"><x/></i>"#);
        s
    };
    let row_items = format!(
        r#"<rowFields count="1"><field x="{}"/></rowFields><rowItems count="{}">{}</rowItems>"#,
        p.rows,
        t.row_items.len() + 1,
        items(t.row_items.len())
    );
    let col_items = match p.cols {
        Some(c) => format!(
            r#"<colFields count="1"><field x="{c}"/></colFields><colItems count="{}">{}</colItems>"#,
            t.col_items.len() + 1,
            items(t.col_items.len())
        ),
        None => r#"<colItems count="1"><i/></colItems>"#.into(),
    };
    let subtotal = if p.function == "sum" {
        String::new()
    } else {
        format!(r#" subtotal="{}""#, p.function)
    };
    format!(
        r#"{DECL}<pivotTableDefinition xmlns="{S_NS}" name="{}" cacheId="{cache_id}" applyNumberFormats="0" applyBorderFormats="0" applyFontFormats="0" applyPatternFormats="0" applyAlignmentFormats="0" applyWidthHeightFormats="1" dataCaption="Values" updatedVersion="6" minRefreshableVersion="3" useAutoFormatting="1" itemPrintTitles="1" createdVersion="6" indent="0" outline="1" outlineData="1" multipleFieldFilters="0"><location ref="{}" firstHeaderRow="1" firstDataRow="{first_data_row}" firstDataCol="1"/><pivotFields count="{}">{fields}</pivotFields>{row_items}{col_items}<dataFields count="1"><dataField name="{}" fld="{}"{subtotal} baseField="0" baseItem="0"/></dataFields><pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"/></pivotTableDefinition>"#,
        esc(&p.name),
        rect_a1(&loc),
        t.headers.len(),
        esc(&t.data_caption),
        p.values
    )
}
