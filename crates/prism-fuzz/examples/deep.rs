//! Deep-nesting probes (stack overflow is an abort, invisible to catch_unwind).
fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    let n: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20000);
    match which.as_str() {
        "formula" => prism_fuzz::formula(format!("{}1{}", "(".repeat(n), ")".repeat(n)).as_bytes()),
        "formula-neg" => prism_fuzz::formula(format!("{}1", "-".repeat(n)).as_bytes()),
        "parse" => {
            let src = format!("{}1{}", "(".repeat(n), ")".repeat(n));
            let r = prism_sheet::parser::parse(&src, prism_sheet::addr::CellRef { row: 0, col: 0 });
            println!("parsed: {}", r.is_ok());
        }
        "eval" => {
            let src = format!("={}1{}", "(".repeat(n), ")".repeat(n));
            let mut e = prism_sheet::api::Engine::new(&Default::default(), 1.0);
            e.edit(
                &[prism_sheet::api::Edit {
                    sheet: 0,
                    r: 0,
                    c: 0,
                    text: src,
                }],
                1.0,
            );
        }
        "fn" => {
            let src = format!("={}1{}", "ABS(".repeat(n), ")".repeat(n));
            let mut e = prism_sheet::api::Engine::new(&Default::default(), 1.0);
            e.edit(
                &[prism_sheet::api::Edit {
                    sheet: 0,
                    r: 0,
                    c: 0,
                    text: src,
                }],
                1.0,
            );
        }
        "chain" => {
            let src = format!("={}", vec!["1"; n].join("+"));
            let mut e = prism_sheet::api::Engine::new(&Default::default(), 1.0);
            e.edit(
                &[prism_sheet::api::Edit {
                    sheet: 0,
                    r: 0,
                    c: 0,
                    text: src,
                }],
                1.0,
            );
        }
        "lambda" => {
            // self-recursive LAMBDA through a defined name
            let mut e = prism_sheet::api::Engine::new(&Default::default(), 1.0);
            e.edit(
                &[prism_sheet::api::Edit {
                    sheet: 0,
                    r: 0,
                    c: 0,
                    text: "=LET(f,LAMBDA(g,x,g(g,x+1)),f(f,1))".into(),
                }],
                1.0,
            );
        }
        "docx" | "sdt" => {
            let mut p = prism_openxml::templates::blank(prism_openxml::DocumentKind::Document);
            let name = prism_openxml::PartName::new("/word/document.xml").unwrap();
            let (open, close) = if which == "docx" {
                ("<w:tbl><w:tr><w:tc>", "<w:p/></w:tc></w:tr></w:tbl>")
            } else {
                ("<w:sdt><w:sdtContent>", "</w:sdtContent></w:sdt>")
            };
            let xml = format!(
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{}<w:p/>{}</w:body></w:document>"#,
                open.repeat(n),
                close.repeat(n)
            );
            p.set_part(name, xml.into_bytes(), None);
            let bytes = p.to_bytes().unwrap();
            prism_fuzz::docx(&bytes);
        }
        "xml" => {
            prism_fuzz::drawingml(format!("{}{}", "<a:p>".repeat(n), "</a:p>".repeat(n)).as_bytes())
        }
        "grp" => {
            let open = "<p:grpSp><p:nvGrpSpPr><p:cNvPr id=\"2\" name=\"g\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>";
            prism_fuzz::drawingml(
                format!(
                    "<p:spTree>{}{}</p:spTree>",
                    open.repeat(n),
                    "</p:grpSp>".repeat(n)
                )
                .as_bytes(),
            )
        }
        _ => eprintln!("formula|formula-neg|xml|grp"),
    }
    println!("ok {which} {n}");
}
