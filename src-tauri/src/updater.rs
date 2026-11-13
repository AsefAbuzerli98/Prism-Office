//! Auto-update (P5-04): the Tauri updater checks the release endpoint for a
//! `latest.json` manifest and only installs packages whose minisign
//! signature matches the public key in `tauri.conf.json`. Without a
//! configured key the updater reports "not configured" and does nothing.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub configured: bool,
    pub current: String,
    pub available: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// A release public key is configured (signature checks are possible).
pub fn configured(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

pub async fn check(app: &AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    if !configured(app) {
        return Ok(UpdateInfo {
            configured: false,
            current,
            available: None,
            notes: None,
            date: None,
        });
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(UpdateInfo {
        configured: true,
        current,
        available: update.as_ref().map(|u| u.version.clone()),
        notes: update.as_ref().and_then(|u| u.body.clone()),
        date: update.as_ref().and_then(|u| u.date.map(|d| d.to_string())),
    })
}

/// Download, verify (minisign) and install the update, then restart.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if !configured(app) {
        return Err("updates are not configured for this build".into());
    }
    let Some(update) = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Err("no update available".into());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}
