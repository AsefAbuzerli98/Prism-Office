//! XLSX import/export.
//!
//! Reading produces a [`WorkbookModel`] plus an [`XlsxSource`] that keeps the
//! original package. Writing regenerates only what the model owns
//! (`sheetData`, dimension, columns, merges, frozen panes, sheet list, defined
//! names, shared strings, dynamic-array metadata, appended number formats);
//! every other part and worksheet child is copied through byte-for-byte.

use std::collections::{BTreeMap, HashMap};

use prism_openxml::xml::{attrs_of, escape_attr, name_of, ref_text, text_of};
use prism_openxml::{rel_types, DocumentKind, Package, PartName, Relationships, TargetMode};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::addr::{a1, rect_a1, CellRef, Rect};
use crate::model::{CellModel, CellValueModel, SheetModel, WorkbookModel};
use crate::parser::{self, WriteOptions};
use crate::value::Value;
use crate::workbook::{DefinedName, Input, TableDef, Workbook};

const S_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_SST: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
const REL_TABLE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table";
const REL_META: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata";
const REL_CALC_CHAIN: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain";
const CT_SHEET: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const CT_SST: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
const CT_META: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml";

#[derive(Debug, thiserror::Error)]
pub enum XlsxError {
    #[error(transparent)]
    Package(#[from] prism_openxml::Error),
    #[error("malformed {part}: {msg}")]
    Malformed { part: String, msg: String },
}

type Result<T> = std::result::Result<T, XlsxError>;

fn bad(part: &str, msg: impl ToString) -> XlsxError {
    XlsxError::Malformed {
        part: part.into(),
        msg: msg.to_string(),
    }
}

/// A top-level XML element split into raw children for passthrough.
#[derive(Clone, Debug, Default)]
struct RawDoc {
    /// Everything up to and including the root start tag.
    head: String,
    /// (local name, raw xml) of each root child.
    children: Vec<(String, String)>,
    root: String,
}

impl RawDoc {
    fn parse(part: &str, xml: &str) -> Result<Self> {
        let mut r = Reader::from_str(xml);
        let mut doc = RawDoc::default();
        let mut depth = 0usize;
        let mut child_start = 0usize;
        let mut child_name = String::new();
        loop {
            let before = r.buffer_position() as usize;
            let ev = r.read_event().map_err(|e| bad(part, e))?;
            let after = r.buffer_position() as usize;
            match ev {
                Event::Start(e) => {
                    if depth == 0 {
                        doc.head = xml[..after].to_string();
                        doc.root = name_of(&e);
                    } else if depth == 1 {
                        child_start = before;
                        child_name = name_of(&e);
                    }
                    depth += 1;
                }
                Event::Empty(e) => {
                    if depth == 0 {
                        doc.head = xml[..after].to_string();
                        doc.root = name_of(&e);
                        break;
                    } else if depth == 1 {
                        doc.children
                            .push((name_of(&e), xml[before..after].to_string()));
                    }
                }
                Event::End(_) => {
                    depth -= 1;
                    if depth == 1 {
                        doc.children.push((
                            std::mem::take(&mut child_name),
                            xml[child_start..after].to_string(),
                        ));
                    }
                    if depth == 0 {
                        break;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(doc)
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.children
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, x)| x.as_str())
    }

    fn head_open(&self) -> String {
        // turn a self-closing root into an open tag
        let h = self.head.trim_end();
        match h.strip_suffix("/>") {
            Some(x) => format!("{x}>"),
            None => h.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct SourceSheet {
    part: PartName,
    raw: RawDoc,
    frozen: (u32, u32),
    col_widths: Vec<(u32, f64)>,
    merges: Vec<Rect>,
}

/// The original package of an imported workbook, needed for lossless save.
#[derive(Clone, Debug)]
pub struct XlsxSource {
    package: Package,
    workbook_part: PartName,
    workbook_raw: RawDoc,
    sheets: Vec<SourceSheet>,
    xf_count: usize,
    names: Vec<DefinedName>,
}

// ---- styles ---------------------------------------------------------------------------

pub fn builtin_format(id: u32) -> Option<&'static str> {
    Some(match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        5 => "\"$\"#,##0_);(\"$\"#,##0)",
        6 => "\"$\"#,##0_);[Red](\"$\"#,##0)",
        7 => "\"$\"#,##0.00_);(\"$\"#,##0.00)",
        8 => "\"$\"#,##0.00_);[Red](\"$\"#,##0.00)",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => "m/d/yyyy",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "m/d/yyyy h:mm",
        37 => "#,##0 ;(#,##0)",
        38 => "#,##0 ;[Red](#,##0)",
        39 => "#,##0.00;(#,##0.00)",
        40 => "#,##0.00;[Red](#,##0.00)",
        41 => "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)",
        42 => "_(\"$\"* #,##0_);_(\"$\"* (#,##0);_(\"$\"* \"-\"_);_(@_)",
        43 => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
        44 => "_(\"$\"* #,##0.00_);_(\"$\"* (#,##0.00);_(\"$\"* \"-\"??_);_(@_)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mm:ss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}

/// Number-format code for each cellXfs entry.
fn parse_styles(xml: &[u8]) -> Result<Vec<String>> {
    let mut r = Reader::from_reader(xml);
    let mut custom: HashMap<u32, String> = HashMap::new();
    let mut xfs = Vec::new();
    let mut in_xfs = false;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_event_into(&mut buf).map_err(|e| bad("styles", e))? {
            Event::Start(e) | Event::Empty(e) => {
                let n = name_of(&e);
                let a = attrs_of(&e);
                let get = |k: &str| a.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
                match n.as_str() {
                    "numFmt" => {
                        if let (Some(id), Some(code)) = (
                            get("numFmtId").and_then(|x| x.parse().ok()),
                            get("formatCode"),
                        ) {
                            custom.insert(id, code);
                        }
                    }
                    "cellXfs" => in_xfs = true,
                    "xf" if in_xfs => {
                        let id: u32 = get("numFmtId").and_then(|x| x.parse().ok()).unwrap_or(0);
                        xfs.push(
                            custom
                                .get(&id)
                                .cloned()
                                .or_else(|| builtin_format(id).map(String::from))
                                .unwrap_or_else(|| "General".into()),
                        );
                    }
                    _ => {}
                }
            }
            Event::End(e) if e.local_name().into_inner() == "cellXfs" => in_xfs = false,
            Event::Eof => break,
            _ => {}
        }
    }
    if xfs.is_empty() {
        xfs.push("General".into());
    }
    Ok(xfs)
}

/// Append cell formats for number formats beyond the source's cellXfs.
fn extend_styles(xml: &str, existing: usize, formats: &[String]) -> String {
    if formats.len() <= existing {
        return xml.to_string();
    }
    let mut s = xml.to_string();
    // custom numFmt ids start at 164
    let mut next_id = 164u32;
    for cap in regex::Regex::new(r#"numFmtId="(\d+)""#)
        .unwrap()
        .captures_iter(xml)
    {
        if let Ok(n) = cap[1].parse::<u32>() {
            next_id = next_id.max(n + 1);
        }
    }
    let mut fmts = String::new();
    let mut xfs = String::new();
    let mut ids: HashMap<String, u32> = HashMap::new();
    for code in &formats[existing..] {
        let id = match (0..50).find(|i| builtin_format(*i) == Some(code.as_str())) {
            Some(b) => b,
            None => *ids.entry(code.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                fmts.push_str(&format!(
                    r#"<numFmt numFmtId="{id}" formatCode="{}"/>"#,
                    escape_attr(code)
                ));
                id
            }),
        };
        xfs.push_str(&format!(r#"<xf numFmtId="{id}" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>"#));
    }
    let added = formats.len() - existing;
    let bump = |s: &mut String, tag: &str, n: usize, insert: &str| {
        let open = format!("<{tag}");
        if let Some(i) = s.find(&open) {
            let end = s[i..].find('>').map(|e| e + i).unwrap();
            let start_tag = s[i..=end].to_string();
            let re = regex::Regex::new(r#"count="(\d+)""#).unwrap();
            let new_tag = match re.captures(&start_tag) {
                Some(c) => re
                    .replace(
                        &start_tag,
                        format!(r#"count="{}""#, c[1].parse::<usize>().unwrap_or(0) + n).as_str(),
                    )
                    .into_owned(),
                None => start_tag.clone(),
            };
            s.replace_range(i..=end, &new_tag);
            if let Some(close) = s.find(&format!("</{tag}>")) {
                s.insert_str(close, insert);
            }
            true
        } else {
            false
        }
    };
    if !fmts.is_empty() {
        let n = fmts.matches("<numFmt ").count();
        if !bump(&mut s, "numFmts", n, &fmts) {
            if let Some(i) = s.find("<styleSheet") {
                let end = s[i..].find('>').unwrap() + i;
                s.insert_str(
                    end + 1,
                    &format!(r#"<numFmts count="{n}">{fmts}</numFmts>"#),
                );
            }
        }
    }
    bump(&mut s, "cellXfs", added, &xfs);
    s
}

// ---- reading -------------------------------------------------------------------------------

fn part_text(pkg: &Package, p: &PartName) -> Result<String> {
    let b = pkg.part(p).ok_or_else(|| bad(p.as_str(), "missing part"))?;
    Ok(String::from_utf8_lossy(b).into_owned())
}

fn serial_from_iso(s: &str) -> Option<f64> {
    crate::functions::date::parse_date_text(s)
}

struct ArrayAnchor {
    rect: Rect,
}

fn read_sheet(xml: &str, part: &str, sst: &[String], out: &mut SheetModel) -> Result<RawDoc> {
    let raw = RawDoc::parse(part, xml)?;
    let mut shared: HashMap<String, (CellRef, crate::parser::Expr)> = HashMap::new();
    let mut arrays: Vec<ArrayAnchor> = Vec::new();
    if let Some(sd) = raw.get("sheetData") {
        let mut r = Reader::from_str(sd);
        let mut row_idx = 0u32;
        let mut next_col = 0u32;
        // current cell
        let mut cur: Option<(CellRef, String, u32)> = None; // pos, t, style
        let mut f_attrs: Vec<(String, String)> = Vec::new();
        let mut has_f = false;
        let (mut v_text, mut f_text, mut is_text) = (String::new(), String::new(), String::new());
        #[derive(PartialEq)]
        enum T {
            None,
            V,
            F,
            Is,
        }
        let mut target = T::None;
        let mut in_rph = false;
        loop {
            match r.read_event().map_err(|e| bad(part, e))? {
                Event::Start(e) => {
                    let n = name_of(&e);
                    match n.as_str() {
                        "row" => {
                            let a = attrs_of(&e);
                            row_idx = a
                                .iter()
                                .find(|(k, _)| k == "r")
                                .and_then(|(_, v)| v.parse::<u32>().ok())
                                .map(|x| x - 1)
                                .unwrap_or(row_idx);
                            if let Some(ht) = a
                                .iter()
                                .find(|(k, _)| k == "ht")
                                .and_then(|(_, v)| v.parse::<f64>().ok())
                            {
                                if a.iter()
                                    .any(|(k, v)| k == "customHeight" && (v == "1" || v == "true"))
                                {
                                    out.row_heights.push((row_idx, ht));
                                }
                            }
                            next_col = 0;
                        }
                        "c" => {
                            let a = attrs_of(&e);
                            let pos = a
                                .iter()
                                .find(|(k, _)| k == "r")
                                .and_then(|(_, v)| crate::addr::parse_a1(v))
                                .map(|x| x.0)
                                .unwrap_or(CellRef::new(row_idx, next_col));
                            next_col = pos.col + 1;
                            let t = a
                                .iter()
                                .find(|(k, _)| k == "t")
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default();
                            let s = a
                                .iter()
                                .find(|(k, _)| k == "s")
                                .and_then(|(_, v)| v.parse().ok())
                                .unwrap_or(0);
                            cur = Some((pos, t, s));
                            has_f = false;
                            v_text.clear();
                            f_text.clear();
                            is_text.clear();
                        }
                        "v" => target = T::V,
                        "f" => {
                            f_attrs = attrs_of(&e);
                            has_f = true;
                            target = T::F;
                        }
                        "is" => target = T::Is,
                        "rPh" => in_rph = true,
                        _ => {}
                    }
                }
                Event::Empty(e) => {
                    let n = name_of(&e);
                    match n.as_str() {
                        "row" => {
                            let a = attrs_of(&e);
                            row_idx = a
                                .iter()
                                .find(|(k, _)| k == "r")
                                .and_then(|(_, v)| v.parse::<u32>().ok())
                                .map(|x| x - 1)
                                .unwrap_or(row_idx)
                                + 1;
                        }
                        "c" => {
                            let a = attrs_of(&e);
                            let pos = a
                                .iter()
                                .find(|(k, _)| k == "r")
                                .and_then(|(_, v)| crate::addr::parse_a1(v))
                                .map(|x| x.0)
                                .unwrap_or(CellRef::new(row_idx, next_col));
                            next_col = pos.col + 1;
                            let s = a
                                .iter()
                                .find(|(k, _)| k == "s")
                                .and_then(|(_, v)| v.parse().ok())
                                .unwrap_or(0);
                            if s != 0 {
                                out.cells.push(CellModel {
                                    r: pos.row,
                                    c: pos.col,
                                    f: None,
                                    v: None,
                                    s,
                                });
                            }
                        }
                        "f" => {
                            f_attrs = attrs_of(&e);
                            has_f = true;
                        }
                        _ => {}
                    }
                }
                Event::Text(t) => {
                    let s = text_of(&t);
                    match target {
                        T::V => v_text.push_str(&s),
                        T::F => f_text.push_str(&s),
                        T::Is if !in_rph => is_text.push_str(&s),
                        _ => {}
                    }
                }
                Event::GeneralRef(g) => {
                    let s = ref_text(&g);
                    match target {
                        T::V => v_text.push_str(&s),
                        T::F => f_text.push_str(&s),
                        T::Is if !in_rph => is_text.push_str(&s),
                        _ => {}
                    }
                }
                Event::End(e) => {
                    let n = e.local_name().into_inner().to_string();
                    match n.as_str() {
                        "v" | "f" => target = T::None,
                        "is" => target = T::None,
                        "rPh" => in_rph = false,
                        "row" => row_idx += 1,
                        "c" => {
                            let Some((pos, t, s)) = cur.take() else {
                                continue;
                            };
                            let value = match t.as_str() {
                                "s" => v_text
                                    .trim()
                                    .parse::<usize>()
                                    .ok()
                                    .and_then(|i| sst.get(i))
                                    .map(|x| CellValueModel::Text(x.clone())),
                                "str" => Some(CellValueModel::Text(v_text.clone())),
                                "inlineStr" => Some(CellValueModel::Text(is_text.clone())),
                                "b" => Some(CellValueModel::Bool(
                                    v_text.trim() == "1"
                                        || v_text.trim().eq_ignore_ascii_case("true"),
                                )),
                                "e" => Some(CellValueModel::Error {
                                    e: v_text.trim().to_string(),
                                }),
                                "d" => serial_from_iso(v_text.trim()).map(CellValueModel::Number),
                                _ if v_text.trim().is_empty() => None,
                                _ => v_text
                                    .trim()
                                    .parse::<f64>()
                                    .ok()
                                    .map(CellValueModel::Number),
                            };
                            let mut formula = None;
                            if has_f {
                                let get = |k: &str| {
                                    f_attrs.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone())
                                };
                                let ft = get("t").unwrap_or_default();
                                let text = f_text.trim().to_string();
                                if ft == "shared" {
                                    let si = get("si").unwrap_or_default();
                                    if !text.is_empty() {
                                        if let Ok(e) = parser::parse(&text, pos) {
                                            shared.insert(si, (pos, e));
                                        }
                                        formula = Some(text);
                                    } else if let Some((base, e)) = shared.get(&si) {
                                        let moved = parser::shift(
                                            e,
                                            pos.row as i64 - base.row as i64,
                                            pos.col as i64 - base.col as i64,
                                        );
                                        formula = Some(parser::to_formula(
                                            &moved,
                                            &WriteOptions::default(),
                                        ));
                                    }
                                } else if !text.is_empty() {
                                    if ft == "array" {
                                        if let Some(rect) = get("ref").and_then(|r| parse_rect(&r))
                                        {
                                            arrays.push(ArrayAnchor { rect });
                                        }
                                    }
                                    formula = Some(text);
                                }
                                // normalise file prefixes (_xlfn. …) to display form
                                if let Some(f) = &formula {
                                    if let Ok(e) = parser::parse(f, pos) {
                                        formula =
                                            Some(parser::to_formula(&e, &WriteOptions::default()));
                                    }
                                }
                            }
                            if formula.is_some() || value.is_some() || s != 0 {
                                out.cells.push(CellModel {
                                    r: pos.row,
                                    c: pos.col,
                                    f: formula,
                                    v: value,
                                    s,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
    // spilled/array result cells are recomputed: drop their cached plain values
    if !arrays.is_empty() {
        out.cells.retain(|c| {
            let p = CellRef::new(c.r, c.c);
            !arrays
                .iter()
                .any(|a| a.rect.contains(p) && a.rect.start() != p && c.f.is_none())
        });
    }
    // columns
    if let Some(cols) = raw.get("cols") {
        let mut r = Reader::from_str(cols);
        loop {
            match r.read_event().map_err(|e| bad(part, e))? {
                Event::Start(e) | Event::Empty(e) if name_of(&e) == "col" => {
                    let a = attrs_of(&e);
                    let g = |k: &str| a.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
                    if let (Some(min), Some(max), Some(w)) = (
                        g("min").and_then(|x| x.parse::<u32>().ok()),
                        g("max").and_then(|x| x.parse::<u32>().ok()),
                        g("width").and_then(|x| x.parse::<f64>().ok()),
                    ) {
                        for c in min..=max.min(min + 16384) {
                            out.col_widths.push((c - 1, w));
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
    if let Some(m) = raw.get("mergeCells") {
        for cap in regex::Regex::new(r#"ref="([^"]+)""#)
            .unwrap()
            .captures_iter(m)
        {
            if let Some(rect) = parse_rect(&cap[1]) {
                out.merges.push(rect);
            }
        }
    }
    if let Some(v) = raw.get("sheetViews") {
        if v.contains(r#"state="frozen""#) {
            let num = |k: &str| {
                regex::Regex::new(&format!(r#"{k}="([\d.]+)""#))
                    .unwrap()
                    .captures(v)
                    .and_then(|c| c[1].parse::<f64>().ok())
                    .unwrap_or(0.0) as u32
            };
            out.frozen = (num("ySplit"), num("xSplit"));
        }
    }
    Ok(raw)
}

fn parse_rect(s: &str) -> Option<Rect> {
    let (a, b) = s.split_once(':').unwrap_or((s, s));
    Some(Rect::new(
        crate::addr::parse_a1(a)?.0,
        crate::addr::parse_a1(b)?.0,
    ))
}

fn read_table(xml: &str) -> Option<TableDef> {
    let mut r = Reader::from_str(xml);
    let mut t: Option<TableDef> = None;
    loop {
        match r.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) => {
                let a = attrs_of(&e);
                let g = |k: &str| a.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
                match name_of(&e).as_str() {
                    "table" => {
                        t = Some(TableDef {
                            name: g("displayName").or(g("name"))?,
                            rect: parse_rect(&g("ref")?)?,
                            columns: vec![],
                            header_row: g("headerRowCount").is_none_or_zero(),
                            totals_row: g("totalsRowCount").is_some_and(|x| x != "0")
                                || g("totalsRowShown").is_some_and(|x| x == "1")
                                    && g("totalsRowCount").is_some(),
                        })
                    }
                    "tableColumn" => {
                        if let (Some(tt), Some(n)) = (t.as_mut(), g("name")) {
                            tt.columns.push(n);
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    t
}

trait ZeroCheck {
    fn is_none_or_zero(&self) -> bool;
}
impl ZeroCheck for Option<String> {
    /// headerRowCount defaults to 1; "0" disables the header row.
    fn is_none_or_zero(&self) -> bool {
        self.as_deref() != Some("0")
    }
}

/// Read an `.xlsx` file into a model and its source package.
pub fn read_xlsx(bytes: &[u8]) -> Result<(WorkbookModel, XlsxSource)> {
    let pkg = Package::from_bytes(bytes)?;
    if pkg.kind()? != DocumentKind::Workbook {
        return Err(bad("package", "not a spreadsheet"));
    }
    let wb_part = pkg.main_part()?;
    let wb_xml = part_text(&pkg, &wb_part)?;
    let wb_raw = RawDoc::parse(wb_part.as_str(), &wb_xml)?;
    let rels = pkg.relationships(Some(&wb_part))?;
    let sst = match rels
        .first_of_type(REL_SST)
        .map(|r| wb_part.resolve(&r.target))
    {
        Some(Ok(p)) => pkg
            .part(&p)
            .map(prism_openxml::spreadsheetml::parse_shared_strings)
            .transpose()?
            .unwrap_or_default(),
        _ => vec![],
    };
    let num_formats = match rels
        .first_of_type(rel_types::STYLES)
        .map(|r| wb_part.resolve(&r.target))
    {
        Some(Ok(p)) => pkg
            .part(&p)
            .map(parse_styles)
            .transpose()?
            .unwrap_or_else(|| vec!["General".into()]),
        _ => vec!["General".into()],
    };
    let mut model = WorkbookModel {
        num_formats: num_formats.clone(),
        ..Default::default()
    };
    model.date1904 = wb_raw
        .get("workbookPr")
        .is_some_and(|x| x.contains(r#"date1904="1""#) || x.contains(r#"date1904="true""#));
    // sheets
    let mut src_sheets = Vec::new();
    let mut sheet_names = Vec::new();
    if let Some(sheets) = wb_raw.get("sheets") {
        let mut r = Reader::from_str(sheets);
        loop {
            match r.read_event().map_err(|e| bad("workbook", e))? {
                Event::Start(e) | Event::Empty(e) if name_of(&e) == "sheet" => {
                    let a = attrs_of(&e);
                    let g = |k: &str| a.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
                    sheet_names.push((
                        g("name").unwrap_or_default(),
                        g("id").unwrap_or_default(),
                        g("state").is_some_and(|s| s != "visible"),
                    ));
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
    for (name, rid, hidden) in sheet_names {
        let Some(rel) = rels.by_id(&rid) else {
            continue;
        };
        if rel.rel_type != rel_types::WORKSHEET {
            continue; // chartsheets/dialogsheets are kept as raw parts only
        }
        let part = wb_part.resolve(&rel.target)?;
        let xml = part_text(&pkg, &part)?;
        let mut sm = SheetModel {
            name,
            hidden,
            source: Some(src_sheets.len()),
            ..Default::default()
        };
        let raw = read_sheet(&xml, part.as_str(), &sst, &mut sm)?;
        // tables
        if let Ok(srels) = pkg.relationships(Some(&part)) {
            for tr in srels.of_type(REL_TABLE) {
                if let Ok(tp) = part.resolve(&tr.target) {
                    if let Some(t) = pkg
                        .part(&tp)
                        .and_then(|b| read_table(&String::from_utf8_lossy(b)))
                    {
                        sm.tables.push(t);
                    }
                }
            }
        }
        src_sheets.push(SourceSheet {
            part,
            raw,
            frozen: sm.frozen,
            col_widths: sm.col_widths.clone(),
            merges: sm.merges.clone(),
        });
        model.sheets.push(sm);
    }
    // defined names
    if let Some(dn) = wb_raw.get("definedNames") {
        let mut r = Reader::from_str(dn);
        let mut cur: Option<DefinedName> = None;
        loop {
            match r.read_event().map_err(|e| bad("workbook", e))? {
                Event::Start(e) if name_of(&e) == "definedName" => {
                    let a = attrs_of(&e);
                    let g = |k: &str| a.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
                    cur = Some(DefinedName {
                        name: g("name").unwrap_or_default(),
                        formula: String::new(),
                        sheet: g("localSheetId").and_then(|x| x.parse().ok()),
                    });
                }
                Event::Text(t) => {
                    if let Some(c) = cur.as_mut() {
                        c.formula.push_str(&text_of(&t));
                    }
                }
                Event::GeneralRef(g) => {
                    if let Some(c) = cur.as_mut() {
                        c.formula.push_str(&ref_text(&g));
                    }
                }
                Event::End(e) if e.local_name().into_inner() == "definedName" => {
                    if let Some(mut c) = cur.take() {
                        if let Ok(e) = parser::parse(&c.formula, CellRef::new(0, 0)) {
                            c.formula = parser::to_formula(&e, &WriteOptions::default());
                        }
                        model.names.push(c);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
    if model.sheets.is_empty() {
        return Err(bad("workbook", "no worksheets"));
    }
    let source = XlsxSource {
        package: pkg,
        workbook_part: wb_part,
        workbook_raw: wb_raw,
        sheets: src_sheets,
        xf_count: num_formats.len(),
        names: model.names.clone(),
    };
    Ok((model, source))
}

// ---- writing --------------------------------------------------------------------------------

const SHEET_ORDER: &[&str] = &[
    "sheetPr",
    "dimension",
    "sheetViews",
    "sheetFormatPr",
    "cols",
    "sheetData",
    "sheetCalcPr",
    "sheetProtection",
    "protectedRanges",
    "scenarios",
    "autoFilter",
    "sortState",
    "dataConsolidate",
    "customSheetViews",
    "mergeCells",
    "phoneticPr",
    "conditionalFormatting",
    "dataValidations",
    "hyperlinks",
    "printOptions",
    "pageMargins",
    "pageSetup",
    "headerFooter",
    "rowBreaks",
    "colBreaks",
    "customProperties",
    "cellWatches",
    "ignoredErrors",
    "smartTags",
    "drawing",
    "legacyDrawing",
    "legacyDrawingHF",
    "picture",
    "oleObjects",
    "controls",
    "webPublishItems",
    "tableParts",
    "extLst",
];

const WORKBOOK_ORDER: &[&str] = &[
    "fileVersion",
    "fileSharing",
    "workbookPr",
    "workbookProtection",
    "bookViews",
    "sheets",
    "functionGroups",
    "externalReferences",
    "definedNames",
    "calcPr",
    "oleSize",
    "customWorkbookViews",
    "pivotCaches",
    "smartTagPr",
    "smartTagTypes",
    "webPublishing",
    "fileRecoveryPr",
    "webPublishObjects",
    "extLst",
];

fn order_index(order: &[&str], name: &str) -> usize {
    order
        .iter()
        .position(|n| *n == name)
        .unwrap_or(order.len() - 1)
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            c => o.push(c),
        }
    }
    o
}

struct Sst {
    list: Vec<String>,
    index: HashMap<String, usize>,
    count: usize,
}

impl Sst {
    fn id(&mut self, s: &str) -> usize {
        self.count += 1;
        if let Some(i) = self.index.get(s) {
            return *i;
        }
        self.list.push(s.to_string());
        self.index.insert(s.to_string(), self.list.len() - 1);
        self.list.len() - 1
    }

    fn xml(&self) -> String {
        let mut x = format!(
            r#"{}<sst xmlns="{S_NS}" count="{}" uniqueCount="{}">"#,
            prism_openxml::xml::XML_DECL,
            self.count,
            self.list.len()
        );
        for s in &self.list {
            let space = if s.starts_with(char::is_whitespace)
                || s.ends_with(char::is_whitespace)
                || s.contains('\n')
            {
                r#" xml:space="preserve""#
            } else {
                ""
            };
            x.push_str(&format!("<si><t{space}>{}</t></si>", esc(s)));
        }
        x.push_str("</sst>");
        x
    }
}

fn value_xml(v: &Value, sst: &mut Sst, formula: bool) -> (Option<&'static str>, Option<String>) {
    match v {
        Value::Number(n) if n.is_finite() => {
            (None, Some(crate::value::format_number_roundtrip(*n)))
        }
        Value::Bool(b) => (Some("b"), Some(if *b { "1" } else { "0" }.into())),
        Value::Error(e) => (Some("e"), Some(e.code().into())),
        Value::Text(t) if formula => (Some("str"), Some(esc(t))),
        Value::Text(t) => (Some("s"), Some(sst.id(t).to_string())),
        _ => (None, None),
    }
}

fn sheet_data(
    wb: &Workbook,
    si: usize,
    sst: &mut Sst,
    dynamic: &mut bool,
) -> (String, Option<Rect>) {
    let sh = &wb.sheets[si];
    // all positions to write: cells + spilled cells
    let mut rows: BTreeMap<u32, BTreeMap<u32, String>> = BTreeMap::new();
    let opts = WriteOptions {
        file_prefixes: true,
        ..Default::default()
    };
    for (k, cell) in &sh.cells {
        let style = if cell.style != 0 {
            format!(r#" s="{}""#, cell.style)
        } else {
            String::new()
        };
        let r = a1(*k);
        let xml = match &cell.input {
            Input::Formula { expr, .. } => {
                let ftext = esc(&parser::to_formula(expr, &opts));
                let shown = wb.value_at(si, *k);
                let (t, v) = value_xml(&shown, sst, true);
                let t = t.map(|t| format!(r#" t="{t}""#)).unwrap_or_default();
                let v = v.map(|v| format!("<v>{v}</v>")).unwrap_or_default();
                if let Some(rect) = sh.spills.get(k) {
                    *dynamic = true;
                    format!(
                        r#"<c r="{r}"{style}{t} cm="1"><f t="array" ref="{}">{ftext}</f>{v}</c>"#,
                        rect_a1(rect)
                    )
                } else {
                    format!(r#"<c r="{r}"{style}{t}><f>{ftext}</f>{v}</c>"#)
                }
            }
            Input::Value(v) => {
                let (t, val) = value_xml(v, sst, false);
                let t = t.map(|t| format!(r#" t="{t}""#)).unwrap_or_default();
                match val {
                    Some(val) => format!(r#"<c r="{r}"{style}{t}><v>{val}</v></c>"#),
                    None => format!(r#"<c r="{r}"{style}/>"#),
                }
            }
        };
        rows.entry(k.row).or_default().insert(k.col, xml);
    }
    for k in sh.spill_owner.keys() {
        if sh.cells.contains_key(k) {
            continue;
        }
        let v = wb.value_at(si, *k);
        let (t, val) = value_xml(&v, sst, true);
        let t = t.map(|t| format!(r#" t="{t}""#)).unwrap_or_default();
        let xml = match val {
            Some(val) => format!(r#"<c r="{}"{t}><v>{val}</v></c>"#, a1(*k)),
            None => continue,
        };
        rows.entry(k.row).or_default().insert(k.col, xml);
    }
    for r in sh.row_heights.keys() {
        rows.entry(*r).or_default();
    }
    let mut out = String::from("<sheetData>");
    for (r, cells) in &rows {
        let ht = sh
            .row_heights
            .get(r)
            .map(|h| format!(r#" ht="{h}" customHeight="1""#))
            .unwrap_or_default();
        if cells.is_empty() {
            out.push_str(&format!(r#"<row r="{}"{ht}/>"#, r + 1));
            continue;
        }
        out.push_str(&format!(r#"<row r="{}"{ht}>"#, r + 1));
        for x in cells.values() {
            out.push_str(x);
        }
        out.push_str("</row>");
    }
    out.push_str("</sheetData>");
    (out, sh.used_rect())
}

fn cols_xml(widths: &BTreeMap<u32, f64>) -> Option<String> {
    if widths.is_empty() {
        return None;
    }
    let mut out = String::from("<cols>");
    let v: Vec<(u32, f64)> = widths.iter().map(|(a, b)| (*a, *b)).collect();
    let mut i = 0;
    while i < v.len() {
        let (start, w) = v[i];
        let mut j = i;
        while j + 1 < v.len() && v[j + 1].0 == v[j].0 + 1 && v[j + 1].1 == w {
            j += 1;
        }
        out.push_str(&format!(
            r#"<col min="{}" max="{}" width="{w}" customWidth="1"/>"#,
            start + 1,
            v[j].0 + 1
        ));
        i = j + 1;
    }
    out.push_str("</cols>");
    Some(out)
}

fn views_xml(frozen: (u32, u32), first_sheet: bool) -> String {
    let sel = if first_sheet {
        r#" tabSelected="1""#
    } else {
        ""
    };
    if frozen == (0, 0) {
        return format!(r#"<sheetViews><sheetView{sel} workbookViewId="0"/></sheetViews>"#);
    }
    let (r, c) = frozen;
    let top_left = a1(CellRef::new(r, c));
    let pane = match (r > 0, c > 0) {
        (true, true) => "bottomRight",
        (true, false) => "bottomLeft",
        _ => "topRight",
    };
    let xs = if c > 0 {
        format!(r#" xSplit="{c}""#)
    } else {
        String::new()
    };
    let ys = if r > 0 {
        format!(r#" ySplit="{r}""#)
    } else {
        String::new()
    };
    format!(
        r#"<sheetViews><sheetView{sel} workbookViewId="0"><pane{xs}{ys} topLeftCell="{top_left}" activePane="{pane}" state="frozen"/><selection pane="{pane}"/></sheetView></sheetViews>"#
    )
}

const METADATA_XML: &str = r#"<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:xda="http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray"><metadataTypes count="1"><metadataType name="XLDAPR" minSupportedVersion="120000" copy="1" pasteAll="1" pasteValues="1" merge="1" splitFirst="1" rowColShift="1" clearFormats="1" clearComments="1" assign="1" coerce="1" cellMeta="1"/></metadataTypes><futureMetadata name="XLDAPR" count="1"><bk><extLst><ext uri="{bdbb8cdc-fa1e-496e-a857-3c3f30c029c3}"><xda:dynamicArrayProperties fDynamic="1" fCollapsed="0"/></ext></extLst></bk></futureMetadata><cellMetadata count="1"><bk><rc t="1" v="0"/></bk></cellMetadata></metadata>"#;

/// Write a model to `.xlsx`, reusing `source` for lossless round-trips.
pub fn write_xlsx(model: &WorkbookModel, source: Option<&XlsxSource>, now: f64) -> Result<Vec<u8>> {
    let mut wb = Workbook::from_model(model);
    wb.recalc(now);
    // async (=PY) cells keep their cached value instead of #BUSY!
    for (si, sm) in model.sheets.iter().enumerate() {
        for c in sm.cells.iter().filter(|c| c.f.is_some()) {
            let k = CellRef::new(c.r, c.c);
            if matches!(
                wb.value_at(si, k),
                Value::Error(crate::value::ErrorKind::Busy)
            ) {
                if let (Some(v), Some(cell)) = (&c.v, wb.sheets[si].cells.get_mut(&k)) {
                    cell.value = v.to_value();
                }
            }
        }
    }
    let mut pkg = match source {
        Some(s) => s.package.clone(),
        None => prism_openxml::templates::blank(DocumentKind::Workbook),
    };
    let wb_part = match source {
        Some(s) => s.workbook_part.clone(),
        None => pkg.main_part()?,
    };
    let wb_raw = match source {
        Some(s) => s.workbook_raw.clone(),
        None => RawDoc::parse("workbook", &part_text(&pkg, &wb_part)?)?,
    };
    let mut rels = pkg.relationships(Some(&wb_part))?;
    // stale calculation chain
    let chain: Vec<_> = rels
        .of_type(REL_CALC_CHAIN)
        .filter_map(|r| wb_part.resolve(&r.target).ok().map(|p| (r.id.clone(), p)))
        .collect();
    let mut new_rels = Relationships::default();
    for r in rels.iter() {
        if chain.iter().any(|(id, _)| *id == r.id) {
            continue;
        }
        new_rels.push(r.clone());
    }
    for (_, p) in chain {
        pkg.remove_part(&p);
    }
    rels = new_rels;

    // worksheet parts: reuse source parts, drop removed ones, create new ones
    let used_sources: Vec<usize> = model.sheets.iter().filter_map(|s| s.source).collect();
    let mut sheet_rel_ids: Vec<String> = Vec::new();
    let mut sst = Sst {
        list: vec![],
        index: HashMap::new(),
        count: 0,
    };
    let mut dynamic = false;
    let mut next_sheet_no = 1usize;
    let first_free = |pkg: &Package, n: &mut usize| loop {
        let name = format!("/xl/worksheets/sheet{n}.xml");
        *n += 1;
        let p = PartName::new(&name).expect("static");
        if !pkg.has_part(&p) {
            return p;
        }
    };
    if let Some(src) = source {
        for (i, ss) in src.sheets.iter().enumerate() {
            if !used_sources.contains(&i) {
                if let Some(id) = rel_id_for(&rels, &wb_part, &ss.part) {
                    rels.remove(&id);
                }
                pkg.remove_part(&ss.part);
            }
        }
    } else {
        // template sheet1 is replaced by our own sheets
        let p = PartName::new("/xl/worksheets/sheet1.xml").expect("static");
        if let Some(id) = rel_id_for(&rels, &wb_part, &p) {
            rels.remove(&id);
        }
        pkg.remove_part(&p);
    }
    for (si, sm) in model.sheets.iter().enumerate() {
        let src_sheet = sm.source.and_then(|i| source.and_then(|s| s.sheets.get(i)));
        let part = match src_sheet {
            Some(ss) => ss.part.clone(),
            None => first_free(&pkg, &mut next_sheet_no),
        };
        let (data, used) = sheet_data(&wb, si, &mut sst, &mut dynamic);
        let sh = &wb.sheets[si];
        let mut children: Vec<(String, String)> = src_sheet
            .map(|s| s.raw.children.clone())
            .unwrap_or_default();
        let mut set = |name: &str, xml: Option<String>| {
            children.retain(|(n, _)| n != name);
            if let Some(x) = xml {
                children.push((name.to_string(), x));
            }
        };
        set("sheetData", Some(data));
        set(
            "dimension",
            Some(format!(
                r#"<dimension ref="{}"/>"#,
                used.map(|r| rect_a1(&r)).unwrap_or_else(|| "A1".into())
            )),
        );
        let widths_changed = src_sheet.is_none_or_changed(|s| s.col_widths != sm.col_widths);
        if widths_changed {
            set("cols", cols_xml(&sh.col_widths));
        }
        if src_sheet.is_none_or_changed(|s| s.merges != sm.merges) {
            let m = if sh.merges.is_empty() {
                None
            } else {
                Some(format!(
                    r#"<mergeCells count="{}">{}</mergeCells>"#,
                    sh.merges.len(),
                    sh.merges
                        .iter()
                        .map(|r| format!(r#"<mergeCell ref="{}"/>"#, rect_a1(r)))
                        .collect::<String>()
                ))
            };
            set("mergeCells", m);
        }
        if src_sheet
            .is_none_or_changed(|s| s.frozen != sm.frozen || s.raw.get("sheetViews").is_none())
        {
            set("sheetViews", Some(views_xml(sh.frozen, si == 0)));
        }
        children.sort_by_key(|(n, _)| order_index(SHEET_ORDER, n));
        let head = src_sheet.map(|s| s.raw.head_open()).unwrap_or_else(|| {
            format!(
                r#"{}<worksheet xmlns="{S_NS}" xmlns:r="{R_NS}">"#,
                prism_openxml::xml::XML_DECL
            )
        });
        let body: String = children.into_iter().map(|(_, x)| x).collect();
        let xml = format!("{head}{body}</worksheet>");
        pkg.set_part(part.clone(), xml.into_bytes(), Some(CT_SHEET));
        let id = match rel_id_for(&rels, &wb_part, &part) {
            Some(id) => id,
            None => rels.add(
                rel_types::WORKSHEET,
                &relative(&wb_part, &part),
                TargetMode::Internal,
            ),
        };
        sheet_rel_ids.push(id);
    }
    // shared strings
    let sst_part = match rels
        .first_of_type(REL_SST)
        .and_then(|r| wb_part.resolve(&r.target).ok())
    {
        Some(p) => p,
        None => {
            rels.add(REL_SST, "sharedStrings.xml", TargetMode::Internal);
            PartName::new("/xl/sharedStrings.xml").expect("static")
        }
    };
    pkg.set_part(sst_part, sst.xml().into_bytes(), Some(CT_SST));
    // dynamic-array metadata
    if dynamic && rels.first_of_type(REL_META).is_none() {
        rels.add(REL_META, "metadata.xml", TargetMode::Internal);
        pkg.set_part(
            PartName::new("/xl/metadata.xml").expect("static"),
            format!("{}{METADATA_XML}", prism_openxml::xml::XML_DECL).into_bytes(),
            Some(CT_META),
        );
    }
    // styles: append formats added in the editor
    if let Some(sp) = rels
        .first_of_type(rel_types::STYLES)
        .and_then(|r| wb_part.resolve(&r.target).ok())
    {
        let existing = source.map(|s| s.xf_count).unwrap_or(1);
        if model.num_formats.len() > existing {
            let xml = part_text(&pkg, &sp)?;
            pkg.set_part(
                sp,
                extend_styles(&xml, existing, &model.num_formats).into_bytes(),
                None,
            );
        }
    }
    // workbook.xml: regenerate sheets and definedNames
    let mut children = wb_raw.children.clone();
    let sheets_xml = {
        let mut s = String::from("<sheets>");
        for (i, sm) in model.sheets.iter().enumerate() {
            let state = if sm.hidden { r#" state="hidden""# } else { "" };
            s.push_str(&format!(
                r#"<sheet name="{}" sheetId="{}"{state} r:id="{}"/>"#,
                escape_attr(&sm.name),
                i + 1,
                sheet_rel_ids[i]
            ));
        }
        s.push_str("</sheets>");
        s
    };
    children.retain(|(n, _)| n != "sheets");
    children.push(("sheets".into(), sheets_xml));
    let names_changed = source.is_none_or_changed(|s| s.names != model.names);
    if names_changed {
        children.retain(|(n, _)| n != "definedNames");
        if !model.names.is_empty() {
            let mut s = String::from("<definedNames>");
            for n in &model.names {
                let local = n
                    .sheet
                    .map(|i| format!(r#" localSheetId="{i}""#))
                    .unwrap_or_default();
                let f = parser::parse(&n.formula, CellRef::new(0, 0))
                    .map(|e| {
                        parser::to_formula(
                            &e,
                            &WriteOptions {
                                file_prefixes: true,
                                ..Default::default()
                            },
                        )
                    })
                    .unwrap_or_else(|_| n.formula.clone());
                s.push_str(&format!(
                    r#"<definedName name="{}"{local}>{}</definedName>"#,
                    escape_attr(&n.name),
                    esc(&f)
                ));
            }
            s.push_str("</definedNames>");
            children.push(("definedNames".into(), s));
        }
    }
    if !children.iter().any(|(n, _)| n == "calcPr") {
        children.push((
            "calcPr".into(),
            r#"<calcPr calcId="191029" fullCalcOnLoad="1"/>"#.into(),
        ));
    }
    children.sort_by_key(|(n, _)| order_index(WORKBOOK_ORDER, n));
    let head = wb_raw.head_open();
    let head = if head.contains("xmlns:r=") {
        head
    } else {
        head.replacen("<workbook", &format!(r#"<workbook xmlns:r="{R_NS}""#), 1)
    };
    let xml = format!(
        "{head}{}</workbook>",
        children.into_iter().map(|(_, x)| x).collect::<String>()
    );
    pkg.set_part(wb_part.clone(), xml.into_bytes(), None);
    pkg.set_relationships(Some(&wb_part), &rels);
    Ok(pkg.to_bytes()?)
}

trait ChangedCheck<T> {
    fn is_none_or_changed(&self, f: impl Fn(&T) -> bool) -> bool;
}
impl<T> ChangedCheck<T> for Option<&T> {
    fn is_none_or_changed(&self, f: impl Fn(&T) -> bool) -> bool {
        match self {
            None => true,
            Some(x) => f(x),
        }
    }
}

fn rel_id_for(rels: &Relationships, from: &PartName, part: &PartName) -> Option<String> {
    let found = rels
        .iter()
        .find(|r| from.resolve(&r.target).ok().as_ref() == Some(part))
        .map(|r| r.id.clone());
    found
}

/// Relationship target of `to` relative to the directory of `from`.
fn relative(from: &PartName, to: &PartName) -> String {
    let dir = from.directory().trim_end_matches('/');
    let t = to.as_str();
    match t.strip_prefix(&format!("{dir}/")) {
        Some(rest) => rest.to_string(),
        None => t.to_string(),
    }
}
