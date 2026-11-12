//! Prism Office native host (Tauri v2).
//!
//! Owns OS integration, file I/O, shared daemons and the open-document
//! registry. The TypeScript shell talks to it exclusively through the typed
//! commands below and the `prism://event` broadcast channel.

pub mod documents;
pub mod events;
pub mod launch;
pub mod recent;
pub mod services;
pub mod settings;
mod store;
pub mod windows;

use std::path::PathBuf;

use documents::{DocError, DocumentInfo, DocumentRegistry};
use events::{AppEvent, EventBus};
use launch::PendingOpens;
use prism_openxml::DocumentKind;
use recent::{RecentFile, RecentStore};
use serde::Serialize;
use services::{ServiceInfo, ServiceRegistry};
use settings::{Settings, SettingsStore};
use tauri::{AppHandle, Emitter, Manager, State};

pub const EVENT_CHANNEL: &str = "prism://event";

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

// ---- documents --------------------------------------------------------------

#[tauri::command]
fn new_document(
    kind: DocumentKind,
    docs: State<'_, DocumentRegistry>,
    bus: State<'_, EventBus>,
) -> DocumentInfo {
    let info = docs.create(kind);
    bus.publish(AppEvent::DocumentOpened { info: info.clone() });
    info
}

#[tauri::command]
fn open_document(
    path: PathBuf,
    docs: State<'_, DocumentRegistry>,
    recent: State<'_, RecentStore>,
    bus: State<'_, EventBus>,
) -> Result<DocumentInfo, DocError> {
    let info = docs.open(&path)?;
    recent.touch(&path, info.kind);
    bus.publish(AppEvent::DocumentOpened { info: info.clone() });
    bus.publish(AppEvent::RecentChanged);
    Ok(info)
}

#[tauri::command]
fn get_document(id: u64, docs: State<'_, DocumentRegistry>) -> Result<DocumentInfo, DocError> {
    docs.get(id)
}

#[tauri::command]
fn save_document(
    id: u64,
    path: Option<PathBuf>,
    docs: State<'_, DocumentRegistry>,
    recent: State<'_, RecentStore>,
    bus: State<'_, EventBus>,
) -> Result<DocumentInfo, DocError> {
    let info = docs.save(id, path.as_deref())?;
    if let Some(p) = &info.path {
        recent.touch(std::path::Path::new(p), info.kind);
        bus.publish(AppEvent::RecentChanged);
    }
    bus.publish(AppEvent::DocumentSaved { info: info.clone() });
    Ok(info)
}

#[tauri::command]
fn close_document(
    id: u64,
    docs: State<'_, DocumentRegistry>,
    bus: State<'_, EventBus>,
) -> Result<(), DocError> {
    docs.close(id)?;
    bus.publish(AppEvent::DocumentClosed { id });
    Ok(())
}

#[tauri::command]
fn list_documents(docs: State<'_, DocumentRegistry>) -> Vec<DocumentInfo> {
    docs.list()
}

#[tauri::command]
fn get_word_document(
    id: u64,
    docs: State<'_, DocumentRegistry>,
) -> Result<prism_word::WordDocument, DocError> {
    docs.word(id)
}

#[tauri::command]
fn update_word_document(
    id: u64,
    content: prism_word::WordDocument,
    docs: State<'_, DocumentRegistry>,
) -> Result<(), DocError> {
    docs.set_word(id, content)
}

// ---- windows -----------------------------------------------------------------

#[tauri::command]
fn open_document_window(
    id: u64,
    app: AppHandle,
    docs: State<'_, DocumentRegistry>,
) -> Result<(), String> {
    let info = docs.get(id).map_err(|e| e.to_string())?;
    windows::open_document_window(&app, id, &info.title)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn take_pending_opens(pending: State<'_, PendingOpens>) -> Vec<String> {
    pending.take()
}

// ---- recent / settings / services ---------------------------------------------

#[tauri::command]
fn recent_files(recent: State<'_, RecentStore>) -> Vec<RecentFile> {
    recent.list()
}

#[tauri::command]
fn remove_recent(path: String, recent: State<'_, RecentStore>, bus: State<'_, EventBus>) {
    recent.remove(&path);
    bus.publish(AppEvent::RecentChanged);
}

#[tauri::command]
fn clear_recent(recent: State<'_, RecentStore>, bus: State<'_, EventBus>) {
    recent.clear();
    bus.publish(AppEvent::RecentChanged);
}

#[tauri::command]
fn get_settings(settings: State<'_, SettingsStore>) -> Settings {
    settings.get()
}

#[tauri::command]
fn set_settings(
    value: Settings,
    settings: State<'_, SettingsStore>,
    bus: State<'_, EventBus>,
) -> Result<Settings, String> {
    let saved = settings.set(value).map_err(|e| e.to_string())?;
    bus.publish(AppEvent::SettingsChanged {
        settings: saved.clone(),
    });
    Ok(saved)
}

#[tauri::command]
fn list_services(services: State<'_, ServiceRegistry>) -> Vec<ServiceInfo> {
    services.list()
}

// ---- app ---------------------------------------------------------------------

fn deliver_open_files(app: &AppHandle, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let as_str: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    app.state::<PendingOpens>().push(paths);
    app.state::<EventBus>()
        .publish(AppEvent::OpenFiles { paths: as_str });
    windows::focus_main(app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be registered first: a second launch forwards its argv here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let paths = launch::document_args(argv, std::path::Path::new(&cwd));
            if paths.is_empty() {
                windows::focus_main(app);
            } else {
                deliver_open_files(app, paths);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(DocumentRegistry::default())
        .manage(EventBus::default())
        .manage(PendingOpens::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir().ok();
            let config_dir = app.path().app_config_dir().ok();
            app.manage(RecentStore::load(
                data_dir.map(|d| d.join("recent.json")),
                20,
            ));
            app.manage(SettingsStore::load(
                config_dir.map(|d| d.join("settings.json")),
            ));

            let bus = app.state::<EventBus>().inner().clone();
            let emitter = handle.clone();
            bus.subscribe(move |event| {
                let _ = emitter.emit(EVENT_CHANNEL, event);
            });

            let registry = services::default_registry();
            registry.start_all(&bus);
            app.manage(registry);

            let cwd = std::env::current_dir().unwrap_or_default();
            app.state::<PendingOpens>()
                .push(launch::document_args(std::env::args(), &cwd));

            windows::create_main(&handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // A dedicated document window owns its document: closing it closes the doc.
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(id) = windows::doc_id_from_label(window.label()) {
                    let app = window.app_handle();
                    if app.state::<DocumentRegistry>().close(id).is_ok() {
                        app.state::<EventBus>()
                            .publish(AppEvent::DocumentClosed { id });
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            new_document,
            open_document,
            get_document,
            save_document,
            close_document,
            list_documents,
            get_word_document,
            update_word_document,
            open_document_window,
            take_pending_opens,
            recent_files,
            remove_recent,
            clear_recent,
            get_settings,
            set_settings,
            list_services
        ])
        .build(tauri::generate_context!())
        .expect("error while building Prism Office");

    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            let paths = urls
                .into_iter()
                .filter_map(|u| u.to_file_path().ok())
                .collect();
            deliver_open_files(app, paths);
        }
        tauri::RunEvent::Exit => {
            let bus = app.state::<EventBus>().inner().clone();
            app.state::<ServiceRegistry>().stop_all(&bus);
        }
        _ => {}
    });
}
