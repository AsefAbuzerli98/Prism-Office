//! wasm-bindgen surface used by the webview (and vitest). All structured
//! data crosses the boundary as JSON strings.

use wasm_bindgen::prelude::*;

use crate::addr::{CellRef, Rect};
use crate::api::{Edit, Engine};
use crate::model::{CellValueModel, WorkbookModel};

fn js(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

fn rect(r0: u32, c0: u32, r1: u32, c1: u32) -> Rect {
    Rect::new(CellRef::new(r0, c0), CellRef::new(r1, c1))
}

#[wasm_bindgen]
pub struct SheetEngine {
    e: Engine,
}

#[wasm_bindgen]
impl SheetEngine {
    /// Load a `WorkbookModel` (JSON) and calculate it. `now` is an Excel serial.
    #[wasm_bindgen(constructor)]
    pub fn new(model_json: &str, now: f64) -> Result<SheetEngine, JsError> {
        let m: WorkbookModel = if model_json.trim().is_empty() {
            WorkbookModel::blank()
        } else {
            serde_json::from_str(model_json).map_err(js)?
        };
        Ok(SheetEngine {
            e: Engine::new(&m, now),
        })
    }

    pub fn model(&self) -> String {
        json(&self.e.model())
    }

    /// `[{sheet,r,c,text}]` → change report.
    pub fn edit(&mut self, edits_json: &str, now: f64) -> Result<String, JsError> {
        let edits: Vec<Edit> = serde_json::from_str(edits_json).map_err(js)?;
        Ok(json(&self.e.edit(&edits, now)))
    }

    pub fn recalc(&mut self, now: f64) -> String {
        json(&self.e.recalc(now))
    }

    pub fn view(&self, sheet: usize, r0: u32, c0: u32, r1: u32, c1: u32) -> String {
        json(&self.e.view(sheet, rect(r0, c0, r1, c1)))
    }

    pub fn input(&self, sheet: usize, r: u32, c: u32) -> String {
        self.e.input(sheet, r, c)
    }

    pub fn sheets(&self) -> String {
        json(&self.e.sheets())
    }

    /// Visual styles by style index.
    pub fn styles(&self) -> String {
        json(&self.e.wb.styles)
    }

    #[wasm_bindgen(js_name = styleOf)]
    pub fn style_of(&self, sheet: usize, r: u32, c: u32) -> String {
        let (f, s) = self.e.style_of(sheet, r, c);
        json(&serde_json::json!({ "numFormat": f, "style": s }))
    }

    #[wasm_bindgen(js_name = applyStyle)]
    #[allow(clippy::too_many_arguments)]
    pub fn apply_style(
        &mut self,
        sheet: usize,
        r0: u32,
        c0: u32,
        r1: u32,
        c1: u32,
        num_format: Option<String>,
        patch_json: &str,
    ) -> Result<(), JsError> {
        let patch: serde_json::Value = if patch_json.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(patch_json).map_err(js)?
        };
        self.e
            .apply_style(sheet, rect(r0, c0, r1, c1), num_format.as_deref(), &patch)
            .map_err(js)
    }

    #[wasm_bindgen(js_name = addSheet)]
    pub fn add_sheet(&mut self, name: &str) -> usize {
        self.e.add_sheet(name)
    }

    #[wasm_bindgen(js_name = renameSheet)]
    pub fn rename_sheet(&mut self, sheet: usize, name: &str, now: f64) -> Result<String, JsError> {
        self.e
            .rename_sheet(sheet, name, now)
            .map(|c| json(&c))
            .map_err(js)
    }

    #[wasm_bindgen(js_name = deleteSheet)]
    pub fn delete_sheet(&mut self, sheet: usize, now: f64) -> Result<String, JsError> {
        self.e
            .delete_sheet(sheet, now)
            .map(|c| json(&c))
            .map_err(js)
    }

    /// Negative width resets to the default.
    #[wasm_bindgen(js_name = setColWidth)]
    pub fn set_col_width(&mut self, sheet: usize, col: u32, width: f64) {
        self.e
            .set_col_width(sheet, col, (width >= 0.0).then_some(width));
    }

    #[wasm_bindgen(js_name = setRowHeight)]
    pub fn set_row_height(&mut self, sheet: usize, row: u32, height: f64) {
        self.e
            .set_row_height(sheet, row, (height >= 0.0).then_some(height));
    }

    #[wasm_bindgen(js_name = setFrozen)]
    pub fn set_frozen(&mut self, sheet: usize, rows: u32, cols: u32) {
        self.e.set_frozen(sheet, rows, cols);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill(
        &mut self,
        sheet: usize,
        sr0: u32,
        sc0: u32,
        sr1: u32,
        sc1: u32,
        dr0: u32,
        dc0: u32,
        dr1: u32,
        dc1: u32,
        now: f64,
    ) -> String {
        json(&self.e.fill(
            sheet,
            rect(sr0, sc0, sr1, sc1),
            rect(dr0, dc0, dr1, dc1),
            now,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paste(
        &mut self,
        src_sheet: usize,
        sr0: u32,
        sc0: u32,
        sr1: u32,
        sc1: u32,
        dst_sheet: usize,
        dr: u32,
        dc: u32,
        now: f64,
    ) -> String {
        json(&self.e.paste(
            src_sheet,
            rect(sr0, sc0, sr1, sc1),
            dst_sheet,
            CellRef::new(dr, dc),
            now,
        ))
    }

    /// Pending `=PY` requests.
    #[wasm_bindgen(js_name = asyncRequests)]
    pub fn async_requests(&self) -> String {
        json(&self.e.wb.async_requests)
    }

    /// Answer a `=PY` request with a value (`1`, `"x"`, `true`, `{"e":"#VALUE!"}`) or a 2-D array.
    #[wasm_bindgen(js_name = supplyAsync)]
    pub fn supply_async(
        &mut self,
        key: &str,
        sheet: usize,
        r: u32,
        c: u32,
        value_json: &str,
        now: f64,
    ) -> Result<String, JsError> {
        let raw: serde_json::Value = serde_json::from_str(value_json).map_err(js)?;
        let v = json_to_value(&raw);
        self.e.wb.supply_async(key, sheet, CellRef::new(r, c), v);
        Ok(json(&self.e.recalc(now)))
    }

    #[wasm_bindgen(js_name = functionNames)]
    pub fn function_names() -> String {
        json(&Engine::function_names())
    }
}

/// JSON → engine value; arrays of arrays become spilled arrays.
pub fn json_to_value(v: &serde_json::Value) -> crate::value::Value {
    use crate::value::{Array, Value};
    match v {
        serde_json::Value::Array(rows) if rows.iter().all(|r| r.is_array()) && !rows.is_empty() => {
            let cols = rows
                .iter()
                .map(|r| r.as_array().map_or(0, |x| x.len()))
                .max()
                .unwrap_or(0)
                .max(1);
            let mut data = Vec::with_capacity(rows.len() * cols);
            for r in rows {
                let r = r.as_array().unwrap();
                for c in 0..cols {
                    data.push(r.get(c).map(json_to_value).unwrap_or(Value::Empty));
                }
            }
            Value::array(Array::new(rows.len(), cols, data))
        }
        serde_json::Value::Array(items) => {
            Value::array(Array::column(items.iter().map(json_to_value).collect()))
        }
        serde_json::Value::Null => Value::Empty,
        other => serde_json::from_value::<CellValueModel>(other.clone())
            .map(|m| m.to_value())
            .unwrap_or(Value::Error(crate::value::ErrorKind::Value)),
    }
}
