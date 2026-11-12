use prism_openxml::{
    rel_types, templates, DocumentKind, Package, PartName, Relationships, TargetMode,
};
use prism_word::*;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// A document exercising modelled and unmodelled features.
fn fixture() -> Package {
    let mut pkg = templates::blank(DocumentKind::Document);
    let main = PartName::new("/word/document.xml").unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><w:background w:color="FFFFEE"/><w:body>
<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:shd w:val="clear" w:color="auto" w:fill="EEEEEE"/><w:jc w:val="center"/></w:pPr><w:r><w:t>Quarterly &amp; Annual</w:t></w:r></w:p>
<w:p w14:paraId="1234ABCD"><w:pPr><w:spacing w:before="120" w:after="240"/><w:ind w:left="720" w:hanging="360"/></w:pPr>
  <w:r><w:rPr><w:b/><w:bCs/><w:lang w:val="az-Latn-AZ"/></w:rPr><w:t xml:space="preserve">Bold </w:t></w:r>
  <w:r><w:rPr><w:i/><w:color w:val="FF0000"/><w:sz w:val="28"/></w:rPr><w:t>red</w:t></w:r>
  <w:proofErr w:type="spellStart"/>
  <w:r><w:t xml:space="preserve"> plain</w:t><w:tab/><w:t>after tab</w:t><w:br/><w:t>line2</w:t></w:r>
  <w:bookmarkStart w:id="0" w:name="intro"/><w:bookmarkEnd w:id="0"/>
  <w:hyperlink r:id="rIdLink1"><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>site</w:t></w:r></w:hyperlink>
  <w:hyperlink w:anchor="intro"><w:r><w:t>jump</w:t></w:r></w:hyperlink>
  <w:r><w:drawing><wp:inline><a:graphic/></wp:inline></w:drawing></w:r>
</w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="5"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:br w:type="page"/></w:r></w:p>
<w:sectPr><w:headerReference w:type="default" r:id="rIdH"/><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="1000" w:right="1100" w:bottom="1200" w:left="1300" w:header="500" w:footer="600" w:gutter="0"/><w:cols w:space="708"/></w:sectPr>
</w:body></w:document>"#
    );
    pkg.set_part(main.clone(), xml.into_bytes(), None);
    let mut rels = pkg.relationships(Some(&main)).unwrap();
    rels.add(
        rel_types::HYPERLINK,
        "https://example.com/a?b=1&c=2",
        TargetMode::External,
    );
    // rename the generated id to the one referenced in the XML
    let xml = rels.to_xml().replace(r#"Id="rId2""#, r#"Id="rIdLink1""#);
    let rels = Relationships::parse("x", xml.as_bytes()).unwrap();
    pkg.set_relationships(Some(&main), &rels);
    pkg
}

fn para(doc: &WordDocument, i: usize) -> &Paragraph {
    match &doc.blocks[i] {
        Block::Paragraph(p) => p,
        b => panic!("block {i} is {b:?}"),
    }
}

#[test]
fn reads_modelled_and_preserves_unmodelled_content() {
    let doc = read_package(&fixture()).unwrap();
    assert_eq!(doc.blocks.len(), 5);

    let h = para(&doc, 0);
    assert_eq!(h.style.as_deref(), Some("Heading1"));
    assert_eq!(h.format.align, Some(Align::Center));
    assert_eq!(h.text(), "Quarterly & Annual");
    assert_eq!(h.extra.len(), 1, "w:shd preserved");

    let p = para(&doc, 1);
    assert_eq!(p.format.space_before, Some(120));
    assert_eq!(p.format.indent_first, Some(-360));
    match &p.inlines[0] {
        Inline::Text { text, marks } => {
            assert_eq!(text, "Bold ");
            assert_eq!(marks.bold, Some(true));
            assert_eq!(marks.extra.len(), 2, "bCs + lang preserved");
        }
        other => panic!("{other:?}"),
    }
    match &p.inlines[1] {
        Inline::Text { marks, .. } => {
            assert_eq!(
                (marks.italic, marks.size, marks.color.as_deref()),
                (Some(true), Some(28), Some("FF0000"))
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(p.text(), "Bold red plain\tafter tab\nline2sitejump\u{FFFC}");
    let links: Vec<_> = p
        .inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Text { marks, text } => {
                marks.link.as_ref().map(|l| (text.as_str(), l.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        links,
        vec![
            ("site", "https://example.com/a?b=1&c=2"),
            ("jump", "#intro")
        ]
    );
    assert!(p
        .inlines
        .iter()
        .any(|i| matches!(i, Inline::Opaque { label, .. } if label == "Image")));
    assert!(p.inlines.iter().any(|i| matches!(i, Inline::Opaque { label, xml } if label.is_empty() && xml.contains("bookmarkStart"))));

    assert_eq!(
        para(&doc, 2).list,
        Some(ListRef {
            num_id: 5,
            level: 1
        })
    );
    assert!(
        matches!(&doc.blocks[3], Block::Opaque { label, xml } if label == "Table" && xml.contains("<w:t>cell</w:t>"))
    );
    assert!(matches!(
        para(&doc, 4).inlines[..],
        [Inline::Break {
            kind: BreakKind::Page
        }]
    ));

    let s = &doc.section;
    assert!(s.landscape);
    assert_eq!(
        (s.page_width, s.margin_left, s.margin_header),
        (16838, 1300, 500)
    );
    assert_eq!(s.extra.len(), 2, "headerReference + cols preserved");

    // styles: template defines Normal/Heading1; builtins supply the rest
    let h1 = doc.styles.iter().find(|s| s.id == "Heading1").unwrap();
    assert_eq!(h1.run.bold, Some(true));
    assert!(doc.styles.iter().any(|s| s.id == "Heading2"));
    assert!(doc.numbering.iter().any(|n| n.num_id == BULLET_NUM_ID));
}

#[test]
fn export_then_import_is_stable() {
    let mut pkg = fixture();
    let doc = read_package(&pkg).unwrap();
    write_package(&mut pkg, &doc).unwrap();
    let bytes = pkg.to_bytes().unwrap();
    let pkg2 = Package::from_bytes(&bytes).unwrap();
    let doc2 = read_package(&pkg2).unwrap();
    assert_eq!(doc.blocks, doc2.blocks);
    assert_eq!(doc.section, doc2.section);

    let xml = String::from_utf8(pkg2.part_str("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(
        xml.contains(r#"<w:background w:color="FFFFEE"/>"#),
        "pre-body content kept"
    );
    assert!(xml.contains("xmlns:w14="), "root namespaces kept");
    assert!(xml.contains(r#"<w:headerReference w:type="default" r:id="rIdH"/>"#));
    assert!(
        xml.contains(r#"<w:hyperlink r:id="rIdLink1">"#),
        "existing link relationship reused"
    );
    // schema order in pPr: pStyle < shd < jc ; sectPr: headerReference < pgSz < pgMar < cols
    let order = |hay: &str, needles: &[&str]| {
        let pos: Vec<usize> = needles
            .iter()
            .map(|n| hay.find(n).unwrap_or_else(|| panic!("{n} missing")))
            .collect();
        assert!(
            pos.windows(2).all(|w| w[0] < w[1]),
            "{needles:?} out of order"
        );
    };
    order(&xml, &["<w:pStyle", "<w:shd", "<w:jc"]);
    order(
        &xml,
        &["<w:headerReference", "<w:pgSz", "<w:pgMar", "<w:cols"],
    );
    order(&xml, &["<w:b/>", "<w:bCs/>", "<w:lang"]);

    let issues = pkg2.validate();
    assert!(issues.is_empty(), "{issues:#?}");
    // idempotent: second export is byte-identical
    let mut pkg3 = pkg2.clone();
    write_package(&mut pkg3, &doc2).unwrap();
    assert_eq!(
        pkg3.part_str("word/document.xml"),
        pkg2.part_str("word/document.xml")
    );
}

#[test]
fn editing_adds_lists_styles_and_links() {
    let mut pkg = templates::blank(DocumentKind::Document);
    let mut doc = read_package(&pkg).unwrap();
    let mut heading = Paragraph::plain("Title here");
    heading.style = Some("Heading2".into());
    let mut item = Paragraph::plain("first");
    item.list = Some(ListRef {
        num_id: BULLET_NUM_ID,
        level: 0,
    });
    let mut num = Paragraph::plain("one");
    num.list = Some(ListRef {
        num_id: DECIMAL_NUM_ID,
        level: 0,
    });
    let link = Paragraph {
        inlines: vec![Inline::Text {
            text: "docs".into(),
            marks: Marks {
                link: Some("https://prism.example/docs".into()),
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    doc.blocks = vec![
        Block::Paragraph(heading),
        Block::Paragraph(item),
        Block::Paragraph(num),
        Block::Paragraph(link),
    ];
    write_package(&mut pkg, &doc).unwrap();

    let numbering = String::from_utf8(
        pkg.part_str("word/numbering.xml")
            .expect("numbering part created")
            .to_vec(),
    )
    .unwrap();
    let a = numbering.find("<w:abstractNum ").unwrap();
    let n = numbering.find("<w:num ").unwrap();
    assert!(a < n, "abstractNum must precede num");
    assert!(numbering.contains(r#"w:numId="9001""#) && numbering.contains(r#"w:numId="9002""#));

    let styles = String::from_utf8(pkg.part_str("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"w:styleId="Heading2""#));
    assert!(
        !styles.contains(r#"w:styleId="Quote""#),
        "only referenced styles are added"
    );

    let back = read_package(&Package::from_bytes(&pkg.to_bytes().unwrap()).unwrap()).unwrap();
    assert_eq!(back.blocks, doc.blocks);
    assert!(pkg.validate().is_empty(), "{:#?}", pkg.validate());

    // second save does not duplicate definitions
    write_package(&mut pkg, &back).unwrap();
    let numbering2 =
        String::from_utf8(pkg.part_str("word/numbering.xml").unwrap().to_vec()).unwrap();
    assert_eq!(numbering2.matches("<w:num ").count(), 2);
}

#[test]
fn blank_document_roundtrip_and_json_shape() {
    let mut pkg = templates::blank(DocumentKind::Document);
    let doc = read_package(&pkg).unwrap();
    assert_eq!(doc.blocks.len(), 1);
    assert_eq!(doc.section.page_width, 11906);
    write_package(&mut pkg, &doc).unwrap();
    assert_eq!(read_package(&pkg).unwrap().blocks, doc.blocks);

    let v = serde_json::to_value(Block::Paragraph(Paragraph::plain("x"))).unwrap();
    assert_eq!(v["type"], "paragraph");
    assert_eq!(v["inlines"][0]["type"], "text");
    let back: Block = serde_json::from_value(v).unwrap();
    assert_eq!(back, Block::Paragraph(Paragraph::plain("x")));
    // camelCase + defaults when the UI omits fields
    let p: Paragraph =
        serde_json::from_str(r#"{"inlines":[],"spaceBefore":10,"list":{"numId":9001,"level":0}}"#)
            .unwrap();
    assert_eq!(p.format.space_before, Some(10));
    assert_eq!(p.list.unwrap().num_id, 9001);
}

#[test]
fn rejects_non_documents() {
    let pkg = templates::blank(DocumentKind::Workbook);
    assert!(matches!(read_package(&pkg), Err(Error::NotADocument)));
}
