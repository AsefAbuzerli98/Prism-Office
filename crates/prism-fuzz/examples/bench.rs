//! Open/save timings for the test corpora (P5-01 profiling):
//! `cargo run -p prism-fuzz --release --example bench [iterations]`.
use std::time::Instant;

fn files(dir: &str, ext: &str) -> Vec<(String, Vec<u8>)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(dir);
    let mut out = Vec::new();
    for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == ext) {
            out.push((
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            ));
        }
    }
    out.sort();
    out
}

fn time<F: FnMut()>(n: usize, mut f: F) -> f64 {
    let t = Instant::now();
    for _ in 0..n {
        f();
    }
    t.elapsed().as_secs_f64() * 1000.0 / n as f64
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    println!(
        "{:<28} {:>9} {:>10} {:>10}",
        "file", "KiB", "open ms", "save ms"
    );
    for (name, b) in files("crates/prism-word/tests/corpus", "docx") {
        let open = time(n, || {
            let p = prism_openxml::Package::from_bytes(&b).unwrap();
            let _ = prism_word::read_package(&p);
        });
        let mut p = prism_openxml::Package::from_bytes(&b).unwrap();
        let Ok(doc) = prism_word::read_package(&p) else {
            continue;
        };
        let save = time(n, || {
            prism_word::write_package(&mut p, &doc).unwrap();
            let _ = p.to_bytes().unwrap();
        });
        println!(
            "{name:<28} {:>9.1} {open:>10.2} {save:>10.2}",
            b.len() as f64 / 1024.0
        );
    }
    for (name, b) in files("crates/prism-sheet/tests/fixtures", "xlsx") {
        let open = time(n, || {
            let _ = prism_sheet::xlsx::read_xlsx(&b).unwrap();
        });
        let (m, src) = prism_sheet::xlsx::read_xlsx(&b).unwrap();
        let save = time(n, || {
            let _ = prism_sheet::xlsx::write_xlsx(&m, Some(&src), 45000.0).unwrap();
        });
        println!(
            "{name:<28} {:>9.1} {open:>10.2} {save:>10.2}",
            b.len() as f64 / 1024.0
        );
    }
    for (name, b) in files("crates/prism-slide/tests/fixtures", "pptx") {
        let open = time(n, || {
            let _ = prism_slide::read_pptx(&b).unwrap();
        });
        let (m, src) = prism_slide::read_pptx(&b).unwrap();
        let save = time(n, || {
            let _ = prism_slide::write_pptx(&m, &src).unwrap();
        });
        println!(
            "{name:<28} {:>9.1} {open:>10.2} {save:>10.2}",
            b.len() as f64 / 1024.0
        );
    }
}
