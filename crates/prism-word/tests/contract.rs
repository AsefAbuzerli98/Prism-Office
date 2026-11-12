//! The TS editor's JSON (see src/__tests__/word-contract.test.ts) must
//! deserialise into the Rust model and export to a valid DOCX.

use prism_openxml::{templates, DocumentKind, Package};
use prism_word::*;

#[test]
fn ts_document_json_deserialises_and_exports() {
    let json = include_str!("fixtures/ts-document.json");
    let doc: WordDocument = serde_json::from_str(json).expect("TS JSON must match the Rust model");
    assert_eq!(doc.blocks.len(), 5);
    match &doc.blocks[3] {
        Block::Table(t) => {
            assert!(t.rows[0].header && t.borders);
            assert_eq!(t.rows[0].cells[0].shading.as_deref(), Some("DDEEFF"));
        }
        b => panic!("{b:?}"),
    }
    match &doc.blocks[4] {
        Block::Paragraph(p) => {
            assert!(p.format.bidi);
            assert_eq!(p.format.tabs[0].leader, Some('.'));
            assert!(p.inlines.iter().any(|i| matches!(i, Inline::Shape { .. })));
            assert!(p
                .inlines
                .iter()
                .any(|i| matches!(i, Inline::Note { id: 1, .. })));
        }
        b => panic!("{b:?}"),
    }
    assert_eq!(
        (doc.notes.len(), doc.comments.len(), doc.sources.len()),
        (1, 1, 1)
    );
    assert!(doc.settings.track_revisions);
    assert!(doc.section.columns.as_ref().unwrap().separator);
    match &doc.blocks[1] {
        Block::Paragraph(p) => {
            assert!(p.inlines.iter().any(|i| matches!(i, Inline::Tab { .. })));
            assert!(p.inlines.iter().any(|i| matches!(i, Inline::Text { marks, .. } if marks.bold == Some(true) && marks.size == Some(28))));
            assert!(p.inlines.iter().any(|i| matches!(i, Inline::Text { marks, .. } if marks.link.as_deref() == Some("https://example.com"))));
        }
        b => panic!("{b:?}"),
    }
    match &doc.blocks[2] {
        Block::Paragraph(p) => {
            assert_eq!(
                p.list,
                Some(ListRef {
                    num_id: BULLET_NUM_ID,
                    level: 1
                })
            );
            assert_eq!(p.format.indent_first, Some(-360));
        }
        b => panic!("{b:?}"),
    }

    let mut pkg = templates::blank(DocumentKind::Document);
    write_package(&mut pkg, &doc).unwrap();
    assert!(pkg.validate().is_empty(), "{:#?}", pkg.validate());
    let back = read_package(&Package::from_bytes(&pkg.to_bytes().unwrap()).unwrap()).unwrap();
    let mut back = back;
    // New tables gain a generated tblPr on export; compare structure only.
    // and generated run XML is captured on re-read.
    for b in &mut back.blocks {
        match b {
            Block::Table(t) => t.props = None,
            Block::Paragraph(p) => {
                for i in &mut p.inlines {
                    if let Inline::Note { xml, .. }
                    | Inline::Shape { xml, .. }
                    | Inline::Image { xml, .. } = i
                    {
                        xml.clear();
                    }
                }
            }
            _ => {}
        }
    }
    for (i, (a, b)) in back.blocks.iter().zip(&doc.blocks).enumerate() {
        assert_eq!(a, b, "block {i} differs");
    }
    assert_eq!(back.blocks.len(), doc.blocks.len());
    assert_eq!(back.notes, doc.notes);
    assert_eq!(back.comments, doc.comments);
    assert_eq!(back.sources, doc.sources);
    assert!(back.section.landscape);
}
