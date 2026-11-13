//! wasm-bindgen surface for the slide editor: geometry and colour resolution.

use wasm_bindgen::prelude::*;

use crate::color::{Color, Rgba, Theme};
use crate::geom::GeomDef;
use crate::shape::Geometry;

fn js(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Evaluate a `Geometry` (JSON) at a size → `{paths, textRect}` JSON.
#[wasm_bindgen(js_name = evaluateGeometry)]
pub fn evaluate_geometry(geom_json: &str, w: f64, h: f64) -> Result<String, JsError> {
    let g: Geometry = serde_json::from_str(geom_json).map_err(js)?;
    let e = match &g {
        Geometry::Preset { name, adj } => crate::geom::evaluate_preset(name, w, h, adj),
        Geometry::Custom { def } => def.evaluate(w, h, &[]),
    };
    serde_json::to_string(&e).map_err(js)
}

/// A preset's definition (adjust handles and defaults) as JSON.
#[wasm_bindgen(js_name = presetDefinition)]
pub fn preset_definition(name: &str) -> String {
    let d: &GeomDef = crate::geom::preset(name);
    serde_json::to_string(d).unwrap_or_default()
}

#[wasm_bindgen(js_name = presetNames)]
pub fn preset_names() -> String {
    serde_json::to_string(&crate::geom::preset_names()).unwrap_or_default()
}

/// Resolve a colour (JSON) to CSS against a theme and colour map (JSON).
#[wasm_bindgen(js_name = resolveColor)]
pub fn resolve_color(
    color_json: &str,
    theme_json: &str,
    clr_map_json: &str,
    ph_css: Option<String>,
) -> Result<String, JsError> {
    let c: Color = serde_json::from_str(color_json).map_err(js)?;
    let t: Theme = if theme_json.is_empty() {
        Theme::default()
    } else {
        serde_json::from_str(theme_json).map_err(js)?
    };
    let m: Vec<(String, String)> = if clr_map_json.is_empty() {
        vec![]
    } else {
        serde_json::from_str(clr_map_json).map_err(js)?
    };
    let ph = ph_css.and_then(|p| {
        let h = p.trim_start_matches('#');
        let v = u32::from_str_radix(h.get(..6)?, 16).ok()?;
        Some(Rgba {
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
            a: 1.0,
        })
    });
    Ok(c.resolve(&t, &m, ph).css())
}
