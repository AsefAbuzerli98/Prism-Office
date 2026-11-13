//! User preferences persisted to `settings.json` in the app config dir.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::store;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Windowing {
    /// One master window hosting multi-modal document tabs.
    #[default]
    Tabbed,
    /// One native window per document.
    MultiInstance,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub windowing: Windowing,
    pub appearance: Appearance,
    /// BCP-47 tag, or `None` to follow the OS.
    pub locale: Option<String>,
    /// Opt-in: keep local crash reports (never sent anywhere).
    pub crash_reports: bool,
}

pub struct SettingsStore {
    path: Option<PathBuf>,
    inner: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        let inner = store::load(path.as_deref());
        Self {
            path,
            inner: Mutex::new(inner),
        }
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().unwrap().clone()
    }

    pub fn set(&self, settings: Settings) -> std::io::Result<Settings> {
        let mut inner = self.inner.lock().unwrap();
        *inner = settings;
        store::save(self.path.as_ref(), &*inner)?;
        Ok(inner.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_partial_json_and_persist() {
        let s: Settings = serde_json::from_str(r#"{"windowing":"multiInstance"}"#).unwrap();
        assert_eq!(s.windowing, Windowing::MultiInstance);
        assert_eq!(s.appearance, Appearance::System);

        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cfg/settings.json");
        let store = SettingsStore::load(Some(p.clone()));
        assert_eq!(store.get(), Settings::default());
        store
            .set(Settings {
                locale: Some("az".into()),
                ..s
            })
            .unwrap();
        let again = SettingsStore::load(Some(p));
        assert_eq!(again.get().locale.as_deref(), Some("az"));
        assert_eq!(again.get().windowing, Windowing::MultiInstance);
    }
}
