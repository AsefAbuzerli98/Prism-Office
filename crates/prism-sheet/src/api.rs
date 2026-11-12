//! Engine API shared by the WASM bindings and native callers: edits,
//! viewport rendering data and sheet operations, all JSON-friendly.

use serde::{Deserialize, Serialize};

use crate::addr::{CellRef, Rect};
use crate::calc::Recalc;
use crate::model::{CellStyle, WorkbookModel};
use crate::numfmt;
use crate::value::Value;
use crate::workbook::{Input, Workbook};

/// One rendered cell for the grid.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ViewCell {
    pub r: u32,
    pub c: u32,
    /// Formatted text.
    pub t: String,
    /// Kind: n number, s text, b bool, e error, f formula-with-empty-result.
    pub k: char,
    /// Style index.
    #[serde(skip_serializing_if = "is_zero")]
    pub s: u32,
    /// Number-format colour (`[Red]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Raw number for numeric cells (status-bar aggregates).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<f64>,
    /// Part of a spill (not the anchor).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub spill: bool,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub changed: Vec<(usize, Rect)>,
    pub evaluated: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub circular: Vec<(usize, CellRef)>,
    pub async_requests: usize,
}

impl Change {
    fn from(wb: &Workbook, r: Recalc) -> Self {
        Self {
            changed: r.changed,
            evaluated: r.evaluated,
            error: None,
            circular: wb.circular.clone(),
            async_requests: wb.async_requests.len(),
        }
    }
    fn error(msg: String) -> Self {
        Self {
            error: Some(msg),
            ..Default::default()
        }
    }
}

/// A single cell edit in a batch.
#[derive(Clone, Debug, Deserialize)]
pub struct Edit {
    pub sheet: usize,
    pub r: u32,
    pub c: u32,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetInfo {
    pub name: String,
    pub hidden: bool,
    pub used: Option<Rect>,
    pub frozen: (u32, u32),
    pub col_widths: Vec<(u32, f64)>,
    pub row_heights: Vec<(u32, f64)>,
    pub merges: Vec<Rect>,
    pub spills: Vec<Rect>,
}

pub struct Engine {
    pub wb: Workbook,
}

impl Engine {
    pub fn new(model: &WorkbookModel, now: f64) -> Self {
        let mut wb = Workbook::from_model(model);
        wb.recalc(now);
        Self { wb }
    }

    pub fn model(&self) -> WorkbookModel {
        self.wb.to_model()
    }

    /// Apply edits (text as typed: `=…` formulas or literals) and recalc.
    pub fn edit(&mut self, edits: &[Edit], now: f64) -> Change {
        for e in edits {
            if e.sheet >= self.wb.sheets.len() {
                return Change::error(format!("no sheet {}", e.sheet));
            }
            if let Err(err) = self.wb.put_input(e.sheet, CellRef::new(e.r, e.c), &e.text) {
                // keep earlier edits; report the first parse error
                let r = self.wb.recalc(now);
                let mut ch = Change::from(&self.wb, r);
                ch.error = Some(err.to_string());
                return ch;
            }
        }
        let r = self.wb.recalc(now);
        Change::from(&self.wb, r)
    }

    pub fn recalc(&mut self, now: f64) -> Change {
        let r = self.wb.recalc(now);
        Change::from(&self.wb, r)
    }

    /// Rendered cells in a rectangle (non-empty or styled ones only).
    pub fn view(&self, sheet: usize, rect: Rect) -> Vec<ViewCell> {
        let Some(sh) = self.wb.sheets.get(sheet) else {
            return vec![];
        };
        // (position, value, style, spilled, is formula)
        let mut items: Vec<(CellRef, Value, u32, bool, bool)> = Vec::new();
        let cells: Vec<(CellRef, &crate::workbook::Cell)> = if (sh.cells.len() as u64) < rect.size()
        {
            sh.cells
                .iter()
                .filter(|(k, _)| rect.contains(**k))
                .map(|(k, c)| (*k, c))
                .collect()
        } else {
            (rect.r0..=rect.r1)
                .flat_map(|r| {
                    sh.cells
                        .range(CellRef::new(r, rect.c0)..=CellRef::new(r, rect.c1))
                        .map(|(k, c)| (*k, c))
                })
                .collect()
        };
        for (k, c) in cells {
            items.push((
                k,
                self.wb.value_at(sheet, k),
                c.style,
                false,
                matches!(c.input, Input::Formula { .. }),
            ));
        }
        for (anchor, sr) in &sh.spills {
            if !sr.intersects(&rect) {
                continue;
            }
            let style = sh.cells.get(anchor).map(|c| c.style).unwrap_or(0);
            for r in sr.r0.max(rect.r0)..=sr.r1.min(rect.r1) {
                for c in sr.c0.max(rect.c0)..=sr.c1.min(rect.c1) {
                    let k = CellRef::new(r, c);
                    if k != *anchor && !sh.cells.contains_key(&k) {
                        items.push((k, self.wb.value_at(sheet, k), style, true, false));
                    }
                }
            }
        }
        let fmt = |style: u32| {
            self.wb
                .num_formats
                .get(style as usize)
                .map(String::as_str)
                .unwrap_or("General")
        };
        items
            .into_iter()
            .filter_map(|(k, v, style, spill, formula)| {
                let kind = match v {
                    Value::Number(_) => 'n',
                    Value::Text(_) => 's',
                    Value::Bool(_) => 'b',
                    Value::Error(_) => 'e',
                    _ if formula => 'f',
                    _ if style != 0 => '-',
                    _ => return None,
                };
                let f = numfmt::format_value(&v, fmt(style));
                let n = if let Value::Number(x) = v {
                    Some(x)
                } else {
                    None
                };
                Some(ViewCell {
                    n,
                    r: k.row,
                    c: k.col,
                    t: f.text,
                    k: kind,
                    s: style,
                    color: f.color,
                    spill,
                })
            })
            .collect()
    }

    /// Text shown in the formula bar / cell editor.
    pub fn input(&self, sheet: usize, r: u32, c: u32) -> String {
        self.wb.input_text(sheet, CellRef::new(r, c))
    }

    pub fn sheets(&self) -> Vec<SheetInfo> {
        self.wb
            .sheets
            .iter()
            .map(|s| SheetInfo {
                name: s.name.clone(),
                hidden: s.hidden,
                used: s.used_rect(),
                frozen: s.frozen,
                col_widths: s.col_widths.iter().map(|(a, b)| (*a, *b)).collect(),
                row_heights: s.row_heights.iter().map(|(a, b)| (*a, *b)).collect(),
                merges: s.merges.clone(),
                spills: s.spills.values().copied().collect(),
            })
            .collect()
    }

    pub fn add_sheet(&mut self, name: &str) -> usize {
        let mut name = name.to_string();
        let mut n = 2;
        while self.wb.sheet_index(&name).is_some() {
            name = format!(
                "{} ({n})",
                name.trim_end_matches(|c: char| c == ')' || c.is_ascii_digit())
                    .trim_end_matches(" (")
                    .trim()
            );
            n += 1;
        }
        self.wb.add_sheet(&name)
    }

    /// Rename a sheet, rewriting formulas that reference it.
    pub fn rename_sheet(&mut self, sheet: usize, name: &str, now: f64) -> Result<Change, String> {
        let name = name.trim();
        if name.is_empty() || name.len() > 31 || name.contains(['\\', '/', '?', '*', '[', ']', ':'])
        {
            return Err("invalid sheet name".into());
        }
        if self.wb.sheet_index(name).is_some_and(|i| i != sheet) {
            return Err("a sheet with this name already exists".into());
        }
        let old = std::mem::replace(&mut self.wb.sheets[sheet].name, name.to_string());
        let rewrite = |e: &crate::parser::Expr| crate::parser::rename_sheet(e, &old, name);
        self.rewrite_formulas(rewrite);
        for n in &mut self.wb.names {
            if let Ok(e) = crate::parser::parse(&n.formula, CellRef::new(0, 0)) {
                n.formula = crate::parser::to_formula(
                    &crate::parser::rename_sheet(&e, &old, name),
                    &Default::default(),
                );
            }
        }
        self.wb.mark_all_dirty();
        Ok(self.recalc(now))
    }

    pub fn delete_sheet(&mut self, sheet: usize, now: f64) -> Result<Change, String> {
        if self.wb.sheets.len() <= 1 {
            return Err("a workbook needs at least one sheet".into());
        }
        let name = self.wb.sheets[sheet].name.clone();
        self.wb.sheets.remove(sheet);
        self.rewrite_formulas(|e| crate::parser::rename_sheet(e, &name, "#REF"));
        self.wb.names.retain(|n| n.sheet != Some(sheet));
        for n in &mut self.wb.names {
            if let Some(s) = n.sheet.as_mut() {
                if *s > sheet {
                    *s -= 1;
                }
            }
        }
        self.wb.graph = Default::default();
        self.wb.async_results.clear();
        self.wb.mark_all_dirty();
        Ok(self.recalc(now))
    }

    fn rewrite_formulas(&mut self, f: impl Fn(&crate::parser::Expr) -> crate::parser::Expr) {
        for s in &mut self.wb.sheets {
            // shared trees are rewritten once
            let mut done: crate::fxhash::FxHashMap<usize, std::sync::Arc<crate::parser::Expr>> =
                Default::default();
            for c in s.cells.values_mut() {
                if let Input::Formula { expr, .. } = &mut c.input {
                    let key = std::sync::Arc::as_ptr(expr) as usize;
                    let new = done
                        .entry(key)
                        .or_insert_with(|| {
                            let e = f(expr);
                            if e != **expr {
                                std::sync::Arc::new(e)
                            } else {
                                expr.clone()
                            }
                        })
                        .clone();
                    *expr = new;
                }
            }
        }
    }

    pub fn set_col_width(&mut self, sheet: usize, col: u32, width: Option<f64>) {
        let s = &mut self.wb.sheets[sheet];
        match width {
            Some(w) => s.col_widths.insert(col, w),
            None => s.col_widths.remove(&col),
        };
    }

    pub fn set_row_height(&mut self, sheet: usize, row: u32, height: Option<f64>) {
        let s = &mut self.wb.sheets[sheet];
        match height {
            Some(h) => s.row_heights.insert(row, h),
            None => s.row_heights.remove(&row),
        };
    }

    pub fn set_frozen(&mut self, sheet: usize, rows: u32, cols: u32) {
        self.wb.sheets[sheet].frozen = (rows, cols);
    }

    /// Style of a cell: (number format, look).
    pub fn style_of(&self, sheet: usize, r: u32, c: u32) -> (String, CellStyle) {
        let s = self.wb.sheets[sheet]
            .cells
            .get(&CellRef::new(r, c))
            .map(|c| c.style)
            .unwrap_or(0) as usize;
        (
            self.wb
                .num_formats
                .get(s)
                .cloned()
                .unwrap_or_else(|| "General".into()),
            self.wb.styles.get(s).cloned().unwrap_or_default(),
        )
    }

    /// Apply a number format and/or look to every cell of `rect`.
    pub fn apply_style(
        &mut self,
        sheet: usize,
        rect: Rect,
        num_format: Option<&str>,
        patch: &serde_json::Value,
    ) -> Result<(), String> {
        if rect.size() > 2_000_000 {
            return Err("range too large to format".into());
        }
        for r in rect.r0..=rect.r1 {
            for c in rect.c0..=rect.c1 {
                let (fmt, look) = self.style_of(sheet, r, c);
                let mut v = serde_json::to_value(&look).map_err(|e| e.to_string())?;
                if let (Some(o), Some(p)) = (v.as_object_mut(), patch.as_object()) {
                    for (k, x) in p {
                        if x.is_null() || x == &serde_json::Value::Bool(false) {
                            o.remove(k);
                        } else {
                            o.insert(k.clone(), x.clone());
                        }
                    }
                }
                let look: CellStyle = serde_json::from_value(v).map_err(|e| e.to_string())?;
                let idx = self.wb.style_index(num_format.unwrap_or(&fmt), &look);
                self.wb.set_style(sheet, CellRef::new(r, c), idx);
            }
        }
        Ok(())
    }

    /// Copy `src` to `dst` (relative references shift), e.g. the fill handle.
    pub fn fill(&mut self, sheet: usize, src: Rect, dst: Rect, now: f64) -> Change {
        let (h, w) = (src.rows(), src.cols());
        let mut edits = Vec::new();
        // a numeric series extends when the source is a single row/column of numbers
        let series = self.series(sheet, src, dst);
        for r in dst.r0..=dst.r1 {
            for c in dst.c0..=dst.c1 {
                if src.contains(CellRef::new(r, c)) {
                    continue;
                }
                let sr = src.r0 + (r - dst.r0) % h;
                let sc = src.c0 + (c - dst.c0) % w;
                let from = CellRef::new(sr, sc);
                let text = match self.wb.sheets[sheet].cells.get(&from).map(|x| &x.input) {
                    Some(Input::Formula { expr, base }) => {
                        let moved = crate::parser::shift(
                            expr,
                            r as i64 - sr as i64 + sr as i64 - base.row as i64,
                            c as i64 - sc as i64 + sc as i64 - base.col as i64,
                        );
                        format!(
                            "={}",
                            crate::parser::to_formula(&moved, &Default::default())
                        )
                    }
                    _ => match series.as_ref().and_then(|s| s(r, c)) {
                        Some(n) => crate::value::format_number_roundtrip(n),
                        None => self.wb.input_text(sheet, from),
                    },
                };
                edits.push(Edit { sheet, r, c, text });
                let style = self.wb.sheets[sheet]
                    .cells
                    .get(&from)
                    .map(|x| x.style)
                    .unwrap_or(0);
                self.wb.set_style(sheet, CellRef::new(r, c), style);
            }
        }
        self.edit(&edits, now)
    }

    /// Paste a copied block: formulas shift relative to the new position,
    /// styles are copied. `src_sheet` may differ from `dst_sheet`.
    pub fn paste(
        &mut self,
        src_sheet: usize,
        src: Rect,
        dst_sheet: usize,
        dst: CellRef,
        now: f64,
    ) -> Change {
        let mut edits = Vec::new();
        let mut styles = Vec::new();
        for r in src.r0..=src.r1 {
            for c in src.c0..=src.c1 {
                let from = CellRef::new(r, c);
                let (tr, tc) = (
                    dst.row as i64 + (r - src.r0) as i64,
                    dst.col as i64 + (c - src.c0) as i64,
                );
                if tr >= crate::addr::MAX_ROWS as i64 || tc >= crate::addr::MAX_COLS as i64 {
                    continue;
                }
                let cell = self.wb.sheets[src_sheet].cells.get(&from);
                let text = match cell.map(|x| &x.input) {
                    Some(Input::Formula { expr, base }) => {
                        let moved =
                            crate::parser::shift(expr, tr - base.row as i64, tc - base.col as i64);
                        format!(
                            "={}",
                            crate::parser::to_formula(&moved, &Default::default())
                        )
                    }
                    _ => self.wb.input_text(src_sheet, from),
                };
                styles.push((
                    CellRef::new(tr as u32, tc as u32),
                    cell.map(|x| x.style).unwrap_or(0),
                ));
                edits.push(Edit {
                    sheet: dst_sheet,
                    r: tr as u32,
                    c: tc as u32,
                    text,
                });
            }
        }
        let ch = self.edit(&edits, now);
        for (k, s) in styles {
            self.wb.set_style(dst_sheet, k, s);
        }
        ch
    }

    #[allow(clippy::type_complexity)]
    fn series(
        &self,
        sheet: usize,
        src: Rect,
        dst: Rect,
    ) -> Option<Box<dyn Fn(u32, u32) -> Option<f64>>> {
        let vertical = src.cols() == 1 && dst.c0 == src.c0 && dst.c1 == src.c1;
        let horizontal = src.rows() == 1 && dst.r0 == src.r0 && dst.r1 == src.r1;
        if !(vertical || horizontal) {
            return None;
        }
        let nums: Vec<f64> = (0..if vertical { src.rows() } else { src.cols() })
            .map(|i| {
                let k = if vertical {
                    CellRef::new(src.r0 + i, src.c0)
                } else {
                    CellRef::new(src.r0, src.c0 + i)
                };
                match self.wb.sheets[sheet].cells.get(&k).map(|x| &x.input) {
                    Some(Input::Value(Value::Number(n))) => Some(*n),
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>()?;
        if nums.len() < 2 {
            return None;
        }
        let step = (nums[nums.len() - 1] - nums[0]) / (nums.len() - 1) as f64;
        let (first, start) = (nums[0], if vertical { src.r0 } else { src.c0 });
        Some(Box::new(move |r, c| {
            let i = if vertical {
                r as f64 - start as f64
            } else {
                c as f64 - start as f64
            };
            Some(first + step * i)
        }))
    }

    pub fn add_chart(&mut self, sheet: usize, def: crate::model::ChartDef) -> usize {
        self.wb.sheets[sheet].charts.push(def);
        self.wb.sheets[sheet].charts.len() - 1
    }

    pub fn update_chart(
        &mut self,
        sheet: usize,
        index: usize,
        def: crate::model::ChartDef,
    ) -> Result<(), String> {
        let c = self
            .wb
            .sheets
            .get_mut(sheet)
            .and_then(|s| s.charts.get_mut(index))
            .ok_or("no such chart")?;
        *c = def;
        Ok(())
    }

    pub fn remove_chart(&mut self, sheet: usize, index: usize) {
        if let Some(s) = self.wb.sheets.get_mut(sheet) {
            if index < s.charts.len() {
                s.charts.remove(index);
            }
        }
    }

    /// Charts of a sheet with their current data.
    pub fn charts(&self, sheet: usize) -> Vec<(crate::model::ChartDef, crate::chart::ChartData)> {
        self.wb
            .sheets
            .get(sheet)
            .map(|s| {
                s.charts
                    .iter()
                    .map(|c| (c.clone(), crate::chart::chart_data(&self.wb, sheet, c)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Function names for autocomplete.
    pub fn function_names() -> Vec<&'static str> {
        crate::functions::function_names()
    }
}
