//! Prism Office native host (Tauri v2).
//!
//! Owns OS integration, file I/O and the open-document registry. The
//! TypeScript shell talks to it exclusively through the typed commands below.

pub mod documents;

use std::path::PathBuf;

use documents::{DocError, DocumentInfo, DocumentRegistry};
use prism_openxml::DocumentKind;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Prism Office",
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

#[tauri::command]
fn new_document(kind: DocumentKind, docs: State<'_, DocumentRegistry>) -> DocumentInfo {
    docs.create(kind)
}

#[tauri::command]
fn open_document(
    path: PathBuf,
    docs: State<'_, DocumentRegistry>,
) -> Result<DocumentInfo, DocError> {
    docs.open(&path)
}

#[tauri::command]
fn save_document(
    id: u64,
    path: Option<PathBuf>,
    docs: State<'_, DocumentRegistry>,
) -> Result<DocumentInfo, DocError> {
    docs.save(id, path.as_deref())
}

#[tauri::command]
fn close_document(id: u64, docs: State<'_, DocumentRegistry>) -> Result<(), DocError> {
    docs.close(id)
}

#[tauri::command]
fn list_documents(docs: State<'_, DocumentRegistry>) -> Vec<DocumentInfo> {
    docs.list()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(DocumentRegistry::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            new_document,
            open_document,
            save_document,
            close_document,
            list_documents
        ])
        .run(tauri::generate_context!())
        .expect("error while running Prism Office");
}
