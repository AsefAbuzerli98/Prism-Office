//! Local-first auto-recovery (X-01).
//!
//! Every open, modified document is journaled to
//! `<app-data>/recovery/<key>/`: `meta.json` (title, path, kind), `base.json`
//! (a full snapshot of the editable content) and `journal.jsonl` — one line
//! per tick with a structural JSON diff against the previous state, appended
//! and fsynced every 3 seconds. Journals are compacted into a new base when
//! they grow, removed when the document is saved or deliberately closed, and
//! offered for restore on the next launch otherwise (crash, power loss, quit
//! without saving). A truncated last line (crash mid-write) is ignored.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use prism_openxml::DocumentKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INTERVAL_SECS: u64 = 3;
const COMPACT_ENTRIES: usize = 256;
const COMPACT_BYTES: u64 = 2 * 1024 * 1024;

// ---- structural JSON diff ------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Seg {
    Index(usize),
    Key(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Op {
    /// Replace the value at `path` (or add an object key).
    Set { path: Vec<Seg>, value: Value },
    /// Remove an object key.
    Remove { path: Vec<Seg> },
    /// Replace `delete` array items at `index` with `insert`.
    Splice {
        path: Vec<Seg>,
        index: usize,
        delete: usize,
        insert: Vec<Value>,
    },
}

/// Ops that turn `old` into `new`.
pub fn diff(old: &Value, new: &Value) -> Vec<Op> {
    let mut ops = Vec::new();
    diff_at(&mut Vec::new(), old, new, &mut ops);
    ops
}

fn diff_at(path: &mut Vec<Seg>, old: &Value, new: &Value, ops: &mut Vec<Op>) {
    if old == new {
        return;
    }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            for k in a.keys() {
                if !b.contains_key(k) {
                    let mut p = path.clone();
                    p.push(Seg::Key(k.clone()));
                    ops.push(Op::Remove { path: p });
                }
            }
            for (k, v) in b {
                path.push(Seg::Key(k.clone()));
                match a.get(k) {
                    Some(old_v) => diff_at(path, old_v, v, ops),
                    None => ops.push(Op::Set {
                        path: path.clone(),
                        value: v.clone(),
                    }),
                }
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
            let max_suffix = a.len().min(b.len()) - prefix;
            let suffix = a
                .iter()
                .rev()
                .zip(b.iter().rev())
                .take(max_suffix)
                .take_while(|(x, y)| x == y)
                .count();
            let (da, db) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
            if da.len() == db.len() && da.len() <= 4 {
                // same-size edit: recurse for fine-grained changes (typing in one paragraph)
                for (i, (x, y)) in da.iter().zip(db).enumerate() {
                    path.push(Seg::Index(prefix + i));
                    diff_at(path, x, y, ops);
                    path.pop();
                }
            } else {
                ops.push(Op::Splice {
                    path: path.clone(),
                    index: prefix,
                    delete: da.len(),
                    insert: db.to_vec(),
                });
            }
        }
        _ => ops.push(Op::Set {
            path: path.clone(),
            value: new.clone(),
        }),
    }
}

fn target<'a>(root: &'a mut Value, path: &[Seg]) -> Option<&'a mut Value> {
    let mut cur = root;
    for seg in path {
        cur = match (seg, cur) {
            (Seg::Key(k), Value::Object(m)) => m.get_mut(k)?,
            (Seg::Index(i), Value::Array(a)) => a.get_mut(*i)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Apply ops produced by [`diff`]. Returns false if an op did not fit.
pub fn apply(doc: &mut Value, ops: &[Op]) -> bool {
    for op in ops {
        let ok = match op {
            Op::Set { path, value } => match path.split_last() {
                None => {
                    *doc = value.clone();
                    true
                }
                Some((last, parent)) => match (last, target(doc, parent)) {
                    (Seg::Key(k), Some(Value::Object(m))) => {
                        m.insert(k.clone(), value.clone());
                        true
                    }
                    (Seg::Index(i), Some(Value::Array(a))) if *i < a.len() => {
                        a[*i] = value.clone();
                        true
                    }
                    _ => false,
                },
            },
            Op::Remove { path } => match path.split_last() {
                Some((Seg::Key(k), parent)) => match target(doc, parent) {
                    Some(Value::Object(m)) => m.remove(k).is_some(),
                    _ => false,
                },
                _ => false,
            },
            Op::Splice {
                path,
                index,
                delete,
                insert,
            } => match target(doc, path) {
                Some(Value::Array(a)) if index + delete <= a.len() => {
                    a.splice(*index..index + delete, insert.iter().cloned());
                    true
                }
                _ => false,
            },
        };
        if !ok {
            return false;
        }
    }
    true
}

// ---- journal store ------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryMeta {
    pub title: String,
    pub path: Option<String>,
    pub kind: DocumentKind,
    /// Unix ms of the last journaled change.
    #[serde(default)]
    pub updated: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableDoc {
    pub key: String,
    #[serde(flatten)]
    pub meta: RecoveryMeta,
    /// Journal entries since the last base snapshot.
    pub entries: usize,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    seq: u64,
    t: u64,
    ops: Vec<Op>,
}

struct Live {
    last: Value,
    seq: u64,
    entries: usize,
}

pub struct RecoveryStore {
    root: PathBuf,
    live: Mutex<HashMap<String, Live>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(tmp, path)
}

impl RecoveryStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            live: Mutex::new(HashMap::new()),
        }
    }

    /// A key unique to this process and document.
    pub fn key_for(doc_id: u64) -> String {
        format!("{}-{}-{}", std::process::id(), doc_id, now_ms())
    }

    fn dir(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }

    /// Journal the current content of a document (no-op if unchanged).
    pub fn record(&self, key: &str, meta: &RecoveryMeta, content: &Value) -> std::io::Result<bool> {
        let dir = self.dir(key);
        let mut live = self.live.lock().unwrap();
        let meta = RecoveryMeta {
            updated: now_ms(),
            ..meta.clone()
        };
        let first = !live.contains_key(key);
        if first {
            fs::create_dir_all(&dir)?;
            write_atomic(&dir.join("base.json"), &serde_json::to_vec(content)?)?;
            File::create(dir.join("journal.jsonl"))?.sync_all()?;
            write_atomic(&dir.join("meta.json"), &serde_json::to_vec(&meta)?)?;
            live.insert(
                key.to_string(),
                Live {
                    last: content.clone(),
                    seq: 0,
                    entries: 0,
                },
            );
            return Ok(true);
        }
        let l = live.get_mut(key).unwrap();
        let ops = diff(&l.last, content);
        if ops.is_empty() {
            return Ok(false);
        }
        l.seq += 1;
        let line = serde_json::to_string(&Entry {
            seq: l.seq,
            t: meta.updated,
            ops,
        })?;
        let journal = dir.join("journal.jsonl");
        {
            let mut f = OpenOptions::new()
                .append(true)
                .create(true)
                .open(&journal)?;
            writeln!(f, "{line}")?;
            f.sync_data()?;
        }
        l.entries += 1;
        l.last = content.clone();
        write_atomic(&dir.join("meta.json"), &serde_json::to_vec(&meta)?)?;
        if l.entries >= COMPACT_ENTRIES
            || fs::metadata(&journal).map(|m| m.len()).unwrap_or(0) > COMPACT_BYTES
        {
            write_atomic(&dir.join("base.json"), &serde_json::to_vec(content)?)?;
            File::create(&journal)?.sync_all()?;
            l.entries = 0;
        }
        Ok(true)
    }

    /// Forget a document's journal (saved, or closed deliberately).
    pub fn discard(&self, key: &str) {
        self.live.lock().unwrap().remove(key);
        let _ = fs::remove_dir_all(self.dir(key));
    }

    /// Journals left by earlier sessions (not owned by this process).
    pub fn list(&self) -> Vec<RecoverableDoc> {
        let live = self.live.lock().unwrap();
        let Ok(rd) = fs::read_dir(&self.root) else {
            return vec![];
        };
        let mut out: Vec<RecoverableDoc> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let key = e.file_name().to_string_lossy().into_owned();
                if live.contains_key(&key) {
                    return None;
                }
                let meta: RecoveryMeta =
                    serde_json::from_slice(&fs::read(e.path().join("meta.json")).ok()?).ok()?;
                let entries = fs::read_to_string(e.path().join("journal.jsonl"))
                    .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
                    .unwrap_or(0);
                Some(RecoverableDoc { key, meta, entries })
            })
            .collect();
        out.sort_by_key(|d| std::cmp::Reverse(d.meta.updated));
        out
    }

    /// Rebuild the latest content: base snapshot + every intact journal entry.
    pub fn restore(&self, key: &str) -> std::io::Result<(RecoveryMeta, Value)> {
        let dir = self.dir(key);
        let meta: RecoveryMeta = serde_json::from_slice(&fs::read(dir.join("meta.json"))?)?;
        let mut content: Value = serde_json::from_slice(&fs::read(dir.join("base.json"))?)?;
        if let Ok(f) = File::open(dir.join("journal.jsonl")) {
            for line in BufReader::new(f).lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                // a torn final write after a crash: stop at the last intact entry
                let Ok(entry) = serde_json::from_str::<Entry>(&line) else {
                    break;
                };
                if !apply(&mut content, &entry.ops) {
                    break;
                }
            }
        }
        Ok((meta, content))
    }
}

/// Registry entry for the journaling daemon.
pub struct RecoveryService;

impl crate::services::Service for RecoveryService {
    fn id(&self) -> &'static str {
        "recovery"
    }
    fn name(&self) -> &'static str {
        "Auto-recovery journal"
    }
    fn start(&self, _bus: &crate::events::EventBus) -> Result<Option<String>, String> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roundtrip(a: Value, b: Value) -> Vec<Op> {
        let ops = diff(&a, &b);
        let mut x = a.clone();
        assert!(apply(&mut x, &ops), "apply failed for {ops:?}");
        assert_eq!(x, b);
        // ops survive serialisation
        let back: Vec<Op> = serde_json::from_str(&serde_json::to_string(&ops).unwrap()).unwrap();
        assert_eq!(back, ops);
        ops
    }

    #[test]
    fn diffs_are_small_and_exact() {
        let doc = |blocks: Vec<&str>| json!({ "blocks": blocks.iter().map(|t| json!({"type": "paragraph", "inlines": [{"type": "text", "text": t}]})).collect::<Vec<_>>(), "section": {"pageWidth": 11906}, "notes": [] });
        let a = doc(vec!["one", "two", "three", "four", "five"]);
        // typing in one paragraph -> a single leaf Set
        let ops = roundtrip(a.clone(), doc(vec!["one", "twoo", "three", "four", "five"]));
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::Set { path, .. } if path.len() == 5));
        // inserting a paragraph at the top -> one splice, not a rewrite of every block
        let ops = roundtrip(
            a.clone(),
            doc(vec!["zero", "one", "two", "three", "four", "five"]),
        );
        assert_eq!(
            ops,
            vec![Op::Splice {
                path: vec![Seg::Key("blocks".into())],
                index: 0,
                delete: 0,
                insert: vec![
                    json!({"type": "paragraph", "inlines": [{"type": "text", "text": "zero"}]})
                ]
            }]
        );
        roundtrip(a.clone(), doc(vec!["one", "five"]));
        roundtrip(
            a.clone(),
            json!({"blocks": [], "section": {"pageWidth": 16838, "landscape": true}}),
        );
        roundtrip(json!([1, 2, 3]), json!("replaced"));
        roundtrip(
            json!({"a": [1, {"b": [1, 2]}]}),
            json!({"a": [1, {"b": [2, 1, 3]}], "c": null}),
        );
        assert!(diff(&a, &a).is_empty());
        assert!(!apply(
            &mut json!({"x": 1}),
            &[Op::Splice {
                path: vec![Seg::Key("x".into())],
                index: 0,
                delete: 0,
                insert: vec![]
            }]
        ));
    }

    fn meta() -> RecoveryMeta {
        RecoveryMeta {
            title: "Report.docx".into(),
            path: Some("/tmp/Report.docx".into()),
            kind: DocumentKind::Document,
            updated: 0,
        }
    }

    #[test]
    fn journals_restore_and_discard() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(dir.path().to_path_buf());
        let key = "1-7-100";
        let mut content = json!({"blocks": ["a"]});
        assert!(store.record(key, &meta(), &content).unwrap());
        assert!(
            !store.record(key, &meta(), &content).unwrap(),
            "unchanged content writes nothing"
        );
        for i in 0..10 {
            content["blocks"]
                .as_array_mut()
                .unwrap()
                .push(json!(format!("b{i}")));
            store.record(key, &meta(), &content).unwrap();
        }
        // a new process sees the journal; the owning one doesn't list it
        assert!(store.list().is_empty());
        let other = RecoveryStore::new(dir.path().to_path_buf());
        let listed = other.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (
                listed[0].key.as_str(),
                listed[0].entries,
                listed[0].meta.title.as_str()
            ),
            (key, 10, "Report.docx")
        );
        let (m, restored) = other.restore(key).unwrap();
        assert_eq!(m.path.as_deref(), Some("/tmp/Report.docx"));
        assert_eq!(restored, content);
        other.discard(key);
        assert!(other.list().is_empty());
    }

    #[test]
    fn survives_a_torn_final_write() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(dir.path().to_path_buf());
        let key = "1-1-1";
        store.record(key, &meta(), &json!({"t": "x"})).unwrap();
        store.record(key, &meta(), &json!({"t": "xy"})).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(dir.path().join(key).join("journal.jsonl"))
            .unwrap();
        write!(f, "{{\"seq\":3,\"t\":1,\"ops\":[{{\"op\":\"set\",\"pa").unwrap();
        let (_, restored) = RecoveryStore::new(dir.path().to_path_buf())
            .restore(key)
            .unwrap();
        assert_eq!(restored, json!({"t": "xy"}));
    }

    #[test]
    fn compacts_long_journals() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(dir.path().to_path_buf());
        let key = "1-2-3";
        for i in 0..(COMPACT_ENTRIES + 5) {
            store.record(key, &meta(), &json!({ "n": i })).unwrap();
        }
        let lines = fs::read_to_string(dir.path().join(key).join("journal.jsonl"))
            .unwrap()
            .lines()
            .count();
        assert!(lines < 10, "journal compacted ({lines} lines)");
        let (_, v) = RecoveryStore::new(dir.path().to_path_buf())
            .restore(key)
            .unwrap();
        assert_eq!(v, json!({ "n": COMPACT_ENTRIES + 4 }));
    }
}
