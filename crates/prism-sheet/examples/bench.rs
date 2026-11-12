//! `cargo run --release -p prism-sheet --example bench -- 1000000`
use std::time::Instant;

use prism_sheet::api::{Edit, Engine};
use prism_sheet::model::{CellModel, CellValueModel, SheetModel, WorkbookModel};

fn main() {
    let n: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(200_000);
    let mut cells = Vec::with_capacity(n as usize * 2 + 1);
    for r in 0..n {
        cells.push(CellModel {
            r,
            c: 0,
            f: None,
            v: Some(CellValueModel::Number((r % 1000) as f64)),
            s: 0,
        });
        cells.push(CellModel {
            r,
            c: 1,
            f: Some(format!("A{}*2", r + 1)),
            v: None,
            s: 0,
        });
    }
    cells.push(CellModel {
        r: 0,
        c: 2,
        f: Some("SUM(B:B)".into()),
        v: None,
        s: 0,
    });
    let m = WorkbookModel {
        sheets: vec![SheetModel {
            name: "S".into(),
            cells,
            ..Default::default()
        }],
        num_formats: vec!["General".into()],
        ..Default::default()
    };
    let json = serde_json::to_string(&m).unwrap();
    let t = Instant::now();
    let m: WorkbookModel = serde_json::from_str(&json).unwrap();
    println!(
        "parse json ({} MB): {:?}",
        json.len() / 1_000_000,
        t.elapsed()
    );
    let t = Instant::now();
    let wb = prism_sheet::Workbook::from_model(&m);
    println!("from_model: {:?}", t.elapsed());
    drop(wb);
    let t = Instant::now();
    let mut e = Engine::new(&m, 45000.0);
    println!("load + full calc: {:?}", t.elapsed());
    let t = Instant::now();
    e.wb.put_input(0, prism_sheet::CellRef::new(600, 0), "8")
        .unwrap();
    println!("  put_input: {:?}", t.elapsed());
    let t = Instant::now();
    let r = e.wb.recalc(45000.0);
    println!("  recalc: {:?} ({} evaluated)", t.elapsed(), r.evaluated);
    let t = Instant::now();
    let ch = e.edit(
        &[Edit {
            sheet: 0,
            r: 500,
            c: 0,
            text: "7".into(),
        }],
        45000.0,
    );
    println!(
        "edit one input: {:?} ({} evaluated)",
        t.elapsed(),
        ch.evaluated
    );
    let t = Instant::now();
    for i in 0..100 {
        let r0 = i * (n / 100);
        e.view(
            0,
            prism_sheet::Rect {
                r0,
                c0: 0,
                r1: r0 + 40,
                c1: 20,
            },
        );
    }
    println!("viewport: {:?}/frame", t.elapsed() / 100);
    let t = Instant::now();
    let _ = e.model();
    println!("to_model: {:?}", t.elapsed());
}
