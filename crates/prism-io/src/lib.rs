//! An open document held in memory, like the host's `DocumentRegistry`
//! entry: the original package (for lossless saving) plus its editable
//! model as JSON. Used by the browser preview, where there is no host.

use prism_openxml::{DocumentKind, Package};

#[cfg(feature = "wasm")]
pub mod wasm;

pub enum Source {
    Word,
    Sheet(prism_sheet::xlsx::XlsxSource),
    Slides(prism_slide::SlideSource),
}

pub struct Document {
    pub kind: DocumentKind,
    pub package: Package,
    source: Source,
    /// Editable model as JSON (the same shape the host sends the UI).
    model: String,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Document {
    /// Open file bytes (any of DOCX / XLSX / PPTX).
    pub fn open(bytes: &[u8]) -> Result<Document, String> {
        let package = Package::from_bytes(bytes).map_err(err)?;
        let kind = package.kind().map_err(err)?;
        let (source, model) = match kind {
            DocumentKind::Document => {
                let doc = prism_word::read_package(&package).map_err(err)?;
                (Source::Word, serde_json::to_string(&doc).map_err(err)?)
            }
            DocumentKind::Workbook => {
                let (m, src) = prism_sheet::xlsx::read_package(package.clone()).map_err(err)?;
                (Source::Sheet(src), serde_json::to_string(&m).map_err(err)?)
            }
            DocumentKind::Presentation => {
                let (m, src) = prism_slide::read_package(package.clone()).map_err(err)?;
                (Source::Slides(src), serde_json::to_string(&m).map_err(err)?)
            }
        };
        Ok(Document {
            kind,
            package,
            source,
            model,
        })
    }

    /// A new blank document of a kind ("document" | "workbook" | "presentation").
    pub fn blank(kind: &str) -> Result<Document, String> {
        let k = match kind {
            "document" => DocumentKind::Document,
            "workbook" => DocumentKind::Workbook,
            "presentation" => DocumentKind::Presentation,
            _ => return Err(format!("unknown kind {kind}")),
        };
        let bytes = prism_openxml::templates::blank(k).to_bytes().map_err(err)?;
        Document::open(&bytes)
    }

    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            DocumentKind::Document => "document",
            DocumentKind::Workbook => "workbook",
            DocumentKind::Presentation => "presentation",
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, json: &str) {
        self.model = json.to_string();
    }

    /// Serialise the current model into the original package.
    pub fn save(&mut self, now: f64) -> Result<Vec<u8>, String> {
        match &mut self.source {
            Source::Word => {
                let mut doc: prism_word::WordDocument =
                    serde_json::from_str(&self.model).map_err(err)?;
                // read-only catalogues always come from the package
                let base = prism_word::read_package(&self.package).map_err(err)?;
                doc.styles = base.styles;
                doc.numbering = base.numbering;
                doc.default_run = base.default_run;
                doc.default_para = base.default_para;
                prism_word::write_package(&mut self.package, &doc).map_err(err)?;
            }
            Source::Sheet(src) => {
                let m: prism_sheet::model::WorkbookModel =
                    serde_json::from_str(&self.model).map_err(err)?;
                self.package = prism_sheet::xlsx::write_package(&m, Some(src), now).map_err(err)?;
                let (m2, src2) =
                    prism_sheet::xlsx::read_package(self.package.clone()).map_err(err)?;
                *src = src2;
                self.model = serde_json::to_string(&m2).map_err(err)?;
            }
            Source::Slides(src) => {
                let m: prism_slide::PresentationModel =
                    serde_json::from_str(&self.model).map_err(err)?;
                self.package = prism_slide::write_package(&m, src).map_err(err)?;
                let (m2, src2) = prism_slide::read_package(self.package.clone()).map_err(err)?;
                *src = src2;
                self.model = serde_json::to_string(&m2).map_err(err)?;
            }
        }
        self.package.to_bytes().map_err(err)
    }

    /// Picture bytes: Word relationship id, or a presentation part name.
    pub fn image(&self, key: &str) -> Option<(String, Vec<u8>)> {
        match &self.source {
            Source::Word => prism_word::image_data(&self.package, key).ok().flatten(),
            Source::Slides(src) => src.part_bytes(key),
            Source::Sheet(_) => None,
        }
    }

    /// Store a picture; returns `{relId,cx,cy}` (Word) or the part name (slides) as JSON.
    pub fn add_image(&mut self, bytes: &[u8]) -> Result<String, String> {
        match &mut self.source {
            Source::Word => {
                let (rel, cx, cy) =
                    prism_word::add_image(&mut self.package, bytes.to_vec()).map_err(err)?;
                Ok(serde_json::json!({ "relId": rel, "cx": cx, "cy": cy }).to_string())
            }
            Source::Slides(src) => Ok(serde_json::Value::String(src.add_image(bytes)).to_string()),
            Source::Sheet(_) => Err("pictures are not stored in workbooks here".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(p: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(p),
        )
        .unwrap()
    }

    #[test]
    fn opens_edits_and_saves_all_three_kinds() {
        for (p, kind) in [
            ("crates/prism-word/tests/corpus/SampleDoc.docx", "document"),
            (
                "crates/prism-sheet/tests/fixtures/openpyxl.xlsx",
                "workbook",
            ),
            (
                "crates/prism-slide/tests/fixtures/python-pptx.pptx",
                "presentation",
            ),
        ] {
            let mut d = Document::open(&fixture(p)).unwrap();
            assert_eq!(d.kind_name(), kind);
            let model = d.model().to_string();
            assert!(model.len() > 100);
            let out = d.save(45000.0).unwrap();
            let again = Document::open(&out).unwrap();
            assert_eq!(again.kind_name(), kind);
        }
    }
}
