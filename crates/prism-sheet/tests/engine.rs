use prism_sheet::{ErrorKind, Value, Workbook};

const NOW: f64 = 45306.5; // 2024-01-15 12:00

fn wb_with(cells: &[(&str, &str)]) -> Workbook {
    let mut wb = Workbook::new();
    for (a, t) in cells {
        let c = prism_sheet::addr::parse_a1(a).unwrap().0;
        wb.put_input(0, c, t).unwrap();
    }
    wb.recalc(NOW);
    wb
}

fn eval(wb: &mut Workbook, f: &str) -> Value {
    wb.set(0, "ZZ1000", f, NOW).unwrap();
    wb.get(0, "ZZ1000")
}

fn n(x: f64) -> Value {
    Value::Number(x)
}
fn t(s: &str) -> Value {
    Value::text(s)
}

fn data() -> Workbook {
    // A: region, B: product, C: sales
    wb_with(&[
        ("A1", "Region"),
        ("B1", "Product"),
        ("C1", "Sales"),
        ("A2", "East"),
        ("B2", "Apple"),
        ("C2", "10"),
        ("A3", "West"),
        ("B3", "Pear"),
        ("C3", "20"),
        ("A4", "East"),
        ("B4", "Pear"),
        ("C4", "30"),
        ("A5", "North"),
        ("B5", "Apple"),
        ("C5", "40"),
        ("A6", "West"),
        ("B6", "Apple"),
        ("C6", "50"),
    ])
}

#[test]
fn arithmetic_and_precedence() {
    let mut wb = Workbook::new();
    assert_eq!(eval(&mut wb, "=1+2*3"), n(7.0));
    assert_eq!(eval(&mut wb, "=-2^2"), n(4.0));
    assert_eq!(eval(&mut wb, "=2^3^2"), n(64.0));
    assert_eq!(eval(&mut wb, "=10%*50"), n(5.0));
    assert_eq!(eval(&mut wb, "=\"a\"&1&TRUE"), t("a1TRUE"));
    assert_eq!(eval(&mut wb, "=1/0"), Value::Error(ErrorKind::Div0));
    assert_eq!(eval(&mut wb, "=\"3\"+4"), n(7.0));
    assert_eq!(eval(&mut wb, "=\"x\"+4"), Value::Error(ErrorKind::Value));
    assert_eq!(eval(&mut wb, "=\"b\">\"A\""), Value::Bool(true));
    assert_eq!(eval(&mut wb, "=1<\"a\""), Value::Bool(true));
    assert_eq!(eval(&mut wb, "=NOPE(1)"), Value::Error(ErrorKind::Name));
}

#[test]
fn incremental_recalc_and_chain() {
    let mut wb = wb_with(&[
        ("A1", "1"),
        ("A2", "=A1*2"),
        ("A3", "=A2+A1"),
        ("B1", "=SUM(A:A)"),
    ]);
    assert_eq!(wb.get(0, "A3"), n(3.0));
    assert_eq!(wb.get(0, "B1"), n(6.0));
    let r = wb.set(0, "A1", "10", NOW).unwrap();
    assert_eq!(wb.get(0, "A2"), n(20.0));
    assert_eq!(wb.get(0, "A3"), n(30.0));
    assert_eq!(wb.get(0, "B1"), n(60.0));
    assert_eq!(r.evaluated, 3);
    // unrelated edit evaluates nothing but itself
    let r = wb.set(0, "D9", "5", NOW).unwrap();
    assert_eq!(r.evaluated, 0);
}

#[test]
fn circular_reference_reports() {
    let mut wb = wb_with(&[("A1", "=B1+1"), ("B1", "=A1+1")]);
    assert_eq!(wb.circular.len(), 2);
    assert_eq!(wb.get(0, "A1"), n(0.0));
    wb.set(0, "B1", "5", NOW).unwrap();
    assert!(wb.circular.is_empty());
    assert_eq!(wb.get(0, "A1"), n(6.0));
}

#[test]
fn spill_block_and_unblock() {
    let mut wb = wb_with(&[("A1", "=SEQUENCE(3)"), ("B1", "=SUM(A1#)")]);
    assert_eq!(wb.get(0, "A3"), n(3.0));
    assert_eq!(wb.get(0, "B1"), n(6.0));
    wb.set(0, "A2", "x", NOW).unwrap();
    assert_eq!(wb.get(0, "A1"), Value::Error(ErrorKind::Spill));
    assert_eq!(wb.get(0, "A3"), Value::Empty);
    wb.set(0, "A2", "", NOW).unwrap();
    assert_eq!(wb.get(0, "A1"), n(1.0));
    assert_eq!(wb.get(0, "A2"), n(2.0));
    // growing spill updates A1# readers
    wb.set(0, "A1", "=SEQUENCE(5)", NOW).unwrap();
    assert_eq!(wb.get(0, "B1"), n(15.0));
    assert_eq!(wb.get(0, "A5"), n(5.0));
    // shrinking frees cells
    wb.set(0, "A1", "=SEQUENCE(2)", NOW).unwrap();
    assert_eq!(wb.get(0, "A5"), Value::Empty);
    assert_eq!(wb.get(0, "B1"), n(3.0));
    // readers of spilled cells by plain reference
    wb.set(0, "C1", "=A2*10", NOW).unwrap();
    assert_eq!(wb.get(0, "C1"), n(20.0));
    wb.set(0, "A1", "=SEQUENCE(2,1,5)", NOW).unwrap();
    assert_eq!(wb.get(0, "C1"), n(60.0));
}

#[test]
fn competing_spills() {
    let mut wb = wb_with(&[("A1", "=SEQUENCE(1,3)"), ("A2", "=SEQUENCE(2)")]);
    assert_eq!(wb.get(0, "C1"), n(3.0));
    assert_eq!(wb.get(0, "A3"), n(2.0));
    wb.set(0, "A3", "=1", NOW).unwrap();
    assert_eq!(wb.get(0, "A2"), Value::Error(ErrorKind::Spill));
    wb.set(0, "A3", "", NOW).unwrap();
    assert_eq!(wb.get(0, "A3"), n(2.0));
}

#[test]
fn math_and_stats() {
    let mut wb = data();
    assert_eq!(eval(&mut wb, "=SUM(C2:C6)"), n(150.0));
    assert_eq!(eval(&mut wb, "=AVERAGE(C2:C6)"), n(30.0));
    assert_eq!(eval(&mut wb, "=COUNT(A1:C6)"), n(5.0));
    assert_eq!(eval(&mut wb, "=COUNTA(A1:A6)"), n(6.0));
    assert_eq!(eval(&mut wb, "=SUMIF(A2:A6,\"East\",C2:C6)"), n(40.0));
    assert_eq!(
        eval(&mut wb, "=SUMIFS(C2:C6,A2:A6,\"West\",B2:B6,\"A*\")"),
        n(50.0)
    );
    assert_eq!(eval(&mut wb, "=COUNTIF(C2:C6,\">=30\")"), n(3.0));
    assert_eq!(eval(&mut wb, "=AVERAGEIFS(C2:C6,B2:B6,\"Pear\")"), n(25.0));
    assert_eq!(eval(&mut wb, "=MAXIFS(C2:C6,A2:A6,\"East\")"), n(30.0));
    assert_eq!(eval(&mut wb, "=ROUND(2.675,2)"), n(2.68));
    assert_eq!(eval(&mut wb, "=ROUND(-1234.5,-2)"), n(-1200.0));
    assert_eq!(eval(&mut wb, "=MOD(-3,2)"), n(1.0));
    assert_eq!(eval(&mut wb, "=MEDIAN(1,2,3,4)"), n(2.5));
    assert_eq!(
        eval(&mut wb, "=STDEV.S(2,4,4,4,5,5,7,9)"),
        n(2.138089935299395)
    );
    assert_eq!(eval(&mut wb, "=LARGE(C2:C6,2)"), n(40.0));
    assert_eq!(
        eval(&mut wb, "=SUMPRODUCT((A2:A6=\"East\")*C2:C6)"),
        n(40.0)
    );
    assert_eq!(eval(&mut wb, "=SUBTOTAL(9,C2:C6)"), n(150.0));
    assert_eq!(eval(&mut wb, "=PRODUCT(2,3,4)"), n(24.0));
}

#[test]
fn logic_let_lambda() {
    let mut wb = data();
    assert_eq!(eval(&mut wb, "=IF(C2>5,\"big\",\"small\")"), t("big"));
    assert_eq!(eval(&mut wb, "=IFERROR(1/0,\"x\")"), t("x"));
    assert_eq!(eval(&mut wb, "=IFS(C3>100,1,C3>10,2)"), n(2.0));
    assert_eq!(eval(&mut wb, "=SWITCH(A2,\"West\",1,\"East\",2,0)"), n(2.0));
    assert_eq!(eval(&mut wb, "=LET(x,2,y,x*3,x+y)"), n(8.0));
    assert_eq!(eval(&mut wb, "=LAMBDA(a,b,a*b)(3,4)"), n(12.0));
    assert_eq!(eval(&mut wb, "=SUM(MAP(C2:C6,LAMBDA(v,v*2)))"), n(300.0));
    assert_eq!(eval(&mut wb, "=REDUCE(0,C2:C6,LAMBDA(a,v,a+v))"), n(150.0));
    assert_eq!(
        eval(&mut wb, "=INDEX(SCAN(0,C2:C6,LAMBDA(a,v,a+v)),5)"),
        n(150.0)
    );
    assert_eq!(eval(&mut wb, "=SUM(BYROW(SEQUENCE(3,2),SUM))"), n(21.0));
    assert_eq!(
        eval(&mut wb, "=SUM(MAKEARRAY(2,2,LAMBDA(r,c,r*c)))"),
        n(9.0)
    );
    assert_eq!(eval(&mut wb, "=AND(TRUE,C2>1)"), Value::Bool(true));
    assert_eq!(eval(&mut wb, "=LAMBDA(x,x)"), Value::Error(ErrorKind::Calc));
    wb.names.push(prism_sheet::workbook::DefinedName {
        name: "DOUBLE".into(),
        formula: "LAMBDA(x,x*2)".into(),
        sheet: None,
    });
    wb.mark_all_dirty();
    wb.recalc(NOW);
    assert_eq!(eval(&mut wb, "=DOUBLE(21)"), n(42.0));
}

#[test]
fn text_functions() {
    let mut wb = Workbook::new();
    assert_eq!(
        eval(&mut wb, "=LEFT(\"Prism\",2)&RIGHT(\"Office\",3)"),
        t("Price")
    );
    assert_eq!(eval(&mut wb, "=MID(\"abcdef\",2,3)"), t("bcd"));
    assert_eq!(eval(&mut wb, "=PROPER(\"hello wORLD\")"), t("Hello World"));
    assert_eq!(eval(&mut wb, "=TRIM(\"  a   b \")"), t("a b"));
    assert_eq!(
        eval(&mut wb, "=SUBSTITUTE(\"a-b-c\",\"-\",\"+\",2)"),
        t("a-b+c")
    );
    assert_eq!(
        eval(&mut wb, "=TEXTJOIN(\", \",TRUE,\"a\",\"\",\"b\")"),
        t("a, b")
    );
    assert_eq!(eval(&mut wb, "=SEARCH(\"o*e\",\"Prism Office\")"), n(7.0));
    assert_eq!(eval(&mut wb, "=FIND(\"O\",\"Prism Office\")"), n(7.0));
    assert_eq!(eval(&mut wb, "=TEXT(1234.5,\"#,##0.00\")"), t("1,234.50"));
    assert_eq!(
        eval(&mut wb, "=TEXT(DATE(2024,1,15),\"dddd d mmm\")"),
        t("Monday 15 Jan")
    );
    assert_eq!(eval(&mut wb, "=TEXTBEFORE(\"a.b.c\",\".\",-1)"), t("a.b"));
    assert_eq!(eval(&mut wb, "=TEXTAFTER(\"a.b.c\",\".\")"), t("b.c"));
    assert_eq!(
        eval(&mut wb, "=COLUMNS(TEXTSPLIT(\"a,b,c\",\",\"))"),
        n(3.0)
    );
    assert_eq!(eval(&mut wb, "=VALUE(\"12.5\")"), n(12.5));
    assert_eq!(eval(&mut wb, "=LEN(\"çay\")"), n(3.0));
}

#[test]
fn dates() {
    let mut wb = Workbook::new();
    assert_eq!(eval(&mut wb, "=DATE(2024,1,15)"), n(45306.0));
    assert_eq!(eval(&mut wb, "=TODAY()"), n(45306.0));
    assert_eq!(eval(&mut wb, "=YEAR(\"2024-03-01\")"), n(2024.0));
    assert_eq!(eval(&mut wb, "=EOMONTH(DATE(2024,1,15),1)"), n(45351.0));
    assert_eq!(eval(&mut wb, "=EDATE(DATE(2024,1,31),1)"), n(45351.0));
    assert_eq!(eval(&mut wb, "=WEEKDAY(DATE(2024,1,15))"), n(2.0));
    assert_eq!(
        eval(&mut wb, "=NETWORKDAYS(DATE(2024,1,1),DATE(2024,1,31))"),
        n(23.0)
    );
    assert_eq!(eval(&mut wb, "=WORKDAY(DATE(2024,1,12),1)"), n(45306.0));
    assert_eq!(
        eval(&mut wb, "=DATEDIF(DATE(2020,5,10),DATE(2024,1,15),\"Y\")"),
        n(3.0)
    );
    assert_eq!(eval(&mut wb, "=ISOWEEKNUM(DATE(2024,1,15))"), n(3.0));
    assert_eq!(eval(&mut wb, "=HOUR(0.75)"), n(18.0));
}

#[test]
fn lookups_including_regex_mode() {
    let mut wb = data();
    assert_eq!(eval(&mut wb, "=XLOOKUP(\"North\",A2:A6,C2:C6)"), n(40.0));
    assert_eq!(
        eval(&mut wb, "=XLOOKUP(\"South\",A2:A6,C2:C6,\"none\")"),
        t("none")
    );
    assert_eq!(
        eval(&mut wb, "=XLOOKUP(\"West\",A2:A6,C2:C6,,0,-1)"),
        n(50.0)
    );
    assert_eq!(eval(&mut wb, "=XLOOKUP(35,C2:C6,B2:B6,,1)"), t("Apple"));
    assert_eq!(eval(&mut wb, "=XLOOKUP(35,C2:C6,B2:B6,,-1)"), t("Pear"));
    assert_eq!(eval(&mut wb, "=XLOOKUP(\"P*\",B2:B6,C2:C6,,2)"), n(20.0));
    // match_mode 3: regex
    assert_eq!(eval(&mut wb, "=XLOOKUP(\"^N.r\",A2:A6,C2:C6,,3)"), n(40.0));
    assert_eq!(eval(&mut wb, "=XLOOKUP(\"t$\",A2:A6,C2:C6,,3,-1)"), n(50.0));
    assert_eq!(eval(&mut wb, "=XMATCH(\"[aeiou]{2}\",B2:B6,3)"), n(2.0));
    assert_eq!(
        eval(&mut wb, "=XMATCH(\"(\",B2:B6,3)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(eval(&mut wb, "=XMATCH(30,C2:C6,0,2)"), n(3.0));
    assert_eq!(
        eval(&mut wb, "=COLUMNS(XLOOKUP(\"East\",A2:A6,A2:C6))"),
        n(3.0)
    );
    assert_eq!(eval(&mut wb, "=VLOOKUP(\"North\",A2:C6,3,FALSE)"), n(40.0));
    assert_eq!(eval(&mut wb, "=MATCH(25,C2:C6)"), n(2.0));
    assert_eq!(eval(&mut wb, "=INDEX(A1:C6,3,2)"), t("Pear"));
    assert_eq!(
        eval(&mut wb, "=SUM(INDEX(C2:C6,2):INDEX(C2:C6,4))"),
        n(90.0)
    );
    assert_eq!(eval(&mut wb, "=SUM(OFFSET(C1,1,0,2,1))"), n(30.0));
    assert_eq!(eval(&mut wb, "=INDIRECT(\"B\"&3)"), t("Pear"));
    assert_eq!(eval(&mut wb, "=ADDRESS(2,3,4)"), t("C2"));
}

#[test]
fn regex_functions() {
    let mut wb = Workbook::new();
    assert_eq!(
        eval(&mut wb, "=REGEXTEST(\"Prism-42\",\"\\d+\")"),
        Value::Bool(true)
    );
    assert_eq!(
        eval(&mut wb, "=REGEXTEST(\"ABC\",\"abc\",1)"),
        Value::Bool(true)
    );
    assert_eq!(
        eval(&mut wb, "=REGEXTEST(\"ABC\",\"abc\")"),
        Value::Bool(false)
    );
    assert_eq!(
        eval(&mut wb, "=REGEXEXTRACT(\"tel 555-1234\",\"\\d{3}-\\d{4}\")"),
        t("555-1234")
    );
    assert_eq!(
        eval(&mut wb, "=ROWS(REGEXEXTRACT(\"a1b22c333\",\"\\d+\",1))"),
        n(3.0)
    );
    assert_eq!(
        eval(
            &mut wb,
            "=INDEX(REGEXEXTRACT(\"John Smith\",\"(\\w+) (\\w+)\",2),2)"
        ),
        t("Smith")
    );
    assert_eq!(
        eval(
            &mut wb,
            "=REGEXREPLACE(\"John Smith\",\"(\\w+) (\\w+)\",\"$2, $1\")"
        ),
        t("Smith, John")
    );
    assert_eq!(
        eval(&mut wb, "=REGEXREPLACE(\"a1b2c3\",\"\\d\",\"#\",2)"),
        t("a1b#c3")
    );
    assert_eq!(
        eval(&mut wb, "=REGEXREPLACE(\"a1b2c3\",\"\\d\",\"\")"),
        t("abc")
    );
    assert_eq!(
        eval(&mut wb, "=REGEXEXTRACT(\"abc\",\"\\d\")"),
        Value::Error(ErrorKind::NA)
    );
}

#[test]
fn dynamic_arrays() {
    let mut wb = data();
    assert_eq!(eval(&mut wb, "=ROWS(FILTER(A2:C6,C2:C6>25))"), n(3.0));
    assert_eq!(
        eval(&mut wb, "=FILTER(A2:A6,C2:C6>100,\"none\")"),
        t("none")
    );
    assert_eq!(eval(&mut wb, "=INDEX(SORT(C2:C6,1,-1),1)"), n(50.0));
    assert_eq!(
        eval(&mut wb, "=INDEX(SORTBY(B2:B6,C2:C6,-1),1)"),
        t("Apple")
    );
    assert_eq!(eval(&mut wb, "=ROWS(UNIQUE(A2:A6))"), n(3.0));
    assert_eq!(eval(&mut wb, "=ROWS(UNIQUE(A2:A6,,TRUE))"), n(1.0));
    assert_eq!(eval(&mut wb, "=SUM(SEQUENCE(4,1,10,10))"), n(100.0));
    assert_eq!(eval(&mut wb, "=ROWS(VSTACK(A2:A3,A4:A6))"), n(5.0));
    assert_eq!(eval(&mut wb, "=COLUMNS(HSTACK(A2:A3,B2:C3))"), n(3.0));
    assert_eq!(eval(&mut wb, "=SUM(TAKE(C2:C6,-2))"), n(90.0));
    assert_eq!(eval(&mut wb, "=SUM(DROP(C2:C6,1))"), n(140.0));
    assert_eq!(
        eval(&mut wb, "=INDEX(CHOOSECOLS(A2:C6,3,1),1,2)"),
        t("East")
    );
    assert_eq!(eval(&mut wb, "=COLUMNS(TOROW(A2:B3))"), n(4.0));
    assert_eq!(eval(&mut wb, "=ROWS(WRAPROWS(SEQUENCE(7),3))"), n(3.0));
    assert_eq!(eval(&mut wb, "=SUM(C2:C6*{1;0;1;0;1})"), n(90.0));
    // spilled into the grid
    wb.set(0, "E1", "=SORT(UNIQUE(A2:A6))", NOW).unwrap();
    assert_eq!(wb.get(0, "E1"), t("East"));
    assert_eq!(wb.get(0, "E3"), t("West"));
}

#[test]
fn groupby_pivotby_percentof_trimrange() {
    let mut wb = data();
    wb.set(0, "E1", "=GROUPBY(A2:A6,C2:C6,SUM)", NOW).unwrap();
    // East 40, North 40, West 70, Total 150
    assert_eq!(wb.get(0, "E1"), t("East"));
    assert_eq!(wb.get(0, "F1"), n(40.0));
    assert_eq!(wb.get(0, "E3"), t("West"));
    assert_eq!(wb.get(0, "F3"), n(70.0));
    assert_eq!(wb.get(0, "E4"), t("Total"));
    assert_eq!(wb.get(0, "F4"), n(150.0));
    // headers, descending by value, no totals
    wb.set(0, "E1", "=GROUPBY(A1:A6,C1:C6,SUM,3,0,-2)", NOW)
        .unwrap();
    assert_eq!(wb.get(0, "E1"), t("Region"));
    assert_eq!(wb.get(0, "F1"), t("Sales"));
    assert_eq!(wb.get(0, "E2"), t("West"));
    assert_eq!(wb.get(0, "E5"), Value::Empty);
    // PERCENTOF as the aggregation
    wb.set(0, "E1", "=GROUPBY(A2:A6,C2:C6,PERCENTOF,,0)", NOW)
        .unwrap();
    assert_eq!(wb.get(0, "F3"), n(70.0 / 150.0));
    // filter_array
    wb.set(
        0,
        "E1",
        "=GROUPBY(A2:A6,C2:C6,COUNTA,,0,,B2:B6=\"Apple\")",
        NOW,
    )
    .unwrap();
    assert_eq!(wb.get(0, "E1"), t("East"));
    assert_eq!(wb.get(0, "E2"), t("North"));
    assert_eq!(wb.get(0, "F3"), n(1.0));
    // PIVOTBY: regions × products
    wb.set(0, "E1", "=PIVOTBY(A2:A6,B2:B6,C2:C6,SUM)", NOW)
        .unwrap();
    assert_eq!(wb.get(0, "F1"), t("Apple"));
    assert_eq!(wb.get(0, "G1"), t("Pear"));
    assert_eq!(wb.get(0, "H1"), t("Total"));
    assert_eq!(wb.get(0, "E2"), t("East"));
    assert_eq!(wb.get(0, "F2"), n(10.0));
    assert_eq!(wb.get(0, "G2"), n(30.0));
    assert_eq!(wb.get(0, "G3"), Value::Empty); // North has no Pear
    assert_eq!(wb.get(0, "H5"), n(150.0));
    assert_eq!(eval(&mut wb, "=PERCENTOF(C2,C2:C6)"), n(10.0 / 150.0));
    assert_eq!(eval(&mut wb, "=ROWS(TRIMRANGE(A1:C100))"), n(6.0));
    assert_eq!(
        eval(&mut wb, "=GROUPBY(A2:A6,C2:C6,LAMBDA(x,MAX(x)-MIN(x)),,0)"),
        t("East")
    );
}

#[test]
fn python_cells_are_async() {
    let mut wb = wb_with(&[("A1", "2"), ("B1", "=PY(\"xl(\"\"A1\"\") * 21\",0)")]);
    assert_eq!(wb.get(0, "B1"), Value::Error(ErrorKind::Busy));
    let req = wb.async_requests[0].clone();
    assert_eq!(req.inputs[0].0, "A1");
    assert_eq!(req.inputs[0].1, vec![vec![serde_json::json!(2.0)]]);
    let c = prism_sheet::CellRef::new(0, 1);
    wb.supply_async(&req.key, 0, c, n(42.0));
    wb.recalc(NOW);
    assert_eq!(wb.get(0, "B1"), n(42.0));
    // changing the input invalidates the cached answer
    wb.set(0, "A1", "3", NOW).unwrap();
    assert_eq!(wb.get(0, "B1"), Value::Error(ErrorKind::Busy));
}

#[test]
fn structured_references() {
    let mut wb = data();
    wb.sheets[0].tables.push(prism_sheet::workbook::TableDef {
        name: "Sales".into(),
        rect: prism_sheet::Rect::new(
            prism_sheet::CellRef::new(0, 0),
            prism_sheet::CellRef::new(5, 2),
        ),
        columns: vec!["Region".into(), "Product".into(), "Sales".into()],
        header_row: true,
        totals_row: false,
    });
    assert_eq!(eval(&mut wb, "=SUM(Sales[Sales])"), n(150.0));
    assert_eq!(eval(&mut wb, "=ROWS(Sales[#All])"), n(6.0));
    assert_eq!(eval(&mut wb, "=COUNTIF(Sales[Region],\"West\")"), n(2.0));
}

#[test]
fn large_parallel_recalc() {
    let mut wb = Workbook::new();
    for r in 0..2000u32 {
        wb.put_input(0, prism_sheet::CellRef::new(r, 0), &(r + 1).to_string())
            .unwrap();
        wb.put_input(
            0,
            prism_sheet::CellRef::new(r, 1),
            &format!("=A{}*2", r + 1),
        )
        .unwrap();
    }
    wb.put_input(0, prism_sheet::CellRef::new(0, 2), "=SUM(B:B)")
        .unwrap();
    let r = wb.recalc(NOW);
    assert_eq!(r.evaluated, 2001);
    assert_eq!(wb.get(0, "C1"), n(2000.0 * 2001.0));
    let r = wb.set(0, "A5", "0", NOW).unwrap();
    assert_eq!(r.evaluated, 2);
    assert_eq!(wb.get(0, "C1"), n(2000.0 * 2001.0 - 10.0));
}

#[test]
fn shared_formula_trees_from_model() {
    use prism_sheet::model::{CellModel, CellValueModel, SheetModel, WorkbookModel};
    let mut cells = vec![];
    for r in 0..50u32 {
        cells.push(CellModel {
            r,
            c: 0,
            f: None,
            v: Some(CellValueModel::Number(r as f64)),
            s: 0,
        });
        cells.push(CellModel {
            r,
            c: 1,
            f: Some(format!("A{}*2+$A$1+SUM($A$1:A{})", r + 1, r + 1)),
            v: None,
            s: 0,
        });
        cells.push(CellModel {
            r,
            c: 2,
            f: Some(format!("LOG10(10)+Data!A{}", r + 1)),
            v: None,
            s: 0,
        });
        cells.push(CellModel {
            r,
            c: 3,
            f: Some("SUM(A:A)".into()),
            v: None,
            s: 0,
        });
    }
    let data = SheetModel {
        name: "Data".into(),
        cells: (0..50u32)
            .map(|r| CellModel {
                r,
                c: 0,
                f: None,
                v: Some(CellValueModel::Number(100.0)),
                s: 0,
            })
            .collect(),
        ..Default::default()
    };
    let m = WorkbookModel {
        sheets: vec![
            SheetModel {
                name: "S".into(),
                cells,
                ..Default::default()
            },
            data,
        ],
        num_formats: vec!["General".into()],
        ..Default::default()
    };
    let mut wb = Workbook::from_model(&m);
    wb.recalc(NOW);
    assert_eq!(wb.get(0, "B10"), n(9.0 * 2.0 + 45.0));
    assert_eq!(wb.get(0, "C7"), n(101.0));
    assert_eq!(wb.get(0, "D50"), n(1225.0));
    assert_eq!(
        wb.input_text(0, prism_sheet::CellRef::new(9, 1)),
        "=A10*2+$A$1+SUM($A$1:A10)"
    );
    // dependencies follow the shifted references
    wb.set(0, "A10", "100", NOW).unwrap();
    assert_eq!(wb.get(0, "B10"), n(200.0 + 45.0 - 9.0 + 100.0));
    assert_eq!(wb.get(0, "B9"), n(8.0 * 2.0 + 36.0));
    assert_eq!(m.sheets[0].cells.len(), wb.to_model().sheets[0].cells.len());
    assert_eq!(
        wb.to_model().sheets[0].cells[4 * 20 + 1].f.as_deref(),
        Some("A21*2+$A$1+SUM($A$1:A21)")
    );
}

#[test]
fn relative_keys() {
    use prism_sheet::{parser::relative_key, CellRef};
    let k = |t: &str, r, c| relative_key(t, CellRef::new(r, c));
    assert_eq!(k("A1*2", 0, 1), k("A2*2", 1, 1));
    assert_ne!(k("A1*2", 0, 1), k("A1*2", 1, 1));
    assert_eq!(k("$A$1+A1", 0, 1), k("$A$1+A2", 1, 1));
    assert_eq!(k("LOG10(B1)", 0, 0), k("LOG10(B2)", 1, 0));
    assert_eq!(k("\"A1\"&B1", 0, 0), k("\"A1\"&B2", 1, 0));
    assert_ne!(k("\"A1\"&B1", 0, 0), k("\"A2\"&B2", 1, 0));
    assert_eq!(k("Sheet1!A1", 0, 0), k("Sheet1!A2", 1, 0));
    assert_eq!(k("SUM(A:A)", 0, 0), None);
    assert_eq!(k("SUM(1:3)", 0, 0), None);
}

#[test]
fn financial_functions_match_excel() {
    let mut wb = Workbook::new();
    let close = |v: Value, x: f64, tol: f64| match v {
        Value::Number(n) => assert!((n - x).abs() <= tol, "{n} vs {x}"),
        other => panic!("{other:?} vs {x}"),
    };
    close(
        eval(&mut wb, "=PMT(0.05/12,360,200000)"),
        -1073.643246,
        1e-5,
    );
    close(
        eval(&mut wb, "=FV(0.06/12,10,-200,-500,1)"),
        2581.403374,
        1e-5,
    );
    close(eval(&mut wb, "=PV(0.08/12,240,500)"), -59777.14585, 1e-4);
    close(
        eval(&mut wb, "=NPER(0.12/12,-100,-1000,10000,1)"),
        59.6738657,
        1e-6,
    );
    close(eval(&mut wb, "=RATE(48,-200,8000)"), 0.0077014724, 1e-9);
    close(
        eval(&mut wb, "=NPV(0.1,-10000,3000,4200,6800)"),
        1188.443412,
        1e-5,
    );
    close(
        eval(&mut wb, "=IRR({-70000,12000,15000,18000,21000,26000})"),
        0.086630948,
        1e-8,
    );
    close(
        eval(
            &mut wb,
            "=XNPV(0.09,{-10000,2750,4250,3250,2750},{39448,39508,39751,39859,39904})",
        ),
        2086.647602,
        1e-4,
    );
    close(
        eval(
            &mut wb,
            "=XIRR({-10000,2750,4250,3250,2750},{39448,39508,39751,39859,39904})",
        ),
        0.373362535,
        1e-7,
    );
    close(eval(&mut wb, "=IPMT(0.1/12,1,36,8000)"), -66.66666667, 1e-6);
    close(eval(&mut wb, "=PPMT(0.1/12,1,24,2000)"), -75.62318601, 1e-6);
    close(
        eval(&mut wb, "=CUMIPMT(0.09/12,360,125000,13,24,0)"),
        -11135.23213,
        1e-4,
    );
    close(
        eval(
            &mut wb,
            "=MIRR({-120000,39000,30000,21000,37000,46000},0.1,0.12)",
        ),
        0.126094,
        1e-6,
    );
    close(eval(&mut wb, "=SLN(30000,7500,10)"), 2250.0, 1e-9);
    close(eval(&mut wb, "=SYD(30000,7500,10,1)"), 4090.909091, 1e-5);
    close(eval(&mut wb, "=DDB(2400,300,10,1)"), 480.0, 1e-9);
    close(
        eval(&mut wb, "=DB(1000000,100000,6,1,7)"),
        186083.3333,
        1e-3,
    );
    close(eval(&mut wb, "=EFFECT(0.0525,4)"), 0.053542667, 1e-8);
    close(eval(&mut wb, "=NOMINAL(0.053543,4)"), 0.05250032, 1e-7);
}

#[test]
fn trim_range_operators() {
    let mut wb = wb_with(&[("B3", "1"), ("B4", "2"), ("C4", "3")]);
    assert_eq!(eval(&mut wb, "=ROWS(B1:.B100)"), n(4.0)); // trailing blanks trimmed
    assert_eq!(eval(&mut wb, "=ROWS(B1.:B100)"), n(98.0)); // leading blanks trimmed
    assert_eq!(eval(&mut wb, "=ROWS(B1.:.B100)"), n(2.0));
    assert_eq!(eval(&mut wb, "=SUM(A:.C)"), n(6.0));
    assert_eq!(eval(&mut wb, "=\"a:.b\""), t("a:.b"));
    assert_eq!(
        wb.input_text(0, prism_sheet::CellRef::new(999, 701)),
        "=\"a:.b\""
    );
    wb.set(0, "E1", "=ROWS(B1.:.B100)", NOW).unwrap();
    assert_eq!(
        wb.input_text(0, prism_sheet::CellRef::new(0, 4)),
        "=ROWS(B1.:.B100)"
    );
    let e = prism_sheet::parse("ROWS(B1:.B100)", prism_sheet::CellRef::new(0, 0)).unwrap();
    let file = prism_sheet::parser::to_formula(
        &e,
        &prism_sheet::parser::WriteOptions {
            file_prefixes: true,
            ..Default::default()
        },
    );
    assert_eq!(file, "ROWS(_xlfn._TRO_TRAILING(B1:B100))");
    assert_eq!(
        prism_sheet::parser::to_formula(
            &prism_sheet::parse(&file, prism_sheet::CellRef::new(0, 0)).unwrap(),
            &Default::default()
        ),
        "ROWS(B1:.B100)"
    );
}

#[test]
fn pcre_features_in_regex_functions() {
    let mut wb = Workbook::new();
    // look-ahead / look-behind
    assert_eq!(
        eval(
            &mut wb,
            "=REGEXEXTRACT(\"price: $42.50\",\"(?<=\\$)\\d+(?=\\.)\")"
        ),
        t("42")
    );
    // back-reference
    assert_eq!(
        eval(&mut wb, "=REGEXTEST(\"hello\",\"(l)\\1\")"),
        Value::Bool(true)
    );
    assert_eq!(
        eval(&mut wb, "=REGEXREPLACE(\"aa bb cd\",\"(\\w)\\1\",\"<$1>\")"),
        t("<a> <b> cd")
    );
    // negative look-ahead in XLOOKUP regex mode
    let mut wb = wb_with(&[("A1", "apple"), ("A2", "apricot"), ("B1", "1"), ("B2", "2")]);
    assert_eq!(
        eval(&mut wb, "=XLOOKUP(\"^ap(?!p)\",A1:A2,B1:B2,,3)"),
        n(2.0)
    );
}
