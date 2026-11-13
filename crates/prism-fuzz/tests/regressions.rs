//! Inputs that once made a target panic (found by the fuzzer) must stay fixed.
//! File names are `<target>-<hash>.bin`.

#[test]
fn fuzz_regressions_do_not_panic() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("regressions");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let target = name.split('-').next().unwrap();
        let f = prism_fuzz::TARGETS
            .iter()
            .find(|(t, _)| *t == target)
            .unwrap_or_else(|| panic!("unknown target in {name}"))
            .1;
        let data = std::fs::read(&p).unwrap();
        assert!(
            std::panic::catch_unwind(|| f(&data)).is_ok(),
            "{name} panics"
        );
        n += 1;
    }
    assert!(n > 0);
}

#[test]
fn seeds_smoke() {
    // every target survives empty and tiny inputs
    for (name, f) in prism_fuzz::TARGETS {
        for data in [&b""[..], b"\0", b"PK\x03\x04", b"<a:p>", b"=", b"\xff\xfe"] {
            assert!(
                std::panic::catch_unwind(|| f(data)).is_ok(),
                "{name} panics on {data:?}"
            );
        }
    }
}

/// Deep nesting must be rejected or bounded, never overflow the stack
/// (test threads have a 2 MB stack, smaller than the main thread's).
#[test]
fn deep_nesting_is_bounded() {
    let n = 20_000;
    // formulas: parentheses, unary chains, nested calls
    let base = prism_sheet::addr::CellRef { row: 0, col: 0 };
    assert!(
        prism_sheet::parser::parse(&format!("{}1{}", "(".repeat(n), ")".repeat(n)), base).is_err()
    );
    assert!(prism_sheet::parser::parse(&format!("{}1", "-".repeat(4000)), base).is_err());
    let mut e = prism_sheet::api::Engine::new(&Default::default(), 1.0);
    e.edit(
        &[prism_sheet::api::Edit {
            sheet: 0,
            r: 0,
            c: 0,
            text: format!("={}1{}", "ABS(".repeat(3000), ")".repeat(3000)),
        }],
        1.0,
    );
}

#[test]
fn deep_groups_are_bounded() {
    let n = 20_000;
    // DrawingML group nesting
    let open = "<p:grpSp><p:nvGrpSpPr><p:cNvPr id=\"2\" name=\"g\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>";
    prism_fuzz::drawingml(
        format!(
            "<p:spTree>{}{}</p:spTree>",
            open.repeat(n),
            "</p:grpSp>".repeat(n)
        )
        .as_bytes(),
    );
}

#[test]
fn deep_tables_are_bounded() {
    let n = 20_000;
    // Word nested tables
    let mut p = prism_openxml::templates::blank(prism_openxml::DocumentKind::Document);
    let xml = format!(
        r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{}<w:p/>{}</w:body></w:document>"#,
        "<w:tbl><w:tr><w:tc>".repeat(n),
        "<w:p/></w:tc></w:tr></w:tbl>".repeat(n)
    );
    p.set_part(
        prism_openxml::PartName::new("/word/document.xml").unwrap(),
        xml.into_bytes(),
        None,
    );
    prism_fuzz::docx(&p.to_bytes().unwrap());
}
