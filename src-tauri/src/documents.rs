//! Open-document registry shared by all windows (Tauri-independent so it is
//! unit-testable without a webview).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use prism_openxml::{templates, DocumentKind, Issue, Package};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error(transparent)]
    OpenXml(#[from] prism_openxml::Error),
    #[error("no open document with id {0}")]
    NotFound(u64),
    #[error("document has never been saved; a path is required")]
    PathRequired,
    #[error(transparent)]
    Word(#[from] prism_word::Error),
    #[error("document {0} is not a word-processing document")]
    NotWord(u64),
    #[error(transparent)]
    Sheet(#[from] prism_sheet::xlsx::XlsxError),
    #[error("document {0} is not a spreadsheet")]
    NotSheet(u64),
    #[error(transparent)]
    Slide(#[from] prism_slide::SlideError),
    #[error("document {0} is not a presentation")]
    NotSlides(u64),
    #[error("file extension `.{ext}` does not match a {kind:?}")]
    ExtensionMismatch { ext: String, kind: DocumentKind },
}

impl Serialize for DocError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// Lightweight description of an open document sent to the UI.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub id: u64,
    pub kind: DocumentKind,
    pub title: String,
    pub path: Option<String>,
    pub part_count: usize,
    pub issues: Vec<Issue>,
}

struct OpenDocument {
    kind: DocumentKind,
    title: String,
    path: Option<PathBuf>,
    package: Package,
    /// Parsed word-processing model (lazily created; authoritative once edited).
    word: Option<prism_word::WordDocument>,
    /// The model has changes not yet written into `package`.
    word_dirty: bool,
    /// Spreadsheet model and the package it was read from (lossless save).
    sheet: Option<(
        prism_sheet::model::WorkbookModel,
        prism_sheet::xlsx::XlsxSource,
    )>,
    sheet_dirty: bool,
    /// Presentation model and its source package (lossless save; pictures
    /// added in the editor live in the source package until saved).
    slides: Option<(prism_slide::PresentationModel, prism_slide::SlideSource)>,
    slides_dirty: bool,
    /// Auto-recovery journal key, edit revision and last journaled revision.
    recovery_key: String,
    revision: u64,
    journaled: u64,
}

#[derive(Default)]
pub struct DocumentRegistry {
    next_id: AtomicU64,
    docs: Mutex<HashMap<u64, OpenDocument>>,
    untitled: Mutex<HashMap<DocumentKind, u32>>,
}

/// Recovery journal of a presentation: the model plus pictures added since
/// the file was opened (they exist only in the in-memory source package).
fn slides_journal(
    model: &prism_slide::PresentationModel,
    src: &prism_slide::SlideSource,
    saved: &Package,
) -> serde_json::Value {
    use base64::Engine;
    let mut media = serde_json::Map::new();
    for name in src.package.part_names() {
        if name.as_str().starts_with("/ppt/media/") && !saved.has_part(name) {
            if let Some(b) = src.package.part(name) {
                media.insert(
                    name.as_str().to_string(),
                    base64::engine::general_purpose::STANDARD.encode(b).into(),
                );
            }
        }
    }
    serde_json::json!({ "model": model, "media": media })
}

/// "Now" as an Excel serial in the computer's local time zone (TODAY/NOW
/// cached in saved files match what the user sees).
pub fn now_serial() -> f64 {
    let now = chrono::Local::now();
    let offset_min = now.offset().local_minus_utc() as f64 / 60.0;
    prism_sheet::functions::date::serial_from_unix_ms(now.timestamp_millis() as f64, offset_min)
}

fn title_from_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into())
}

impl DocumentRegistry {
    fn insert(&self, mut doc: OpenDocument) -> DocumentInfo {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        doc.recovery_key = crate::recovery::RecoveryStore::key_for(id);
        let info = Self::info(id, &doc);
        self.docs.lock().unwrap().insert(id, doc);
        info
    }

    fn info(id: u64, doc: &OpenDocument) -> DocumentInfo {
        DocumentInfo {
            id,
            kind: doc.kind,
            title: doc.title.clone(),
            path: doc.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            part_count: doc.package.len(),
            issues: doc.package.validate(),
        }
    }

    pub fn create(&self, kind: DocumentKind) -> DocumentInfo {
        let n = {
            let mut u = self.untitled.lock().unwrap();
            let n = u.entry(kind).or_insert(0);
            *n += 1;
            *n
        };
        let base = match kind {
            DocumentKind::Document => "Document",
            DocumentKind::Workbook => "Workbook",
            DocumentKind::Presentation => "Presentation",
        };
        self.insert(OpenDocument {
            kind,
            title: format!("{base} {n}"),
            path: None,
            package: templates::blank(kind),
            word: None,
            word_dirty: false,
            sheet: None,
            sheet_dirty: false,
            slides: None,
            slides_dirty: false,
            recovery_key: String::new(),
            revision: 0,
            journaled: 0,
        })
    }

    pub fn open(&self, path: &Path) -> Result<DocumentInfo, DocError> {
        let package = Package::open(path)?;
        let kind = package.kind()?;
        Ok(self.insert(OpenDocument {
            kind,
            title: title_from_path(path),
            path: Some(path.to_path_buf()),
            package,
            word: None,
            word_dirty: false,
            sheet: None,
            sheet_dirty: false,
            slides: None,
            slides_dirty: false,
            recovery_key: String::new(),
            revision: 0,
            journaled: 0,
        }))
    }

    pub fn save(&self, id: u64, path: Option<&Path>) -> Result<DocumentInfo, DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        let target = match path {
            Some(p) => p.to_path_buf(),
            None => doc.path.clone().ok_or(DocError::PathRequired)?,
        };
        if let Some(ext) = target.extension().map(|e| e.to_string_lossy().into_owned()) {
            if DocumentKind::from_extension(&ext) != Some(doc.kind) {
                return Err(DocError::ExtensionMismatch {
                    ext,
                    kind: doc.kind,
                });
            }
        }
        if doc.word_dirty {
            if let Some(w) = &doc.word {
                prism_word::write_package(&mut doc.package, w)?;
                // pick up ids assigned on write (new headers, pictures)
                let mut fresh = prism_word::read_package(&doc.package)?;
                fresh.blocks = w.blocks.clone();
                doc.word = Some(fresh);
            }
            doc.word_dirty = false;
        }
        if doc.sheet_dirty {
            if let Some((m, src)) = &doc.sheet {
                doc.package = prism_sheet::xlsx::write_package(m, Some(src), now_serial())?;
            }
            doc.sheet_dirty = false;
        }
        if doc.slides_dirty {
            if let Some((m, src)) = &doc.slides {
                doc.package = prism_slide::write_package(m, src)?;
                // new slides/notes get part names; re-read for a fresh source
                doc.slides = Some(prism_slide::read_package(doc.package.clone())?);
            }
            doc.slides_dirty = false;
        }
        doc.package.save(&target)?;
        doc.title = title_from_path(&target);
        doc.path = Some(target);
        Ok(Self::info(id, doc))
    }

    /// The editable word-processing model of a document.
    pub fn word(&self, id: u64) -> Result<prism_word::WordDocument, DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        if doc.kind != DocumentKind::Document {
            return Err(DocError::NotWord(id));
        }
        if doc.word.is_none() {
            doc.word = Some(prism_word::read_package(&doc.package)?);
        }
        Ok(doc.word.clone().unwrap())
    }

    /// Replace the model after an edit. The read-only catalogues (styles,
    /// numbering) are always taken from the package, never from the UI.
    pub fn set_word(&self, id: u64, mut content: prism_word::WordDocument) -> Result<(), DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        if doc.kind != DocumentKind::Document {
            return Err(DocError::NotWord(id));
        }
        let base = match doc.word.take() {
            Some(w) => w,
            None => prism_word::read_package(&doc.package)?,
        };
        // Headers created in the editor carry an empty relId until saved;
        // adopt the id assigned on the previous save.
        for hf in &mut content.headers {
            if hf.rel_id.is_empty() {
                if let Some(b) = base
                    .headers
                    .iter()
                    .find(|b| b.kind == hf.kind && b.which == hf.which && !b.rel_id.is_empty())
                {
                    hf.rel_id = b.rel_id.clone();
                }
            }
        }
        content.styles = base.styles;
        content.numbering = base.numbering;
        content.default_run = base.default_run;
        content.default_para = base.default_para;
        doc.word = Some(content);
        doc.word_dirty = true;
        doc.revision += 1;
        Ok(())
    }

    fn ensure_sheet(doc: &mut OpenDocument, id: u64) -> Result<(), DocError> {
        if doc.kind != DocumentKind::Workbook {
            return Err(DocError::NotSheet(id));
        }
        if doc.sheet.is_none() {
            doc.sheet = Some(prism_sheet::xlsx::read_package(doc.package.clone())?);
        }
        Ok(())
    }

    /// The spreadsheet model of a workbook.
    pub fn sheet(&self, id: u64) -> Result<prism_sheet::model::WorkbookModel, DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        Self::ensure_sheet(doc, id)?;
        Ok(doc.sheet.as_ref().unwrap().0.clone())
    }

    /// Replace the spreadsheet model after an edit.
    pub fn set_sheet(
        &self,
        id: u64,
        model: prism_sheet::model::WorkbookModel,
    ) -> Result<(), DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        Self::ensure_sheet(doc, id)?;
        doc.sheet.as_mut().unwrap().0 = model;
        doc.sheet_dirty = true;
        doc.revision += 1;
        Ok(())
    }

    fn ensure_slides(doc: &mut OpenDocument, id: u64) -> Result<(), DocError> {
        if doc.kind != DocumentKind::Presentation {
            return Err(DocError::NotSlides(id));
        }
        if doc.slides.is_none() {
            doc.slides = Some(prism_slide::read_package(doc.package.clone())?);
        }
        Ok(())
    }

    /// The presentation model (masters, layouts, slides).
    pub fn slides(&self, id: u64) -> Result<prism_slide::PresentationModel, DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        Self::ensure_slides(doc, id)?;
        Ok(doc.slides.as_ref().unwrap().0.clone())
    }

    /// Replace the presentation model after an edit.
    pub fn set_slides(
        &self,
        id: u64,
        model: prism_slide::PresentationModel,
    ) -> Result<(), DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        Self::ensure_slides(doc, id)?;
        doc.slides.as_mut().unwrap().0 = model;
        doc.slides_dirty = true;
        doc.revision += 1;
        Ok(())
    }

    /// Bytes and content type of a presentation part (pictures, media).
    pub fn slide_part(&self, id: u64, part: &str) -> Result<Option<(String, Vec<u8>)>, DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        Self::ensure_slides(doc, id)?;
        Ok(doc.slides.as_ref().unwrap().1.part_bytes(part))
    }

    /// Store a picture; returns its part name for `Fill::Image`.
    pub fn add_slide_image(&self, id: u64, bytes: &[u8]) -> Result<String, DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        Self::ensure_slides(doc, id)?;
        Ok(doc.slides.as_mut().unwrap().1.add_image(bytes))
    }

    /// Journal every document edited since the last tick (auto-recovery).
    pub fn journal_changes(&self, store: &crate::recovery::RecoveryStore) -> usize {
        let mut docs = self.docs.lock().unwrap();
        let mut n = 0;
        for doc in docs.values_mut() {
            if doc.revision == doc.journaled {
                continue;
            }
            let content = match (&doc.word, &doc.sheet, &doc.slides) {
                (Some(w), _, _) => serde_json::to_value(w),
                (None, Some((m, _)), _) => serde_json::to_value(m),
                (None, None, Some((m, src))) => Ok(slides_journal(m, src, &doc.package)),
                _ => continue,
            };
            let Ok(content) = content else {
                continue;
            };
            let meta = crate::recovery::RecoveryMeta {
                title: doc.title.clone(),
                path: doc.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                kind: doc.kind,
                updated: 0,
            };
            if store.record(&doc.recovery_key, &meta, &content).is_ok() {
                doc.journaled = doc.revision;
                n += 1;
            }
        }
        n
    }

    /// The journal key of a document (to discard it after save/close).
    pub fn recovery_key(&self, id: u64) -> Option<String> {
        self.docs
            .lock()
            .unwrap()
            .get(&id)
            .map(|d| d.recovery_key.clone())
    }

    /// Mark a document's content as journaled-and-clean (after a save).
    pub fn mark_saved(&self, id: u64) {
        if let Some(d) = self.docs.lock().unwrap().get_mut(&id) {
            d.journaled = d.revision;
        }
    }

    /// Re-open a document from a recovery journal; it comes back modified.
    pub fn restore(
        &self,
        store: &crate::recovery::RecoveryStore,
        key: &str,
    ) -> Result<DocumentInfo, DocError> {
        let (meta, content) = store
            .restore(key)
            .map_err(|e| DocError::OpenXml(prism_openxml::Error::Io(e)))?;
        let original = meta.path.as_ref().map(PathBuf::from).filter(|p| p.exists());
        let info = match &original {
            Some(p) => self.open(p)?,
            None => self.create(meta.kind),
        };
        if meta.kind == DocumentKind::Document {
            let word: prism_word::WordDocument = serde_json::from_value(content).map_err(|e| {
                DocError::OpenXml(prism_openxml::Error::Xml {
                    part: "recovery".into(),
                    message: e.to_string(),
                })
            })?;
            self.set_word(info.id, word)?;
        } else if meta.kind == DocumentKind::Workbook {
            let model: prism_sheet::model::WorkbookModel = serde_json::from_value(content)
                .map_err(|e| {
                    DocError::OpenXml(prism_openxml::Error::Xml {
                        part: "recovery".into(),
                        message: e.to_string(),
                    })
                })?;
            self.set_sheet(info.id, model)?;
        } else if meta.kind == DocumentKind::Presentation {
            let model: prism_slide::PresentationModel =
                serde_json::from_value(content["model"].clone()).map_err(|e| {
                    DocError::OpenXml(prism_openxml::Error::Xml {
                        part: "recovery".into(),
                        message: e.to_string(),
                    })
                })?;
            {
                use base64::Engine;
                let mut docs = self.docs.lock().unwrap();
                let doc = docs.get_mut(&info.id).ok_or(DocError::NotFound(info.id))?;
                Self::ensure_slides(doc, info.id)?;
                let src = &mut doc.slides.as_mut().unwrap().1;
                if let Some(media) = content["media"].as_object() {
                    for (part, v) in media {
                        let (Ok(name), Some(data)) =
                            (prism_openxml::PartName::new(part), v.as_str())
                        else {
                            continue;
                        };
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
                            let ct = match name.extension().as_deref() {
                                Some("jpeg" | "jpg") => "image/jpeg",
                                Some("gif") => "image/gif",
                                Some("svg") => "image/svg+xml",
                                _ => "image/png",
                            };
                            src.package.set_part(name, bytes, Some(ct));
                        }
                    }
                }
            }
            self.set_slides(info.id, model)?;
        }
        {
            let mut docs = self.docs.lock().unwrap();
            if let Some(d) = docs.get_mut(&info.id) {
                d.title = format!("{} (recovered)", meta.title);
                if original.is_none() {
                    d.path = None;
                }
            }
        }
        store.discard(key);
        self.get(info.id)
    }

    fn with_word_package<T>(
        &self,
        id: u64,
        f: impl FnOnce(&mut OpenDocument) -> Result<T, DocError>,
    ) -> Result<T, DocError> {
        let mut docs = self.docs.lock().unwrap();
        let doc = docs.get_mut(&id).ok_or(DocError::NotFound(id))?;
        if doc.kind != DocumentKind::Document {
            return Err(DocError::NotWord(id));
        }
        f(doc)
    }

    /// Picture bytes for an `Inline::Image` relationship id.
    pub fn word_image(&self, id: u64, rel_id: &str) -> Result<Option<(String, Vec<u8>)>, DocError> {
        self.with_word_package(id, |d| Ok(prism_word::image_data(&d.package, rel_id)?))
    }

    /// Store picture bytes in the package; returns `(relId, cx, cy)`.
    pub fn add_word_image(&self, id: u64, bytes: Vec<u8>) -> Result<(String, u64, u64), DocError> {
        self.with_word_package(id, |d| Ok(prism_word::add_image(&mut d.package, bytes)?))
    }

    pub fn word_fonts(&self, id: u64) -> Result<Vec<prism_word::EmbeddedFont>, DocError> {
        self.with_word_package(id, |d| Ok(prism_word::embedded_fonts(&d.package)?))
    }

    pub fn get(&self, id: u64) -> Result<DocumentInfo, DocError> {
        let docs = self.docs.lock().unwrap();
        let doc = docs.get(&id).ok_or(DocError::NotFound(id))?;
        Ok(Self::info(id, doc))
    }

    pub fn close(&self, id: u64) -> Result<(), DocError> {
        self.docs
            .lock()
            .unwrap()
            .remove(&id)
            .map(|_| ())
            .ok_or(DocError::NotFound(id))
    }

    pub fn list(&self) -> Vec<DocumentInfo> {
        let docs = self.docs.lock().unwrap();
        let mut v: Vec<_> = docs.iter().map(|(id, d)| Self::info(*id, d)).collect();
        v.sort_by_key(|d| d.id);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_recovery_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::recovery::RecoveryStore::new(dir.path().to_path_buf());
        let reg = DocumentRegistry::default();
        let d = reg.create(DocumentKind::Document);
        assert_eq!(reg.journal_changes(&store), 0, "nothing edited yet");
        let mut w = reg.word(d.id).unwrap();
        w.blocks = vec![prism_word::Block::Paragraph(prism_word::Paragraph::plain(
            "unsaved work",
        ))];
        reg.set_word(d.id, w.clone()).unwrap();
        assert_eq!(reg.journal_changes(&store), 1);
        assert_eq!(reg.journal_changes(&store), 0, "no change since last tick");
        w.blocks
            .push(prism_word::Block::Paragraph(prism_word::Paragraph::plain(
                "more",
            )));
        reg.set_word(d.id, w).unwrap();
        reg.journal_changes(&store);

        // "crash": a new process with a new registry finds and restores the journal
        let store2 = crate::recovery::RecoveryStore::new(dir.path().to_path_buf());
        let found = store2.list();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].meta.title, "Document 1");
        let reg2 = DocumentRegistry::default();
        let info = reg2.restore(&store2, &found[0].key).unwrap();
        assert_eq!(info.title, "Document 1 (recovered)");
        assert_eq!(
            reg2.word(info.id).unwrap().plain_text(),
            "unsaved work\nmore"
        );
        assert!(store2.list().is_empty(), "restored journal is consumed");
    }

    #[test]
    fn word_model_edit_and_save() {
        let reg = DocumentRegistry::default();
        let d = reg.create(DocumentKind::Document);
        let mut w = reg.word(d.id).unwrap();
        assert_eq!(w.blocks.len(), 1);
        w.blocks = vec![prism_word::Block::Paragraph(prism_word::Paragraph::plain(
            "Salam dünya",
        ))];
        w.styles.clear(); // UI must not be able to clobber catalogues
        reg.set_word(d.id, w).unwrap();
        assert!(!reg.word(d.id).unwrap().styles.is_empty());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.docx");
        reg.save(d.id, Some(&path)).unwrap();
        let reopened = reg.open(&path).unwrap();
        assert_eq!(reg.word(reopened.id).unwrap().plain_text(), "Salam dünya");

        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(10u32.to_be_bytes());
        png.extend(20u32.to_be_bytes());
        let (rel, cx, cy) = reg.add_word_image(reopened.id, png.clone()).unwrap();
        assert_eq!((cx, cy), (95250, 190500));
        let (mime, data) = reg.word_image(reopened.id, &rel).unwrap().unwrap();
        assert_eq!((mime.as_str(), data), ("image/png", png));
        assert!(reg.word_fonts(reopened.id).unwrap().is_empty());
    }

    #[test]
    fn workbook_model_edit_save_and_recover() {
        let dir = std::env::temp_dir().join(format!("prism-sheet-doc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = DocumentRegistry::default();
        let d = reg.create(DocumentKind::Workbook);
        let mut m = reg.sheet(d.id).unwrap();
        assert_eq!(m.sheets[0].name, "Sheet1");
        m.sheets[0].cells.push(prism_sheet::model::CellModel {
            r: 0,
            c: 0,
            f: None,
            v: Some(prism_sheet::model::CellValueModel::Number(21.0)),
            s: 0,
        });
        m.sheets[0].cells.push(prism_sheet::model::CellModel {
            r: 0,
            c: 1,
            f: Some("A1*2".into()),
            v: None,
            s: 0,
        });
        reg.set_sheet(d.id, m.clone()).unwrap();
        let store = crate::recovery::RecoveryStore::new(dir.join("rec"));
        assert_eq!(reg.journal_changes(&store), 1);
        let path = dir.join("Book.xlsx");
        reg.save(d.id, Some(&path)).unwrap();
        let reopened = reg.open(&path).unwrap();
        let m2 = reg.sheet(reopened.id).unwrap();
        let b1 = m2.sheets[0].cells.iter().find(|c| c.c == 1).unwrap();
        assert_eq!(b1.v, Some(prism_sheet::model::CellValueModel::Number(42.0)));
        // recovery of an unsaved workbook
        let key = reg.recovery_key(d.id).unwrap();
        let restored = reg.restore(&store, &key).unwrap();
        assert_eq!(reg.sheet(restored.id).unwrap().sheets[0].cells.len(), 2);
        assert!(matches!(reg.word(restored.id), Err(DocError::NotWord(_))));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_save_open_close() {
        let reg = DocumentRegistry::default();
        let a = reg.create(DocumentKind::Workbook);
        let b = reg.create(DocumentKind::Workbook);
        assert_eq!(
            (a.title.as_str(), b.title.as_str()),
            ("Workbook 1", "Workbook 2")
        );
        assert!(a.issues.is_empty());

        assert!(matches!(reg.save(a.id, None), Err(DocError::PathRequired)));

        let dir = tempfile::tempdir().unwrap();
        let wrong = dir.path().join("x.docx");
        assert!(matches!(
            reg.save(a.id, Some(&wrong)),
            Err(DocError::ExtensionMismatch { .. })
        ));

        let path = dir.path().join("Budget.xlsx");
        let saved = reg.save(a.id, Some(&path)).unwrap();
        assert_eq!(saved.title, "Budget.xlsx");

        let opened = reg.open(&path).unwrap();
        assert_eq!(opened.kind, DocumentKind::Workbook);
        assert_eq!(reg.list().len(), 3);

        assert!(matches!(reg.word(opened.id), Err(DocError::NotWord(_))));
        reg.close(opened.id).unwrap();
        assert!(matches!(reg.get(opened.id), Err(DocError::NotFound(_))));
    }

    #[test]
    fn presentation_model_save_and_recovery() {
        let dir = std::env::temp_dir().join(format!("prism-slide-doc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = DocumentRegistry::default();
        let d = reg.create(DocumentKind::Presentation);
        let mut m = reg.slides(d.id).unwrap();
        assert_eq!(m.masters[0].layouts.len(), 6);
        let png = [0x89u8, b'P', b'N', b'G', 1, 2, 3];
        let part = reg.add_slide_image(d.id, &png).unwrap();
        assert!(part.starts_with("/ppt/media/"));
        let mut slide = m.slides[0].clone();
        slide.part = None;
        slide.notes = "second".into();
        slide.layout = m.masters[0].layouts[5].part.clone();
        let pic: prism_drawingml::shape::Node = serde_json::from_value(serde_json::json!({
            "type": "picture", "id": 10, "name": "Picture 1",
            "xfrm": {"x": 0, "y": 0, "cx": 914400, "cy": 914400},
            "image": {"t": "image", "relId": part}
        }))
        .unwrap();
        slide.shapes = vec![pic];
        m.slides.push(slide);
        reg.set_slides(d.id, m).unwrap();
        let store = crate::recovery::RecoveryStore::new(dir.join("rec"));
        assert_eq!(reg.journal_changes(&store), 1);
        // recovery of the unsaved presentation brings back the picture bytes
        let key = reg.recovery_key(d.id).unwrap();
        let restored = reg.restore(&store, &key).unwrap();
        let rm = reg.slides(restored.id).unwrap();
        assert_eq!(rm.slides.len(), 2);
        assert_eq!(reg.slide_part(restored.id, &part).unwrap().unwrap().1, png);
        // save and reopen
        let path = dir.join("Deck.pptx");
        reg.save(d.id, Some(&path)).unwrap();
        let reopened = reg.open(&path).unwrap();
        let m2 = reg.slides(reopened.id).unwrap();
        assert_eq!(m2.slides.len(), 2);
        assert_eq!(m2.slides[1].notes, "second");
        let prism_drawingml::shape::Node::Picture(p) = &m2.slides[1].shapes[0] else {
            panic!()
        };
        let prism_drawingml::fill::Fill::Image { rel_id, .. } = &p.image else {
            panic!()
        };
        assert_eq!(reg.slide_part(reopened.id, rel_id).unwrap().unwrap().1, png);
        assert!(matches!(reg.sheet(reopened.id), Err(DocError::NotSheet(_))));
        std::fs::remove_dir_all(&dir).ok();
    }
}
