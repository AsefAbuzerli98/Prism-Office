//! Fuzz targets shared by the cargo-fuzz harness (`fuzz/`, nightly) and the
//! stable mutation runner (`cargo run -p prism-fuzz --release -- <target>`).
//! A target must never panic, whatever the input; errors are fine.

use prism_openxml::{Limits, Package};

/// Tight limits so decompression bombs fail fast instead of exhausting memory.
pub fn limits() -> Limits {
    Limits {
        max_parts: 2_000,
        max_part_size: 32 * 1024 * 1024,
        ..Limits::default()
    }
}

fn package(data: &[u8]) -> Option<Package> {
    Package::read(std::io::Cursor::new(data), limits()).ok()
}

/// OPC container: open, list parts, validate, re-serialise.
pub fn openxml(data: &[u8]) {
    if let Some(p) = package(data) {
        let _ = p.kind();
        let _ = p.validate();
        let _ = p.to_bytes();
    }
}

/// DOCX: read the word model, write it back, read again.
pub fn docx(data: &[u8]) {
    let Some(mut p) = package(data) else { return };
    if let Ok(doc) = prism_word::read_package(&p) {
        if prism_word::write_package(&mut p, &doc).is_ok() {
            let _ = prism_word::read_package(&p);
        }
    }
}

/// XLSX: read, calculate, write back.
pub fn xlsx(data: &[u8]) {
    let Some(p) = package(data) else { return };
    if let Ok((model, src)) = prism_sheet::xlsx::read_package(p) {
        let mut engine = prism_sheet::api::Engine::new(&model, 45000.0);
        let _ = engine.recalc(45000.0);
        let _ = prism_sheet::xlsx::write_package(&engine.model(), Some(&src), 45000.0);
    }
}

/// PPTX: read the presentation model and write it back.
pub fn pptx(data: &[u8]) {
    let Some(p) = package(data) else { return };
    if let Ok((model, src)) = prism_slide::read_package(p) {
        if let Ok(out) = prism_slide::write_package(&model, &src) {
            let _ = prism_slide::read_package(out);
        }
    }
}

/// Formula text: parse, print, evaluate in a small sheet.
pub fn formula(data: &[u8]) {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    if src.len() > 4096 {
        return;
    }
    let base = prism_sheet::addr::CellRef { row: 5, col: 5 };
    if let Ok(e) = prism_sheet::parser::parse(src, base) {
        let _ = prism_sheet::parser::to_formula(&e, &Default::default());
    }
    let _ = prism_sheet::parser::parse_mode(src, base, true);
    let model = prism_sheet::model::WorkbookModel::default();
    let mut engine = prism_sheet::api::Engine::new(&model, 45000.0);
    let edits = [
        prism_sheet::api::Edit {
            sheet: 0,
            r: 0,
            c: 0,
            text: "3".into(),
        },
        prism_sheet::api::Edit {
            sheet: 0,
            r: 1,
            c: 0,
            text: "hello".into(),
        },
        prism_sheet::api::Edit {
            sheet: 0,
            r: 2,
            c: 1,
            text: format!("={src}"),
        },
        prism_sheet::api::Edit {
            sheet: 0,
            r: 2,
            c: 2,
            text: src.to_string(),
        },
    ];
    let _ = engine.edit(&edits, 45000.0);
}

/// Number formats against a few values.
pub fn numfmt(data: &[u8]) {
    let Ok(code) = std::str::from_utf8(data) else {
        return;
    };
    if code.len() > 512 {
        return;
    }
    use prism_sheet::value::Value;
    for v in [
        Value::Number(0.0),
        Value::Number(-1234.5678),
        Value::Number(45123.75),
        Value::Number(1e300),
        Value::Bool(true),
    ] {
        let _ = prism_sheet::numfmt::format_value(&v, code);
    }
}

/// DrawingML XML fragments: DOM, theme, custom geometry, text bodies.
pub fn drawingml(data: &[u8]) {
    let Ok(xml) = std::str::from_utf8(data) else {
        return;
    };
    let _ = prism_drawingml::color::Theme::parse(xml);
    if let Some(el) = prism_drawingml::dom::El::parse(xml) {
        let _ = el.to_xml();
        let _ = prism_drawingml::fill::Fill::from_el(&el);
        let _ = prism_drawingml::fill::Line::from_el(&el);
        let _ = prism_drawingml::text::TextBody::from_el(&el);
        let (nodes, originals) = prism_drawingml::shape::read_tree(&el);
        let _ = prism_drawingml::shape::write_tree("p", &nodes, &originals);
        for n in &nodes {
            if let prism_drawingml::shape::Node::Shape(s) = n {
                if let Some(prism_drawingml::shape::Geometry::Custom { def }) = &s.geom {
                    let _ = def.evaluate(914400.0, 457200.0, &[]);
                }
            }
        }
    }
}

/// Preset geometry with arbitrary adjust values and sizes.
pub fn geometry(data: &[u8]) {
    if data.len() < 9 {
        return;
    }
    let names = prism_drawingml::geom::preset_names();
    let name = names[data[0] as usize % names.len()];
    let f = |i: usize| {
        f64::from(i32::from_le_bytes([
            data[i],
            data[i + 1],
            data[i + 2],
            data[i + 3],
        ]))
    };
    let (w, h) = (f(1), f(5));
    let adj: Vec<(String, f64)> = data[9..]
        .chunks(4)
        .take(8)
        .enumerate()
        .filter(|(_, c)| c.len() == 4)
        .map(|(i, c)| {
            (
                format!(
                    "adj{}",
                    if i == 0 {
                        String::new()
                    } else {
                        (i + 1).to_string()
                    }
                ),
                f64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])),
            )
        })
        .collect();
    let _ = prism_drawingml::geom::evaluate_preset(name, w, h, &adj);
}

pub type Target = fn(&[u8]);

pub const TARGETS: &[(&str, Target)] = &[
    ("openxml", openxml),
    ("docx", docx),
    ("xlsx", xlsx),
    ("pptx", pptx),
    ("formula", formula),
    ("numfmt", numfmt),
    ("drawingml", drawingml),
    ("geometry", geometry),
];
