//! `cargo run -p prism-sheet --example xlsx_roundtrip -- in.xlsx out.xlsx`
//! Reads a workbook, recalculates it and writes it back (manual interop checks).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (&args[1], &args[2]);
    let bytes = std::fs::read(input).expect("read input");
    let (mut model, source) = prism_sheet::xlsx::read_xlsx(&bytes).expect("parse xlsx");
    if args.iter().any(|a| a == "--demo-edits") {
        use prism_sheet::model::CellModel;
        model.sheets[0].cells.push(CellModel {
            r: 0,
            c: 10,
            f: Some("SORT(UNIQUE(A2:A5))".into()),
            v: None,
            s: 0,
        });
        model.sheets[0].cells.push(CellModel {
            r: 0,
            c: 11,
            f: Some("GROUPBY(A2:A5,C2:C5,SUM)".into()),
            v: None,
            s: 0,
        });
    }
    let out = prism_sheet::xlsx::write_xlsx(&model, Some(&source), 45306.5).expect("write xlsx");
    std::fs::write(output, out).expect("write output");
}
