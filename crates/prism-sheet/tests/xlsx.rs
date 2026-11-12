#![cfg(feature = "xlsx")]

use prism_openxml::{Package, PartName};
use prism_sheet::model::{CellModel, CellValueModel, SheetModel, WorkbookModel};
use prism_sheet::xlsx::{read_xlsx, write_xlsx};
use prism_sheet::{Value, Workbook};

const NOW: f64 = 45306.5;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn cell<'a>(m: &'a WorkbookModel, sheet: usize, a1: &str) -> Option<&'a CellModel> {
    let p = prism_sheet::addr::parse_a1(a1).unwrap().0;
    m.sheets[sheet]
        .cells
        .iter()
        .find(|c| c.r == p.row && c.c == p.col)
}

fn check_sales(m: &WorkbookModel) {
    assert_eq!(
        m.sheets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["Sales", "Rates"]
    );
    assert_eq!(
        cell(m, 0, "A2").unwrap().v,
        Some(CellValueModel::Text("East".into()))
    );
    assert_eq!(cell(m, 0, "F2").unwrap().f.as_deref(), Some("C2*D2"));
    assert_eq!(
        cell(m, 0, "I2").unwrap().f.as_deref(),
        Some("XLOOKUP(\"North\",A2:A5,F2:F5)")
    );
    assert_eq!(
        cell(m, 0, "I3").unwrap().v,
        Some(CellValueModel::Bool(true))
    );
    assert_eq!(
        cell(m, 0, "I4").unwrap().v,
        Some(CellValueModel::Text("  spaced & <tagged>  ".into()))
    );
    let d2 = cell(m, 0, "D2").unwrap();
    assert_eq!(m.num_formats[d2.s as usize], "#,##0.00");
    let e2 = cell(m, 0, "E2").unwrap();
    assert_eq!(e2.v, Some(CellValueModel::Number(45306.0)));
    assert!(prism_sheet::numfmt::is_date_format(
        &m.num_formats[e2.s as usize]
    ));
    assert_eq!(m.sheets[0].frozen, (1, 1));
    assert_eq!(m.sheets[0].merges.len(), 1);
    assert!(m.sheets[0]
        .col_widths
        .iter()
        .any(|(c, w)| *c == 0 && (*w - 18.0).abs() < 1.0));
    assert_eq!(m.sheets[0].tables[0].name, "SalesTbl");
    assert_eq!(m.sheets[0].tables[0].columns.len(), 6);
    assert!(m
        .names
        .iter()
        .any(|n| n.name == "TaxRate" && n.formula == "Rates!$B$1"));

    let mut wb = Workbook::from_model(m);
    wb.recalc(NOW);
    assert_eq!(wb.get(0, "F2"), Value::Number(15.0));
    assert_eq!(wb.get(0, "I1"), Value::Number(15.0 + 45.0 + 60.0 + 70.0));
    assert_eq!(wb.get(0, "I2"), Value::Number(70.0));
    assert!(matches!(wb.get(0, "I5"), Value::Number(x) if (x - 0.18 * 190.0).abs() < 1e-9));
}

#[test]
fn reads_openpyxl_and_libreoffice_files() {
    let (m, _) = read_xlsx(&fixture("openpyxl.xlsx")).unwrap();
    check_sales(&m);
    let (m, _) = read_xlsx(&fixture("libreoffice.xlsx")).unwrap();
    check_sales(&m);
}

#[test]
fn unchanged_round_trip_preserves_everything_else() {
    for f in ["openpyxl.xlsx", "libreoffice.xlsx"] {
        let src_bytes = fixture(f);
        let (m, src) = read_xlsx(&src_bytes).unwrap();
        let out = write_xlsx(&m, Some(&src), NOW).unwrap();
        let (m2, _) = read_xlsx(&out).unwrap();
        check_sales(&m2);
        assert_eq!(m.sheets[0].cells.len(), m2.sheets[0].cells.len(), "{f}");
        // passthrough: comments, data validation, table part, theme
        let a = Package::from_bytes(&src_bytes).unwrap();
        let b = Package::from_bytes(&out).unwrap();
        for p in a.part_names() {
            if p.as_str().contains("comments")
                || p.as_str().contains("tables/")
                || p.as_str().contains("theme")
                || p.as_str().contains("vmlDrawing")
            {
                assert_eq!(a.part(p), b.part(p), "{f}: {p} changed");
            }
        }
        let sheet = String::from_utf8(
            b.part(&PartName::new("/xl/worksheets/sheet1.xml").unwrap())
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(sheet.contains("<dataValidations"), "{f}");
        assert!(sheet.contains("<tableParts"), "{f}");
        assert!(sheet.contains("legacyDrawing"), "{f}");
        assert!(!b.part_names().any(|p| p.as_str().contains("calcChain")));
    }
}

#[test]
fn edits_dynamic_arrays_and_new_sheets() {
    let (mut m, src) = read_xlsx(&fixture("openpyxl.xlsx")).unwrap();
    m.sheets[0].cells.push(CellModel {
        r: 0,
        c: 10,
        f: Some("SORT(UNIQUE(A2:A5))".into()),
        v: None,
        s: 0,
    });
    m.sheets[0].cells.push(CellModel {
        r: 9,
        c: 0,
        f: Some("REGEXREPLACE(B2,\"p+\",\"P\")".into()),
        v: None,
        s: 0,
    });
    m.sheets[0].cells.push(CellModel {
        r: 11,
        c: 0,
        f: Some("GROUPBY(A2:A5,C2:C5,SUM,,0)".into()),
        v: None,
        s: 0,
    });
    m.sheets[0].cells.push(CellModel {
        r: 11,
        c: 3,
        f: Some("LET(k,2,MAP(C2:C5,LAMBDA(v,v*k)))".into()),
        v: None,
        s: 0,
    });
    while m.styles.len() < m.num_formats.len() {
        m.styles.push(Default::default());
    }
    m.num_formats.push("0.000".into());
    let look = prism_sheet::model::CellStyle {
        bold: true,
        color: Some("#C00000".into()),
        fill: Some("#FFF2CC".into()),
        h_align: Some("center".into()),
        border: 8,
        ..Default::default()
    };
    m.styles.push(look.clone());
    let s = (m.num_formats.len() - 1) as u32;
    m.sheets[0].cells.push(CellModel {
        r: 10,
        c: 0,
        f: None,
        v: Some(CellValueModel::Number(1.23456)),
        s,
    });
    m.sheets.push(SheetModel {
        name: "New & <Sheet>".into(),
        cells: vec![CellModel {
            r: 0,
            c: 0,
            f: Some("Sales!I1*2".into()),
            v: None,
            s: 0,
        }],
        ..Default::default()
    });
    m.sheets[0].frozen = (2, 0);
    let out = write_xlsx(&m, Some(&src), NOW).unwrap();
    let pkg = Package::from_bytes(&out).unwrap();
    let sheet = String::from_utf8(
        pkg.part(&PartName::new("/xl/worksheets/sheet1.xml").unwrap())
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(sheet.contains(r#"<c r="K1" t="str" cm="1"><f t="array" ref="K1:K3">_xlfn._xlws.SORT(_xlfn.UNIQUE(A2:A5))</f><v>East</v></c>"#), "{sheet}");
    assert!(sheet.contains(r#"<c r="K3" t="str"><v>West</v></c>"#));
    assert!(sheet.contains("_xlfn.REGEXREPLACE"));
    assert!(
        sheet.contains("_xlfn.GROUPBY(A2:A5,C2:C5,_xleta.SUM,,0)"),
        "{sheet}"
    );
    assert!(
        sheet.contains(
            "_xlfn.LET(_xlpm.k,2,_xlfn.MAP(C2:C5,_xlfn.LAMBDA(_xlpm.v,_xlpm.v*_xlpm.k)))"
        ),
        "{sheet}"
    );
    assert!(sheet.contains(r#"ySplit="2""#));
    assert!(pkg.part_names().any(|p| p.as_str() == "/xl/metadata.xml"));
    let styles = String::from_utf8(
        pkg.part(&PartName::new("/xl/styles.xml").unwrap())
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(styles.contains(r#"formatCode="0.000""#));
    let (m2, _) = read_xlsx(&out).unwrap();
    assert_eq!(m2.sheets.len(), 3);
    assert_eq!(m2.sheets[2].name, "New & <Sheet>");
    assert_eq!(
        cell(&m2, 0, "K1").unwrap().f.as_deref(),
        Some("SORT(UNIQUE(A2:A5))")
    );
    assert!(cell(&m2, 0, "K2").is_none(), "spilled cells are not inputs");
    assert_eq!(
        m2.num_formats[cell(&m2, 0, "A11").unwrap().s as usize],
        "0.000"
    );
    let mut wb = Workbook::from_model(&m2);
    wb.recalc(NOW);
    assert_eq!(wb.get(0, "K2"), Value::text("North"));
    assert_eq!(wb.get(0, "A10"), Value::text("APle"));
    assert_eq!(
        cell(&m2, 0, "A12").unwrap().f.as_deref(),
        Some("GROUPBY(A2:A5,C2:C5,SUM,,0)")
    );
    assert_eq!(wb.get(0, "B12"), Value::Number(40.0));
    assert_eq!(wb.get(0, "D15"), Value::Number(80.0));
    assert_eq!(wb.get(2, "A1"), Value::Number(380.0));
    // removing a sheet drops its part
    m.sheets.remove(1);
    m.sheets[0]
        .cells
        .retain(|c| c.f.as_deref() != Some("TaxRate*I1"));
    m.names.clear();
    let out = write_xlsx(&m, Some(&src), NOW).unwrap();
    let (m3, _) = read_xlsx(&out).unwrap();
    assert_eq!(
        m3.sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Sales", "New & <Sheet>"]
    );
    assert!(m3.names.is_empty());
}

#[test]
fn shared_and_array_formulas_from_excel() {
    let mut pkg = prism_openxml::templates::blank(prism_openxml::DocumentKind::Workbook);
    let sheet = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1"><v>1</v></c><c r="B1"><f t="shared" ref="B1:B3" si="0">A1*10</f><v>10</v></c><c r="D1" cm="1"><f t="array" ref="D1:D3">_xlfn.SEQUENCE(3)</f><v>1</v></c></row>
<row r="2"><c r="A2"><v>2</v></c><c r="B2"><f t="shared" si="0"/><v>20</v></c><c r="D2"><v>2</v></c></row>
<row r="3"><c r="A3"><v>3</v></c><c r="B3"><f t="shared" si="0"/><v>30</v></c><c r="D3"><v>3</v></c><c r="E3" t="inlineStr"><is><t>inline</t></is></c><c r="F3" t="e"><v>#N/A</v></c></row>
</sheetData></worksheet>"#;
    pkg.set_part(
        PartName::new("/xl/worksheets/sheet1.xml").unwrap(),
        sheet.as_bytes().to_vec(),
        None,
    );
    let (m, _) = read_xlsx(&pkg.to_bytes().unwrap()).unwrap();
    assert_eq!(cell(&m, 0, "B3").unwrap().f.as_deref(), Some("A3*10"));
    assert_eq!(cell(&m, 0, "D1").unwrap().f.as_deref(), Some("SEQUENCE(3)"));
    assert!(cell(&m, 0, "D2").is_none());
    assert_eq!(
        cell(&m, 0, "E3").unwrap().v,
        Some(CellValueModel::Text("inline".into()))
    );
    assert_eq!(
        cell(&m, 0, "F3").unwrap().v,
        Some(CellValueModel::Error { e: "#N/A".into() })
    );
    let mut wb = Workbook::from_model(&m);
    wb.recalc(NOW);
    assert_eq!(wb.get(0, "D3"), Value::Number(3.0));
    assert_eq!(wb.get(0, "B2"), Value::Number(20.0));
}

#[test]
fn new_workbook_from_blank_model() {
    let mut m = WorkbookModel::blank();
    m.sheets[0].cells.push(CellModel {
        r: 0,
        c: 0,
        f: None,
        v: Some(CellValueModel::Number(2.0)),
        s: 0,
    });
    m.sheets[0].cells.push(CellModel {
        r: 0,
        c: 1,
        f: Some("A1^10".into()),
        v: None,
        s: 0,
    });
    let out = write_xlsx(&m, None, NOW).unwrap();
    let (m2, _) = read_xlsx(&out).unwrap();
    assert_eq!(
        cell(&m2, 0, "B1").unwrap().v,
        Some(CellValueModel::Number(1024.0))
    );
    std::fs::write(std::env::temp_dir().join("prism-sheet-new.xlsx"), &out).unwrap();
}

#[test]
fn charts_are_written_and_read_back() {
    let (mut m, src) = read_xlsx(&fixture("openpyxl.xlsx")).unwrap();
    let rect = |r0, c0, r1, c1| prism_sheet::Rect { r0, c0, r1, c1 };
    m.sheets[0].charts.push(prism_sheet::model::ChartDef {
        kind: "column".into(),
        title: "Units & revenue".into(),
        data: rect(0, 2, 4, 2),
        anchor: rect(9, 7, 22, 13),
        from_file: false,
    });
    m.sheets[1].charts.push(prism_sheet::model::ChartDef {
        kind: "pie".into(),
        title: String::new(),
        data: rect(0, 0, 0, 1),
        anchor: rect(2, 0, 12, 5),
        from_file: false,
    });
    let out = write_xlsx(&m, Some(&src), NOW).unwrap();
    std::fs::write(std::env::temp_dir().join("prism-sheet-charts.xlsx"), &out).unwrap();
    let pkg = Package::from_bytes(&out).unwrap();
    let names: Vec<String> = pkg.part_names().map(|p| p.as_str().to_string()).collect();
    assert!(
        names.iter().any(|n| n.starts_with("/xl/charts/chart")),
        "{names:?}"
    );
    let chart = String::from_utf8(
        pkg.part(&PartName::new("/xl/charts/chart1.xml").unwrap())
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(chart.contains("<c:barDir val=\"col\"/>"));
    assert!(chart.contains("<c:f>Sales!$C$2:$C$5</c:f>"), "{chart}");
    assert!(chart.contains("Units &amp; revenue"));
    // the comment's legacy VML drawing is untouched; the chart drawing is separate
    let (m2, _) = read_xlsx(&out).unwrap();
    let c = &m2.sheets[0].charts[0];
    assert_eq!(
        (c.kind.as_str(), c.title.as_str(), c.from_file),
        ("column", "Units & revenue", true)
    );
    assert_eq!(c.data, rect(0, 2, 4, 2));
    assert_eq!(c.anchor, rect(9, 7, 22, 13));
    assert_eq!(m2.sheets[1].charts[0].kind, "pie");
    // saving again keeps exactly one chart per definition
    let out2 = write_xlsx(&m2, Some(&read_xlsx(&out).unwrap().1), NOW).unwrap();
    let (m3, _) = read_xlsx(&out2).unwrap();
    assert_eq!(m3.sheets[0].charts.len(), 1);
}
