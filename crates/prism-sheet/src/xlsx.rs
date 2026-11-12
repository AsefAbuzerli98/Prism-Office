//! XLSX import/export.
//!
//! Reading produces a [`WorkbookModel`] plus an [`XlsxSource`] that keeps the
//! original package. Writing regenerates only what the model owns
//! (`sheetData`, dimension, columns, merges, frozen panes, sheet list, defined
//! names, shared strings, dynamic-array metadata, appended number formats);
//! every other part and worksheet child is copied through byte-for-byte.

// Regexes in loops are compiled once via `re!` or are cheap one-offs per part.
#![allow(clippy::regex_creation_in_loops)]

use std::collections::{BTreeMap, HashMap};

use prism_openxml::xml::{attrs_of, escape_attr, name_of, ref_text, text_of};
use prism_openxml::{rel_types, DocumentKind, Package, PartName, Relationships, TargetMode};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::addr::{a1, rect_a1, CellRef, Rect};
use crate::model::{CellModel, CellStyle, CellValueModel, SheetModel, WorkbookModel};
use crate::parser::{self, WriteOptions};
use crate::value::Value;
use crate::workbook::{DefinedName, Input, TableDef, Workbook};

/// A literal regex compiled once.
macro_rules! re {
    ($p:expr) => {{
        static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        R.get_or_init(|| regex::Regex::new($p).expect("static regex"))
    }};
}

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
    /// Pivot table parts that were imported into the model.
    pivots: Vec<String>,
    /// Chart parts of charts shown from the file.
    charts: Vec<String>,
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
    connections: Vec<crate::model::DataConnection>,
    /// `<connection>` elements not created by Prism (kept verbatim).
    foreign_connections: Vec<String>,
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

const THEME: [&str; 12] = [
    "FFFFFF", "000000", "E7E6E6", "44546A", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5",
    "70AD47", "0563C1", "954F72",
];

/// `#RRGGBB` from a SpreadsheetML colour element's attributes.
fn color_of(a: &[(String, String)]) -> Option<String> {
    let get = |k: &str| a.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str());
    let base = if let Some(rgb) = get("rgb") {
        let h = rgb.trim_start_matches('#');
        let h = if h.len() == 8 { &h[2..] } else { h };
        u32::from_str_radix(h, 16).ok()?
    } else if let Some(t) = get("theme") {
        u32::from_str_radix(THEME.get(t.parse::<usize>().ok()?)?, 16).ok()?
    } else {
        match get("indexed")?.parse::<u32>().ok()? {
            8 | 64 => 0x000000,
            9 | 65 => 0xFFFFFF,
            10 => 0xFF0000,
            11 => 0x00FF00,
            12 => 0x0000FF,
            13 => 0xFFFF00,
            14 => 0xFF00FF,
            15 => 0x00FFFF,
            _ => return None,
        }
    };
    let tint: f64 = get("tint").and_then(|t| t.parse().ok()).unwrap_or(0.0);
    let ch = |shift: u32| {
        let c = ((base >> shift) & 0xFF) as f64;
        let v = if tint < 0.0 {
            c * (1.0 + tint)
        } else {
            c + (255.0 - c) * tint
        };
        v.round().clamp(0.0, 255.0) as u32
    };
    Some(format!("#{:02X}{:02X}{:02X}", ch(16), ch(8), ch(0)))
}

/// Number-format code and visual style for each cellXfs entry.
fn parse_styles(xml: &[u8]) -> Result<(Vec<String>, Vec<CellStyle>)> {
    let mut r = Reader::from_reader(xml);
    let mut custom: HashMap<u32, String> = HashMap::new();
    let mut fonts: Vec<CellStyle> = Vec::new();
    let mut fills: Vec<Option<String>> = Vec::new();
    let mut borders: Vec<u8> = Vec::new();
    let mut xfs = Vec::new();
    let mut looks = Vec::new();
    let mut section = String::new();
    let mut in_pattern_solid = false;
    let mut cur_border = 0u8;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let ev = r.read_event_into(&mut buf).map_err(|e| bad("styles", e))?;
        let (e, empty) = match &ev {
            Event::Start(e) => (Some(e.clone()), false),
            Event::Empty(e) => (Some(e.clone()), true),
            _ => (None, false),
        };
        if let Some(e) = e {
            let n = name_of(&e);
            let a = attrs_of(&e);
            let get = |k: &str| a.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
            let on = |k: &str| get(k).is_none_or_true();
            match n.as_str() {
                "numFmt" => {
                    if let (Some(id), Some(code)) = (
                        get("numFmtId").and_then(|x| x.parse().ok()),
                        get("formatCode"),
                    ) {
                        custom.insert(id, code);
                    }
                }
                "fonts" | "fills" | "borders" | "cellXfs" | "cellStyleXfs" | "dxfs" => {
                    if !empty {
                        section = n.clone();
                    }
                }
                "font" if section == "fonts" => fonts.push(CellStyle::default()),
                "b" if section == "fonts" => fonts
                    .last_mut()
                    .into_iter()
                    .for_each(|f| f.bold = on("val")),
                "i" if section == "fonts" => fonts
                    .last_mut()
                    .into_iter()
                    .for_each(|f| f.italic = on("val")),
                "strike" if section == "fonts" => fonts
                    .last_mut()
                    .into_iter()
                    .for_each(|f| f.strike = on("val")),
                "u" if section == "fonts" => fonts
                    .last_mut()
                    .into_iter()
                    .for_each(|f| f.underline = get("val").as_deref() != Some("none")),
                "sz" if section == "fonts" => fonts
                    .last_mut()
                    .into_iter()
                    .for_each(|f| f.font_size = get("val").and_then(|v| v.parse().ok())),
                "name" if section == "fonts" => fonts
                    .last_mut()
                    .into_iter()
                    .for_each(|f| f.font_name = get("val")),
                "color" if section == "fonts" => {
                    let c = color_of(&a).filter(|c| c != "#000000");
                    fonts
                        .last_mut()
                        .into_iter()
                        .for_each(|f| f.color = c.clone())
                }
                "fill" if section == "fills" => fills.push(None),
                "patternFill" if section == "fills" => {
                    in_pattern_solid = get("patternType").as_deref() == Some("solid")
                }
                "fgColor" if section == "fills" && in_pattern_solid => {
                    let c = color_of(&a);
                    if let Some(f) = fills.last_mut() {
                        *f = c;
                    }
                }
                "border" if section == "borders" => {
                    cur_border = 0;
                    if empty {
                        borders.push(0);
                    }
                }
                "left" | "right" | "top" | "bottom" | "start" | "end" if section == "borders" => {
                    if get("style").is_some_and(|s| s != "none") {
                        cur_border |= match n.as_str() {
                            "left" | "start" => 1,
                            "right" | "end" => 2,
                            "top" => 4,
                            _ => 8,
                        };
                    }
                }
                "xf" if section == "cellXfs" => {
                    let id: u32 = get("numFmtId").and_then(|x| x.parse().ok()).unwrap_or(0);
                    xfs.push(
                        custom
                            .get(&id)
                            .cloned()
                            .or_else(|| builtin_format(id).map(String::from))
                            .unwrap_or_else(|| "General".into()),
                    );
                    let idx = |k: &str| get(k).and_then(|x| x.parse::<usize>().ok()).unwrap_or(0);
                    let mut look = fonts.get(idx("fontId")).cloned().unwrap_or_default();
                    // the default font is the sheet's base font, not a style
                    if idx("fontId") == 0 {
                        look = CellStyle::default();
                    }
                    look.fill = fills.get(idx("fillId")).cloned().flatten();
                    look.border = borders.get(idx("borderId")).copied().unwrap_or(0);
                    looks.push(look);
                }
                "alignment" if section == "cellXfs" => {
                    if let Some(l) = looks.last_mut() {
                        l.h_align = get("horizontal").filter(|h| h != "general");
                        l.v_align = get("vertical").filter(|v| v != "bottom");
                        l.wrap = get("wrapText").is_some_and(|w| w == "1" || w == "true");
                    }
                }
                _ => {}
            }
        }
        match ev {
            Event::End(e) => {
                let n = e.local_name().into_inner().to_string();
                match n.as_str() {
                    "fonts" | "fills" | "borders" | "cellXfs" | "cellStyleXfs" | "dxfs" => {
                        section.clear()
                    }
                    "border" if section == "borders" => borders.push(cur_border),
                    "patternFill" => in_pattern_solid = false,
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if xfs.is_empty() {
        xfs.push("General".into());
        looks.push(CellStyle::default());
    }
    Ok((xfs, looks))
}

trait AttrBool {
    fn is_none_or_true(&self) -> bool;
}
impl AttrBool for Option<String> {
    fn is_none_or_true(&self) -> bool {
        !matches!(self.as_deref(), Some("0") | Some("false"))
    }
}

fn hex8(c: &str) -> String {
    format!("FF{}", c.trim_start_matches('#').to_ascii_uppercase())
}

/// Insert `insert` before `</tag>` and add `n` to the count attribute.
fn bump(s: &mut String, tag: &str, n: usize, insert: &str) -> bool {
    let open = format!("<{tag}");
    let Some(i) = s
        .match_indices(&open)
        .map(|(i, _)| i)
        .find(|&i| matches!(s.as_bytes().get(i + open.len()), Some(b' ' | b'>' | b'/')))
    else {
        return false;
    };
    let end = s[i..].find('>').map(|e| e + i).unwrap();
    let start_tag = s[i..=end].to_string();
    if start_tag.ends_with("/>") {
        let body = format!(r#"<{tag} count="{n}">{insert}</{tag}>"#);
        s.replace_range(i..=end, &body);
        return true;
    }
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
}

fn count_of(s: &str, child: &str, parent: &str) -> usize {
    let (Some(a), Some(b)) = (
        s.find(&format!("<{parent}")),
        s.find(&format!("</{parent}>")),
    ) else {
        return 0;
    };
    let open = format!("<{child}");
    s[a..b]
        .match_indices(&open)
        .filter(|(i, _)| {
            matches!(
                s.as_bytes()[a + i + open.len()..].first(),
                Some(b' ' | b'>' | b'/')
            )
        })
        .count()
}

/// Append cell formats (number format + font/fill/border/alignment) for
/// styles created in the editor beyond the source's cellXfs.
fn extend_styles(xml: &str, existing: usize, formats: &[String], looks: &[CellStyle]) -> String {
    if formats.len() <= existing {
        return xml.to_string();
    }
    let mut s = xml.to_string();
    let mut next_id = 164u32;
    for cap in regex::Regex::new(r#"numFmtId="(\d+)""#)
        .unwrap()
        .captures_iter(xml)
    {
        if let Ok(n) = cap[1].parse::<u32>() {
            next_id = next_id.max(n + 1);
        }
    }
    let (mut n_fonts, mut n_fills, mut n_borders) = (
        count_of(&s, "font", "fonts"),
        count_of(&s, "fill", "fills"),
        count_of(&s, "border", "borders"),
    );
    let (mut fmts, mut fonts, mut fills, mut borders, mut xfs) = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    let (mut add_fonts, mut add_fills, mut add_borders) = (0, 0, 0);
    let mut ids: HashMap<String, u32> = HashMap::new();
    for (k, code) in formats.iter().enumerate().skip(existing) {
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
        let look = looks.get(k).cloned().unwrap_or_default();
        let font_id = if look.has_font() {
            let mut f = String::from("<font>");
            if look.bold {
                f.push_str("<b/>");
            }
            if look.italic {
                f.push_str("<i/>");
            }
            if look.strike {
                f.push_str("<strike/>");
            }
            if look.underline {
                f.push_str("<u/>");
            }
            f.push_str(&format!(
                r#"<sz val="{}"/>"#,
                look.font_size.unwrap_or(11.0)
            ));
            if let Some(c) = &look.color {
                f.push_str(&format!(r#"<color rgb="{}"/>"#, hex8(c)));
            }
            f.push_str(&format!(
                r#"<name val="{}"/>"#,
                escape_attr(look.font_name.as_deref().unwrap_or("Calibri"))
            ));
            f.push_str("</font>");
            fonts.push_str(&f);
            add_fonts += 1;
            n_fonts += 1;
            n_fonts - 1
        } else {
            0
        };
        let fill_id = match &look.fill {
            Some(c) => {
                fills.push_str(&format!(r#"<fill><patternFill patternType="solid"><fgColor rgb="{}"/><bgColor indexed="64"/></patternFill></fill>"#, hex8(c)));
                add_fills += 1;
                n_fills += 1;
                n_fills - 1
            }
            None => 0,
        };
        let border_id = if look.border != 0 {
            let side = |bit: u8, name: &str| {
                if look.border & bit != 0 {
                    format!(r#"<{name} style="thin"><color auto="1"/></{name}>"#)
                } else {
                    format!("<{name}/>")
                }
            };
            borders.push_str(&format!(
                "<border>{}{}{}{}<diagonal/></border>",
                side(1, "left"),
                side(2, "right"),
                side(4, "top"),
                side(8, "bottom")
            ));
            add_borders += 1;
            n_borders += 1;
            n_borders - 1
        } else {
            0
        };
        let align = if look.h_align.is_some() || look.v_align.is_some() || look.wrap {
            let mut a = String::from("<alignment");
            if let Some(h) = &look.h_align {
                a.push_str(&format!(r#" horizontal="{}""#, escape_attr(h)));
            }
            if let Some(v) = &look.v_align {
                a.push_str(&format!(r#" vertical="{}""#, escape_attr(v)));
            }
            if look.wrap {
                a.push_str(r#" wrapText="1""#);
            }
            a.push_str("/>");
            Some(a)
        } else {
            None
        };
        let flags = format!(
            r#" applyNumberFormat="1"{}{}{}{}"#,
            if font_id > 0 { r#" applyFont="1""# } else { "" },
            if fill_id > 0 { r#" applyFill="1""# } else { "" },
            if border_id > 0 {
                r#" applyBorder="1""#
            } else {
                ""
            },
            if align.is_some() {
                r#" applyAlignment="1""#
            } else {
                ""
            }
        );
        match align {
            Some(a) => xfs.push_str(&format!(r#"<xf numFmtId="{id}" fontId="{font_id}" fillId="{fill_id}" borderId="{border_id}" xfId="0"{flags}>{a}</xf>"#)),
            None => xfs.push_str(&format!(r#"<xf numFmtId="{id}" fontId="{font_id}" fillId="{fill_id}" borderId="{border_id}" xfId="0"{flags}/>"#)),
        }
    }
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
    if add_fonts > 0 {
        bump(&mut s, "fonts", add_fonts, &fonts);
    }
    if add_fills > 0 {
        bump(&mut s, "fills", add_fills, &fills);
    }
    if add_borders > 0 {
        bump(&mut s, "borders", add_borders, &borders);
    }
    bump(&mut s, "cellXfs", formats.len() - existing, &xfs);
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
    read_package(Package::from_bytes(bytes)?)
}

/// Read an already-opened spreadsheet package.
pub fn read_package(pkg: Package) -> Result<(WorkbookModel, XlsxSource)> {
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
    let default_styles = || (vec!["General".to_string()], vec![CellStyle::default()]);
    let (num_formats, looks) = match rels
        .first_of_type(rel_types::STYLES)
        .map(|r| wb_part.resolve(&r.target))
    {
        Some(Ok(p)) => pkg
            .part(&p)
            .map(parse_styles)
            .transpose()?
            .unwrap_or_else(default_styles),
        _ => default_styles(),
    };
    let mut model = WorkbookModel {
        num_formats: num_formats.clone(),
        styles: looks,
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
        sm.charts = read_charts(&pkg, &part, &sm.name);
        sm.pivots = read_pivots(&pkg, &part);
        src_sheets.push(SourceSheet {
            part,
            raw,
            frozen: sm.frozen,
            col_widths: sm.col_widths.clone(),
            merges: sm.merges.clone(),
            pivots: vec![],
            charts: sm.charts.iter().filter_map(|c| c.part.clone()).collect(),
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
    // pivots sharing a cache with another pivot stay unmodelled (passthrough)
    let mut cache_use: HashMap<String, usize> = HashMap::new();
    for sm in &model.sheets {
        for p in &sm.pivots {
            *cache_use
                .entry(p.cache_part.clone().unwrap_or_default())
                .or_default() += 1;
        }
    }
    for (sm, ss) in model.sheets.iter_mut().zip(src_sheets.iter_mut()) {
        sm.pivots
            .retain(|p| cache_use.get(p.cache_part.as_deref().unwrap_or_default()) == Some(&1));
        ss.pivots = sm.pivots.iter().filter_map(|p| p.part.clone()).collect();
    }
    if model.sheets.is_empty() {
        return Err(bad("workbook", "no worksheets"));
    }
    let (connections, foreign_connections) = match rels
        .first_of_type(REL_CONNECTIONS)
        .and_then(|r| wb_part.resolve(&r.target).ok())
    {
        Some(p) => read_connections(&part_text(&pkg, &p)?),
        None => (vec![], vec![]),
    };
    model.connections = connections.clone();
    let source = XlsxSource {
        package: pkg,
        workbook_part: wb_part,
        workbook_raw: wb_raw,
        sheets: src_sheets,
        xf_count: num_formats.len(),
        names: model.names.clone(),
        connections,
        foreign_connections,
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
            Input::Formula { expr, base } => {
                let offset = (
                    k.row as i64 - base.row as i64,
                    k.col as i64 - base.col as i64,
                );
                let ftext = esc(&parser::to_formula(expr, &WriteOptions { offset, ..opts }));
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
    Ok(write_package(model, source, now)?.to_bytes()?)
}

/// Build the output package for a model (see [`write_xlsx`]).
pub fn write_package(
    model: &WorkbookModel,
    source: Option<&XlsxSource>,
    now: f64,
) -> Result<Package> {
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
    let mut next_cache_id = wb_raw
        .get("pivotCaches")
        .map(|x| {
            re!(r#"cacheId="(\d+)""#)
                .captures_iter(x)
                .filter_map(|c| c[1].parse::<u32>().ok())
                .max()
                .unwrap_or(0)
                + 1
        })
        .unwrap_or(1);
    let mut new_caches: Vec<(u32, String)> = Vec::new();
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
        // file charts that were edited or deleted in the editor
        let file_parts: Vec<String> = src_sheet.map(|s| s.charts.clone()).unwrap_or_default();
        let edited: Vec<&crate::model::ChartDef> = sh
            .charts
            .iter()
            .filter(|c| !c.from_file && c.part.as_ref().is_some_and(|p| file_parts.contains(p)))
            .collect();
        let deleted: Vec<&String> = file_parts
            .iter()
            .filter(|p| !sh.charts.iter().any(|c| c.part.as_ref() == Some(*p)))
            .collect();
        if !edited.is_empty() || !deleted.is_empty() {
            let srels = pkg.relationships(Some(&part)).unwrap_or_default();
            for dr in srels.of_type(REL_DRAWING) {
                let Ok(dp) = part.resolve(&dr.target) else {
                    continue;
                };
                let mut dxml = part_text(&pkg, &dp)?;
                let mut drels = pkg.relationships(Some(&dp)).unwrap_or_default();
                let block_of = |dxml: &str, rid: &str| {
                    re!(r"(?s)<(\w+:)?twoCellAnchor\b.*?</(\w+:)?twoCellAnchor>")
                        .find_iter(dxml)
                        .find(|m| m.as_str().contains(&format!(r#"r:id="{rid}""#)))
                        .map(|m| (m.start(), m.end(), m.as_str().to_string()))
                };
                for c in &edited {
                    let Ok(cp) = PartName::new(c.part.as_deref().unwrap_or_default()) else {
                        continue;
                    };
                    let Some(rid) = rel_id_for(&drels, &dp, &cp) else {
                        continue;
                    };
                    let data = crate::chart::chart_data(&wb, si, c);
                    pkg.set_part(
                        cp.clone(),
                        crate::chart::chart_xml(c, &data, &sm.name).into_bytes(),
                        Some(CT_CHART),
                    );
                    if let Some((a, b, block)) = block_of(&dxml, &rid) {
                        let pos = |tag: &str, col: u32, row: u32, block: &str| {
                            re!(r"(?s)<(\w+:)?(from|to)>.*?</(\w+:)?(from|to)>")
                                .replace_all(block, |m: &regex::Captures| {
                                    if &m[2] != tag {
                                        return m[0].to_string();
                                    }
                                    let p = m.get(1).map_or("", |x| x.as_str());
                                    format!("<{p}{tag}><{p}col>{col}</{p}col><{p}colOff>0</{p}colOff><{p}row>{row}</{p}row><{p}rowOff>0</{p}rowOff></{p}{tag}>")
                                })
                                .into_owned()
                        };
                        let nb = pos("from", c.anchor.c0, c.anchor.r0, &block);
                        let nb = pos("to", c.anchor.c1 + 1, c.anchor.r1 + 1, &nb);
                        dxml.replace_range(a..b, &nb);
                    }
                }
                for p in &deleted {
                    let Ok(cp) = PartName::new(p) else { continue };
                    let Some(rid) = rel_id_for(&drels, &dp, &cp) else {
                        continue;
                    };
                    if let Some((a, b, _)) = block_of(&dxml, &rid) {
                        dxml.replace_range(a..b, "");
                    }
                    drels.remove(&rid);
                    pkg.remove_part(&cp);
                    pkg.remove_part(&cp.rels_part());
                }
                pkg.set_part(dp.clone(), dxml.into_bytes(), None);
                pkg.set_relationships(Some(&dp), &drels);
            }
        }
        // charts created in the editor
        let new_charts: Vec<&crate::model::ChartDef> = sh
            .charts
            .iter()
            .filter(|c| !c.from_file && !c.part.as_ref().is_some_and(|p| file_parts.contains(p)))
            .collect();
        if !new_charts.is_empty() {
            let mut srels = pkg.relationships(Some(&part)).unwrap_or_default();
            let existing = children
                .iter()
                .find(|(n, _)| n == "drawing")
                .and_then(|(_, x)| {
                    regex::Regex::new(r#"r:id="([^"]+)""#)
                        .unwrap()
                        .captures(x)
                        .map(|c| c[1].to_string())
                });
            let drawing = match existing
                .as_ref()
                .and_then(|id| srels.by_id(id))
                .and_then(|r| part.resolve(&r.target).ok())
            {
                Some(p) => p,
                None => {
                    let p = free_part(&pkg, "/xl/drawings/drawing", ".xml");
                    let id = srels.add(REL_DRAWING, &relative_any(&part, &p), TargetMode::Internal);
                    children.retain(|(n, _)| n != "drawing");
                    children.push(("drawing".into(), format!(r#"<drawing r:id="{id}"/>"#)));
                    pkg.set_part(
                        p.clone(),
                        format!("{}</xdr:wsDr>", crate::chart::DRAWING_OPEN).into_bytes(),
                        Some(CT_DRAWING),
                    );
                    p
                }
            };
            pkg.set_relationships(Some(&part), &srels);
            let mut dxml = part_text(&pkg, &drawing)?;
            let mut drels = pkg.relationships(Some(&drawing)).unwrap_or_default();
            for (k, c) in new_charts.iter().enumerate() {
                let cp = free_part(&pkg, "/xl/charts/chart", ".xml");
                let data = crate::chart::chart_data(&wb, si, c);
                pkg.set_part(
                    cp.clone(),
                    crate::chart::chart_xml(c, &data, &sm.name).into_bytes(),
                    Some(CT_CHART),
                );
                let rid = drels.add(
                    REL_CHART,
                    &relative_any(&drawing, &cp),
                    TargetMode::Internal,
                );
                let anchor = crate::chart::anchor_xml(
                    1000 + k,
                    &format!("Chart {}", k + 1),
                    &c.anchor,
                    &rid,
                );
                // the drawing may use a prefix other than xdr; insert before its closing tag
                let close = dxml.rfind("</").unwrap_or(dxml.len());
                let prefix = dxml[close + 2..]
                    .split(':')
                    .next()
                    .filter(|p| !p.contains('>'))
                    .unwrap_or("xdr")
                    .to_string();
                let anchor = if prefix == "xdr" {
                    anchor
                } else {
                    anchor.replace("xdr:", &format!("{prefix}:"))
                };
                dxml.insert_str(close, &anchor);
            }
            if !dxml.contains("xmlns:a=") {
                dxml = dxml.replacen(
                    ":wsDr ",
                    r#":wsDr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
                    1,
                );
            }
            if !dxml.contains("xmlns:r=") {
                dxml = dxml.replacen(":wsDr ", &format!(r#":wsDr xmlns:r="{R_NS}" "#), 1);
            }
            pkg.set_part(drawing.clone(), dxml.into_bytes(), None);
            pkg.set_relationships(Some(&drawing), &drels);
        }
        // classic PivotTables
        {
            let mut srels = pkg.relationships(Some(&part)).unwrap_or_default();
            if let Some(ss) = src_sheet {
                for pp in &ss.pivots {
                    if !sh
                        .pivots
                        .iter()
                        .any(|p| p.part.as_deref() == Some(pp.as_str()))
                    {
                        if let Ok(pn) = PartName::new(pp) {
                            if let Some(id) = rel_id_for(&srels, &part, &pn) {
                                srels.remove(&id);
                            }
                            pkg.remove_part(&pn);
                            pkg.remove_part(&pn.rels_part());
                        }
                    }
                }
            }
            for p in &sh.pivots {
                let Ok(t) = crate::pivot::compute(&wb, p) else {
                    continue;
                };
                let existing = p
                    .part
                    .as_deref()
                    .zip(p.cache_part.as_deref())
                    .and_then(|(a, b)| Some((PartName::new(a).ok()?, PartName::new(b).ok()?)))
                    .filter(|(a, _)| pkg.has_part(a));
                match existing {
                    Some((tp, cp)) => {
                        let old = part_text(&pkg, &tp)?;
                        let cache_id = re!(r#"cacheId="(\d+)""#)
                            .captures(&old)
                            .and_then(|c| c[1].parse().ok())
                            .unwrap_or(1);
                        pkg.set_part(
                            cp,
                            crate::pivot::cache_definition_xml(p, &t, &wb).into_bytes(),
                            Some(CT_PIVOT_CACHE),
                        );
                        pkg.set_part(
                            tp,
                            crate::pivot::table_definition_xml(p, &t, cache_id).into_bytes(),
                            Some(CT_PIVOT_TABLE),
                        );
                    }
                    None => {
                        let cp = free_part(&pkg, "/xl/pivotCache/pivotCacheDefinition", ".xml");
                        pkg.set_part(
                            cp.clone(),
                            crate::pivot::cache_definition_xml(p, &t, &wb).into_bytes(),
                            Some(CT_PIVOT_CACHE),
                        );
                        let cache_id = next_cache_id;
                        next_cache_id += 1;
                        let rid = rels.add(
                            REL_PIVOT_CACHE,
                            &relative_any(&wb_part, &cp),
                            TargetMode::Internal,
                        );
                        new_caches.push((cache_id, rid));
                        let tp = free_part(&pkg, "/xl/pivotTables/pivotTable", ".xml");
                        pkg.set_part(
                            tp.clone(),
                            crate::pivot::table_definition_xml(p, &t, cache_id).into_bytes(),
                            Some(CT_PIVOT_TABLE),
                        );
                        let mut trels = Relationships::default();
                        trels.add(
                            REL_PIVOT_CACHE,
                            &relative_any(&tp, &cp),
                            TargetMode::Internal,
                        );
                        pkg.set_relationships(Some(&tp), &trels);
                        srels.add(
                            REL_PIVOT_TABLE,
                            &relative_any(&part, &tp),
                            TargetMode::Internal,
                        );
                    }
                }
            }
            pkg.set_relationships(Some(&part), &srels);
        }
        children.sort_by_key(|(n, _)| order_index(SHEET_ORDER, n));
        let head = src_sheet.map(|s| s.raw.head_open()).unwrap_or_else(|| {
            format!(
                r#"{}<worksheet xmlns="{S_NS}" xmlns:r="{R_NS}">"#,
                prism_openxml::xml::XML_DECL
            )
        });
        let head = if head.contains("xmlns:r=") {
            head
        } else {
            head.replacen("<worksheet", &format!(r#"<worksheet xmlns:r="{R_NS}""#), 1)
        };
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
                extend_styles(&xml, existing, &model.num_formats, &model.styles).into_bytes(),
                None,
            );
        }
    }
    // data connections
    let src_conns = source.map(|s| s.connections.clone()).unwrap_or_default();
    let foreign = source
        .map(|s| s.foreign_connections.clone())
        .unwrap_or_default();
    if model.connections != src_conns {
        let existing = rels
            .first_of_type(REL_CONNECTIONS)
            .and_then(|r| wb_part.resolve(&r.target).ok().map(|p| (r.id.clone(), p)));
        if model.connections.is_empty() && foreign.is_empty() {
            if let Some((id, p)) = existing {
                rels.remove(&id);
                pkg.remove_part(&p);
            }
        } else {
            let p = match existing {
                Some((_, p)) => p,
                None => {
                    rels.add(REL_CONNECTIONS, "connections.xml", TargetMode::Internal);
                    PartName::new("/xl/connections.xml").expect("static")
                }
            };
            pkg.set_part(
                p,
                connections_xml(&model.connections, &foreign).into_bytes(),
                Some(CT_CONNECTIONS),
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
    if !new_caches.is_empty() {
        let entries: String = new_caches
            .iter()
            .map(|(id, rid)| format!(r#"<pivotCache cacheId="{id}" r:id="{rid}"/>"#))
            .collect();
        match children.iter_mut().find(|(n, _)| n == "pivotCaches") {
            Some((_, x)) => {
                let at = x.rfind("</").unwrap_or(x.len());
                x.insert_str(at, &entries);
            }
            None => children.push((
                "pivotCaches".into(),
                format!("<pivotCaches>{entries}</pivotCaches>"),
            )),
        }
    }
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
    Ok(pkg)
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

const REL_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const CT_DRAWING: &str = "application/vnd.openxmlformats-officedocument.drawing+xml";
const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";

/// First unused part name `{stem}{n}{ext}`.
fn free_part(pkg: &Package, stem: &str, ext: &str) -> PartName {
    (1..)
        .map(|n| PartName::new(&format!("{stem}{n}{ext}")).expect("static"))
        .find(|p| !pkg.has_part(p))
        .unwrap()
}

/// Relative target from `from`'s directory to `to` (with `../` as needed).
fn relative_any(from: &PartName, to: &PartName) -> String {
    let a: Vec<&str> = from
        .directory()
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let b: Vec<&str> = to.as_str().trim_start_matches('/').split('/').collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = std::iter::repeat("..".to_string())
        .take(a.len() - common)
        .collect();
    parts.extend(b[common..].iter().map(|s| s.to_string()));
    parts.join("/")
}

/// Charts in a sheet's drawing (type, title, data range, anchor).
fn read_charts(
    pkg: &Package,
    sheet_part: &PartName,
    sheet_name: &str,
) -> Vec<crate::model::ChartDef> {
    let mut out = Vec::new();
    let Ok(srels) = pkg.relationships(Some(sheet_part)) else {
        return out;
    };
    for dr in srels.of_type(REL_DRAWING) {
        let Ok(dp) = sheet_part.resolve(&dr.target) else {
            continue;
        };
        let Ok(dxml) = part_text(pkg, &dp) else {
            continue;
        };
        let drels = pkg.relationships(Some(&dp)).unwrap_or_default();
        let anchor_re = re!(r"(?s)<(\w+:)?twoCellAnchor.*?</(\w+:)?twoCellAnchor>");
        let num = |x: &str, tag: &str| {
            regex::Regex::new(&format!(r"(?s)<(?:\w+:)?{tag}>.*?<(?:\w+:)?col>(\d+)</(?:\w+:)?col>.*?<(?:\w+:)?row>(\d+)</(?:\w+:)?row>")).unwrap().captures(x).map(|c| (c[1].parse::<u32>().unwrap_or(0), c[2].parse::<u32>().unwrap_or(0)))
        };
        for m in anchor_re.find_iter(&dxml) {
            let a = m.as_str();
            let Some(rid) = regex::Regex::new(r#"chart[^>]*r:id="([^"]+)""#)
                .unwrap()
                .captures(a)
                .map(|c| c[1].to_string())
            else {
                continue;
            };
            let (Some((c0, r0)), Some((c1, r1))) = (num(a, "from"), num(a, "to")) else {
                continue;
            };
            let Some(cp) = drels.by_id(&rid).and_then(|r| dp.resolve(&r.target).ok()) else {
                continue;
            };
            let Ok(cx) = part_text(pkg, &cp) else {
                continue;
            };
            let kind = [
                "barChart",
                "bar3DChart",
                "lineChart",
                "line3DChart",
                "pieChart",
                "pie3DChart",
                "doughnutChart",
                "areaChart",
                "scatterChart",
            ]
            .iter()
            .find(|k| cx.contains(&format!(":{k}>")) || cx.contains(&format!("<{k}>")))
            .map(|k| match *k {
                "barChart" | "bar3DChart" if cx.contains(r#"barDir val="bar""#) => "bar",
                "barChart" | "bar3DChart" => "column",
                "lineChart" | "line3DChart" => "line",
                "areaChart" => "area",
                "scatterChart" => "scatter",
                _ => "pie",
            })
            .unwrap_or("column");
            let title = re!(r"(?s)<(?:\w+:)?title>.*?</(?:\w+:)?title>")
                .find(&cx)
                .map(|t| {
                    re!(r"<(?:\w+:)?t>([^<]*)</")
                        .captures_iter(t.as_str())
                        .map(|c| unescape(&c[1]))
                        .collect::<String>()
                })
                .unwrap_or_default();
            // union of the series formulas on this sheet
            let mut data: Option<Rect> = None;
            for f in re!(r"<(?:\w+:)?f>([^<]+)</").captures_iter(&cx) {
                let text = unescape(&f[1]);
                let (sh, rng) = text.rsplit_once('!').unwrap_or(("", &text));
                let sh = sh.trim_matches('\'');
                if !sh.is_empty() && !sh.eq_ignore_ascii_case(sheet_name) {
                    continue;
                }
                if let Some(r) = parse_rect(&rng.replace('$', "")) {
                    data = Some(match data {
                        Some(d) => Rect {
                            r0: d.r0.min(r.r0),
                            c0: d.c0.min(r.c0),
                            r1: d.r1.max(r.r1),
                            c1: d.c1.max(r.c1),
                        },
                        None => r,
                    });
                }
            }
            if let Some(data) = data {
                out.push(crate::model::ChartDef {
                    kind: kind.into(),
                    title,
                    data,
                    anchor: Rect {
                        r0,
                        c0,
                        r1: r1.saturating_sub(1).max(r0),
                        c1: c1.saturating_sub(1).max(c0),
                    },
                    from_file: true,
                    part: Some(cp.as_str().to_string()),
                });
            }
        }
    }
    out
}

const REL_PIVOT_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable";
const REL_PIVOT_CACHE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
const CT_PIVOT_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";
const CT_PIVOT_CACHE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";

/// PivotTables simple enough for the model: one row field, at most one
/// column field and one data field over a worksheet range.
fn read_pivots(pkg: &Package, sheet_part: &PartName) -> Vec<crate::model::PivotDef> {
    let mut out = Vec::new();
    let Ok(srels) = pkg.relationships(Some(sheet_part)) else {
        return out;
    };
    let attr = |x: &str, name: &str| {
        regex::Regex::new(&format!(r#"\b{name}="([^"]*)""#))
            .unwrap()
            .captures(x)
            .map(|c| unescape(&c[1]))
    };
    let fields = |x: &str, tag: &str| -> Option<Vec<i64>> {
        let block = regex::Regex::new(&format!(r"(?s)<{tag}\b[^>]*>(.*?)</{tag}>"))
            .unwrap()
            .captures(x)?;
        Some(
            re!(r#"<field x="(-?\d+)""#)
                .captures_iter(&block[1])
                .filter_map(|c| c[1].parse().ok())
                .collect(),
        )
    };
    for r in srels.of_type(REL_PIVOT_TABLE) {
        let Ok(tp) = sheet_part.resolve(&r.target) else {
            continue;
        };
        let Ok(tx) = part_text(pkg, &tp) else {
            continue;
        };
        let Some(cp) = pkg.relationships(Some(&tp)).ok().and_then(|rr| {
            rr.first_of_type(REL_PIVOT_CACHE)
                .and_then(|c| tp.resolve(&c.target).ok())
        }) else {
            continue;
        };
        let Ok(cx) = part_text(pkg, &cp) else {
            continue;
        };
        let rows = fields(&tx, "rowFields").unwrap_or_default();
        let cols = fields(&tx, "colFields").unwrap_or_default();
        let data: Vec<(u32, Option<String>)> = re!(r"<dataField\b[^>]*>")
            .find_iter(&tx)
            .filter_map(|m| {
                Some((
                    attr(m.as_str(), "fld")?.parse().ok()?,
                    attr(m.as_str(), "subtotal"),
                ))
            })
            .collect();
        let Some(src) = re!(r"<worksheetSource\b[^>]*>")
            .find(&cx)
            .map(|m| m.as_str().to_string())
        else {
            continue;
        };
        let (Some(sref), Some(ssheet)) = (attr(&src, "ref"), attr(&src, "sheet")) else {
            continue;
        };
        let (Some(source), Some(loc)) = (
            parse_rect(&sref),
            re!(r"<location\b[^>]*>")
                .find(&tx)
                .and_then(|m| attr(m.as_str(), "ref"))
                .and_then(|r| parse_rect(&r)),
        ) else {
            continue;
        };
        if rows.len() != 1
            || rows[0] < 0
            || cols.len() > 1
            || cols.first().is_some_and(|c| *c < 0)
            || data.len() != 1
        {
            continue;
        }
        let function = data[0].1.clone().unwrap_or_else(|| "sum".into());
        if !matches!(
            function.as_str(),
            "sum" | "average" | "count" | "max" | "min"
        ) {
            continue;
        }
        out.push(crate::model::PivotDef {
            name: attr(&tx, "name").unwrap_or_else(|| "PivotTable".into()),
            source_sheet: ssheet,
            source,
            rows: rows[0] as u32,
            cols: cols.first().map(|c| *c as u32),
            values: data[0].0,
            function,
            location: loc.start(),
            extent: Some(loc),
            part: Some(tp.as_str().to_string()),
            cache_part: Some(cp.as_str().to_string()),
        });
    }
    out
}

const REL_CONNECTIONS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/connections";
const CT_CONNECTIONS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml";
const PRISM_TAG: &str = "prism:";

/// Prism connections (metadata in `description`) and foreign `<connection>` elements.
fn read_connections(xml: &str) -> (Vec<crate::model::DataConnection>, Vec<String>) {
    let mut ours = Vec::new();
    let mut foreign = Vec::new();
    for m in re!(r"(?s)<connection\b(?:[^>]*/>|.*?</connection>)").find_iter(xml) {
        let x = m.as_str();
        let desc = re!(r#"description="([^"]*)""#)
            .captures(x.split('>').next().unwrap_or(""))
            .map(|c| unescape(&c[1]));
        match desc
            .as_deref()
            .and_then(|d| d.strip_prefix(PRISM_TAG))
            .and_then(|j| serde_json::from_str::<crate::model::DataConnection>(j).ok())
        {
            Some(c) => ours.push(c),
            None => foreign.push(x.to_string()),
        }
    }
    (ours, foreign)
}

/// `connections.xml`: CSV/JSON as text connections, SQLite/ODBC as ODBC
/// connections; Prism's own metadata rides in `description`.
fn connections_xml(conns: &[crate::model::DataConnection], foreign: &[String]) -> String {
    let max_foreign = foreign
        .iter()
        .filter_map(|f| {
            re!(r#"\bid="(\d+)""#)
                .captures(f)
                .and_then(|c| c[1].parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    let mut x = format!(
        r#"{}<connections xmlns="{S_NS}">"#,
        prism_openxml::xml::XML_DECL
    );
    for f in foreign {
        x.push_str(f);
    }
    for (i, c) in conns.iter().enumerate() {
        let id = max_foreign + 1 + i as u32;
        let meta = escape_attr(&format!(
            "{PRISM_TAG}{}",
            serde_json::to_string(c).unwrap_or_default()
        ));
        let name = escape_attr(&format!("{} ({})", c.sheet, c.kind));
        match c.kind.as_str() {
            "csv" | "json" => x.push_str(&format!(
                r#"<connection id="{id}" name="{name}" type="6" refreshedVersion="6" background="1" saveData="1" description="{meta}"><textPr codePage="65001" sourceFile="{}" delimited="1" comma="1"/></connection>"#,
                escape_attr(&c.source)
            )),
            _ => {
                let conn = if c.kind == "sqlite" { format!("Driver={{SQLite3 ODBC Driver}};Database={}", c.source) } else { c.source.clone() };
                x.push_str(&format!(
                    r#"<connection id="{id}" name="{name}" type="1" refreshedVersion="6" saveData="1" description="{meta}"><dbPr connection="{}" command="{}"/></connection>"#,
                    escape_attr(&conn),
                    escape_attr(c.query.as_deref().unwrap_or(""))
                ));
            }
        }
    }
    x.push_str("</connections>");
    x
}

fn unescape(s: &str) -> String {
    quick_xml::escape::unescape(s)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| s.to_string())
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
