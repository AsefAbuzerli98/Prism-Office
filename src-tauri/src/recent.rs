//! Most-recently-used files, persisted to `recent.json` in the app data dir.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use prism_openxml::DocumentKind;
use serde::{Deserialize, Serialize};

use crate::store;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentFile {
    pub path: String,
    pub kind: DocumentKind,
    /// Unix epoch milliseconds.
    pub opened_at: u64,
    /// Computed at list time; not persisted meaningfully.
    #[serde(default)]
    pub exists: bool,
}

pub struct RecentStore {
    path: Option<PathBuf>,
    items: Mutex<Vec<RecentFile>>,
    max: usize,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn same_path(a: &str, b: &str) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

impl RecentStore {
    pub fn load(path: Option<PathBuf>, max: usize) -> Self {
        let items: Vec<RecentFile> = store::load(path.as_deref());
        Self {
            path,
            items: Mutex::new(items),
            max,
        }
    }

    pub fn touch(&self, file: &Path, kind: DocumentKind) {
        let p = file.to_string_lossy().into_owned();
        let mut items = self.items.lock().unwrap();
        items.retain(|r| !same_path(&r.path, &p));
        items.insert(
            0,
            RecentFile {
                path: p,
                kind,
                opened_at: now_ms(),
                exists: true,
            },
        );
        items.truncate(self.max);
        let _ = store::save(self.path.as_ref(), &*items);
    }

    pub fn remove(&self, file: &str) {
        let mut items = self.items.lock().unwrap();
        items.retain(|r| !same_path(&r.path, file));
        let _ = store::save(self.path.as_ref(), &*items);
    }

    pub fn clear(&self) {
        let mut items = self.items.lock().unwrap();
        items.clear();
        let _ = store::save(self.path.as_ref(), &*items);
    }

    pub fn list(&self) -> Vec<RecentFile> {
        self.items
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .map(|mut r| {
                r.exists = Path::new(&r.path).exists();
                r
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mru_order_dedupe_cap_and_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let store_path = dir.path().join("recent.json");
        let f = |n: &str| {
            let p = dir.path().join(n);
            std::fs::write(&p, b"x").unwrap();
            p
        };
        let (a, b, c) = (f("a.docx"), f("b.xlsx"), f("c.pptx"));
        {
            let s = RecentStore::load(Some(store_path.clone()), 2);
            s.touch(&a, DocumentKind::Document);
            s.touch(&b, DocumentKind::Workbook);
            s.touch(&a, DocumentKind::Document);
            s.touch(&c, DocumentKind::Presentation);
            let names: Vec<_> = s.list().iter().map(|r| r.path.clone()).collect();
            assert_eq!(names, vec![c.to_string_lossy(), a.to_string_lossy()]);
        }
        std::fs::remove_file(&a).unwrap();
        let s = RecentStore::load(Some(store_path), 2);
        let l = s.list();
        assert_eq!(l.len(), 2);
        assert!(l[0].exists);
        assert!(!l[1].exists, "deleted file must be flagged");
        s.clear();
        assert!(s.list().is_empty());
    }
}
