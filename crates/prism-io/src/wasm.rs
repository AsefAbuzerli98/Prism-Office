//! wasm-bindgen surface for the browser preview host.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct IoDocument(crate::Document);

#[wasm_bindgen]
impl IoDocument {
    /// Open DOCX / XLSX / PPTX bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<IoDocument, JsError> {
        crate::Document::open(bytes)
            .map(IoDocument)
            .map_err(|e| JsError::new(&e))
    }

    /// A new blank document ("document" | "workbook" | "presentation").
    pub fn blank(kind: &str) -> Result<IoDocument, JsError> {
        crate::Document::blank(kind)
            .map(IoDocument)
            .map_err(|e| JsError::new(&e))
    }

    /// "document" | "workbook" | "presentation"
    pub fn kind(&self) -> String {
        self.0.kind_name().into()
    }

    /// Editable model (JSON).
    pub fn model(&self) -> String {
        self.0.model().into()
    }

    #[wasm_bindgen(js_name = setModel)]
    pub fn set_model(&mut self, json: &str) {
        self.0.set_model(json)
    }

    /// File bytes with the current model (`now` = Excel serial for TODAY/NOW).
    pub fn save(&mut self, now: f64) -> Result<Vec<u8>, JsError> {
        self.0.save(now).map_err(|e| JsError::new(&e))
    }

    /// `[mime, bytes]` of a picture, or undefined.
    pub fn image(&self, key: &str) -> Option<js_sys_free::Pair> {
        self.0
            .image(key)
            .map(|(mime, bytes)| js_sys_free::Pair { mime, bytes })
    }

    #[wasm_bindgen(js_name = addImage)]
    pub fn add_image(&mut self, bytes: &[u8]) -> Result<String, JsError> {
        self.0.add_image(bytes).map_err(|e| JsError::new(&e))
    }
}

/// Small helper type so pictures cross without js-sys.
pub mod js_sys_free {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(getter_with_clone)]
    pub struct Pair {
        pub mime: String,
        pub bytes: Vec<u8>,
    }
}
