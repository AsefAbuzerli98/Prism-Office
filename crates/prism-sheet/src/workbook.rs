//! Workbook model: sheets, sparse cells, spills, tables and defined names.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::fxhash::FxHashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::addr::{CellRef, Rect};
use crate::parser::{self, Expr};
use crate::value::{ErrorKind, Value};

/// What the user typed into a cell.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Value(Value),
    /// `expr` is written relative to `base`; cells filled with the same
    /// relative formula share one tree (like Excel's shared formulas).
    Formula {
        expr: Arc<Expr>,
        base: CellRef,
    },
}

impl Input {
    /// The formula as it reads in cell `at` (references shifted from `base`).
    pub fn expr_at(&self, at: CellRef) -> Option<std::borrow::Cow<'_, Expr>> {
        let Input::Formula { expr, base } = self else {
            return None;
        };
        if *base == at {
            return Some(std::borrow::Cow::Borrowed(expr.as_ref()));
        }
        Some(std::borrow::Cow::Owned(parser::shift(
            expr,
            at.row as i64 - base.row as i64,
            at.col as i64 - base.col as i64,
        )))
    }

    /// Formula text (without `=`) as shown in cell `at`.
    pub fn formula_text(&self, at: CellRef) -> Option<String> {
        let Input::Formula { expr, base } = self else {
            return None;
        };
        let offset = (
            at.row as i64 - base.row as i64,
            at.col as i64 - base.col as i64,
        );
        Some(parser::to_formula(
            expr,
            &parser::WriteOptions {
                offset,
                ..Default::default()
            },
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub input: Input,
    /// Calculated value (for formulas: the anchor value; spilled cells are
    /// read through the anchor's array).
    pub value: Value,
    pub style: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDef {
    pub name: String,
    pub rect: Rect,
    pub columns: Vec<String>,
    pub header_row: bool,
    pub totals_row: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Sheet {
    pub name: String,
    pub cells: BTreeMap<CellRef, Cell>,
    /// Formula cells by column → rows (dependency queries).
    pub(crate) formulas: BTreeMap<u32, BTreeSet<u32>>,
    /// Spill anchor → occupied rect (successful spills).
    pub spills: FxHashMap<CellRef, Rect>,
    /// Anchor → rect it wanted but could not take (#SPILL!).
    pub blocked: FxHashMap<CellRef, Rect>,
    /// Spilled cell → anchor.
    pub spill_owner: FxHashMap<CellRef, CellRef>,
    pub tables: Vec<TableDef>,
    pub charts: Vec<crate::model::ChartDef>,
    pub col_widths: BTreeMap<u32, f64>,
    pub row_heights: BTreeMap<u32, f64>,
    pub merges: Vec<Rect>,
    /// Frozen rows/columns (top-left split).
    pub frozen: (u32, u32),
    pub hidden: bool,
    /// Index of the worksheet in the source file (XLSX round-trip).
    pub source: Option<usize>,
    /// Lazily computed bounding box of content (reset on every mutation).
    pub(crate) used: std::sync::OnceLock<Option<Rect>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinedName {
    pub name: String,
    /// Formula text without `=` (e.g. `Sheet1!$A$1:$A$10` or `LAMBDA(x,x*2)`).
    pub formula: String,
    /// Sheet-scoped names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Workbook {
    pub sheets: Vec<Sheet>,
    pub names: Vec<DefinedName>,
    /// Number-format code per style index (`General`, `0.00`, `yyyy-mm-dd` …).
    pub num_formats: Vec<String>,
    /// Visual style per style index.
    pub styles: Vec<crate::model::CellStyle>,
    /// Cells found in circular references during the last recalculation.
    pub circular: Vec<(usize, CellRef)>,
    /// Values supplied for asynchronous functions (`=PY`), by cell, tagged
    /// with the request key they answer (stale answers are ignored).
    pub async_results: HashMap<(usize, CellRef), (String, Value)>,
    /// Pending asynchronous requests produced by the last recalculation.
    pub async_requests: Vec<AsyncRequest>,
    pub(crate) parsed_names: HashMap<String, Arc<Expr>>,
    /// Dependency graph over formula cells.
    pub(crate) graph: crate::calc::Graph,
    /// Positions edited since the last recalculation.
    pub(crate) pending: BTreeSet<(usize, CellRef)>,
    /// 1904 date system.
    pub date1904: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsyncRequest {
    /// Hash of code + inputs; echo it back with the result.
    pub key: String,
    pub sheet: usize,
    pub row: u32,
    pub col: u32,
    pub function: String,
    pub code: String,
    pub return_type: i32,
    /// Ranges referenced through `xl("…")`, already resolved to values.
    pub inputs: Vec<(String, Vec<Vec<serde_json::Value>>)>,
}

/// Parse literal input the way Excel does: numbers, percentages,
/// booleans, errors; everything else is text. A leading `'` forces text.
pub fn parse_literal(s: &str) -> Value {
    if s.is_empty() {
        return Value::Empty;
    }
    if let Some(t) = s.strip_prefix('\'') {
        return Value::text(t);
    }
    let t = s.trim();
    if t.eq_ignore_ascii_case("TRUE") {
        return Value::Bool(true);
    }
    if t.eq_ignore_ascii_case("FALSE") {
        return Value::Bool(false);
    }
    if let Some(e) = ErrorKind::parse(t) {
        return Value::Error(e);
    }
    if let Some(n) = parse_number(t) {
        return Value::Number(n);
    }
    Value::text(s)
}

/// Numbers as typed: `1,234.5`, `-3`, `1e3`, `50%`, `(12)`.
pub fn parse_number(t: &str) -> Option<f64> {
    let (t, pct) = t
        .strip_suffix('%')
        .map_or((t, false), |x| (x.trim_end(), true));
    let (t, neg) = t
        .strip_prefix('(')
        .and_then(|x| x.strip_suffix(')'))
        .map_or((t, false), |x| (x, true));
    if t.is_empty()
        || !t
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
    {
        return None;
    }
    let clean: String = if t.contains(',') {
        // thousands separators must be in groups of three
        let (int, frac) = t.split_once('.').map_or((t, None), |(a, b)| (a, Some(b)));
        let groups: Vec<&str> = int.trim_start_matches(['-', '+']).split(',').collect();
        if groups.len() > 1
            && (groups[0].is_empty()
                || groups[0].len() > 3
                || groups[1..].iter().any(|g| g.len() != 3))
        {
            return None;
        }
        format!(
            "{}{}",
            int.replace(',', ""),
            frac.map(|f| format!(".{f}")).unwrap_or_default()
        )
    } else {
        t.to_string()
    };
    let n: f64 = clean.parse().ok().filter(|n: &f64| n.is_finite())?;
    let n = if neg { -n } else { n };
    Some(if pct { n / 100.0 } else { n })
}

impl Sheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Bounding box of cells with content (including spills).
    pub fn used_rect(&self) -> Option<Rect> {
        *self.used.get_or_init(|| self.compute_used())
    }

    /// Extend the cached used rectangle by a newly occupied cell.
    pub(crate) fn grow(&mut self, c: CellRef) {
        match self.used.get().copied() {
            Some(Some(r)) => {
                let n = Rect {
                    r0: r.r0.min(c.row),
                    c0: r.c0.min(c.col),
                    r1: r.r1.max(c.row),
                    c1: r.c1.max(c.col),
                };
                self.used = std::sync::OnceLock::from(Some(n));
            }
            Some(None) => self.used = std::sync::OnceLock::from(Some(Rect::cell(c))),
            None => {}
        }
    }

    /// Forget the cached used rectangle after a mutation.
    pub(crate) fn touch(&mut self) {
        self.used = std::sync::OnceLock::new();
    }

    fn compute_used(&self) -> Option<Rect> {
        let mut it = self
            .cells
            .keys()
            .copied()
            .chain(self.spill_owner.keys().copied());
        let first = it.next()?;
        let mut r = Rect::cell(first);
        for c in it {
            r.r0 = r.r0.min(c.row);
            r.r1 = r.r1.max(c.row);
            r.c0 = r.c0.min(c.col);
            r.c1 = r.c1.max(c.col);
        }
        Some(r)
    }

    pub(crate) fn index_formula(&mut self, c: CellRef, on: bool) {
        if on {
            self.formulas.entry(c.col).or_default().insert(c.row);
        } else if let Some(rows) = self.formulas.get_mut(&c.col) {
            rows.remove(&c.row);
            if rows.is_empty() {
                self.formulas.remove(&c.col);
            }
        }
    }

    /// Formula cells inside `r`.
    pub fn formulas_in(&self, r: &Rect) -> Vec<CellRef> {
        let mut out = Vec::new();
        for (&col, rows) in self.formulas.range(r.c0..=r.c1) {
            for &row in rows.range(r.r0..=r.r1) {
                out.push(CellRef::new(row, col));
            }
        }
        out
    }

    pub fn table_at(&self, c: CellRef) -> Option<&TableDef> {
        self.tables.iter().find(|t| t.rect.contains(c))
    }
}

impl Workbook {
    pub fn new() -> Self {
        let mut wb = Self {
            num_formats: vec!["General".into()],
            ..Default::default()
        };
        wb.sheets.push(Sheet::new("Sheet1"));
        wb
    }

    pub fn sheet_index(&self, name: &str) -> Option<usize> {
        self.sheets
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn add_sheet(&mut self, name: &str) -> usize {
        self.sheets.push(Sheet::new(name));
        self.sheets.len() - 1
    }

    /// Value shown in a cell (follows spills).
    pub fn value_at(&self, sheet: usize, c: CellRef) -> Value {
        let Some(s) = self.sheets.get(sheet) else {
            return Value::Error(ErrorKind::Ref);
        };
        if let Some(cell) = s.cells.get(&c) {
            return match &cell.value {
                Value::Array(a) => a.data.first().cloned().unwrap_or_default(),
                v => v.clone(),
            };
        }
        if let Some(anchor) = s.spill_owner.get(&c) {
            if let Some(Value::Array(a)) = s.cells.get(anchor).map(|x| &x.value) {
                let (dr, dc) = ((c.row - anchor.row) as usize, (c.col - anchor.col) as usize);
                if dr < a.rows && dc < a.cols {
                    return a.get(dr, dc).clone();
                }
            }
        }
        Value::Empty
    }

    pub fn input_text(&self, sheet: usize, c: CellRef) -> String {
        match self
            .sheets
            .get(sheet)
            .and_then(|s| s.cells.get(&c))
            .map(|x| &x.input)
        {
            Some(f @ Input::Formula { .. }) => {
                format!("={}", f.formula_text(c).unwrap_or_default())
            }
            Some(Input::Value(Value::Text(t))) => {
                // keep text that would otherwise re-parse as a number visibly text
                if t.starts_with('=')
                    || t.starts_with('\'')
                    || !matches!(parse_literal(t), Value::Text(_))
                {
                    format!("'{t}")
                } else {
                    t.to_string()
                }
            }
            Some(Input::Value(v)) => v.to_string(),
            None => String::new(),
        }
    }

    /// Store raw input without recalculating. Returns the parse error for formulas.
    pub fn put_input(
        &mut self,
        sheet: usize,
        c: CellRef,
        text: &str,
    ) -> Result<(), parser::ParseError> {
        let style = self.sheets[sheet]
            .cells
            .get(&c)
            .map(|x| x.style)
            .unwrap_or(0);
        let input = if let Some(f) = text.strip_prefix('=').filter(|f| !f.trim().is_empty()) {
            let expr = parser::parse(f, c)?;
            Input::Formula {
                expr: Arc::new(expr),
                base: c,
            }
        } else {
            let v = parse_literal(text);
            if matches!(v, Value::Empty) && style == 0 {
                self.clear_cell(sheet, c);
                return Ok(());
            }
            Input::Value(v)
        };
        self.set_cell_input(sheet, c, input, style);
        Ok(())
    }

    pub(crate) fn set_cell_input(&mut self, sheet: usize, c: CellRef, input: Input, style: u32) {
        let s = &mut self.sheets[sheet];
        let is_formula = matches!(input, Input::Formula { .. });
        let value = match &input {
            Input::Value(v) => v.clone(),
            Input::Formula { .. } => Value::Empty,
        };
        s.index_formula(c, is_formula);
        if !is_formula {
            Self::drop_spill(s, c);
        }
        s.grow(c);
        s.cells.insert(
            c,
            Cell {
                input,
                value,
                style,
            },
        );
        self.pending.insert((sheet, c));
        crate::calc::reindex(self, sheet, c);
    }

    pub fn clear_cell(&mut self, sheet: usize, c: CellRef) {
        let s = &mut self.sheets[sheet];
        s.index_formula(c, false);
        Self::drop_spill(s, c);
        s.cells.remove(&c);
        s.touch();
        self.pending.insert((sheet, c));
        crate::calc::reindex(self, sheet, c);
    }

    pub(crate) fn drop_spill(s: &mut Sheet, anchor: CellRef) {
        if let Some(r) = s.spills.remove(&anchor) {
            s.touch();
            for row in r.r0..=r.r1 {
                for col in r.c0..=r.c1 {
                    let k = CellRef::new(row, col);
                    if s.spill_owner.get(&k) == Some(&anchor) {
                        s.spill_owner.remove(&k);
                    }
                }
            }
        }
        s.blocked.remove(&anchor);
    }

    pub fn set_style(&mut self, sheet: usize, c: CellRef, style: u32) {
        let s = &mut self.sheets[sheet];
        match s.cells.get_mut(&c) {
            Some(cell) => cell.style = style,
            None if style != 0 => {
                s.touch();
                s.cells.insert(
                    c,
                    Cell {
                        input: Input::Value(Value::Empty),
                        value: Value::Empty,
                        style,
                    },
                );
            }
            None => {}
        }
    }

    /// Defined name lookup (sheet scope first), parsed and cached.
    pub(crate) fn name_expr(&self, name: &str, sheet: usize) -> Option<Arc<Expr>> {
        let key = name.to_ascii_uppercase();
        let def = self
            .names
            .iter()
            .find(|n| n.name.eq_ignore_ascii_case(name) && n.sheet == Some(sheet))
            .or_else(|| {
                self.names
                    .iter()
                    .find(|n| n.name.eq_ignore_ascii_case(name) && n.sheet.is_none())
            })?;
        if let Some(e) = self.parsed_names.get(&format!("{key}@{:?}", def.sheet)) {
            return Some(e.clone());
        }
        parser::parse(&def.formula, CellRef::new(0, 0))
            .ok()
            .map(Arc::new)
    }

    pub(crate) fn reparse_names(&mut self) {
        self.parsed_names = self
            .names
            .iter()
            .filter_map(|n| {
                parser::parse(&n.formula, CellRef::new(0, 0)).ok().map(|e| {
                    (
                        format!("{}@{:?}", n.name.to_ascii_uppercase(), n.sheet),
                        Arc::new(e),
                    )
                })
            })
            .collect();
    }

    /// Index of a style with this number format and look, appending one if needed.
    pub fn style_index(&mut self, num_format: &str, look: &crate::model::CellStyle) -> u32 {
        while self.styles.len() < self.num_formats.len() {
            self.styles.push(Default::default());
        }
        if let Some(i) = (0..self.num_formats.len())
            .find(|&i| self.num_formats[i] == num_format && self.styles[i] == *look)
        {
            return i as u32;
        }
        self.num_formats.push(num_format.to_string());
        self.styles.push(look.clone());
        (self.num_formats.len() - 1) as u32
    }

    pub fn find_table(&self, name: &str) -> Option<(usize, &TableDef)> {
        self.sheets.iter().enumerate().find_map(|(i, s)| {
            s.tables
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(name))
                .map(|t| (i, t))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals() {
        assert_eq!(parse_literal("12"), Value::Number(12.0));
        assert_eq!(parse_literal("1,234.5"), Value::Number(1234.5));
        assert_eq!(parse_literal("50%"), Value::Number(0.5));
        assert_eq!(parse_literal("(3)"), Value::Number(-3.0));
        assert_eq!(parse_literal("true"), Value::Bool(true));
        assert_eq!(parse_literal("#N/A"), Value::Error(ErrorKind::NA));
        assert_eq!(parse_literal("'007"), Value::text("007"));
        assert_eq!(parse_literal("1,23"), Value::text("1,23"));
        assert_eq!(parse_literal("abc"), Value::text("abc"));
        assert_eq!(parse_literal("1e3"), Value::Number(1000.0));
    }

    #[test]
    fn formula_index_queries() {
        let mut wb = Workbook::new();
        wb.put_input(0, CellRef::new(2, 1), "=1").unwrap();
        wb.put_input(0, CellRef::new(9, 1), "=2").unwrap();
        wb.put_input(0, CellRef::new(3, 4), "=3").unwrap();
        let r = Rect::new(CellRef::new(0, 0), CellRef::new(5, 5));
        assert_eq!(
            wb.sheets[0].formulas_in(&r),
            vec![CellRef::new(2, 1), CellRef::new(3, 4)]
        );
        wb.put_input(0, CellRef::new(2, 1), "x").unwrap();
        assert_eq!(wb.sheets[0].formulas_in(&r), vec![CellRef::new(3, 4)]);
        assert_eq!(wb.input_text(0, CellRef::new(3, 4)), "=3");
        wb.put_input(0, CellRef::new(0, 0), "'12").unwrap();
        assert_eq!(wb.input_text(0, CellRef::new(0, 0)), "'12");
    }
}
