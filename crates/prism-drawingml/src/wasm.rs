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
    let ph = ph_css.as_deref().and_then(parse_hex);
    Ok(c.resolve(&t, &m, ph).css())
}

/// Colour resolution against one theme + colour map (parsed once).
#[wasm_bindgen]
pub struct ColorContext {
    theme: Theme,
    clr_map: Vec<(String, String)>,
}

#[wasm_bindgen]
impl ColorContext {
    #[wasm_bindgen(constructor)]
    pub fn new(theme_json: &str, clr_map_json: &str) -> Result<ColorContext, JsError> {
        Ok(ColorContext {
            theme: if theme_json.is_empty() {
                Theme::default()
            } else {
                serde_json::from_str(theme_json).map_err(js)?
            },
            clr_map: if clr_map_json.is_empty() {
                vec![]
            } else {
                serde_json::from_str(clr_map_json).map_err(js)?
            },
        })
    }

    /// `color_json` → CSS `rgba(...)`; `ph` is the placeholder colour (`#RRGGBB`) for `phClr`.
    pub fn resolve(&self, color_json: &str, ph: Option<String>) -> Result<String, JsError> {
        let c: Color = serde_json::from_str(color_json).map_err(js)?;
        Ok(c.resolve(
            &self.theme,
            &self.clr_map,
            ph.as_deref().and_then(parse_hex),
        )
        .css())
    }
}

fn parse_hex(p: &str) -> Option<Rgba> {
    let h = p.trim_start_matches('#');
    let v = u32::from_str_radix(h.get(..6)?, 16).ok()?;
    Some(Rgba {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
        a: 1.0,
    })
}

/// Theme style-matrix entries (`fillStyles` / `bgFillStyles` XML) → `Fill` JSON.
#[wasm_bindgen(js_name = parseFill)]
pub fn parse_fill(xml: &str) -> String {
    crate::dom::El::parse(xml)
        .and_then(|e| crate::fill::Fill::from_el(&e))
        .and_then(|f| serde_json::to_string(&f).ok())
        .unwrap_or_else(|| "null".into())
}

/// Theme `lineStyles` XML (`a:ln`) → `Line` JSON.
#[wasm_bindgen(js_name = parseLine)]
pub fn parse_line(xml: &str) -> String {
    crate::dom::El::parse(xml)
        .map(|e| crate::fill::Line::from_el(&e))
        .and_then(|l| serde_json::to_string(&l).ok())
        .unwrap_or_else(|| "null".into())
}
