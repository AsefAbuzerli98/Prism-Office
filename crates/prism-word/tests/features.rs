//! Tables, pictures, notes, comments, tracked changes, headers, settings,
//! sources, tab stops, bidi and columns.

use prism_openxml::{rel_types, templates, DocumentKind, Package, PartName, TargetMode};
use prism_word::*;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b.extend([8, 6, 0, 0, 0]);
    b
}

fn rich_package() -> Package {
    let mut pkg = templates::blank(DocumentKind::Document);
    let main = PartName::new("/word/document.xml").unwrap();
    let mut rels = pkg.relationships(Some(&main)).unwrap();
    let img = rels.add(rel_types::IMAGE, "media/image1.png", TargetMode::Internal);
    let foot = rels.add(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes",
        "footnotes.xml",
        TargetMode::Internal,
    );
    let com = rels.add(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
        "comments.xml",
        TargetMode::Internal,
    );
    let hdr = rels.add(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header",
        "header1.xml",
        TargetMode::Internal,
    );
    let set = rels.add(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings",
        "settings.xml",
        TargetMode::Internal,
    );
    let _ = (foot, com, set);
    pkg.set_relationships(Some(&main), &rels);
    pkg.content_types_mut().set_default("png", "image/png");
    pkg.set_part(
        PartName::new("/word/media/image1.png").unwrap(),
        png(200, 100),
        None,
    );

    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{W}" xmlns:r="{R}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>
<w:p><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9350"/></w:tabs><w:bidi/></w:pPr><w:r><w:rPr><w:rtl/></w:rPr><w:t>مرحبا</w:t></w:r><w:r><w:tab/><w:t>7</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">See </w:t></w:r><w:r><w:drawing><wp:inline><wp:extent cx="1905000" cy="952500"/><wp:docPr id="1" name="Picture 1" descr="old alt"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="{img}"/></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1905000" cy="952500"/></a:xfrm></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="1"/></w:r>
<w:commentRangeStart w:id="0"/><w:r><w:t>commented</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r>
<w:ins w:id="5" w:author="Ana" w:date="2026-01-01T00:00:00Z"><w:r><w:t>added</w:t></w:r></w:ins><w:del w:id="6" w:author="Ana"><w:r><w:delText>removed</w:delText></w:r></w:del>
<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>
<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/></w:tblGrid>
<w:tr><w:trPr><w:tblHeader/><w:cantSplit/></w:trPr><w:tc><w:tcPr><w:tcW w:w="6000" w:type="dxa"/><w:gridSpan w:val="2"/><w:shd w:val="clear" w:color="auto" w:fill="DDEEFF"/><w:vAlign w:val="center"/></w:tcPr><w:p><w:r><w:t>Head</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>Tall</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>nested</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/></w:tc><w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc></w:tr></w:tbl>
<w:p/>
<w:sectPr><w:headerReference w:type="default" r:id="{hdr}"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/><w:cols w:num="2" w:space="567"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set_part(main.clone(), doc.into_bytes(), None);
    pkg.set_part(
        PartName::new("/word/footnotes.xml").unwrap(),
        format!(r#"<w:footnotes xmlns:w="{W}"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote><w:footnote w:id="1"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Note text</w:t></w:r></w:p></w:footnote></w:footnotes>"#).into_bytes(),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"),
    );
    pkg.set_part(
        PartName::new("/word/comments.xml").unwrap(),
        format!(r#"<w:comments xmlns:w="{W}"><w:comment w:id="0" w:author="Ana" w:initials="A" w:date="2026-01-02T00:00:00Z"><w:p><w:r><w:t>Check this</w:t></w:r></w:p></w:comment></w:comments>"#).into_bytes(),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"),
    );
    pkg.set_part(
        PartName::new("/word/header1.xml").unwrap(),
        format!(r#"<w:hdr xmlns:w="{W}" xmlns:r="{R}"><w:p><w:r><w:t>Company header</w:t></w:r></w:p></w:hdr>"#).into_bytes(),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"),
    );
    pkg.set_part(
        PartName::new("/word/settings.xml").unwrap(),
        format!(r#"<w:settings xmlns:w="{W}"><w:zoom w:percent="100"/><w:trackRevisions/><w:defaultTabStop w:val="708"/></w:settings>"#).into_bytes(),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"),
    );
    pkg
}

fn para(b: &Block) -> &Paragraph {
    match b {
        Block::Paragraph(p) => p,
        other => panic!("{other:?}"),
    }
}

#[test]
fn reads_all_features() {
    let doc = read_package(&rich_package()).unwrap();
    let p0 = para(&doc.blocks[0]);
    assert!(p0.format.bidi);
    assert_eq!(
        p0.format.tabs,
        vec![TabStop {
            pos: 9350,
            align: TabAlign::Right,
            leader: Some('.')
        }]
    );
    assert!(matches!(&p0.inlines[0], Inline::Text { marks, .. } if marks.rtl == Some(true)));

    let p1 = para(&doc.blocks[1]);
    let img = p1.inlines.iter().find_map(|i| match i {
        Inline::Image {
            rel_id,
            cx,
            cy,
            descr,
            ..
        } => Some((rel_id.clone(), *cx, *cy, descr.clone())),
        _ => None,
    });
    assert_eq!(
        img.as_ref().map(|i| (i.1, i.2, i.3.as_str())),
        Some((1905000, 952500, "old alt"))
    );
    let pkg = rich_package();
    let (mime, bytes) = image_data(&pkg, &img.unwrap().0).unwrap().unwrap();
    assert_eq!(
        (mime.as_str(), bytes.len()),
        ("image/png", png(200, 100).len())
    );

    assert!(p1.inlines.iter().any(|i| matches!(
        i,
        Inline::Note {
            kind: NoteKind::Footnote,
            id: 1,
            ..
        }
    )));
    assert!(p1
        .inlines
        .iter()
        .any(|i| matches!(i, Inline::Comment { id, edge: CommentEdge::Start } if id == "0")));
    assert!(p1.inlines.iter().any(|i| matches!(
        i,
        Inline::Comment {
            edge: CommentEdge::Reference,
            ..
        }
    )));
    let revs: Vec<_> = p1
        .inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Text { text, marks } => marks
                .revision
                .as_ref()
                .map(|r| (text.as_str(), r.kind, r.author.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        revs,
        vec![
            ("added", RevisionKind::Insert, "Ana"),
            ("removed", RevisionKind::Delete, "Ana")
        ]
    );
    // field code runs are invisible; the result "1" is visible text
    assert!(p1.text().ends_with("addedremoved1"), "{:?}", p1.text());

    match &doc.blocks[2] {
        Block::Table(t) => {
            assert!(t.borders);
            assert_eq!(t.grid, vec![3000, 3000, 3000]);
            assert!(t.rows[0].header);
            assert!(t.rows[0].props.as_deref().unwrap().contains("cantSplit"));
            let c = &t.rows[0].cells[0];
            assert_eq!(
                (c.span, c.width, c.shading.as_deref()),
                (2, Some(6000), Some("DDEEFF"))
            );
            assert_eq!(c.extra.len(), 1, "vAlign preserved");
            assert_eq!(t.rows[0].cells[1].v_merge, Some(VMerge::Restart));
            assert_eq!(t.rows[1].cells[2].v_merge, Some(VMerge::Continue));
            assert!(matches!(&t.rows[1].cells[0].blocks[1], Block::Table(n) if !n.borders));
        }
        b => panic!("{b:?}"),
    }

    assert_eq!(
        doc.section.columns,
        Some(Columns {
            count: 2,
            space: 567,
            separator: false
        })
    );
    assert_eq!(doc.notes.len(), 1);
    assert_eq!(
        WordDocument {
            blocks: doc.notes[0].blocks.clone(),
            ..Default::default()
        }
        .plain_text(),
        " Note text"
    );
    assert_eq!(doc.comments[0].author, "Ana");
    assert_eq!(doc.headers.len(), 1);
    assert_eq!(doc.headers[0].kind, HeaderKind::Header);
    assert!(doc.settings.track_revisions);
    assert!(doc.plain_text().contains("Head\tTall"));
}

#[test]
fn unchanged_roundtrip_keeps_secondary_parts_byte_identical() {
    let mut pkg = rich_package();
    let before = pkg.clone();
    let doc = read_package(&pkg).unwrap();
    write_package(&mut pkg, &doc).unwrap();
    for part in [
        "word/footnotes.xml",
        "word/comments.xml",
        "word/header1.xml",
        "word/settings.xml",
    ] {
        assert_eq!(
            pkg.part_str(part),
            before.part_str(part),
            "{part} must be untouched"
        );
    }
    let doc2 = read_package(&Package::from_bytes(&pkg.to_bytes().unwrap()).unwrap()).unwrap();
    assert_eq!(doc.blocks, doc2.blocks);
    assert_eq!(doc.section, doc2.section);
    let xml = String::from_utf8(pkg.part_str("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains("<w:delText xml:space=\"preserve\">removed</w:delText>"));
    assert!(xml.contains(r#"<w:ins w:id="5" w:author="Ana" w:date="2026-01-01T00:00:00Z">"#));
    assert!(pkg.validate().is_empty(), "{:#?}", pkg.validate());
}

#[test]
fn edits_to_every_feature_are_written() {
    let mut pkg = rich_package();
    let mut doc = read_package(&pkg).unwrap();

    // picture: change alt text and size; add a new picture
    let (rel, cx, cy) = add_image(&mut pkg, png(1600, 800)).unwrap();
    assert_eq!(
        (cx, cy),
        (5_486_400, 2_743_200),
        "clamped to 6in keeping aspect"
    );
    if let Block::Paragraph(p) = &mut doc.blocks[1] {
        for i in &mut p.inlines {
            if let Inline::Image { descr, cx, .. } = i {
                *descr = "new alt".into();
                *cx = 952500;
            }
        }
        p.inlines.push(Inline::Image {
            rel_id: rel.clone(),
            cx,
            cy,
            descr: "chart".into(),
            xml: String::new(),
        });
        p.inlines.push(Inline::Note {
            kind: NoteKind::Footnote,
            id: 2,
            xml: String::new(),
        });
        p.inlines.push(Inline::Shape {
            preset: "ellipse".into(),
            cx: 914400,
            cy: 457200,
            stroke: "FF0000".into(),
            fill: Some("FFFF00".into()),
            xml: String::new(),
        });
        p.inlines.push(Inline::Comment {
            id: "1".into(),
            edge: CommentEdge::Reference,
        });
    }
    doc.notes.push(Note {
        kind: NoteKind::Footnote,
        id: 2,
        blocks: vec![Block::Paragraph(Paragraph::plain("Second note"))],
    });
    doc.comments.push(Comment {
        id: "1".into(),
        author: "Prism".into(),
        initials: "P".into(),
        date: None,
        blocks: vec![Block::Paragraph(Paragraph::plain("New comment"))],
    });
    doc.headers[0].blocks = vec![Block::Paragraph(Paragraph::plain("Edited header"))];
    doc.settings.track_revisions = false;
    doc.settings.auto_hyphenation = true;
    doc.sources.push(Source {
        tag: "Doe24".into(),
        kind: "Report".into(),
        title: "AI & Docs".into(),
        authors: vec!["Doe, J".into()],
        year: "2024".into(),
        ..Default::default()
    });
    if let Block::Table(t) = &mut doc.blocks[2] {
        t.rows[1].cells[1].shading = Some("FFEEDD".into());
        t.rows[0].header = false;
    }
    doc.blocks.push(Block::Table(Table {
        grid: vec![2000, 2000],
        borders: true,
        rows: vec![TableRow {
            cells: vec![TableCell::default(), TableCell::default()],
            ..Default::default()
        }],
        props: None,
    }));
    doc.section.columns = None;
    write_package(&mut pkg, &doc).unwrap();

    let bytes = pkg.to_bytes().unwrap();
    let pkg2 = Package::from_bytes(&bytes).unwrap();
    assert!(pkg2.validate().is_empty(), "{:#?}", pkg2.validate());
    let back = read_package(&pkg2).unwrap();

    let imgs: Vec<_> = para(&back.blocks[1])
        .inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Image { descr, cx, .. } => Some((descr.clone(), *cx)),
            _ => None,
        })
        .collect();
    assert_eq!(
        imgs,
        vec![
            ("new alt".to_string(), 952500),
            ("chart".to_string(), 5_486_400)
        ]
    );
    assert!(para(&back.blocks[1]).inlines.iter().any(|i| matches!(i,
        Inline::Shape { preset, cx: 914400, cy: 457200, stroke, fill: Some(f), .. } if preset == "ellipse" && stroke == "FF0000" && f == "FFFF00")));
    assert_eq!(back.notes.len(), 2);
    assert_eq!(back.comments.len(), 2);
    assert_eq!(
        WordDocument {
            blocks: back.headers[0].blocks.clone(),
            ..Default::default()
        }
        .plain_text(),
        "Edited header"
    );
    assert_eq!(
        back.settings,
        DocSettings {
            track_revisions: false,
            auto_hyphenation: true
        }
    );
    assert_eq!(back.sources, doc.sources);
    let footnotes =
        String::from_utf8(pkg2.part_str("word/footnotes.xml").unwrap().to_vec()).unwrap();
    assert!(
        footnotes.contains(r#"w:type="separator""#),
        "separators kept"
    );
    match (
        &back.blocks[2],
        back.blocks
            .iter()
            .rev()
            .find(|b| matches!(b, Block::Table(_))),
    ) {
        (Block::Table(t), Some(Block::Table(new))) => {
            assert_eq!(t.rows[1].cells[1].shading.as_deref(), Some("FFEEDD"));
            assert!(!t.rows[0].header);
            assert!(t.rows[0].props.as_deref().unwrap().contains("cantSplit"));
            assert!(new.borders);
            assert_eq!(new.rows[0].cells.len(), 2);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(back.section.columns, None);
    // idempotent second save
    let mut pkg3 = pkg2.clone();
    write_package(&mut pkg3, &back).unwrap();
    let a = String::from_utf8_lossy(pkg3.part_str("word/document.xml").unwrap()).into_owned();
    let b = String::from_utf8_lossy(pkg2.part_str("word/document.xml").unwrap()).into_owned();
    if a != b {
        let i = a
            .bytes()
            .zip(b.bytes())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()));
        panic!(
            "second save differs at {i}:\n  {}\n  {}",
            &a[i.saturating_sub(120)..(i + 120).min(a.len())],
            &b[i.saturating_sub(120)..(i + 120).min(b.len())]
        );
    }
}

#[test]
fn blank_document_gains_parts_on_demand() {
    let mut pkg = templates::blank(DocumentKind::Document);
    let mut doc = read_package(&pkg).unwrap();
    doc.notes.push(Note {
        kind: NoteKind::Endnote,
        id: 1,
        blocks: vec![Block::Paragraph(Paragraph::plain("End"))],
    });
    doc.comments.push(Comment {
        id: "0".into(),
        author: "A".into(),
        initials: "A".into(),
        date: None,
        blocks: vec![Block::Paragraph(Paragraph::plain("c"))],
    });
    doc.settings.track_revisions = true;
    doc.sources.push(Source {
        tag: "X".into(),
        title: "T".into(),
        ..Default::default()
    });
    doc.headers.push(HeaderFooter {
        kind: HeaderKind::Footer,
        which: "default".into(),
        rel_id: String::new(),
        blocks: vec![Block::Paragraph(Paragraph::plain("Page footer"))],
    });
    write_package(&mut pkg, &doc).unwrap();
    assert!(pkg.validate().is_empty(), "{:#?}", pkg.validate());
    let back = read_package(&pkg).unwrap();
    assert_eq!(back.notes.len(), 1);
    assert_eq!(back.comments.len(), 1);
    assert!(back.settings.track_revisions);
    assert_eq!(back.sources.len(), 1);
    assert_eq!(back.headers.len(), 1);
    assert_eq!(back.headers[0].kind, HeaderKind::Footer);
    assert_eq!(
        WordDocument {
            blocks: back.headers[0].blocks.clone(),
            ..Default::default()
        }
        .plain_text(),
        "Page footer"
    );
}
