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
}

#[derive(Default)]
pub struct DocumentRegistry {
    next_id: AtomicU64,
    docs: Mutex<HashMap<u64, OpenDocument>>,
    untitled: Mutex<HashMap<DocumentKind, u32>>,
}

fn title_from_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into())
}

impl DocumentRegistry {
    fn insert(&self, doc: OpenDocument) -> DocumentInfo {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
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
        doc.package.save(&target)?;
        doc.title = title_from_path(&target);
        doc.path = Some(target);
        Ok(Self::info(id, doc))
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

        reg.close(opened.id).unwrap();
        assert!(matches!(reg.get(opened.id), Err(DocError::NotFound(_))));
    }
}
