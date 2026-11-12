//! Native window construction and platform styling (P1-04, P1-05).

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
pub const DOC_PREFIX: &str = "doc-";

pub fn doc_window_label(doc_id: u64) -> String {
    format!("{DOC_PREFIX}{doc_id}")
}

pub fn doc_id_from_label(label: &str) -> Option<u64> {
    label.strip_prefix(DOC_PREFIX)?.parse().ok()
}

/// Apply per-platform chrome:
/// - macOS: overlay title bar with inset traffic lights (content draws under it).
/// - Windows: Mica backdrop on a transparent window; native caption keeps Snap Layouts.
/// - Linux: native decorations.
fn styled<'a, R: Runtime, M: Manager<R>>(
    b: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    let b = b.min_inner_size(900.0, 600.0);
    #[cfg(target_os = "macos")]
    let b = b
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 20.0));
    #[cfg(target_os = "windows")]
    let b = {
        use tauri::utils::config::WindowEffectsConfig;
        use tauri::window::{Effect, EffectState};
        b.transparent(true).effects(WindowEffectsConfig {
            effects: vec![Effect::Mica],
            state: Some(EffectState::FollowsWindowActiveState),
            ..Default::default()
        })
    };
    b
}

/// Script injected before the page loads, telling the UI which window it is.
fn context_script(label: &str, doc_id: Option<u64>) -> String {
    format!(
        "window.__PRISM_WINDOW__ = {{ label: {:?}, docId: {}, platform: {:?} }};",
        label,
        doc_id.map_or("null".to_string(), |d| d.to_string()),
        std::env::consts::OS
    )
}

pub fn create_main<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    styled(
        WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
            .title("Prism Office")
            .inner_size(1280.0, 820.0)
            .initialization_script(context_script(MAIN, None)),
    )
    .build()
}

/// Open (or focus) a dedicated native window for one document.
pub fn open_document_window<R: Runtime>(
    app: &AppHandle<R>,
    doc_id: u64,
    title: &str,
) -> tauri::Result<WebviewWindow<R>> {
    let label = doc_window_label(doc_id);
    if let Some(w) = app.get_webview_window(&label) {
        w.set_focus()?;
        return Ok(w);
    }
    styled(
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
            .title(title)
            .inner_size(1180.0, 780.0)
            .initialization_script(context_script(&label, Some(doc_id))),
    )
    .build()
}

pub fn focus_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(doc_window_label(12), "doc-12");
        assert_eq!(doc_id_from_label("doc-12"), Some(12));
        assert_eq!(doc_id_from_label("main"), None);
        assert!(context_script("doc-3", Some(3)).contains("docId: 3"));
        assert!(context_script("main", None).contains("docId: null"));
    }
}
