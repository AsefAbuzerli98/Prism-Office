//! Telemetry-free diagnostics (X-03): structured JSON-lines logs in the app
//! log directory (rotated, never containing document content) and opt-in
//! crash reports written locally by a panic hook. Nothing is sent anywhere;
//! the user can open or attach a report themselves.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

const MAX_LOG: u64 = 5 * 1024 * 1024;

struct Sink {
    dir: PathBuf,
    lock: Mutex<()>,
}

static SINK: OnceLock<Sink> = OnceLock::new();
static CRASH_REPORTS: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Set up logging into `dir` and install the panic hook.
pub fn init(dir: PathBuf, crash_reports: bool) {
    let _ = fs::create_dir_all(dir.join("crashes"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    CRASH_REPORTS.store(crash_reports, Ordering::Relaxed);
    let _ = SINK.set(Sink {
        dir,
        lock: Mutex::new(()),
    });
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        log(
            "error",
            "panic",
            &msg,
            serde_json::json!({ "location": loc }),
        );
        if CRASH_REPORTS.load(Ordering::Relaxed) {
            write_crash(&msg, &loc);
        }
        prev(info);
    }));
    log(
        "info",
        "app",
        "started",
        serde_json::json!({ "version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "arch": std::env::consts::ARCH }),
    );
}

pub fn set_crash_reports(on: bool) {
    CRASH_REPORTS.store(on, Ordering::Relaxed);
}

/// Append one structured record.
pub fn log(level: &str, target: &str, message: &str, fields: serde_json::Value) {
    let Some(s) = SINK.get() else { return };
    let _g = s.lock.lock().unwrap_or_else(|e| e.into_inner());
    let path = s.dir.join("prism.log");
    if fs::metadata(&path)
        .map(|m| m.len() > MAX_LOG)
        .unwrap_or(false)
    {
        let _ = fs::rename(&path, s.dir.join("prism.log.1"));
    }
    let rec = serde_json::json!({ "ts": now_ms() as u64, "level": level, "target": target, "msg": message, "fields": fields });
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{rec}");
    }
}

fn write_crash(msg: &str, loc: &str) {
    let Some(s) = SINK.get() else { return };
    let bt = std::backtrace::Backtrace::force_capture().to_string();
    let report = serde_json::json!({
        "ts": now_ms() as u64,
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "message": msg,
        "location": loc,
        "backtrace": bt,
    });
    let _ = fs::write(
        s.dir
            .join("crashes")
            .join(format!("crash-{}.json", now_ms())),
        report.to_string(),
    );
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub file: String,
    pub ts: u64,
    pub message: String,
    pub location: String,
}

pub fn crashes(dir: &Path) -> Vec<CrashReport> {
    let mut out: Vec<CrashReport> = fs::read_dir(dir.join("crashes"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let v: serde_json::Value = serde_json::from_slice(&fs::read(&p).ok()?).ok()?;
            Some(CrashReport {
                file: p.to_string_lossy().into_owned(),
                ts: v["ts"].as_u64().unwrap_or(0),
                message: v["message"].as_str().unwrap_or_default().to_string(),
                location: v["location"].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.ts));
    out
}

pub fn clear_crashes(dir: &Path) {
    for e in fs::read_dir(dir.join("crashes"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let _ = fs::remove_file(e.path());
    }
}

pub fn dir() -> Option<PathBuf> {
    SINK.get().map(|s| s.dir.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_and_crash_reports() {
        let d = std::env::temp_dir().join(format!("prism-diag-{}", std::process::id()));
        init(d.clone(), true);
        log("warn", "test", "hello", serde_json::json!({ "n": 1 }));
        let r = std::panic::catch_unwind(|| panic!("boom"));
        assert!(r.is_err());
        let text = fs::read_to_string(d.join("prism.log")).unwrap();
        assert!(text.lines().any(|l| l.contains("\"msg\":\"hello\"")));
        assert!(text.lines().any(|l| l.contains("boom")));
        let c = crashes(&d);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].message, "boom");
        clear_crashes(&d);
        assert!(crashes(&d).is_empty());
        let _ = fs::remove_dir_all(&d);
    }
}
