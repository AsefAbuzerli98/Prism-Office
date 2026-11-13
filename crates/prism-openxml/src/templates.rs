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

/// A placeholder shape: `(type, idx)`, optional position (EMU) and body properties.
#[allow(clippy::too_many_arguments)]
fn ph_sp(
    id: u32,
    name: &str,
    ph: &str,
    idx: Option<u32>,
    xfrm: Option<(i64, i64, i64, i64)>,
    body: &str,
    lst: &str,
    text: &str,
) -> String {
    let idx = idx.map(|i| format!(r#" idx="{i}""#)).unwrap_or_default();
    let ty = if ph.is_empty() {
        String::new()
    } else {
        format!(r#" type="{ph}""#)
    };
    let xf = xfrm
        .map(|(x, y, cx, cy)| {
            format!(r#"<a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#)
        })
        .unwrap_or_default();
    let geom = if xfrm.is_some() {
        r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#
    } else {
        ""
    };
    // `body` is either bare attributes or `attrs><children` (closed explicitly).
    let body_el = if body.contains('<') {
        format!("<a:bodyPr{body}</a:bodyPr>")
    } else {
        format!("<a:bodyPr{body}/>")
    };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph{ty}{idx}/></p:nvPr></p:nvSpPr><p:spPr>{xf}{geom}</p:spPr><p:txBody>{body_el}<a:lstStyle>{lst}</a:lstStyle><a:p>{text}<a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp>"#
    )
}

fn blank_presentation() -> Package {
    const SP_TREE_HEAD: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;
    const LAYOUT_CT: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
    let mut p = Package::new();
    root_rels(&mut p, "ppt/presentation.xml");
    doc_props(&mut p);

    // (name, type, shapes)
    let title = |id, xf| ph_sp(id, "Title", "title", None, xf, "", "", "");
    let body = |id, idx, xf| ph_sp(id, "Content Placeholder", "", Some(idx), xf, "", "", "");
    let layouts: Vec<(&str, &str, String)> = vec![
        (
            "Title Slide",
            "title",
            format!(
                "{}{}",
                ph_sp(
                    2,
                    "Title 1",
                    "ctrTitle",
                    None,
                    Some((1524000, 1122363, 9144000, 2387600)),
                    r#" anchor="b""#,
                    r#"<a:lvl1pPr algn="ctr"><a:defRPr sz="6000"/></a:lvl1pPr>"#,
                    ""
                ),
                ph_sp(
                    3,
                    "Subtitle 2",
                    "subTitle",
                    Some(1),
                    Some((1524000, 3602038, 9144000, 1655762)),
                    "",
                    r#"<a:lvl1pPr marL="0" indent="0" algn="ctr"><a:buNone/><a:defRPr sz="2400"/></a:lvl1pPr>"#,
                    ""
                )
            ),
        ),
        (
            "Title and Content",
            "obj",
            format!("{}{}", title(2, None), body(3, 1, None)),
        ),
        (
            "Section Header",
            "secHead",
            format!(
                "{}{}",
                ph_sp(
                    2,
                    "Title 1",
                    "title",
                    None,
                    Some((831850, 1709738, 10515600, 2852737)),
                    r#" anchor="b""#,
                    r#"<a:lvl1pPr><a:defRPr sz="6000"/></a:lvl1pPr>"#,
                    ""
                ),
                ph_sp(
                    3,
                    "Text Placeholder 2",
                    "body",
                    Some(1),
                    Some((831850, 4589463, 10515600, 1500187)),
                    "",
                    r#"<a:lvl1pPr marL="0" indent="0"><a:buNone/><a:defRPr sz="2400"><a:solidFill><a:schemeClr val="tx1"><a:tint val="75000"/></a:schemeClr></a:solidFill></a:defRPr></a:lvl1pPr>"#,
                    ""
                )
            ),
        ),
        (
            "Two Content",
            "twoObj",
            format!(
                "{}{}{}",
                title(2, None),
                body(3, 1, Some((838200, 1825625, 5181600, 4351338))),
                body(4, 2, Some((6172200, 1825625, 5181600, 4351338)))
            ),
        ),
        ("Title Only", "titleOnly", title(2, None)),
        ("Blank", "blank", String::new()),
    ];
    let mut layout_ids = String::new();
    let mut master_rels: Vec<(String, String)> = Vec::new();
    for (i, (name, ty, shapes)) in layouts.iter().enumerate() {
        let n = i + 1;
        add(
            &mut p,
            &format!("/ppt/slideLayouts/slideLayout{n}.xml"),
            LAYOUT_CT,
            format!(
                r#"{DECL}<p:sldLayout xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}" type="{ty}" preserve="1"><p:cSld name="{name}"><p:spTree>{SP_TREE_HEAD}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
            ),
        );
        rels(
            &mut p,
            Some(&format!("/ppt/slideLayouts/slideLayout{n}.xml")),
            &[(rel_types::SLIDE_MASTER, "../slideMasters/slideMaster1.xml")],
        );
        layout_ids.push_str(&format!(
            r#"<p:sldLayoutId id="{}" r:id="rId{n}"/>"#,
            2147483648u64 + n as u64
        ));
        master_rels.push((
            rel_types::SLIDE_LAYOUT.to_string(),
            format!("../slideLayouts/slideLayout{n}.xml"),
        ));
    }
    master_rels.push((
        rel_types::THEME.to_string(),
        "../theme/theme1.xml".to_string(),
    ));

    let lvl = |n: u32, mar: i64, sz: u32| {
        format!(
            r#"<a:lvl{n}pPr marL="{mar}" indent="-228600" algn="l" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPts val="{}"/></a:spcBef><a:buFont typeface="Arial"/><a:buChar char="•"/><a:defRPr sz="{sz}" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl{n}pPr>"#,
            if n == 1 { 1000 } else { 500 }
        )
    };
    let body_style: String = [
        (1, 228600, 2800),
        (2, 685800, 2400),
        (3, 1143000, 2000),
        (4, 1600200, 1800),
        (5, 2057400, 1800),
        (6, 2514600, 1800),
        (7, 2971800, 1800),
        (8, 3429000, 1800),
        (9, 3886200, 1800),
    ]
    .iter()
    .map(|(n, m, s)| lvl(*n, *m, *s))
    .collect();
    let other_style: String = (1..=9).map(|n| format!(r#"<a:lvl{n}pPr marL="{}" algn="l"><a:defRPr sz="1800" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl{n}pPr>"#, (n - 1) * 457200)).collect();
    let master_shapes = format!(
        "{}{}{}{}{}",
        ph_sp(
            2,
            "Title Placeholder 1",
            "title",
            None,
            Some((838200, 365125, 10515600, 1325563)),
            r#" vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0" anchor="ctr"><a:normAutofit/>"#,
            "",
            r#"<a:r><a:rPr lang="en-US"/><a:t>Click to edit Master title style</a:t></a:r>"#
        ),
        ph_sp(
            3,
            "Text Placeholder 2",
            "body",
            Some(1),
            Some((838200, 1825625, 10515600, 4351338)),
            r#" vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0"><a:normAutofit/>"#,
            "",
            r#"<a:pPr lvl="0"/><a:r><a:rPr lang="en-US"/><a:t>Click to edit Master text styles</a:t></a:r>"#
        ),
        ph_sp(
            4,
            "Date Placeholder 3",
            "dt",
            Some(2),
            Some((838200, 6356350, 2743200, 365125)),
            r#" anchor="ctr""#,
            r#"<a:lvl1pPr algn="l"><a:defRPr sz="1200"><a:solidFill><a:schemeClr val="tx1"><a:tint val="75000"/></a:schemeClr></a:solidFill></a:defRPr></a:lvl1pPr>"#,
            ""
        ),
        ph_sp(
            5,
            "Footer Placeholder 4",
            "ftr",
            Some(3),
            Some((4038600, 6356350, 4114800, 365125)),
            r#" anchor="ctr""#,
            r#"<a:lvl1pPr algn="ctr"><a:defRPr sz="1200"><a:solidFill><a:schemeClr val="tx1"><a:tint val="75000"/></a:schemeClr></a:solidFill></a:defRPr></a:lvl1pPr>"#,
            ""
        ),
        ph_sp(
            6,
            "Slide Number Placeholder 5",
            "sldNum",
            Some(4),
            Some((8610600, 6356350, 2743200, 365125)),
            r#" anchor="ctr""#,
            r#"<a:lvl1pPr algn="r"><a:defRPr sz="1200"><a:solidFill><a:schemeClr val="tx1"><a:tint val="75000"/></a:schemeClr></a:solidFill></a:defRPr></a:lvl1pPr>"#,
            ""
        ),
    );
    let title_style = r#"<a:lvl1pPr algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPct val="0"/></a:spcBef><a:buNone/><a:defRPr sz="4400" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/><a:ea typeface="+mj-ea"/><a:cs typeface="+mj-cs"/></a:defRPr></a:lvl1pPr>"#;
    add(
        &mut p,
        "/ppt/slideMasters/slideMaster1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml",
        format!(
            r#"{DECL}<p:sldMaster xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{SP_TREE_HEAD}{master_shapes}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst>{layout_ids}</p:sldLayoutIdLst><p:txStyles><p:titleStyle>{title_style}</p:titleStyle><p:bodyStyle>{body_style}</p:bodyStyle><p:otherStyle><a:defPPr><a:defRPr lang="en-US"/></a:defPPr>{other_style}</p:otherStyle></p:txStyles></p:sldMaster>"#
        ),
    );
    let mr: Vec<(&str, &str)> = master_rels
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    rels(&mut p, Some("/ppt/slideMasters/slideMaster1.xml"), &mr);

    add(
        &mut p,
        "/ppt/presentation.xml",
        DocumentKind::Presentation.main_content_type(),
        format!(
            r#"{DECL}<p:presentation xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}" saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle><a:defPPr><a:defRPr lang="en-US"/></a:defPPr></p:defaultTextStyle></p:presentation>"#
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
    let slide_shapes = format!(
        "{}{}",
        ph_sp(2, "Title 1", "ctrTitle", None, None, "", "", ""),
        ph_sp(3, "Subtitle 2", "subTitle", Some(1), None, "", "", "")
    );
    add(
        &mut p,
        "/ppt/slides/slide1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
        format!(
            r#"{DECL}<p:sld xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:cSld><p:spTree>{SP_TREE_HEAD}{slide_shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
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
