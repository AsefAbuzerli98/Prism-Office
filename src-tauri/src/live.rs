//! Live Presenting (P4-14): a small HTTP server on the local network that
//! lets the audience follow the slideshow on their phones (QR link) with
//! live subtitles in a language each viewer chooses.
//!
//! Routes (all need `?t=<token>`): `/` viewer page, `/slide.png` current
//! slide image, `/events?lang=xx` server-sent events with slide changes and
//! subtitles. The app pushes slide images and (translated) subtitles.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;

#[derive(Default)]
struct Shared {
    version: u64,
    slide: u32,
    total: u32,
    png: Vec<u8>,
    /// Latest subtitle line: original text and translations by language.
    subtitle: String,
    translations: HashMap<String, String>,
    final_line: bool,
    /// Languages requested by connected viewers (count per language).
    langs: HashMap<String, usize>,
}

struct Inner {
    token: String,
    state: Mutex<Shared>,
    changed: Condvar,
    stop: AtomicBool,
    viewers: AtomicUsize,
}

pub struct LiveServer {
    inner: Arc<Inner>,
    pub url: String,
    pub port: u16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveInfo {
    pub url: String,
    pub port: u16,
    pub token: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveStatus {
    pub running: bool,
    pub url: Option<String>,
    pub viewers: usize,
    /// Languages viewers asked for (translate subtitles into these).
    pub languages: Vec<String>,
}

/// A random session token (process-random SipHash keys + time + counter).
fn token() -> String {
    static N: AtomicUsize = AtomicUsize::new(0);
    let mut out = String::new();
    for _ in 0..2 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        h.write_usize(N.fetch_add(1, Ordering::Relaxed));
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// The address other devices on the LAN reach this computer at.
fn lan_ip() -> String {
    // connecting a UDP socket sends nothing; it only picks the outgoing interface
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").map(|_| s))
        .and_then(|s| s.local_addr())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

impl LiveServer {
    pub fn start() -> std::io::Result<LiveServer> {
        let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        let port = listener.local_addr()?.port();
        let inner = Arc::new(Inner {
            token: token(),
            state: Mutex::new(Shared::default()),
            changed: Condvar::new(),
            stop: AtomicBool::new(false),
            viewers: AtomicUsize::new(0),
        });
        let url = format!("http://{}:{port}/?t={}", lan_ip(), inner.token);
        let worker = inner.clone();
        listener.set_nonblocking(true)?;
        std::thread::spawn(move || loop {
            if worker.stop.load(Ordering::Relaxed) {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let w = worker.clone();
                    std::thread::spawn(move || {
                        let _ = stream.set_nonblocking(false);
                        let _ = handle(stream, &w);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        });
        Ok(LiveServer { inner, url, port })
    }

    pub fn info(&self) -> LiveInfo {
        LiveInfo {
            url: self.url.clone(),
            port: self.port,
            token: self.inner.token.clone(),
        }
    }

    pub fn set_slide(&self, slide: u32, total: u32, png: Vec<u8>) {
        let mut s = self.inner.state.lock().unwrap();
        s.slide = slide;
        s.total = total;
        s.png = png;
        s.version += 1;
        self.inner.changed.notify_all();
    }

    pub fn set_subtitle(
        &self,
        text: String,
        translations: HashMap<String, String>,
        final_line: bool,
    ) {
        let mut s = self.inner.state.lock().unwrap();
        s.subtitle = text;
        s.translations = translations;
        s.final_line = final_line;
        s.version += 1;
        self.inner.changed.notify_all();
    }

    pub fn status(&self) -> LiveStatus {
        let s = self.inner.state.lock().unwrap();
        let mut languages: Vec<String> = s
            .langs
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(l, _)| l.clone())
            .collect();
        languages.sort();
        LiveStatus {
            running: true,
            url: Some(self.url.clone()),
            viewers: self.inner.viewers.load(Ordering::Relaxed),
            languages,
        }
    }

    pub fn stop(&self) {
        self.inner.stop.store(true, Ordering::Relaxed);
        self.inner.changed.notify_all();
    }
}

impl Drop for LiveServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn query<'a>(q: &'a str, key: &str) -> Option<&'a str> {
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == key).then_some(v)
    })
}

fn respond(s: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    s.write_all(body)
}

fn handle(mut stream: TcpStream, inner: &Inner) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    // drain headers
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, q) = target.split_once('?').unwrap_or((target, ""));
    if method != "GET" {
        return respond(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"GET only",
        );
    }
    if query(q, "t") != Some(inner.token.as_str()) {
        return respond(
            &mut stream,
            "403 Forbidden",
            "text/plain",
            b"This link has expired or is invalid.",
        );
    }
    match path {
        "/" => respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            VIEWER.as_bytes(),
        ),
        "/slide.png" => {
            let png = inner.state.lock().unwrap().png.clone();
            if png.is_empty() {
                respond(&mut stream, "404 Not Found", "text/plain", b"no slide yet")
            } else {
                respond(&mut stream, "200 OK", "image/png", &png)
            }
        }
        "/events" => {
            let lang = query(q, "lang")
                .filter(|l| {
                    l.len() <= 12 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                })
                .unwrap_or("")
                .to_string();
            serve_events(stream, inner, lang)
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn json_escape(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn serve_events(mut stream: TcpStream, inner: &Inner, lang: String) -> std::io::Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n"
    )?;
    inner.viewers.fetch_add(1, Ordering::Relaxed);
    if !lang.is_empty() {
        *inner
            .state
            .lock()
            .unwrap()
            .langs
            .entry(lang.clone())
            .or_default() += 1;
    }
    let result = (|| -> std::io::Result<()> {
        let mut seen = u64::MAX;
        loop {
            if inner.stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let msg = {
                let guard = inner.state.lock().unwrap();
                let (s, _) = inner
                    .changed
                    .wait_timeout_while(guard, Duration::from_secs(15), |s| {
                        s.version == seen && !inner.stop.load(Ordering::Relaxed)
                    })
                    .unwrap();
                if s.version == seen {
                    None
                } else {
                    seen = s.version;
                    let text = s.translations.get(&lang).unwrap_or(&s.subtitle);
                    Some(format!(
                        "{{\"version\":{},\"slide\":{},\"total\":{},\"subtitle\":{},\"final\":{}}}",
                        s.version,
                        s.slide,
                        s.total,
                        json_escape(text),
                        s.final_line
                    ))
                }
            };
            match msg {
                Some(m) => write!(stream, "data: {m}\n\n")?,
                None => write!(stream, ": keep-alive\n\n")?,
            }
            stream.flush()?;
        }
    })();
    inner.viewers.fetch_sub(1, Ordering::Relaxed);
    if !lang.is_empty() {
        if let Some(n) = inner.state.lock().unwrap().langs.get_mut(&lang) {
            *n = n.saturating_sub(1);
        }
    }
    result
}

const VIEWER: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Prism Office — Live</title>
<style>
html,body{margin:0;height:100%;background:#0c0a09;color:#fafaf9;font:16px system-ui,sans-serif}
main{display:flex;flex-direction:column;height:100%}
header{display:flex;gap:8px;align-items:center;padding:8px 12px;background:#1c1917}
header strong{flex:1}
select{font:inherit;padding:4px;border-radius:6px}
#stage{flex:1;display:flex;align-items:center;justify-content:center;min-height:0;padding:8px}
#slide{max-width:100%;max-height:100%;border-radius:6px;box-shadow:0 6px 24px rgba(0,0,0,.5);background:#fff}
#sub{min-height:3.2em;padding:12px 16px;font-size:20px;line-height:1.4;text-align:center;background:#1c1917}
#sub.interim{color:#a8a29e}
#count{color:#a8a29e;font-size:14px}
</style></head>
<body><main>
<header><strong>Prism Office · Live</strong><span id="count"></span>
<select id="lang" aria-label="Subtitle language">
<option value="">Original</option><option value="en">English</option><option value="az">Azərbaycanca</option><option value="tr">Türkçe</option><option value="ru">Русский</option><option value="de">Deutsch</option><option value="fr">Français</option><option value="es">Español</option><option value="ar">العربية</option><option value="zh">中文</option><option value="ja">日本語</option>
</select></header>
<div id="stage"><img id="slide" alt="Current slide"></div>
<div id="sub" aria-live="polite"></div>
</main>
<script>
const t = new URLSearchParams(location.search).get("t");
const img = document.getElementById("slide"), sub = document.getElementById("sub"), sel = document.getElementById("lang"), count = document.getElementById("count");
sel.value = localStorage.getItem("prism.live.lang") || (navigator.language || "").slice(0, 2);
if (![...sel.options].some(o => o.value === sel.value)) sel.value = "";
let es = null, slide = -1;
function connect() {
  es && es.close();
  es = new EventSource("/events?t=" + encodeURIComponent(t) + "&lang=" + encodeURIComponent(sel.value));
  es.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.slide !== slide || !img.src) { slide = m.slide; img.src = "/slide.png?t=" + encodeURIComponent(t) + "&v=" + m.version; }
    count.textContent = m.total ? (m.slide + 1) + " / " + m.total : "";
    sub.textContent = m.subtitle || "";
    sub.className = m.final ? "" : "interim";
    document.documentElement.dir = sel.value === "ar" ? "rtl" : "ltr";
  };
  es.onerror = () => { setTimeout(connect, 2000); es.close(); };
}
sel.onchange = () => { localStorage.setItem("prism.live.lang", sel.value); connect(); };
connect();
</script></body></html>"#;

/// Registry state for the Tauri commands.
#[derive(Default)]
pub struct LiveState(pub Mutex<Option<LiveServer>>);

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn get(port: u16, path: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn serves_viewer_slide_and_events_with_token() {
        let srv = LiveServer::start().unwrap();
        let tok = srv.info().token;
        assert!(get(srv.port, "/?t=wrong").starts_with("HTTP/1.1 403"));
        let page = get(srv.port, &format!("/?t={tok}"));
        assert!(page.starts_with("HTTP/1.1 200") && page.contains("EventSource"));
        srv.set_slide(2, 10, vec![0x89, b'P', b'N', b'G']);
        let png = get(srv.port, &format!("/slide.png?t={tok}"));
        assert!(png.contains("image/png") && png.ends_with("PNG"));
        // events: a viewer asking for German gets the translation
        let mut s = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
        write!(s, "GET /events?t={tok}&lang=de HTTP/1.1\r\n\r\n").unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(s);
        let mut first = String::new();
        // skip headers
        loop {
            let mut l = String::new();
            reader.read_line(&mut l).unwrap();
            if l == "\r\n" {
                break;
            }
        }
        reader.read_line(&mut first).unwrap();
        assert!(first.contains("\"slide\":2"), "{first}");
        // the viewer's language is reported to the app
        for _ in 0..50 {
            if srv.status().languages == ["de"] {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(srv.status().languages, ["de"]);
        assert_eq!(srv.status().viewers, 1);
        let mut tr = HashMap::new();
        tr.insert("de".to_string(), "Hallo zusammen".to_string());
        srv.set_subtitle("Hello everyone".into(), tr, true);
        let mut blank = String::new();
        reader.read_line(&mut blank).unwrap();
        let mut next = String::new();
        reader.read_line(&mut next).unwrap();
        assert!(next.contains("Hallo zusammen"), "{next}");
        srv.stop();
    }
}
