//! Chart data extraction and DrawingML chart XML generation.

use serde::Serialize;

use crate::addr::{col_name, CellRef, Rect};
use crate::model::ChartDef;
use crate::value::Value;
use crate::workbook::Workbook;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Series {
    pub name: String,
    pub values: Vec<Option<f64>>,
    /// Source references (sheet-qualified, absolute) for file output.
    #[serde(skip)]
    pub name_ref: Option<Rect>,
    #[serde(skip)]
    pub values_ref: Rect,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ChartData {
    pub categories: Vec<String>,
    pub series: Vec<Series>,
    #[serde(skip)]
    pub categories_ref: Option<Rect>,
}

fn is_label(v: &Value) -> bool {
    matches!(v, Value::Text(_) | Value::Empty)
}

/// Split a data block into categories and series (columns are series when
/// the block is taller than wide, like Excel's default).
pub fn chart_data(wb: &Workbook, sheet: usize, def: &ChartDef) -> ChartData {
    let d = def.data;
    let get = |r: u32, c: u32| wb.value_at(sheet, CellRef::new(r, c));
    let by_col = d.rows() >= d.cols();
    // header row/column present when the first row (resp. column) holds labels
    let (outer, inner) = if by_col {
        (d.cols(), d.rows())
    } else {
        (d.rows(), d.cols())
    };
    let at = |o: u32, i: u32| {
        if by_col {
            get(d.r0 + i, d.c0 + o)
        } else {
            get(d.r0 + o, d.c0 + i)
        }
    };
    let named = if outer > 1 { 1..outer } else { 0..outer };
    let has_names = inner > 1
        && named.clone().all(|o| is_label(&at(o, 0)))
        && named.clone().any(|o| matches!(at(o, 0), Value::Text(_)));
    let has_cats = inner > 0
        && outer > 1
        && (0..inner)
            .skip(has_names as usize)
            .all(|i| is_label(&at(0, i)) || def.kind == "scatter");
    let first_val = has_names as u32;
    let first_series = has_cats as u32;
    let rect_of = |o: u32, i0: u32, i1: u32| {
        if by_col {
            Rect {
                r0: d.r0 + i0,
                r1: d.r0 + i1,
                c0: d.c0 + o,
                c1: d.c0 + o,
            }
        } else {
            Rect {
                r0: d.r0 + o,
                r1: d.r0 + o,
                c0: d.c0 + i0,
                c1: d.c0 + i1,
            }
        }
    };
    let categories: Vec<String> = if has_cats {
        (first_val..inner).map(|i| at(0, i).to_string()).collect()
    } else {
        (first_val..inner)
            .map(|i| (i - first_val + 1).to_string())
            .collect()
    };
    let series = (first_series..outer)
        .map(|o| Series {
            name: if has_names {
                at(o, 0).to_string()
            } else {
                format!("Series{}", o - first_series + 1)
            },
            values: (first_val..inner)
                .map(|i| {
                    if let Value::Number(n) = at(o, i) {
                        Some(n)
                    } else {
                        None
                    }
                })
                .collect(),
            name_ref: has_names.then(|| rect_of(o, 0, 0)),
            values_ref: rect_of(o, first_val, inner.saturating_sub(1).max(first_val)),
        })
        .collect();
    ChartData {
        categories,
        series,
        categories_ref: (has_cats && inner > first_val).then(|| rect_of(0, first_val, inner - 1)),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn abs_ref(sheet: &str, r: &Rect) -> String {
    let q = if sheet.chars().all(|c| c.is_alphanumeric() || c == '_') {
        sheet.to_string()
    } else {
        format!("'{}'", sheet.replace('\'', "''"))
    };
    let a = |c: CellRef| format!("${}${}", col_name(c.col), c.row + 1);
    if r.rows() == 1 && r.cols() == 1 {
        format!("{q}!{}", a(r.start()))
    } else {
        format!("{q}!{}:{}", a(r.start()), a(CellRef::new(r.r1, r.c1)))
    }
}

fn str_cache(items: &[String]) -> String {
    let mut s = format!(r#"<c:strCache><c:ptCount val="{}"/>"#, items.len());
    for (i, v) in items.iter().enumerate() {
        s.push_str(&format!(r#"<c:pt idx="{i}"><c:v>{}</c:v></c:pt>"#, esc(v)));
    }
    s.push_str("</c:strCache>");
    s
}

fn num_cache(items: &[Option<f64>]) -> String {
    let mut s = format!(
        r#"<c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{}"/>"#,
        items.len()
    );
    for (i, v) in items.iter().enumerate() {
        if let Some(v) = v {
            s.push_str(&format!(
                r#"<c:pt idx="{i}"><c:v>{}</c:v></c:pt>"#,
                crate::value::format_number_roundtrip(*v)
            ));
        }
    }
    s.push_str("</c:numCache>");
    s
}

/// `c:chartSpace` XML for a chart.
pub fn chart_xml(def: &ChartDef, data: &ChartData, sheet: &str) -> String {
    let kind = def.kind.as_str();
    let mut sers = String::new();
    for (i, s) in data.series.iter().enumerate() {
        let tx = match &s.name_ref {
            Some(r) => format!(
                r#"<c:tx><c:strRef><c:f>{}</c:f>{}</c:strRef></c:tx>"#,
                esc(&abs_ref(sheet, r)),
                str_cache(std::slice::from_ref(&s.name))
            ),
            None => format!(r#"<c:tx><c:v>{}</c:v></c:tx>"#, esc(&s.name)),
        };
        let cats = match &data.categories_ref {
            Some(r) => format!(
                r#"<c:strRef><c:f>{}</c:f>{}</c:strRef>"#,
                esc(&abs_ref(sheet, r)),
                str_cache(&data.categories)
            ),
            None => String::new(),
        };
        let vals = format!(
            r#"<c:numRef><c:f>{}</c:f>{}</c:numRef>"#,
            esc(&abs_ref(sheet, &s.values_ref)),
            num_cache(&s.values)
        );
        let extra = match kind {
            "line" => {
                r#"<c:marker><c:symbol val="circle"/><c:size val="5"/></c:marker>"#.to_string()
            }
            "scatter" => {
                r#"<c:marker><c:symbol val="circle"/><c:size val="6"/></c:marker>"#.to_string()
            }
            _ => String::new(),
        };
        let (cat_tag, val_tag) = if kind == "scatter" {
            ("c:xVal", "c:yVal")
        } else {
            ("c:cat", "c:val")
        };
        let cat_xml = if cats.is_empty() {
            String::new()
        } else {
            format!("<{cat_tag}>{cats}</{cat_tag}>")
        };
        let smooth = if kind == "line" || kind == "scatter" {
            r#"<c:smooth val="0"/>"#
        } else {
            ""
        };
        sers.push_str(&format!(r#"<c:ser><c:idx val="{i}"/><c:order val="{i}"/>{tx}{extra}{cat_xml}<{val_tag}>{vals}</{val_tag}>{smooth}</c:ser>"#));
    }
    let axes_ids = r#"<c:axId val="500000001"/><c:axId val="500000002"/>"#;
    let cat_ax = |pos: &str| {
        format!(
            r#"<c:catAx><c:axId val="500000001"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="{pos}"/><c:numFmt formatCode="General" sourceLinked="1"/><c:tickLblPos val="nextTo"/><c:crossAx val="500000002"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/></c:catAx>"#
        )
    };
    let val_ax = |id: &str, cross: &str, pos: &str, grid: bool| {
        format!(
            r#"<c:valAx><c:axId val="{id}"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="{pos}"/>{}<c:numFmt formatCode="General" sourceLinked="1"/><c:tickLblPos val="nextTo"/><c:crossAx val="{cross}"/><c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>"#,
            if grid { "<c:majorGridlines/>" } else { "" }
        )
    };
    let plot = match kind {
        "pie" => format!(
            r#"<c:pieChart><c:varyColors val="1"/>{sers}<c:firstSliceAng val="0"/></c:pieChart>"#
        ),
        "line" => format!(
            r#"<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>{sers}<c:marker val="1"/>{axes_ids}</c:lineChart>{}{}"#,
            cat_ax("b"),
            val_ax("500000002", "500000001", "l", true)
        ),
        "area" => format!(
            r#"<c:areaChart><c:grouping val="standard"/><c:varyColors val="0"/>{sers}{axes_ids}</c:areaChart>{}{}"#,
            cat_ax("b"),
            val_ax("500000002", "500000001", "l", true)
        ),
        "scatter" => format!(
            r#"<c:scatterChart><c:scatterStyle val="lineMarker"/><c:varyColors val="0"/>{sers}{axes_ids}</c:scatterChart>{}{}"#,
            val_ax("500000001", "500000002", "b", false),
            val_ax("500000002", "500000001", "l", true)
        ),
        "bar" => format!(
            r#"<c:barChart><c:barDir val="bar"/><c:grouping val="clustered"/><c:varyColors val="0"/>{sers}<c:gapWidth val="150"/>{axes_ids}</c:barChart>{}{}"#,
            cat_ax("l"),
            val_ax("500000002", "500000001", "b", true)
        ),
        _ => format!(
            r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>{sers}<c:gapWidth val="150"/>{axes_ids}</c:barChart>{}{}"#,
            cat_ax("b"),
            val_ax("500000002", "500000001", "l", true)
        ),
    };
    let title = if def.title.is_empty() {
        r#"<c:autoTitleDeleted val="1"/>"#.to_string()
    } else {
        format!(
            r#"<c:title><c:tx><c:rich><a:bodyPr/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/>"#,
            esc(&def.title)
        )
    };
    format!(
        r#"{}<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><c:roundedCorners val="0"/><c:chart>{title}<c:plotArea><c:layout/>{plot}</c:plotArea><c:legend><c:legendPos val="{}"/><c:overlay val="0"/></c:legend><c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart></c:chartSpace>"#,
        prism_xml_decl(),
        if kind == "pie" { "r" } else { "b" }
    )
}

fn prism_xml_decl() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#
}

/// `xdr:twoCellAnchor` placing chart `rid` over `anchor`.
pub fn anchor_xml(id: usize, name: &str, anchor: &Rect, rid: &str) -> String {
    format!(
        r#"<xdr:twoCellAnchor editAs="oneCell"><xdr:from><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="{id}" name="{}"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="{rid}"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>"#,
        anchor.c0,
        anchor.r0,
        anchor.c1 + 1,
        anchor.r1 + 1,
        esc(name)
    )
}

pub const DRAWING_OPEN: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#;
