//! `=PY` runtime: runs cell code in a separate `python3 -I` process with
//! resource limits, a wall-clock timeout, no network sockets and a scratch
//! working directory (best-effort sandbox, see `python/runner.py`).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const RUNNER: &str = include_str!("python/runner.py");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PyResult {
    pub value: serde_json::Value,
    pub stdout: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PyStatus {
    pub available: bool,
    pub interpreter: Option<String>,
    pub version: Option<String>,
    pub pandas: bool,
    pub matplotlib: bool,
    pub isolation: Isolation,
}

/// OS-level confinement used for Python cells, strongest available first.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Isolation {
    /// Linux bubblewrap: no network, read-only filesystem, private /tmp and $HOME.
    Bubblewrap,
    /// Linux user + network namespaces (`unshare -rn`): no network.
    Namespaces,
    /// macOS Seatbelt profile: no network, writes only to the scratch folder.
    SandboxExec,
    /// Windows Job Object: memory cap, no child processes, killed with the job.
    JobObject,
    /// Only the in-process guards of the runner.
    None,
}

fn works(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Detected once; `PRISM_PY_SANDBOX=off` disables OS confinement.
pub fn isolation() -> Isolation {
    static ISO: std::sync::OnceLock<Isolation> = std::sync::OnceLock::new();
    *ISO.get_or_init(|| {
        if std::env::var("PRISM_PY_SANDBOX").is_ok_and(|v| v == "off") {
            return Isolation::None;
        }
        if cfg!(target_os = "linux") {
            if works(
                "bwrap",
                &[
                    "--ro-bind",
                    "/",
                    "/",
                    "--dev",
                    "/dev",
                    "--unshare-all",
                    "--die-with-parent",
                    "true",
                ],
            ) {
                return Isolation::Bubblewrap;
            }
            if works("unshare", &["-rn", "true"]) {
                return Isolation::Namespaces;
            }
            return Isolation::None;
        }
        if cfg!(target_os = "macos") {
            return if std::path::Path::new("/usr/bin/sandbox-exec").exists() {
                Isolation::SandboxExec
            } else {
                Isolation::None
            };
        }
        if cfg!(windows) {
            return Isolation::JobObject;
        }
        Isolation::None
    })
}

/// The command that runs `python -I script` under the chosen confinement.
fn confined(py: &str, script: &std::path::Path, dir: &std::path::Path, iso: Isolation) -> Command {
    match iso {
        Isolation::Bubblewrap => {
            let mut c = Command::new("bwrap");
            c.args([
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
            ]);
            if let Some(home) =
                std::env::var_os("HOME").filter(|h| std::path::Path::new(h).is_dir())
            {
                c.arg("--tmpfs").arg(home);
            }
            c.arg("--bind").arg(dir).arg(dir).arg("--chdir").arg(dir);
            c.args(["--unshare-all", "--die-with-parent", "--new-session", "--"]);
            c.arg(py).arg("-I").arg(script);
            c
        }
        Isolation::Namespaces => {
            let mut c = Command::new("unshare");
            c.arg("-rn").arg(py).arg("-I").arg(script);
            c
        }
        Isolation::SandboxExec => {
            let d = dir.to_string_lossy().replace('"', "");
            let profile = format!(
                r#"(version 1)(allow default)(deny network*)(allow network* (local unix))(deny file-write*)(allow file-write* (subpath "{d}") (subpath "/private{d}") (literal "/dev/null") (subpath "/dev/fd"))"#
            );
            let mut c = Command::new("/usr/bin/sandbox-exec");
            c.arg("-p").arg(profile).arg(py).arg("-I").arg(script);
            c
        }
        _ => {
            let mut c = Command::new(py);
            c.arg("-I").arg(script);
            c
        }
    }
}

/// Windows: put the process in a Job Object (1 GB memory cap, no child
/// processes, killed when the handle closes).
#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(windows)]
fn confine_job(child: &std::process::Child) -> Option<Job> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::*;
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        info.BasicLimitInformation.ActiveProcessLimit = 1;
        info.ProcessMemoryLimit = 1024 * 1024 * 1024;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0 {
            windows_sys::Win32::Foundation::CloseHandle(job);
            return None;
        }
        Some(Job(job))
    }
}

/// Interpreter to use: `PRISM_PYTHON`, else `python3`, else `python`.
pub fn interpreter() -> Option<String> {
    let mut candidates = vec![];
    if let Ok(p) = std::env::var("PRISM_PYTHON") {
        candidates.push(p);
    }
    candidates.extend(["python3".to_string(), "python".to_string()]);
    candidates.into_iter().find(|c| {
        Command::new(c)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

pub fn status() -> PyStatus {
    let Some(py) = interpreter() else {
        return PyStatus {
            available: false,
            interpreter: None,
            version: None,
            pandas: false,
            matplotlib: false,
            isolation: isolation(),
        };
    };
    let out = Command::new(&py).args(["-I", "-c", "import sys,importlib.util as u;print(sys.version.split()[0]);print(bool(u.find_spec('pandas')));print(bool(u.find_spec('matplotlib')))"]).output().ok();
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut lines = text.lines();
    PyStatus {
        available: true,
        version: lines.next().map(String::from),
        pandas: lines.next() == Some("True"),
        matplotlib: lines.next() == Some("True"),
        isolation: isolation(),
        interpreter: Some(py),
    }
}

pub fn run(
    code: &str,
    inputs: &serde_json::Value,
    return_type: i32,
    timeout: Duration,
) -> Result<PyResult, String> {
    let py = interpreter().ok_or("Python 3 was not found (install it or set PRISM_PYTHON)")?;
    let dir = std::env::temp_dir().join(format!(
        "prism-py-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let script = dir.join("runner.py");
    std::fs::write(&script, RUNNER).map_err(|e| e.to_string())?;
    let iso = isolation();
    let mut cmd = confined(&py, &script, &dir, iso);
    cmd.current_dir(&dir)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // keep what Python needs to start, nothing else (no credentials in the environment)
    for k in [
        "PATH",
        "SYSTEMROOT",
        "LANG",
        "LC_ALL",
        "HOME",
        "USERPROFILE",
        "TMPDIR",
        "TEMP",
        "TMP",
    ] {
        if let Ok(v) = std::env::var(k) {
            cmd.env(k, v);
        }
    }
    cmd.env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONIOENCODING", "utf-8");
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("could not start Python: {e}"))?;
    #[cfg(windows)]
    let _job = confine_job(&child);
    let req = serde_json::json!({ "code": code, "inputs": inputs, "returnType": return_type, "memoryMb": 1024, "cpuSeconds": timeout.as_secs().max(1) });
    child
        .stdin
        .take()
        .unwrap()
        .write_all(req.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&dir);
            return Ok(PyResult {
                value: serde_json::Value::Null,
                stdout: String::new(),
                error: Some(format!(
                    "TimeoutError: cell ran longer than {}s",
                    timeout.as_secs()
                )),
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let out = reader.join().unwrap_or_default();
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let _ = std::fs::remove_dir_all(&dir);
    match serde_json::from_str::<PyResult>(&out) {
        Ok(r) => Ok(r),
        Err(_) => Ok(PyResult {
            value: serde_json::Value::Null,
            stdout: String::new(),
            error: Some(if status.success() {
                "the Python runner returned no result".into()
            } else {
                format!(
                    "Python exited with {status}: {}",
                    err.lines().last().unwrap_or("")
                )
            }),
        }),
    }
}

const DB_QUERY: &str = include_str!("python/dbquery.py");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<serde_json::Value>>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub error: Option<String>,
}

/// Read-only SQLite / ODBC query through the Python standard library (sqlite3)
/// or pyodbc, so no native database driver is linked into the app.
pub fn query_database(
    kind: &str,
    source: &str,
    query: &str,
    limit: usize,
) -> Result<QueryResult, String> {
    let py = interpreter().ok_or("Python 3 was not found (install it or set PRISM_PYTHON)")?;
    let mut child = Command::new(py)
        .args(["-I", "-c", DB_QUERY])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start Python: {e}"))?;
    let req = serde_json::json!({ "kind": kind, "source": source, "query": query, "limit": limit });
    child
        .stdin
        .take()
        .unwrap()
        .write_all(req.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    serde_json::from_slice(&out.stdout).map_err(|_| {
        format!(
            "query failed: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("no output")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have_python() -> bool {
        interpreter().is_some()
    }

    #[test]
    fn queries_sqlite_read_only() {
        if !have_python() {
            return;
        }
        let db = std::env::temp_dir().join(format!("prism-q-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&db);
        let setup = format!("import sqlite3;c=sqlite3.connect(r'{}');c.execute('create table t(a,b)');c.executemany('insert into t values(?,?)',[(1,'x'),(2,'y'),(3,None)]);c.commit()", db.display());
        assert!(Command::new(interpreter().unwrap())
            .args(["-c", &setup])
            .status()
            .unwrap()
            .success());
        let r = query_database(
            "sqlite",
            &db.to_string_lossy(),
            "select a, b from t order by a",
            2,
        )
        .unwrap();
        assert_eq!(r.columns, ["a", "b"]);
        assert_eq!(
            r.rows,
            vec![
                vec![serde_json::json!(1), serde_json::json!("x")],
                vec![serde_json::json!(2), serde_json::json!("y")]
            ]
        );
        assert!(r.truncated);
        let r = query_database("sqlite", &db.to_string_lossy(), "delete from t", 10).unwrap();
        assert!(r.error.unwrap().contains("readonly"));
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn evaluates_last_expression_with_inputs() {
        if !have_python() {
            return;
        }
        let inputs = serde_json::json!([["A1:A3", [[1.0], [2.0], [3.0]]], ["B1", [[10.0]]]]);
        let r = run(
            "x = xl(\"B1\")\nprint('hi')\nx * 2",
            &inputs,
            0,
            Duration::from_secs(20),
        )
        .unwrap();
        assert_eq!(r.error, None);
        assert_eq!(r.value, serde_json::json!(20.0));
        assert_eq!(r.stdout, "hi\n");
        let r = run("[[1, 'a'], [2, 'b']]", &inputs, 1, Duration::from_secs(20)).unwrap();
        assert_eq!(r.value, serde_json::json!([[1, "a"], [2, "b"]]));
    }

    #[test]
    fn sandbox_blocks_network_processes_and_outside_writes() {
        if !have_python() {
            return;
        }
        let none = serde_json::json!([]);
        let r = run(
            "import socket\nsocket.create_connection(('example.com', 80))",
            &none,
            0,
            Duration::from_secs(20),
        )
        .unwrap();
        assert!(r.error.unwrap().contains("network access is disabled"));
        let r = run(
            "import os\nos.system('echo hi')",
            &none,
            0,
            Duration::from_secs(20),
        )
        .unwrap();
        assert!(r.error.unwrap().contains("PermissionError"));
        let r = run(
            "open('/tmp/prism-escape.txt','w').write('x')",
            &none,
            0,
            Duration::from_secs(20),
        )
        .unwrap();
        assert!(r.error.unwrap().contains("scratch folder"));
        let r = run(
            "open('ok.txt','w').write('x')",
            &none,
            0,
            Duration::from_secs(20),
        )
        .unwrap();
        assert_eq!(r.error, None);
        let r = run("while True: pass", &none, 0, Duration::from_secs(2)).unwrap();
        // killed by the CPU limit or the wall-clock timeout
        assert!(r.error.is_some());
        println!("isolation: {:?}", isolation());
        // with OS isolation even a direct syscall-level connect is impossible
        if matches!(isolation(), Isolation::Bubblewrap | Isolation::Namespaces) {
            let code =
                "import _socket\ns = _socket.socket()\ns.settimeout(2)\ns.connect(('1.1.1.1', 80))";
            let r = run(code, &none, 0, Duration::from_secs(20)).unwrap();
            assert!(
                r.error.unwrap().contains("unreachable"),
                "network must be unreachable"
            );
        }
        if isolation() == Isolation::Bubblewrap {
            let r = run(
                "import _io\n_io.open('/etc/prism-test', 'w')",
                &none,
                0,
                Duration::from_secs(20),
            )
            .unwrap();
            assert!(r.error.unwrap().contains("Read-only"));
            let home = std::env::var("HOME").unwrap_or_default();
            let r = run(
                &format!("import os\nlen(os.listdir({home:?}))"),
                &none,
                0,
                Duration::from_secs(20),
            )
            .unwrap();
            assert_eq!(r.value, serde_json::json!(0), "home is hidden");
        }
        let r = run("1/0", &none, 0, Duration::from_secs(20)).unwrap();
        assert_eq!(
            r.error.as_deref(),
            Some("ZeroDivisionError: division by zero")
        );
        if status().matplotlib {
            let r = run(
                "import matplotlib.pyplot as plt\nplt.plot([1, 3, 2])",
                &none,
                0,
                Duration::from_secs(60),
            )
            .unwrap();
            assert_eq!(r.error, None);
            let png = r.value["image"].as_str().unwrap();
            assert!(png.starts_with("iVBORw0KGgo"), "PNG data");
            assert!(r.value["width"].as_u64().unwrap() > 100);
        }
        let r = run("1/0", &none, 0, Duration::from_secs(20)).unwrap();
        assert_eq!(
            r.error.as_deref(),
            Some("ZeroDivisionError: division by zero")
        );
    }
}
