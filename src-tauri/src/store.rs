//! Small JSON-file persistence helper (atomic write).

use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

pub fn load<T: DeserializeOwned + Default>(path: Option<&Path>) -> T {
    path.and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save<T: Serialize>(path: Option<&PathBuf>, value: &T) -> std::io::Result<()> {
    let Some(path) = path else { return Ok(()) };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(tmp, path)
}
