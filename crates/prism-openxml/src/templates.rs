//! Minimal blank packages for "New Document / Workbook / Presentation".

use crate::content_types::ct;
use crate::{rel_types, DocumentKind, Package, PartName, Relationships, TargetMode};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const S_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DECL: &str = crate::xml::XML_DECL;

fn pn(s: &str) -> PartName {
    PartName::new(s).expect("static part name")
}

fn add(pkg: &mut Package, name: &str, ct: &str, body: String) {
    pkg.set_part(pn(name), body.into_bytes(), Some(ct));
}

fn rels(pkg: &mut Package, source: Option<&str>, items: &[(&str, &str)]) {
    let mut r = Relationships::default();
    for (t, target) in items {
        r.add(t, target, TargetMode::Internal);
    }
    let src = source.map(pn);
    pkg.set_relationships(src.as_ref(), &r);
}

fn doc_props(pkg: &mut Package) {
    add(
        pkg,
        "/docProps/core.xml",
        ct::CORE_PROPERTIES,
        format!(
            r#"{DECL}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>Prism Office</dc:creator></cp:coreProperties>"#
        ),
    );
    add(
        pkg,
        "/docProps/app.xml",
        ct::EXTENDED_PROPERTIES,
        format!(
            r#"{DECL}<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Prism Office</Application></Properties>"#
        ),
    );
}

fn root_rels(pkg: &mut Package, main: &str) {
    rels(
        pkg,
        None,
        &[
            (rel_types::OFFICE_DOCUMENT, main),
            (rel_types::CORE_PROPERTIES, "docProps/core.xml"),
            (rel_types::EXTENDED_PROPERTIES, "docProps/app.xml"),
        ],
    );
}

/// Build a blank package of the requested kind.
pub fn blank(kind: DocumentKind) -> Package {
    match kind {
        DocumentKind::Document => blank_document(),
        DocumentKind::Workbook => blank_workbook(),
        DocumentKind::Presentation => blank_presentation(),
    }
}

fn blank_document() -> Package {
    let mut p = Package::new();
    root_rels(&mut p, "word/document.xml");
    doc_props(&mut p);
    add(
        &mut p,
        "/word/document.xml",
        DocumentKind::Document.main_content_type(),
        format!(
            r#"{DECL}<w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}"><w:body><w:p/><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>"#
        ),
    );
    add(
        &mut p,
        "/word/styles.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml",
        format!(
            r#"{DECL}<w:styles xmlns:w="{W_NS}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="Calibri" w:cs="Calibri"/><w:sz w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:keepNext/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style></w:styles>"#
        ),
    );
    rels(
        &mut p,
        Some("/word/document.xml"),
        &[(rel_types::STYLES, "styles.xml")],
    );
    p
}

fn blank_workbook() -> Package {
    let mut p = Package::new();
    root_rels(&mut p, "xl/workbook.xml");
    doc_props(&mut p);
    add(
        &mut p,
        "/xl/workbook.xml",
        DocumentKind::Workbook.main_content_type(),
        format!(
            r#"{DECL}<workbook xmlns="{S_NS}" xmlns:r="{R_NS}"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#
        ),
    );
    add(
        &mut p,
        "/xl/worksheets/sheet1.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
        format!(r#"{DECL}<worksheet xmlns="{S_NS}"><sheetData/></worksheet>"#),
    );
    add(
        &mut p,
        "/xl/styles.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
        format!(
            r#"{DECL}<styleSheet xmlns="{S_NS}"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#
        ),
    );
    rels(
        &mut p,
        Some("/xl/workbook.xml"),
        &[
            (rel_types::WORKSHEET, "worksheets/sheet1.xml"),
            (rel_types::STYLES, "styles.xml"),
        ],
    );
    p
}

fn theme_xml() -> String {
    let colors = [
        ("dk1", r#"<a:sysClr val="windowText" lastClr="000000"/>"#),
        ("lt1", r#"<a:sysClr val="window" lastClr="FFFFFF"/>"#),
        ("dk2", r#"<a:srgbClr val="1F2937"/>"#),
        ("lt2", r#"<a:srgbClr val="F3F4F6"/>"#),
        ("accent1", r#"<a:srgbClr val="C2553A"/>"#),
        ("accent2", r#"<a:srgbClr val="2563EB"/>"#),
        ("accent3", r#"<a:srgbClr val="059669"/>"#),
        ("accent4", r#"<a:srgbClr val="D97706"/>"#),
        ("accent5", r#"<a:srgbClr val="7C3AED"/>"#),
        ("accent6", r#"<a:srgbClr val="0891B2"/>"#),
        ("hlink", r#"<a:srgbClr val="2563EB"/>"#),
        ("folHlink", r#"<a:srgbClr val="7C3AED"/>"#),
    ];
    let clr: String = colors
        .iter()
        .map(|(n, c)| format!("<a:{n}>{c}</a:{n}>"))
        .collect();
    let solid = r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#;
    let fills = solid.repeat(3);
    let lines = format!(r#"<a:ln w="6350">{solid}</a:ln>"#).repeat(3);
    let effects = "<a:effectStyle><a:effectLst/></a:effectStyle>".repeat(3);
    format!(
        r#"{DECL}<a:theme xmlns:a="{A_NS}" name="Prism"><a:themeElements><a:clrScheme name="Prism">{clr}</a:clrScheme><a:fontScheme name="Prism"><a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Prism"><a:fillStyleLst>{fills}</a:fillStyleLst><a:lnStyleLst>{lines}</a:lnStyleLst><a:effectStyleLst>{effects}</a:effectStyleLst><a:bgFillStyleLst>{fills}</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
    )
}

fn blank_presentation() -> Package {
    const SP_TREE_HEAD: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#;
    let mut p = Package::new();
    root_rels(&mut p, "ppt/presentation.xml");
    doc_props(&mut p);

    add(
        &mut p,
        "/ppt/presentation.xml",
        DocumentKind::Presentation.main_content_type(),
        format!(
            r#"{DECL}<p:presentation xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
        ),
    );
    rels(
        &mut p,
        Some("/ppt/presentation.xml"),
        &[
            (rel_types::SLIDE_MASTER, "slideMasters/slideMaster1.xml"),
            (rel_types::SLIDE, "slides/slide1.xml"),
            (rel_types::THEME, "theme/theme1.xml"),
        ],
    );

    add(
        &mut p,
        "/ppt/slideMasters/slideMaster1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml",
        format!(
            r#"{DECL}<p:sldMaster xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:cSld><p:spTree>{SP_TREE_HEAD}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst></p:sldMaster>"#
        ),
    );
    rels(
        &mut p,
        Some("/ppt/slideMasters/slideMaster1.xml"),
        &[
            (rel_types::SLIDE_LAYOUT, "../slideLayouts/slideLayout1.xml"),
            (rel_types::THEME, "../theme/theme1.xml"),
        ],
    );

    add(
        &mut p,
        "/ppt/slideLayouts/slideLayout1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml",
        format!(
            r#"{DECL}<p:sldLayout xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>{SP_TREE_HEAD}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
        ),
    );
    rels(
        &mut p,
        Some("/ppt/slideLayouts/slideLayout1.xml"),
        &[(rel_types::SLIDE_MASTER, "../slideMasters/slideMaster1.xml")],
    );

    add(
        &mut p,
        "/ppt/slides/slide1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
        format!(
            r#"{DECL}<p:sld xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:cSld><p:spTree>{SP_TREE_HEAD}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
        ),
    );
    rels(
        &mut p,
        Some("/ppt/slides/slide1.xml"),
        &[(rel_types::SLIDE_LAYOUT, "../slideLayouts/slideLayout1.xml")],
    );

    add(
        &mut p,
        "/ppt/theme/theme1.xml",
        "application/vnd.openxmlformats-officedocument.theme+xml",
        theme_xml(),
    );
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_templates_are_valid_and_roundtrip() {
        for kind in DocumentKind::ALL {
            let pkg = blank(kind);
            let issues = pkg.validate();
            assert!(issues.is_empty(), "{kind:?}: {issues:#?}");
            let bytes = pkg.to_bytes().unwrap();
            let back = Package::from_bytes(&bytes).unwrap();
            assert_eq!(back.kind().unwrap(), kind);
            assert!(back.validate().is_empty());
            // every XML part must be well-formed
            for name in back.part_names() {
                let data = back.part(name).unwrap();
                let mut r = quick_xml::Reader::from_reader(data);
                let mut buf = Vec::new();
                loop {
                    match r.read_event_into(&mut buf) {
                        Ok(quick_xml::events::Event::Eof) => break,
                        Ok(_) => {}
                        Err(e) => panic!("{kind:?} {name}: {e}"),
                    }
                    buf.clear();
                }
            }
        }
    }
}
