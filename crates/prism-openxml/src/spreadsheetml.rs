//! Streaming SpreadsheetML readers (P1-17).
//!
//! [`SheetReader`] pulls one `<row>` at a time from a worksheet part so
//! million-row sheets can be scanned with memory proportional to one row.

use std::io::BufRead;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::xml::{attr, attrs_of, name_of, ref_text, text_of};
use crate::{Error, Result};

/// Parse an A1-style cell reference into zero-based (column, row).
pub fn parse_cell_ref(r: &str) -> Option<(u32, u32)> {
    let r = r.trim_start_matches('$');
    let split = r.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = r.split_at(split);
    let letters = letters.trim_end_matches('$');
    if letters.is_empty() || letters.len() > 3 {
        return None;
    }
    let mut col: u32 = 0;
    for c in letters.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        col = col * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
    }
    let row: u32 = digits.parse().ok()?;
    if row == 0 {
        return None;
    }
    Some((col - 1, row - 1))
}

#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    Empty,
    Number(f64),
    Bool(bool),
    /// Index into the shared strings table.
    SharedString(u32),
    /// Inline rich/plain string (`t="inlineStr"`) or formula string result (`t="str"`).
    Text(String),
    Error(String),
    /// ISO 8601 date (`t="d"`).
    Date(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// Zero-based column.
    pub col: u32,
    /// Index into `cellXfs`.
    pub style: u32,
    pub value: CellValue,
    pub formula: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Zero-based row index.
    pub index: u32,
    pub cells: Vec<Cell>,
}

/// Pull-parser over `<sheetData>` rows.
pub struct SheetReader<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
    next_row: u32,
    done: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum TextTarget {
    None,
    Value,
    Formula,
    Inline,
}

impl<R: BufRead> SheetReader<R> {
    pub fn new(input: R) -> Self {
        Self {
            reader: Reader::from_reader(input),
            buf: Vec::new(),
            next_row: 0,
            done: false,
        }
    }

    fn err(&self, e: impl ToString) -> Error {
        Error::xml("worksheet", e)
    }

    fn read_row(&mut self, attrs: Vec<(String, String)>, empty: bool) -> Result<Row> {
        let index = attr(&attrs, "r")
            .and_then(|r| r.parse::<u32>().ok())
            .filter(|&r| r > 0)
            .map(|r| r - 1)
            .unwrap_or(self.next_row);
        self.next_row = index + 1;
        let mut row = Row {
            index,
            cells: Vec::new(),
        };
        if empty {
            return Ok(row);
        }

        let mut next_col = 0u32;
        let mut cell: Option<(Cell, String)> = None; // cell + type attr
        let mut target = TextTarget::None;
        let mut text = String::new();
        let mut in_rph = false; // phonetic runs are not cell text
        let mut buf = Vec::new();

        loop {
            buf.clear();
            let ev = self
                .reader
                .read_event_into(&mut buf)
                .map_err(|e| Error::xml("worksheet", e))?;
            let (start, is_empty) = match ev {
                Event::Start(e) => (e, false),
                Event::Empty(e) => (e, true),
                Event::Text(t) if !in_rph && target != TextTarget::None => {
                    text.push_str(&text_of(&t));
                    continue;
                }
                Event::CData(t) if !in_rph && target != TextTarget::None => {
                    text.push_str(&t.into_inner());
                    continue;
                }
                Event::GeneralRef(r) if !in_rph && target != TextTarget::None => {
                    text.push_str(&ref_text(&r));
                    continue;
                }
                Event::End(e) => {
                    match e.local_name().into_inner() {
                        "rPh" => in_rph = false,
                        "v" => {
                            if let Some((c, t)) = cell.as_mut() {
                                c.value = convert_value(t, std::mem::take(&mut text));
                            }
                            target = TextTarget::None;
                        }
                        "f" => {
                            if let Some((c, _)) = cell.as_mut() {
                                c.formula = Some(std::mem::take(&mut text));
                            }
                            target = TextTarget::None;
                        }
                        "is" => {
                            if let Some((c, _)) = cell.as_mut() {
                                c.value = CellValue::Text(std::mem::take(&mut text));
                            }
                            target = TextTarget::None;
                        }
                        "c" => {
                            if let Some((c, _)) = cell.take() {
                                row.cells.push(c);
                            }
                        }
                        "row" => return Ok(row),
                        _ => {}
                    }
                    continue;
                }
                Event::Eof => return Err(self.err("unexpected end of file inside <row>")),
                _ => continue,
            };
            let name = name_of(&start);
            if in_rph {
                continue;
            }
            let a = attrs_of(&start);
            match name.as_str() {
                "c" => {
                    let col = attr(&a, "r")
                        .and_then(parse_cell_ref)
                        .map(|(c, _)| c)
                        .unwrap_or(next_col);
                    next_col = col + 1;
                    let style = attr(&a, "s").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let t = attr(&a, "t").unwrap_or("n").to_string();
                    let c = Cell {
                        col,
                        style,
                        value: CellValue::Empty,
                        formula: None,
                    };
                    if is_empty {
                        row.cells.push(c);
                    } else {
                        cell = Some((c, t));
                    }
                }
                "v" | "f" | "is" if !is_empty => {
                    target = match name.as_str() {
                        "v" => TextTarget::Value,
                        "f" => TextTarget::Formula,
                        _ => TextTarget::Inline,
                    };
                    text.clear();
                }
                "f" => {
                    // shared-formula follower: <f t="shared" si="0"/>
                    if let Some((c, _)) = cell.as_mut() {
                        c.formula = Some(String::new());
                    }
                }
                "rPh" if !is_empty => in_rph = true,
                _ => {}
            }
        }
    }
}

impl<R: BufRead> Iterator for SheetReader<R> {
    type Item = Result<Row>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            self.buf.clear();
            let ev = match self.reader.read_event_into(&mut self.buf) {
                Ok(ev) => ev,
                Err(e) => {
                    self.done = true;
                    return Some(Err(Error::xml("worksheet", e)));
                }
            };
            let (attrs, empty) = match ev {
                Event::Start(e) if e.local_name().into_inner() == "row" => (attrs_of(&e), false),
                Event::Empty(e) if e.local_name().into_inner() == "row" => (attrs_of(&e), true),
                Event::End(e) if e.local_name().into_inner() == "sheetData" => {
                    self.done = true;
                    return None;
                }
                Event::Eof => {
                    self.done = true;
                    return None;
                }
                _ => continue,
            };
            let r = self.read_row(attrs, empty);
            if r.is_err() {
                self.done = true;
            }
            return Some(r);
        }
    }
}

/// Parse `xl/sharedStrings.xml` into plain strings (rich runs concatenated,
/// phonetic runs skipped).
pub fn parse_shared_strings(data: &[u8]) -> Result<Vec<String>> {
    let mut reader = Reader::from_reader(data);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut in_si, mut in_t, mut in_rph) = (false, false, false);
    loop {
        buf.clear();
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| Error::xml("sharedStrings", e))?
        {
            Event::Start(e) => match e.local_name().into_inner() {
                "si" => {
                    in_si = true;
                    cur.clear();
                }
                "t" if in_si && !in_rph => in_t = true,
                "rPh" => in_rph = true,
                _ => {}
            },
            Event::Empty(e) if e.local_name().into_inner() == "si" => out.push(String::new()),
            Event::Text(t) if in_t => cur.push_str(&text_of(&t)),
            Event::GeneralRef(r) if in_t => cur.push_str(&ref_text(&r)),
            Event::End(e) => match e.local_name().into_inner() {
                "si" => {
                    in_si = false;
                    out.push(std::mem::take(&mut cur));
                }
                "t" => in_t = false,
                "rPh" => in_rph = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn convert_value(t: &str, raw: String) -> CellValue {
    match t {
        "s" => raw
            .trim()
            .parse()
            .map(CellValue::SharedString)
            .unwrap_or(CellValue::Error("#REF!".into())),
        "b" => CellValue::Bool(raw.trim() == "1"),
        "e" => CellValue::Error(raw),
        "str" | "inlineStr" => CellValue::Text(raw),
        "d" => CellValue::Date(raw),
        _ => raw
            .trim()
            .parse()
            .map(CellValue::Number)
            .unwrap_or(CellValue::Text(raw)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_refs() {
        assert_eq!(parse_cell_ref("A1"), Some((0, 0)));
        assert_eq!(parse_cell_ref("$AB$12"), Some((27, 11)));
        assert_eq!(parse_cell_ref("XFD1048576"), Some((16383, 1048575)));
        assert_eq!(parse_cell_ref("A0"), None);
        assert_eq!(parse_cell_ref("12"), None);
        assert_eq!(parse_cell_ref("ABCD1"), None);
    }

    const SHEET: &str = r#"<?xml version="1.0"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" s="2"><v>3.5</v></c><c r="D1" t="b"><v>1</v></c></row>
<row r="3" spans="1:3"><c r="A3"><f>SUM(C1:C2)</f><v>3.5</v></c><c t="inlineStr"><is><r><t>Tom &amp; </t></r><r><t>Jerry</t></r><rPh><t>x</t></rPh></is></c><c t="e"><v>#DIV/0!</v></c></row>
<row r="4"/>
<row><c r="B5" t="str"><f t="shared" si="0">A1&amp;"!"</f><v>hi!</v></c><c r="C5"><f t="shared" si="0"/><v>2</v></c><c r="D5"/></row>
</sheetData></worksheet>"#;

    #[test]
    fn streams_rows() {
        let rows: Vec<Row> = SheetReader::new(SHEET.as_bytes())
            .collect::<Result<_>>()
            .unwrap();
        assert_eq!(
            rows.iter().map(|r| r.index).collect::<Vec<_>>(),
            vec![0, 2, 3, 4]
        );

        let r0 = &rows[0].cells;
        assert_eq!(r0[0].value, CellValue::SharedString(0));
        assert_eq!(
            (r0[1].col, r0[1].style, &r0[1].value),
            (2, 2, &CellValue::Number(3.5))
        );
        assert_eq!(r0[2].value, CellValue::Bool(true));

        let r2 = &rows[1].cells;
        assert_eq!(r2[0].formula.as_deref(), Some("SUM(C1:C2)"));
        assert_eq!(
            (r2[1].col, &r2[1].value),
            (1, &CellValue::Text("Tom & Jerry".into()))
        );
        assert_eq!(r2[2].value, CellValue::Error("#DIV/0!".into()));

        assert!(rows[2].cells.is_empty());
        let r4 = &rows[3].cells;
        assert_eq!(r4[0].formula.as_deref(), Some("A1&\"!\""));
        assert_eq!(r4[0].value, CellValue::Text("hi!".into()));
        assert_eq!(r4[1].formula.as_deref(), Some(""));
        assert_eq!((r4[2].col, &r4[2].value), (3, &CellValue::Empty));
    }

    #[test]
    fn truncated_sheet_errors() {
        let bad = r#"<worksheet><sheetData><row r="1"><c><v>1</v>"#;
        let res: Vec<_> = SheetReader::new(bad.as_bytes()).collect();
        assert_eq!(res.len(), 1);
        assert!(res[0].is_err());
    }

    #[test]
    fn shared_strings() {
        let xml = r#"<sst><si><t>Plain</t></si><si><r><t>Ri</t></r><r><t>ch</t></r><rPh><t>no</t></rPh></si><si/><si><t>&lt;x&gt;</t></si></sst>"#;
        assert_eq!(
            parse_shared_strings(xml.as_bytes()).unwrap(),
            vec!["Plain", "Rich", "", "<x>"]
        );
    }

    #[test]
    fn streams_large_sheet_row_by_row() {
        let mut xml = String::from("<worksheet><sheetData>");
        for r in 1..=20_000 {
            xml.push_str(&format!(r#"<row r="{r}"><c r="A{r}"><v>{r}</v></c></row>"#));
        }
        xml.push_str("</sheetData></worksheet>");
        let mut n = 0u64;
        let mut sum = 0.0;
        for row in SheetReader::new(std::io::BufReader::new(xml.as_bytes())) {
            let row = row.unwrap();
            if let CellValue::Number(v) = row.cells[0].value {
                sum += v;
            }
            n += 1;
        }
        assert_eq!(n, 20_000);
        assert_eq!(sum, (20_000.0 * 20_001.0) / 2.0);
    }
}
