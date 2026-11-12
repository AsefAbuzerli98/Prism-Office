//! The TS editor's JSON (see src/__tests__/word-contract.test.ts) must
//! deserialise into the Rust model and export to a valid DOCX.

use prism_openxml::{templates, DocumentKind, Package};
use prism_word::*;

#[test]
fn ts_document_json_deserialises_and_exports() {
    let json = include_str!("fixtures/ts-document.json");
    let doc: WordDocument = serde_json::from_str(json).expect("TS JSON must match the Rust model");
    assert_eq!(doc.blocks.len(), 4);
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
    // Word requires a paragraph after a trailing table; the writer adds one.
    assert_eq!(back.blocks[..doc.blocks.len()], doc.blocks[..]);
    assert_eq!(
        back.blocks.last(),
        Some(&Block::Paragraph(Paragraph::default()))
    );
    assert!(back.section.landscape);
}
