//! Stable mutation fuzzer: `cargo run -p prism-fuzz --release -- <target|all> [seconds] [seed]`.
//!
//! Seeds come from the repository's test corpora. Office files are mutated
//! *inside* the ZIP (an XML part is mutated and the package re-zipped), so
//! inputs get past the CRC checks and reach the XML readers; raw byte
//! mutations exercise the ZIP layer. Panics are caught and the input saved
//! to `target/fuzz-artifacts/<target>-<n>.bin`.

use std::io::{Read, Write};
use std::panic;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Target name, input and start time of the execution in flight (hang watchdog).
type Current = (String, Vec<u8>, Option<Instant>);

thread_local! {
    static LAST: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

const TOKENS: &[&str] = &[
    "<",
    ">",
    "/>",
    "</",
    "\"",
    "&amp;",
    "&#x0;",
    "&#xD800;",
    "<!--",
    "]]>",
    "<![CDATA[",
    " val=\"",
    "-1",
    "0",
    "4294967296",
    "1e309",
    "NaN",
    "999999999999",
    "r:id=\"rId999\"",
    "Target=\"../../../../etc/passwd\"",
    "TargetMode=\"External\"",
    "xmlns:a=\"\"",
    "\u{0}",
    "\u{FFFF}",
    "\u{1F600}",
    "=",
    "(",
    ")",
    ",",
    ":",
    "!",
    "#REF!",
    "SUM(",
    "LAMBDA(x,",
    "LET(",
    "INDIRECT(",
    "A1:XFD1048576",
    "1:1048576",
    "R[-1]C",
    "{1,2;3}",
    "\"\"",
    "%",
    "^",
    "&",
    "@",
    "#",
    "[",
    "]",
    "'",
    "_xlfn.",
    "\\",
    "*",
    "?",
    ";",
    "General",
    "0.00E+00",
    "[Red]",
    "yyyy",
    "@\"x\"",
    "[h]:mm",
    "#,##0",
    "???/???",
];

fn mutate(rng: &mut Rng, data: &mut Vec<u8>, dict: &[Vec<u8>]) {
    let rounds = 1 + rng.below(6);
    for _ in 0..rounds {
        let len = data.len();
        match rng.below(9) {
            0 if len > 0 => {
                let i = rng.below(len);
                data[i] ^= 1 << rng.below(8);
            }
            1 if len > 0 => {
                let i = rng.below(len);
                data[i] = rng.next() as u8;
            }
            2 if len > 0 => {
                let i = rng.below(len);
                let n = 1 + rng.below(len.min(64));
                data.drain(i..(i + n).min(len));
            }
            3 => {
                let i = rng.below(len + 1);
                let t = TOKENS[rng.below(TOKENS.len())].as_bytes();
                data.splice(i..i, t.iter().copied());
            }
            4 if len > 1 => {
                // duplicate a chunk (nesting, repetition)
                let a = rng.below(len);
                let n = 1 + rng.below((len - a).min(256));
                let chunk = data[a..a + n].to_vec();
                let at = rng.below(len + 1);
                data.splice(at..at, chunk);
            }
            5 if !dict.is_empty() => {
                // splice from another seed
                let other = &dict[rng.below(dict.len())];
                if !other.is_empty() {
                    let a = rng.below(other.len());
                    let n = 1 + rng.below((other.len() - a).min(512));
                    let at = rng.below(len + 1);
                    data.splice(at..at, other[a..a + n].iter().copied());
                }
            }
            6 if len > 0 => {
                // interesting integers inside numbers
                let i = rng.below(len);
                if data[i].is_ascii_digit() {
                    let t = [
                        "0",
                        "-1",
                        "65535",
                        "2147483648",
                        "18446744073709551616",
                        "1e400",
                        "-0",
                    ][rng.below(7)];
                    data.splice(i..i + 1, t.bytes());
                }
            }
            7 if len > 2 => {
                let i = rng.below(len);
                let j = rng.below(len);
                data.swap(i, j);
            }
            _ => {
                if len > 4096 {
                    data.truncate(rng.below(len));
                }
            }
        }
    }
}

/// Mutate one part of a ZIP and re-zip it (stored entries).
fn mutate_zip(rng: &mut Rng, zip: &[u8], dict: &[Vec<u8>]) -> Option<Vec<u8>> {
    let mut parts = read_zip(zip)?;
    if parts.is_empty() {
        return None;
    }
    let n = 1 + rng.below(2);
    for _ in 0..n {
        let i = rng.below(parts.len());
        mutate(rng, &mut parts[i].1, dict);
    }
    if rng.below(10) == 0 {
        let i = rng.below(parts.len());
        parts.remove(i);
    }
    Some(write_zip(&parts))
}

fn read_zip(bytes: &[u8]) -> Option<Vec<(String, Vec<u8>)>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut out = Vec::new();
    for i in 0..z.len().min(500) {
        let mut f = z.by_index(i).ok()?;
        if f.size() > 16 * 1024 * 1024 {
            return None;
        }
        let mut d = Vec::new();
        f.read_to_end(&mut d).ok()?;
        out.push((f.name().ok()?.to_string(), d));
    }
    Some(out)
}

/// Write a ZIP with stored entries.
fn write_zip(parts: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, data) in parts {
        if w.start_file(name.as_str(), opts).is_ok() {
            let _ = w.write_all(data);
        }
    }
    w.finish().map(|c| c.into_inner()).unwrap_or_default()
}

fn files(dir: &Path, ext: &[&str], out: &mut Vec<Vec<u8>>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p
            .extension()
            .is_some_and(|x| ext.iter().any(|e| x.eq_ignore_ascii_case(e)))
        {
            if let Ok(mut f) = std::fs::File::open(&p) {
                let mut b = Vec::new();
                if f.read_to_end(&mut b).is_ok() {
                    out.push(b);
                }
            }
        }
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn seeds(target: &str) -> (Vec<Vec<u8>>, bool) {
    let r = root();
    let mut out = Vec::new();
    let zip = matches!(target, "openxml" | "docx" | "xlsx" | "pptx");
    match target {
        "docx" => files(&r.join("crates/prism-word/tests"), &["docx"], &mut out),
        "xlsx" => files(&r.join("crates/prism-sheet/tests"), &["xlsx"], &mut out),
        "pptx" => files(&r.join("crates/prism-slide/tests"), &["pptx"], &mut out),
        "openxml" => {
            files(&r.join("crates"), &["docx", "xlsx", "pptx"], &mut out);
        }
        "drawingml" => {
            let mut z = Vec::new();
            files(&r.join("crates/prism-slide/tests"), &["pptx"], &mut z);
            for b in z {
                if let Ok(p) = prism_openxml::Package::from_bytes(&b) {
                    for n in p.part_names() {
                        let s = n.as_str();
                        if s.contains("slides/slide")
                            || s.contains("theme")
                            || s.contains("slideLayout")
                        {
                            if let Some(d) = p.part(n) {
                                out.push(d.to_vec());
                            }
                        }
                    }
                }
            }
        }
        "formula" => {
            for f in [
                "SUM(A1:A10)*2",
                "IF(A1>2,\"big\",\"small\")",
                "XLOOKUP(3,A1:A3,B1:B3,,0,1)",
                "LET(x,A1,x*x)",
                "LAMBDA(a,b,a+b)(1,2)",
                "SEQUENCE(3,3)",
                "TEXT(A1,\"0.00\")",
                "REGEXEXTRACT(\"abc123\",\"[0-9]+\")",
                "A1#",
                "SUM(Sheet1!A:A)",
                "INDEX(A1:C3,2,2)",
                "{1,2;3,4}",
                "-A1^2%",
                "MAP(SEQUENCE(3),LAMBDA(v,v*2))",
                "GROUPBY(A1:A3,B1:B3,SUM)",
            ] {
                out.push(f.as_bytes().to_vec());
            }
        }
        "numfmt" => {
            for f in [
                "General",
                "0.00",
                "#,##0.00 \"₼\"",
                "[Red]-0.00;[Blue]0.00",
                "yyyy-mm-dd hh:mm:ss",
                "[h]:mm:ss",
                "# ?/?",
                "0.00E+00",
                "@",
                "\"x\"@",
                "mmmm d, yyyy",
                "0%",
                "[$-409]dddd",
            ] {
                out.push(f.as_bytes().to_vec());
            }
        }
        _ => {
            for _ in 0..8 {
                out.push(vec![0u8; 40]);
            }
        }
    }
    (out, zip)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("repro") {
        // prism-fuzz repro <target> <file>...
        let t = args.get(2).cloned().unwrap_or_default();
        let f = prism_fuzz::TARGETS
            .iter()
            .find(|(n, _)| *n == t)
            .map(|x| x.1)
            .expect("target");
        let mut bad = 0;
        for path in &args[3..] {
            let data = std::fs::read(path).expect("read");
            let ok = panic::catch_unwind(|| f(&data)).is_ok();
            println!("{} {path}", if ok { "ok   " } else { "PANIC" });
            bad += usize::from(!ok);
        }
        std::process::exit(i32::from(bad > 0));
    }
    let which = args.get(1).map(String::as_str).unwrap_or("all");
    let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    let seed: u64 = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let targets: Vec<_> = prism_fuzz::TARGETS
        .iter()
        .filter(|(n, _)| which == "all" || *n == which)
        .collect();
    if targets.is_empty() {
        eprintln!(
            "unknown target {which}; targets: {}",
            prism_fuzz::TARGETS
                .iter()
                .map(|t| t.0)
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(2);
    }
    let art = root().join("target/fuzz-artifacts");
    let _ = std::fs::create_dir_all(&art);
    panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        LAST.with(|c| *c.borrow_mut() = loc);
    }));
    let per = Duration::from_secs(secs.max(1) / targets.len().max(1) as u64 + 1);
    // watchdog: an execution running > 10 s is a hang — save the input and stop
    let current: std::sync::Arc<std::sync::Mutex<Current>> = Default::default();
    {
        let current = current.clone();
        let art = art.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            let g = current.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(t0) = g.2 {
                if t0.elapsed() > Duration::from_secs(10) {
                    let path = art.join(format!("{}-hang.bin", g.0));
                    let _ = std::fs::write(&path, &g.1);
                    println!("HANG {}: input took > 10 s -> {}", g.0, path.display());
                    std::process::exit(1);
                }
            }
        });
    }
    let mut total_crashes = 0;
    let save_current = std::env::var_os("PRISM_FUZZ_SAVE").is_some();
    for (name, f) in targets {
        let (corpus, zip) = seeds(name);
        if corpus.is_empty() {
            eprintln!("{name}: no seeds");
            continue;
        }
        let mut rng = Rng(seed ^ (name.len() as u64 * 0x1000_0001));
        let start = Instant::now();
        let mut execs = 0u64;
        let mut crashes = 0;
        let mut seen = std::collections::HashSet::new();
        // seeds themselves must pass
        for s in &corpus {
            if panic::catch_unwind(|| f(s)).is_err() {
                eprintln!("{name}: a seed input panics");
            }
        }
        while start.elapsed() < per {
            let base = &corpus[rng.below(corpus.len())];
            let input = if zip && rng.below(4) != 0 {
                match mutate_zip(&mut rng, base, &corpus) {
                    Some(z) => z,
                    None => continue,
                }
            } else {
                let mut d = base.clone();
                mutate(&mut rng, &mut d, &corpus);
                d
            };
            execs += 1;
            if save_current {
                // aborts (stack overflow) cannot be caught: keep the input on disk first
                let _ = std::fs::write(art.join(format!("current-{name}.bin")), &input);
            }
            {
                let mut g = current.lock().unwrap_or_else(|e| e.into_inner());
                g.0.clear();
                g.0.push_str(name);
                g.1.clear();
                g.1.extend_from_slice(&input);
                g.2 = Some(Instant::now());
            }
            let r = panic::catch_unwind(|| f(&input));
            current.lock().unwrap_or_else(|e| e.into_inner()).2 = None;
            if let Err(e) = r {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                let loc = LAST.with(|c| c.borrow().clone());
                let key = format!("{loc} {}", msg.chars().take(80).collect::<String>());
                let dedup = loc.clone();
                if seen.insert(dedup) {
                    crashes += 1;
                    let path = art.join(format!("{name}-{crashes}.bin"));
                    if let Ok(mut fh) = std::fs::File::create(&path) {
                        let _ = fh.write_all(&input);
                    }
                    println!("CRASH {name}: {key}  -> {}", path.display());
                }
            }
        }
        total_crashes += crashes;
        println!(
            "{name}: {execs} execs in {:.0}s, {crashes} unique panics",
            start.elapsed().as_secs_f64()
        );
    }
    if total_crashes > 0 {
        std::process::exit(1);
    }
}
